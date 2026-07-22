use assert_cmd::Command;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signer as _, SigningKey};
use forge_core_contracts::{
    ProductLifecycleAsset, ProductLifecycleAssetKind,
    ProductLifecycleAssetTrustedVerificationInput, ProductLifecycleChange,
    ProductLifecycleChangeKind, ProductLifecycleChannel, ProductLifecycleDetachedProvenanceInput,
    ProductLifecycleRelease, ProductLifecycleReleaseDocument, ProductLifecycleSigstoreSubjectInput,
    ProductLifecycleTrustedVerificationInput, ProductLifecycleTrustedVerificationInputDocument,
    RepoPath, RuntimeKind, StableId, PRODUCT_LIFECYCLE_RELEASE_SCHEMA_VERSION,
    PRODUCT_LIFECYCLE_TRUSTED_VERIFICATION_SCHEMA_VERSION,
};
use p256::ecdsa::SigningKey as P256SigningKey;
use p256::pkcs8::{EncodePublicKey, LineEnding};
use rcgen::{
    date_time_ymd, BasicConstraints, CertificateParams, CustomExtension, DnType,
    ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair, KeyUsagePurpose, SanType, SerialNumber,
    SigningKey as RcgenSigningKey,
};
use semver::Version;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const CORPUS: &str = include_str!(
    "../../../docs/fixtures/product-lifecycle-recovery-v0/C2.4-product-lifecycle-recovery-corpus.yaml"
);
const EXPECTED_CASE_IDS: [&str; 8] = [
    "C2.4.partial-setup",
    "C2.4.stale-host-configuration",
    "C2.4.interrupted-update",
    "C2.4.downgrade",
    "C2.4.wrapper-mismatch",
    "C2.4.retained-state-uninstall",
    "C2.4.backup-interruption",
    "C2.4.replacement-machine-restore",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryCorpus {
    schema_version: String,
    artifact_kind: String,
    corpus_id: String,
    closed_schema: bool,
    selected_host: Option<String>,
    shared: SharedCorpusBoundary,
    cases: Vec<RecoveryCase>,
    evidence_status: EvidenceStatus,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedCorpusBoundary {
    product_lifecycle_spec: String,
    explicit_install_root_rule: String,
    candidate_data_only: bool,
    authority_grants: AuthorityGrants,
    selected_host: Option<String>,
    private_external_broker_keys: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct AuthorityGrants {
    claim: bool,
    mutation: bool,
    phase_transition: bool,
    release: bool,
    signing: bool,
    trust: bool,
    admission: bool,
    install: bool,
    activation: bool,
    lifecycle: bool,
    protected_anchor: bool,
    private_key: bool,
    host_selection: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceStatus {
    source_cases: String,
    stabilization_evidence: String,
    deferred: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryCase {
    case_id: String,
    category: RecoveryCategory,
    preconditions: BTreeMap<String, CorpusValue>,
    interruption_or_input: String,
    requested_operation: String,
    expected_result: String,
    required_observations: BTreeMap<String, Option<String>>,
    preservation_requirements: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RecoveryCategory {
    PartialSetup,
    StaleHostConfiguration,
    InterruptedUpdate,
    Downgrade,
    WrapperMismatch,
    RetainedStateUninstall,
    BackupInterruption,
    ReplacementMachineRestore,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CorpusValue {
    Text(String),
    TextList(Vec<String>),
}

struct TempFixture {
    root: PathBuf,
}

impl TempFixture {
    fn new(case_id: &str) -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "forge-product-lifecycle-recovery-{case_id}-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create corpus test root");
        Self { root }
    }

    fn install_root(&self) -> PathBuf {
        self.root.join("lifecycle-root")
    }

    fn bundle_root(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

fn lifecycle(root: &Path, subcommand: &str) -> std::process::Output {
    bin()
        .args(["lifecycle", subcommand, "--install-root"])
        .arg(root)
        .arg("--json")
        .output()
        .expect("run lifecycle command")
}

struct ReleaseFixture {
    release_file: PathBuf,
    trusted_verification_file: PathBuf,
}

fn lifecycle_with_release(
    root: &Path,
    subcommand: &str,
    release: &ReleaseFixture,
) -> std::process::Output {
    bin()
        .args(["lifecycle", subcommand, "--install-root"])
        .arg(root)
        .args(["--release-file"])
        .arg(&release.release_file)
        .args(["--trusted-verification-file"])
        .arg(&release.trusted_verification_file)
        .arg("--json")
        .output()
        .expect("run lifecycle release command")
}

fn json(output: &std::process::Output, label: &str) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{label} must return a JSON envelope: {error}; stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
    })
}

fn successful(output: &std::process::Output, label: &str) -> Value {
    assert!(
        output.status.success(),
        "{label} unexpectedly failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let envelope = json(output, label);
    assert_eq!(envelope["ok"], true, "{label}: {envelope:#}");
    envelope
}

fn rejected(output: &std::process::Output, label: &str) -> Value {
    assert!(
        !output.status.success(),
        "{label} unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout),
    );
    let envelope = json(output, label);
    assert_eq!(envelope["ok"], false, "{label}: {envelope:#}");
    envelope
}

fn assert_observation_only(envelope: &Value, case: &RecoveryCase) {
    assert!(
        envelope["data"]["selected_host"].is_null(),
        "{} must not select a host: {envelope:#}",
        case.case_id
    );
    assert_eq!(
        case.required_observations.get("selected_host"),
        Some(&None),
        "{} must require selected_host: null",
        case.case_id
    );
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn next_version() -> String {
    let current = Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is semver");
    Version::new(current.major, current.minor, current.patch + 1).to_string()
}

fn release_file(bundle: &Path, version: &str) -> ReleaseFixture {
    const CORE: &[u8] = b"forge-core-product-lifecycle-corpus\n";
    const CONFIG: &[u8] = b"{\"mode\":\"governed\"}\n";
    const WRAPPER: &[u8] = b"forge-wrapper-product-lifecycle-corpus\n";
    fs::create_dir_all(bundle.join("bundle")).expect("create release bundle");
    fs::write(bundle.join("bundle/forge-core"), CORE).expect("write core asset");
    fs::write(bundle.join("bundle/config.json"), CONFIG).expect("write config asset");
    fs::write(bundle.join("bundle/forge-wrapper"), WRAPPER).expect("write wrapper asset");
    let document = ProductLifecycleReleaseDocument {
        schema_version: PRODUCT_LIFECYCLE_RELEASE_SCHEMA_VERSION.to_owned(),
        product_lifecycle_release: ProductLifecycleRelease {
            release_id: StableId(format!("forge-core-{version}")),
            version: version.to_owned(),
            compatible_core_version: env!("CARGO_PKG_VERSION").to_owned(),
            channel: ProductLifecycleChannel::Stable,
            source_ref:
                "git+https://example.invalid/forge@0123456789abcdef0123456789abcdef01234567"
                    .to_owned(),
            provenance_ref: "provenance/forge-core.json".to_owned(),
            signature_ref: Some("signatures/forge-core.sig".to_owned()),
            rollback_ref: "releases/previous".to_owned(),
            changes: vec![ProductLifecycleChange {
                change_id: StableId(format!("change-{version}")),
                kind: ProductLifecycleChangeKind::Changed,
                summary: format!("C2.4 recovery corpus release {version}"),
            }],
            assets: vec![
                ProductLifecycleAsset {
                    asset_id: StableId("forge-core-binary".to_owned()),
                    kind: ProductLifecycleAssetKind::CoreBinary,
                    source_path: RepoPath("bundle/forge-core".to_owned()),
                    install_path: RepoPath("bin/forge-core".to_owned()),
                    sha256: digest(CORE),
                    executable: true,
                    host: None,
                },
                ProductLifecycleAsset {
                    asset_id: StableId("runtime-neutral-config".to_owned()),
                    kind: ProductLifecycleAssetKind::HostConfiguration,
                    source_path: RepoPath("bundle/config.json".to_owned()),
                    install_path: RepoPath("hosts/runtime-neutral/config.json".to_owned()),
                    sha256: digest(CONFIG),
                    executable: false,
                    host: Some(RuntimeKind::Codex),
                },
                ProductLifecycleAsset {
                    asset_id: StableId("forge-wrapper".to_owned()),
                    kind: ProductLifecycleAssetKind::Wrapper,
                    source_path: RepoPath("bundle/forge-wrapper".to_owned()),
                    install_path: RepoPath("bin/forge-wrapper".to_owned()),
                    sha256: digest(WRAPPER),
                    executable: true,
                    host: None,
                },
            ],
        },
    };
    let release_file = bundle.join("release.yaml");
    fs::write(
        &release_file,
        yaml_serde::to_string(&document).expect("serialize release document"),
    )
    .expect("write release document");
    let trusted_verification_file = write_trusted_verification(bundle, &document);
    ReleaseFixture {
        release_file,
        trusted_verification_file,
    }
}

fn write_trusted_verification(bundle: &Path, release: &ProductLifecycleReleaseDocument) -> PathBuf {
    let evidence_root = bundle.join("evidence");
    fs::create_dir_all(&evidence_root).expect("create trusted evidence root");
    let leaf_key = write_sigstore_identity(&evidence_root);
    write_sigstore_policy(&evidence_root);
    let rekor_key = P256SigningKey::from_slice(&[10_u8; 32]).expect("Rekor signing key");
    fs::write(
        evidence_root.join("rekor.pub"),
        rekor_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("Rekor public key"),
    )
    .expect("write Rekor public key");

    let assets = release
        .product_lifecycle_release
        .assets
        .iter()
        .map(|asset| {
            let asset_root = evidence_root.join(&asset.asset_id.0);
            fs::create_dir_all(&asset_root).expect("create per-asset evidence root");
            let bytes = fs::read(bundle.join(&asset.source_path.0)).expect("read release asset");
            let detached_provenance = write_detached_provenance(
                bundle,
                &asset_root,
                asset,
                &bytes,
                &release.product_lifecycle_release.source_ref,
            );
            let sigstore =
                write_sigstore_dsse(bundle, &asset_root, asset, &bytes, &leaf_key, &rekor_key);
            ProductLifecycleAssetTrustedVerificationInput {
                asset_id: asset.asset_id.clone(),
                asset_source_path: asset.source_path.clone(),
                expected_sha256: asset.sha256.clone(),
                detached_provenance,
                sigstore,
            }
        })
        .collect();
    let document = ProductLifecycleTrustedVerificationInputDocument {
        schema_version: PRODUCT_LIFECYCLE_TRUSTED_VERIFICATION_SCHEMA_VERSION.to_owned(),
        product_lifecycle_trusted_verification_input: ProductLifecycleTrustedVerificationInput {
            release_id: release.product_lifecycle_release.release_id.clone(),
            assets,
        },
    };
    let path = bundle.join("trusted-verification.yaml");
    fs::write(
        &path,
        yaml_serde::to_string(&document).expect("serialize trusted verification input"),
    )
    .expect("write trusted verification input");
    path
}

fn write_detached_provenance(
    bundle: &Path,
    asset_root: &Path,
    asset: &ProductLifecycleAsset,
    asset_bytes: &[u8],
    source_ref: &str,
) -> ProductLifecycleDetachedProvenanceInput {
    let builder_id = "https://github.com/DanielCarva1/forge-method-core/.github/workflows/release.yml@refs/heads/main";
    let source_uri = "github.com/DanielCarva1/forge-method-core";
    let artifact_digest = digest(asset_bytes)
        .strip_prefix("sha256:")
        .expect("sha256 prefix")
        .to_owned();
    let statement = json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": asset.source_path.0,
            "digest": { "sha256": artifact_digest }
        }],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "builder": { "id": builder_id },
            "buildDefinition": {
                "buildType": "https://forge.dev/build/release",
                "externalParameters": {
                    "source_uri": source_uri,
                    "source_ref": source_ref
                }
            },
            "resolvedDependencies": [{
                "uri": source_uri,
                "digest": { "gitCommit": source_ref }
            }]
        }
    });
    let provenance = serde_json::to_vec_pretty(&statement).expect("serialize provenance");
    let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
    let signature = signing_key.sign(&provenance);
    let provenance_path = asset_root.join("provenance.json");
    let signature_path = asset_root.join("provenance.sig");
    let public_key_path = asset_root.join("provenance.pub");
    let transparency_log_path = asset_root.join("provenance.tlog.json");
    fs::write(&provenance_path, &provenance).expect("write provenance");
    fs::write(&signature_path, BASE64.encode(signature.to_bytes())).expect("write signature");
    fs::write(
        &public_key_path,
        BASE64.encode(signing_key.verifying_key().to_bytes()),
    )
    .expect("write provenance public key");
    let provenance_sha256 = digest(&provenance);
    let signature_sha256 = digest(&signature.to_bytes());
    let mut leaf_payload = vec![0_u8];
    leaf_payload.extend_from_slice(
        format!(
            "{}\n{}",
            provenance_sha256
                .strip_prefix("sha256:")
                .expect("provenance digest"),
            signature_sha256
                .strip_prefix("sha256:")
                .expect("signature digest")
        )
        .as_bytes(),
    );
    let leaf_hash = digest(&leaf_payload);
    fs::write(
        &transparency_log_path,
        serde_json::to_vec_pretty(&json!({
            "log_id": "forge-release-transparency-log",
            "log_index": 0,
            "tree_size": 1,
            "leaf_hash": leaf_hash,
            "root_hash": leaf_hash,
            "hashes": []
        }))
        .expect("serialize provenance transparency log"),
    )
    .expect("write provenance transparency log");
    ProductLifecycleDetachedProvenanceInput {
        provenance_path: repo_relative(bundle, &provenance_path),
        signature_path: repo_relative(bundle, &signature_path),
        public_key_path: repo_relative(bundle, &public_key_path),
        transparency_log_path: repo_relative(bundle, &transparency_log_path),
        expected_builder_id: builder_id.to_owned(),
        expected_source_uri: source_uri.to_owned(),
        expected_source_ref: source_ref.to_owned(),
    }
}

fn write_sigstore_identity(evidence_root: &Path) -> KeyPair {
    let mut ca_params = CertificateParams::new(Vec::new()).expect("Fulcio CA params");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "Forge Test Fulcio Root");
    ca_params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    ca_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    ca_params.key_usages.push(KeyUsagePurpose::CrlSign);
    ca_params.not_before = date_time_ymd(2026, 1, 1);
    ca_params.not_after = date_time_ymd(2030, 1, 1);
    let ca_key = KeyPair::generate().expect("generate Fulcio CA key");
    let ca_certificate = ca_params.self_signed(&ca_key).expect("self-sign Fulcio CA");
    let issuer = Issuer::new(ca_params, ca_key);

    let identity = "https://github.com/DanielCarva1/forge-method-core/.github/workflows/release.yml@refs/heads/main";
    let repository = "https://github.com/DanielCarva1/forge-method-core";
    let git_ref = "refs/heads/main";
    let git_sha = "0123456789abcdef0123456789abcdef01234567";
    let leaf_key = KeyPair::generate().expect("generate Fulcio leaf key");
    let mut leaf_params = CertificateParams::new(Vec::new()).expect("Fulcio leaf params");
    leaf_params.serial_number = Some(SerialNumber::from(0x1234_u64));
    leaf_params
        .subject_alt_names
        .push(SanType::URI(identity.try_into().expect("identity URI")));
    leaf_params
        .distinguished_name
        .push(DnType::CommonName, "Forge Test Release Identity");
    leaf_params
        .key_usages
        .push(KeyUsagePurpose::DigitalSignature);
    leaf_params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::CodeSigning);
    leaf_params.not_before = date_time_ymd(2026, 1, 1);
    leaf_params.not_after = date_time_ymd(2030, 1, 1);
    for (oid, value) in [
        (
            &[1, 3, 6, 1, 4, 1, 57264, 1, 8][..],
            "https://token.actions.githubusercontent.com",
        ),
        (&[1, 3, 6, 1, 4, 1, 57264, 1, 9][..], identity),
        (&[1, 3, 6, 1, 4, 1, 57264, 1, 10][..], git_sha),
        (&[1, 3, 6, 1, 4, 1, 57264, 1, 12][..], repository),
        (&[1, 3, 6, 1, 4, 1, 57264, 1, 13][..], git_sha),
        (&[1, 3, 6, 1, 4, 1, 57264, 1, 14][..], git_ref),
        (
            &[1, 3, 6, 1, 4, 1, 57264, 1, 24][..],
            "repo:DanielCarva1/forge-method-core:ref:refs/heads/main",
        ),
    ] {
        leaf_params
            .custom_extensions
            .push(CustomExtension::from_oid_content(oid, der_utf8(value)));
    }
    let leaf_certificate = leaf_params
        .signed_by(&leaf_key, &issuer)
        .expect("sign Fulcio leaf");
    fs::write(evidence_root.join("fulcio-root.pem"), ca_certificate.pem())
        .expect("write Fulcio root");
    fs::write(
        evidence_root.join("fulcio-leaf.pem"),
        leaf_certificate.pem(),
    )
    .expect("write Fulcio leaf");
    leaf_key
}

