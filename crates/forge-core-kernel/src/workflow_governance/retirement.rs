//! Fixed, repository-compiled P5d.5 retirement admission boundary.

use std::sync::OnceLock;

use forge_core_authority::{
    verify_workflow_retirement_authorization_v2, VerifiedWorkflowRetirementAuthorizationAuditV2,
    VerifiedWorkflowRetirementAuthorizationV2, WorkflowRetirementExpectedContextV2,
};
use forge_core_contracts::{
    WorkflowConsumerCompatibilityMatrixDocument, WorkflowConsumerCompatibilityReportDocument,
    WorkflowDeletionProofDocument, WorkflowFinalScorecardDocument,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceReleaseManifestDocument,
    WorkflowGovernanceReleaseRegistryDocument, WorkflowReleaseReviewerRegistryDocument,
    WorkflowRetirementArtifactBinding, WorkflowRetirementAuthorizationV2Document,
    WorkflowRetirementEvidenceIndexDocument, WorkflowRetirementSnapshotManifestDocument,
    WorkflowRetirementTombstoneCatalogDocument,
};
use forge_core_decisions::{
    evaluate_workflow_retirement, WorkflowRetirementCandidateInput,
    WorkflowRetirementEvaluationStatus,
};
use include_dir::{include_dir, Dir};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

use super::load_admitted_workflow_governance_reviewed_release_registry;

pub const WORKFLOW_RETIREMENT_AUDIENCE: &str = "forge-core:workflow-retirement:embedded";
pub const WORKFLOW_RETIREMENT_AUTHORIZATION_REF: &str =
    "contracts/migration/workflow-retirement-authorization-v0.yaml";
pub const WORKFLOW_RETIREMENT_ADMISSION_EPOCH_UNIX: u64 = 1_783_821_600;
pub const WORKFLOW_RETIREMENT_CONSUMER_OBSERVED_UNTIL_UNIX: u64 = 1_783_814_400;
// Trust roots are compiled into the binary. Updating YAML and signatures alone cannot rotate them.
pub const WORKFLOW_RETIREMENT_REVIEWER_REGISTRY_RAW_DIGEST: &str =
    "sha256:4a139af44d559a98bf30f71fbe1483fc5ae93f3088ec9ee9632f68866270bc76";
pub const WORKFLOW_RETIREMENT_EVIDENCE_REVIEWER_KEY_FINGERPRINT: &str =
    "sha256:c3ef6bb19f098211e6d083a9d5394197a12226b77964b988ceee9508f87565e4";
pub const WORKFLOW_RETIREMENT_AUTHORIZER_KEY_FINGERPRINT: &str =
    "sha256:3cba322e182dd6620017210ea840aae21e890d079f21f4192c2ed5c770d1c926";

const RELEASE_HISTORY_REF: &str =
    "contracts/migration/workflow-governance-release-registry-agent-native-continuity-v0.yaml";
static LEGACY_SNAPSHOT: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/../../contracts/evidence/workflow-retirement/legacy-catalog");

pub const WORKFLOW_RETIREMENT_RELEASE_MANIFEST_REF: &str =
    "contracts/migration/workflow-governance-release-agent-native-continuity-candidate-v0.yaml";
pub const WORKFLOW_RETIREMENT_RUNTIME_BUNDLE_REF: &str =
    "contracts/workflow-governance/runtime-agent-native-continuity-v0.yaml";
pub const WORKFLOW_RETIREMENT_RUNTIME_EVIDENCE_REF: &str =
    "crates/forge-core-kernel/tests/workflow_retirement_runtime_evidence.rs";
pub const WORKFLOW_RETIREMENT_SNAPSHOT_MANIFEST_REF: &str =
    "contracts/evidence/workflow-retirement-legacy-snapshot-manifest-v0.yaml";
const EVIDENCE_INDEX_REF: &str = "contracts/migration/workflow-retirement-evidence-index-v0.yaml";
const DELETION_PROOF_REF: &str = "contracts/evidence/workflow-retirement-deletion-proof-v0.yaml";
const CONSUMER_MATRIX_REF: &str = "contracts/evidence/workflow-retirement-consumer-matrix-v0.yaml";
const CONSUMER_REPORT_REF: &str = "contracts/evidence/workflow-retirement-consumer-window-v0.yaml";
const TOMBSTONES_REF: &str = "contracts/migration/workflow-retirement-tombstones-v0.yaml";
const SCORECARD_REF: &str = "contracts/migration/workflow-governance-final-scorecard-v0.yaml";
const REVIEWER_REGISTRY_REF: &str =
    "contracts/policies/workflow-retirement-reviewer-registry-v0.yaml";

