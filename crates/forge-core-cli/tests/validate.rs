#![allow(clippy::struct_field_names)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::format_collect)]
#![allow(clippy::doc_markdown)]
// Test fixtures legitimately reuse the `_path` suffix across fields for
// readability (e.g. `bundle_path`, `artifact_path`), and a few integration
// tests walk the full CLI surface which makes them long. Renaming fields or
// splitting end-to-end tests would hurt fixture/test clarity for no real
// benefit.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signer as _, SigningKey};
use forge_core_cli::{
    run_execute_operation, run_host_adapter_artifact_verification,
    run_host_adapter_certificate_crl_status_verification,
    run_host_adapter_certificate_ocsp_status_verification,
    run_host_adapter_certificate_revocation_policy_verification,
    run_host_adapter_certificate_transparency_sct_verification,
    run_host_adapter_distribution_admission, run_host_adapter_distribution_policy,
    run_host_adapter_fulcio_certificate_identity_verification,
    run_host_adapter_invocation_admission, run_host_adapter_manifest,
    run_host_adapter_process_security_policy, run_host_adapter_projection,
    run_host_adapter_provenance_verification, run_host_adapter_rekor_verification,
    run_host_adapter_sigstore_bundle_subject_verification,
    run_host_adapter_sigstore_dsse_in_toto_subject_verification,
    run_host_adapter_sigstore_timestamp_authority_verification,
    run_host_adapter_tuf_trusted_root_freshness_verification, run_query_effect_index,
    run_rebuild_effect_index, run_validate, ExecuteOperationInput,
    HostAdapterArtifactVerificationInput, HostAdapterArtifactVerificationStatus,
    HostAdapterAuthorityClass, HostAdapterAutoTrigger,
    HostAdapterCertificateCrlStatusVerificationInput,
    HostAdapterCertificateCrlStatusVerificationStatus,
    HostAdapterCertificateOcspStatusVerificationInput,
    HostAdapterCertificateOcspStatusVerificationStatus,
    HostAdapterCertificateRevocationPolicyVerificationInput,
    HostAdapterCertificateRevocationPolicyVerificationStatus,
    HostAdapterCertificateTransparencySctVerificationInput,
    HostAdapterCertificateTransparencySctVerificationStatus, HostAdapterCommandKind,
    HostAdapterDistributionAdmissionStatus, HostAdapterDistributionEvidence,
    HostAdapterFulcioCertificateIdentityVerificationInput,
    HostAdapterFulcioCertificateIdentityVerificationStatus, HostAdapterInvocationAdmissionStatus,
    HostAdapterInvocationRequest, HostAdapterMutationClass, HostAdapterProcessTarget,
    HostAdapterProjectionTarget, HostAdapterProvenanceVerificationInput,
    HostAdapterProvenanceVerificationStatus, HostAdapterRekorVerificationInput,
    HostAdapterRekorVerificationStatus, HostAdapterSigstoreBundleSubjectVerificationInput,
    HostAdapterSigstoreBundleSubjectVerificationStatus,
    HostAdapterSigstoreDsseInTotoSubjectVerificationInput,
    HostAdapterSigstoreDsseInTotoSubjectVerificationStatus,
    HostAdapterSigstoreTimestampAuthorityVerificationInput,
    HostAdapterSigstoreTimestampAuthorityVerificationStatus,
    HostAdapterSigstoreTrustPolicyVerificationInput,
    HostAdapterSigstoreTrustPolicyVerificationStatus,
    HostAdapterTufTrustedRootFreshnessVerificationInput,
    HostAdapterTufTrustedRootFreshnessVerificationStatus, HostAdapterUpdateChannel, OcspNonceHex,
    PayloadFileSpec, PayloadLoadPolicy, QueryEffectIndexInput, RebuildEffectIndexInput,
    ValidationStatus,
};
use forge_core_command_surface as command_surface;
use forge_core_contracts::claim::ActorRole;
use forge_core_contracts::runtime::RuntimeKind;
use forge_core_contracts::tool_effect::{AccessMode, EffectTargetKind};
use forge_core_contracts::StableId;
use forge_core_store::{
    append_json_line, sha256_content_hash, EffectMetadataConsumerUse,
    EffectTargetMetadataIndexQueryStatus, EffectTargetMetadataIndexRebuildStatus,
    EffectTargetMetadataRecord, EffectTargetMetadataRecordKind, EffectWalRecord, EffectWalStage,
    EffectWalTargetMetadata, WalDurability,
};
use p256::ecdsa::SigningKey as P256SigningKey;
use p256::pkcs8::{EncodePublicKey, LineEnding};
use rcgen::{
    date_time_ymd, BasicConstraints, Certificate, CertificateParams,
    CertificateRevocationListParams, CustomExtension, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyIdMethod, KeyPair, KeyUsagePurpose, RevocationReason, RevokedCertParams, SanType,
    SerialNumber, SigningKey as RcgenSigningKey,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use x509_parser::prelude::{FromDer as _, X509Certificate};

const RFC3161_VALID_BUNDLE: &str = include_str!("fixtures/rfc3161/valid_bundle.json");
const RFC3161_PAYLOAD_MISMATCH_BUNDLE: &str =
    include_str!("fixtures/rfc3161/payload_mismatch_bundle.json");
const RFC3161_VALID_TRUSTED_ROOT: &str = include_str!("fixtures/rfc3161/valid_trusted_root.json");
const CT_GOOGLE_CERT: &[u8] = include_bytes!("fixtures/ct/google-cert.bin");
const CT_GOOGLE_SCT0: &[u8] = include_bytes!("fixtures/ct/google-sct0.bin");
const CT_GOOGLE_SCT1: &[u8] = include_bytes!("fixtures/ct/google-sct1.bin");
const CT_GOOGLE_PILOT_PUBKEY: &[u8] = include_bytes!("fixtures/ct/google-pilot-pubkey.bin");
const CT_SYMANTEC_LOG_PUBKEY: &[u8] = include_bytes!("fixtures/ct/symantec-log-pubkey.bin");
const CT_GOOGLE_PILOT_LOG_ID: [u8; 32] = [
    164, 185, 9, 144, 180, 24, 88, 20, 135, 187, 19, 162, 204, 103, 112, 10, 60, 53, 152, 4, 249,
    27, 223, 184, 227, 119, 205, 14, 200, 13, 220, 16,
];
const CT_SYMANTEC_LOG_ID: [u8; 32] = [
    221, 235, 29, 43, 122, 13, 79, 166, 32, 139, 129, 173, 129, 104, 112, 126, 46, 142, 157, 1,
    213, 92, 136, 141, 61, 17, 196, 205, 182, 236, 190, 204,
];
const CT_GOOGLE_SCT_VERIFICATION_TIME_MS: u64 = 1_499_619_463_644;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

fn temp_payload_file(label: &str, content: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("forge-core-cli-{label}-{}", std::process::id()));
    fs::write(&path, content).expect("write temp payload");
    path
}

fn temp_repo_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!(
        "forge-core-cli-root-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp root");
    path
}

/// Copy a directory tree recursively into `target` (created if missing).
fn copy_dir_recursive(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create target dir");
    for entry in fs::read_dir(source).expect("read source dir") {
        let entry = entry.expect("dir entry");
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path);
        } else {
            fs::copy(&source_path, &target_path).expect("copy file");
        }
    }
}

/// Build a temp validation root that mirrors the Forge core repo's contract
/// tree plus the append-only ledger.
///
/// The core repo is now a normal consumer: its `.forge-method.yaml` Project
/// Link points at a sibling sidecar (`../forge-forge-method-core`) that holds
/// the runtime state, including `ledger.ndjson`. The completion contracts
/// reference `.forge-method/ledger.ndjson` as a repo-relative ref, so a
/// validation root must carry both the contract tree (from the repo) and the
/// ledger (from the sidecar) to pass cleanly. This builds that merged tree in a
/// temp dir and returns it.
fn merged_validation_root(label: &str) -> PathBuf {
    let root = repo_root();
    let temp = temp_repo_root(label);
    copy_dir_recursive(&root.join("contracts"), &temp.join("contracts"));
    copy_dir_recursive(
        &root
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
        &temp
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
    );
    // The ledger lives in the core repo's sibling sidecar via the Project Link;
    // fall back to a repo-local copy for repos that still ship it themselves.
    let ledger_source = [
        root.join("../forge-forge-method-core/.forge-method/ledger.ndjson"),
        root.join(".forge-method").join("ledger.ndjson"),
    ]
    .into_iter()
    .find(|candidate| candidate.exists());
    let ledger_target = temp.join(".forge-method").join("ledger.ndjson");
    fs::create_dir_all(ledger_target.parent().expect("ledger parent"))
        .expect("create .forge-method dir");
    if let Some(ledger_source) = ledger_source {
        fs::copy(&ledger_source, &ledger_target).expect("copy ledger.ndjson");
    } else {
        fs::write(&ledger_target, []).expect("create empty validation ledger");
    }
    temp
}

struct SidecarCliFixture {
    app: PathBuf,
    sidecar: PathBuf,
    state: PathBuf,
}

fn temp_sidecar_cli_fixture(label: &str) -> SidecarCliFixture {
    let parent = temp_repo_root(label);
    let app = parent.join("app");
    let sidecar = parent.join("forge-app");
    let state = sidecar.join(".forge-method");
    fs::create_dir_all(&app).expect("create app root");
    fs::create_dir_all(&state).expect("create sidecar state root");
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");
    SidecarCliFixture {
        app,
        sidecar,
        state,
    }
}

struct SignedProvenanceFixture {
    artifact_path: PathBuf,
    provenance_path: PathBuf,
    signature_path: PathBuf,
    public_key_path: PathBuf,
    transparency_log_path: PathBuf,
    artifact_sha256: String,
    builder_id: String,
    source_uri: String,
    source_ref: String,
}

struct RekorEntryFixture {
    log_entry_path: PathBuf,
    public_key_path: PathBuf,
    expected_log_id: String,
}

struct SigstoreTrustPolicyFixture {
    policy_path: PathBuf,
}

struct FulcioCertificateFixture {
    policy_path: PathBuf,
    certificate_path: PathBuf,
    issuer_certificate_path: PathBuf,
    verification_time_unix: i64,
    leaf_certificate_der: Vec<u8>,
    leaf_key_pair: KeyPair,
    issuer: Issuer<'static, KeyPair>,
}

struct Rfc3161TimestampFixture {
    token_path: PathBuf,
    signature_path: PathBuf,
}

struct CertificateTransparencySctFixture {
    policy_path: PathBuf,
    certificate_path: PathBuf,
    sct0_path: PathBuf,
    sct1_path: PathBuf,
}

struct SigstoreBundleSubjectFixture {
    bundle_path: PathBuf,
    artifact_path: PathBuf,
    policy_path: PathBuf,
    certificate_path: PathBuf,
    issuer_certificate_path: PathBuf,
    rekor_log_entry_path: PathBuf,
    rekor_public_key_path: PathBuf,
    expected_rekor_log_id: String,
}

struct SigstoreDsseInTotoSubjectFixture {
    bundle_path: PathBuf,
    artifact_path: PathBuf,
    policy_path: PathBuf,
    certificate_path: PathBuf,
    issuer_certificate_path: PathBuf,
    rekor_log_entry_path: PathBuf,
    rekor_public_key_path: PathBuf,
    expected_rekor_log_id: String,
    expected_predicate_type: String,
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn rekor_leaf_hash(entry: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update([0]);
    hasher.update(entry);
    hex_bytes(&hasher.finalize())
}

fn signed_provenance_fixture(label: &str) -> SignedProvenanceFixture {
    let root = temp_repo_root(label);
    let artifact_path = root.join("forge-core.exe");
    let provenance_path = root.join("forge-core.intoto.json");
    let signature_path = root.join("forge-core.intoto.sig");
    let public_key_path = root.join("forge-release.pub");
    let transparency_log_path = root.join("forge-core.tlog.json");
    let artifact_bytes = b"forge signed artifact";
    fs::write(&artifact_path, artifact_bytes).expect("write artifact");
    let artifact_sha256 = sha256_content_hash(artifact_bytes);
    let artifact_digest = artifact_sha256
        .strip_prefix("sha256:")
        .expect("sha256 prefix");
    let builder_id = "https://github.com/DanielCarva1/forge-method-core/.github/workflows/release.yml@refs/heads/main".to_string();
    let source_uri = "github.com/DanielCarva1/forge-method-core".to_string();
    let source_ref = "0123456789abcdef0123456789abcdef01234567".to_string();
    let statement = json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [
            {
                "name": "forge-core.exe",
                "digest": {
                    "sha256": artifact_digest
                }
            }
        ],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "builder": {
                "id": builder_id
            },
            "buildDefinition": {
                "buildType": "https://forge.dev/build/release",
                "externalParameters": {
                    "source_uri": source_uri,
                    "source_ref": source_ref
                }
            },
            "resolvedDependencies": [
                {
                    "uri": source_uri,
                    "digest": {
                        "gitCommit": source_ref
                    }
                }
            ]
        }
    });
    let provenance_bytes =
        serde_json::to_vec_pretty(&statement).expect("serialize provenance fixture");
    fs::write(&provenance_path, &provenance_bytes).expect("write provenance");

    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let signature = signing_key.sign(&provenance_bytes);
    fs::write(&signature_path, BASE64.encode(signature.to_bytes())).expect("write signature");
    fs::write(
        &public_key_path,
        BASE64.encode(signing_key.verifying_key().to_bytes()),
    )
    .expect("write public key");

    let provenance_sha256 = sha256_content_hash(&provenance_bytes);
    let signature_sha256 = sha256_content_hash(&signature.to_bytes());
    let mut leaf_payload = Vec::new();
    leaf_payload.push(0);
    leaf_payload.extend_from_slice(
        format!(
            "{}\n{}",
            provenance_sha256.strip_prefix("sha256:").unwrap(),
            signature_sha256.strip_prefix("sha256:").unwrap()
        )
        .as_bytes(),
    );
    let leaf_hash = sha256_content_hash(&leaf_payload);
    let transparency_log = json!({
        "log_id": "forge-release-transparency-log",
        "log_index": 0,
        "tree_size": 1,
        "leaf_hash": leaf_hash,
        "root_hash": leaf_hash,
        "hashes": []
    });
    fs::write(
        &transparency_log_path,
        serde_json::to_vec_pretty(&transparency_log).expect("serialize transparency log"),
    )
    .expect("write transparency log");

    SignedProvenanceFixture {
        artifact_path,
        provenance_path,
        signature_path,
        public_key_path,
        transparency_log_path,
        artifact_sha256,
        builder_id,
        source_uri,
        source_ref,
    }
}