fn write_sigstore_policy(evidence_root: &Path) {
    const REKOR_LOG_ID: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
    let policy = format!(
        r#"schema_version: "0.1"
sigstore_trusted_root_policy:
  root_source: "tuf"
  trusted_root_ref: "sigstore-public-good-instance@2026-07-21"
  offline_allowed: true
  fulcio:
    required: true
    certificate_authority_refs: ["fulcio-root.pem"]
  rekor:
    required: true
    log_ids: ["{REKOR_LOG_ID}"]
    public_key_refs: ["rekor.pub"]
  certificate_transparency:
    required: true
    log_ids: ["ctfe-log-id"]
    public_key_refs: ["ctfe.pub"]
  timestamp_authority:
    mode: "either"
    certificate_refs: ["tsa-root.pem"]
  identity_policy:
    expected_oidc_issuer: "https://token.actions.githubusercontent.com"
    expected_certificate_identity: "https://github.com/DanielCarva1/forge-method-core/.github/workflows/release.yml@refs/heads/main"
    expected_github_repository: "DanielCarva1/forge-method-core"
    expected_github_ref: "refs/heads/main"
    expected_github_sha: "0123456789abcdef0123456789abcdef01234567"
"#
    );
    fs::write(evidence_root.join("sigstore-trust-policy.yaml"), policy)
        .expect("write Sigstore trust policy");
}