#[derive(Clone, Copy)]
struct EmbeddedRetirementArtifacts<'a> {
    manifest: &'a [u8],
    runtime: &'a [u8],
    history: &'a [u8],
    snapshot_manifest: &'a [u8],
    runtime_evidence: &'a [u8],
    index: &'a [u8],
    deletion: &'a [u8],
    matrix: &'a [u8],
    report: &'a [u8],
    tombstones: &'a [u8],
    scorecard: &'a [u8],
    registry: &'a [u8],
    authorization: &'a [u8],
}

const EMBEDDED: EmbeddedRetirementArtifacts<'static> = EmbeddedRetirementArtifacts {
    manifest: include_bytes!(
        "../../../../contracts/migration/workflow-governance-release-agent-native-continuity-candidate-v0.yaml"
    ),
    runtime: include_bytes!(
        "../../../../contracts/workflow-governance/runtime-agent-native-continuity-v0.yaml"
    ),
    history: include_bytes!(
        "../../../../contracts/migration/workflow-governance-release-registry-agent-native-continuity-v0.yaml"
    ),
    snapshot_manifest: include_bytes!(
        "../../../../contracts/evidence/workflow-retirement-legacy-snapshot-manifest-v0.yaml"
    ),
    runtime_evidence: include_bytes!("../../tests/workflow_retirement_runtime_evidence.rs"),
    index: include_bytes!(
        "../../../../contracts/migration/workflow-retirement-evidence-index-v0.yaml"
    ),
    deletion: include_bytes!(
        "../../../../contracts/evidence/workflow-retirement-deletion-proof-v0.yaml"
    ),
    matrix: include_bytes!(
        "../../../../contracts/evidence/workflow-retirement-consumer-matrix-v0.yaml"
    ),
    report: include_bytes!(
        "../../../../contracts/evidence/workflow-retirement-consumer-window-v0.yaml"
    ),
    tombstones: include_bytes!(
        "../../../../contracts/migration/workflow-retirement-tombstones-v0.yaml"
    ),
    scorecard: include_bytes!(
        "../../../../contracts/migration/workflow-governance-final-scorecard-v0.yaml"
    ),
    registry: include_bytes!(
        "../../../../contracts/policies/workflow-retirement-reviewer-registry-v0.yaml"
    ),
    authorization: include_bytes!(
        "../../../../contracts/migration/workflow-retirement-authorization-v0.yaml"
    ),
};

/// Opaque process-admitted all-42 retirement checkpoint.
///
/// The verified capability is retained privately for the process lifetime.
/// Public projections are diagnostic data and cannot recreate authority.
pub struct AdmittedWorkflowRetirementCheckpoint {
    _authorization: VerifiedWorkflowRetirementAuthorizationV2,
    audit: VerifiedWorkflowRetirementAuthorizationAuditV2,
    tombstones: WorkflowRetirementTombstoneCatalogDocument,
    scorecard: WorkflowFinalScorecardDocument,
}

impl AdmittedWorkflowRetirementCheckpoint {
    #[must_use]
    pub const fn audit(&self) -> &VerifiedWorkflowRetirementAuthorizationAuditV2 {
        &self.audit
    }

    #[must_use]
    pub const fn tombstones(&self) -> &WorkflowRetirementTombstoneCatalogDocument {
        &self.tombstones
    }

    #[must_use]
    pub const fn scorecard(&self) -> &WorkflowFinalScorecardDocument {
        &self.scorecard
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedWorkflowRetirementError {
    pub artifact: &'static str,
    pub issue: String,
}

impl std::fmt::Display for AdmittedWorkflowRetirementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.artifact, self.issue)
    }
}

impl std::error::Error for AdmittedWorkflowRetirementError {}

/// Loads and admits the one fixed aggregate retirement checkpoint.
///
/// # Errors
/// Returns a stable fail-closed error if any embedded byte, binding,
/// deterministic evaluation, trusted release, or signature differs.
pub fn load_admitted_workflow_retirement_checkpoint(
) -> Result<&'static AdmittedWorkflowRetirementCheckpoint, AdmittedWorkflowRetirementError> {
    static CHECKPOINT: OnceLock<
        Result<AdmittedWorkflowRetirementCheckpoint, AdmittedWorkflowRetirementError>,
    > = OnceLock::new();
    CHECKPOINT
        .get_or_init(|| admit(EMBEDDED))
        .as_ref()
        .map_err(Clone::clone)
}

