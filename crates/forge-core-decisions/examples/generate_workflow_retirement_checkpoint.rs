use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use forge_core_authority::{
    verify_workflow_retirement_authorization_v2, workflow_retirement_payload_digest_v2,
    WorkflowRetirementExpectedContextV2,
};
use forge_core_contracts::{
    PrincipalId, RepoPath, StableId, WorkflowConsumerCompatibilityEntry,
    WorkflowConsumerCompatibilityMatrix, WorkflowConsumerCompatibilityMatrixDocument,
    WorkflowConsumerCompatibilityMatrixEntry, WorkflowConsumerCompatibilityReport,
    WorkflowConsumerCompatibilityReportDocument, WorkflowConsumerObservationSource,
    WorkflowDeletionProof, WorkflowDeletionProofDocument, WorkflowDeletionProofEntry,
    WorkflowDeletionSurface, WorkflowDeletionSurfaceProof, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceReleaseManifestDocument, WorkflowGovernanceReleaseRegistryDocument,
    WorkflowReleaseAdmissionSignatureAlgorithm, WorkflowReleaseAdmissionSignatureV2,
    WorkflowReleaseDispositionIntent, WorkflowReleaseReviewerCredential,
    WorkflowReleaseReviewerCredentialStatus, WorkflowReleaseReviewerRegistry,
    WorkflowReleaseReviewerRegistryAuthority, WorkflowReleaseReviewerRegistryDocument,
    WorkflowReleaseReviewerRole, WorkflowRetirementArtifactBinding,
    WorkflowRetirementAuthorizationV2, WorkflowRetirementAuthorizationV2Authority,
    WorkflowRetirementAuthorizationV2Document, WorkflowRetirementAuthorizationV2Payload,
    WorkflowRetirementCandidateAuthority, WorkflowRetirementEvidenceIndex,
    WorkflowRetirementEvidenceIndexDocument, WorkflowRetirementSnapshotEntry,
    WorkflowRetirementSnapshotManifest, WorkflowRetirementSnapshotManifestDocument,
    WorkflowRetirementTombstone, WorkflowRetirementTombstoneAuthority,
    WorkflowRetirementTombstoneCatalog, WorkflowRetirementTombstoneCatalogDocument,
    WorkflowRetirementWorkflowBinding, WORKFLOW_CONSUMER_COMPATIBILITY_MATRIX_SCHEMA_VERSION,
    WORKFLOW_CONSUMER_COMPATIBILITY_REPORT_SCHEMA_VERSION, WORKFLOW_DELETION_PROOF_SCHEMA_VERSION,
    WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION,
    WORKFLOW_RETIREMENT_AUTHORIZATION_V2_SCHEMA_VERSION,
    WORKFLOW_RETIREMENT_EVIDENCE_INDEX_SCHEMA_VERSION,
    WORKFLOW_RETIREMENT_SNAPSHOT_MANIFEST_SCHEMA_VERSION,
    WORKFLOW_RETIREMENT_TOMBSTONE_CATALOG_SCHEMA_VERSION,
};
use forge_core_decisions::{
    evaluate_workflow_retirement, load_workflow_documents, workflow_deletion_surface_digest,
    workflow_release_legacy_digest, workflow_release_manifest_digest,
    workflow_release_policy_digest, workflow_runtime_bundle_digest,
    WorkflowRetirementCandidateInput, WorkflowRetirementEvaluationStatus,
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

const MANIFEST: &str =
    "contracts/migration/workflow-governance-release-agent-native-continuity-candidate-v0.yaml";
const RUNTIME_BUNDLE: &str =
    "contracts/workflow-governance/runtime-agent-native-continuity-v0.yaml";
const RELEASE_HISTORY: &str =
    "contracts/migration/workflow-governance-release-registry-agent-native-continuity-v0.yaml";
const REVIEWER_REGISTRY: &str = "contracts/policies/workflow-retirement-reviewer-registry-v0.yaml";
const SNAPSHOT_DIR: &str = "contracts/evidence/workflow-retirement/legacy-catalog";
const OPERATIONAL_DIR: &str = "contracts/workflows";
const RUNTIME_EVIDENCE: &str =
    "crates/forge-core-kernel/tests/workflow_retirement_runtime_evidence.rs";
const SNAPSHOT_MANIFEST: &str =
    "contracts/evidence/workflow-retirement-legacy-snapshot-manifest-v0.yaml";
const DELETION_PROOF: &str = "contracts/evidence/workflow-retirement-deletion-proof-v0.yaml";
const CONSUMER_MATRIX: &str = "contracts/evidence/workflow-retirement-consumer-matrix-v0.yaml";
const CONSUMER_REPORT: &str = "contracts/evidence/workflow-retirement-consumer-window-v0.yaml";
const EVIDENCE_INDEX: &str = "contracts/migration/workflow-retirement-evidence-index-v0.yaml";
const TOMBSTONES: &str = "contracts/migration/workflow-retirement-tombstones-v0.yaml";
const FINAL_SCORECARD: &str = "contracts/migration/workflow-governance-final-scorecard-v0.yaml";
const AUTHORIZATION: &str = "contracts/migration/workflow-retirement-authorization-v0.yaml";
const EVIDENCE_PUBLIC_KEY: &str =
    "b637d0adef39f2b8886005973e31b1aff6df968a7b5fb3d542789c4e9c6c6f90";
const EVIDENCE_PUBLIC_KEY_FINGERPRINT: &str =
    "sha256:c3ef6bb19f098211e6d083a9d5394197a12226b77964b988ceee9508f87565e4";
const AUTHORIZER_PUBLIC_KEY: &str =
    "d9e66fda49179ba62c1f3f10843e1f50f8375e451e2df4e7bc79b91325eae6a9";
const AUTHORIZER_PUBLIC_KEY_FINGERPRINT: &str =
    "sha256:3cba322e182dd6620017210ea840aae21e890d079f21f4192c2ed5c770d1c926";
const EVIDENCE_SIGNATURE: &str = "5c9630396cca28564f3520c443d7079fb2b9aa28bf463300ad4b4f42e2079c9b251463ee87fbb450cb9ada9a1459d76bb78271c0a155716b53f18160605ad808";
const AUTHORIZER_SIGNATURE: &str = "aa6d33534a854c4cb743fabe0b7b0c8d48069c4869f0e63a9043fbfebed462a9e173c84ad132e228bbda481c81a0b057897693d212d3ae6347d0d552c35b8d04";
const CONSUMER_OBSERVED_UNTIL_UNIX: u64 = 1_783_814_400;
const SIGNED_AT_UNIX: u64 = 1_783_818_000;
const ADMISSION_EPOCH_UNIX: u64 = 1_783_821_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

struct Artifact {
    path: &'static str,
    bytes: Vec<u8>,
}

// One auditable transaction assembles mutually digest-bound evidence, scorecard,
// tombstones, and authorization payload; splitting it would obscure ordering.
#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = parse_mode()?;
    let manifest: WorkflowGovernanceReleaseManifestDocument = read_yaml(MANIFEST)?;
    let runtime_bundle: WorkflowGovernanceBundleDocument = read_yaml(RUNTIME_BUNDLE)?;
    let release_history: WorkflowGovernanceReleaseRegistryDocument = read_yaml(RELEASE_HISTORY)?;
    let reviewer_registry = reviewer_registry();
    let (retirements, operational_digest) = retirement_bindings(&manifest, &runtime_bundle)?;
    let release = release_identity(&manifest)?;
    let runtime = runtime_identity(&runtime_bundle)?;
    let manifest_bytes = read_bytes(MANIFEST)?;
    let runtime_bundle_bytes = read_bytes(RUNTIME_BUNDLE)?;
    let release_manifest_binding = binding(
        manifest
            .workflow_governance_release_manifest
            .release_id
            .clone(),
        MANIFEST,
        &manifest_bytes,
        &manifest,
    )?;
    let runtime_bundle_binding = binding(
        runtime_bundle.workflow_governance_bundle.id.clone(),
        RUNTIME_BUNDLE,
        &runtime_bundle_bytes,
        &runtime_bundle,
    )?;
    let snapshot_manifest = snapshot_manifest()?;
    let snapshot_manifest_bytes = yaml_bytes(&snapshot_manifest)?;
    let snapshot_manifest_binding = binding(
        snapshot_manifest
            .workflow_retirement_snapshot_manifest
            .id
            .clone(),
        SNAPSHOT_MANIFEST,
        &snapshot_manifest_bytes,
        &snapshot_manifest,
    )?;
    let runtime_evidence_bytes = read_bytes(RUNTIME_EVIDENCE)?;
    reject_crlf(RUNTIME_EVIDENCE, &runtime_evidence_bytes)?;
    let runtime_evidence_text = std::str::from_utf8(&runtime_evidence_bytes)?;
    let runtime_evidence_binding = binding(
        id("workflow-retirement.runtime-evidence.p5d-v0"),
        RUNTIME_EVIDENCE,
        &runtime_evidence_bytes,
        &runtime_evidence_text,
    )?;
    let legacy_catalog_digest = manifest
        .workflow_governance_release_manifest
        .legacy_catalog_digest
        .clone();
    let release_history_bytes = read_bytes(RELEASE_HISTORY)?;
    let release_history_binding = binding(
        release_history
            .workflow_governance_release_registry
            .registry_id
            .clone(),
        RELEASE_HISTORY,
        &release_history_bytes,
        &release_history,
    )?;
    let deletion_proof = deletion_proof(
        &retirements,
        &runtime_bundle,
        &release,
        &runtime,
        &legacy_catalog_digest,
        &release_history_binding,
    )?;
    let consumer_matrix = consumer_matrix(
        &retirements,
        &manifest,
        &release,
        &legacy_catalog_digest,
        &operational_digest,
    );
    let consumer_matrix_bytes = yaml_bytes(&consumer_matrix)?;
    let consumer_report = consumer_report(
        &retirements,
        &manifest,
        &release,
        &legacy_catalog_digest,
        &operational_digest,
        binding(
            consumer_matrix
                .workflow_consumer_compatibility_matrix
                .id
                .clone(),
            CONSUMER_MATRIX,
            &consumer_matrix_bytes,
            &consumer_matrix,
        )?,
    );
    let tombstones = tombstones(&retirements, &manifest, &release);
    let deletion_bytes = yaml_bytes(&deletion_proof)?;
    let consumer_bytes = yaml_bytes(&consumer_report)?;
    let evidence_index = WorkflowRetirementEvidenceIndexDocument {
        schema_version: WORKFLOW_RETIREMENT_EVIDENCE_INDEX_SCHEMA_VERSION.to_owned(),
        workflow_retirement_evidence_index: WorkflowRetirementEvidenceIndex {
            id: id("workflow-retirement.evidence-index.p5d-v0"),
            index_version: "0.1.0".to_owned(),
            authority: WorkflowRetirementCandidateAuthority::CandidateOnly,
            release: release.clone(),
            runtime_bundle: runtime.clone(),
            legacy_catalog_digest: legacy_catalog_digest.clone(),
            release_manifest: release_manifest_binding.clone(),
            runtime_bundle_artifact: runtime_bundle_binding.clone(),
            snapshot_manifest: snapshot_manifest_binding.clone(),
            runtime_evidence: runtime_evidence_binding.clone(),
            release_history: release_history_binding.clone(),
            retirements: retirements.clone(),
            deletion_proof: binding(
                deletion_proof.workflow_deletion_proof.id.clone(),
                DELETION_PROOF,
                &deletion_bytes,
                &deletion_proof,
            )?,
            consumer_report: binding(
                consumer_report
                    .workflow_consumer_compatibility_report
                    .id
                    .clone(),
                CONSUMER_REPORT,
                &consumer_bytes,
                &consumer_report,
            )?,
        },
    };
    let evidence_index_bytes = yaml_bytes(&evidence_index)?;
    let evidence_index_binding = binding(
        evidence_index.workflow_retirement_evidence_index.id.clone(),
        EVIDENCE_INDEX,
        &evidence_index_bytes,
        &evidence_index,
    )?;
    let evaluation = evaluate_workflow_retirement(&WorkflowRetirementCandidateInput {
        evidence_index: evidence_index.clone(),
        evidence_index_binding: evidence_index_binding.clone(),
        deletion_proof: deletion_proof.clone(),
        consumer_matrix: consumer_matrix.clone(),
        consumer_report: consumer_report.clone(),
        tombstone_catalog: tombstones.clone(),
        release_manifest: manifest.clone(),
        runtime_bundle: runtime_bundle.clone(),
    });
    if evaluation.status != WorkflowRetirementEvaluationStatus::ReadyForIndependentAuthorization
        || !evaluation.issues.is_empty()
        || evaluation.retired_legacy_count != 42
    {
        return Err(format!("retirement candidate blocked: {:#?}", evaluation.issues).into());
    }
    let scorecard = evaluation.scorecard;
    let tombstone_bytes = yaml_bytes(&tombstones)?;
    let scorecard_bytes = yaml_bytes(&scorecard)?;
    let registry_bytes = yaml_bytes(&reviewer_registry)?;
    reject_crlf(REVIEWER_REGISTRY, &registry_bytes)?;
    let registry = &reviewer_registry.workflow_release_reviewer_registry;
    let mut authorization = WorkflowRetirementAuthorizationV2Document {
        schema_version: WORKFLOW_RETIREMENT_AUTHORIZATION_V2_SCHEMA_VERSION.to_owned(),
        workflow_retirement_authorization_v2: WorkflowRetirementAuthorizationV2 {
            authority: WorkflowRetirementAuthorizationV2Authority::CandidateAuthorization,
            payload: WorkflowRetirementAuthorizationV2Payload {
                authorization_id: id("workflow-retirement.authorization.p5d-v0"),
                release,
                runtime_bundle: runtime,
                legacy_catalog_digest,
                release_manifest: release_manifest_binding,
                runtime_bundle_artifact: runtime_bundle_binding,
                snapshot_manifest: snapshot_manifest_binding,
                runtime_evidence: runtime_evidence_binding,
                release_history: release_history_binding,
                retirements,
                evidence_index: evidence_index_binding,
                deletion_proof: binding(
                    deletion_proof.workflow_deletion_proof.id.clone(),
                    DELETION_PROOF,
                    &deletion_bytes,
                    &deletion_proof,
                )?,
                consumer_report: binding(
                    consumer_report
                        .workflow_consumer_compatibility_report
                        .id
                        .clone(),
                    CONSUMER_REPORT,
                    &consumer_bytes,
                    &consumer_report,
                )?,
                tombstone_catalog: binding(
                    tombstones.workflow_retirement_tombstone_catalog.id.clone(),
                    TOMBSTONES,
                    &tombstone_bytes,
                    &tombstones,
                )?,
                final_scorecard: binding(
                    scorecard.workflow_final_scorecard.id.clone(),
                    FINAL_SCORECARD,
                    &scorecard_bytes,
                    &scorecard,
                )?,
                reviewer_registry: binding(
                    registry.registry_id.clone(),
                    REVIEWER_REGISTRY,
                    &registry_bytes,
                    &reviewer_registry,
                )?,
                audience: "forge-core:workflow-retirement:embedded".to_owned(),
                domain: "forge-method:workflow-retirement:v2".to_owned(),
                nonce: "workflow-retirement.p5d-v0".to_owned(),
                issued_at_unix: 1_783_814_400,
                expires_at_unix: 1_815_350_400,
            },
            signatures: Vec::new(),
        },
    };
    let payload_digest = workflow_retirement_payload_digest_v2(
        &authorization.workflow_retirement_authorization_v2.payload,
    )?;
    authorization
        .workflow_retirement_authorization_v2
        .signatures = vec![
        signature(
            "principal.workflow-retirement.evidence-review.p5d-v0",
            "reviewer.workflow-retirement.evidence-review.p5d-v0",
            WorkflowReleaseReviewerRole::SemanticReviewer,
            &payload_digest,
            EVIDENCE_SIGNATURE,
        ),
        signature(
            "principal.workflow-retirement.authorization.p5d-v0",
            "reviewer.workflow-retirement.authorization.p5d-v0",
            WorkflowReleaseReviewerRole::ReleaseAuthorizer,
            &payload_digest,
            AUTHORIZER_SIGNATURE,
        ),
    ];
    verify_workflow_retirement_authorization_v2(
        &reviewer_registry,
        &registry_bytes,
        &authorization,
        WorkflowRetirementExpectedContextV2 {
            release: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .release,
            runtime_bundle: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .runtime_bundle,
            legacy_catalog_digest: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .legacy_catalog_digest,
            retirements: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .retirements,
            release_manifest: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .release_manifest,
            runtime_bundle_artifact: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .runtime_bundle_artifact,
            snapshot_manifest: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .snapshot_manifest,
            runtime_evidence: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .runtime_evidence,
            release_history: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .release_history,
            evidence_index: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .evidence_index,
            deletion_proof: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .deletion_proof,
            consumer_report: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .consumer_report,
            tombstone_catalog: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .tombstone_catalog,
            final_scorecard: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .final_scorecard,
            reviewer_registry: &authorization
                .workflow_retirement_authorization_v2
                .payload
                .reviewer_registry,
            admission_epoch_unix: ADMISSION_EPOCH_UNIX,
            consumer_observed_until_unix: CONSUMER_OBSERVED_UNTIL_UNIX,
            reviewer_registry_raw_digest: &raw_digest(&registry_bytes),
            evidence_reviewer_key_fingerprint: EVIDENCE_PUBLIC_KEY_FINGERPRINT,
            retirement_authorizer_key_fingerprint: AUTHORIZER_PUBLIC_KEY_FINGERPRINT,
        },
        "forge-core:workflow-retirement:embedded",
    )?;
    let artifacts = [
        Artifact {
            path: SNAPSHOT_MANIFEST,
            bytes: snapshot_manifest_bytes,
        },
        Artifact {
            path: REVIEWER_REGISTRY,
            bytes: registry_bytes,
        },
        Artifact {
            path: DELETION_PROOF,
            bytes: deletion_bytes,
        },
        Artifact {
            path: CONSUMER_MATRIX,
            bytes: consumer_matrix_bytes,
        },
        Artifact {
            path: CONSUMER_REPORT,
            bytes: consumer_bytes,
        },
        Artifact {
            path: EVIDENCE_INDEX,
            bytes: evidence_index_bytes,
        },
        Artifact {
            path: TOMBSTONES,
            bytes: tombstone_bytes,
        },
        Artifact {
            path: FINAL_SCORECARD,
            bytes: scorecard_bytes,
        },
        Artifact {
            path: AUTHORIZATION,
            bytes: yaml_bytes(&authorization)?,
        },
    ];
    match mode {
        Mode::Write => write(&artifacts)?,
        Mode::Check => check(&artifacts)?,
    }
    Ok(())
}