fn rekor_entry_fixture(label: &str) -> RekorEntryFixture {
    let root = temp_repo_root(label);
    let log_entry_path = root.join("rekor-log-entry.json");
    let public_key_path = root.join("rekor.pub");
    let signing_key = P256SigningKey::from_slice(&[8u8; 32]).expect("p256 signing key");
    let public_key_pem = signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .expect("public key pem");
    fs::write(&public_key_path, public_key_pem.as_bytes()).expect("write rekor public key");

    let artifact_sha256 = sha256_content_hash(b"forge rekor artifact");
    let body = json!({
        "kind": "hashedrekord",
        "apiVersion": "0.0.1",
        "spec": {
            "data": {
                "hash": {
                    "algorithm": "sha256",
                    "value": artifact_sha256.strip_prefix("sha256:").expect("sha256 prefix")
                }
            },
            "signature": {
                "content": BASE64.encode(b"artifact signature bytes"),
                "publicKey": {
                    "content": BASE64.encode(public_key_pem.as_bytes())
                }
            }
        }
    });
    let body_bytes = serde_json::to_vec(&body).expect("serialize rekor body");
    let expected_log_id =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
    let canonical_body = serde_json_canonicalizer::to_vec(&body).expect("canonical rekor body");
    let root_hash = rekor_leaf_hash(&canonical_body);
    let root_bytes = (0..root_hash.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&root_hash[index..index + 2], 16).expect("root hex"))
        .collect::<Vec<_>>();
    let checkpoint_body = format!("forge-test-rekor\n1\n{}\n", BASE64.encode(root_bytes));
    let checkpoint_signature: p256::ecdsa::Signature = signing_key.sign(checkpoint_body.as_bytes());
    let mut signed_note_payload = vec![0, 0, 0, 0];
    signed_note_payload.extend_from_slice(checkpoint_signature.to_der().as_bytes());
    let checkpoint = format!(
        "{}\n\\u{{2014}} forge-test {}\n",
        checkpoint_body,
        BASE64.encode(signed_note_payload)
    )
    .replace("\\u{2014}", "\u{2014}");
    let log_entry = json!({
        "body": BASE64.encode(&body_bytes),
        "integratedTime": 1_767_225_600_i64,
        "logID": expected_log_id,
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
    fs::write(
        &log_entry_path,
        serde_json::to_vec_pretty(&log_entry).expect("serialize rekor entry"),
    )
    .expect("write rekor log entry");

    RekorEntryFixture {
        log_entry_path,
        public_key_path,
        expected_log_id,
    }
}

fn sigstore_trust_policy_fixture(label: &str, fulcio_refs: &[&str]) -> SigstoreTrustPolicyFixture {
    let root = temp_repo_root(label);
    let policy_path = root.join("sigstore-trust-policy.yaml");
    let fulcio_refs = if fulcio_refs.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[{}]",
            fulcio_refs
                .iter()
                .map(|item| format!("\"{item}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let policy = format!(
        r#"schema_version: "0.1"
sigstore_trusted_root_policy:
  root_source: "tuf"
  trusted_root_ref: "sigstore-public-good-instance@2026-06-25"
  offline_allowed: true
  fulcio:
    required: true
    certificate_authority_refs: {fulcio_refs}
  rekor:
    required: true
    log_ids:
      - "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    public_key_refs:
      - "rekor.pub"
  certificate_transparency:
    required: true
    log_ids:
      - "ctfe-log-id"
    public_key_refs:
      - "ctfe.pub"
  timestamp_authority:
    mode: "either"
    certificate_refs:
      - "tsa-root.pem"
  identity_policy:
    expected_oidc_issuer: "https://token.actions.githubusercontent.com"
    expected_certificate_identity: "https://github.com/DanielCarva1/forge-method-core/.github/workflows/release.yml@refs/heads/main"
    expected_github_repository: "DanielCarva1/forge-method-core"
    expected_github_ref: "refs/heads/main"
    expected_github_sha: "0123456789abcdef0123456789abcdef01234567"
"#
    );
    fs::write(&policy_path, policy).expect("write Sigstore trust policy");
    SigstoreTrustPolicyFixture { policy_path }
}

fn set_sigstore_revocation_policy(
    policy_path: &Path,
    mode: &str,
    max_certificate_lifetime_seconds: Option<i64>,
) {
    let policy = fs::read_to_string(policy_path).expect("read Sigstore trust policy");
    let max_lifetime_yaml = max_certificate_lifetime_seconds
        .map(|seconds| format!("    max_certificate_lifetime_seconds: {seconds}\n"))
        .unwrap_or_default();
    let revocation_policy =
        format!("  revocation:\n    mode: \"{mode}\"\n{max_lifetime_yaml}  identity_policy:");
    let policy = policy.replace("  identity_policy:", &revocation_policy);
    fs::write(policy_path, policy).expect("write Sigstore revocation policy");
}

fn write_tuf_metadata(root: &Path, role: &str, version: i64, expires: &str) -> PathBuf {
    let path = root.join(format!("{role}.json"));
    let metadata = json!({
        "signatures": [],
        "signed": {
            "_type": role,
            "spec_version": "1.0.0",
            "version": version,
            "expires": expires
        }
    });
    fs::write(
        &path,
        serde_json::to_vec_pretty(&metadata).expect("serialize TUF metadata"),
    )
    .expect("write TUF metadata");
    path
}

fn write_crl_fixture(
    root: &Path,
    label: &str,
    issuer: &Issuer<'static, KeyPair>,
    revoked_leaf: bool,
    this_update: (i32, u8, u8),
    next_update: (i32, u8, u8),
) -> PathBuf {
    let path = root.join(format!("{label}.crl"));
    let revoked_certs = if revoked_leaf {
        vec![RevokedCertParams {
            serial_number: SerialNumber::from(0x1234_u64),
            revocation_time: date_time_ymd(2026, 6, 1),
            reason_code: Some(RevocationReason::KeyCompromise),
            invalidity_date: None,
        }]
    } else {
        Vec::new()
    };
    let crl = CertificateRevocationListParams {
        this_update: date_time_ymd(this_update.0, this_update.1, this_update.2),
        next_update: date_time_ymd(next_update.0, next_update.1, next_update.2),
        crl_number: SerialNumber::from(1_u64),
        issuing_distribution_point: None,
        revoked_certs,
        key_identifier_method: KeyIdMethod::Sha256,
    }
    .signed_by(issuer)
    .expect("sign CRL fixture");
    fs::write(&path, crl.der().as_ref()).expect("write CRL fixture");
    path
}

struct OcspCertificateFixture {
    policy_path: PathBuf,
    certificate_path: PathBuf,
    issuer_certificate_path: PathBuf,
    verification_time_unix: i64,
    issuer_certificate_der: Vec<u8>,
    issuer_key_pair: KeyPair,
    responder_mismatch_name_der: Vec<u8>,
}

#[derive(Clone, Copy)]
enum OcspFixtureCertStatus {
    Good,
    Revoked,
    Unknown,
}

struct OcspResponseFixtureOptions {
    status: OcspFixtureCertStatus,
    produced_at: &'static str,
    this_update: &'static str,
    next_update: Option<&'static str>,
    hash_algorithm_oid: &'static [u64],
    cert_serial: &'static [u8],
    nonce: Option<&'static [u8]>,
    responder_name_der: Option<Vec<u8>>,
    tamper_signature: bool,
}

impl OcspResponseFixtureOptions {
    fn good() -> Self {
        Self {
            status: OcspFixtureCertStatus::Good,
            produced_at: "20260707022640Z",
            this_update: "20260701000000Z",
            next_update: Some("20270701000000Z"),
            hash_algorithm_oid: &[2, 16, 840, 1, 101, 3, 4, 2, 1],
            cert_serial: &[0x12, 0x34],
            nonce: None,
            responder_name_der: None,
            tamper_signature: false,
        }
    }
}

fn ocsp_certificate_fixture(label: &str) -> OcspCertificateFixture {
    let policy = sigstore_trust_policy_fixture(label, &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(&policy.policy_path, "explicit_status_required", None);
    let root = policy.policy_path.parent().expect("policy parent");
    let certificate_path = root.join("fulcio-leaf.pem");
    let issuer_certificate_path = root.join("fulcio-root.pem");
    let verification_time_unix = 1_783_391_200_i64;

    let (ca_certificate, ca_params, ca_key_pair) =
        test_ocsp_ca("Forge Test Fulcio OCSP Root", (2026, 1, 1), (2027, 1, 1));
    let leaf_key_pair = KeyPair::generate().expect("generate OCSP leaf key");
    let issuer = Issuer::from_params(&ca_params, &ca_key_pair);
    let leaf_certificate = test_ocsp_leaf(&issuer, &leaf_key_pair);
    fs::write(&issuer_certificate_path, ca_certificate.pem()).expect("write OCSP root");
    fs::write(&certificate_path, leaf_certificate.pem()).expect("write OCSP leaf");

    let (mismatch_certificate, _, _) = test_ocsp_ca(
        "Forge Test Unauthorized OCSP Responder",
        (2026, 1, 1),
        (2027, 1, 1),
    );
    let mismatch_certificate_der = mismatch_certificate.der().to_vec();

    OcspCertificateFixture {
        policy_path: policy.policy_path,
        certificate_path,
        issuer_certificate_path,
        verification_time_unix,
        issuer_certificate_der: ca_certificate.der().to_vec(),
        issuer_key_pair: ca_key_pair,
        responder_mismatch_name_der: x509_subject_der(&mismatch_certificate_der),
    }
}

fn test_ocsp_ca(
    common_name: &str,
    not_before: (i32, u8, u8),
    not_after: (i32, u8, u8),
) -> (Certificate, CertificateParams, KeyPair) {
    let mut params =
        CertificateParams::new(Vec::default()).expect("empty SAN can create OCSP CA params");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, common_name);
    params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    params.key_usages.push(KeyUsagePurpose::CrlSign);
    params.not_before = date_time_ymd(not_before.0, not_before.1, not_before.2);
    params.not_after = date_time_ymd(not_after.0, not_after.1, not_after.2);
    let key_pair = KeyPair::generate().expect("generate OCSP CA key");
    let certificate = params.self_signed(&key_pair).expect("self-sign OCSP CA");
    (certificate, params, key_pair)
}

fn test_ocsp_leaf(issuer: &Issuer<'_, &KeyPair>, key_pair: &KeyPair) -> Certificate {
    let mut params =
        CertificateParams::new(Vec::default()).expect("empty SAN can create OCSP leaf params");
    params.serial_number = Some(SerialNumber::from(0x1234_u64));
    params
        .distinguished_name
        .push(DnType::CommonName, "Forge Test OCSP Leaf");
    params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::CodeSigning);
    params.not_before = date_time_ymd(2026, 1, 1);
    params.not_after = date_time_ymd(2027, 1, 1);
    params
        .signed_by(key_pair, issuer)
        .expect("sign OCSP leaf certificate")
}

fn write_ocsp_response_fixture(
    fixture: &OcspCertificateFixture,
    label: &str,
    options: OcspResponseFixtureOptions,
) -> PathBuf {
    let root = fixture.policy_path.parent().expect("policy parent");
    let path = root.join(format!("{label}.ocsp.der"));
    let ocsp_response = ocsp_response_der(fixture, options);
    fs::write(&path, ocsp_response).expect("write OCSP response fixture");
    path
}

fn ocsp_response_der(
    fixture: &OcspCertificateFixture,
    options: OcspResponseFixtureOptions,
) -> Vec<u8> {
    let issuer_subject_der = x509_subject_der(&fixture.issuer_certificate_der);
    let responder_name_der = options
        .responder_name_der
        .unwrap_or_else(|| issuer_subject_der.clone());
    let responder_id = der_context_explicit(1, &responder_name_der);
    let produced_at = der_generalized_time(options.produced_at);
    let single_response = der_ocsp_single_response(
        fixture,
        options.status,
        options.this_update,
        options.next_update,
        options.hash_algorithm_oid,
        options.cert_serial,
    );
    let responses = der_sequence(&[single_response]);
    let mut response_data_parts = vec![responder_id, produced_at, responses];
    if let Some(nonce) = options.nonce {
        response_data_parts.push(der_context_explicit(
            1,
            &der_sequence(&[der_ocsp_nonce_extension(nonce)]),
        ));
    }
    let response_data = der_sequence(&response_data_parts);
    let mut signature = fixture
        .issuer_key_pair
        .sign(&response_data)
        .expect("sign OCSP response data");
    if options.tamper_signature {
        let last = signature.last_mut().expect("OCSP signature byte");
        *last ^= 0x01;
    }
    let basic_ocsp_response = der_sequence(&[
        response_data,
        der_algorithm_identifier(&[1, 2, 840, 10045, 4, 3, 2]),
        der_bit_string(&signature),
    ]);
    let response_bytes = der_sequence(&[
        der_oid(&[1, 3, 6, 1, 5, 5, 7, 48, 1, 1]),
        der_octet_string(&basic_ocsp_response),
    ]);
    der_sequence(&[der_enumerated(0), der_context_explicit(0, &response_bytes)])
}

fn der_ocsp_single_response(
    fixture: &OcspCertificateFixture,
    status: OcspFixtureCertStatus,
    this_update: &str,
    next_update: Option<&str>,
    hash_algorithm_oid: &[u64],
    cert_serial: &[u8],
) -> Vec<u8> {
    let mut parts = vec![
        der_ocsp_cert_id(
            &fixture.issuer_certificate_der,
            hash_algorithm_oid,
            cert_serial,
        ),
        der_ocsp_cert_status(status),
        der_generalized_time(this_update),
    ];
    if let Some(next_update) = next_update {
        parts.push(der_context_explicit(0, &der_generalized_time(next_update)));
    }
    der_sequence(&parts)
}

fn der_ocsp_cert_id(
    issuer_certificate_der: &[u8],
    hash_algorithm_oid: &[u64],
    cert_serial: &[u8],
) -> Vec<u8> {
    let (_, issuer_certificate) =
        X509Certificate::from_der(issuer_certificate_der).expect("parse OCSP issuer certificate");
    let issuer_name_hash = Sha256::digest(issuer_certificate.tbs_certificate.subject.as_raw());
    let issuer_key_hash = Sha256::digest(
        issuer_certificate
            .tbs_certificate
            .subject_pki
            .subject_public_key
            .data
            .as_ref(),
    );
    der_sequence(&[
        der_algorithm_identifier(hash_algorithm_oid),
        der_octet_string(&issuer_name_hash),
        der_octet_string(&issuer_key_hash),
        der_integer_positive(cert_serial),
    ])
}

fn der_ocsp_cert_status(status: OcspFixtureCertStatus) -> Vec<u8> {
    match status {
        OcspFixtureCertStatus::Good => der_context_primitive(0, &[]),
        OcspFixtureCertStatus::Revoked => der(0xa1, &der_generalized_time("20260615000000Z")),
        OcspFixtureCertStatus::Unknown => der_context_primitive(2, &[]),
    }
}

fn der_ocsp_nonce_extension(nonce: &[u8]) -> Vec<u8> {
    der_sequence(&[
        der_oid(&[1, 3, 6, 1, 5, 5, 7, 48, 1, 2]),
        der_octet_string(&der_octet_string(nonce)),
    ])
}

fn x509_subject_der(certificate_der: &[u8]) -> Vec<u8> {
    let (_, certificate) =
        X509Certificate::from_der(certificate_der).expect("parse certificate subject");
    certificate.tbs_certificate.subject.as_raw().to_vec()
}

fn der_sequence(parts: &[Vec<u8>]) -> Vec<u8> {
    let content_len = parts.iter().map(Vec::len).sum();
    let mut content = Vec::with_capacity(content_len);
    for part in parts {
        content.extend_from_slice(part);
    }
    der(0x30, &content)
}

fn der_context_explicit(tag_number: u8, content: &[u8]) -> Vec<u8> {
    der(0xa0 | tag_number, content)
}

fn der_context_primitive(tag_number: u8, content: &[u8]) -> Vec<u8> {
    der(0x80 | tag_number, content)
}

fn der_algorithm_identifier(oid: &[u64]) -> Vec<u8> {
    der_sequence(&[der_oid(oid)])
}

fn der_oid(arcs: &[u64]) -> Vec<u8> {
    assert!(arcs.len() >= 2, "OID needs at least two arcs");
    let mut body = Vec::new();
    body.push(u8::try_from((arcs[0] * 40) + arcs[1]).expect("OID first arcs fit in u8"));
    for arc in &arcs[2..] {
        let mut encoded = vec![u8::try_from(arc & 0x7f).expect("OID arc byte")];
        let mut value = arc >> 7;
        while value > 0 {
            let byte = u8::try_from(value & 0x7f).expect("OID arc byte") | 0x80;
            encoded.push(byte);
            value >>= 7;
        }
        body.extend(encoded.iter().rev());
    }
    der(0x06, &body)
}

fn der_octet_string(content: &[u8]) -> Vec<u8> {
    der(0x04, content)
}

fn der_bit_string(content: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(content.len() + 1);
    body.push(0);
    body.extend_from_slice(content);
    der(0x03, &body)
}

fn der_integer_positive(content: &[u8]) -> Vec<u8> {
    let first_non_zero = content
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or_else(|| content.len().saturating_sub(1));
    let mut body = content[first_non_zero..].to_vec();
    if body.first().is_some_and(|byte| byte & 0x80 != 0) {
        body.insert(0, 0);
    }
    der(0x02, &body)
}

fn der_enumerated(value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let first_non_zero = bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len() - 1);
    der(0x0a, &bytes[first_non_zero..])
}

fn der_generalized_time(value: &str) -> Vec<u8> {
    der(0x18, value.as_bytes())
}

fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(1 + 5 + content.len());
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

fn fulcio_certificate_fixture(label: &str, fulcio_refs: &[&str]) -> FulcioCertificateFixture {
    fulcio_certificate_fixture_with_validity(label, fulcio_refs, (2026, 1, 1), (2027, 1, 1))
}

fn fulcio_certificate_fixture_with_validity(
    label: &str,
    fulcio_refs: &[&str],
    not_before: (i32, u8, u8),
    not_after: (i32, u8, u8),
) -> FulcioCertificateFixture {
    let policy = sigstore_trust_policy_fixture(label, fulcio_refs);
    let root = policy.policy_path.parent().expect("policy parent");
    let certificate_path = root.join("fulcio-leaf.pem");
    let issuer_certificate_path = root.join("fulcio-root.pem");
    let verification_time_unix = 1_783_391_200_i64;

    let (ca_certificate, issuer) = test_fulcio_ca_with_validity(not_before, not_after);
    let leaf_key_pair = KeyPair::generate().expect("generate Fulcio leaf key");
    let leaf_certificate =
        test_fulcio_leaf_with_validity(&issuer, &leaf_key_pair, not_before, not_after);

    fs::write(&issuer_certificate_path, ca_certificate.pem()).expect("write Fulcio root");
    fs::write(&certificate_path, leaf_certificate.pem()).expect("write Fulcio leaf");

    FulcioCertificateFixture {
        policy_path: policy.policy_path,
        certificate_path,
        issuer_certificate_path,
        verification_time_unix,
        leaf_certificate_der: leaf_certificate.der().to_vec(),
        leaf_key_pair,
        issuer,
    }
}

fn test_fulcio_ca_with_validity(
    not_before: (i32, u8, u8),
    not_after: (i32, u8, u8),
) -> (Certificate, Issuer<'static, KeyPair>) {
    let mut params =
        CertificateParams::new(Vec::default()).expect("empty SAN can create CA params");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, "Forge Test Fulcio Root");
    params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    params.key_usages.push(KeyUsagePurpose::CrlSign);
    params.not_before = date_time_ymd(not_before.0, not_before.1, not_before.2);
    params.not_after = date_time_ymd(not_after.0, not_after.1, not_after.2);
    let key_pair = KeyPair::generate().expect("generate Fulcio CA key");
    let certificate = params.self_signed(&key_pair).expect("self-sign Fulcio CA");
    (certificate, Issuer::new(params, key_pair))
}

fn test_fulcio_leaf_with_validity(
    issuer: &Issuer<'static, KeyPair>,
    key_pair: &KeyPair,
    not_before: (i32, u8, u8),
    not_after: (i32, u8, u8),
) -> Certificate {
    let identity =
        "https://github.com/DanielCarva1/forge-method-core/.github/workflows/release.yml@refs/heads/main";
    let repository = "https://github.com/DanielCarva1/forge-method-core";
    let git_ref = "refs/heads/main";
    let git_sha = "0123456789abcdef0123456789abcdef01234567";
    let mut params =
        CertificateParams::new(Vec::default()).expect("empty SAN can create leaf params");
    params.serial_number = Some(SerialNumber::from(0x1234_u64));
    params
        .subject_alt_names
        .push(SanType::URI(identity.try_into().expect("URI SAN")));
    params
        .distinguished_name
        .push(DnType::CommonName, "Forge Test Release Identity");
    params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::CodeSigning);
    params.not_before = date_time_ymd(not_before.0, not_before.1, not_before.2);
    params.not_after = date_time_ymd(not_after.0, not_after.1, not_after.2);
    params
        .custom_extensions
        .push(CustomExtension::from_oid_content(
            &[1, 3, 6, 1, 4, 1, 57264, 1, 8],
            der_utf8("https://token.actions.githubusercontent.com"),
        ));
    params
        .custom_extensions
        .push(CustomExtension::from_oid_content(
            &[1, 3, 6, 1, 4, 1, 57264, 1, 9],
            der_utf8(identity),
        ));
    params
        .custom_extensions
        .push(CustomExtension::from_oid_content(
            &[1, 3, 6, 1, 4, 1, 57264, 1, 10],
            der_utf8(git_sha),
        ));
    params
        .custom_extensions
        .push(CustomExtension::from_oid_content(
            &[1, 3, 6, 1, 4, 1, 57264, 1, 12],
            der_utf8(repository),
        ));
    params
        .custom_extensions
        .push(CustomExtension::from_oid_content(
            &[1, 3, 6, 1, 4, 1, 57264, 1, 13],
            der_utf8(git_sha),
        ));
    params
        .custom_extensions
        .push(CustomExtension::from_oid_content(
            &[1, 3, 6, 1, 4, 1, 57264, 1, 14],
            der_utf8(git_ref),
        ));
    params
        .custom_extensions
        .push(CustomExtension::from_oid_content(
            &[1, 3, 6, 1, 4, 1, 57264, 1, 24],
            der_utf8("repo:DanielCarva1/forge-method-core:ref:refs/heads/main"),
        ));

    params
        .signed_by(key_pair, issuer)
        .expect("sign Fulcio leaf certificate")
}

fn der_utf8(value: &str) -> Vec<u8> {
    der(0x0c, value.as_bytes())
}

fn install_rfc3161_timestamp_fixture(
    policy_path: &Path,
    bundle_json: &str,
    trusted_root_json: &str,
) -> Rfc3161TimestampFixture {
    let root = policy_path.parent().expect("policy parent");
    let token_path = root.join("rfc3161-token.der");
    let signature_path = root.join("timestamped-signature.bin");
    fs::write(&token_path, extract_rfc3161_timestamp_token(bundle_json))
        .expect("write RFC3161 timestamp token");
    fs::write(&signature_path, extract_rfc3161_signature(bundle_json))
        .expect("write timestamped signature");

    let tsa_certificates = extract_rfc3161_tsa_certificates(trusted_root_json);
    let certificate_refs = tsa_certificates
        .iter()
        .enumerate()
        .map(|(index, certificate)| {
            let name = format!("tsa-{index}.der");
            fs::write(root.join(&name), certificate).expect("write TSA certificate");
            name
        })
        .collect::<Vec<_>>();

    let certificate_refs_yaml = certificate_refs
        .iter()
        .map(|name| format!("      - \"{name}\""))
        .collect::<Vec<_>>()
        .join("\n");
    let policy = fs::read_to_string(policy_path).expect("read trust policy");
    let policy = policy
        .replace("mode: \"either\"", "mode: \"rfc3161_tsa\"")
        .replace(
            "    certificate_refs:\n      - \"tsa-root.pem\"",
            &format!("    certificate_refs:\n{certificate_refs_yaml}"),
        );
    fs::write(policy_path, policy).expect("write RFC3161 trust policy");

    Rfc3161TimestampFixture {
        token_path,
        signature_path,
    }
}