#[allow(clippy::too_many_lines)]
fn write_sigstore_dsse(
    bundle: &Path,
    asset_root: &Path,
    asset: &ProductLifecycleAsset,
    asset_bytes: &[u8],
    leaf_key: &KeyPair,
    rekor_key: &P256SigningKey,
) -> ProductLifecycleSigstoreSubjectInput {
    const REKOR_LOG_ID: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
    const PREDICATE_TYPE: &str = "https://slsa.dev/provenance/v1";
    let payload_type = "application/vnd.in-toto+json";
    let statement = json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": asset.source_path.0,
            "digest": { "sha256": hex_bytes(&Sha256::digest(asset_bytes)) }
        }],
        "predicateType": PREDICATE_TYPE,
        "predicate": {
            "builder": {
                "id": "https://github.com/DanielCarva1/forge-method-core/actions"
            }
        }
    });
    let payload = serde_json::to_vec(&statement).expect("serialize DSSE statement");
    let pae = dsse_pae(payload_type, &payload);
    let signature = RcgenSigningKey::sign(leaf_key, &pae).expect("sign DSSE PAE");
    let envelope = json!({
        "payloadType": payload_type,
        "payload": BASE64.encode(&payload),
        "signatures": [{ "keyid": "forge-test", "sig": BASE64.encode(&signature) }]
    });
    let leaf_pem =
        fs::read_to_string(bundle.join("evidence/fulcio-leaf.pem")).expect("read Fulcio leaf PEM");
    let leaf_der = pem_to_der(&leaf_pem);
    let sigstore_bundle = json!({
        "mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json",
        "verificationMaterial": {
            "certificate": { "rawBytes": BASE64.encode(leaf_der) }
        },
        "dsseEnvelope": envelope
    });
    let bundle_path = asset_root.join("dsse-bundle.json");
    fs::write(
        &bundle_path,
        serde_json::to_vec_pretty(&sigstore_bundle).expect("serialize DSSE bundle"),
    )
    .expect("write DSSE bundle");

    let canonical_envelope =
        serde_json_canonicalizer::to_vec(&envelope).expect("canonical DSSE envelope");
    let body = json!({
        "kind": "dsse",
        "apiVersion": "0.0.1",
        "spec": {
            "dsseObj": {
                "payloadHash": {
                    "algorithm": "sha256",
                    "value": hex_bytes(&Sha256::digest(&payload))
                },
                "envelopeHash": {
                    "algorithm": "sha256",
                    "value": hex_bytes(&Sha256::digest(&canonical_envelope))
                },
                "signatures": [{ "signature": BASE64.encode(&signature) }]
            }
        }
    });
    let body_bytes = serde_json::to_vec(&body).expect("serialize DSSE Rekor body");
    let canonical_body = serde_json_canonicalizer::to_vec(&body).expect("canonical Rekor body");
    let root_hash = rekor_leaf_hash(&canonical_body);
    let root_bytes = hex_decode(&root_hash);
    let checkpoint_body = format!("forge-test-rekor\n1\n{}\n", BASE64.encode(root_bytes));
    let checkpoint_signature: p256::ecdsa::Signature = rekor_key.sign(checkpoint_body.as_bytes());
    let mut signed_note_payload = vec![0_u8; 4];
    signed_note_payload.extend_from_slice(checkpoint_signature.to_der().as_bytes());
    let checkpoint = format!(
        "{}\n— forge-test {}\n",
        checkpoint_body,
        BASE64.encode(signed_note_payload)
    );
    let rekor_entry = json!({
        "body": BASE64.encode(&body_bytes),
        "integratedTime": 1_783_391_200_i64,
        "logID": REKOR_LOG_ID,
        "logIndex": 0_i64,
        "verification": {
            "inclusionProof": {
                "hashes": [],
                "logIndex": 0_i64,
                "rootHash": root_hash,
                "treeSize": 1_u64,
                "checkpoint": checkpoint
            },
            "signedEntryTimestamp": ""
        }
    });
    let rekor_path = asset_root.join("rekor-log-entry.json");
    fs::write(
        &rekor_path,
        serde_json::to_vec_pretty(&rekor_entry).expect("serialize Rekor entry"),
    )
    .expect("write Rekor entry");

    ProductLifecycleSigstoreSubjectInput {
        bundle_path: repo_relative(bundle, &bundle_path),
        trust_policy_path: RepoPath("evidence/sigstore-trust-policy.yaml".to_owned()),
        certificate_path: RepoPath("evidence/fulcio-leaf.pem".to_owned()),
        issuer_certificate_paths: vec![RepoPath("evidence/fulcio-root.pem".to_owned())],
        rekor_log_entry_path: repo_relative(bundle, &rekor_path),
        rekor_public_key_path: RepoPath("evidence/rekor.pub".to_owned()),
        expected_rekor_log_id: REKOR_LOG_ID.to_owned(),
        expected_predicate_type: PREDICATE_TYPE.to_owned(),
    }
}

