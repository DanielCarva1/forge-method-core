use assert_cmd::Command;
use forge_core_contracts::{
    CliEnvelope, SoloHostAssertionStatus, SoloHostCapability, SoloHostConformanceOutcome,
    SoloHostConformanceResultDocument, SoloHostGapKind,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "forge-host-conformance-{}-{}-{sequence}",
        std::process::id(),
        label
    ))
}

fn reference_adapter() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/hosts/solo-host-conformance-v1/reference-adapter.py")
}

fn codex_adapter() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/hosts/codex/solo-host-conformance-v1/adapter.py")
}

fn python_program() -> &'static str {
    if cfg!(windows) {
        "python"
    } else {
        "python3"
    }
}

fn run_command(bundle: &Path, mode: Option<&str>, timeout_ms: Option<u64>) -> Command {
    let mut command = Command::cargo_bin("forge-core").expect("forge binary");
    command.args([
        "host-conformance",
        "run",
        "--adapter",
        python_program(),
        "--adapter-arg",
        reference_adapter().to_str().expect("UTF-8 adapter path"),
    ]);
    if let Some(mode) = mode {
        command.args(["--adapter-arg", mode]);
    }
    command.args([
        "--host-id",
        "example.host",
        "--host-version",
        "1.2.3",
        "--adapter-id",
        "example.adapter",
        "--adapter-version",
        "4.5.6",
        "--platform-id",
        "declared-test-platform",
        "--environment-id",
        "isolated-e2e",
        "--canonical-root",
        env!("CARGO_MANIFEST_DIR"),
    ]);
    if let Some(timeout_ms) = timeout_ms {
        command.args(["--timeout-ms", &timeout_ms.to_string()]);
    }
    command.args([
        "--output-dir",
        bundle.to_str().expect("UTF-8 bundle path"),
        "--json",
    ]);
    command
}

fn run_bundle(label: &str, mode: Option<&str>) -> (PathBuf, SoloHostConformanceResultDocument) {
    let bundle = temp_dir(label);
    let output = run_command(&bundle, mode, None)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: CliEnvelope<SoloHostConformanceResultDocument> =
        serde_json::from_slice(&output).expect("run envelope");
    (bundle, envelope.data.expect("run result"))
}

fn verify(bundle: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("forge-core")
        .expect("forge binary")
        .args([
            "host-conformance",
            "verify",
            "--bundle-dir",
            bundle.to_str().expect("UTF-8 bundle path"),
            "--json",
        ])
        .assert()
}