fn certificate_transparency_sct_fixture(label: &str) -> CertificateTransparencySctFixture {
    let policy = sigstore_trust_policy_fixture(label, &["fulcio-root.pem"]);
    let root = policy.policy_path.parent().expect("policy parent");
    let certificate_path = root.join("ct-google-cert.der");
    let sct0_path = root.join("google-sct0.bin");
    let sct1_path = root.join("google-sct1.bin");
    fs::write(&certificate_path, CT_GOOGLE_CERT).expect("write CT certificate");
    fs::write(&sct0_path, CT_GOOGLE_SCT0).expect("write CT SCT0");
    fs::write(&sct1_path, CT_GOOGLE_SCT1).expect("write CT SCT1");
    fs::write(root.join("google-pilot-pubkey.bin"), CT_GOOGLE_PILOT_PUBKEY)
        .expect("write Google Pilot CT key");
    fs::write(root.join("symantec-log-pubkey.bin"), CT_SYMANTEC_LOG_PUBKEY)
        .expect("write Symantec CT key");

    let policy_text = fs::read_to_string(&policy.policy_path).expect("read CT policy");
    let policy_text = policy_text.replace(
        "    log_ids:\n      - \"ctfe-log-id\"\n    public_key_refs:\n      - \"ctfe.pub\"",
        &format!(
            "    log_ids:\n      - \"{}\"\n      - \"{}\"\n    public_key_refs:\n      - \"google-pilot-pubkey.bin\"\n      - \"symantec-log-pubkey.bin\"",
            hex_bytes(&CT_GOOGLE_PILOT_LOG_ID),
            hex_bytes(&CT_SYMANTEC_LOG_ID)
        ),
    );
    fs::write(&policy.policy_path, policy_text).expect("write CT policy");

    CertificateTransparencySctFixture {
        policy_path: policy.policy_path,
        certificate_path,
        sct0_path,
        sct1_path,
    }
}

fn extract_rfc3161_timestamp_token(bundle_json: &str) -> Vec<u8> {
    let bundle: Value = serde_json::from_str(bundle_json).expect("parse RFC3161 bundle");
    let signed_timestamp = bundle["verificationMaterial"]["timestampVerificationData"]
        ["rfc3161Timestamps"][0]["signedTimestamp"]
        .as_str()
        .expect("signed timestamp");
    BASE64
        .decode(signed_timestamp.as_bytes())
        .expect("decode signed timestamp")
}

fn extract_rfc3161_signature(bundle_json: &str) -> Vec<u8> {
    let bundle: Value = serde_json::from_str(bundle_json).expect("parse RFC3161 bundle");
    let signature = bundle["messageSignature"]["signature"]
        .as_str()
        .expect("message signature");
    BASE64
        .decode(signature.as_bytes())
        .expect("decode message signature")
}

fn extract_rfc3161_tsa_certificates(trusted_root_json: &str) -> Vec<Vec<u8>> {
    let root: Value = serde_json::from_str(trusted_root_json).expect("parse RFC3161 trusted root");
    root["timestampAuthorities"]
        .as_array()
        .expect("timestamp authorities")
        .iter()
        .flat_map(|tsa| {
            tsa["certChain"]["certificates"]
                .as_array()
                .expect("TSA cert chain")
                .iter()
        })
        .map(|certificate| {
            let raw = certificate["rawBytes"].as_str().expect("TSA raw bytes");
            BASE64
                .decode(raw.as_bytes())
                .expect("decode TSA certificate")
        })
        .collect()
}

fn sigstore_bundle_subject_fixture(label: &str) -> SigstoreBundleSubjectFixture {
    let fulcio = fulcio_certificate_fixture(label, &["fulcio-root.pem"]);
    let root = fulcio.policy_path.parent().expect("policy parent");
    let artifact_path = root.join("forge-core-bundle.exe");
    let bundle_path = root.join("forge-core.sigstore-bundle.json");
    let artifact_bytes = b"forge sigstore bundle subject artifact";
    fs::write(&artifact_path, artifact_bytes).expect("write bundle artifact");

    let artifact_digest = Sha256::digest(artifact_bytes).to_vec();
    let signature = RcgenSigningKey::sign(&fulcio.leaf_key_pair, &artifact_digest)
        .expect("sign bundle digest with Fulcio leaf key");
    let bundle = json!({
        "mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json",
        "verificationMaterial": {
            "certificate": {
                "rawBytes": BASE64.encode(&fulcio.leaf_certificate_der)
            }
        },
        "messageSignature": {
            "messageDigest": {
                "algorithm": "SHA2_256",
                "digest": BASE64.encode(&artifact_digest)
            },
            "signature": BASE64.encode(&signature)
        }
    });
    fs::write(
        &bundle_path,
        serde_json::to_vec_pretty(&bundle).expect("serialize bundle"),
    )
    .expect("write bundle");

    let rekor = rekor_entry_fixture_for_bundle(label, &artifact_digest, &signature);

    SigstoreBundleSubjectFixture {
        bundle_path,
        artifact_path,
        policy_path: fulcio.policy_path,
        certificate_path: fulcio.certificate_path,
        issuer_certificate_path: fulcio.issuer_certificate_path,
        rekor_log_entry_path: rekor.log_entry_path,
        rekor_public_key_path: rekor.public_key_path,
        expected_rekor_log_id: rekor.expected_log_id,
    }
}

fn rekor_entry_fixture_for_bundle(
    label: &str,
    artifact_digest: &[u8],
    signature: &[u8],
) -> RekorEntryFixture {
    let root = temp_repo_root(&format!("{label}-bundle-rekor"));
    let log_entry_path = root.join("rekor-log-entry.json");
    let public_key_path = root.join("rekor.pub");
    let signing_key = P256SigningKey::from_slice(&[9u8; 32]).expect("p256 signing key");
    let public_key_pem = signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .expect("public key pem");
    fs::write(&public_key_path, public_key_pem.as_bytes()).expect("write rekor public key");

    let expected_log_id =
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string();
    let body = json!({
        "kind": "hashedrekord",
        "apiVersion": "0.0.1",
        "spec": {
            "data": {
                "hash": {
                    "algorithm": "sha256",
                    "value": hex_bytes(artifact_digest)
                }
            },
            "signature": {
                "content": BASE64.encode(signature),
                "publicKey": {
                    "content": BASE64.encode(b"bundle certificate public key")
                }
            }
        }
    });
    let body_bytes = serde_json::to_vec(&body).expect("serialize rekor body");
    let canonical_body = serde_json_canonicalizer::to_vec(&body).expect("canonical rekor body");
    let root_hash = rekor_leaf_hash(&canonical_body);
    let root_bytes = (0..root_hash.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&root_hash[index..index + 2], 16).expect("root hex"))
        .collect::<Vec<_>>();
    let checkpoint_body = format!("forge-test-rekor\n1\n{}\n", BASE64.encode(root_bytes));
    let checkpoint_signature: p256::ecdsa::Signature = signing_key.sign(checkpoint_body.as_bytes());
    let mut signed_note_payload = vec![0, 0, 0, 0];
    signed_note_payload.extend_from_slice(checkpoint_signature.to_der().as_bytes());
    let checkpoint = format!(
        "{}\n\\u{{2014}} forge-test {}\n",
        checkpoint_body,
        BASE64.encode(signed_note_payload)
    )
    .replace("\\u{2014}", "\u{2014}");
    let log_entry = json!({
        "body": BASE64.encode(&body_bytes),
        "integratedTime": 1_783_391_200_i64,
        "logID": expected_log_id,
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
    fs::write(
        &log_entry_path,
        serde_json::to_vec_pretty(&log_entry).expect("serialize rekor entry"),
    )
    .expect("write rekor log entry");

    RekorEntryFixture {
        log_entry_path,
        public_key_path,
        expected_log_id,
    }
}

fn dsse_pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let payload_type = payload_type.as_bytes();
    let mut out = Vec::new();
    out.extend_from_slice(b"DSSEv1 ");
    out.extend_from_slice(payload_type.len().to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(payload_type);
    out.push(b' ');
    out.extend_from_slice(payload.len().to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(payload);
    out
}

fn sigstore_dsse_in_toto_subject_fixture(label: &str) -> SigstoreDsseInTotoSubjectFixture {
    let fulcio = fulcio_certificate_fixture(label, &["fulcio-root.pem"]);
    let root = fulcio.policy_path.parent().expect("policy parent");
    let artifact_path = root.join("forge-core-dsse.exe");
    let bundle_path = root.join("forge-core.dsse.sigstore-bundle.json");
    let artifact_bytes = b"forge sigstore dsse in-toto subject artifact";
    fs::write(&artifact_path, artifact_bytes).expect("write dsse artifact");

    let artifact_digest = hex_bytes(&Sha256::digest(artifact_bytes));
    let payload_type = "application/vnd.in-toto+json";
    let expected_predicate_type = "https://slsa.dev/provenance/v1".to_string();
    let statement = json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [
            {
                "name": "forge-core-dsse.exe",
                "digest": {
                    "sha256": artifact_digest
                }
            }
        ],
        "predicateType": expected_predicate_type,
        "predicate": {
            "builder": {
                "id": "https://github.com/DanielCarva1/forge-method-core/actions"
            }
        }
    });
    let payload = serde_json::to_vec(&statement).expect("serialize in-toto statement");
    let pae = dsse_pae(payload_type, &payload);
    let signature =
        RcgenSigningKey::sign(&fulcio.leaf_key_pair, &pae).expect("sign DSSE PAE with leaf key");
    let envelope = json!({
        "payloadType": payload_type,
        "payload": BASE64.encode(&payload),
        "signatures": [
            {
                "keyid": "forge-test",
                "sig": BASE64.encode(&signature)
            }
        ]
    });
    let bundle = json!({
        "mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json",
        "verificationMaterial": {
            "certificate": {
                "rawBytes": BASE64.encode(&fulcio.leaf_certificate_der)
            }
        },
        "dsseEnvelope": envelope
    });
    fs::write(
        &bundle_path,
        serde_json::to_vec_pretty(&bundle).expect("serialize dsse bundle"),
    )
    .expect("write dsse bundle");

    let rekor = rekor_entry_fixture_for_dsse(label, &payload, &envelope, &signature);

    SigstoreDsseInTotoSubjectFixture {
        bundle_path,
        artifact_path,
        policy_path: fulcio.policy_path,
        certificate_path: fulcio.certificate_path,
        issuer_certificate_path: fulcio.issuer_certificate_path,
        rekor_log_entry_path: rekor.log_entry_path,
        rekor_public_key_path: rekor.public_key_path,
        expected_rekor_log_id: rekor.expected_log_id,
        expected_predicate_type,
    }
}

fn rekor_entry_fixture_for_dsse(
    label: &str,
    payload: &[u8],
    envelope: &Value,
    signature: &[u8],
) -> RekorEntryFixture {
    let root = temp_repo_root(&format!("{label}-dsse-rekor"));
    let log_entry_path = root.join("rekor-log-entry.json");
    let public_key_path = root.join("rekor.pub");
    let signing_key = P256SigningKey::from_slice(&[10u8; 32]).expect("p256 signing key");
    let public_key_pem = signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .expect("public key pem");
    fs::write(&public_key_path, public_key_pem.as_bytes()).expect("write rekor public key");

    let expected_log_id =
        "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".to_string();
    let canonical_envelope =
        serde_json_canonicalizer::to_vec(envelope).expect("canonical DSSE envelope");
    let body = json!({
        "kind": "dsse",
        "apiVersion": "0.0.1",
        "spec": {
            "dsseObj": {
                "payloadHash": {
                    "algorithm": "sha256",
                    "value": hex_bytes(&Sha256::digest(payload))
                },
                "envelopeHash": {
                    "algorithm": "sha256",
                    "value": hex_bytes(&Sha256::digest(&canonical_envelope))
                },
                "signatures": [
                    {
                        "signature": BASE64.encode(signature)
                    }
                ]
            }
        }
    });
    let body_bytes = serde_json::to_vec(&body).expect("serialize DSSE rekor body");
    let canonical_body = serde_json_canonicalizer::to_vec(&body).expect("canonical rekor body");
    let root_hash = rekor_leaf_hash(&canonical_body);
    let root_bytes = (0..root_hash.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&root_hash[index..index + 2], 16).expect("root hex"))
        .collect::<Vec<_>>();
    let checkpoint_body = format!("forge-test-rekor\n1\n{}\n", BASE64.encode(root_bytes));
    let checkpoint_signature: p256::ecdsa::Signature = signing_key.sign(checkpoint_body.as_bytes());
    let mut signed_note_payload = vec![0, 0, 0, 0];
    signed_note_payload.extend_from_slice(checkpoint_signature.to_der().as_bytes());
    let checkpoint = format!(
        "{}\n\\u{{2014}} forge-test {}\n",
        checkpoint_body,
        BASE64.encode(signed_note_payload)
    )
    .replace("\\u{2014}", "\u{2014}");
    let log_entry = json!({
        "body": BASE64.encode(&body_bytes),
        "integratedTime": 1_783_391_200_i64,
        "logID": expected_log_id,
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
    fs::write(
        &log_entry_path,
        serde_json::to_vec_pretty(&log_entry).expect("serialize rekor entry"),
    )
    .expect("write rekor log entry");

    RekorEntryFixture {
        log_entry_path,
        public_key_path,
        expected_log_id,
    }
}

#[test]
fn validate_library_passes_current_repo() {
    let root = merged_validation_root("validate-library-current-repo");
    let summary = run_validate(&root);
    assert_eq!(summary.status, ValidationStatus::Passed);
    assert!(
        summary.diagnostics.is_empty(),
        "diagnostics: {:?}",
        summary.diagnostics
    );
    assert!(!summary.checks.is_empty());
}

#[test]
fn validate_binary_outputs_json_summary() {
    let root = merged_validation_root("validate-binary-json-summary");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["validate", "--root"])
        .arg(&root)
        .arg("--json")
        .output()
        .expect("run forge-core validate");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    let checks = json["checks"].as_array().expect("checks array");
    assert!(checks.len() > 1);
    assert!(checks.iter().any(|item| item["name"] == "yaml_parse"));
    assert!(checks
        .iter()
        .any(|item| item["name"] == "yaml_source_id_refs"));
    assert!(checks
        .iter()
        .any(|item| item["name"] == "yaml_known_repo_refs"));
    assert_eq!(
        json["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .len(),
        0
    );
}

#[test]
fn host_adapter_manifest_library_classifies_command_authority() {
    let manifest = run_host_adapter_manifest();
    assert_eq!(manifest.schema_version, "0.1");
    assert!(manifest
        .authority_boundary
        .adapters_must_not
        .iter()
        .any(|item| item.contains("auto-run mutating operations")));

    let execute = manifest
        .commands
        .iter()
        .find(|command| command.name == "execute-operation")
        .expect("execute-operation command");
    assert_eq!(
        execute.command_kind,
        HostAdapterCommandKind::OperationExecution
    );
    assert_eq!(
        execute.mutation_class,
        HostAdapterMutationClass::MutatingOperation
    );
    assert_eq!(
        execute.authority_class,
        HostAdapterAuthorityClass::RequiresOperationAuthority
    );
    assert!(
        execute.safe_auto_invocation_triggers.is_empty(),
        "mutating operation must not be auto-invoked"
    );
    assert!(execute
        .required_contracts
        .iter()
        .any(|item| item == "OperationContract"));

    let query = manifest
        .commands
        .iter()
        .find(|command| command.name == "query-effect-index")
        .expect("query-effect-index command");
    assert_eq!(query.mutation_class, HostAdapterMutationClass::ReadOnly);
    assert_eq!(
        query.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(query
        .safe_auto_invocation_triggers
        .contains(&HostAdapterAutoTrigger::EvidenceDiscovery));

    let verify_artifact = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-artifact")
        .expect("host-adapter-verify-artifact command");
    assert_eq!(
        verify_artifact.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_artifact.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_artifact.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_artifact
        .policy_refs
        .contains(&"contracts/policies/release-artifact-verification-boundary.yaml".to_string()));

    let verify_provenance = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-provenance")
        .expect("host-adapter-verify-provenance command");
    assert_eq!(
        verify_provenance.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_provenance.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_provenance.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_provenance.policy_refs.contains(
        &"contracts/policies/signature-and-provenance-verification-boundary.yaml".to_string()
    ));

    let verify_rekor = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-rekor-entry")
        .expect("host-adapter-verify-rekor-entry command");
    assert_eq!(
        verify_rekor.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_rekor.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_rekor.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_rekor
        .policy_refs
        .contains(&"contracts/policies/sigstore-rekor-backend-boundary.yaml".to_string()));

    let verify_sigstore_trust_policy = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-sigstore-trust-policy")
        .expect("host-adapter-verify-sigstore-trust-policy command");
    assert_eq!(
        verify_sigstore_trust_policy.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_sigstore_trust_policy.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_sigstore_trust_policy.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_sigstore_trust_policy
        .policy_refs
        .contains(&"contracts/policies/sigstore-trusted-root-policy-boundary.yaml".to_string()));

    let verify_fulcio_identity = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-fulcio-certificate-identity")
        .expect("host-adapter-verify-fulcio-certificate-identity command");
    assert_eq!(
        verify_fulcio_identity.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_fulcio_identity.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_fulcio_identity.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_fulcio_identity.policy_refs.contains(
        &"contracts/policies/sigstore-fulcio-certificate-identity-boundary.yaml".to_string()
    ));

    let verify_bundle_subject = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-sigstore-bundle-subject")
        .expect("host-adapter-verify-sigstore-bundle-subject command");
    assert_eq!(
        verify_bundle_subject.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_bundle_subject.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_bundle_subject.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_bundle_subject
        .policy_refs
        .contains(&"contracts/policies/sigstore-bundle-subject-binding-boundary.yaml".to_string()));

    let verify_dsse_subject = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-sigstore-dsse-in-toto-subject")
        .expect("host-adapter-verify-sigstore-dsse-in-toto-subject command");
    assert_eq!(
        verify_dsse_subject.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_dsse_subject.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_dsse_subject.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_dsse_subject
        .policy_refs
        .contains(&"contracts/policies/sigstore-dsse-in-toto-subject-boundary.yaml".to_string()));

    let verify_timestamp_authority = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-sigstore-timestamp-authority")
        .expect("host-adapter-verify-sigstore-timestamp-authority command");
    assert_eq!(
        verify_timestamp_authority.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_timestamp_authority.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_timestamp_authority.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_timestamp_authority
        .policy_refs
        .contains(&"contracts/policies/sigstore-timestamp-authority-boundary.yaml".to_string()));
    assert!(verify_timestamp_authority
        .policy_refs
        .contains(&"contracts/policies/sigstore-rfc3161-tsa-token-boundary.yaml".to_string()));

    let verify_ct_sct = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-certificate-transparency-sct")
        .expect("host-adapter-verify-certificate-transparency-sct command");
    assert_eq!(
        verify_ct_sct.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_ct_sct.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_ct_sct.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_ct_sct
        .policy_refs
        .contains(&"contracts/policies/certificate-transparency-sct-boundary.yaml".to_string()));

    let verify_revocation_policy = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-certificate-revocation-policy")
        .expect("host-adapter-verify-certificate-revocation-policy command");
    assert_eq!(
        verify_revocation_policy.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_revocation_policy.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_revocation_policy.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_revocation_policy
        .policy_refs
        .contains(&"contracts/policies/certificate-revocation-policy-boundary.yaml".to_string()));

    let verify_tuf_freshness = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-tuf-trusted-root-freshness")
        .expect("host-adapter-verify-tuf-trusted-root-freshness command");
    assert_eq!(
        verify_tuf_freshness.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_tuf_freshness.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_tuf_freshness.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_tuf_freshness
        .policy_refs
        .contains(&"contracts/policies/tuf-trusted-root-freshness-boundary.yaml".to_string()));

    let verify_crl_status = manifest
        .commands
        .iter()
        .find(|command| command.name == "host-adapter-verify-certificate-crl-status")
        .expect("host-adapter-verify-certificate-crl-status command");
    assert_eq!(
        verify_crl_status.command_kind,
        HostAdapterCommandKind::Validation
    );
    assert_eq!(
        verify_crl_status.mutation_class,
        HostAdapterMutationClass::ReadOnly
    );
    assert_eq!(
        verify_crl_status.authority_class,
        HostAdapterAuthorityClass::NoWorkflowAuthority
    );
    assert!(verify_crl_status
        .policy_refs
        .contains(&"contracts/policies/explicit-crl-revocation-status-boundary.yaml".to_string()));
}

#[test]
fn host_adapter_manifest_binary_outputs_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["host-adapter-manifest", "--json"])
        .output()
        .expect("run forge-core host-adapter-manifest");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["manifest_id"], "forge_core_host_adapter_manifest_v0");
    let commands = json["commands"].as_array().expect("commands array");
    assert!(commands.iter().any(|item| item["name"] == "validate"));
    let execute = commands
        .iter()
        .find(|item| item["name"] == "execute-operation")
        .expect("execute-operation command");
    assert_eq!(execute["mutation_class"], "mutating_operation");
    assert_eq!(execute["authority_class"], "requires_operation_authority");
    assert!(execute["safe_auto_invocation_triggers"]
        .as_array()
        .expect("auto triggers")
        .is_empty());
}