#[allow(clippy::too_many_lines)]
fn admit(
    artifacts: EmbeddedRetirementArtifacts<'_>,
) -> Result<AdmittedWorkflowRetirementCheckpoint, AdmittedWorkflowRetirementError> {
    let manifest: WorkflowGovernanceReleaseManifestDocument =
        parse("manifest", artifacts.manifest)?;
    let runtime: WorkflowGovernanceBundleDocument = parse("runtime", artifacts.runtime)?;
    let history: WorkflowGovernanceReleaseRegistryDocument = parse("history", artifacts.history)?;
    let snapshot_manifest: WorkflowRetirementSnapshotManifestDocument =
        parse("snapshot manifest", artifacts.snapshot_manifest)?;
    let index: WorkflowRetirementEvidenceIndexDocument = parse("index", artifacts.index)?;
    let deletion: WorkflowDeletionProofDocument = parse("deletion", artifacts.deletion)?;
    let matrix: WorkflowConsumerCompatibilityMatrixDocument = parse("matrix", artifacts.matrix)?;
    let report: WorkflowConsumerCompatibilityReportDocument = parse("report", artifacts.report)?;
    let tombstones: WorkflowRetirementTombstoneCatalogDocument =
        parse("tombstones", artifacts.tombstones)?;
    let scorecard: WorkflowFinalScorecardDocument = parse("scorecard", artifacts.scorecard)?;
    let registry: WorkflowReleaseReviewerRegistryDocument =
        parse("reviewer registry", artifacts.registry)?;
    let authorization: WorkflowRetirementAuthorizationV2Document =
        parse("authorization", artifacts.authorization)?;
    let payload = &authorization.workflow_retirement_authorization_v2.payload;

    verify_binding(
        "manifest",
        WORKFLOW_RETIREMENT_RELEASE_MANIFEST_REF,
        artifacts.manifest,
        &manifest,
        &payload.release_manifest,
    )?;
    verify_binding(
        "runtime",
        WORKFLOW_RETIREMENT_RUNTIME_BUNDLE_REF,
        artifacts.runtime,
        &runtime,
        &payload.runtime_bundle_artifact,
    )?;
    verify_binding(
        "snapshot manifest",
        WORKFLOW_RETIREMENT_SNAPSHOT_MANIFEST_REF,
        artifacts.snapshot_manifest,
        &snapshot_manifest,
        &index.workflow_retirement_evidence_index.snapshot_manifest,
    )?;
    require(
        "snapshot manifest",
        snapshot_manifest
            .workflow_retirement_snapshot_manifest
            .entries
            .len()
            == 110,
        "snapshot manifest must bind exactly 110 files",
    )?;
    require(
        "snapshot manifest",
        index.workflow_retirement_evidence_index.snapshot_manifest == payload.snapshot_manifest,
        "authorization does not bind snapshot manifest",
    )?;
    verify_snapshot_entries(&snapshot_manifest)?;
    let runtime_evidence_text = std::str::from_utf8(artifacts.runtime_evidence)
        .map_err(|error| failure("runtime evidence", error.to_string()))?;
    verify_binding(
        "runtime evidence",
        WORKFLOW_RETIREMENT_RUNTIME_EVIDENCE_REF,
        artifacts.runtime_evidence,
        &runtime_evidence_text,
        &index.workflow_retirement_evidence_index.runtime_evidence,
    )?;
    require(
        "runtime evidence",
        index.workflow_retirement_evidence_index.runtime_evidence == payload.runtime_evidence,
        "authorization does not bind runtime evidence",
    )?;
    verify_binding(
        "history",
        RELEASE_HISTORY_REF,
        artifacts.history,
        &history,
        &index.workflow_retirement_evidence_index.release_history,
    )?;
    let index_binding = computed_binding(
        index.workflow_retirement_evidence_index.id.clone(),
        EVIDENCE_INDEX_REF,
        artifacts.index,
        &index,
    )?;
    require(
        "index",
        index_binding == payload.evidence_index,
        "authorization does not bind the exact evidence index",
    )?;
    verify_binding(
        "deletion",
        DELETION_PROOF_REF,
        artifacts.deletion,
        &deletion,
        &index.workflow_retirement_evidence_index.deletion_proof,
    )?;
    verify_binding(
        "report",
        CONSUMER_REPORT_REF,
        artifacts.report,
        &report,
        &index.workflow_retirement_evidence_index.consumer_report,
    )?;
    verify_binding(
        "matrix",
        CONSUMER_MATRIX_REF,
        artifacts.matrix,
        &matrix,
        &report
            .workflow_consumer_compatibility_report
            .compatibility_matrix,
    )?;
    verify_binding(
        "tombstones",
        TOMBSTONES_REF,
        artifacts.tombstones,
        &tombstones,
        &payload.tombstone_catalog,
    )?;
    verify_binding(
        "scorecard",
        SCORECARD_REF,
        artifacts.scorecard,
        &scorecard,
        &payload.final_scorecard,
    )?;
    verify_binding(
        "reviewer registry",
        REVIEWER_REGISTRY_REF,
        artifacts.registry,
        &registry,
        &payload.reviewer_registry,
    )?;

    let trusted = load_admitted_workflow_governance_reviewed_release_registry()
        .map_err(|error| failure("release history", format!("{error:?}")))?;
    let latest = trusted.latest_release();
    require(
        "release history",
        trusted.release_count() == 5,
        "trusted registry is not the exact five-release history",
    )?;
    require(
        "release history",
        latest.policy_count() == 42,
        "trusted latest release is not the exact 42-policy runtime",
    )?;
    require(
        "release history",
        latest.release() == &index.workflow_retirement_evidence_index.release,
        "history/index release identity mismatch",
    )?;
    require(
        "release history",
        latest.runtime_bundle() == &index.workflow_retirement_evidence_index.runtime_bundle,
        "history/index runtime identity mismatch",
    )?;
    let history_value = &history.workflow_governance_release_registry;
    let provenance = trusted.registry_provenance();
    require(
        "release history",
        history_value.registry_id == provenance.registry_id
            && history_value.registry_version == provenance.registry_version,
        "embedded history is not the trusted admitted registry",
    )?;

    let evaluation = evaluate_workflow_retirement(&WorkflowRetirementCandidateInput {
        evidence_index: index.clone(),
        evidence_index_binding: index_binding,
        deletion_proof: deletion,
        consumer_matrix: matrix,
        consumer_report: report,
        tombstone_catalog: tombstones.clone(),
        release_manifest: manifest,
        runtime_bundle: runtime,
    });
    require(
        "evaluation",
        evaluation.status == WorkflowRetirementEvaluationStatus::ReadyForIndependentAuthorization
            && evaluation.issues.is_empty()
            && evaluation.retired_legacy_count == 42,
        "deterministic all-42 retirement evaluation is blocked",
    )?;
    require(
        "scorecard",
        evaluation.scorecard == scorecard,
        "authored scorecard differs from deterministic evaluation",
    )?;

    let verified = verify_workflow_retirement_authorization_v2(
        &registry,
        artifacts.registry,
        &authorization,
        WorkflowRetirementExpectedContextV2 {
            release: &index.workflow_retirement_evidence_index.release,
            runtime_bundle: &index.workflow_retirement_evidence_index.runtime_bundle,
            legacy_catalog_digest: &index
                .workflow_retirement_evidence_index
                .legacy_catalog_digest,
            retirements: &index.workflow_retirement_evidence_index.retirements,
            release_manifest: &index.workflow_retirement_evidence_index.release_manifest,
            runtime_bundle_artifact: &index
                .workflow_retirement_evidence_index
                .runtime_bundle_artifact,
            snapshot_manifest: &index.workflow_retirement_evidence_index.snapshot_manifest,
            runtime_evidence: &index.workflow_retirement_evidence_index.runtime_evidence,
            release_history: &index.workflow_retirement_evidence_index.release_history,
            evidence_index: &payload.evidence_index,
            deletion_proof: &index.workflow_retirement_evidence_index.deletion_proof,
            consumer_report: &index.workflow_retirement_evidence_index.consumer_report,
            tombstone_catalog: &payload.tombstone_catalog,
            final_scorecard: &payload.final_scorecard,
            reviewer_registry: &payload.reviewer_registry,
            admission_epoch_unix: WORKFLOW_RETIREMENT_ADMISSION_EPOCH_UNIX,
            consumer_observed_until_unix: WORKFLOW_RETIREMENT_CONSUMER_OBSERVED_UNTIL_UNIX,
            reviewer_registry_raw_digest: WORKFLOW_RETIREMENT_REVIEWER_REGISTRY_RAW_DIGEST,
            evidence_reviewer_key_fingerprint:
                WORKFLOW_RETIREMENT_EVIDENCE_REVIEWER_KEY_FINGERPRINT,
            retirement_authorizer_key_fingerprint: WORKFLOW_RETIREMENT_AUTHORIZER_KEY_FINGERPRINT,
        },
        WORKFLOW_RETIREMENT_AUDIENCE,
    )
    .map_err(|error| failure("authorization", error.to_string()))?;
    let audit = verified.audit();
    Ok(AdmittedWorkflowRetirementCheckpoint {
        _authorization: verified,
        audit,
        tombstones,
        scorecard,
    })
}