fn repo_relative(root: &Path, path: &Path) -> RepoPath {
    RepoPath(
        path.strip_prefix(root)
            .expect("evidence below bundle root")
            .to_string_lossy()
            .replace('\\', "/"),
    )
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("write hex");
        output
    })
}

fn hex_decode(value: &str) -> Vec<u8> {
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).expect("hex byte"))
        .collect()
}

fn rekor_leaf_hash(entry: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update([0_u8]);
    hasher.update(entry);
    hex_bytes(&hasher.finalize())
}

fn dsse_pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let payload_type = payload_type.as_bytes();
    let mut output = Vec::new();
    output.extend_from_slice(b"DSSEv1 ");
    output.extend_from_slice(payload_type.len().to_string().as_bytes());
    output.push(b' ');
    output.extend_from_slice(payload_type);
    output.push(b' ');
    output.extend_from_slice(payload.len().to_string().as_bytes());
    output.push(b' ');
    output.extend_from_slice(payload);
    output
}

fn pem_to_der(pem: &str) -> Vec<u8> {
    let encoded = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>();
    BASE64.decode(encoded).expect("decode certificate PEM")
}

fn der_utf8(value: &str) -> Vec<u8> {
    der(0x0c, value.as_bytes())
}

fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(content.len() + 6);
    output.push(tag);
    output.extend(der_length(content.len()));
    output.extend_from_slice(content);
    output
}

