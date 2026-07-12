use forge_core_contracts::{
    RepoPath, StableId, WorkflowGovernanceBundleDocument, WorkflowGovernanceReleaseIdentity,
    WorkflowGovernanceReleaseManifestDocument, WorkflowGovernanceReleaseRegistry,
    WorkflowGovernanceReleaseRegistryDocument, WorkflowGovernanceReleaseRegistryEntry,
    WorkflowReceiptCarryover, WorkflowReleasePredecessorReference,
    WorkflowReleaseRegistryAuthority, WorkflowReleaseRegistrySource, WorkflowRuntimeBundleIdentity,
    WorkflowRuntimeBundleReference, WORKFLOW_GOVERNANCE_RELEASE_REGISTRY_SCHEMA_VERSION,
};
use forge_core_decisions::{
    workflow_implicit_p5c_release_digest, workflow_policy_set_digest,
    workflow_release_manifest_digest, workflow_runtime_bundle_digest,
};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::path::{Path, PathBuf};

const LEGACY_BUNDLE_PATH: &str = "contracts/workflow-governance/golden-path-v0.yaml";
const FOUNDATION_BUNDLE_PATH: &str =
    "contracts/workflow-governance/runtime-release-foundation-v0.yaml";
const FOUNDATION_MANIFEST_PATH: &str =
    "contracts/migration/workflow-governance-release-foundation-v0.yaml";
const REGISTRY_PATH: &str = "contracts/migration/workflow-governance-release-registry-v0.yaml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

struct Artifact {
    path: &'static str,
    bytes: Vec<u8>,
}

struct RegistryDocumentInputs {
    lineage_id: StableId,
    genesis_release: WorkflowGovernanceReleaseIdentity,
    successor_release: WorkflowGovernanceReleaseIdentity,
    legacy_runtime: WorkflowRuntimeBundleIdentity,
    foundation_runtime: WorkflowRuntimeBundleIdentity,
    legacy_bundle_bytes: Vec<u8>,
    foundation_bundle_bytes: Vec<u8>,
    manifest_bytes: Vec<u8>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mode = parse_mode()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let artifacts = generate(&root)?;
    match mode {
        Mode::Write => write_artifacts(&root, &artifacts),
        Mode::Check => check_artifacts(&root, &artifacts),
    }
}

fn parse_mode() -> Result<Mode, Box<dyn Error>> {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [flag] if flag == "--write" => Ok(Mode::Write),
        [flag] if flag == "--check" => Ok(Mode::Check),
        _ => Err(error(
            "usage: cargo run -p forge-core-decisions --example generate_workflow_release_registry -- (--write|--check)",
        )),
    }
}

fn generate(root: &Path) -> Result<Vec<Artifact>, Box<dyn Error>> {
    let legacy_bundle_text = std::fs::read_to_string(root.join(LEGACY_BUNDLE_PATH))?;
    let legacy_bundle_bytes = legacy_bundle_text.as_bytes().to_vec();
    let legacy_bundle: WorkflowGovernanceBundleDocument =
        yaml_serde::from_str(&legacy_bundle_text)?;
    let policy_set_digest =
        workflow_policy_set_digest(&legacy_bundle.workflow_governance_bundle.policies)
            .map_err(error)?;

    let mut foundation_bundle = legacy_bundle.clone();
    foundation_bundle.workflow_governance_bundle.id =
        id("bundle.workflow-governance.release-foundation-v0");
    let foundation_bundle_bytes = yaml_bytes(&foundation_bundle)?;

    let legacy_runtime = runtime_identity(&legacy_bundle, &policy_set_digest)?;
    let foundation_runtime = runtime_identity(&foundation_bundle, &policy_set_digest)?;
    let lineage_id = id("workflow-governance.core");
    let genesis_release_id = id("workflow-governance.release.p5c-implicit-v0");
    let genesis_version = "0.0.0+p5c";
    let genesis_release = WorkflowGovernanceReleaseIdentity {
        lineage_id: lineage_id.clone(),
        release_id: genesis_release_id.clone(),
        release_version: genesis_version.to_owned(),
        release_digest: workflow_implicit_p5c_release_digest(
            &lineage_id,
            &genesis_release_id,
            genesis_version,
            &legacy_runtime,
        )
        .map_err(error)?,
    };

    let manifest_text = std::fs::read_to_string(root.join(FOUNDATION_MANIFEST_PATH))?;
    let manifest_bytes = manifest_text.as_bytes().to_vec();
    let manifest: WorkflowGovernanceReleaseManifestDocument = yaml_serde::from_str(&manifest_text)?;
    let successor_release_digest = workflow_release_manifest_digest(&manifest).map_err(error)?;
    let subject = manifest.workflow_governance_release_manifest;
    let successor_release = WorkflowGovernanceReleaseIdentity {
        lineage_id: subject.lineage_id,
        release_id: subject.release_id,
        release_version: subject.release_version,
        release_digest: successor_release_digest,
    };
    let registry = registry_document(RegistryDocumentInputs {
        lineage_id,
        genesis_release,
        successor_release,
        legacy_runtime,
        foundation_runtime,
        legacy_bundle_bytes: legacy_bundle_bytes.clone(),
        foundation_bundle_bytes: foundation_bundle_bytes.clone(),
        manifest_bytes,
    });
    Ok(vec![
        Artifact {
            path: FOUNDATION_BUNDLE_PATH,
            bytes: foundation_bundle_bytes,
        },
        Artifact {
            path: REGISTRY_PATH,
            bytes: yaml_bytes(&registry)?,
        },
    ])
}