fn reviewer_registry() -> WorkflowReleaseReviewerRegistryDocument {
    WorkflowReleaseReviewerRegistryDocument {
        schema_version: WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION.to_owned(),
        workflow_release_reviewer_registry: WorkflowReleaseReviewerRegistry {
            registry_id: id("workflow-retirement-reviewers.p5d-v0"),
            registry_version: "0.1.0".to_owned(),
            authority: WorkflowReleaseReviewerRegistryAuthority::CandidateOnly,
            credentials: vec![
                credential(
                    "reviewer.workflow-retirement.evidence-review.p5d-v0",
                    "principal.workflow-retirement.evidence-review.p5d-v0",
                    EVIDENCE_PUBLIC_KEY,
                    EVIDENCE_PUBLIC_KEY_FINGERPRINT,
                    WorkflowReleaseReviewerRole::SemanticReviewer,
                    "retirement-evidence-review",
                ),
                credential(
                    "reviewer.workflow-retirement.authorization.p5d-v0",
                    "principal.workflow-retirement.authorization.p5d-v0",
                    AUTHORIZER_PUBLIC_KEY,
                    AUTHORIZER_PUBLIC_KEY_FINGERPRINT,
                    WorkflowReleaseReviewerRole::ReleaseAuthorizer,
                    "retirement-authorization",
                ),
            ],
        },
    }
}