fn der_length(len: usize) -> Vec<u8> {
    if len < 0x80 {
        return vec![u8::try_from(len).expect("short DER length")];
    }
    let bytes = len.to_be_bytes();
    let first_non_zero = bytes
        .iter()
        .position(|byte| *byte != 0)
        .expect("non-zero DER length");
    let content = &bytes[first_non_zero..];
    let mut output = vec![0x80 | u8::try_from(content.len()).expect("DER length width")];
    output.extend_from_slice(content);
    output
}

fn installed_fixture(case: &RecoveryCase) -> (TempFixture, ReleaseFixture, Value) {
    let fixture = TempFixture::new(&case.case_id);
    let root = fixture.install_root();
    successful(&lifecycle(&root, "setup"), "setup owned lifecycle root");
    let release = release_file(
        &fixture.bundle_root("initial-bundle"),
        env!("CARGO_PKG_VERSION"),
    );
    let installed = successful(
        &lifecycle_with_release(&root, "install", &release),
        "install initial lifecycle generation",
    );
    (fixture, release, installed)
}

fn installed_asset_root(root: &Path, generation: &str) -> PathBuf {
    root.join("product-lifecycle/generations")
        .join(generation)
        .join("assets")
}

fn corpus_text<'a>(case: &'a RecoveryCase, key: &str) -> &'a str {
    match case.preconditions.get(key) {
        Some(CorpusValue::Text(value)) => value,
        Some(CorpusValue::TextList(_)) => {
            panic!("{} precondition {key} must be text", case.case_id)
        }
        None => panic!("{} missing precondition {key}", case.case_id),
    }
}