fn verify_snapshot_entries(
    manifest: &WorkflowRetirementSnapshotManifestDocument,
) -> Result<(), AdmittedWorkflowRetirementError> {
    let entries = &manifest.workflow_retirement_snapshot_manifest.entries;
    require(
        "snapshot manifest",
        LEGACY_SNAPSHOT.files().count() == entries.len(),
        "embedded archive count differs from snapshot manifest",
    )?;
    for entry in entries {
        let prefix = "contracts/evidence/workflow-retirement/legacy-catalog/";
        let relative = entry
            .archive_ref
            .0
            .strip_prefix(prefix)
            .ok_or_else(|| failure("snapshot manifest", "archive ref leaves frozen root"))?;
        let file = LEGACY_SNAPSHOT
            .get_file(relative)
            .ok_or_else(|| failure("snapshot manifest", format!("missing archive {relative}")))?;
        let bytes = file.contents();
        reject_crlf("snapshot entry", bytes)?;
        require(
            "snapshot entry",
            digest(bytes) == entry.raw_digest,
            "raw archive digest mismatch",
        )?;
        let semantic: serde_json::Value = yaml_serde::from_slice(bytes)
            .map_err(|error| failure("snapshot entry", error.to_string()))?;
        let canonical = serde_json_canonicalizer::to_vec(&semantic)
            .map_err(|error| failure("snapshot entry", error.to_string()))?;
        require(
            "snapshot entry",
            digest(&canonical) == entry.canonical_digest,
            "canonical archive digest mismatch",
        )?;
        require(
            "snapshot entry",
            entry.logical_ref.0 == format!("contracts/workflows/{relative}"),
            "logical/archive mapping mismatch",
        )?;
    }
    Ok(())
}