fn registry_document(inputs: RegistryDocumentInputs) -> WorkflowGovernanceReleaseRegistryDocument {
    let RegistryDocumentInputs {
        lineage_id,
        genesis_release,
        successor_release,
        legacy_runtime,
        foundation_runtime,
        legacy_bundle_bytes,
        foundation_bundle_bytes,
        manifest_bytes,
    } = inputs;
    WorkflowGovernanceReleaseRegistryDocument {
        schema_version: WORKFLOW_GOVERNANCE_RELEASE_REGISTRY_SCHEMA_VERSION.to_owned(),
        workflow_governance_release_registry: WorkflowGovernanceReleaseRegistry {
            registry_id: id("workflow-governance.registry.foundation-v0"),
            registry_version: "0.1.0".to_owned(),
            lineage_id,
            default_successor_release_id: successor_release.release_id.clone(),
            releases: vec![
                WorkflowGovernanceReleaseRegistryEntry {
                    release: genesis_release.clone(),
                    runtime_bundle: runtime_reference(
                        legacy_runtime,
                        LEGACY_BUNDLE_PATH,
                        &legacy_bundle_bytes,
                    ),
                    predecessor: None,
                    source: WorkflowReleaseRegistrySource::ImplicitP5cGenesis,
                    receipt_carryover: WorkflowReceiptCarryover::NotApplicable,
                    authority: WorkflowReleaseRegistryAuthority::CandidateOnly,
                },
                WorkflowGovernanceReleaseRegistryEntry {
                    release: successor_release,
                    runtime_bundle: runtime_reference(
                        foundation_runtime,
                        FOUNDATION_BUNDLE_PATH,
                        &foundation_bundle_bytes,
                    ),
                    predecessor: Some(WorkflowReleasePredecessorReference {
                        release_id: genesis_release.release_id,
                        release_digest: genesis_release.release_digest,
                    }),
                    source: WorkflowReleaseRegistrySource::EmbeddedManifest {
                        embedded_ref: RepoPath(FOUNDATION_MANIFEST_PATH.to_owned()),
                        expected_digest: sha256(&manifest_bytes),
                    },
                    receipt_carryover: WorkflowReceiptCarryover::PreservePolicyEquivalent,
                    authority: WorkflowReleaseRegistryAuthority::CandidateOnly,
                },
            ],
        },
    }
}

fn runtime_identity(
    bundle: &WorkflowGovernanceBundleDocument,
    policy_set_digest: &str,
) -> Result<WorkflowRuntimeBundleIdentity, Box<dyn Error>> {
    Ok(WorkflowRuntimeBundleIdentity {
        bundle_id: bundle.workflow_governance_bundle.id.clone(),
        bundle_digest: workflow_runtime_bundle_digest(bundle).map_err(error)?,
        policy_set_digest: policy_set_digest.to_owned(),
    })
}

fn runtime_reference(
    identity: WorkflowRuntimeBundleIdentity,
    path: &str,
    bytes: &[u8],
) -> WorkflowRuntimeBundleReference {
    WorkflowRuntimeBundleReference {
        identity,
        embedded_ref: RepoPath(path.to_owned()),
        expected_digest: sha256(bytes),
    }
}

fn yaml_bytes<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut text = yaml_serde::to_string(value)?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text.into_bytes())
}

fn write_artifacts(root: &Path, artifacts: &[Artifact]) -> Result<(), Box<dyn Error>> {
    for artifact in artifacts {
        let path = root.join(artifact.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &artifact.bytes)?;
        println!("wrote {}", artifact.path);
    }
    Ok(())
}

fn check_artifacts(root: &Path, artifacts: &[Artifact]) -> Result<(), Box<dyn Error>> {
    for artifact in artifacts {
        let found = std::fs::read(root.join(artifact.path))?;
        if found != artifact.bytes {
            return Err(error(format!("{} has byte drift", artifact.path)));
        }
        println!("checked {}", artifact.path);
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn error(message: impl Into<String>) -> Box<dyn Error> {
    message.into().into()
}