fn assert_case_shape(case: &RecoveryCase) {
    assert!(
        case.interruption_or_input.contains(' '),
        "{} must retain its interruption boundary",
        case.case_id
    );
    assert!(
        !case.preservation_requirements.is_empty(),
        "{} must retain preservation requirements",
        case.case_id
    );
    assert!(
        !case.requested_operation.is_empty() && !case.expected_result.is_empty(),
        "{} must retain executable-source expectations",
        case.case_id
    );
    assert!(
        corpus_text(case, "install_root").starts_with("/fixture/product-lifecycle-recovery/"),
        "{} must keep an explicit fictional install root",
        case.case_id
    );
}

fn partial_setup(case: &RecoveryCase) {
    assert_eq!(case.category, RecoveryCategory::PartialSetup);
    assert_eq!(
        case.expected_result,
        "fail_closed_incomplete_setup; do not adopt, overwrite, or activate retained data"
    );
    let fixture = TempFixture::new(&case.case_id);
    let root = fixture.install_root();
    fs::create_dir_all(&root).expect("create partial setup root");
    fs::write(
        root.join(".forge-product-lifecycle.json"),
        "{\"schema_version\":\"0.1\",\"owner\":\"forge-core-product-lifecycle\",\"product\":\"forge-core\",\"authority_boundary\":\"owns only exact inventory below this install root; consumer projects, Forge sidecars, operator anchors, backups, external broker keys, signing keys, trust roots, and private keys remain outside lifecycle custody\"}\n",
    )
    .expect("write partial ownership marker");
    let note = root.join(corpus_text(case, "retained_unknown_file"));
    fs::write(&note, b"operator-owned\n").expect("write retained note");

    for operation in ["status", "doctor"] {
        let envelope = successful(&lifecycle(&root, operation), operation);
        assert_observation_only(&envelope, case);
        assert!(envelope["data"]["active_generation"].is_null());
    }
    let setup = successful(&lifecycle(&root, "setup"), "retry incomplete setup");
    assert_observation_only(&setup, case);
    assert!(setup["data"]["active_generation"].is_null());
    assert_eq!(
        fs::read(&note).expect("read retained note"),
        b"operator-owned\n"
    );
}

fn stale_host_configuration(case: &RecoveryCase) {
    assert_eq!(case.category, RecoveryCategory::StaleHostConfiguration);
    assert_eq!(
        case.expected_result,
        "report_exact_degraded_observation_only"
    );
    let (fixture, _, installed) = installed_fixture(case);
    let root = fixture.install_root();
    let active_generation = installed["data"]["active_generation"]
        .as_str()
        .unwrap_or_else(|| panic!("installed active generation: {installed:#}"))
        .to_owned();
    let generation = active_generation.as_str();
    let config =
        installed_asset_root(&root, generation).join(corpus_text(case, "retained_configuration"));
    fs::write(&config, b"operator-modified\n").expect("modify retained configuration");

    for operation in ["status", "doctor"] {
        let envelope = successful(&lifecycle(&root, operation), operation);
        assert_observation_only(&envelope, case);
        assert_eq!(envelope["data"]["status"], "degraded");
        assert_eq!(
            envelope["data"]["active_generation"], active_generation,
            "stale configuration must not alter the active generation"
        );
    }
}