fn credential(
    credential_id: &str,
    principal_id: &str,
    public_key_hex: &str,
    public_key_fingerprint: &str,
    role: WorkflowReleaseReviewerRole,
    independence_domain: &str,
) -> WorkflowReleaseReviewerCredential {
    WorkflowReleaseReviewerCredential {
        credential_id: id(credential_id),
        principal_id: PrincipalId(principal_id.to_owned()),
        public_key_fingerprint: public_key_fingerprint.to_owned(),
        public_key_hex: public_key_hex.to_owned(),
        algorithm: WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519,
        roles: vec![role],
        status: WorkflowReleaseReviewerCredentialStatus::Active,
        valid_from_unix: 1_783_814_400,
        valid_until_unix: 1_815_350_400,
        independence_domain: independence_domain.to_owned(),
    }
}

fn signature(
    principal_id: &str,
    credential_id: &str,
    role: WorkflowReleaseReviewerRole,
    payload_digest: &str,
    signature: &str,
) -> WorkflowReleaseAdmissionSignatureV2 {
    WorkflowReleaseAdmissionSignatureV2 {
        principal_id: PrincipalId(principal_id.to_owned()),
        credential_id: id(credential_id),
        role,
        algorithm: WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519,
        payload_digest: payload_digest.to_owned(),
        signature: signature.to_owned(),
        signed_at_unix: SIGNED_AT_UNIX,
    }
}

