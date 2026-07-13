//! Profile-aware preflight end-to-end tests.
//!
//! These exercise the project-agnostic behaviour introduced to fix the
//! "preflight assumes Rust/Cargo" defect (issue #2). The tests focus on the
//! gate *selection* logic (which gates a profile selects, and whether they
//! are run or Skipped), because the full `run_preflight` also invokes
//! `validate`/`regression_anchor` which shell out via `cargo run` and would
//! make every test re-compile the CLI. Selection + execution dispatch are
//! tested directly; the slow end-to-end path is covered by the existing
//! Rust-repo CI runs.
//!
//! Contract under test:
//! - On a non-Rust project, cargo gates are `Skipped`, never `Failed`.
//! - `--profile <name>` overrides detection.
//! - A resolved sidecar profile (or standalone `.forge-preflight.yaml`)
//!   supplies the gate list.
//! - `preflight init` writes to resolved sidecar state for a linked consumer.
//! - Standalone repositories use `.forge-preflight.yaml` without creating a
//!   bootstrap-conflicting state directory.

use forge_core_cli::preflight_cmd::{
    execute_builtin_gate, resolve_gate_specs, resolve_profile, GateOutcome, GateStatus,
    PreflightInput,
};
use forge_core_cli::project_profile::{
    GateSpec, PreflightProfileDocument, ProjectProfile, PREFLIGHT_PROFILE_FILE_NAME,
    PREFLIGHT_PROFILE_SCHEMA_VERSION,
};
use std::path::PathBuf;

fn tmp_project_dir(label: &str) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "forge-preflight-e2e-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn input_for(root: &std::path::Path) -> PreflightInput {
    PreflightInput {
        root: root.to_path_buf(),
        ..PreflightInput::default()
    }
}

#[test]
fn non_rust_project_resolves_to_generic_and_skips_cargo_gates() {
    // A bare directory with no manifest → Generic profile. The cargo gates
    // (type_check/clippy_pedantic/test/format) must be Skipped when dispatched
    // — never Failed. This is the core fix: previously such a project surfaced
    // a misleading "Cargo.toml not found" failure.
    let dir = tmp_project_dir("generic");

    let profile = resolve_profile(&input_for(&dir));
    assert_eq!(
        profile,
        ProjectProfile::Generic,
        "empty dir should detect Generic"
    );

    // The default gate set for Generic includes only language-agnostic gates.
    let specs = resolve_gate_specs(&input_for(&dir), profile);
    let names: Vec<&str> = specs.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["validate", "regression_anchor"]);

    // Each cargo gate, when executed under Generic, is Skipped.
    for cargo_gate in ["type_check", "clippy_pedantic", "test", "format"] {
        let outcome = execute_builtin_gate(cargo_gate, profile, &input_for(&dir));
        assert!(
            matches!(outcome, GateOutcome::Skipped),
            "cargo gate '{cargo_gate}' should be Skipped under Generic, got {outcome:?}"
        );
    }
}

#[test]
fn node_project_detected_from_package_json() {
    let dir = tmp_project_dir("node");
    std::fs::write(dir.join("package.json"), "{}").unwrap();

    let profile = resolve_profile(&input_for(&dir));
    assert_eq!(profile, ProjectProfile::Node);

    // Node profile skips cargo gates too.
    let outcome = execute_builtin_gate("type_check", profile, &input_for(&dir));
    assert!(matches!(outcome, GateOutcome::Skipped));
}

#[test]
fn rust_project_detected_from_cargo_toml() {
    let dir = tmp_project_dir("rust");
    std::fs::write(dir.join("Cargo.toml"), "").unwrap();

    let profile = resolve_profile(&input_for(&dir));
    assert_eq!(profile, ProjectProfile::Rust);

    // Under Rust, cargo gates are NOT skipped — they would be attempted.
    // We don't execute them (that would spawn cargo); we assert the gate set
    // includes them, which is the contract that the bug broke for non-Rust.
    let specs = resolve_gate_specs(&input_for(&dir), profile);
    let names: Vec<&str> = specs.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "type_check",
            "format",
            "clippy_pedantic",
            "test",
            "validate",
            "regression_anchor"
        ]
    );
}

#[test]
fn profile_override_forces_rust_gates_even_without_cargo_toml() {
    // `--profile rust` overrides detection. Under forced Rust the cargo gates
    // are dispatched (and would FAIL here because there's no Cargo.toml, but
    // they must NOT be Skipped — the override explicitly asked for Rust).
    let dir = tmp_project_dir("forced-rust");
    let input = PreflightInput {
        root: dir.clone(),
        profile_override: Some(ProjectProfile::Rust),
        ..PreflightInput::default()
    };
    let profile = resolve_profile(&input);
    assert_eq!(profile, ProjectProfile::Rust);

    let outcome = execute_builtin_gate("type_check", profile, &input);
    assert!(
        !matches!(outcome, GateOutcome::Skipped),
        "forced Rust profile must not Skip cargo gates (would run/fail, not skip)"
    );
}

#[test]
fn profile_document_pinned_in_preflight_yaml_supplies_gate_list() {
    // A pinned standalone `.forge-preflight.yaml` with a custom gate list
    // overrides the default gate set for the detected profile.
    let dir = tmp_project_dir("pinned");
    let profile_file = dir.join(".forge-preflight.yaml");

    let doc = PreflightProfileDocument {
        schema_version: PREFLIGHT_PROFILE_SCHEMA_VERSION.to_string(),
        profile: ProjectProfile::Generic,
        gates: vec![GateSpec::custom(
            "my_custom_gate".to_string(),
            vec!["echo".to_string(), "hello".to_string()],
            forge_core_cli::preflight_cmd::GateRequirement::Required,
        )],
    };
    let yaml = yaml_serde::to_string(&doc).unwrap();
    std::fs::write(profile_file, yaml).unwrap();

    let specs = resolve_gate_specs(&input_for(&dir), ProjectProfile::Generic);
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "my_custom_gate");
    assert!(!specs[0].is_builtin());
}