fn interrupted_update(case: &RecoveryCase) {
    assert_eq!(case.category, RecoveryCategory::InterruptedUpdate);
    assert_eq!(
        case.expected_result,
        "retain_generation-a_as_active; staged_candidate_is_not_active; retry_reverifies_input"
    );
    let (fixture, _, installed) = installed_fixture(case);
    let root = fixture.install_root();
    let active = installed["data"]["active_generation"]
        .as_str()
        .expect("initial active generation")
        .to_owned();
    let interrupted_stage = root.join("product-lifecycle/staging/candidate-interrupted");
    fs::create_dir_all(&interrupted_stage).expect("create interrupted candidate staging");
    fs::write(interrupted_stage.join("unverified"), b"candidate-only\n")
        .expect("write interrupted candidate marker");

    let doctor = successful(&lifecycle(&root, "doctor"), "doctor interrupted update");
    assert_observation_only(&doctor, case);
    assert_eq!(doctor["data"]["active_generation"], active);
    assert_ne!(doctor["data"]["active_generation"], "candidate-interrupted");
    assert!(interrupted_stage.is_dir());

    let retry = release_file(&fixture.bundle_root("retry-bundle"), &next_version());
    let updated = successful(
        &lifecycle_with_release(&root, "update", &retry),
        "retry interrupted update",
    );
    assert_observation_only(&updated, case);
    assert_ne!(updated["data"]["active_generation"], active);
}

fn downgrade(case: &RecoveryCase) {
    assert_eq!(case.category, RecoveryCategory::Downgrade);
    assert_eq!(
        case.expected_result,
        "reject_downgrade_before_active_state_mutation"
    );
    let (fixture, initial, installed) = installed_fixture(case);
    let root = fixture.install_root();
    let active_before = installed["data"]["active_generation"].clone();
    let update = release_file(&fixture.bundle_root("newer-bundle"), &next_version());
    let updated = successful(
        &lifecycle_with_release(&root, "update", &update),
        "install newer generation before downgrade",
    );
    let active_updated = updated["data"]["active_generation"].clone();
    assert_ne!(active_updated, active_before);

    let rejection = rejected(
        &lifecycle_with_release(&root, "update", &initial),
        "reject semantic downgrade",
    );
    assert!(
        rejection["error"]["message"]
            .as_str()
            .expect("downgrade message")
            .contains("must be newer"),
        "downgrade must be rejected before lifecycle mutation: {rejection:#}"
    );
    let status = successful(
        &lifecycle(&root, "status"),
        "status after downgrade rejection",
    );
    assert_observation_only(&status, case);
    assert_eq!(status["data"]["active_generation"], active_updated);
}

fn wrapper_mismatch(case: &RecoveryCase) {
    assert_eq!(case.category, RecoveryCategory::WrapperMismatch);
    assert_eq!(
        case.expected_result,
        "report_digest_mismatch; preserve_wrapper_on_uninstall"
    );
    let (fixture, _, installed) = installed_fixture(case);
    let root = fixture.install_root();
    let generation = installed["data"]["active_generation"]
        .as_str()
        .expect("installed generation");
    let wrapper =
        installed_asset_root(&root, generation).join(corpus_text(case, "wrapper_inventory_path"));
    fs::write(&wrapper, b"operator-modified-wrapper\n").expect("modify wrapper");

    let doctor = successful(&lifecycle(&root, "doctor"), "doctor wrapper mismatch");
    assert_observation_only(&doctor, case);
    assert_eq!(doctor["data"]["status"], "degraded");
    let uninstall = successful(&lifecycle(&root, "uninstall"), "uninstall wrapper mismatch");
    assert_observation_only(&uninstall, case);
    assert!(wrapper.is_file(), "mismatched wrapper must be preserved");
}

fn retained_state_uninstall(case: &RecoveryCase) {
    assert_eq!(case.category, RecoveryCategory::RetainedStateUninstall);
    assert_eq!(
        case.expected_result,
        "delete_only_matching_regular_product_inventory"
    );
    let (fixture, _, installed) = installed_fixture(case);
    let root = fixture.install_root();
    let generation = installed["data"]["active_generation"]
        .as_str()
        .expect("installed generation");
    let generation_root = installed_asset_root(&root, generation);
    let matching = generation_root.join(corpus_text(case, "matching_product_asset"));
    let modified = generation_root.join(corpus_text(case, "modified_product_asset"));
    fs::write(&modified, b"operator-modified-wrapper\n").expect("modify product asset");
    let unknown = root.join(corpus_text(case, "unknown_file"));
    fs::write(&unknown, b"operator-note\n").expect("write unknown file");
    for value in match case.preconditions.get("excluded_paths") {
        Some(CorpusValue::TextList(values)) => values,
        _ => panic!("{} must contain excluded paths", case.case_id),
    } {
        let path = root.join(value.trim_end_matches('/'));
        fs::create_dir_all(&path).expect("create excluded path");
        fs::write(path.join("retain"), b"external-owner\n").expect("write excluded state");
    }

    let uninstall = successful(&lifecycle(&root, "uninstall"), "uninstall retained state");
    assert_observation_only(&uninstall, case);
    assert!(
        !matching.exists(),
        "matching inventory asset must be removed"
    );
    assert!(
        modified.is_file(),
        "modified product asset must be retained"
    );
    assert!(unknown.is_file(), "unknown file must be retained");
    assert!(
        root.join("external-broker-private-key/retain").is_file(),
        "external private key fixture must remain outside lifecycle custody"
    );
}