#[test]
#[allow(clippy::too_many_lines)] // One end-to-end scenario keeps its ordered evidence assertions together.
fn codex_adapter_preserves_partial_observations_and_typed_gaps() {
    let observation_path = temp_dir("codex-observation.json");
    let bundle = temp_dir("codex-bundle");
    let corpus: serde_json::Value = serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../contracts/hosts/solo-host-conformance-v1/corpus.json"),
        )
        .expect("read corpus"),
    )
    .expect("parse corpus");
    let mut cases = corpus["cases"]
        .as_array()
        .expect("corpus cases")
        .iter()
        .map(|case| {
            let assertions = case["required_assertions"]
                .as_array()
                .expect("required assertions")
                .iter()
                .map(|assertion| {
                    (
                        assertion.as_str().expect("assertion").to_owned(),
                        serde_json::Value::Bool(true),
                    )
                })
                .collect::<serde_json::Map<String, serde_json::Value>>();
            serde_json::json!({
                "case_id": case["case_id"],
                "assertions": assertions,
                "gaps": [],
                "fact_codes": ["codex_cooperative_observation"]
            })
        })
        .collect::<Vec<_>>();
    for case in &mut cases {
        match case["case_id"].as_str().expect("case id") {
            "canonical-project-root" => {
                case["assertions"]["ambiguous_roots_rejected"] = serde_json::Value::Bool(false);
                case["assertions"]["windows_wsl_bridge_applied_only_when_required"] =
                    serde_json::Value::Bool(true);
                case["gaps"] = serde_json::json!([
                    {
                        "kind": "canonical_root_unavailable",
                        "code": "ambiguous_root_rejection_not_exercised"
                    },
                    {
                        "kind": "platform_boundary_unavailable",
                        "code": "linux_runner_cannot_independently_verify_windows_bridge"
                    }
                ]);
            }
            "isolated-work" => {
                case["assertions"]["ownership_mismatch_stops"] = serde_json::Value::Bool(false);
                case["gaps"] = serde_json::json!([
                    {
                        "kind": "isolation_unavailable",
                        "code": "ownership_mismatch_not_exercised_in_host_run"
                    }
                ]);
            }
            _ => {}
        }
    }
    fs::write(
        &observation_path,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": "forge_codex_host_observation_v1",
            "evidence_mode": "cooperative_same_owner",
            "cases": cases
        }))
        .expect("serialize observation"),
    )
    .expect("write observation");

    let output = Command::cargo_bin("forge-core")
        .expect("forge binary")
        .args([
            "host-conformance",
            "run",
            "--adapter",
            python_program(),
            "--adapter-arg",
            codex_adapter().to_str().expect("UTF-8 adapter path"),
            "--adapter-arg",
            "--observation-file",
            "--adapter-arg",
            observation_path.to_str().expect("UTF-8 observation path"),
            "--host-id",
            "openai.codex",
            "--host-version",
            "0.144.6",
            "--adapter-id",
            "forge.codex.cooperative",
            "--adapter-version",
            "1.0.0",
            "--platform-id",
            "windows-10.0.26200",
            "--environment-id",
            "codex-desktop-wsl-ubuntu-24.04",
            "--canonical-root",
            env!("CARGO_MANIFEST_DIR"),
            "--output-dir",
            bundle.to_str().expect("UTF-8 bundle path"),
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let envelope: CliEnvelope<SoloHostConformanceResultDocument> =
        serde_json::from_slice(&output).expect("run envelope");
    let result = envelope.data.expect("run result");
    assert_eq!(result.capabilities.len(), 8);
    assert!(result.capabilities.iter().all(|capability| {
        capability.outcome == SoloHostConformanceOutcome::PartiallySupported
            && capability.gaps.iter().any(|gap| {
                gap.kind == SoloHostGapKind::NativeAuthenticityUnavailable
                    && gap.code == "forge_native_verifier_unavailable"
            })
    }));

    let canonical_root = result
        .capabilities
        .iter()
        .find(|capability| capability.capability == SoloHostCapability::CanonicalProjectRoot)
        .expect("canonical root result");
    assert!(canonical_root.assertions.iter().any(|assertion| {
        assertion.assertion == "ambiguous_roots_rejected"
            && assertion.status == SoloHostAssertionStatus::Failed
    }));
    assert!(canonical_root.assertions.iter().any(|assertion| {
        assertion.assertion == "windows_wsl_bridge_applied_only_when_required"
            && assertion.status == SoloHostAssertionStatus::NotApplicable
    }));
    assert!(canonical_root.gaps.iter().any(|gap| {
        gap.kind == SoloHostGapKind::CanonicalRootUnavailable
            && gap.code == "ambiguous_root_rejection_not_exercised"
    }));
    assert!(canonical_root.gaps.iter().any(|gap| {
        gap.kind == SoloHostGapKind::PlatformBoundaryUnavailable
            && gap.code == "linux_runner_cannot_independently_verify_windows_bridge"
    }));

    let isolated_work = result
        .capabilities
        .iter()
        .find(|capability| capability.capability == SoloHostCapability::IsolatedWork)
        .expect("isolated work result");
    assert!(isolated_work.assertions.iter().any(|assertion| {
        assertion.assertion == "ownership_mismatch_stops"
            && assertion.status == SoloHostAssertionStatus::Failed
    }));
    assert!(isolated_work.gaps.iter().any(|gap| {
        gap.kind == SoloHostGapKind::IsolationUnavailable
            && gap.code == "ownership_mismatch_not_exercised_in_host_run"
    }));

    verify(&bundle).success();

    fs::remove_file(observation_path).expect("observation cleanup");
    fs::remove_dir_all(bundle).expect("bundle cleanup");
}

#[test]
fn codex_adapter_rejects_observations_with_unknown_fields() {
    let observation_path = temp_dir("codex-unknown-field.json");
    let bundle = temp_dir("codex-unknown-field-bundle");
    fs::write(
        &observation_path,
        br#"{"schema_version":"forge_codex_host_observation_v1","evidence_mode":"cooperative_same_owner","cases":[],"raw_chat":"forbidden"}"#,
    )
    .expect("write observation");

    Command::cargo_bin("forge-core")
        .expect("forge binary")
        .args([
            "host-conformance",
            "run",
            "--adapter",
            python_program(),
            "--adapter-arg",
            codex_adapter().to_str().expect("UTF-8 adapter path"),
            "--adapter-arg",
            "--observation-file",
            "--adapter-arg",
            observation_path.to_str().expect("UTF-8 observation path"),
            "--host-id",
            "openai.codex",
            "--host-version",
            "0.144.6",
            "--adapter-id",
            "forge.codex.cooperative",
            "--adapter-version",
            "1.0.0",
            "--platform-id",
            "windows-10.0.26200",
            "--environment-id",
            "codex-desktop-wsl-ubuntu-24.04",
            "--canonical-root",
            env!("CARGO_MANIFEST_DIR"),
            "--output-dir",
            bundle.to_str().expect("UTF-8 bundle path"),
            "--json",
        ])
        .assert()
        .failure();
    assert!(!bundle.exists());

    fs::remove_file(observation_path).expect("observation cleanup");
}

#[test]
fn retained_codex_run_summary_matches_bundle_manifest_and_result() {
    let retained = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/hosts/conformance-results/codex/0.144.6");
    let read_json = |path: &Path| -> serde_json::Value {
        serde_json::from_slice(&fs::read(path).expect("read retained JSON"))
            .expect("parse retained JSON")
    };
    let summary = read_json(&retained.join("run-summary.json"));
    let manifest = read_json(&retained.join("bundle/manifest.json"));
    let result = read_json(&retained.join("bundle/result.json"));

    let summary_bundle_digest = summary
        .pointer("/bundle/bundle_digest")
        .and_then(serde_json::Value::as_str)
        .expect("summary bundle digest");
    let manifest_bundle_digest = manifest["bundle_digest"]
        .as_str()
        .expect("manifest bundle digest");
    assert_eq!(summary_bundle_digest, manifest_bundle_digest);

    let summary_forge_hash = summary
        .pointer("/identities/forge_executable_sha256")
        .and_then(serde_json::Value::as_str)
        .expect("summary Forge hash");
    let manifest_forge_hash = manifest
        .pointer("/bindings/observed/forge_executable_sha256")
        .and_then(serde_json::Value::as_str)
        .expect("manifest Forge hash");
    let result_forge_hash = result
        .pointer("/bindings/observed/forge_executable_sha256")
        .and_then(serde_json::Value::as_str)
        .expect("result Forge hash");
    assert_eq!(summary_forge_hash, manifest_forge_hash);
    assert_eq!(summary_forge_hash, result_forge_hash);

    let capabilities = result["capabilities"]
        .as_array()
        .expect("retained capabilities");
    let mut passed = 0_u64;
    let mut failed = 0_u64;
    let mut not_applicable = 0_u64;
    for assertion in capabilities.iter().flat_map(|capability| {
        capability["assertions"]
            .as_array()
            .expect("retained assertions")
    }) {
        match assertion["status"].as_str().expect("assertion status") {
            "passed" => passed += 1,
            "failed" => failed += 1,
            "not_applicable" => not_applicable += 1,
            other => panic!("unexpected retained assertion status: {other}"),
        }
    }
    let derived_counts = (capabilities.len() as u64, passed, failed, not_applicable);
    let summary_counts = (
        summary["result"]["capabilities"]
            .as_u64()
            .expect("summary capability count"),
        summary["result"]["assertions_passed"]
            .as_u64()
            .expect("summary passed count"),
        summary["result"]["assertions_failed"]
            .as_u64()
            .expect("summary failed count"),
        summary["result"]["assertions_not_applicable"]
            .as_u64()
            .expect("summary not-applicable count"),
    );
    assert_eq!(derived_counts, (8, 21, 2, 1));
    assert_eq!(summary_counts, derived_counts);
}

#[test]
fn public_kit_reference_fabricated_and_unsupported_paths_are_honest() {
    let kit = temp_dir("kit");
    Command::cargo_bin("forge-core")
        .expect("forge binary")
        .args([
            "host-conformance",
            "corpus",
            "--output-dir",
            kit.to_str().expect("UTF-8 kit path"),
            "--json",
        ])
        .assert()
        .success();
    for file in [
        "README.md",
        "corpus.json",
        "protocol-contract.json",
        "response.example.json",
        "reference-adapter.py",
    ] {
        assert!(kit.join(file).is_file(), "missing exported {file}");
    }

    let (reference_bundle, reference) = run_bundle("reference", None);
    assert_eq!(reference.capabilities.len(), 8);
    assert!(reference.capabilities.iter().all(|capability| {
        capability.outcome == SoloHostConformanceOutcome::PartiallySupported
    }));
    assert!(!reference.integrity_proves_authenticity);
    assert_eq!(
        reference.bindings.declared.platform_label,
        "declared-test-platform"
    );
    assert_eq!(
        reference.bindings.observed.platform.os,
        std::env::consts::OS
    );
    assert_eq!(
        reference.bindings.observed.platform.architecture,
        std::env::consts::ARCH
    );
    assert!(reference
        .bindings
        .observed
        .adapter_invocation
        .executable
        .file_name
        .contains("python"));
    assert_eq!(
        reference
            .bindings
            .observed
            .adapter_invocation
            .arguments
            .len(),
        1
    );
    let serialized = serde_json::to_string(&reference).expect("serialize result");
    assert!(!serialized.contains(env!("CARGO_MANIFEST_DIR")));
    let root = reference
        .capabilities
        .iter()
        .find(|capability| capability.capability == SoloHostCapability::CanonicalProjectRoot)
        .expect("root result");
    assert!(root.assertions.iter().any(|assertion| {
        assertion.assertion == "windows_wsl_bridge_applied_only_when_required"
            && assertion.status == SoloHostAssertionStatus::NotApplicable
    }));
    verify(&reference_bundle).success();

    let (fabricated_bundle, fabricated) = run_bundle("fabricated", Some("fabricated"));
    assert!(fabricated.capabilities.iter().all(|capability| {
        capability.outcome == SoloHostConformanceOutcome::PartiallySupported
            && capability.gaps.iter().any(|gap| {
                gap.kind == SoloHostGapKind::NativeAuthenticityUnavailable
                    && gap.code == "forge_native_verifier_unavailable"
            })
    }));

    let (unsupported_bundle, unsupported) = run_bundle("unsupported", Some("unsupported"));
    assert!(unsupported
        .capabilities
        .iter()
        .all(|capability| capability.outcome == SoloHostConformanceOutcome::Unsupported));

    fs::remove_dir_all(kit).expect("kit cleanup");
    fs::remove_dir_all(reference_bundle).expect("reference cleanup");
    fs::remove_dir_all(fabricated_bundle).expect("fabricated cleanup");
    fs::remove_dir_all(unsupported_bundle).expect("unsupported cleanup");
}

#[test]
fn adapter_output_timeout_and_secret_like_evidence_stop_before_bundle() {
    let secret_bundle = temp_dir("unsafe-secret");
    let secret_assert = run_command(&secret_bundle, Some("unsafe-secret"), None)
        .assert()
        .failure();
    let secret_output = secret_assert.get_output();
    assert!(!String::from_utf8_lossy(&secret_output.stdout).contains("client_secret_value"));
    assert!(!String::from_utf8_lossy(&secret_output.stderr).contains("client_secret_value"));
    assert!(!secret_bundle.exists());

    let path_bundle = temp_dir("unsafe-path");
    let path_assert = run_command(&path_bundle, Some("unsafe-path"), None)
        .assert()
        .failure();
    let path_output = path_assert.get_output();
    assert!(!String::from_utf8_lossy(&path_output.stdout).contains("/home/alice/private"));
    assert!(!String::from_utf8_lossy(&path_output.stderr).contains("/home/alice/private"));
    assert!(!path_bundle.exists());

    let oversized_bundle = temp_dir("oversized");
    run_command(&oversized_bundle, Some("oversize"), None)
        .assert()
        .failure();
    assert!(!oversized_bundle.exists());

    let timeout_bundle = temp_dir("timeout");
    run_command(&timeout_bundle, Some("timeout"), Some(100))
        .assert()
        .failure();
    assert!(!timeout_bundle.exists());
}

#[test]
fn verifier_rejects_missing_extra_changed_deep_and_hardlinked_files() {
    let (bundle, _) = run_bundle("tamper", None);

    let result = bundle.join("result.json");
    let parked = bundle.join("result.parked");
    fs::rename(&result, &parked).expect("park result");
    verify(&bundle).failure();
    fs::rename(&parked, &result).expect("restore result");

    let extra = bundle.join("extra.json");
    fs::write(&extra, b"{}\n").expect("extra file");
    verify(&bundle).failure();
    fs::remove_file(&extra).expect("remove extra");

    let deep = bundle.join("a/b/c/d/e");
    fs::create_dir_all(&deep).expect("deep dirs");
    fs::write(deep.join("extra.json"), b"{}\n").expect("deep file");
    verify(&bundle).failure();
    fs::remove_dir_all(bundle.join("a")).expect("remove deep tree");

    #[cfg(unix)]
    {
        let artifact = bundle.join("artifacts/activation.json");
        let original = fs::read(&artifact).expect("artifact bytes");
        let source = temp_dir("hardlink-source");
        fs::write(&source, &original).expect("hardlink source");
        fs::remove_file(&artifact).expect("remove artifact");
        fs::hard_link(&source, &artifact).expect("create hardlink");
        verify(&bundle).failure();
        fs::remove_file(&artifact).expect("remove hardlink");
        fs::remove_file(&source).expect("remove source");
        fs::write(&artifact, original).expect("restore artifact");
    }

    fs::write(
        bundle.join("artifacts/activation.json"),
        b"{\"changed\":true}\n",
    )
    .expect("alter artifact");
    verify(&bundle).failure();

    fs::remove_dir_all(bundle).expect("cleanup");
}