#[test]
fn preflight_init_writes_profile_document_for_detected_profile() {
    let dir = tmp_project_dir("init-rust");
    std::fs::write(dir.join("Cargo.toml"), "").unwrap();

    // Invoke the init subcommand path directly via the parser entry point.
    // args[0] is the command name ("preflight"); args[1] is the subcommand.
    let args = vec![
        "preflight".to_string(),
        "init".to_string(),
        "--root".to_string(),
        dir.to_string_lossy().into_owned(),
    ];
    forge_core_cli::preflight_cmd::run_preflight_command(&args).expect("init should succeed");

    let written = std::fs::read_to_string(dir.join(".forge-preflight.yaml"))
        .expect(".forge-preflight.yaml should exist after init");
    let doc: PreflightProfileDocument = yaml_serde::from_str(&written).unwrap();
    assert_eq!(doc.profile, ProjectProfile::Rust);
    assert_eq!(doc.schema_version, PREFLIGHT_PROFILE_SCHEMA_VERSION);
    assert!(!dir.join(".forge-method").exists());
    let names: Vec<&str> = doc.gates.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "type_check",
            "format",
            "clippy_pedantic",
            "test",
            "validate",
            "regression_anchor"
        ]
    );
}

#[test]
fn linked_consumer_preflight_init_writes_only_to_resolved_sidecar() {
    let parent = tmp_project_dir("linked-init");
    let app = parent.join("app");
    let sidecar = parent.join("forge-app");
    let state = sidecar.join(".forge-method");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(app.join("Cargo.toml"), "").unwrap();
    std::fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .unwrap();

    let args = vec![
        "preflight".to_string(),
        "init".to_string(),
        "--root".to_string(),
        app.to_string_lossy().into_owned(),
    ];
    forge_core_cli::preflight_cmd::run_preflight_command(&args).expect("init should succeed");

    assert!(state.join(PREFLIGHT_PROFILE_FILE_NAME).is_file());
    assert!(
        !app.join(".forge-method").exists(),
        "linked consumer must never receive local Forge state"
    );
    assert_eq!(resolve_profile(&input_for(&app)), ProjectProfile::Rust);
    let specs = resolve_gate_specs(&input_for(&app), ProjectProfile::Rust);
    assert_eq!(
        specs.first().map(|gate| gate.name.as_str()),
        Some("type_check")
    );
}

#[test]
fn invalid_project_link_never_falls_back_to_consumer_local_state() {
    let app = tmp_project_dir("invalid-link");
    std::fs::write(app.join(".forge-method.yaml"), "not: [valid").unwrap();
    let args = vec![
        "preflight".to_string(),
        "init".to_string(),
        "--root".to_string(),
        app.to_string_lossy().into_owned(),
    ];
    let error = forge_core_cli::preflight_cmd::run_preflight_command(&args)
        .expect_err("invalid Project Link must fail closed");
    assert!(error.to_string().contains("invalid Project Link"));
    assert!(!app.join(".forge-method").exists());
    assert!(!app.join(".forge-preflight.yaml").exists());
}

#[test]
fn missing_linked_sidecar_requires_complete_start_repair() {
    let parent = tmp_project_dir("missing-sidecar");
    let app = parent.join("app");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .unwrap();
    let args = vec![
        "preflight".to_string(),
        "init".to_string(),
        "--root".to_string(),
        app.to_string_lossy().into_owned(),
    ];
    let error = forge_core_cli::preflight_cmd::run_preflight_command(&args)
        .expect_err("missing linked state must require start repair");
    assert!(error.to_string().contains("forge-core start"));
    assert!(!parent.join("forge-app").exists());
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn malformed_existing_profile_fails_before_any_gate_runs() {
    let app = tmp_project_dir("malformed-profile");
    std::fs::write(app.join(".forge-preflight.yaml"), "schema_version: [").unwrap();
    let args = vec![
        "preflight".to_string(),
        "--root".to_string(),
        app.to_string_lossy().into_owned(),
        "--json".to_string(),
    ];
    let error = forge_core_cli::preflight_cmd::run_preflight_command(&args)
        .expect_err("malformed profile must fail closed");
    assert!(error.to_string().contains("invalid profile document"));
}

#[test]
fn preflight_run_on_non_rust_project_does_not_fail_on_cargo() {
    // End-to-end smoke: run_preflight on a bare dir. The cargo gates will be
    // Skipped. validate/regression_anchor may pass or fail (they shell out),
    // but the run must not panic and must produce a well-formed report whose
    // cargo gates are all Skipped.
    let dir = tmp_project_dir("run-generic");
    let input = PreflightInput {
        root: dir.clone(),
        // Restrict to a single gate that doesn't shell out to cargo validate,
        // to keep the test fast. The point is to confirm the profile-aware
        // dispatch path runs cleanly end-to-end.
        gates: vec![forge_core_cli::preflight_cmd::GateKind::Format],
        ..PreflightInput::default()
    };
    let report = forge_core_cli::preflight_cmd::run_preflight(&input);
    let format_gate = report
        .gates
        .iter()
        .find(|g| g.name == "format")
        .expect("format gate should be in the report");
    assert_eq!(
        format_gate.status,
        GateStatus::Skipped,
        "format gate under Generic profile must be Skipped, not Failed"
    );
}