fn parse<T: DeserializeOwned>(
    artifact: &'static str,
    bytes: &[u8],
) -> Result<T, AdmittedWorkflowRetirementError> {
    reject_crlf(artifact, bytes)?;
    yaml_serde::from_slice(bytes).map_err(|error| failure(artifact, error.to_string()))
}

fn verify_binding<T: Serialize>(
    artifact: &'static str,
    expected_ref: &str,
    bytes: &[u8],
    value: &T,
    binding: &WorkflowRetirementArtifactBinding,
) -> Result<(), AdmittedWorkflowRetirementError> {
    let computed = computed_binding(binding.artifact_id.clone(), expected_ref, bytes, value)?;
    require(
        artifact,
        computed == *binding,
        "raw/canonical/ref binding mismatch",
    )
}

fn computed_binding<T: Serialize>(
    artifact_id: forge_core_contracts::StableId,
    embedded_ref: &str,
    bytes: &[u8],
    value: &T,
) -> Result<WorkflowRetirementArtifactBinding, AdmittedWorkflowRetirementError> {
    reject_crlf("binding", bytes)?;
    let canonical = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| failure("binding", error.to_string()))?;
    Ok(WorkflowRetirementArtifactBinding {
        artifact_id,
        embedded_ref: forge_core_contracts::RepoPath(embedded_ref.to_owned()),
        raw_digest: digest(bytes),
        canonical_digest: digest(&canonical),
    })
}

fn reject_crlf(
    artifact: &'static str,
    bytes: &[u8],
) -> Result<(), AdmittedWorkflowRetirementError> {
    require(
        artifact,
        !bytes.windows(2).any(|pair| pair == b"\r\n"),
        "embedded artifact must be LF-only",
    )
}

fn require(
    artifact: &'static str,
    condition: bool,
    issue: &str,
) -> Result<(), AdmittedWorkflowRetirementError> {
    if condition {
        Ok(())
    } else {
        Err(failure(artifact, issue))
    }
}