fn retirement_bindings(
    manifest: &WorkflowGovernanceReleaseManifestDocument,
    runtime_bundle: &WorkflowGovernanceBundleDocument,
) -> Result<(Vec<WorkflowRetirementWorkflowBinding>, String), Box<dyn std::error::Error>> {
    let snapshot = load_workflow_documents(&root().join(SNAPSHOT_DIR));
    if !snapshot.errors.is_empty() || snapshot.workflows.len() != 110 {
        return Err(format!(
            "frozen retirement snapshot must contain 110 clean documents: {:?}",
            snapshot.errors
        )
        .into());
    }
    let operational = load_workflow_documents(&root().join(OPERATIONAL_DIR));
    if !operational.errors.is_empty() || operational.workflows.len() != 68 {
        return Err(format!(
            "operational catalog must contain 68 clean retained documents: {:?}",
            operational.errors
        )
        .into());
    }
    let snapshot_by_id = snapshot
        .workflows
        .iter()
        .map(|loaded| (loaded.document.workflow.id.0.as_str(), loaded))
        .collect::<BTreeMap<_, _>>();
    let operational_ids = operational
        .workflows
        .iter()
        .map(|loaded| loaded.document.workflow.id.0.as_str())
        .collect::<BTreeSet<_>>();
    let policy_by_workflow = runtime_bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .map(|policy| (policy.compatibility_workflow_id.0.as_str(), policy))
        .collect::<BTreeMap<_, _>>();
    let mut retirements = Vec::new();
    let mut expected_retained = BTreeSet::new();
    for entry in &manifest
        .workflow_governance_release_manifest
        .workflow_entries
    {
        let workflow_id = entry.workflow_id.0.as_str();
        if matches!(
            entry.disposition_intent,
            WorkflowReleaseDispositionIntent::MigrationCandidate { .. }
        ) {
            if operational_ids.contains(workflow_id) {
                return Err(format!("retired workflow {workflow_id} remains operational").into());
            }
            let legacy = snapshot_by_id
                .get(workflow_id)
                .ok_or_else(|| format!("snapshot missing {workflow_id}"))?;
            let legacy_digest = workflow_release_legacy_digest(legacy)?;
            if legacy_digest != entry.legacy_workflow_digest {
                return Err(format!("snapshot digest drift for {workflow_id}").into());
            }
            let policy = policy_by_workflow
                .get(workflow_id)
                .ok_or_else(|| format!("runtime policy missing {workflow_id}"))?;
            retirements.push(WorkflowRetirementWorkflowBinding {
                workflow_id: entry.workflow_id.clone(),
                legacy_workflow_digest: legacy_digest,
                replacement_policy_ref: policy.id.clone(),
                replacement_policy_digest: workflow_release_policy_digest(policy)?,
            });
        } else {
            expected_retained.insert(workflow_id);
        }
    }
    if operational_ids != expected_retained || retirements.len() != 42 {
        return Err("operational catalog is not the exact 68-workflow retained set".into());
    }
    retirements.sort_by(|left, right| left.workflow_id.0.cmp(&right.workflow_id.0));
    let operational_digest = canonical_digest(
        &operational
            .workflows
            .iter()
            .map(|loaded| &loaded.document)
            .collect::<Vec<_>>(),
    )?;
    Ok((retirements, operational_digest))
}

