use forge_core_cli::run_validate;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn legacy_validator_and_forge_core_cli_both_pass_current_repo() {
    // The Forge core repo is now a normal consumer: its `.forge-method.yaml`
    // Project Link points at a sibling sidecar (`../forge-forge-method-core`)
    // that holds the runtime state, including `ledger.ndjson`. The completion
    // contracts reference `.forge-method/ledger.ndjson` as a repo-relative ref,
    // so a validation root must carry both the contract tree (from the repo)
    // and the ledger (from the sidecar) to pass cleanly. Build that merged
    // validation tree in a temp dir and run both validators against it.
    let source = repo_root();
    let temp = temp_repo_root("forge-parity-pass");
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("remove old temp repo");
    }
    copy_validation_tree(&source, &temp);

    let legacy = run_legacy_validator(&temp);
    assert!(
        legacy.status.success(),
        "legacy stderr: {}",
        String::from_utf8_lossy(&legacy.stderr)
    );
    let stdout = String::from_utf8_lossy(&legacy.stdout);
    assert!(
        stdout.starts_with("rust_contract_validation_passed "),
        "legacy stdout: {stdout}"
    );
    for field in [
        "yaml_files=",
        "gate_contracts=",
        "decision_contracts=",
        "runtime_contracts=",
        "tool_effect_contracts=",
        "request_contracts=",
        "eval_contracts=",
        "recovery_contracts=",
        "operation_policies=",
        "command_contracts=",
        "inventory_contracts=",
        "evidence_sources=",
        "operation_fixtures=",
    ] {
        assert!(
            stdout.contains(field),
            "legacy stdout missing {field}: {stdout}"
        );
    }

    let cli = run_validate(&temp);
    assert!(cli.passed(), "cli diagnostics: {:?}", cli.diagnostics);

    fs::remove_dir_all(&temp).expect("clean temp repo");
}

#[test]
fn legacy_validator_and_forge_core_cli_both_reject_unknown_source_id() {
    let source = repo_root();
    let temp = temp_repo_root("forge-source-id-parity");
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("remove old temp repo");
    }
    copy_dir(&source.join("contracts"), &temp.join("contracts"));
    copy_dir(
        &source
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
        &temp
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
    );
    let synthetic_policy = temp
        .join("contracts")
        .join("policies")
        .join("synthetic-unknown-source.yaml");
    fs::write(
        &synthetic_policy,
        r#"schema_version: "0.1"
policy: "synthetic_unknown_source"
evidence_basis:
  direct_patterns:
    - source_id: "missing_source_for_parity_test"
"#,
    )
    .expect("write synthetic policy");

    let legacy = run_legacy_validator(&temp);
    assert!(
        !legacy.status.success(),
        "legacy unexpectedly passed with stdout: {}",
        String::from_utf8_lossy(&legacy.stdout)
    );
    assert!(
        String::from_utf8_lossy(&legacy.stderr).contains("missing_source_for_parity_test"),
        "legacy stderr: {}",
        String::from_utf8_lossy(&legacy.stderr)
    );

    let cli = run_validate(&temp);
    assert!(!cli.passed(), "cli unexpectedly passed");
    assert!(
        cli.diagnostics
            .iter()
            .any(|item| item.message.contains("missing_source_for_parity_test")),
        "cli diagnostics: {:?}",
        cli.diagnostics
    );

    fs::remove_dir_all(&temp).expect("clean temp repo");
}

#[test]
fn legacy_validator_and_forge_core_cli_both_reject_missing_policy_ref() {
    let source = repo_root();
    let temp = temp_repo_root("forge-known-ref-parity");
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("remove old temp repo");
    }
    copy_validation_tree(&source, &temp);

    let inventory_path = temp
        .join("contracts")
        .join("inventory")
        .join("v0-contract-family-lock.yaml");
    let inventory = fs::read_to_string(&inventory_path).expect("read inventory");
    let inventory = inventory.replace(
        "    - \"contracts/policies/rust-validation-authority.yaml\"",
        "    - \"contracts/policies/rust-validation-authority.yaml\"\n    - \"contracts/policies/missing-policy-for-parity.yaml\"",
    );
    fs::write(&inventory_path, inventory).expect("write inventory");

    let legacy = run_legacy_validator(&temp);
    assert!(
        !legacy.status.success(),
        "legacy unexpectedly passed with stdout: {}",
        String::from_utf8_lossy(&legacy.stdout)
    );
    assert!(
        String::from_utf8_lossy(&legacy.stderr).contains("missing-policy-for-parity.yaml"),
        "legacy stderr: {}",
        String::from_utf8_lossy(&legacy.stderr)
    );

    let cli = run_validate(&temp);
    assert!(!cli.passed(), "cli unexpectedly passed");
    assert!(
        cli.diagnostics
            .iter()
            .any(|item| item.message.contains("missing-policy-for-parity.yaml")),
        "cli diagnostics: {:?}",
        cli.diagnostics
    );

    fs::remove_dir_all(&temp).expect("clean temp repo");
}

