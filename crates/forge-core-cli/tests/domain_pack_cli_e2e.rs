use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    domain_pack_package_record_digest, domain_pack_publisher_signing_bytes,
    domain_pack_registry_signing_bytes, domain_pack_registry_snapshot_digest,
};
use forge_core_contracts::*;
use forge_core_decisions::MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create fixture destination");
    for entry in fs::read_dir(source).expect("read fixture source") {
        let entry = entry.expect("fixture entry");
        let destination = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).expect("copy fixture");
        }
    }
}

fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, current: &Path, output: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(current).expect("read snapshot tree") {
            let entry = entry.expect("snapshot entry");
            if entry.path().is_dir() {
                walk(root, &entry.path(), output);
            } else {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("relative fixture")
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = fs::read(entry.path()).expect("snapshot bytes");
                output.insert(relative, format!("{:x}", Sha256::digest(bytes)));
            }
        }
    }
    let mut output = BTreeMap::new();
    walk(root, root, &mut output);
    output
}

fn fresh_temp(label: &str) -> PathBuf {
    let temp = std::env::temp_dir().join(format!(
        "forge-domain-pack-cli-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("clean stale temp");
    }
    fs::create_dir_all(&temp).expect("create temp root");
    temp
}

fn write_oversized(path: &Path) {
    let file = fs::File::create(path).expect("create oversized input");
    file.set_len((MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES as u64) + 1)
        .expect("size oversized input");
}

fn assert_too_large(temp: &Path, args: &[&str], label: &str) {
    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(temp)
        .args(args)
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(
        stderr.contains(label) && stderr.contains("exceeds maximum"),
        "unexpected bounded-read error: {stderr}"
    );
}

fn create_dir_link(link: &Path, target: &Path) {
    #[cfg(windows)]
    {
        // PowerShell binds paths as parameters instead of reparsing one
        // `cmd /C` command string, which keeps nested temporary paths intact.
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "New-Item -ItemType Junction -Path $env:FORGE_TEST_LINK -Target $env:FORGE_TEST_TARGET | Out-Null",
            ])
            .env("FORGE_TEST_LINK", link)
            .env("FORGE_TEST_TARGET", target)
            .output()
            .expect("create Windows directory junction");
        assert!(
            output.status.success(),
            "junction creation failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).expect("create Unix directory symlink");

    #[cfg(not(any(windows, unix)))]
    panic!("directory-link escape tests require Windows junctions or Unix symlinks");
}

fn remove_dir_link(link: &Path) {
    #[cfg(windows)]
    fs::remove_dir(link).expect("remove Windows directory link");

    #[cfg(unix)]
    fs::remove_file(link).expect("remove Unix directory symlink");

    #[cfg(not(any(windows, unix)))]
    panic!("directory-link escape tests require Windows junctions or Unix symlinks");
}

fn digest(seed: u64) -> String {
    format!("sha256:{seed:064x}")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        },
    )
}