fn backup_interruption(case: &RecoveryCase) {
    assert_eq!(case.category, RecoveryCategory::BackupInterruption);
    assert_eq!(
        case.expected_result,
        "backup_artifact_is_not_lifecycle_state_or_restore_authority"
    );
    let (fixture, _, installed) = installed_fixture(case);
    let root = fixture.install_root();
    let active = installed["data"]["active_generation"].clone();
    let backup = root.join(corpus_text(case, "backup_artifact"));
    fs::create_dir_all(backup.parent().expect("backup parent")).expect("create backup parent");
    fs::write(&backup, b"incomplete-copy\n").expect("write incomplete backup");

    for operation in ["status", "doctor"] {
        let envelope = successful(&lifecycle(&root, operation), operation);
        assert_observation_only(&envelope, case);
        assert_eq!(envelope["data"]["active_generation"], active);
    }
    assert!(
        backup.is_file(),
        "observation must not consume incomplete backup material"
    );
}

fn replacement_machine_restore(case: &RecoveryCase) {
    assert_eq!(case.category, RecoveryCategory::ReplacementMachineRestore);
    assert_eq!(
        case.expected_result,
        "observe_candidate_only; no_implicit_setup_restore_or_activation"
    );
    let fixture = TempFixture::new(&case.case_id);
    let root = fixture.install_root();
    fs::create_dir_all(root.join("candidate-recovery-material"))
        .expect("create candidate material");
    fs::write(
        root.join("candidate-recovery-material/generation-a.json"),
        b"candidate-only\n",
    )
    .expect("write candidate metadata");

    for operation in ["status", "doctor"] {
        let envelope = successful(&lifecycle(&root, operation), operation);
        assert_observation_only(&envelope, case);
        assert_eq!(envelope["data"]["status"], "unmanaged_root");
        assert!(envelope["data"]["active_generation"].is_null());
    }
    assert!(
        !root.join(".forge-product-lifecycle.json").exists(),
        "candidate material must not claim an install root"
    );
    assert!(
        root.join("candidate-recovery-material/generation-a.json")
            .is_file(),
        "candidate material must remain observation-only"
    );
}

#[test]
fn c2_4_recovery_corpus_maps_each_case_to_public_lifecycle_observations() {
    let corpus: RecoveryCorpus =
        yaml_serde::from_str(CORPUS).expect("parse typed C2.4 recovery corpus");
    assert_eq!(corpus.schema_version, "0.1");
    assert_eq!(
        corpus.artifact_kind,
        "product-lifecycle-recovery-source-corpus"
    );
    assert_eq!(corpus.corpus_id, "C2.4-product-lifecycle-recovery-v0");
    assert!(corpus.closed_schema);
    assert!(corpus.selected_host.is_none());
    assert!(corpus.shared.candidate_data_only);
    assert!(corpus.shared.selected_host.is_none());
    assert_eq!(
        corpus.shared.product_lifecycle_spec,
        "contracts/spec/product-lifecycle-v0.yaml"
    );
    assert!(corpus
        .shared
        .explicit_install_root_rule
        .contains("inference is forbidden"));
    assert!(corpus
        .shared
        .private_external_broker_keys
        .contains("excluded"));
    let grants = &corpus.shared.authority_grants;
    assert!(!grants.claim && !grants.mutation && !grants.phase_transition && !grants.release);
    assert!(!grants.signing && !grants.trust && !grants.admission && !grants.install);
    assert!(!grants.activation && !grants.lifecycle && !grants.protected_anchor);
    assert!(!grants.private_key && !grants.host_selection);
    assert_eq!(corpus.evidence_status.source_cases, "authored_not_executed");
    assert_eq!(corpus.evidence_status.stabilization_evidence, "unexecuted");
    assert!(corpus
        .evidence_status
        .deferred
        .iter()
        .any(|item| item.contains("Rust source-test execution")));

    let ids = corpus
        .cases
        .iter()
        .map(|case| case.case_id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids, BTreeSet::from(EXPECTED_CASE_IDS));
    assert_eq!(corpus.cases.len(), EXPECTED_CASE_IDS.len());

    for case in &corpus.cases {
        assert_case_shape(case);
        match case.case_id.as_str() {
            "C2.4.partial-setup" => partial_setup(case),
            "C2.4.stale-host-configuration" => stale_host_configuration(case),
            "C2.4.interrupted-update" => interrupted_update(case),
            "C2.4.downgrade" => downgrade(case),
            "C2.4.wrapper-mismatch" => wrapper_mismatch(case),
            "C2.4.retained-state-uninstall" => retained_state_uninstall(case),
            "C2.4.backup-interruption" => backup_interruption(case),
            "C2.4.replacement-machine-restore" => replacement_machine_restore(case),
            _ => unreachable!("closed corpus case set was checked above"),
        }
    }
}
