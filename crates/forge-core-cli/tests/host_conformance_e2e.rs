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