fn failure(artifact: &'static str, issue: impl Into<String>) -> AdmittedWorkflowRetirementError {
    AdmittedWorkflowRetirementError {
        artifact,
        issue: issue.into(),
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_checkpoint_admits_exact_all_42_aggregate_once() {
        let checkpoint = load_admitted_workflow_retirement_checkpoint().expect("signed checkpoint");
        assert_eq!(
            checkpoint
                .scorecard()
                .workflow_final_scorecard
                .legacy_authority_counts
                .retired,
            42
        );
        assert_eq!(
            checkpoint
                .tombstones()
                .workflow_retirement_tombstone_catalog
                .tombstones
                .len(),
            42
        );
        assert_eq!(
            checkpoint.audit().release_id,
            "workflow-governance.release.agent-native-continuity-v0"
        );
        assert!(std::ptr::eq(
            checkpoint,
            load_admitted_workflow_retirement_checkpoint().unwrap()
        ));
    }

    #[test]
    fn tampered_or_missing_artifacts_fail_closed() {
        for field in [
            "snapshot_manifest",
            "runtime_evidence",
            "index",
            "deletion",
            "matrix",
            "report",
            "tombstones",
            "scorecard",
            "registry",
            "authorization",
        ] {
            let mut owned = EMBEDDED.index.to_vec();
            owned[0] ^= 1;
            let mut candidate = EMBEDDED;
            match field {
                "snapshot_manifest" => candidate.snapshot_manifest = &owned,
                "runtime_evidence" => candidate.runtime_evidence = &owned,
                "index" => candidate.index = &owned,
                "deletion" => candidate.deletion = &owned,
                "matrix" => candidate.matrix = &owned,
                "report" => candidate.report = &owned,
                "tombstones" => candidate.tombstones = &owned,
                "scorecard" => candidate.scorecard = &owned,
                "registry" => candidate.registry = &owned,
                "authorization" => candidate.authorization = &owned,
                _ => unreachable!(),
            }
            assert!(admit(candidate).is_err(), "{field} tamper admitted");
        }
        let mut missing = EMBEDDED;
        missing.history = b"";
        assert!(admit(missing).is_err());
    }

    #[test]
    fn subset_and_cross_history_transplants_fail_closed() {
        let text = std::str::from_utf8(EMBEDDED.index).unwrap();
        let first = text.find("  - workflow_id:").unwrap();
        let second = text[first + 1..].find("  - workflow_id:").unwrap() + first + 1;
        let mut subset = text.as_bytes().to_vec();
        subset.drain(first..second);
        let mut candidate = EMBEDDED;
        candidate.index = &subset;
        assert!(admit(candidate).is_err());

        let mut cross_history = EMBEDDED.history.to_vec();
        let needle = b"workflow-governance.release.agent-native-continuity-v0";
        let at = cross_history
            .windows(needle.len())
            .position(|window| window == needle)
            .unwrap();
        cross_history[at] = b'X';
        candidate = EMBEDDED;
        candidate.history = &cross_history;
        assert!(admit(candidate).is_err());
    }
    #[test]
    fn manifest_runtime_and_snapshot_raw_or_semantic_drift_fail_closed() {
        for field in ["manifest", "runtime", "snapshot"] {
            let source = match field {
                "manifest" => EMBEDDED.manifest,
                "runtime" => EMBEDDED.runtime,
                "snapshot" => EMBEDDED.snapshot_manifest,
                _ => unreachable!(),
            };
            let mut whitespace = source.to_vec();
            whitespace.push(b'\n');
            let mut candidate = EMBEDDED;
            match field {
                "manifest" => candidate.manifest = &whitespace,
                "runtime" => candidate.runtime = &whitespace,
                "snapshot" => candidate.snapshot_manifest = &whitespace,
                _ => unreachable!(),
            }
            assert!(
                admit(candidate).is_err(),
                "{field} whitespace drift admitted"
            );

            let mut semantic = source.to_vec();
            let at = semantic
                .iter()
                .position(|byte| *byte == b'0')
                .expect("version byte");
            semantic[at] = b'9';
            candidate = EMBEDDED;
            match field {
                "manifest" => candidate.manifest = &semantic,
                "runtime" => candidate.runtime = &semantic,
                "snapshot" => candidate.snapshot_manifest = &semantic,
                _ => unreachable!(),
            }
            assert!(admit(candidate).is_err(), "{field} semantic drift admitted");
        }
    }
}