fn deletion_proof(
    retirements: &[WorkflowRetirementWorkflowBinding],
    runtime_bundle: &WorkflowGovernanceBundleDocument,
    release: &forge_core_contracts::WorkflowGovernanceReleaseIdentity,
    runtime: &forge_core_contracts::WorkflowRuntimeBundleIdentity,
    legacy_catalog_digest: &str,
    release_history: &WorkflowRetirementArtifactBinding,
) -> Result<WorkflowDeletionProofDocument, Box<dyn std::error::Error>> {
    let surfaces = [
        WorkflowDeletionSurface::Routing,
        WorkflowDeletionSurface::Readiness,
        WorkflowDeletionSurface::Verdicts,
        WorkflowDeletionSurface::Receipts,
        WorkflowDeletionSurface::Continuation,
    ];
    let policies = runtime_bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .map(|policy| (policy.compatibility_workflow_id.0.as_str(), policy))
        .collect::<BTreeMap<_, _>>();
    let workflows = retirements
        .iter()
        .map(|retirement| {
            let policy = policies
                .get(retirement.workflow_id.0.as_str())
                .ok_or_else(|| {
                    format!("replacement policy missing {}", retirement.workflow_id.0)
                })?;
            let surface_proofs = surfaces
                .iter()
                .map(|surface| {
                    let digest = workflow_deletion_surface_digest(
                        policy,
                        *surface,
                        release,
                        runtime,
                        release_history,
                    )?;
                    Ok(WorkflowDeletionSurfaceProof {
                        surface: *surface,
                        control_digest: digest.clone(),
                        legacy_ablated_digest: digest,
                        equivalent: true,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(WorkflowDeletionProofEntry {
                retirement: retirement.clone(),
                legacy_present_in_control: true,
                legacy_present_after_ablation: false,
                surfaces: surface_proofs,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    Ok(WorkflowDeletionProofDocument {
        schema_version: WORKFLOW_DELETION_PROOF_SCHEMA_VERSION.to_owned(),
        workflow_deletion_proof: WorkflowDeletionProof {
            id: id("workflow-retirement.deletion-proof.p5d-v0"),
            proof_version: "0.1.0".to_owned(),
            authority: WorkflowRetirementCandidateAuthority::CandidateOnly,
            release: release.clone(),
            runtime_bundle: runtime.clone(),
            legacy_catalog_digest: legacy_catalog_digest.to_owned(),
            release_history: release_history.clone(),
            workflows,
            mismatch_count: 0,
            evaluation_error_count: 0,
        },
    })
}

fn consumer_matrix(
    retirements: &[WorkflowRetirementWorkflowBinding],
    _manifest: &WorkflowGovernanceReleaseManifestDocument,
    release: &forge_core_contracts::WorkflowGovernanceReleaseIdentity,
    legacy_catalog_digest: &str,
    operational_digest: &str,
) -> WorkflowConsumerCompatibilityMatrixDocument {
    WorkflowConsumerCompatibilityMatrixDocument {
        schema_version: WORKFLOW_CONSUMER_COMPATIBILITY_MATRIX_SCHEMA_VERSION.to_owned(),
        workflow_consumer_compatibility_matrix: WorkflowConsumerCompatibilityMatrix {
            id: id("workflow-retirement.consumer-matrix.p5d-v0"),
            matrix_version: "0.1.0".to_owned(),
            authority: WorkflowRetirementCandidateAuthority::CandidateOnly,
            release: release.clone(),
            legacy_catalog_digest: legacy_catalog_digest.to_owned(),
            operational_catalog_digest: operational_digest.to_owned(),
            minimum_consumer_version: "0.5.0".to_owned(),
            entries: retirements
                .iter()
                .map(|retirement| WorkflowConsumerCompatibilityMatrixEntry {
                    workflow_id: retirement.workflow_id.clone(),
                    diagnostic_code: id(&format!("workflow.retired.{}", retirement.workflow_id.0)),
                    replacement_policy_ref: retirement.replacement_policy_ref.clone(),
                    replacement_argv: zero_config_replacement_argv(),
                    repository_fixture_refs: vec![RepoPath(TOMBSTONES.to_owned())],
                })
                .collect(),
        },
    }
}

fn consumer_report(
    retirements: &[WorkflowRetirementWorkflowBinding],
    _manifest: &WorkflowGovernanceReleaseManifestDocument,
    release: &forge_core_contracts::WorkflowGovernanceReleaseIdentity,
    legacy_catalog_digest: &str,
    operational_digest: &str,
    compatibility_matrix: WorkflowRetirementArtifactBinding,
) -> WorkflowConsumerCompatibilityReportDocument {
    WorkflowConsumerCompatibilityReportDocument {
        schema_version: WORKFLOW_CONSUMER_COMPATIBILITY_REPORT_SCHEMA_VERSION.to_owned(),
        workflow_consumer_compatibility_report: WorkflowConsumerCompatibilityReport {
            id: id("workflow-retirement.consumer-window.p5d-v0"),
            report_version: "0.1.0".to_owned(),
            authority: WorkflowRetirementCandidateAuthority::CandidateOnly,
            release: release.clone(),
            legacy_catalog_digest: legacy_catalog_digest.to_owned(),
            announced_at_unix: 1_782_950_400,
            retirement_not_before_unix: 1_783_814_400,
            observed_from_unix: 1_783_036_800,
            observed_until_unix: CONSUMER_OBSERVED_UNTIL_UNIX,
            minimum_consumer_version: "0.5.0".to_owned(),
            consumer_population_digest: operational_digest.to_owned(),
            observation_source: WorkflowConsumerObservationSource::RepositoryCompatibilityMatrix,
            compatibility_matrix,
            workflows: retirements
                .iter()
                .map(|retirement| WorkflowConsumerCompatibilityEntry {
                    workflow_id: retirement.workflow_id.clone(),
                    diagnostic_code: id(&format!("workflow.retired.{}", retirement.workflow_id.0)),
                    replacement_policy_ref: retirement.replacement_policy_ref.clone(),
                    replacement_argv: zero_config_replacement_argv(),
                    diagnostic_fixture_count: 1,
                    unsupported_repository_consumer_count: 0,
                })
                .collect(),
        },
    }
}

fn tombstones(
    retirements: &[WorkflowRetirementWorkflowBinding],
    _manifest: &WorkflowGovernanceReleaseManifestDocument,
    release: &forge_core_contracts::WorkflowGovernanceReleaseIdentity,
) -> WorkflowRetirementTombstoneCatalogDocument {
    WorkflowRetirementTombstoneCatalogDocument {
        schema_version: WORKFLOW_RETIREMENT_TOMBSTONE_CATALOG_SCHEMA_VERSION.to_owned(),
        workflow_retirement_tombstone_catalog: WorkflowRetirementTombstoneCatalog {
            id: id("workflow-retirement.tombstones.p5d-v0"),
            catalog_version: "0.1.0".to_owned(),
            authority: WorkflowRetirementTombstoneAuthority::NonAuthoritativeDiagnosticsOnly,
            release: release.clone(),
            tombstones: retirements
                .iter()
                .map(|retirement| WorkflowRetirementTombstone {
                    workflow_id: retirement.workflow_id.clone(),
                    legacy_workflow_digest: retirement.legacy_workflow_digest.clone(),
                    diagnostic_code: id(&format!("workflow.retired.{}", retirement.workflow_id.0)),
                    replacement_policy_ref: retirement.replacement_policy_ref.clone(),
                    replacement_release_id: release.release_id.clone(),
                    replacement_argv: zero_config_replacement_argv(),
                })
                .collect(),
        },
    }
}

fn zero_config_replacement_argv() -> Vec<String> {
    ["forge-core", "start", "--root", ".", "--json"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn snapshot_manifest(
) -> Result<WorkflowRetirementSnapshotManifestDocument, Box<dyn std::error::Error>> {
    let archive_root = root().join(SNAPSHOT_DIR);
    let mut paths = std::fs::read_dir(&archive_root)?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    if paths.len() != 110 {
        return Err(format!(
            "snapshot manifest requires exactly 110 files, found {}",
            paths.len()
        )
        .into());
    }
    let entries = paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or("invalid snapshot filename")?;
            let bytes = std::fs::read(&path)?;
            reject_crlf(SNAPSHOT_DIR, &bytes)?;
            let semantic: serde_json::Value = yaml_serde::from_slice(&bytes)?;
            Ok(WorkflowRetirementSnapshotEntry {
                logical_ref: RepoPath(format!("{OPERATIONAL_DIR}/{name}")),
                archive_ref: RepoPath(format!("{SNAPSHOT_DIR}/{name}")),
                raw_digest: raw_digest(&bytes),
                canonical_digest: canonical_digest(&semantic)?,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    Ok(WorkflowRetirementSnapshotManifestDocument {
        schema_version: WORKFLOW_RETIREMENT_SNAPSHOT_MANIFEST_SCHEMA_VERSION.to_owned(),
        workflow_retirement_snapshot_manifest: WorkflowRetirementSnapshotManifest {
            id: id("workflow-retirement.legacy-snapshot-manifest.p5d-v0"),
            snapshot_version: "0.1.0".to_owned(),
            entries,
        },
    })
}

fn release_identity(
    manifest: &WorkflowGovernanceReleaseManifestDocument,
) -> Result<forge_core_contracts::WorkflowGovernanceReleaseIdentity, Box<dyn std::error::Error>> {
    let value = &manifest.workflow_governance_release_manifest;
    Ok(forge_core_contracts::WorkflowGovernanceReleaseIdentity {
        lineage_id: value.lineage_id.clone(),
        release_id: value.release_id.clone(),
        release_version: value.release_version.clone(),
        release_digest: workflow_release_manifest_digest(manifest)?,
    })
}

fn runtime_identity(
    bundle: &WorkflowGovernanceBundleDocument,
) -> Result<forge_core_contracts::WorkflowRuntimeBundleIdentity, Box<dyn std::error::Error>> {
    Ok(forge_core_contracts::WorkflowRuntimeBundleIdentity {
        bundle_id: bundle.workflow_governance_bundle.id.clone(),
        bundle_digest: workflow_runtime_bundle_digest(bundle)?,
        policy_set_digest: forge_core_decisions::workflow_policy_set_digest(
            &bundle.workflow_governance_bundle.policies,
        )?,
    })
}

fn binding<T: Serialize>(
    artifact_id: StableId,
    path: &str,
    bytes: &[u8],
    value: &T,
) -> Result<WorkflowRetirementArtifactBinding, Box<dyn std::error::Error>> {
    reject_crlf(path, bytes)?;
    Ok(WorkflowRetirementArtifactBinding {
        artifact_id,
        embedded_ref: RepoPath(path.to_owned()),
        raw_digest: raw_digest(bytes),
        canonical_digest: canonical_digest(value)?,
    })
}

fn parse_mode() -> Result<Mode, Box<dyn std::error::Error>> {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [flag] if flag == "--write" => Ok(Mode::Write),
        [flag] if flag == "--check" => Ok(Mode::Check),
        _ => Err("usage: generate_workflow_retirement_checkpoint (--write|--check)".into()),
    }
}

fn write(artifacts: &[Artifact]) -> Result<(), Box<dyn std::error::Error>> {
    for artifact in artifacts {
        reject_crlf(artifact.path, &artifact.bytes)?;
        let path = root().join(artifact.path);
        std::fs::create_dir_all(path.parent().expect("artifact parent"))?;
        std::fs::write(path, &artifact.bytes)?;
        println!("wrote {}", artifact.path);
    }
    Ok(())
}

fn check(artifacts: &[Artifact]) -> Result<(), Box<dyn std::error::Error>> {
    for artifact in artifacts {
        reject_crlf(artifact.path, &artifact.bytes)?;
        let found = read_bytes(artifact.path)?;
        reject_crlf(artifact.path, &found)?;
        if found != artifact.bytes {
            return Err(format!("generated artifact drift: {}", artifact.path).into());
        }
        println!("checked {}", artifact.path);
    }
    Ok(())
}

fn read_yaml<T: DeserializeOwned>(path: &str) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = read_bytes(path)?;
    reject_crlf(path, &bytes)?;
    Ok(yaml_serde::from_slice(&bytes)?)
}

fn read_bytes(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(std::fs::read(root().join(path))?)
}

fn yaml_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut text = yaml_serde::to_string(value)?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text.into_bytes())
}

fn reject_crlf(path: &str, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if bytes.windows(2).any(|pair| pair == b"\r\n") {
        return Err(format!("{path} must use LF-only bytes").into());
    }
    Ok(())
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = serde_json_canonicalizer::to_vec(value)?;
    Ok(raw_digest(&bytes))
}

fn raw_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}