#[test]
fn host_adapter_projection_library_preserves_non_authority_boundary() {
    let projection = run_host_adapter_projection(HostAdapterProjectionTarget::McpTools);
    assert_eq!(projection.target, HostAdapterProjectionTarget::McpTools);
    assert!(!projection.projection_authoritative);
    assert!(projection
        .authority_boundary
        .projections_must_not
        .iter()
        .any(|item| item.contains("auto-invoke mutating operations")));

    let execute = projection
        .commands
        .iter()
        .find(|command| command.name == "execute-operation")
        .expect("execute operation projection");
    assert_eq!(
        execute.mutation_class,
        HostAdapterMutationClass::MutatingOperation
    );
    assert_eq!(
        execute.authority_class,
        HostAdapterAuthorityClass::RequiresOperationAuthority
    );
    let execute_mcp = execute.mcp_tool.as_ref().expect("execute mcp projection");
    assert_eq!(
        execute.canonical_usage,
        command_surface::COMMAND_EXECUTE_OPERATION
            .canonical_usage()
            .trim_start()
    );
    assert!(!execute_mcp.annotations.read_only_hint);
    assert!(execute_mcp.annotations.destructive_hint);
    assert!(!execute_mcp.annotations.idempotent_hint);

    let query = projection
        .commands
        .iter()
        .find(|command| command.name == "query-effect-index")
        .expect("query projection");
    assert_eq!(
        query.canonical_usage,
        command_surface::COMMAND_QUERY_EFFECT_INDEX
            .canonical_usage()
            .trim_start()
    );
    let query_mcp = query.mcp_tool.as_ref().expect("query mcp projection");
    assert!(query_mcp.annotations.read_only_hint);
    assert!(!query_mcp.annotations.destructive_hint);
    assert_eq!(query_mcp.input_schema["additionalProperties"], false);
}

#[test]
fn host_adapter_projection_binary_outputs_mcp_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["host-adapter-projection", "--target", "mcp_tools", "--json"])
        .output()
        .expect("run forge-core host-adapter-projection");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["target"], "mcp_tools");
    assert_eq!(json["projection_authoritative"], false);
    let commands = json["commands"].as_array().expect("commands array");
    let execute = commands
        .iter()
        .find(|item| item["name"] == "execute-operation")
        .expect("execute projection");
    assert_eq!(
        execute["canonical_usage"],
        command_surface::COMMAND_EXECUTE_OPERATION
            .canonical_usage()
            .trim_start()
    );
    assert_eq!(execute["mcp_tool"]["annotations"]["destructiveHint"], true);
    assert_eq!(execute["mcp_tool"]["annotations"]["readOnlyHint"], false);
}

#[test]
fn host_adapter_process_policy_blocks_mcp_mutating_operations() {
    let policy = run_host_adapter_process_security_policy(HostAdapterProcessTarget::McpStdio);
    assert_eq!(policy.target, HostAdapterProcessTarget::McpStdio);
    assert_eq!(
        policy.default_admission,
        HostAdapterInvocationAdmissionStatus::Blocked
    );
    assert!(!policy.argv_policy.shell_strings_allowed);
    assert!(!policy.env_policy.inherit_full_environment);
    assert!(!policy.stdio_policy.raw_payload_bytes_allowed);

    let execute = policy
        .command_admissions
        .iter()
        .find(|item| item.command_name == "execute-operation")
        .expect("execute operation admission");
    assert!(!execute.mcp_stdio_enabled);
    assert!(execute.explicit_invocation_required);
    assert!(execute
        .required_controls
        .iter()
        .any(|item| item == "future_installer_trust_boundary_required"));
}

#[test]
fn host_adapter_invocation_admission_enforces_target_and_process_controls() {
    let mcp_execute = run_host_adapter_invocation_admission(HostAdapterInvocationRequest {
        command_name: "execute-operation".to_string(),
        target: HostAdapterProcessTarget::McpStdio,
        explicit_invocation: true,
        argv: vec!["--json".to_string()],
        cwd: Some(".".to_string()),
        env_keys: Vec::new(),
    });
    assert_eq!(
        mcp_execute.status,
        HostAdapterInvocationAdmissionStatus::Blocked
    );
    assert!(mcp_execute
        .reasons
        .contains(&"mcp_stdio_mutating_operation_deferred".to_string()));

    let borrowed_without_explicit =
        run_host_adapter_invocation_admission(HostAdapterInvocationRequest {
            command_name: "execute-operation".to_string(),
            target: HostAdapterProcessTarget::BorrowedShell,
            explicit_invocation: false,
            argv: vec!["--json".to_string()],
            cwd: Some(".".to_string()),
            env_keys: Vec::new(),
        });
    assert_eq!(
        borrowed_without_explicit.status,
        HostAdapterInvocationAdmissionStatus::Blocked
    );
    assert!(borrowed_without_explicit
        .reasons
        .contains(&"explicit_invocation_required".to_string()));

    let borrowed_explicit = run_host_adapter_invocation_admission(HostAdapterInvocationRequest {
        command_name: "execute-operation".to_string(),
        target: HostAdapterProcessTarget::BorrowedShell,
        explicit_invocation: true,
        argv: vec!["--json".to_string()],
        cwd: Some(".".to_string()),
        env_keys: Vec::new(),
    });
    assert_eq!(
        borrowed_explicit.status,
        HostAdapterInvocationAdmissionStatus::Allowed
    );

    let dangerous_readonly = run_host_adapter_invocation_admission(HostAdapterInvocationRequest {
        command_name: "validate".to_string(),
        target: HostAdapterProcessTarget::McpStdio,
        explicit_invocation: false,
        argv: vec!["--json; rm -rf .".to_string()],
        cwd: Some(".".to_string()),
        env_keys: vec!["OPENAI_API_KEY".to_string()],
    });
    assert_eq!(
        dangerous_readonly.status,
        HostAdapterInvocationAdmissionStatus::Blocked
    );
    assert!(dangerous_readonly
        .reasons
        .contains(&"shell_control_token_rejected".to_string()));
    assert!(dangerous_readonly
        .reasons
        .contains(&"forbidden_environment_key".to_string()));
}

#[test]
fn host_adapter_process_policy_binary_outputs_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-process-policy",
            "--target",
            "mcp_stdio",
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-process-policy");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["target"], "mcp_stdio");
    assert_eq!(json["default_admission"], "blocked");
    assert_eq!(json["argv_policy"]["shell_strings_allowed"], false);
    assert_eq!(json["env_policy"]["inherit_full_environment"], false);
}

#[test]
fn host_adapter_admit_invocation_binary_blocks_mcp_mutation() {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-admit-invocation",
            "--command",
            "execute-operation",
            "--target",
            "mcp_stdio",
            "--explicit",
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-admit-invocation");

    assert!(!output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "blocked");
    assert!(json["reasons"]
        .as_array()
        .expect("reasons array")
        .contains(&Value::String(
            "mcp_stdio_mutating_operation_deferred".to_string()
        )));
}

#[test]
fn host_adapter_distribution_policy_requires_supply_chain_evidence() {
    let policy = run_host_adapter_distribution_policy();
    assert_eq!(
        policy.default_admission,
        HostAdapterDistributionAdmissionStatus::Blocked
    );
    assert!(policy.required_evidence.immutable_source_ref);
    assert!(policy.required_evidence.artifact_checksum_or_signature);
    assert!(policy.required_evidence.provenance_ref);
    assert!(policy.required_evidence.version_compatibility);
    assert!(policy.required_evidence.rollback_ref);
    assert!(!policy.channel_policy.dev_allowed_for_general_install);
    assert!(!policy.updater_policy.self_update_may_bypass_admission);
}

#[test]
fn host_adapter_distribution_admission_blocks_weak_evidence_and_allows_complete_evidence() {
    let weak = run_host_adapter_distribution_admission(HostAdapterDistributionEvidence {
        target: RuntimeKind::Codex,
        channel: HostAdapterUpdateChannel::Stable,
        artifact_name: "forge-core.exe".to_string(),
        artifact_sha256: Some("not-a-sha".to_string()),
        signature_ref: None,
        provenance_ref: None,
        source_ref: Some("github:DanielCarva1/forge-method-core@main".to_string()),
        version: Some("1.0.0".to_string()),
        compatible_core_version: Some("1.0.0".to_string()),
        rollback_ref: None,
        update_summary_ref: None,
        explicit_canary_opt_in: false,
    });
    assert_eq!(weak.status, HostAdapterDistributionAdmissionStatus::Blocked);
    assert!(weak
        .reasons
        .contains(&"immutable_source_ref_required".to_string()));
    assert!(weak
        .reasons
        .contains(&"artifact_checksum_or_signature_required".to_string()));
    assert!(weak
        .reasons
        .contains(&"provenance_ref_required".to_string()));
    assert!(weak.reasons.contains(&"rollback_ref_required".to_string()));

    let complete = run_host_adapter_distribution_admission(HostAdapterDistributionEvidence {
        target: RuntimeKind::Codex,
        channel: HostAdapterUpdateChannel::Stable,
        artifact_name: "forge-core.exe".to_string(),
        artifact_sha256: Some(format!("sha256:{}", "a".repeat(64))),
        signature_ref: None,
        provenance_ref: Some("attestations/forge-core.intoto.jsonl".to_string()),
        source_ref: Some(
            "github:DanielCarva1/forge-method-core@0123456789abcdef0123456789abcdef01234567"
                .to_string(),
        ),
        version: Some("1.0.0".to_string()),
        compatible_core_version: Some("1.0.0".to_string()),
        rollback_ref: Some("releases/1.0.0/rollback.json".to_string()),
        update_summary_ref: Some("releases/1.0.0/summary.json".to_string()),
        explicit_canary_opt_in: false,
    });
    assert_eq!(
        complete.status,
        HostAdapterDistributionAdmissionStatus::Allowed
    );
    assert!(complete
        .accepted_evidence
        .contains(&"immutable_source_ref".to_string()));
}

#[test]
fn host_adapter_distribution_admission_requires_canary_opt_in_and_blocks_dev() {
    let base = HostAdapterDistributionEvidence {
        target: RuntimeKind::Cursor,
        channel: HostAdapterUpdateChannel::Canary,
        artifact_name: "forge-core".to_string(),
        artifact_sha256: Some("b".repeat(64)),
        signature_ref: None,
        provenance_ref: Some("attestations/canary.intoto.jsonl".to_string()),
        source_ref: Some(
            "git:forge-method-core#abcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string(),
        ),
        version: Some("1.1.0-canary.1".to_string()),
        compatible_core_version: Some("1.1.0".to_string()),
        rollback_ref: Some("rollback/canary.json".to_string()),
        update_summary_ref: Some("summary/canary.json".to_string()),
        explicit_canary_opt_in: false,
    };
    let canary_blocked = run_host_adapter_distribution_admission(base.clone());
    assert_eq!(
        canary_blocked.status,
        HostAdapterDistributionAdmissionStatus::Blocked
    );
    assert!(canary_blocked
        .reasons
        .contains(&"canary_requires_explicit_opt_in".to_string()));

    let mut canary_allowed_request = base;
    canary_allowed_request.explicit_canary_opt_in = true;
    let canary_allowed = run_host_adapter_distribution_admission(canary_allowed_request);
    assert_eq!(
        canary_allowed.status,
        HostAdapterDistributionAdmissionStatus::Allowed
    );

    let dev_blocked = run_host_adapter_distribution_admission(HostAdapterDistributionEvidence {
        channel: HostAdapterUpdateChannel::Dev,
        explicit_canary_opt_in: true,
        ..HostAdapterDistributionEvidence {
            target: RuntimeKind::Cursor,
            channel: HostAdapterUpdateChannel::Stable,
            artifact_name: "forge-core".to_string(),
            artifact_sha256: Some("c".repeat(64)),
            signature_ref: None,
            provenance_ref: Some("attestations/dev.intoto.jsonl".to_string()),
            source_ref: Some(
                "git:forge-method-core#1234512345123451234512345123451234512345".to_string(),
            ),
            version: Some("1.2.0-dev".to_string()),
            compatible_core_version: Some("1.2.0".to_string()),
            rollback_ref: Some("rollback/dev.json".to_string()),
            update_summary_ref: Some("summary/dev.json".to_string()),
            explicit_canary_opt_in: false,
        }
    });
    assert_eq!(
        dev_blocked.status,
        HostAdapterDistributionAdmissionStatus::Blocked
    );
    assert!(dev_blocked
        .reasons
        .contains(&"dev_channel_not_for_general_install".to_string()));
}

#[test]
fn host_adapter_distribution_policy_binary_outputs_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["host-adapter-distribution-policy", "--json"])
        .output()
        .expect("run forge-core host-adapter-distribution-policy");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["default_admission"], "blocked");
    assert_eq!(json["required_evidence"]["immutable_source_ref"], true);
}

#[test]
fn host_adapter_admit_distribution_binary_blocks_missing_evidence() {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-admit-distribution",
            "--artifact",
            "forge-core.exe",
            "--target",
            "codex",
            "--channel",
            "stable",
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-admit-distribution");

    assert!(!output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "blocked");
    assert!(json["reasons"]
        .as_array()
        .expect("reasons array")
        .contains(&Value::String("source_ref_required".to_string())));
}

#[test]
fn host_adapter_artifact_verification_passes_for_matching_digest_and_metadata() {
    let artifact_path = temp_payload_file("verify-artifact-library", b"forge artifact bytes");
    let expected_sha256 = sha256_content_hash(b"forge artifact bytes");
    let verification =
        run_host_adapter_artifact_verification(HostAdapterArtifactVerificationInput {
            artifact_path,
            expected_sha256,
            signature_ref: Some("sigstore/forge-core.sigstore.json".to_string()),
            provenance_ref: Some("attestations/forge-core.intoto.jsonl".to_string()),
            source_ref: Some(
                "github:DanielCarva1/forge-method-core@0123456789abcdef0123456789abcdef01234567"
                    .to_string(),
            ),
            version: Some("1.0.0".to_string()),
            compatible_core_version: Some("1.0.0".to_string()),
            rollback_ref: Some("releases/1.0.0/rollback.json".to_string()),
            update_summary_ref: Some("releases/1.0.0/summary.json".to_string()),
        });

    assert_eq!(
        verification.status,
        HostAdapterArtifactVerificationStatus::Passed
    );
    assert!(verification
        .verified_evidence
        .contains(&"sha256_match".to_string()));
    assert!(verification
        .deferred_verification
        .contains(&"signature_cryptographic_verification".to_string()));
}

#[test]
fn host_adapter_artifact_verification_fails_for_mismatch_or_missing_metadata() {
    let artifact_path = temp_payload_file("verify-artifact-mismatch", b"actual bytes");
    let verification =
        run_host_adapter_artifact_verification(HostAdapterArtifactVerificationInput {
            artifact_path,
            expected_sha256: format!("sha256:{}", "a".repeat(64)),
            signature_ref: None,
            provenance_ref: None,
            source_ref: Some("github:DanielCarva1/forge-method-core@main".to_string()),
            version: Some("1.0.0".to_string()),
            compatible_core_version: Some("1.0.0".to_string()),
            rollback_ref: None,
            update_summary_ref: None,
        });

    assert_eq!(
        verification.status,
        HostAdapterArtifactVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"sha256_mismatch".to_string()));
    assert!(verification
        .reasons
        .contains(&"provenance_ref_required".to_string()));
    assert!(verification
        .reasons
        .contains(&"immutable_source_ref_required".to_string()));
    assert!(verification
        .reasons
        .contains(&"rollback_ref_required".to_string()));
}

#[test]
fn host_adapter_verify_artifact_binary_outputs_json_and_exit_status() {
    let artifact_path = temp_payload_file("verify-artifact-binary", b"binary artifact bytes");
    let expected_sha256 = sha256_content_hash(b"binary artifact bytes");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-artifact",
            "--artifact-path",
            artifact_path.to_str().expect("utf8 temp path"),
            "--sha256",
            expected_sha256.as_str(),
            "--provenance-ref",
            "attestations/forge-core.intoto.jsonl",
            "--source-ref",
            "github:DanielCarva1/forge-method-core@0123456789abcdef0123456789abcdef01234567",
            "--version",
            "1.0.0",
            "--compatible-core-version",
            "1.0.0",
            "--rollback-ref",
            "releases/1.0.0/rollback.json",
            "--update-summary-ref",
            "releases/1.0.0/summary.json",
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-artifact");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["computed_sha256"], expected_sha256);
}

#[test]
fn host_adapter_verify_artifact_binary_blocks_mismatched_digest() {
    let artifact_path =
        temp_payload_file("verify-artifact-binary-blocked", b"binary artifact bytes");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-artifact",
            "--artifact-path",
            artifact_path.to_str().expect("utf8 temp path"),
            "--sha256",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--provenance-ref",
            "attestations/forge-core.intoto.jsonl",
            "--source-ref",
            "github:DanielCarva1/forge-method-core@0123456789abcdef0123456789abcdef01234567",
            "--version",
            "1.0.0",
            "--compatible-core-version",
            "1.0.0",
            "--rollback-ref",
            "releases/1.0.0/rollback.json",
            "--update-summary-ref",
            "releases/1.0.0/summary.json",
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-artifact");

    assert!(!output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "failed");
    assert!(json["reasons"]
        .as_array()
        .expect("reasons array")
        .contains(&Value::String("sha256_mismatch".to_string())));
}