#[allow(clippy::too_many_lines)] // Cohesive cryptographic fixture: all cross-linked fields are sealed together.
fn write_signed_supply_chain(operator_root: &Path) -> (PathBuf, PathBuf) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let registry_key = SigningKey::from_bytes(&[41_u8; 32]);
    let publisher_key = SigningKey::from_bytes(&[42_u8; 32]);
    let registry_key_id = StableId("registry.key.cli".to_owned());
    let registry_id = StableId("registry.domain-pack.cli".to_owned());
    let audience = StableId("forge.domain-pack.cli".to_owned());
    let policy = DomainPackTrustPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_trust_policy: DomainPackTrustPolicy {
            policy_id: StableId("policy.domain-pack.cli".to_owned()),
            policy_version: "1.0.0".to_owned(),
            audience: audience.clone(),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            registry_keys: vec![DomainPackRegistryTrustKey {
                key_id: registry_key_id.clone(),
                role: DomainPackRegistryTrustRole::RegistrySigner,
                public_key_hex: hex(&registry_key.verifying_key().to_bytes()),
                status: DomainPackCredentialStatus::Active,
                valid_from_unix: now.saturating_sub(60),
                valid_until_unix: now + 3_600,
            }],
            required_registry_signature_threshold: 1,
            minimum_activation_assurance: DomainPackSourceAssurance::SupplyChainVerified,
            rules: vec![DomainPackTrustRule {
                rule_id: StableId("rule.cli".to_owned()),
                pack: DomainPackCoordinate {
                    publisher: StableId("publisher.cli".to_owned()),
                    name: StableId("foundation".to_owned()),
                },
                package_digest: None,
                content_digest: None,
                disposition: DomainPackTrustDisposition::InspectOnly,
            }],
            default_disposition: DomainPackTrustDisposition::Reject,
        },
    };
    let mut record = DomainPackRegistryPackageRecord {
        identity: DomainPackIdentity {
            publisher: StableId("publisher.cli".to_owned()),
            name: StableId("foundation".to_owned()),
            namespace: StableId("sample.foundation".to_owned()),
            version: "1.0.0".to_owned(),
        },
        package_digest: digest(80_001),
        manifest_digest: digest(80_002),
        content_digest: digest(80_003),
        license_digest: digest(80_004),
        fixture_digests: vec![digest(80_005)],
        namespace_grant_id: StableId("grant.cli".to_owned()),
        publisher_credential_id: StableId("credential.cli".to_owned()),
        publisher_signature_hex: "00".repeat(64),
        record_digest: digest(80_006),
    };
    record.record_digest = domain_pack_package_record_digest(&record).expect("record digest");
    let publisher_bytes = domain_pack_publisher_signing_bytes(&registry_id, &audience, &record)
        .expect("publisher signing bytes");
    record.publisher_signature_hex = hex(&publisher_key.sign(&publisher_bytes).to_bytes());
    let mut registry = DomainPackSupplyChainRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
            registry_id,
            registry_version: "1.0.0".to_owned(),
            audience,
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation: 1,
            previous_snapshot_digest: None,
            issued_at_unix: now.saturating_sub(30),
            expires_at_unix: now + 1_800,
            publisher_credentials: vec![DomainPackPublisherCredential {
                credential_id: StableId("credential.cli".to_owned()),
                publisher: StableId("publisher.cli".to_owned()),
                public_key_hex: hex(&publisher_key.verifying_key().to_bytes()),
                status: DomainPackCredentialStatus::Active,
                valid_from_unix: now.saturating_sub(60),
                valid_until_unix: now + 3_600,
            }],
            namespace_grants: vec![DomainPackNamespaceGrant {
                grant_id: StableId("grant.cli".to_owned()),
                publisher: StableId("publisher.cli".to_owned()),
                namespace_prefix: StableId("sample".to_owned()),
                valid_from_unix: now.saturating_sub(60),
                valid_until_unix: now + 3_600,
            }],
            packages: vec![record],
            revocations: Vec::new(),
            snapshot_digest: digest(80_007),
            signatures: Vec::new(),
        },
    };
    registry.domain_pack_supply_chain_registry.snapshot_digest =
        domain_pack_registry_snapshot_digest(&registry).expect("snapshot digest");
    let registry_bytes = domain_pack_registry_signing_bytes(
        &registry,
        &registry_key_id,
        DomainPackRegistryTrustRole::RegistrySigner,
    )
    .expect("registry signing bytes");
    registry
        .domain_pack_supply_chain_registry
        .signatures
        .push(DomainPackRegistrySignature {
            signer_key_id: registry_key_id,
            role: DomainPackRegistryTrustRole::RegistrySigner,
            signature_hex: hex(&registry_key.sign(&registry_bytes).to_bytes()),
        });

    let policy_path = operator_root.join("trust-policy.yaml");
    let registry_path = operator_root.join("registry.yaml");
    fs::write(
        &policy_path,
        yaml_serde::to_string(&policy).expect("serialize policy"),
    )
    .expect("write policy");
    fs::write(
        &registry_path,
        yaml_serde::to_string(&registry).expect("serialize registry"),
    )
    .expect("write registry");
    (policy_path, registry_path)
}

fn resolution_candidate(version: &str, seed: u64) -> DomainPackResolutionCandidate {
    let base: DomainPackCompositionRequestDocument = yaml_serde::from_str(
        &fs::read_to_string(
            repo_root().join("docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml"),
        )
        .expect("read composition fixture"),
    )
    .expect("parse composition fixture");
    let mut input = base
        .domain_pack_composition_request
        .candidates
        .into_iter()
        .next()
        .expect("foundation candidate");
    version.clone_into(&mut input.manifest.domain_pack_manifest.identity.version);
    version.clone_into(&mut input.content.domain_pack_content.pack.version);
    let package = DomainPackPackageBinding {
        package_ref: RepoPath(format!("packages/foundation-{version}.yaml")),
        package_digest: digest(seed),
        manifest: input.manifest_binding.clone(),
        content: input.manifest.domain_pack_manifest.content.clone(),
        license: input
            .manifest
            .domain_pack_manifest
            .provenance
            .license_text
            .clone(),
        fixtures: input
            .content
            .domain_pack_content
            .fixtures
            .iter()
            .map(|fixture| fixture.artifact.clone())
            .collect(),
    };
    DomainPackResolutionCandidate {
        input,
        package,
        registry_record_digest: Some(digest(seed + 10_000)),
    }
}