#[test]
fn legacy_validator_and_forge_core_cli_both_reject_invalid_operation_enum() {
    let source = repo_root();
    let temp = temp_repo_root("forge-operation-enum-parity");
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("remove old temp repo");
    }
    copy_validation_tree(&source, &temp);

    let operation_path = temp
        .join("docs")
        .join("fixtures")
        .join("operation-contract-v0")
        .join("mechanical-story-execute.yaml");
    let operation = fs::read_to_string(&operation_path).expect("read operation fixture");
    let operation = operation.replace(
        "    mode: \"execute\"",
        "    mode: \"invalid_autonomy_mode\"",
    );
    fs::write(&operation_path, operation).expect("write operation fixture");

    assert_both_reject(&temp, "invalid_autonomy_mode");

    fs::remove_dir_all(&temp).expect("clean temp repo");
}

#[test]
fn legacy_validator_and_forge_core_cli_both_reject_invalid_command_enum() {
    let source = repo_root();
    let temp = temp_repo_root("forge-command-enum-parity");
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("remove old temp repo");
    }
    copy_validation_tree(&source, &temp);

    let command_path = temp
        .join("contracts")
        .join("commands")
        .join("story-validation-fast.yaml");
    let command = fs::read_to_string(&command_path).expect("read command contract");
    let command = command.replace("  kind: \"test\"", "  kind: \"invalid_command_kind\"");
    fs::write(&command_path, command).expect("write command contract");

    assert_both_reject(&temp, "invalid_command_kind");

    fs::remove_dir_all(&temp).expect("clean temp repo");
}

fn run_legacy_validator(root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-contract-validator"))
        .arg(root)
        .output()
        .expect("run legacy validator")
}

fn assert_both_reject(root: &Path, expected_text: &str) {
    let legacy = run_legacy_validator(root);
    assert!(
        !legacy.status.success(),
        "legacy unexpectedly passed with stdout: {}",
        String::from_utf8_lossy(&legacy.stdout)
    );
    assert!(
        String::from_utf8_lossy(&legacy.stderr).contains(expected_text),
        "legacy stderr: {}",
        String::from_utf8_lossy(&legacy.stderr)
    );

    let cli = run_validate(root);
    assert!(!cli.passed(), "cli unexpectedly passed");
    assert!(
        cli.diagnostics
            .iter()
            .any(|item| item.message.contains(expected_text)),
        "cli diagnostics: {:?}",
        cli.diagnostics
    );
}

fn copy_validation_tree(source: &Path, target: &Path) {
    copy_dir(&source.join("contracts"), &target.join("contracts"));
    copy_dir(
        &source
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
        &target
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
    );
    // completion contracts reference the append-only ledger; copy it so
    // cross-refs resolve. The core repo's runtime state (including the ledger)
    // now lives in its sibling sidecar via the Project Link at
    // `.forge-method.yaml`, so resolve the ledger there first and fall back to
    // the repo root's own `.forge-method/` for repos that still ship it locally.
    let ledger_source = [
        source.join("../forge-forge-method-core/.forge-method/ledger.ndjson"),
        source.join(".forge-method").join("ledger.ndjson"),
    ]
    .into_iter()
    .find(|candidate| candidate.exists());
    if let Some(ledger_source) = ledger_source {
        let ledger_target = target.join(".forge-method").join("ledger.ndjson");
        fs::create_dir_all(ledger_target.parent().expect("ledger parent"))
            .expect("create .forge-method dir");
        fs::copy(&ledger_source, &ledger_target).expect("copy ledger.ndjson");
    }
}

fn temp_repo_root(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nonce}"))
}

fn copy_dir(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create target dir");
    for entry in fs::read_dir(source).expect("read source dir") {
        let entry = entry.expect("dir entry");
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &target_path);
        } else {
            fs::copy(&source_path, &target_path).expect("copy file");
        }
    }
}