#[test]
fn host_adapter_provenance_verification_passes_signed_slsa_statement() {
    let fixture = signed_provenance_fixture("verify-provenance-library");
    let verification =
        run_host_adapter_provenance_verification(HostAdapterProvenanceVerificationInput {
            artifact_path: fixture.artifact_path,
            provenance_path: fixture.provenance_path,
            signature_path: fixture.signature_path,
            public_key_path: fixture.public_key_path,
            transparency_log_path: fixture.transparency_log_path,
            expected_sha256: fixture.artifact_sha256,
            expected_builder_id: fixture.builder_id,
            expected_source_uri: fixture.source_uri,
            expected_source_ref: fixture.source_ref,
        });

    assert_eq!(
        verification.status,
        HostAdapterProvenanceVerificationStatus::Passed
    );
    assert!(verification
        .verified_evidence
        .contains(&"provenance_signature_valid".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"provenance_subject_matches_artifact".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"transparency_inclusion_proof_valid".to_string()));
}

#[test]
fn host_adapter_provenance_verification_fails_when_source_ref_mismatches() {
    let fixture = signed_provenance_fixture("verify-provenance-source-ref-fail");
    let verification =
        run_host_adapter_provenance_verification(HostAdapterProvenanceVerificationInput {
            artifact_path: fixture.artifact_path,
            provenance_path: fixture.provenance_path,
            signature_path: fixture.signature_path,
            public_key_path: fixture.public_key_path,
            transparency_log_path: fixture.transparency_log_path,
            expected_sha256: fixture.artifact_sha256,
            expected_builder_id: fixture.builder_id,
            expected_source_uri: fixture.source_uri,
            expected_source_ref: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        });

    assert_eq!(
        verification.status,
        HostAdapterProvenanceVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"source_ref_missing".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"provenance_signature_valid".to_string()));
}

#[test]
fn host_adapter_verify_provenance_binary_outputs_json_and_exit_status() {
    let fixture = signed_provenance_fixture("verify-provenance-binary");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-provenance",
            "--artifact-path",
            fixture.artifact_path.to_str().expect("utf8 artifact path"),
            "--provenance-path",
            fixture
                .provenance_path
                .to_str()
                .expect("utf8 provenance path"),
            "--signature-path",
            fixture
                .signature_path
                .to_str()
                .expect("utf8 signature path"),
            "--public-key-path",
            fixture.public_key_path.to_str().expect("utf8 key path"),
            "--transparency-log-path",
            fixture
                .transparency_log_path
                .to_str()
                .expect("utf8 transparency path"),
            "--sha256",
            fixture.artifact_sha256.as_str(),
            "--expected-builder-id",
            fixture.builder_id.as_str(),
            "--expected-source-uri",
            fixture.source_uri.as_str(),
            "--expected-source-ref",
            fixture.source_ref.as_str(),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-provenance");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert!(json["verified_evidence"]
        .as_array()
        .expect("evidence array")
        .contains(&Value::String(
            "transparency_inclusion_proof_valid".to_string()
        )));
}

#[test]
fn host_adapter_verify_provenance_binary_blocks_bad_signature() {
    let fixture = signed_provenance_fixture("verify-provenance-bad-signature");
    fs::write(&fixture.signature_path, BASE64.encode([9u8; 64])).expect("replace signature");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-provenance",
            "--artifact-path",
            fixture.artifact_path.to_str().expect("utf8 artifact path"),
            "--provenance-path",
            fixture
                .provenance_path
                .to_str()
                .expect("utf8 provenance path"),
            "--signature-path",
            fixture
                .signature_path
                .to_str()
                .expect("utf8 signature path"),
            "--public-key-path",
            fixture.public_key_path.to_str().expect("utf8 key path"),
            "--transparency-log-path",
            fixture
                .transparency_log_path
                .to_str()
                .expect("utf8 transparency path"),
            "--sha256",
            fixture.artifact_sha256.as_str(),
            "--expected-builder-id",
            fixture.builder_id.as_str(),
            "--expected-source-uri",
            fixture.source_uri.as_str(),
            "--expected-source-ref",
            fixture.source_ref.as_str(),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-provenance");

    assert!(!output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "failed");
    assert!(json["reasons"]
        .as_array()
        .expect("reasons array")
        .contains(&Value::String("provenance_signature_invalid".to_string())));
}

#[test]
fn host_adapter_rekor_verification_passes_signed_checkpoint_and_inclusion() {
    let fixture = rekor_entry_fixture("verify-rekor-library");
    let verification = run_host_adapter_rekor_verification(HostAdapterRekorVerificationInput {
        log_entry_path: fixture.log_entry_path,
        public_key_path: fixture.public_key_path,
        expected_log_id: fixture.expected_log_id,
    });

    assert_eq!(
        verification.status,
        HostAdapterRekorVerificationStatus::Passed
    );
    assert!(verification
        .verified_evidence
        .contains(&"rekor_log_entry_parsed".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"rekor_signed_checkpoint_valid".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"rekor_inclusion_proof_valid".to_string()));
}

#[test]
fn host_adapter_rekor_verification_fails_when_log_id_mismatches() {
    let fixture = rekor_entry_fixture("verify-rekor-log-id-fail");
    let verification = run_host_adapter_rekor_verification(HostAdapterRekorVerificationInput {
        log_entry_path: fixture.log_entry_path,
        public_key_path: fixture.public_key_path,
        expected_log_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_string(),
    });

    assert_eq!(
        verification.status,
        HostAdapterRekorVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"rekor_log_id_mismatch".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"rekor_inclusion_proof_valid".to_string()));
}

#[test]
fn host_adapter_verify_rekor_entry_binary_outputs_json_and_exit_status() {
    let fixture = rekor_entry_fixture("verify-rekor-binary");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-rekor-entry",
            "--log-entry-path",
            fixture
                .log_entry_path
                .to_str()
                .expect("utf8 log entry path"),
            "--public-key-path",
            fixture
                .public_key_path
                .to_str()
                .expect("utf8 public key path"),
            "--expected-log-id",
            fixture.expected_log_id.as_str(),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-rekor-entry");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert!(json["verified_evidence"]
        .as_array()
        .expect("evidence array")
        .contains(&Value::String("rekor_inclusion_proof_valid".to_string())));
}

#[test]
fn host_adapter_verify_rekor_entry_binary_blocks_wrong_key() {
    let fixture = rekor_entry_fixture("verify-rekor-wrong-key");
    let wrong_signing_key = P256SigningKey::from_slice(&[9u8; 32]).expect("p256 wrong key");
    let wrong_public_key_pem = wrong_signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .expect("wrong public key pem");
    fs::write(&fixture.public_key_path, wrong_public_key_pem.as_bytes())
        .expect("replace rekor public key");

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-rekor-entry",
            "--log-entry-path",
            fixture
                .log_entry_path
                .to_str()
                .expect("utf8 log entry path"),
            "--public-key-path",
            fixture
                .public_key_path
                .to_str()
                .expect("utf8 public key path"),
            "--expected-log-id",
            fixture.expected_log_id.as_str(),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-rekor-entry");

    assert!(!output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "failed");
    assert!(json["reasons"]
        .as_array()
        .expect("reasons array")
        .iter()
        .any(|reason| reason
            .as_str()
            .is_some_and(|value| value.starts_with("rekor_inclusion_verification_failed:"))));
}

#[test]
fn host_adapter_sigstore_trust_policy_verification_passes_complete_policy() {
    let fixture =
        sigstore_trust_policy_fixture("verify-sigstore-trust-policy", &["fulcio-root.pem"]);
    let verification = forge_core_cli::run_host_adapter_sigstore_trust_policy_verification(
        HostAdapterSigstoreTrustPolicyVerificationInput {
            policy_path: fixture.policy_path,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreTrustPolicyVerificationStatus::Passed
    );
    assert!(verification
        .verified_evidence
        .contains(&"sigstore_fulcio_ca_refs_present".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"sigstore_identity_selector_present".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"sigstore_timestamp_policy_has_source".to_string()));
}

#[test]
fn host_adapter_sigstore_trust_policy_verification_fails_missing_fulcio_refs() {
    let fixture = sigstore_trust_policy_fixture("verify-sigstore-trust-policy-fail", &[]);
    let verification = forge_core_cli::run_host_adapter_sigstore_trust_policy_verification(
        HostAdapterSigstoreTrustPolicyVerificationInput {
            policy_path: fixture.policy_path,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreTrustPolicyVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"sigstore_fulcio_ca_refs_missing".to_string()));
}

#[test]
fn host_adapter_verify_sigstore_trust_policy_binary_outputs_json_and_exit_status() {
    let fixture =
        sigstore_trust_policy_fixture("verify-sigstore-trust-policy-binary", &["fulcio-root.pem"]);
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-sigstore-trust-policy",
            "--policy-path",
            fixture.policy_path.to_str().expect("utf8 policy path"),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-sigstore-trust-policy");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["root_source"], "tuf");
    assert!(json["verified_evidence"]
        .as_array()
        .expect("evidence array")
        .contains(&Value::String(
            "sigstore_identity_github_sha_immutable".to_string()
        )));
}

#[test]
fn host_adapter_verify_sigstore_trust_policy_binary_blocks_missing_tsa_source() {
    let fixture =
        sigstore_trust_policy_fixture("verify-sigstore-trust-policy-binary-fail", &["fulcio.pem"]);
    let text = fs::read_to_string(&fixture.policy_path).expect("read policy");
    let text = text
        .replace("mode: \"either\"", "mode: \"rfc3161_tsa\"")
        .replace(
            "    certificate_refs:\n      - \"tsa-root.pem\"",
            "    certificate_refs: []",
        );
    fs::write(&fixture.policy_path, text).expect("write policy without tsa");

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-sigstore-trust-policy",
            "--policy-path",
            fixture.policy_path.to_str().expect("utf8 policy path"),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-sigstore-trust-policy");

    assert!(!output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "failed");
    assert!(json["reasons"]
        .as_array()
        .expect("reasons array")
        .contains(&Value::String(
            "sigstore_timestamp_policy_requires_tsa_certs".to_string()
        )));
}

#[test]
fn host_adapter_fulcio_certificate_identity_verification_passes_matching_policy() {
    let fixture =
        fulcio_certificate_fixture("verify-fulcio-certificate-identity", &["fulcio-root.pem"]);
    let verification = run_host_adapter_fulcio_certificate_identity_verification(
        HostAdapterFulcioCertificateIdentityVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            verification_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterFulcioCertificateIdentityVerificationStatus::Passed
    );
    assert!(verification
        .verified_evidence
        .contains(&"fulcio_identity_oidc_issuer_match".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"fulcio_identity_san_match".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"fulcio_chain_signature_verified_0".to_string()));
    assert_eq!(
        verification.observed_oidc_issuer.as_deref(),
        Some("https://token.actions.githubusercontent.com")
    );
}

#[test]
fn host_adapter_fulcio_certificate_identity_verification_fails_oidc_issuer_mismatch() {
    let fixture = fulcio_certificate_fixture(
        "verify-fulcio-certificate-identity-issuer-fail",
        &["fulcio-root.pem"],
    );
    let text = fs::read_to_string(&fixture.policy_path).expect("read policy");
    fs::write(
        &fixture.policy_path,
        text.replace(
            "expected_oidc_issuer: \"https://token.actions.githubusercontent.com\"",
            "expected_oidc_issuer: \"https://issuer.example.invalid\"",
        ),
    )
    .expect("write mismatched issuer policy");
    let verification = run_host_adapter_fulcio_certificate_identity_verification(
        HostAdapterFulcioCertificateIdentityVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            verification_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterFulcioCertificateIdentityVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"fulcio_identity_oidc_issuer_mismatch".to_string()));
}

#[test]
fn host_adapter_fulcio_certificate_identity_verification_fails_undeclared_root() {
    let fixture = fulcio_certificate_fixture(
        "verify-fulcio-certificate-identity-root-fail",
        &["other-fulcio-root.pem"],
    );
    let verification = run_host_adapter_fulcio_certificate_identity_verification(
        HostAdapterFulcioCertificateIdentityVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            verification_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterFulcioCertificateIdentityVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"fulcio_chain_declared_ca_ref_missing".to_string()));
}

#[test]
fn host_adapter_verify_fulcio_certificate_identity_binary_outputs_json_and_exit_status() {
    let fixture = fulcio_certificate_fixture(
        "verify-fulcio-certificate-identity-binary",
        &["fulcio-root.pem"],
    );
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-fulcio-certificate-identity",
            "--trust-policy-path",
            fixture.policy_path.to_str().expect("utf8 policy path"),
            "--certificate-path",
            fixture
                .certificate_path
                .to_str()
                .expect("utf8 certificate path"),
            "--issuer-certificate-path",
            fixture
                .issuer_certificate_path
                .to_str()
                .expect("utf8 issuer certificate path"),
            "--verification-time-unix",
            &fixture.verification_time_unix.to_string(),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-fulcio-certificate-identity");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(
        json["observed_oidc_issuer"],
        "https://token.actions.githubusercontent.com"
    );
    assert!(json["verified_evidence"]
        .as_array()
        .expect("evidence array")
        .contains(&Value::String(
            "fulcio_identity_github_repository_match".to_string()
        )));
}

#[test]
fn host_adapter_sigstore_bundle_subject_verification_passes_matching_bundle() {
    let fixture = sigstore_bundle_subject_fixture("verify-sigstore-bundle-subject");
    let verification = run_host_adapter_sigstore_bundle_subject_verification(
        HostAdapterSigstoreBundleSubjectVerificationInput {
            bundle_path: fixture.bundle_path,
            artifact_path: fixture.artifact_path,
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            rekor_log_entry_path: fixture.rekor_log_entry_path,
            rekor_public_key_path: fixture.rekor_public_key_path,
            expected_rekor_log_id: fixture.expected_rekor_log_id,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreBundleSubjectVerificationStatus::Passed
    );
    assert!(verification
        .verified_evidence
        .contains(&"bundle_message_digest_matches_artifact".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"bundle_signature_verified_with_certificate_key".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"bundle_fulcio_identity_verified_at_rekor_time".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"rekor_body_binds_bundle_signature".to_string()));
}

#[test]
fn host_adapter_sigstore_bundle_subject_verification_fails_digest_mismatch() {
    let fixture = sigstore_bundle_subject_fixture("verify-sigstore-bundle-subject-digest-fail");
    fs::write(&fixture.artifact_path, b"modified artifact").expect("mutate artifact");
    let verification = run_host_adapter_sigstore_bundle_subject_verification(
        HostAdapterSigstoreBundleSubjectVerificationInput {
            bundle_path: fixture.bundle_path,
            artifact_path: fixture.artifact_path,
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            rekor_log_entry_path: fixture.rekor_log_entry_path,
            rekor_public_key_path: fixture.rekor_public_key_path,
            expected_rekor_log_id: fixture.expected_rekor_log_id,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreBundleSubjectVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"bundle_message_digest_mismatch".to_string()));
}

#[test]
fn host_adapter_sigstore_bundle_subject_verification_fails_rekor_body_mismatch() {
    let fixture = sigstore_bundle_subject_fixture("verify-sigstore-bundle-subject-rekor-fail");
    let text = fs::read_to_string(&fixture.rekor_log_entry_path).expect("read rekor entry");
    let mut log_entry: Value = serde_json::from_str(&text).expect("parse rekor entry");
    let body_b64 = log_entry["body"].as_str().expect("body string");
    let body_bytes = BASE64.decode(body_b64.as_bytes()).expect("decode body");
    let mut body: Value = serde_json::from_slice(&body_bytes).expect("parse body");
    body["spec"]["data"]["hash"]["value"] = Value::String(
        "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
    );
    log_entry["body"] =
        Value::String(BASE64.encode(serde_json::to_vec(&body).expect("serialize mismatched body")));
    fs::write(
        &fixture.rekor_log_entry_path,
        serde_json::to_vec_pretty(&log_entry).expect("serialize mismatched rekor entry"),
    )
    .expect("write mismatched rekor body");
    let verification = run_host_adapter_sigstore_bundle_subject_verification(
        HostAdapterSigstoreBundleSubjectVerificationInput {
            bundle_path: fixture.bundle_path,
            artifact_path: fixture.artifact_path,
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            rekor_log_entry_path: fixture.rekor_log_entry_path,
            rekor_public_key_path: fixture.rekor_public_key_path,
            expected_rekor_log_id: fixture.expected_rekor_log_id,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreBundleSubjectVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"rekor_body_artifact_digest_mismatch".to_string()));
}

#[test]
fn host_adapter_verify_sigstore_bundle_subject_binary_outputs_json_and_exit_status() {
    let fixture = sigstore_bundle_subject_fixture("verify-sigstore-bundle-subject-binary");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-sigstore-bundle-subject",
            "--bundle-path",
            fixture.bundle_path.to_str().expect("utf8 bundle path"),
            "--artifact-path",
            fixture.artifact_path.to_str().expect("utf8 artifact path"),
            "--trust-policy-path",
            fixture.policy_path.to_str().expect("utf8 policy path"),
            "--certificate-path",
            fixture
                .certificate_path
                .to_str()
                .expect("utf8 certificate path"),
            "--issuer-certificate-path",
            fixture
                .issuer_certificate_path
                .to_str()
                .expect("utf8 issuer certificate path"),
            "--rekor-log-entry-path",
            fixture
                .rekor_log_entry_path
                .to_str()
                .expect("utf8 rekor path"),
            "--rekor-public-key-path",
            fixture
                .rekor_public_key_path
                .to_str()
                .expect("utf8 rekor public key path"),
            "--expected-rekor-log-id",
            &fixture.expected_rekor_log_id,
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-sigstore-bundle-subject");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert!(json["verified_evidence"]
        .as_array()
        .expect("evidence array")
        .contains(&Value::String("bundle_rekor_entry_verified".to_string())));
}