fn resolution_registry(
    candidates: &[DomainPackResolutionCandidate],
) -> DomainPackSupplyChainRegistryDocument {
    DomainPackSupplyChainRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
            registry_id: StableId("registry.fixture".to_owned()),
            registry_version: "1.0.0".to_owned(),
            audience: StableId("forge.fixture".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation: 1,
            previous_snapshot_digest: None,
            issued_at_unix: 100,
            expires_at_unix: 200,
            publisher_credentials: vec![DomainPackPublisherCredential {
                credential_id: StableId("credential.fixture".to_owned()),
                publisher: StableId("forge.fixture".to_owned()),
                public_key_hex: "00".repeat(32),
                status: DomainPackCredentialStatus::Active,
                valid_from_unix: 0,
                valid_until_unix: 300,
            }],
            namespace_grants: vec![DomainPackNamespaceGrant {
                grant_id: StableId("grant.fixture".to_owned()),
                publisher: StableId("forge.fixture".to_owned()),
                namespace_prefix: StableId("sample".to_owned()),
                valid_from_unix: 0,
                valid_until_unix: 300,
            }],
            packages: candidates
                .iter()
                .map(|candidate| DomainPackRegistryPackageRecord {
                    identity: candidate
                        .input
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .clone(),
                    package_digest: candidate.package.package_digest.clone(),
                    manifest_digest: candidate.package.manifest.canonical_sha256.clone(),
                    content_digest: candidate.package.content.canonical_sha256.clone(),
                    license_digest: candidate.package.license.canonical_sha256.clone(),
                    fixture_digests: candidate
                        .package
                        .fixtures
                        .iter()
                        .map(|fixture| fixture.canonical_sha256.clone())
                        .collect(),
                    namespace_grant_id: StableId("grant.fixture".to_owned()),
                    publisher_credential_id: StableId("credential.fixture".to_owned()),
                    publisher_signature_hex: "00".repeat(64),
                    record_digest: candidate
                        .registry_record_digest
                        .clone()
                        .expect("record digest"),
                })
                .collect(),
            revocations: Vec::new(),
            snapshot_digest: digest(9_000),
            signatures: Vec::new(),
        },
    }
}

fn resolution_request(
    candidates: Vec<DomainPackResolutionCandidate>,
) -> DomainPackResolutionRequestDocument {
    let base: DomainPackCompositionRequestDocument = yaml_serde::from_str(
        &fs::read_to_string(
            repo_root().join("docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml"),
        )
        .expect("read composition fixture"),
    )
    .expect("parse composition fixture");
    let base = base.domain_pack_composition_request;
    DomainPackResolutionRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_resolution_request: DomainPackResolutionRequest {
            request_id: StableId("resolution.fixture".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: base.requirements.project_id.clone(),
            forge_core_version: base.forge_core_version,
            core: base.core,
            requirements: DomainPackProjectRequirementsDocument {
                schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
                domain_pack_project_requirements: base.requirements,
            },
            roots: vec![DomainPackResolutionRoot {
                pack: DomainPackCoordinate {
                    publisher: StableId("forge.fixture".to_owned()),
                    name: StableId("foundation".to_owned()),
                },
                version_requirement: ">=1,<3".to_owned(),
                required_content_digest: None,
                reason: DomainPackResolutionRootReason::InstallIntent,
            }],
            current_lock: None,
            policy: DomainPackResolutionPolicy {
                selection: DomainPackVersionSelectionPolicy::MinimalChangeThenHighestCompatible,
                prerelease: DomainPackPrereleasePolicy::ExplicitOnly,
                duplicate_version: DomainPackDuplicateVersionPolicy::RejectDivergentContent,
                dependency_source: DomainPackDependencySourcePolicy::ExactPublisherOnly,
                unrelated_updates: DomainPackUnrelatedUpdatePolicy::PreserveLocked,
            },
            registry_snapshot_digest: digest(9_000),
            candidates,
        },
    }
}

fn fixture_document<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    let path = repo_root().join(relative);
    yaml_serde::from_str(&fs::read_to_string(&path).expect("read typed fixture"))
        .unwrap_or_else(|error| panic!("parse fixture '{}': {error}", path.display()))
}

fn artifact_guard_preflight(
    binding: DomainPackArtifactBinding,
) -> DomainPackLifecyclePreflightDocument {
    let project_snapshot_digest = digest(90_001);
    let operation = DomainPackLifecycleOperation::Install {
        root: DomainPackCoordinate {
            publisher: StableId("forge.fixture".to_owned()),
            name: StableId("foundation".to_owned()),
        },
    };
    let expected_state = DomainPackExpectedLifecycleState::Uninitialized {
        project_snapshot_digest: project_snapshot_digest.clone(),
    };
    let request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: StableId("lifecycle.artifact-guard".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: StableId("project.domain-pack.fixture".to_owned()),
            principal_id: StableId("principal.fixture".to_owned()),
            operation,
            expected_state: expected_state.clone(),
            resolution_request_digest: digest(90_002),
            project_snapshot_digest,
        },
    };
    DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: StableId("preflight.artifact-guard".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request,
            request_digest: digest(90_003),
            observed_state: expected_state,
            resolution: fixture_document(
                "docs/fixtures/domain-pack-lifecycle-v0/valid/resolution-projection.yaml",
            ),
            proposed_lock: fixture_document(
                "docs/fixtures/domain-pack-lifecycle-v0/valid/exact-lock.yaml",
            ),
            composition: fixture_document(
                "docs/fixtures/domain-pack-v0/projections/neutral-two-pack.expected.yaml",
            ),
            supply_chain_assessments: Vec::new(),
            trust_decisions: Vec::new(),
            capability_gaps: Vec::new(),
            compatibility_report: fixture_document(
                "docs/fixtures/domain-pack-lifecycle-v0/valid/compatibility-report.yaml",
            ),
            staged_artifacts: vec![binding],
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: Vec::new(),
            preflight_digest: digest(90_004),
        },
    }
}

fn lifecycle_preflight_command(
    current_dir: &Path,
    project_root: &Path,
    artifact_root: &Path,
    state_root: &Path,
    preflight_file: &Path,
    trust_policy_file: &Path,
    registry_file: &Path,
) -> Command {
    let mut command = Command::cargo_bin("forge-core").expect("forge-core binary");
    command
        .current_dir(current_dir)
        .arg("domain-pack")
        .arg("preflight")
        .arg("--preflight-file")
        .arg(preflight_file)
        .arg("--trust-policy-file")
        .arg(trust_policy_file)
        .arg("--registry-file")
        .arg(registry_file)
        .arg("--resolution-request-file")
        .arg(current_dir.join("resolution.yaml"))
        .arg("--composition-request-file")
        .arg(current_dir.join("composition.yaml"))
        .arg("--trust-input-file")
        .arg(current_dir.join("trust-input.yaml"))
        .arg("--project-root")
        .arg(project_root)
        .arg("--artifact-root")
        .arg(artifact_root)
        .arg("--state-root")
        .arg(state_root)
        .arg("--json");
    command
}

#[test]
fn agent_cli_validates_and_composes_without_writes() {
    let temp = std::env::temp_dir().join(format!(
        "forge-domain-pack-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );
    let before = snapshot(&temp);

    let validate = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "validate",
            "--manifest-file",
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "--content-file",
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "--artifact-root",
            ".",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let validate_json: serde_json::Value =
        serde_json::from_slice(&validate).expect("validate JSON");
    assert_eq!(validate_json["ok"], true);
    assert_eq!(validate_json["data"]["structurally_valid"], true);
    assert_eq!(validate_json["data"]["authority"], "candidate_only");

    let compose = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "compose",
            "--request-file",
            "docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml",
            "--artifact-root",
            ".",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let compose_json: serde_json::Value = serde_json::from_slice(&compose).expect("compose JSON");
    assert_eq!(compose_json["ok"], true);
    assert_eq!(
        compose_json["data"]["domain_pack_composition_projection"]["status"],
        "composable"
    );
    assert_eq!(
        compose_json["data"]["domain_pack_composition_projection"]["authority"],
        "candidate_only"
    );
    assert_eq!(
        snapshot(&temp),
        before,
        "read-only CLI changed fixture tree"
    );

    fs::remove_dir_all(temp).expect("cleanup CLI fixture");
}

#[test]
fn compose_rejects_artifact_path_escape() {
    let temp = std::env::temp_dir().join(format!(
        "forge-domain-pack-cli-escape-{}",
        std::process::id()
    ));
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("clean stale temp");
    }
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );
    let request_path = fixture_root.join("requests/escape.yaml");
    let request = fs::read_to_string(fixture_root.join("requests/neutral-extension-removed.yaml"))
        .expect("request");
    fs::write(
        &request_path,
        request.replace(
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "../outside.yaml",
        ),
    )
    .expect("escape request");

    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "compose",
            "--request-file",
            "docs/fixtures/domain-pack-v0/requests/escape.yaml",
            "--artifact-root",
            ".",
            "--json",
        ])
        .assert()
        .failure();
    fs::remove_dir_all(temp).expect("cleanup escape fixture");
}