#[test]
fn host_adapter_sigstore_dsse_in_toto_subject_verification_passes_matching_bundle() {
    let fixture = sigstore_dsse_in_toto_subject_fixture("verify-sigstore-dsse-in-toto-subject");
    let verification = run_host_adapter_sigstore_dsse_in_toto_subject_verification(
        HostAdapterSigstoreDsseInTotoSubjectVerificationInput {
            bundle_path: fixture.bundle_path,
            artifact_path: fixture.artifact_path,
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            rekor_log_entry_path: fixture.rekor_log_entry_path,
            rekor_public_key_path: fixture.rekor_public_key_path,
            expected_rekor_log_id: fixture.expected_rekor_log_id,
            expected_predicate_type: Some(fixture.expected_predicate_type),
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreDsseInTotoSubjectVerificationStatus::Passed
    );
    assert!(verification
        .verified_evidence
        .contains(&"dsse_signature_verified_with_certificate_key".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"dsse_intoto_subject_matches_artifact".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"rekor_body_binds_dsse_payload_hash".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"dsse_fulcio_identity_verified_at_rekor_time".to_string()));
}

#[test]
fn host_adapter_sigstore_dsse_in_toto_subject_verification_fails_payload_type_mismatch() {
    let fixture = sigstore_dsse_in_toto_subject_fixture("verify-sigstore-dsse-payload-type-fail");
    let text = fs::read_to_string(&fixture.bundle_path).expect("read bundle");
    let mut bundle: Value = serde_json::from_str(&text).expect("parse bundle");
    bundle["dsseEnvelope"]["payloadType"] = Value::String("application/json".to_string());
    fs::write(
        &fixture.bundle_path,
        serde_json::to_vec_pretty(&bundle).expect("serialize bundle"),
    )
    .expect("write bundle");

    let verification = run_host_adapter_sigstore_dsse_in_toto_subject_verification(
        HostAdapterSigstoreDsseInTotoSubjectVerificationInput {
            bundle_path: fixture.bundle_path,
            artifact_path: fixture.artifact_path,
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            rekor_log_entry_path: fixture.rekor_log_entry_path,
            rekor_public_key_path: fixture.rekor_public_key_path,
            expected_rekor_log_id: fixture.expected_rekor_log_id,
            expected_predicate_type: Some(fixture.expected_predicate_type),
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreDsseInTotoSubjectVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"dsse_payload_type_unsupported".to_string()));
    assert!(verification
        .reasons
        .contains(&"dsse_signature_verification_failed".to_string()));
}

#[test]
fn host_adapter_sigstore_dsse_in_toto_subject_verification_fails_subject_mismatch() {
    let fixture = sigstore_dsse_in_toto_subject_fixture("verify-sigstore-dsse-subject-fail");
    fs::write(&fixture.artifact_path, b"modified dsse artifact").expect("mutate artifact");
    let verification = run_host_adapter_sigstore_dsse_in_toto_subject_verification(
        HostAdapterSigstoreDsseInTotoSubjectVerificationInput {
            bundle_path: fixture.bundle_path,
            artifact_path: fixture.artifact_path,
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            rekor_log_entry_path: fixture.rekor_log_entry_path,
            rekor_public_key_path: fixture.rekor_public_key_path,
            expected_rekor_log_id: fixture.expected_rekor_log_id,
            expected_predicate_type: Some(fixture.expected_predicate_type),
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreDsseInTotoSubjectVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"dsse_intoto_subject_sha256_missing".to_string()));
}

#[test]
fn host_adapter_sigstore_dsse_in_toto_subject_verification_fails_multiple_signatures() {
    let fixture =
        sigstore_dsse_in_toto_subject_fixture("verify-sigstore-dsse-signature-count-fail");
    let text = fs::read_to_string(&fixture.bundle_path).expect("read bundle");
    let mut bundle: Value = serde_json::from_str(&text).expect("parse bundle");
    let signature = bundle["dsseEnvelope"]["signatures"][0].clone();
    bundle["dsseEnvelope"]["signatures"]
        .as_array_mut()
        .expect("signatures array")
        .push(signature);
    fs::write(
        &fixture.bundle_path,
        serde_json::to_vec_pretty(&bundle).expect("serialize bundle"),
    )
    .expect("write bundle");

    let verification = run_host_adapter_sigstore_dsse_in_toto_subject_verification(
        HostAdapterSigstoreDsseInTotoSubjectVerificationInput {
            bundle_path: fixture.bundle_path,
            artifact_path: fixture.artifact_path,
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            rekor_log_entry_path: fixture.rekor_log_entry_path,
            rekor_public_key_path: fixture.rekor_public_key_path,
            expected_rekor_log_id: fixture.expected_rekor_log_id,
            expected_predicate_type: Some(fixture.expected_predicate_type),
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreDsseInTotoSubjectVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"dsse_signature_count_invalid".to_string()));
}

#[test]
fn host_adapter_sigstore_dsse_in_toto_subject_verification_fails_rekor_body_mismatch() {
    let fixture = sigstore_dsse_in_toto_subject_fixture("verify-sigstore-dsse-rekor-fail");
    let text = fs::read_to_string(&fixture.rekor_log_entry_path).expect("read rekor entry");
    let mut log_entry: Value = serde_json::from_str(&text).expect("parse rekor entry");
    let body_b64 = log_entry["body"].as_str().expect("body string");
    let body_bytes = BASE64.decode(body_b64.as_bytes()).expect("decode body");
    let mut body: Value = serde_json::from_slice(&body_bytes).expect("parse body");
    body["spec"]["dsseObj"]["payloadHash"]["value"] = Value::String(
        "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
    );
    log_entry["body"] =
        Value::String(BASE64.encode(serde_json::to_vec(&body).expect("serialize body")));
    fs::write(
        &fixture.rekor_log_entry_path,
        serde_json::to_vec_pretty(&log_entry).expect("serialize rekor entry"),
    )
    .expect("write rekor entry");

    let verification = run_host_adapter_sigstore_dsse_in_toto_subject_verification(
        HostAdapterSigstoreDsseInTotoSubjectVerificationInput {
            bundle_path: fixture.bundle_path,
            artifact_path: fixture.artifact_path,
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_paths: vec![fixture.issuer_certificate_path],
            rekor_log_entry_path: fixture.rekor_log_entry_path,
            rekor_public_key_path: fixture.rekor_public_key_path,
            expected_rekor_log_id: fixture.expected_rekor_log_id,
            expected_predicate_type: Some(fixture.expected_predicate_type),
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreDsseInTotoSubjectVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"rekor_body_dsse_payload_hash_mismatch".to_string()));
}

#[test]
fn host_adapter_verify_sigstore_dsse_in_toto_subject_binary_outputs_json_and_exit_status() {
    let fixture =
        sigstore_dsse_in_toto_subject_fixture("verify-sigstore-dsse-in-toto-subject-binary");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-sigstore-dsse-in-toto-subject",
            "--bundle-path",
            fixture.bundle_path.to_str().expect("utf8 bundle path"),
            "--artifact-path",
            fixture.artifact_path.to_str().expect("utf8 artifact path"),
            "--trust-policy-path",
            fixture.policy_path.to_str().expect("utf8 policy path"),
            "--certificate-path",
            fixture
                .certificate_path
                .to_str()
                .expect("utf8 certificate path"),
            "--issuer-certificate-path",
            fixture
                .issuer_certificate_path
                .to_str()
                .expect("utf8 issuer certificate path"),
            "--rekor-log-entry-path",
            fixture
                .rekor_log_entry_path
                .to_str()
                .expect("utf8 rekor path"),
            "--rekor-public-key-path",
            fixture
                .rekor_public_key_path
                .to_str()
                .expect("utf8 rekor public key path"),
            "--expected-rekor-log-id",
            &fixture.expected_rekor_log_id,
            "--expected-predicate-type",
            &fixture.expected_predicate_type,
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-sigstore-dsse-in-toto-subject");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["payload_type"], "application/vnd.in-toto+json");
    assert!(json["verified_evidence"]
        .as_array()
        .expect("evidence array")
        .contains(&Value::String("dsse_rekor_entry_verified".to_string())));
}

#[test]
fn host_adapter_sigstore_timestamp_authority_verification_passes_rekor_integrated_time() {
    let fulcio =
        fulcio_certificate_fixture("verify-sigstore-timestamp-authority", &["fulcio-root.pem"]);
    let rekor = rekor_entry_fixture("verify-sigstore-timestamp-authority-rekor");
    let verification = run_host_adapter_sigstore_timestamp_authority_verification(
        HostAdapterSigstoreTimestampAuthorityVerificationInput {
            trust_policy_path: fulcio.policy_path,
            certificate_path: fulcio.certificate_path,
            rekor_log_entry_path: Some(rekor.log_entry_path),
            rekor_public_key_path: Some(rekor.public_key_path),
            expected_rekor_log_id: Some(rekor.expected_log_id),
            rfc3161_timestamp_token_path: None,
            rfc3161_timestamped_signature_path: None,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreTimestampAuthorityVerificationStatus::Passed
    );
    assert_eq!(
        verification.selected_timestamp_source.as_deref(),
        Some("rekor_integrated_time")
    );
    assert_eq!(
        verification.rekor_status,
        Some(HostAdapterRekorVerificationStatus::Passed)
    );
    assert!(verification
        .verified_evidence
        .contains(&"timestamp_rekor_integrated_time_verified".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"timestamp_within_certificate_validity".to_string()));
}

#[test]
fn host_adapter_sigstore_timestamp_authority_verification_fails_outside_certificate_window() {
    let fulcio = fulcio_certificate_fixture(
        "verify-sigstore-timestamp-authority-window-fail",
        &["fulcio-root.pem"],
    );
    let rekor = rekor_entry_fixture("verify-sigstore-timestamp-authority-window-fail-rekor");
    let text = fs::read_to_string(&rekor.log_entry_path).expect("read rekor entry");
    let mut entry: Value = serde_json::from_str(&text).expect("parse rekor entry");
    entry["integratedTime"] = Value::Number(1_i64.into());
    fs::write(
        &rekor.log_entry_path,
        serde_json::to_vec_pretty(&entry).expect("serialize rekor entry"),
    )
    .expect("write rekor entry");

    let verification = run_host_adapter_sigstore_timestamp_authority_verification(
        HostAdapterSigstoreTimestampAuthorityVerificationInput {
            trust_policy_path: fulcio.policy_path,
            certificate_path: fulcio.certificate_path,
            rekor_log_entry_path: Some(rekor.log_entry_path),
            rekor_public_key_path: Some(rekor.public_key_path),
            expected_rekor_log_id: Some(rekor.expected_log_id),
            rfc3161_timestamp_token_path: None,
            rfc3161_timestamped_signature_path: None,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreTimestampAuthorityVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"timestamp_outside_certificate_validity".to_string()));
    assert_eq!(
        verification.rekor_status,
        Some(HostAdapterRekorVerificationStatus::Passed)
    );
}

#[test]
fn host_adapter_sigstore_timestamp_authority_verification_passes_rfc3161_tsa_token() {
    let fulcio = fulcio_certificate_fixture_with_validity(
        "verify-sigstore-timestamp-authority-rfc3161",
        &["fulcio-root.pem"],
        (2020, 1, 1),
        (2035, 1, 1),
    );
    let rfc3161 = install_rfc3161_timestamp_fixture(
        &fulcio.policy_path,
        RFC3161_VALID_BUNDLE,
        RFC3161_VALID_TRUSTED_ROOT,
    );

    let verification = run_host_adapter_sigstore_timestamp_authority_verification(
        HostAdapterSigstoreTimestampAuthorityVerificationInput {
            trust_policy_path: fulcio.policy_path,
            certificate_path: fulcio.certificate_path,
            rekor_log_entry_path: None,
            rekor_public_key_path: None,
            expected_rekor_log_id: None,
            rfc3161_timestamp_token_path: Some(rfc3161.token_path),
            rfc3161_timestamped_signature_path: Some(rfc3161.signature_path),
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreTimestampAuthorityVerificationStatus::Passed
    );
    assert_eq!(verification.policy_mode.as_deref(), Some("rfc3161_tsa"));
    assert!(verification
        .verified_evidence
        .contains(&"timestamp_rfc3161_token_verified".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"timestamp_rfc3161_message_imprint_verified".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"timestamp_rfc3161_cms_signature_verified".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"timestamp_rfc3161_tsa_chain_verified".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"timestamp_within_certificate_validity".to_string()));
}

#[test]
fn host_adapter_sigstore_timestamp_authority_verification_fails_rfc3161_payload_mismatch() {
    let fulcio = fulcio_certificate_fixture_with_validity(
        "verify-sigstore-timestamp-authority-rfc3161-mismatch",
        &["fulcio-root.pem"],
        (2020, 1, 1),
        (2035, 1, 1),
    );
    let rfc3161 = install_rfc3161_timestamp_fixture(
        &fulcio.policy_path,
        RFC3161_PAYLOAD_MISMATCH_BUNDLE,
        RFC3161_VALID_TRUSTED_ROOT,
    );

    let verification = run_host_adapter_sigstore_timestamp_authority_verification(
        HostAdapterSigstoreTimestampAuthorityVerificationInput {
            trust_policy_path: fulcio.policy_path,
            certificate_path: fulcio.certificate_path,
            rekor_log_entry_path: None,
            rekor_public_key_path: None,
            expected_rekor_log_id: None,
            rfc3161_timestamp_token_path: Some(rfc3161.token_path),
            rfc3161_timestamped_signature_path: Some(rfc3161.signature_path),
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreTimestampAuthorityVerificationStatus::Failed
    );
    assert!(verification.reasons.iter().any(|reason| reason
        .starts_with("timestamp_rfc3161_verification_failed:Timestamp message hash mismatch")));
}

#[test]
fn host_adapter_sigstore_timestamp_authority_verification_fails_missing_rfc3161_tsa_refs() {
    let fulcio = fulcio_certificate_fixture_with_validity(
        "verify-sigstore-timestamp-authority-rfc3161-no-tsa",
        &["fulcio-root.pem"],
        (2020, 1, 1),
        (2035, 1, 1),
    );
    let rfc3161 = install_rfc3161_timestamp_fixture(
        &fulcio.policy_path,
        RFC3161_VALID_BUNDLE,
        RFC3161_VALID_TRUSTED_ROOT,
    );
    let policy = fs::read_to_string(&fulcio.policy_path).expect("read policy");
    let start = policy
        .find("    certificate_refs:")
        .expect("certificate refs");
    let end = policy[start..]
        .find("  identity_policy:")
        .map(|index| start + index)
        .expect("identity policy");
    let mut stripped_policy = String::new();
    stripped_policy.push_str(&policy[..start]);
    stripped_policy.push_str("    certificate_refs: []\n");
    stripped_policy.push_str(&policy[end..]);
    fs::write(&fulcio.policy_path, stripped_policy).expect("write policy without TSA refs");

    let verification = run_host_adapter_sigstore_timestamp_authority_verification(
        HostAdapterSigstoreTimestampAuthorityVerificationInput {
            trust_policy_path: fulcio.policy_path,
            certificate_path: fulcio.certificate_path,
            rekor_log_entry_path: None,
            rekor_public_key_path: None,
            expected_rekor_log_id: None,
            rfc3161_timestamp_token_path: Some(rfc3161.token_path),
            rfc3161_timestamped_signature_path: Some(rfc3161.signature_path),
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterSigstoreTimestampAuthorityVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"timestamp_rfc3161_tsa_certificate_refs_missing".to_string()));
}

#[test]
fn host_adapter_verify_sigstore_timestamp_authority_binary_outputs_json_and_exit_status() {
    let fulcio = fulcio_certificate_fixture(
        "verify-sigstore-timestamp-authority-binary",
        &["fulcio-root.pem"],
    );
    let rekor = rekor_entry_fixture("verify-sigstore-timestamp-authority-binary-rekor");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-sigstore-timestamp-authority",
            "--trust-policy-path",
            fulcio.policy_path.to_str().expect("utf8 policy path"),
            "--certificate-path",
            fulcio
                .certificate_path
                .to_str()
                .expect("utf8 certificate path"),
            "--rekor-log-entry-path",
            rekor.log_entry_path.to_str().expect("utf8 rekor path"),
            "--rekor-public-key-path",
            rekor
                .public_key_path
                .to_str()
                .expect("utf8 rekor public key path"),
            "--expected-rekor-log-id",
            &rekor.expected_log_id,
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-sigstore-timestamp-authority");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["selected_timestamp_source"], "rekor_integrated_time");
    assert!(json["verified_evidence"]
        .as_array()
        .expect("evidence array")
        .contains(&Value::String(
            "timestamp_within_certificate_validity".to_string()
        )));
}

#[test]
fn host_adapter_verify_sigstore_timestamp_authority_binary_outputs_json_for_rfc3161() {
    let fulcio = fulcio_certificate_fixture_with_validity(
        "verify-sigstore-timestamp-authority-rfc3161-binary",
        &["fulcio-root.pem"],
        (2020, 1, 1),
        (2035, 1, 1),
    );
    let rfc3161 = install_rfc3161_timestamp_fixture(
        &fulcio.policy_path,
        RFC3161_VALID_BUNDLE,
        RFC3161_VALID_TRUSTED_ROOT,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-sigstore-timestamp-authority",
            "--trust-policy-path",
            fulcio.policy_path.to_str().expect("utf8 policy path"),
            "--certificate-path",
            fulcio
                .certificate_path
                .to_str()
                .expect("utf8 certificate path"),
            "--rfc3161-timestamp-token-path",
            rfc3161.token_path.to_str().expect("utf8 token path"),
            "--rfc3161-timestamped-signature-path",
            rfc3161
                .signature_path
                .to_str()
                .expect("utf8 signature path"),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-sigstore-timestamp-authority");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["selected_timestamp_source"], "rfc3161_tsa");
    assert!(json["verified_evidence"]
        .as_array()
        .expect("evidence array")
        .contains(&Value::String(
            "timestamp_rfc3161_tsa_chain_verified".to_string()
        )));
}

#[test]
fn host_adapter_certificate_transparency_sct_verification_passes_supplied_scts() {
    let fixture = certificate_transparency_sct_fixture("verify-ct-sct");
    let verification = run_host_adapter_certificate_transparency_sct_verification(
        HostAdapterCertificateTransparencySctVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            sct_paths: vec![fixture.sct0_path, fixture.sct1_path],
            verification_time_unix_ms: CT_GOOGLE_SCT_VERIFICATION_TIME_MS,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateTransparencySctVerificationStatus::Passed
    );
    assert_eq!(verification.verified_sct_count, 2);
    assert!(verification
        .verified_log_ids
        .contains(&hex_bytes(&CT_GOOGLE_PILOT_LOG_ID)));
    assert!(verification
        .verified_log_ids
        .contains(&hex_bytes(&CT_SYMANTEC_LOG_ID)));
    assert!(verification
        .verified_evidence
        .iter()
        .any(|item| item.starts_with("ct_sct_signature_verified:")));
}

#[test]
fn host_adapter_certificate_transparency_sct_verification_fails_unknown_log() {
    let fixture = certificate_transparency_sct_fixture("verify-ct-sct-unknown-log");
    let policy_text = fs::read_to_string(&fixture.policy_path).expect("read CT policy");
    let policy_text = policy_text.replace(&hex_bytes(&CT_GOOGLE_PILOT_LOG_ID), &"00".repeat(32));
    fs::write(&fixture.policy_path, policy_text).expect("write mismatched CT policy");

    let verification = run_host_adapter_certificate_transparency_sct_verification(
        HostAdapterCertificateTransparencySctVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            sct_paths: vec![fixture.sct0_path],
            verification_time_unix_ms: CT_GOOGLE_SCT_VERIFICATION_TIME_MS,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateTransparencySctVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .iter()
        .any(|reason| reason.contains("UnknownLog")));
}

#[test]
fn host_adapter_certificate_transparency_sct_verification_fails_future_sct() {
    let fixture = certificate_transparency_sct_fixture("verify-ct-sct-future");
    let verification = run_host_adapter_certificate_transparency_sct_verification(
        HostAdapterCertificateTransparencySctVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            sct_paths: vec![fixture.sct0_path],
            verification_time_unix_ms: 1,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateTransparencySctVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .iter()
        .any(|reason| reason.contains("TimestampInFuture")));
}

#[test]
fn host_adapter_verify_certificate_transparency_sct_binary_outputs_json() {
    let fixture = certificate_transparency_sct_fixture("verify-ct-sct-binary");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-certificate-transparency-sct",
            "--trust-policy-path",
            fixture.policy_path.to_str().expect("utf8 policy path"),
            "--certificate-path",
            fixture
                .certificate_path
                .to_str()
                .expect("utf8 certificate path"),
            "--sct-path",
            fixture.sct0_path.to_str().expect("utf8 SCT0 path"),
            "--sct-path",
            fixture.sct1_path.to_str().expect("utf8 SCT1 path"),
            "--verification-time-unix-ms",
            &CT_GOOGLE_SCT_VERIFICATION_TIME_MS.to_string(),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-certificate-transparency-sct");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["verified_sct_count"], 2);
    assert!(json["deferred_verification"]
        .as_array()
        .expect("deferred array")
        .contains(&Value::String("revocation_status".to_string())));
}

#[test]
fn host_adapter_certificate_revocation_policy_verification_passes_short_lived_policy() {
    let fixture = fulcio_certificate_fixture("verify-revocation-short-lived", &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(
        &fixture.policy_path,
        "short_lived_certificate",
        Some(40_000_000),
    );

    let verification = run_host_adapter_certificate_revocation_policy_verification(
        HostAdapterCertificateRevocationPolicyVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            trusted_signing_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateRevocationPolicyVerificationStatus::Passed
    );
    assert_eq!(
        verification.revocation_strategy.as_deref(),
        Some("implicit_short_lived_certificate")
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("not_checked_by_short_lived_policy")
    );
    assert!(verification.verified_evidence.contains(
        &"certificate_revocation_trusted_signing_time_within_certificate_validity".to_string()
    ));
}

#[test]
fn host_adapter_certificate_revocation_policy_verification_fails_excessive_lifetime() {
    let fixture =
        fulcio_certificate_fixture("verify-revocation-excessive-lifetime", &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(&fixture.policy_path, "short_lived_certificate", Some(60));

    let verification = run_host_adapter_certificate_revocation_policy_verification(
        HostAdapterCertificateRevocationPolicyVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            trusted_signing_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateRevocationPolicyVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"certificate_revocation_certificate_lifetime_exceeds_policy".to_string()));
}

#[test]
fn host_adapter_certificate_revocation_policy_verification_fails_explicit_status_required() {
    let fixture = fulcio_certificate_fixture("verify-revocation-explicit", &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(&fixture.policy_path, "explicit_status_required", None);

    let verification = run_host_adapter_certificate_revocation_policy_verification(
        HostAdapterCertificateRevocationPolicyVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            trusted_signing_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateRevocationPolicyVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"certificate_revocation_explicit_status_not_implemented".to_string()));
    assert!(verification
        .deferred_verification
        .contains(&"explicit_ocsp_status".to_string()));
}

#[test]
fn host_adapter_verify_certificate_revocation_policy_binary_outputs_json() {
    let fixture = fulcio_certificate_fixture("verify-revocation-binary", &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(
        &fixture.policy_path,
        "short_lived_certificate",
        Some(40_000_000),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-certificate-revocation-policy",
            "--trust-policy-path",
            fixture.policy_path.to_str().expect("utf8 policy path"),
            "--certificate-path",
            fixture
                .certificate_path
                .to_str()
                .expect("utf8 certificate path"),
            "--trusted-signing-time-unix",
            &fixture.verification_time_unix.to_string(),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-certificate-revocation-policy");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["policy_mode"], "short_lived_certificate");
    assert_eq!(
        json["revocation_status"],
        "not_checked_by_short_lived_policy"
    );
}

#[test]
fn host_adapter_certificate_crl_status_verification_passes_good_by_supplied_crl() {
    let fixture = fulcio_certificate_fixture("verify-crl-good", &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(&fixture.policy_path, "explicit_status_required", None);
    let root = fixture.policy_path.parent().expect("policy parent");
    let crl_path = write_crl_fixture(
        root,
        "fresh-empty",
        &fixture.issuer,
        false,
        (2026, 1, 1),
        (2027, 1, 1),
    );

    let verification = run_host_adapter_certificate_crl_status_verification(
        HostAdapterCertificateCrlStatusVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_path: fixture.issuer_certificate_path,
            crl_path,
            verification_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateCrlStatusVerificationStatus::Passed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("good_by_supplied_crl")
    );
    assert!(verification
        .verified_evidence
        .contains(&"crl_status_crl_signature_verified".to_string()));
}

#[test]
fn host_adapter_certificate_crl_status_verification_fails_revoked_certificate() {
    let fixture = fulcio_certificate_fixture("verify-crl-revoked", &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(&fixture.policy_path, "explicit_status_required", None);
    let root = fixture.policy_path.parent().expect("policy parent");
    let crl_path = write_crl_fixture(
        root,
        "fresh-revoked",
        &fixture.issuer,
        true,
        (2026, 1, 1),
        (2027, 1, 1),
    );

    let verification = run_host_adapter_certificate_crl_status_verification(
        HostAdapterCertificateCrlStatusVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_path: fixture.issuer_certificate_path,
            crl_path,
            verification_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateCrlStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("revoked_by_supplied_crl")
    );
    assert!(verification
        .reasons
        .contains(&"crl_status_certificate_revoked".to_string()));
}

#[test]
fn host_adapter_certificate_crl_status_verification_fails_expired_crl() {
    let fixture = fulcio_certificate_fixture("verify-crl-expired", &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(&fixture.policy_path, "explicit_status_required", None);
    let root = fixture.policy_path.parent().expect("policy parent");
    let crl_path = write_crl_fixture(
        root,
        "expired-empty",
        &fixture.issuer,
        false,
        (2025, 1, 1),
        (2025, 12, 31),
    );

    let verification = run_host_adapter_certificate_crl_status_verification(
        HostAdapterCertificateCrlStatusVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_path: fixture.issuer_certificate_path,
            crl_path,
            verification_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateCrlStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_crl_verification")
    );
    assert!(verification
        .reasons
        .contains(&"crl_status_crl_expired".to_string()));
}

#[test]
fn host_adapter_certificate_crl_status_verification_fails_without_explicit_policy() {
    let fixture = fulcio_certificate_fixture("verify-crl-policy", &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(
        &fixture.policy_path,
        "short_lived_certificate",
        Some(40_000_000),
    );
    let root = fixture.policy_path.parent().expect("policy parent");
    let crl_path = write_crl_fixture(
        root,
        "fresh-policy",
        &fixture.issuer,
        false,
        (2026, 1, 1),
        (2027, 1, 1),
    );

    let verification = run_host_adapter_certificate_crl_status_verification(
        HostAdapterCertificateCrlStatusVerificationInput {
            trust_policy_path: fixture.policy_path,
            certificate_path: fixture.certificate_path,
            issuer_certificate_path: fixture.issuer_certificate_path,
            crl_path,
            verification_time_unix: fixture.verification_time_unix,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateCrlStatusVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"crl_status_policy_not_explicit_status_required".to_string()));
}

#[test]
fn host_adapter_verify_certificate_crl_status_binary_outputs_json() {
    let fixture = fulcio_certificate_fixture("verify-crl-binary", &["fulcio-root.pem"]);
    set_sigstore_revocation_policy(&fixture.policy_path, "explicit_status_required", None);
    let root = fixture.policy_path.parent().expect("policy parent");
    let crl_path = write_crl_fixture(
        root,
        "fresh-binary",
        &fixture.issuer,
        false,
        (2026, 1, 1),
        (2027, 1, 1),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-certificate-crl-status",
            "--trust-policy-path",
            fixture.policy_path.to_str().expect("utf8 policy path"),
            "--certificate-path",
            fixture
                .certificate_path
                .to_str()
                .expect("utf8 certificate path"),
            "--issuer-certificate-path",
            fixture
                .issuer_certificate_path
                .to_str()
                .expect("utf8 issuer certificate path"),
            "--crl-path",
            crl_path.to_str().expect("utf8 CRL path"),
            "--verification-time-unix",
            &fixture.verification_time_unix.to_string(),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-certificate-crl-status");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["revocation_status"], "good_by_supplied_crl");
}

fn ocsp_verification_input(
    fixture: &OcspCertificateFixture,
    ocsp_response_path: PathBuf,
    expected_nonce: Option<&[u8]>,
) -> HostAdapterCertificateOcspStatusVerificationInput {
    HostAdapterCertificateOcspStatusVerificationInput {
        trust_policy_path: fixture.policy_path.clone(),
        certificate_path: fixture.certificate_path.clone(),
        issuer_certificate_path: fixture.issuer_certificate_path.clone(),
        ocsp_response_path,
        verification_time_unix: fixture.verification_time_unix,
        expected_nonce_hex: expected_nonce.map(|b| OcspNonceHex::new(hex_bytes(b))),
    }
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_passes_good_by_supplied_ocsp() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-good");
    let ocsp_response_path =
        write_ocsp_response_fixture(&fixture, "good", OcspResponseFixtureOptions::good());

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Passed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("good_by_supplied_ocsp")
    );
    assert!(verification
        .verified_evidence
        .contains(&"ocsp_status_response_signature_verified".to_string()));
    assert!(verification
        .verified_evidence
        .contains(&"ocsp_status_nonce_not_supplied".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_revoked_by_supplied_ocsp() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-revoked");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "revoked",
        OcspResponseFixtureOptions {
            status: OcspFixtureCertStatus::Revoked,
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("revoked_by_supplied_ocsp")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_certificate_revoked".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_unknown_by_supplied_ocsp() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-unknown");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "unknown",
        OcspResponseFixtureOptions {
            status: OcspFixtureCertStatus::Unknown,
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_by_supplied_ocsp")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_certificate_unknown".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_expired_response() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-expired");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "expired",
        OcspResponseFixtureOptions {
            next_update: Some("20260701000000Z"),
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_response_expired".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_future_this_update() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-future-this-update");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "future-this-update",
        OcspResponseFixtureOptions {
            this_update: "20270701000000Z",
            next_update: Some("20280701000000Z"),
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_this_update_in_future".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_future_produced_at() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-future-produced-at");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "future-produced-at",
        OcspResponseFixtureOptions {
            produced_at: "20270701000000Z",
            next_update: Some("20280701000000Z"),
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_produced_at_in_future".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_missing_next_update() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-missing-next-update");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "missing-next-update",
        OcspResponseFixtureOptions {
            next_update: None,
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_next_update_missing".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_cert_id_serial_mismatch() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-cert-id-mismatch");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "cert-id-mismatch",
        OcspResponseFixtureOptions {
            cert_serial: &[0x12, 0x35],
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_certificate_serial_not_found".to_string()));
    assert!(verification
        .reasons
        .contains(&"ocsp_status_single_response_match_missing".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_unsupported_cert_id_hash() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-unsupported-cert-id-hash");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "unsupported-cert-id-hash",
        OcspResponseFixtureOptions {
            hash_algorithm_oid: &[1, 2, 3, 4],
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_cert_id_hash_algorithm_unsupported".to_string()));
    assert!(verification
        .reasons
        .contains(&"ocsp_status_single_response_match_missing".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_bad_target_certificate_signature() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-bad-target-certificate-signature");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "bad-target-certificate-signature",
        OcspResponseFixtureOptions::good(),
    );
    let (_, wrong_ca_params, wrong_ca_key_pair) = test_ocsp_ca(
        "Forge Test Wrong OCSP Leaf Issuer",
        (2026, 1, 1),
        (2027, 1, 1),
    );
    let wrong_issuer = Issuer::from_params(&wrong_ca_params, &wrong_ca_key_pair);
    let wrong_leaf_key_pair = KeyPair::generate().expect("generate wrong OCSP leaf key");
    let wrong_leaf_certificate = test_ocsp_leaf(&wrong_issuer, &wrong_leaf_key_pair);
    fs::write(&fixture.certificate_path, wrong_leaf_certificate.pem())
        .expect("overwrite OCSP leaf with bad target certificate");

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .iter()
        .any(|reason| reason.starts_with("ocsp_status_certificate_signature_failed:")));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_responder_mismatch() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-responder-mismatch");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "responder-mismatch",
        OcspResponseFixtureOptions {
            responder_name_der: Some(fixture.responder_mismatch_name_der.clone()),
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_responder_unauthorized".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_bad_signature() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-bad-signature");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "bad-signature",
        OcspResponseFixtureOptions {
            tamper_signature: true,
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_response_signature_invalid".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_does_not_trust_revoked_bad_signature() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-revoked-bad-signature");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "revoked-bad-signature",
        OcspResponseFixtureOptions {
            status: OcspFixtureCertStatus::Revoked,
            tamper_signature: true,
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert_eq!(verification.revoked_at_unix, None);
    assert!(verification
        .reasons
        .contains(&"ocsp_status_response_signature_invalid".to_string()));
    assert!(verification
        .reasons
        .contains(&"ocsp_status_certificate_revoked".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_does_not_trust_unknown_bad_signature() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-unknown-bad-signature");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "unknown-bad-signature",
        OcspResponseFixtureOptions {
            status: OcspFixtureCertStatus::Unknown,
            tamper_signature: true,
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, None),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_response_signature_invalid".to_string()));
    assert!(verification
        .reasons
        .contains(&"ocsp_status_certificate_unknown".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_passes_matching_nonce() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-nonce-match");
    let expected_nonce = b"forge-ocsp-nonce-20260629";
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "nonce-match",
        OcspResponseFixtureOptions {
            nonce: Some(expected_nonce),
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        ocsp_verification_input(&fixture, ocsp_response_path, Some(expected_nonce)),
    );

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Passed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("good_by_supplied_ocsp")
    );
    assert!(verification
        .verified_evidence
        .contains(&"ocsp_status_nonce_verified".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_nonce_mismatch() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-nonce-mismatch");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "nonce-mismatch",
        OcspResponseFixtureOptions {
            nonce: Some(b"actual-forge-ocsp-nonce"),
            ..OcspResponseFixtureOptions::good()
        },
    );

    let verification =
        run_host_adapter_certificate_ocsp_status_verification(ocsp_verification_input(
            &fixture,
            ocsp_response_path,
            Some(b"expected-forge-ocsp-nonce"),
        ));

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_nonce_mismatch".to_string()));
}

#[test]
fn host_adapter_certificate_ocsp_status_verification_fails_missing_nonce_when_expected() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-nonce-missing");
    let ocsp_response_path = write_ocsp_response_fixture(
        &fixture,
        "nonce-missing",
        OcspResponseFixtureOptions::good(),
    );

    let verification =
        run_host_adapter_certificate_ocsp_status_verification(ocsp_verification_input(
            &fixture,
            ocsp_response_path,
            Some(b"expected-forge-ocsp-nonce"),
        ));

    assert_eq!(
        verification.status,
        HostAdapterCertificateOcspStatusVerificationStatus::Failed
    );
    assert_eq!(
        verification.revocation_status.as_deref(),
        Some("unknown_due_to_failed_ocsp_verification")
    );
    assert!(verification
        .reasons
        .contains(&"ocsp_status_nonce_missing".to_string()));
}

#[test]
fn host_adapter_verify_certificate_ocsp_status_binary_outputs_json() {
    let fixture = ocsp_certificate_fixture("verify-ocsp-binary");
    let ocsp_response_path =
        write_ocsp_response_fixture(&fixture, "binary", OcspResponseFixtureOptions::good());

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-certificate-ocsp-status",
            "--trust-policy-path",
            fixture.policy_path.to_str().expect("utf8 policy path"),
            "--certificate-path",
            fixture
                .certificate_path
                .to_str()
                .expect("utf8 certificate path"),
            "--issuer-certificate-path",
            fixture
                .issuer_certificate_path
                .to_str()
                .expect("utf8 issuer certificate path"),
            "--ocsp-response-path",
            ocsp_response_path
                .to_str()
                .expect("utf8 OCSP response path"),
            "--verification-time-unix",
            &fixture.verification_time_unix.to_string(),
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-certificate-ocsp-status");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["revocation_status"], "good_by_supplied_ocsp");
    assert!(json["verified_evidence"]
        .as_array()
        .expect("verified evidence array")
        .contains(&Value::String(
            "ocsp_status_response_signature_verified".to_string()
        )));
}

#[test]
fn host_adapter_tuf_trusted_root_freshness_verification_passes_fresh_metadata() {
    let policy = sigstore_trust_policy_fixture("verify-tuf-freshness-pass", &["fulcio-root.pem"]);
    let root = policy.policy_path.parent().expect("policy parent");
    let root_metadata_path = write_tuf_metadata(root, "root", 3, "2030-01-01T00:00:00Z");
    let timestamp_metadata_path = write_tuf_metadata(root, "timestamp", 7, "2030-01-01T00:00:00Z");
    let snapshot_metadata_path = write_tuf_metadata(root, "snapshot", 6, "2030-01-01T00:00:00Z");

    let verification = run_host_adapter_tuf_trusted_root_freshness_verification(
        HostAdapterTufTrustedRootFreshnessVerificationInput {
            trust_policy_path: policy.policy_path,
            root_metadata_path,
            timestamp_metadata_path: Some(timestamp_metadata_path),
            snapshot_metadata_path: Some(snapshot_metadata_path),
            targets_metadata_path: None,
            update_start_time_unix: 1_783_391_200,
            min_root_version: Some(3),
            min_timestamp_version: Some(7),
            min_snapshot_version: Some(5),
            min_targets_version: None,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterTufTrustedRootFreshnessVerificationStatus::Passed
    );
    assert_eq!(verification.root_source.as_deref(), Some("tuf"));
    assert_eq!(verification.verified_roles.len(), 3);
    assert!(verification
        .verified_evidence
        .contains(&"tuf_root_expires_after_update_start".to_string()));
    assert!(verification
        .deferred_verification
        .contains(&"tuf_metadata_signature_thresholds".to_string()));
}

#[test]
fn host_adapter_tuf_trusted_root_freshness_verification_fails_expired_root() {
    let policy =
        sigstore_trust_policy_fixture("verify-tuf-freshness-expired", &["fulcio-root.pem"]);
    let root = policy.policy_path.parent().expect("policy parent");
    let root_metadata_path = write_tuf_metadata(root, "root", 3, "2020-01-01T00:00:00Z");

    let verification = run_host_adapter_tuf_trusted_root_freshness_verification(
        HostAdapterTufTrustedRootFreshnessVerificationInput {
            trust_policy_path: policy.policy_path,
            root_metadata_path,
            timestamp_metadata_path: None,
            snapshot_metadata_path: None,
            targets_metadata_path: None,
            update_start_time_unix: 1_783_391_200,
            min_root_version: Some(3),
            min_timestamp_version: None,
            min_snapshot_version: None,
            min_targets_version: None,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterTufTrustedRootFreshnessVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"tuf_root_metadata_expired".to_string()));
}

#[test]
fn host_adapter_tuf_trusted_root_freshness_verification_fails_rollback_version() {
    let policy =
        sigstore_trust_policy_fixture("verify-tuf-freshness-rollback", &["fulcio-root.pem"]);
    let root = policy.policy_path.parent().expect("policy parent");
    let root_metadata_path = write_tuf_metadata(root, "root", 2, "2030-01-01T00:00:00Z");

    let verification = run_host_adapter_tuf_trusted_root_freshness_verification(
        HostAdapterTufTrustedRootFreshnessVerificationInput {
            trust_policy_path: policy.policy_path,
            root_metadata_path,
            timestamp_metadata_path: None,
            snapshot_metadata_path: None,
            targets_metadata_path: None,
            update_start_time_unix: 1_783_391_200,
            min_root_version: Some(3),
            min_timestamp_version: None,
            min_snapshot_version: None,
            min_targets_version: None,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterTufTrustedRootFreshnessVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"tuf_root_version_below_floor".to_string()));
}

#[test]
fn host_adapter_tuf_trusted_root_freshness_verification_fails_invalid_expiry_format() {
    let policy =
        sigstore_trust_policy_fixture("verify-tuf-freshness-invalid-expiry", &["fulcio-root.pem"]);
    let root = policy.policy_path.parent().expect("policy parent");
    let root_metadata_path = write_tuf_metadata(root, "root", 3, "203x-01-01T00:00:00Z");

    let verification = run_host_adapter_tuf_trusted_root_freshness_verification(
        HostAdapterTufTrustedRootFreshnessVerificationInput {
            trust_policy_path: policy.policy_path,
            root_metadata_path,
            timestamp_metadata_path: None,
            snapshot_metadata_path: None,
            targets_metadata_path: None,
            update_start_time_unix: 1_783_391_200,
            min_root_version: Some(3),
            min_timestamp_version: None,
            min_snapshot_version: None,
            min_targets_version: None,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterTufTrustedRootFreshnessVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"tuf_root_expires_format_invalid".to_string()));
}

#[test]
fn host_adapter_tuf_trusted_root_freshness_verification_fails_non_tuf_policy() {
    let policy = sigstore_trust_policy_fixture("verify-tuf-freshness-policy", &["fulcio-root.pem"]);
    let policy_text = fs::read_to_string(&policy.policy_path).expect("read policy");
    fs::write(
        &policy.policy_path,
        policy_text.replace("root_source: \"tuf\"", "root_source: \"manual\""),
    )
    .expect("write non-TUF policy");
    let root = policy.policy_path.parent().expect("policy parent");
    let root_metadata_path = write_tuf_metadata(root, "root", 3, "2030-01-01T00:00:00Z");

    let verification = run_host_adapter_tuf_trusted_root_freshness_verification(
        HostAdapterTufTrustedRootFreshnessVerificationInput {
            trust_policy_path: policy.policy_path,
            root_metadata_path,
            timestamp_metadata_path: None,
            snapshot_metadata_path: None,
            targets_metadata_path: None,
            update_start_time_unix: 1_783_391_200,
            min_root_version: Some(3),
            min_timestamp_version: None,
            min_snapshot_version: None,
            min_targets_version: None,
        },
    );

    assert_eq!(
        verification.status,
        HostAdapterTufTrustedRootFreshnessVerificationStatus::Failed
    );
    assert!(verification
        .reasons
        .contains(&"tuf_freshness_root_source_not_tuf".to_string()));
}

#[test]
fn host_adapter_verify_tuf_trusted_root_freshness_binary_outputs_json() {
    let policy = sigstore_trust_policy_fixture("verify-tuf-freshness-binary", &["fulcio-root.pem"]);
    let root = policy.policy_path.parent().expect("policy parent");
    let root_metadata_path = write_tuf_metadata(root, "root", 3, "2030-01-01T00:00:00Z");

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args([
            "host-adapter-verify-tuf-trusted-root-freshness",
            "--trust-policy-path",
            policy.policy_path.to_str().expect("utf8 policy path"),
            "--root-metadata-path",
            root_metadata_path
                .to_str()
                .expect("utf8 root metadata path"),
            "--update-start-time-unix",
            "1783391200",
            "--min-root-version",
            "3",
            "--json",
        ])
        .output()
        .expect("run forge-core host-adapter-verify-tuf-trusted-root-freshness");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["status"], "passed");
    assert_eq!(json["root_source"], "tuf");
    assert_eq!(json["verified_roles"][0]["role"], "root");
}

#[test]
fn execute_operation_binary_outputs_json_even_when_awaiting_human() {
    let fixture = temp_sidecar_cli_fixture("execute-operation-awaiting-human");
    let operation_relative =
        "docs/fixtures/operation-contract-v0/facilitate-first-product-idea.yaml";
    let operation_path = fixture.app.join(operation_relative);
    fs::create_dir_all(operation_path.parent().expect("operation parent"))
        .expect("create operation fixture directory");
    fs::copy(repo_root().join(operation_relative), &operation_path)
        .expect("copy operation fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["execute-operation", "--root"])
        .arg(&fixture.app)
        .args(["--operation", operation_relative, "--json"])
        .output()
        .expect("run forge-core execute-operation");

    assert!(
        !output.status.success(),
        "awaiting human should not be reported as completed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("\"content\""));
    let json: Value = serde_json::from_str(&stdout).expect("json output");
    assert_eq!(json["status"], "awaiting_human");
    assert_eq!(
        json["operation_id"],
        "op_fixture_facilitate_first_product_idea"
    );
    let reasons = json["reasons"].as_array().expect("reasons array");
    assert!(reasons.iter().any(|item| item == "plan_awaiting_human"));
    assert!(json["command_executions"]
        .as_array()
        .expect("command executions")
        .is_empty());
    assert!(json["effect_applications"]
        .as_array()
        .expect("effect applications")
        .is_empty());
}

#[test]
fn execute_operation_rejects_payload_outside_root_by_default() {
    let outside_payload = temp_payload_file("outside-root", b"secret-ish");
    let error = run_execute_operation(ExecuteOperationInput {
        root: repo_root(),
        effect_store_root: None,
        operation_path: PathBuf::from(
            "docs/fixtures/operation-contract-v0/facilitate-first-product-idea.yaml",
        ),
        command_paths: Vec::new(),
        effect_paths: Vec::new(),
        payloads: vec![PayloadFileSpec {
            target_ref: ".forge-method/artifacts/out.yaml".to_string(),
            path: outside_payload,
        }],
        payload_policy: PayloadLoadPolicy::default(),
        recorded_at: "2026-06-25T00:00:00Z".to_string(),
        tx_id_prefix: "test".to_string(),
        durability: WalDurability::default(),
        risk_audit_rules: None,
        require_citation: false,
    })
    .expect_err("outside-root payload should fail");

    assert!(error.to_string().contains("outside root"));
}

#[test]
fn execute_operation_rejects_payload_larger_than_policy() {
    let outside_payload = temp_payload_file("too-large", b"1234");
    let error = run_execute_operation(ExecuteOperationInput {
        root: repo_root(),
        effect_store_root: None,
        operation_path: PathBuf::from(
            "docs/fixtures/operation-contract-v0/facilitate-first-product-idea.yaml",
        ),
        command_paths: Vec::new(),
        effect_paths: Vec::new(),
        payloads: vec![PayloadFileSpec {
            target_ref: ".forge-method/artifacts/out.yaml".to_string(),
            path: outside_payload,
        }],
        payload_policy: PayloadLoadPolicy {
            max_payload_bytes: 1,
            allow_outside_root: true,
        },
        recorded_at: "2026-06-25T00:00:00Z".to_string(),
        tx_id_prefix: "test".to_string(),
        durability: WalDurability::default(),
        risk_audit_rules: None,
        require_citation: false,
    })
    .expect_err("oversized payload should fail");

    assert!(error.to_string().contains("too large"));
}

#[test]
fn rebuild_effect_index_library_rebuilds_from_committed_wal() {
    let root = temp_repo_root("rebuild-library");
    write_committed_metadata_wal(&root, "payload-secret");

    let result = run_rebuild_effect_index(RebuildEffectIndexInput {
        root: root.clone(),
        wal_relative_path: ".forge-method/wal/effects.ndjson".to_string(),
        index_relative_path: ".forge-method/index/effect-targets.ndjson".to_string(),
        lock_relative_path: ".forge-method/locks/effects.lock".to_string(),
        recorded_at: Some("2026-06-25T00:00:00Z".to_string()),
        durability: WalDurability::NoSync,
    });

    assert_eq!(
        result.status,
        EffectTargetMetadataIndexRebuildStatus::Rebuilt
    );
    assert_eq!(result.rebuilt_records, 1);
    assert_eq!(result.appended_records, 1);
    let index = fs::read_to_string(root.join(".forge-method/index/effect-targets.ndjson"))
        .expect("read rebuilt index");
    assert!(index.contains("\"logical_ref\":\"story.result\""));
    assert!(!index.contains("payload-secret"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebuild_effect_index_binary_outputs_json() {
    let fixture = temp_sidecar_cli_fixture("rebuild-binary");
    write_committed_metadata_wal(&fixture.sidecar, "payload-secret");

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["rebuild-effect-index", "--root"])
        .arg(&fixture.app)
        .args(["--recorded-at", "2026-06-25T00:00:00Z", "--json"])
        .output()
        .expect("run forge-core rebuild-effect-index");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("payload-secret"));
    let json: Value = serde_json::from_str(&stdout).expect("json output");
    assert_eq!(json["status"], "rebuilt");
    assert_eq!(json["rebuilt_records"], 1);
    assert_eq!(json["appended_records"], 1);
    assert_eq!(json["records"][0]["logical_ref"], "story.result");
    assert_eq!(
        json["records"][0]["physical_ref"],
        ".forge-method/artifacts/story.result.yaml"
    );
    assert_eq!(json["records"][0]["recorded_at"], "2026-06-25T00:00:00Z");
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "rebuild-effect-index must not create consumer-local .forge-method"
    );
    assert!(
        fixture
            .state
            .join("index")
            .join("effect-targets.ndjson")
            .exists(),
        "rebuild-effect-index should write the resolved sidecar index"
    );
    let _ = fs::remove_dir_all(
        fixture
            .app
            .parent()
            .expect("fixture app has parent directory"),
    );
}

#[test]
fn rebuild_effect_index_binary_no_sync_emits_warning_and_succeeds() {
    // ADR-0009: --no-sync must (1) parse, (2) print a one-line stderr
    // warning naming the durability trade-off, and (3) succeed. The fixture
    // sets up a committed WAL record so the rebuild has actual work to do.
    //
    // Runs WITHOUT --json: per the codebase convention (shared with the four
    // claim sites and execute-operation), the human-facing --no-sync warning
    // is suppressed in --json mode to keep stdout/stderr clean for machine
    // consumers (MCP, agents). The durability trade-off is still observable
    // via the human text path, which is what this test exercises.
    let fixture = temp_sidecar_cli_fixture("rebuild-no-sync");
    write_committed_metadata_wal(&fixture.sidecar, "payload-secret");

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["rebuild-effect-index", "--root"])
        .arg(&fixture.app)
        .args(["--no-sync", "--recorded-at", "2026-06-25T00:00:00Z"])
        .output()
        .expect("run forge-core rebuild-effect-index --no-sync");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--no-sync active"),
        "stderr must warn about --no-sync; got: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Human text path: status is in the formatted line, not JSON.
    assert!(
        stdout.contains("rebuilt") && stdout.contains("appended"),
        "human output should report rebuilt/appended counts; got: {stdout}"
    );
    let _ = fs::remove_dir_all(
        fixture
            .app
            .parent()
            .expect("fixture app has parent directory"),
    );
}

#[test]
fn rebuild_effect_index_binary_default_does_not_warn() {
    // Symmetric counterpart: the default path (no --no-sync) must NOT
    // emit the durability warning. Guards against accidental inversion.
    let fixture = temp_sidecar_cli_fixture("rebuild-default-no-warn");
    write_committed_metadata_wal(&fixture.sidecar, "payload-secret");

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["rebuild-effect-index", "--root"])
        .arg(&fixture.app)
        .args(["--recorded-at", "2026-06-25T00:00:00Z", "--json"])
        .output()
        .expect("run forge-core rebuild-effect-index");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("--no-sync active"),
        "default path must not emit the --no-sync warning; got: {stderr}"
    );
    let _ = fs::remove_dir_all(
        fixture
            .app
            .parent()
            .expect("fixture app has parent directory"),
    );
}

#[test]
fn query_effect_index_library_filters_metadata_records() {
    let root = temp_repo_root("query-library");
    write_effect_index_record(
        &root,
        "story.result",
        "effect.story.result",
        "op.story.result",
        "payload-secret",
    );
    write_effect_index_record(
        &root,
        "story.other",
        "effect.story.other",
        "op.story.other",
        "other-secret",
    );

    let result = run_query_effect_index(QueryEffectIndexInput {
        root: root.clone(),
        logical_ref: Some("story.result".to_string()),
        latest_per_target: true,
        ..QueryEffectIndexInput::default()
    });

    assert_eq!(result.status, EffectTargetMetadataIndexQueryStatus::Queried);
    assert_eq!(result.scanned_records, 2);
    assert_eq!(result.matched_records, 1);
    assert_eq!(result.returned_records, 1);
    assert_eq!(result.consumer_use, EffectMetadataConsumerUse::Discovery);
    assert!(!result.authority_boundary.is_workflow_authority);
    assert_eq!(result.records[0].logical_ref, "story.result");
    assert_ne!(
        result.records[0].content_hash,
        Some("payload-secret".to_string())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn query_effect_index_binary_outputs_json() {
    let fixture = temp_sidecar_cli_fixture("query-binary");
    write_effect_index_record(
        &fixture.sidecar,
        "story.result",
        "effect.story.first",
        "op.story.first",
        "payload-secret",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["query-effect-index", "--root"])
        .arg(&fixture.app)
        .args([
            "--logical-ref",
            "story.result",
            "--consumer-use",
            "handoff_context",
            "--latest",
            "--json",
        ])
        .output()
        .expect("run forge-core query-effect-index");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("payload-secret"));
    let json: Value = serde_json::from_str(&stdout).expect("json output");
    assert_eq!(json["status"], "queried");
    assert_eq!(json["scanned_records"], 1);
    assert_eq!(json["matched_records"], 1);
    assert_eq!(json["returned_records"], 1);
    assert_eq!(json["latest_per_target"], true);
    assert_eq!(json["consumer_use"], "handoff_context");
    assert_eq!(json["authority_boundary"]["is_workflow_authority"], false);
    assert!(json["authority_boundary"]["forbidden_authority"]
        .as_array()
        .expect("forbidden authority array")
        .contains(&Value::String("phase_transition".to_string())));
    assert_eq!(json["records"][0]["logical_ref"], "story.result");
    assert_eq!(
        json["records"][0]["physical_ref"],
        ".forge-method/artifacts/story.result.yaml"
    );
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "query-effect-index must not create consumer-local .forge-method"
    );
    let _ = fs::remove_dir_all(
        fixture
            .app
            .parent()
            .expect("fixture app has parent directory"),
    );
}

#[test]
fn query_effect_index_context_outputs_bounded_json() {
    let fixture = temp_sidecar_cli_fixture("query-context-binary");
    write_effect_index_record(
        &fixture.sidecar,
        "story.result",
        "effect.story.first",
        "op.story.first",
        "payload-secret",
    );
    write_effect_index_record(
        &fixture.sidecar,
        "story.other",
        "effect.story.other",
        "op.story.other",
        "other-secret",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["query-effect-index", "--root"])
        .arg(&fixture.app)
        .args([
            "--consumer-use",
            "diagnostics",
            "--context",
            "--max-context-groups",
            "1",
            "--adapter-kind",
            "codex",
            "--adapter-trigger",
            "diagnostics",
            "--json",
        ])
        .output()
        .expect("run forge-core query-effect-index context");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("payload-secret"));
    assert!(!stdout.contains("other-secret"));
    let json: Value = serde_json::from_str(&stdout).expect("json output");
    assert_eq!(json["status"], "built");
    assert_eq!(json["source_consumer_use"], "diagnostics");
    assert_eq!(json["total_groups"], 2);
    assert_eq!(json["returned_groups"], 1);
    assert_eq!(json["omitted_groups"], 1);
    assert_eq!(json["authority_boundary"]["is_workflow_authority"], false);
    assert_eq!(json["adapter_presentation"]["adapter_kind"], "codex");
    assert_eq!(json["adapter_presentation"]["trigger"], "diagnostics");
    assert_eq!(
        json["adapter_presentation"]["presentation_mode"],
        "advisory_context"
    );
    assert_eq!(
        json["adapter_presentation"]["automatic_invocation_allowed"],
        true
    );
    assert_eq!(
        json["adapter_presentation"]["may_create_workflow_authority"],
        false
    );
    assert_eq!(json["groups"][0]["record_count"], 1);
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "query-effect-index --context must not create consumer-local .forge-method"
    );
    let _ = fs::remove_dir_all(
        fixture
            .app
            .parent()
            .expect("fixture app has parent directory"),
    );
}

#[test]
fn query_effect_index_rejects_workflow_authority_consumer_use() {
    let root = temp_repo_root("query-consumer-use-invalid");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-core"))
        .args(["query-effect-index", "--root"])
        .arg(&root)
        .args(["--consumer-use", "workflow_authority", "--json"])
        .output()
        .expect("run forge-core query-effect-index");

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let _ = fs::remove_dir_all(root);
}

fn write_effect_index_record(
    root: &Path,
    logical_ref: &str,
    effect_id: &str,
    operation_id: &str,
    payload_marker: &str,
) {
    append_json_line(
        root,
        ".forge-method/index/effect-targets.ndjson",
        &EffectTargetMetadataRecord {
            schema_version: "0.1".to_string(),
            record_kind: EffectTargetMetadataRecordKind::EffectTarget,
            recorded_at: Some("2026-06-25T00:00:00Z".to_string()),
            operation_id: StableId(operation_id.to_string()),
            effect_id: StableId(effect_id.to_string()),
            logical_ref: logical_ref.to_string(),
            physical_ref: format!(".forge-method/artifacts/{logical_ref}.yaml"),
            target_kind: EffectTargetKind::ArtifactId,
            access_mode: AccessMode::Write,
            content_hash: Some(sha256_content_hash(payload_marker.as_bytes())),
            byte_len: payload_marker.len() as u64,
            actor_agent_id: StableId("codex-test".to_string()),
            actor_role: ActorRole::Runtime,
            destructive: false,
            redaction_hint: StableId("raw_content_not_indexed".to_string()),
        },
    )
    .expect("append effect index record");
}

fn write_committed_metadata_wal(root: &Path, payload_marker: &str) {
    let wal_ref = ".forge-method/wal/effects.ndjson";
    append_json_line(
        root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-cli-rebuild".to_string(),
            stage: EffectWalStage::Begin,
            effect_id: StableId("effect.story.result".to_string()),
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append begin");
    append_json_line(
        root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-cli-rebuild".to_string(),
            stage: EffectWalStage::WriteApplied,
            effect_id: StableId("effect.story.result".to_string()),
            target_ref: Some("story.result".to_string()),
            physical_target_ref: Some(".forge-method/artifacts/story.result.yaml".to_string()),
            target_metadata: Some(EffectWalTargetMetadata {
                operation_id: StableId("op.story.result".to_string()),
                target_kind: EffectTargetKind::ArtifactId,
                access_mode: AccessMode::Create,
                content_hash: Some(sha256_content_hash(payload_marker.as_bytes())),
                byte_len: payload_marker.len() as u64,
                actor_agent_id: StableId("codex-test".to_string()),
                actor_role: ActorRole::Runtime,
                destructive: false,
                redaction_hint: StableId("raw_content_not_indexed".to_string()),
            }),
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append write applied");
    append_json_line(
        root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-cli-rebuild".to_string(),
            stage: EffectWalStage::Commit,
            effect_id: StableId("effect.story.result".to_string()),
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append commit");
}