#[test]
fn domain_pack_inputs_are_bounded_before_parse() {
    let temp = fresh_temp("bounded-inputs");
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );

    let oversized_request = temp.join("oversized-request.yaml");
    write_oversized(&oversized_request);
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "compose",
            "--request-file",
            "oversized-request.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "composition request",
    );

    let oversized_manifest = temp.join("oversized-manifest.yaml");
    write_oversized(&oversized_manifest);
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "validate",
            "--manifest-file",
            "oversized-manifest.yaml",
            "--content-file",
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "manifest",
    );

    let oversized_content = temp.join("oversized-content.yaml");
    write_oversized(&oversized_content);
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "validate",
            "--manifest-file",
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "--content-file",
            "oversized-content.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "content",
    );

    write_oversized(&fixture_root.join("artifacts/license-notice.yaml"));
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "validate",
            "--manifest-file",
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "--content-file",
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "license artifact",
    );

    fs::remove_dir_all(temp).expect("cleanup bounded-input fixture");
}

#[test]
fn compose_rejects_canonical_directory_link_escape() {
    let temp = fresh_temp("canonical-escape");
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );
    let outside = temp.join("outside");
    fs::create_dir_all(&outside).expect("create external artifact directory");
    fs::copy(
        fixture_root.join("content/foundation.yaml"),
        outside.join("foundation.yaml"),
    )
    .expect("copy external artifact");
    let link = fixture_root.join("escape-link");
    create_dir_link(&link, &outside);

    let request_path = fixture_root.join("requests/canonical-escape.yaml");
    let request = fs::read_to_string(fixture_root.join("requests/neutral-extension-removed.yaml"))
        .expect("request");
    fs::write(
        &request_path,
        request.replace(
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "escape-link/foundation.yaml",
        ),
    )
    .expect("canonical escape request");

    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "compose",
            "--request-file",
            "docs/fixtures/domain-pack-v0/requests/canonical-escape.yaml",
            "--artifact-root",
            "docs/fixtures/domain-pack-v0",
            "--json",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8_lossy(&stderr).contains("escapes canonical --artifact-root"),
        "unexpected canonical confinement error: {}",
        String::from_utf8_lossy(&stderr)
    );

    remove_dir_link(&link);
    fs::remove_dir_all(temp).expect("cleanup canonical escape fixture");
}

#[test]
fn lifecycle_status_and_recover_emit_integrity_checked_empty_state() {
    let temp = fresh_temp("lifecycle-empty-state");
    let state_root = temp.join(".forge-method");
    fs::create_dir(&state_root).expect("create canonical state root");

    for subcommand in ["status", "recover"] {
        let stdout = Command::cargo_bin("forge-core")
            .expect("forge-core binary")
            .current_dir(&temp)
            .args([
                "domain-pack",
                subcommand,
                "--state-root",
                ".forge-method",
                "--json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let envelope: serde_json::Value =
            serde_json::from_slice(&stdout).expect("lifecycle JSON envelope");
        assert_eq!(envelope["ok"], true);
        assert_eq!(envelope["command"], format!("domain-pack {subcommand}"));
        assert_eq!(envelope["data"]["active"], false);
        assert_eq!(envelope["data"]["active_pointer"], serde_json::Value::Null);
        assert_eq!(envelope["data"]["active_lock"], serde_json::Value::Null);
        assert_eq!(envelope["data"]["ledger_records"], serde_json::json!([]));
        assert_eq!(
            envelope["data"]["recovery_report"]["domain_pack_recovery_report"]["authority"],
            "candidate_only"
        );
        assert_eq!(
            envelope["data"]["recovery_report"]["domain_pack_recovery_report"]["status"],
            "clean"
        );
        assert_eq!(
            envelope["data"]["recovery_report"]["domain_pack_recovery_report"]["active_state"],
            serde_json::Value::Null
        );
        assert_eq!(envelope["data"]["recovery_checked"], true);
        assert!(envelope["data"]["state_root"]
            .as_str()
            .expect("state root")
            .ends_with("/.forge-method"));
    }

    fs::remove_dir_all(temp).expect("cleanup lifecycle state fixture");
}

#[test]
fn resolve_requires_both_closed_typed_inputs() {
    let temp = fresh_temp("resolve-inputs");
    fs::write(temp.join("request.yaml"), "schema_version: '0.2'\n")
        .expect("write malformed request");
    fs::write(temp.join("registry.yaml"), "schema_version: '0.2'\n")
        .expect("write malformed registry");

    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "resolve",
            "--request-file",
            "request.yaml",
            "--registry-file",
            "registry.yaml",
            "--json",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8_lossy(&stderr).contains("not a closed typed document"),
        "unexpected resolution input error: {}",
        String::from_utf8_lossy(&stderr)
    );

    fs::remove_dir_all(temp).expect("cleanup resolution input fixture");
}

#[test]
fn resolve_emits_deterministic_highest_compatible_projection() {
    let temp = fresh_temp("resolve-success");
    let candidates = vec![
        resolution_candidate("1.0.0", 1),
        resolution_candidate("2.0.0", 2),
    ];
    let request = resolution_request(candidates.clone());
    let registry = resolution_registry(&candidates);
    fs::write(
        temp.join("request.yaml"),
        yaml_serde::to_string(&request).expect("serialize resolution request"),
    )
    .expect("write resolution request");
    fs::write(
        temp.join("registry.yaml"),
        yaml_serde::to_string(&registry).expect("serialize registry"),
    )
    .expect("write registry");

    let stdout = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "resolve",
            "--request-file",
            "request.yaml",
            "--registry-file",
            "registry.yaml",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: serde_json::Value =
        serde_json::from_slice(&stdout).expect("resolution JSON envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(
        envelope["data"]["domain_pack_resolution_projection"]["status"],
        "resolved"
    );
    assert_eq!(
        envelope["data"]["domain_pack_resolution_projection"]["authority"],
        "candidate_only"
    );
    assert_eq!(
        envelope["data"]["domain_pack_resolution_projection"]["selected"][0]["identity"]["version"],
        "2.0.0"
    );

    fs::remove_dir_all(temp).expect("cleanup successful resolution fixture");
}

#[test]
fn lifecycle_preflight_requires_an_explicit_project_snapshot_root() {
    let temp = fresh_temp("preflight-project-root");
    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "preflight",
            "--preflight-file",
            "preflight.yaml",
            "--trust-policy-file",
            "trust-policy.yaml",
            "--registry-file",
            "registry.yaml",
            "--resolution-request-file",
            "resolution.yaml",
            "--composition-request-file",
            "composition.yaml",
            "--trust-input-file",
            "trust-input.yaml",
            "--json",
        ])
        .assert()
        .code(2)
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains("--project-root <path>"),
        "usage did not expose the mandatory project snapshot root: {stderr}"
    );

    fs::remove_dir_all(temp).expect("cleanup project-root requirement fixture");
}

#[test]
fn lifecycle_commands_reject_caller_authored_time_and_hide_it_from_help() {
    let temp = fresh_temp("preflight-real-clock");
    let help = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args(["domain-pack", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8_lossy(&help);
    assert!(
        !help.contains("--now-unix"),
        "domain-pack help exposed caller-authored time: {help}"
    );

    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "preflight",
            "--preflight-file",
            "preflight.yaml",
            "--trust-policy-file",
            "trust-policy.yaml",
            "--registry-file",
            "registry.yaml",
            "--resolution-request-file",
            "resolution.yaml",
            "--composition-request-file",
            "composition.yaml",
            "--trust-input-file",
            "trust-input.yaml",
            "--project-root",
            ".",
            "--now-unix",
            "100",
            "--json",
        ])
        .assert()
        .code(2)
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        !stderr.contains("--now-unix"),
        "usage accidentally re-advertised rejected caller time: {stderr}"
    );
    assert!(
        stderr.contains("domain-pack preflight"),
        "unexpected rejection diagnostic: {stderr}"
    );

    fs::remove_dir_all(temp).expect("cleanup real-clock CLI fixture");
}

#[test]
#[allow(clippy::too_many_lines)] // One end-to-end ceremony proves denial, initialization, persistence, and replay.
fn trust_provision_is_explicit_signed_and_required_before_lifecycle_authority() {
    let temp = fresh_temp("explicit-trust-provision");
    let project_root = temp.join("project");
    let artifact_root = project_root.join("artifacts");
    let state_root = project_root.join(".forge-method");
    let operator_root = temp.join("operator");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::create_dir_all(&operator_root).expect("create operator root");
    let (trust_policy, registry) = write_signed_supply_chain(&operator_root);

    let mut preflight = artifact_guard_preflight(DomainPackArtifactBinding {
        artifact_ref: RepoPath("unused.yaml".to_owned()),
        raw_sha256: digest(81_001),
        canonical_sha256: digest(81_002),
    });
    preflight
        .domain_pack_lifecycle_preflight
        .staged_artifacts
        .clear();
    let preflight_file = temp.join("preflight.yaml");
    fs::write(
        &preflight_file,
        yaml_serde::to_string(&preflight).expect("serialize preflight"),
    )
    .expect("write preflight");
    for name in ["resolution.yaml", "composition.yaml", "trust-input.yaml"] {
        fs::write(temp.join(name), "not: authoritative\n").expect("write inert input");
    }

    let stderr = lifecycle_preflight_command(
        &temp,
        &project_root,
        &artifact_root,
        &state_root,
        &preflight_file,
        &trust_policy,
        &registry,
    )
    .assert()
    .code(2)
    .get_output()
    .stderr
    .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains("operator registry anchor is not provisioned")
            && stderr.contains("trust-provision"),
        "lifecycle command did not fail closed on absent anchor: {stderr}"
    );

    let base_args = [
        "domain-pack",
        "trust-provision",
        "--operator-root",
        operator_root.to_str().expect("operator root utf8"),
        "--trust-policy-file",
        trust_policy.to_str().expect("policy path utf8"),
        "--registry-file",
        registry.to_str().expect("registry path utf8"),
        "--project-root",
        project_root.to_str().expect("project root utf8"),
        "--artifact-root",
        artifact_root.to_str().expect("artifact root utf8"),
        "--state-root",
        state_root.to_str().expect("state root utf8"),
    ];
    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args(base_args)
        .assert()
        .code(3)
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8_lossy(&stderr)
            .contains("--operator-acknowledge-trust-on-first-use I_UNDERSTAND_TRUST_ON_FIRST_USE"),
        "missing acknowledgement did not fail closed: {}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        !operator_root
            .join(".forge-domain-pack-registry-anchor.yaml")
            .exists(),
        "anchor was created without acknowledgement"
    );

    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args(base_args)
        .args([
            "--operator-acknowledge-trust-on-first-use",
            "I_UNDERSTAND_TRUST_ON_FIRST_USE",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: serde_json::Value =
        serde_json::from_slice(&output).expect("trust provision JSON envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["generation"], 1);
    assert_eq!(envelope["data"]["anchor_previously_present"], false);
    assert!(
        operator_root
            .join(".forge-domain-pack-registry-anchor.yaml")
            .is_file(),
        "explicit signed ceremony did not persist the anchor"
    );

    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args(base_args)
        .args([
            "--operator-acknowledge-trust-on-first-use",
            "I_UNDERSTAND_TRUST_ON_FIRST_USE",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: serde_json::Value =
        serde_json::from_slice(&output).expect("trust replay JSON envelope");
    assert_eq!(envelope["data"]["anchor_previously_present"], true);

    fs::remove_dir_all(temp).expect("cleanup explicit trust fixture");
}

#[test]
fn lifecycle_mutation_rejects_project_controlled_trust_roots() {
    let temp = fresh_temp("trust-root-boundary");
    let project_root = temp.join("project");
    let artifact_root = project_root.join("artifacts");
    let state_root = project_root.join(".forge-method");
    let operator_root = temp.join("operator");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::create_dir_all(&operator_root).expect("create operator root");
    for name in [
        "preflight.yaml",
        "resolution.yaml",
        "composition.yaml",
        "trust-input.yaml",
    ] {
        fs::write(temp.join(name), "not: authoritative\n").expect("write inert input");
    }
    let external_policy = operator_root.join("trust-policy.yaml");
    let external_registry = operator_root.join("registry.yaml");
    let project_policy = project_root.join("trust-policy.yaml");
    let project_registry = project_root.join("registry.yaml");
    for path in [
        &external_policy,
        &external_registry,
        &project_policy,
        &project_registry,
    ] {
        fs::write(path, "not: authoritative\n").expect("write trust-root probe");
    }

    for (policy, registry, label) in [
        (&project_policy, &external_registry, "operator trust policy"),
        (
            &external_policy,
            &project_registry,
            "signed supply-chain registry",
        ),
    ] {
        let stderr = lifecycle_preflight_command(
            &temp,
            &project_root,
            &artifact_root,
            &state_root,
            &temp.join("preflight.yaml"),
            policy,
            registry,
        )
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
        let stderr = String::from_utf8_lossy(&stderr);
        assert!(
            stderr.contains(label)
                && stderr.contains("operator-controlled")
                && stderr.contains("external to --project-root"),
            "unexpected trust-root rejection: {stderr}"
        );
    }

    let linked_operator = project_root.join("linked-operator-root");
    create_dir_link(&linked_operator, &operator_root);
    let stderr = lifecycle_preflight_command(
        &temp,
        &project_root,
        &artifact_root,
        &state_root,
        &temp.join("preflight.yaml"),
        &linked_operator.join("trust-policy.yaml"),
        &external_registry,
    )
    .assert()
    .failure()
    .get_output()
    .stderr
    .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains("operator trust policy") && stderr.contains("external to --project-root"),
        "unexpected linked trust-root rejection: {stderr}"
    );
    remove_dir_link(&linked_operator);

    fs::remove_dir_all(temp).expect("cleanup trust-root fixture");
}

#[test]
fn lifecycle_preflight_rejects_missing_and_tampered_staged_fixture_bytes() {
    let temp = fresh_temp("immutable-artifact-bytes");
    let project_root = temp.join("project");
    let artifact_root = project_root.join("artifacts");
    let state_root = project_root.join(".forge-method");
    let operator_root = temp.join("operator");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::create_dir_all(&operator_root).expect("create operator root");
    for name in ["resolution.yaml", "composition.yaml", "trust-input.yaml"] {
        fs::write(temp.join(name), "not: authoritative\n").expect("write inert input");
    }
    let trust_policy = operator_root.join("trust-policy.yaml");
    let registry = operator_root.join("registry.yaml");
    fs::write(&trust_policy, "not: authoritative\n").expect("write external policy");
    fs::write(&registry, "not: authoritative\n").expect("write external registry");

    let missing_binding = DomainPackArtifactBinding {
        artifact_ref: RepoPath("fixtures/missing.yaml".to_owned()),
        raw_sha256: digest(91_001),
        canonical_sha256: digest(91_002),
    };
    let preflight_file = temp.join("preflight.yaml");
    fs::write(
        &preflight_file,
        yaml_serde::to_string(&artifact_guard_preflight(missing_binding))
            .expect("serialize missing-artifact preflight"),
    )
    .expect("write missing-artifact preflight");
    let stderr = lifecycle_preflight_command(
        &temp,
        &project_root,
        &artifact_root,
        &state_root,
        &preflight_file,
        &trust_policy,
        &registry,
    )
    .assert()
    .failure()
    .get_output()
    .stderr
    .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains("fixtures/missing.yaml") && stderr.contains("cannot inspect artifact"),
        "unexpected missing-artifact rejection: {stderr}"
    );

    let artifact_ref = "fixtures/tampered.yaml";
    let artifact_path = artifact_root.join(artifact_ref);
    fs::create_dir_all(artifact_path.parent().expect("fixture parent"))
        .expect("create fixture parent");
    fs::write(&artifact_path, b"tampered bytes").expect("write tampered fixture");
    let expected_bytes = b"expected immutable bytes";
    let tampered_binding = DomainPackArtifactBinding {
        artifact_ref: RepoPath(artifact_ref.to_owned()),
        raw_sha256: format!("sha256:{:x}", Sha256::digest(expected_bytes)),
        canonical_sha256: digest(91_003),
    };
    fs::write(
        &preflight_file,
        yaml_serde::to_string(&artifact_guard_preflight(tampered_binding))
            .expect("serialize tampered-artifact preflight"),
    )
    .expect("write tampered-artifact preflight");
    let stderr = lifecycle_preflight_command(
        &temp,
        &project_root,
        &artifact_root,
        &state_root,
        &preflight_file,
        &trust_policy,
        &registry,
    )
    .assert()
    .failure()
    .get_output()
    .stderr
    .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains(artifact_ref) && stderr.contains("differs from raw_sha256 binding"),
        "unexpected tampered-artifact rejection: {stderr}"
    );

    fs::remove_dir_all(temp).expect("cleanup immutable-artifact fixture");
}
