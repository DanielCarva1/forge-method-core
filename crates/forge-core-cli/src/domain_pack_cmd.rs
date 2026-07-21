//! Agent surface for governed Domain Pack validation, resolution, and state.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_authority::{
    verify_domain_pack_supply_chain_snapshot, AnchoredDomainPackSupplyChainSnapshot,
    DomainPackRegistryAnchor, DomainPackRegistryAnchorAdvance,
    VerifiedDomainPackSupplyChainSnapshot,
};
use forge_core_command_surface::COMMAND_DOMAIN_PACK;
use forge_core_contracts::{
    CliEnvelope, DomainPackAcquisitionCatalogDocument, DomainPackAcquisitionDerivationInput,
    DomainPackAcquisitionIntentDocument, DomainPackAcquisitionPlanningInput,
    DomainPackActivePointerDocument, DomainPackArtifactBinding,
    DomainPackAuthorSkeletonRequestDocument, DomainPackAuthorTestRequestDocument,
    DomainPackCandidateAuthority, DomainPackCandidateInput,
    DomainPackCapabilitySandboxPolicyDocument, DomainPackCompatibilityStatus,
    DomainPackCompositionIssue, DomainPackCompositionRequestDocument, DomainPackCompositionStatus,
    DomainPackContentDocument, DomainPackCoreBinding, DomainPackDiscoveryGap,
    DomainPackDiscoveryMatch, DomainPackDiscoveryProjectionDocument,
    DomainPackDiscoveryRequestDocument, DomainPackExactLock, DomainPackExactLockDocument,
    DomainPackExactLockPayload, DomainPackExpectedLifecycleState,
    DomainPackInitializedProjectDerivationInput, DomainPackInitializedProjectDerivationMaterial,
    DomainPackInitializedProjectIntentDocument, DomainPackInitializedProjectOperation,
    DomainPackLifecycleLedgerRecord, DomainPackLifecycleOperation, DomainPackLifecyclePreflight,
    DomainPackLifecyclePreflightDocument, DomainPackLifecyclePreflightStatus,
    DomainPackLifecycleReceiptDocument, DomainPackLifecycleRequest,
    DomainPackLifecycleRequestDocument, DomainPackLockedPackage, DomainPackManifestDocument,
    DomainPackPackageRevocation, DomainPackRebasePlanDocument,
    DomainPackRemoteAcquisitionPlanDocument, DomainPackRemoteAcquisitionRequestDocument,
    DomainPackRemoteArtifactMediaType, DomainPackRemoteCacheProjectionDocument,
    DomainPackRemoteFetchEvidenceDocument, DomainPackRemoteFetchReceiptDocument,
    DomainPackRemoteUntrustedFetchObservation, DomainPackRemoteUntrustedFetchObservationDocument,
    DomainPackResolutionRequestDocument, DomainPackResolutionStatus,
    DomainPackRuntimeCapabilityRegistryDocument, DomainPackSemanticAssurance,
    DomainPackSourceAssurance, DomainPackSupplyChainAssessment,
    DomainPackSupplyChainRegistryDocument, DomainPackTrustPolicyDocument,
    DurableAssuranceEpochBinding, RepoPath, StableId, DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION,
    DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
    DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION, MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRIES,
};
use forge_core_decisions::domain_pack_acquisition::derive_domain_pack_initialized_project_lifecycle;
use forge_core_decisions::{
    compose_domain_packs, derive_domain_pack_acquisition_inputs, discover_domain_packs,
    domain_pack_resolution_projection_digest, evaluate_domain_pack_author_test,
    evaluate_domain_pack_compatibility, evaluate_domain_pack_trust,
    generate_domain_pack_author_skeleton, join_reviewed_registry_to_resolution,
    plan_domain_pack_acquisition, plan_domain_pack_remote_acquisition, resolve_domain_packs,
    validate_domain_pack_candidate, verify_domain_pack_discovery_projection,
    verify_domain_pack_rebase_plan, verify_domain_pack_remote_acquisition_plan,
    verify_domain_pack_remote_fetches, DomainPackCandidateMaterial, DomainPackCompatibilityInput,
    DomainPackRemoteAcquisitionPlanningInput, DomainPackRemoteCatalogAvailability,
    DomainPackRemoteCatalogFacts, DomainPackRemoteFetchAttempt,
    DomainPackRemoteFetchVerificationInput, DomainPackReviewedResolutionJoinStatus,
    DomainPackTrustEvaluationInput, DomainPackTrustEvaluationStatus,
    DomainPackTrustSelectedPackage, MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES,
};
use forge_core_domain_pack_tcb::{
    authorize_prepared_domain_pack_lifecycle, derive_domain_pack_capability_demands,
    domain_pack_project_snapshot_digest, lock_domain_pack_lifecycle,
    verify_domain_pack_project_snapshot, DomainPackImmutableArtifact,
    DomainPackLifecycleAuthorizationContext, DomainPackLifecycleStoreError,
    DOMAIN_PACK_MAX_DOCUMENT_BYTES,
};
use forge_core_store::{
    acquire_effect_store_lock, backup::BackupExpectedMember,
    retained_crash_replace::reconcile_file_crash_safe_under_owned_lock,
    OwnedRetainedCrashReplaceRead, OwnedRetainedCrashReplaceSession,
};
use sha2::{Digest, Sha256};

use crate::cli_error::ExitError;

#[derive(Debug, serde::Serialize)]
struct DomainPackValidationPayload {
    authority: DomainPackCandidateAuthority,
    structurally_valid: bool,
    publisher: String,
    name: String,
    version: String,
    issues: Vec<DomainPackCompositionIssue>,
    boundary: &'static str,
}

#[derive(Debug, serde::Serialize)]
struct DomainPackDiscoveryExplanationPayload {
    request_id: StableId,
    demand_digest: String,
    requirement_ref: StableId,
    authority: DomainPackCandidateAuthority,
    assurance_binding: DurableAssuranceEpochBinding,
    uncertainties: Vec<String>,
    matches: Vec<DomainPackDiscoveryMatch>,
    gaps: Vec<DomainPackDiscoveryGap>,
    boundary: &'static str,
}

/// Opaque catalog facts from the host/TCB boundary. This command only passes
/// them to the pure candidate-byte planner; it does not authenticate a catalog,
/// establish or advance an anchor, or infer the facts from local artifacts.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackRemoteCatalogFactsFile {
    snapshot_digest: String,
    availability: DomainPackRemoteCatalogAvailabilityFile,
    host_checked_at_unix: u64,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum DomainPackRemoteCatalogAvailabilityFile {
    CurrentAnchored,
    Unavailable,
    NotAnchored,
    Stale,
    Revoked,
    SignatureTamper,
}

impl From<DomainPackRemoteCatalogAvailabilityFile> for DomainPackRemoteCatalogAvailability {
    fn from(value: DomainPackRemoteCatalogAvailabilityFile) -> Self {
        match value {
            DomainPackRemoteCatalogAvailabilityFile::CurrentAnchored => Self::CurrentAnchored,
            DomainPackRemoteCatalogAvailabilityFile::Unavailable => Self::Unavailable,
            DomainPackRemoteCatalogAvailabilityFile::NotAnchored => Self::NotAnchored,
            DomainPackRemoteCatalogAvailabilityFile::Stale => Self::Stale,
            DomainPackRemoteCatalogAvailabilityFile::Revoked => Self::Revoked,
            DomainPackRemoteCatalogAvailabilityFile::SignatureTamper => Self::SignatureTamper,
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum DomainPackCandidateByteDownloadStatus {
    Blocked,
    ReadyForTrustEvaluation,
}

/// Closed, candidate-only handoff. It deliberately contains no trust,
/// review, lifecycle, install, cache-mutation, or activation decision.
#[derive(Debug, serde::Serialize)]
struct DomainPackCandidateByteDownloadPayload {
    status: DomainPackCandidateByteDownloadStatus,
    authority: DomainPackCandidateAuthority,
    acquisition_plan: forge_core_contracts::DomainPackAcquisitionPlanDocument,
    acquisition_derivation: forge_core_contracts::DomainPackAcquisitionDerivedInputsDocument,
    remote_plan: DomainPackRemoteAcquisitionPlanDocument,
    evidence: DomainPackRemoteFetchEvidenceDocument,
    receipt: DomainPackRemoteFetchReceiptDocument,
    artifact_set_digest: String,
    boundary: &'static str,
}

#[derive(Debug, serde::Serialize)]
struct DomainPackLifecycleStatusPayload {
    state_root: String,
    active: bool,
    active_pointer: Option<DomainPackActivePointerDocument>,
    active_lock: Option<DomainPackExactLockDocument>,
    ledger_records: Vec<DomainPackLifecycleLedgerRecord>,
    recovery_report: forge_core_contracts::DomainPackRecoveryReportDocument,
    recovery_checked: bool,
    boundary: &'static str,
}

#[derive(Debug, serde::Serialize)]
struct DomainPackLifecyclePreflightPayload {
    ready: bool,
    preflight_digest: String,
    supply_chain: forge_core_authority::VerifiedDomainPackSupplyChainSnapshotAudit,
    boundary: &'static str,
}

#[derive(Debug, serde::Serialize)]
struct DomainPackTrustProvisionPayload {
    operator_root: String,
    registry_id: StableId,
    audience: StableId,
    generation: u64,
    snapshot_digest: String,
    trust_policy_digest: String,
    anchor_previously_present: bool,
    boundary: &'static str,
}

struct OwnedMaterial {
    manifest: Vec<u8>,
    content: Vec<u8>,
    license: Vec<u8>,
}

struct OwnedImmutableArtifact {
    binding: DomainPackArtifactBinding,
    raw_bytes: Vec<u8>,
}

struct CanonicalLifecycleRoots {
    project: PathBuf,
    artifacts: PathBuf,
    state: PathBuf,
    project_lexical: PathBuf,
    artifacts_lexical: PathBuf,
    state_lexical: PathBuf,
}

/// Read only the schema identifier before deserializing the complete protected
/// head. This lets a retired head format fail with an explicit compatibility
/// error instead of an incidental missing-field parse error.
#[derive(Debug, serde::Deserialize)]
struct DomainPackRegistryAnchorSchemaHeader {
    schema_version: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackRegistryAnchorHead {
    schema_version: String,
    registry_id: StableId,
    audience: StableId,
    generation: u64,
    snapshot_digest: String,
    trust_policy_digest: String,
    cumulative_revocations: Vec<DomainPackPackageRevocation>,
    cumulative_revocation_digest: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackOperatorSourceBinding {
    schema_version: String,
    operator_root: String,
    trust_policy_file: String,
    registry_file: String,
    reviewer_registry_file: String,
    reviewed_registry_file: String,
    capability_registry_file: String,
    sandbox_policy_file: String,
    artifact_root: String,
}

/// Opaque retained authority for the external supply-chain anchor.
pub(crate) struct LockedOperatorRegistryAnchor {
    operator_root: PathBuf,
    registry_file: PathBuf,
    root_identity: crate::io_util::RetainedDirectoryIdentity,
    reconciliation: Option<OwnedRetainedCrashReplaceSession>,
    exact_read: Option<OwnedRetainedCrashReplaceRead>,
}

#[allow(dead_code, clippy::struct_field_names)]
/// Exact public registry and protected anchor bytes captured under one lock.
pub(crate) struct OperatorRegistryAnchorSnapshot {
    raw_registry: Vec<u8>,
    raw_registry_sha256: String,
    raw_anchor: Vec<u8>,
    raw_anchor_sha256: String,
}

#[allow(dead_code)]
impl OperatorRegistryAnchorSnapshot {
    pub(crate) fn raw_registry(&self) -> &[u8] {
        &self.raw_registry
    }

    pub(crate) fn raw_registry_sha256(&self) -> &str {
        &self.raw_registry_sha256
    }

    pub(crate) fn raw_anchor(&self) -> &[u8] {
        &self.raw_anchor
    }

    pub(crate) fn raw_anchor_sha256(&self) -> &str {
        &self.raw_anchor_sha256
    }
}

/// Opaque retained authority for `domain-packs/operator-sources.yaml`.
pub(crate) struct LockedDomainPackOperatorSources {
    root_identity: crate::io_util::RetainedDirectoryIdentity,
    lock_parent_identity: crate::io_util::RetainedDirectoryIdentity,
    target_parent_identity: crate::io_util::RetainedDirectoryIdentity,
    reconciliation: Option<OwnedRetainedCrashReplaceSession>,
    exact_read: Option<OwnedRetainedCrashReplaceRead>,
}

/// Opaque retained authority for `domain-packs/rebase-plan.yaml`.
pub(crate) struct LockedDomainPackRebasePlan {
    root_identity: crate::io_util::RetainedDirectoryIdentity,
    lock_parent_identity: crate::io_util::RetainedDirectoryIdentity,
    target_parent_identity: crate::io_util::RetainedDirectoryIdentity,
    reconciliation: Option<OwnedRetainedCrashReplaceSession>,
    exact_read: Option<OwnedRetainedCrashReplaceRead>,
}

#[allow(dead_code)]
/// Exact optional bytes of one recovered producer-owned state file.
pub(crate) struct DomainPackStateFileSnapshot {
    relative_path: &'static str,
    raw: Option<Vec<u8>>,
}

#[allow(dead_code)]
impl DomainPackStateFileSnapshot {
    pub(crate) const fn relative_path(&self) -> &'static str {
        self.relative_path
    }

    pub(crate) fn raw(&self) -> Option<&[u8]> {
        self.raw.as_deref()
    }
}

/// Aggregate external producer authority retained by backup while state-root
/// producers hand off to the immediately following host-quiescence boundary.
pub(crate) struct LockedDomainPackBackupAuthorities {
    expected_members: Vec<BackupExpectedMember>,
    _supply_chain: Option<LockedOperatorRegistryAnchor>,
    _reviewed_learning: Option<crate::domain_pack_learning_cmd::LockedReviewedSnapshot>,
}

impl LockedDomainPackBackupAuthorities {
    pub(crate) fn expected_members(&self) -> &[BackupExpectedMember] {
        &self.expected_members
    }
}

const DOMAIN_PACK_REGISTRY_ANCHOR_SCHEMA_VERSION: &str = "forge-domain-pack-registry-anchor-v2";
const DOMAIN_PACK_OPERATOR_SOURCE_SCHEMA_VERSION: &str = "forge-domain-pack-operator-sources-v1";
const DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH: &str = "domain-packs/operator-sources.yaml";
const DOMAIN_PACK_OPERATOR_SOURCE_LOCK_RELATIVE_PATH: &str =
    "locks/domain-packs.operator-sources.lock";
const DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH: &str = "domain-packs/rebase-plan.yaml";
const DOMAIN_PACK_REBASE_PLAN_LOCK_RELATIVE_PATH: &str = "locks/domain-packs.rebase-plan.lock";
const DOMAIN_PACK_REGISTRY_ANCHOR_RELATIVE_PATH: &str = ".forge-domain-pack-registry-anchor.yaml";
const DOMAIN_PACK_REGISTRY_ANCHOR_LOCK_RELATIVE_PATH: &str =
    ".forge-domain-pack-registry-anchor.lock";
const DOMAIN_PACK_TRUST_ON_FIRST_USE_ACKNOWLEDGEMENT: &str = "I_UNDERSTAND_TRUST_ON_FIRST_USE";
/// The local-only candidate-byte command must not recursively enumerate an
/// unbounded untrusted filesystem tree before comparing it to the signed plan.
const MAX_LOCAL_ARTIFACT_TREE_DEPTH: usize = 64;
const MAX_LOCAL_ARTIFACT_TREE_ENTRIES: usize = MAX_DOMAIN_PACK_REMOTE_CACHE_ENTRIES as usize;

/// Dispatch the governed `forge-core domain-pack` command family.
///
/// # Errors
///
/// Returns a typed CLI error when arguments, bounded inputs, trust roots,
/// registry continuity, lifecycle authorization, or durable state fail closed.
pub fn run_domain_pack_command(args: &[String]) -> Result<(), ExitError> {
    match args.get(1).map_or("--help", String::as_str) {
        "validate" => run_validate(&args[2..]),
        "search" => run_search(&args[2..]),
        "explain" => run_explain(&args[2..]),
        "acquire" => run_acquire(&args[2..]),
        "author" => run_author(&args[2..]),
        "compose" => run_compose(&args[2..]),
        "resolve" => run_resolve(&args[2..]),
        "learning" => crate::domain_pack_learning_cmd::run_domain_pack_learning_command(&args[2..]),
        "trust-provision" => run_trust_provision(&args[2..]),
        "status" => run_lifecycle_state(&args[2..], false),
        "recover" => run_lifecycle_state(&args[2..], true),
        "preflight" => run_lifecycle_authorized(&args[2..], false),
        "apply" => run_lifecycle_authorized(&args[2..], true),
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core domain-pack: unknown subcommand '{other}'\n{}",
            usage()
        ))),
    }
}

fn run_search(args: &[String]) -> Result<(), ExitError> {
    let mut request_file: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--request-file" => set_path(&mut request_file, value),
            _ => false,
        },
        &mut want_json,
    )?;
    let request_file = request_file.ok_or_else(|| ExitError::usage(usage()))?;
    let raw = read_bounded(
        &request_file,
        "domain discovery request",
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )?;
    let request: DomainPackDiscoveryRequestDocument = parse(&raw, &request_file)?;
    let projection = discover_domain_packs(&request).map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown discovery validation error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: discovery request rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack search", projection), want_json)
}

fn run_explain(args: &[String]) -> Result<(), ExitError> {
    let mut projection_file: Option<PathBuf> = None;
    let mut requirement_ref: Option<StableId> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--projection-file" => set_path(&mut projection_file, value),
            "--requirement-ref" => {
                requirement_ref = Some(StableId(value.to_owned()));
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let projection_file = projection_file.ok_or_else(|| ExitError::usage(usage()))?;
    let requirement_ref = requirement_ref.ok_or_else(|| ExitError::usage(usage()))?;
    let raw = read_bounded(
        &projection_file,
        "domain discovery projection",
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )?;
    let document: DomainPackDiscoveryProjectionDocument = parse(&raw, &projection_file)?;
    if !verify_domain_pack_discovery_projection(&document) {
        return Err(ExitError::invalid_value(
            "domain-pack: discovery projection digest or authority is invalid",
        ));
    }
    let projection = document.domain_pack_discovery_projection;
    let matches = projection
        .matches
        .into_iter()
        .filter(|candidate| candidate.requirement_ref == requirement_ref)
        .collect::<Vec<_>>();
    let gaps = projection
        .gaps
        .into_iter()
        .filter(|gap| gap.requirement_ref == requirement_ref)
        .collect::<Vec<_>>();
    if matches.is_empty() && gaps.is_empty() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: requirement '{}' is absent from the discovery projection",
            requirement_ref.0
        )));
    }
    crate::cli_util::emit_envelope(
        CliEnvelope::ok(
            "domain-pack explain",
            DomainPackDiscoveryExplanationPayload {
                request_id: projection.request_id,
                demand_digest: projection.demand_digest,
                requirement_ref,
                authority: DomainPackCandidateAuthority::CandidateOnly,
                assurance_binding: projection.assurance_binding,
                uncertainties: projection.uncertainties,
                matches,
                gaps,
                boundary: "explanation of an integrity-checked candidate projection; no trust, selection, installation, or activation authority",
            },
        ),
        want_json,
    )
}

/// Candidate-only authoring commands deliberately stop before signing,
/// publication, remote acquisition, trust, or lifecycle orchestration.
fn run_author(args: &[String]) -> Result<(), ExitError> {
    match args.first().map_or("--help", String::as_str) {
        "skeleton" => run_author_skeleton(&args[1..]),
        "test" => run_author_test(&args[1..]),
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core domain-pack author: unknown subcommand '{other}'\n{}",
            usage()
        ))),
    }
}

fn run_author_skeleton(args: &[String]) -> Result<(), ExitError> {
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h" | "help") {
        println!("{}", usage());
        return Ok(());
    }
    let mut request_file: Option<PathBuf> = None;
    let mut output_root: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--request-file" => set_unique_path(&mut request_file, value),
            "--output-root" => set_unique_path(&mut output_root, value),
            _ => false,
        },
        &mut want_json,
    )?;

    let request_file = required(request_file)?;
    let output_root = required(output_root)?;
    let request: DomainPackAuthorSkeletonRequestDocument =
        read_typed(&request_file, "domain-pack author skeleton request")?;
    let skeleton = generate_domain_pack_author_skeleton(&request);
    if let Some(template) = skeleton.domain_pack_author_skeleton.template.as_ref() {
        let composition_request_raw = yaml_serde::to_string(&template.composition_request)
            .map_err(|error| {
                ExitError::failed(format!(
                    "domain-pack: cannot serialize generated composition request template: {error}"
                ))
            })?
            .into_bytes();
        write_author_skeleton_template(
            &output_root,
            [
                (
                    template.manifest.artifact_ref.0.as_str(),
                    template.manifest.raw_bytes.as_slice(),
                ),
                (
                    template.content.content_ref.0.as_str(),
                    template.content.raw_bytes.as_slice(),
                ),
                (
                    template.license.artifact_ref.0.as_str(),
                    template.license.raw_bytes.as_slice(),
                ),
                (
                    "composition-request.yaml",
                    composition_request_raw.as_slice(),
                ),
            ],
        )?;
    }
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack author skeleton", skeleton),
        want_json,
    )
}

fn run_author_test(args: &[String]) -> Result<(), ExitError> {
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h" | "help") {
        println!("{}", usage());
        return Ok(());
    }
    let mut request_file: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--request-file" => set_unique_path(&mut request_file, value),
            _ => false,
        },
        &mut want_json,
    )?;

    let request_file = required(request_file)?;
    let request: DomainPackAuthorTestRequestDocument =
        read_typed(&request_file, "domain-pack author test request")?;
    let report = evaluate_domain_pack_author_test(&request);
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack author test", report),
        want_json,
    )
}

fn run_acquire(args: &[String]) -> Result<(), ExitError> {
    match args.first().map_or("--help", String::as_str) {
        "plan" => run_acquisition_plan(&args[1..]),
        "prepare" => run_acquisition_prepare(&args[1..]),
        "download" => run_acquisition_download(&args[1..]),
        "derive-initialized" => run_acquisition_derive_initialized(&args[1..]),
        "apply" => run_acquisition_apply(&args[1..]),
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core domain-pack acquire: unknown subcommand '{other}'\n{}",
            usage()
        ))),
    }
}

/// Derive a candidate-only initialized-project lifecycle request from exact
/// retained state. This command intentionally stops before trust, preflight,
/// authorization, artifact staging, or activation.
fn run_acquisition_derive_initialized(args: &[String]) -> Result<(), ExitError> {
    let mut intent_file: Option<PathBuf> = None;
    let mut candidate_input_file: Option<PathBuf> = None;
    let mut target_catalog_file: Option<PathBuf> = None;
    let mut approved_candidate: Option<String> = None;
    let mut state_root: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--intent-file" => set_unique_path(&mut intent_file, value),
            "--candidate-input-file" => set_unique_path(&mut candidate_input_file, value),
            "--target-catalog-file" => set_unique_path(&mut target_catalog_file, value),
            "--operator-approve-candidate" if approved_candidate.is_none() => {
                approved_candidate = Some(value.to_owned());
                true
            }
            "--state-root" => set_unique_path(&mut state_root, value),
            _ => false,
        },
        &mut want_json,
    )?;
    let state_root = state_root.unwrap_or_else(|| PathBuf::from(".forge-method"));

    let intent: DomainPackInitializedProjectIntentDocument =
        read_typed(&required(intent_file)?, "initialized-project intent")?;
    if intent.schema_version != DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION {
        return Err(ExitError::invalid_value(
            "domain-pack: initialized-project intent has unsupported schema",
        ));
    }
    match &intent.domain_pack_initialized_project_intent.operation {
        DomainPackInitializedProjectOperation::Install { selection }
        | DomainPackInitializedProjectOperation::Upgrade { selection, .. } => {
            let approved_candidate = approved_candidate.as_deref().ok_or_else(|| {
                ExitError::invalid_value(
                    "domain-pack: install and upgrade require --operator-approve-candidate naming the exact selected candidate",
                )
            })?;
            if approved_candidate != selection.candidate_id.0 {
                return Err(ExitError::invalid_value(
                    "domain-pack: explicit operator approval does not name the exact selected candidate id",
                ));
            }
        }
        _ if approved_candidate.is_some() => {
            return Err(ExitError::usage(
                "domain-pack: --operator-approve-candidate is only valid for install and upgrade",
            ));
        }
        _ => {}
    }
    let mut candidate_input: Option<DomainPackAcquisitionDerivationInput> = candidate_input_file
        .map(|path| read_typed(&path, "candidate acquisition derivation input"))
        .transpose()?;
    let mut target_catalog: Option<DomainPackAcquisitionCatalogDocument> = target_catalog_file
        .map(|path| read_typed(&path, "target acquisition catalog"))
        .transpose()?;

    let lifecycle = lock_domain_pack_lifecycle(&state_root).map_err(map_lifecycle_error)?;
    let source = lifecycle
        .initialized_project_source(&intent)
        .map_err(map_lifecycle_error)?;
    let material = match &intent.domain_pack_initialized_project_intent.operation {
        DomainPackInitializedProjectOperation::Install { selection }
        | DomainPackInitializedProjectOperation::Upgrade { selection, .. } => {
            let approved_candidate = approved_candidate.ok_or_else(|| {
                ExitError::invalid_value(
                    "domain-pack: install and upgrade require --operator-approve-candidate naming the exact selected candidate",
                )
            })?;
            if approved_candidate != selection.candidate_id.0 {
                return Err(ExitError::invalid_value(
                    "domain-pack: explicit operator approval does not name the exact selected candidate id",
                ));
            }
            if target_catalog.is_some() {
                return Err(ExitError::usage(
                    "domain-pack: --target-catalog-file is only valid for rebase-core",
                ));
            }
            DomainPackInitializedProjectDerivationMaterial::Candidate {
                acquisition: candidate_input.take().ok_or_else(|| {
                    ExitError::usage(
                        "domain-pack: install and upgrade require --candidate-input-file",
                    )
                })?,
            }
        }
        DomainPackInitializedProjectOperation::Remove { .. } => {
            if candidate_input.is_some() || target_catalog.is_some() || approved_candidate.is_some()
            {
                return Err(ExitError::usage(
                    "domain-pack: remove accepts only --intent-file and --state-root",
                ));
            }
            DomainPackInitializedProjectDerivationMaterial::CurrentGeneration {
                generation: source.active_generation.clone(),
            }
        }
        DomainPackInitializedProjectOperation::Rollback { .. } => {
            if candidate_input.is_some() || target_catalog.is_some() || approved_candidate.is_some()
            {
                return Err(ExitError::usage(
                    "domain-pack: rollback accepts only --intent-file and --state-root",
                ));
            }
            let target = source.rollback_target.clone().ok_or_else(|| {
                ExitError::invalid_value(
                    "domain-pack: retained rollback target is unavailable for the exact requested receipt and lock",
                )
            })?;
            DomainPackInitializedProjectDerivationMaterial::Rollback {
                target_lock: target.target_lock,
                target_generation: target.target_generation,
            }
        }
        DomainPackInitializedProjectOperation::RebaseCore { .. } => {
            if candidate_input.is_some() || approved_candidate.is_some() {
                return Err(ExitError::usage(
                    "domain-pack: rebase-core accepts --target-catalog-file but no candidate input or approval",
                ));
            }
            let target_catalog = target_catalog.take().ok_or_else(|| {
                ExitError::usage("domain-pack: rebase-core requires --target-catalog-file")
            })?;
            if target_catalog.schema_version != DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION {
                return Err(ExitError::invalid_value(
                    "domain-pack: target acquisition catalog has unsupported schema",
                ));
            }
            DomainPackInitializedProjectDerivationMaterial::RebaseCore {
                target_core: target_catalog.core.clone(),
                target_catalog,
            }
        }
    };
    let derivation = derive_domain_pack_initialized_project_lifecycle(
        &DomainPackInitializedProjectDerivationInput {
            intent,
            initialized_state: source.expected_state,
            active_lock: source.active_lock,
            active_generation: source.active_generation,
            material,
        },
    )
    .map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown initialized-project derivation error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: initialized-project derivation rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack acquire derive-initialized", derivation),
        want_json,
    )
}

fn run_acquisition_plan(args: &[String]) -> Result<(), ExitError> {
    let mut intent_file: Option<PathBuf> = None;
    let mut request_file: Option<PathBuf> = None;
    let mut projection_file: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--intent-file" => set_path(&mut intent_file, value),
            "--request-file" => set_path(&mut request_file, value),
            "--projection-file" => set_path(&mut projection_file, value),
            _ => false,
        },
        &mut want_json,
    )?;
    let intent_file = intent_file.ok_or_else(|| ExitError::usage(usage()))?;
    let request_file = request_file.ok_or_else(|| ExitError::usage(usage()))?;
    let projection_file = projection_file.ok_or_else(|| ExitError::usage(usage()))?;
    let intent: DomainPackAcquisitionIntentDocument =
        read_typed(&intent_file, "acquisition intent")?;
    let request: DomainPackDiscoveryRequestDocument =
        read_typed(&request_file, "domain discovery request")?;
    let discovery: DomainPackDiscoveryProjectionDocument =
        read_typed(&projection_file, "domain discovery projection")?;
    let plan = plan_domain_pack_acquisition(&DomainPackAcquisitionPlanningInput {
        intent,
        request,
        discovery,
    })
    .map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown acquisition planning error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: acquisition planning rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack acquire plan", plan), want_json)
}

fn run_acquisition_prepare(args: &[String]) -> Result<(), ExitError> {
    let mut intent_file: Option<PathBuf> = None;
    let mut request_file: Option<PathBuf> = None;
    let mut projection_file: Option<PathBuf> = None;
    let mut catalog_file: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--intent-file" => set_path(&mut intent_file, value),
            "--request-file" => set_path(&mut request_file, value),
            "--projection-file" => set_path(&mut projection_file, value),
            "--catalog-file" => set_path(&mut catalog_file, value),
            _ => false,
        },
        &mut want_json,
    )?;
    let planning_input = DomainPackAcquisitionPlanningInput {
        intent: read_typed(&required(intent_file)?, "acquisition intent")?,
        request: read_typed(&required(request_file)?, "domain discovery request")?,
        discovery: read_typed(&required(projection_file)?, "domain discovery projection")?,
    };
    let catalog: DomainPackAcquisitionCatalogDocument =
        read_typed(&required(catalog_file)?, "acquisition catalog")?;
    if catalog.schema_version != forge_core_contracts::DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION {
        return Err(ExitError::invalid_value(
            "domain-pack: acquisition catalog has unsupported schema",
        ));
    }
    let plan = plan_domain_pack_acquisition(&planning_input).map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown acquisition planning error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: acquisition preparation rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    let input = DomainPackAcquisitionDerivationInput {
        planning_input,
        plan,
        forge_core_version: catalog.forge_core_version,
        core: catalog.core,
        registry: catalog.registry,
        candidates: catalog.candidates,
    };
    derive_domain_pack_acquisition_inputs(&input).map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown acquisition derivation error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: acquisition catalog material rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack acquire prepare", input),
        want_json,
    )
}

/// Verify a complete selected candidate artifact set from an operator-provisioned
/// local root. This is deliberately a candidate-byte handoff only: no local
/// source binding is retained, and no trust, review, lifecycle, cache, install,
/// or activation authority is opened.
fn run_acquisition_download(args: &[String]) -> Result<(), ExitError> {
    let mut intent_file: Option<PathBuf> = None;
    let mut request_file: Option<PathBuf> = None;
    let mut projection_file: Option<PathBuf> = None;
    let mut catalog_file: Option<PathBuf> = None;
    let mut remote_request_file: Option<PathBuf> = None;
    let mut remote_plan_file: Option<PathBuf> = None;
    let mut cache_projection_file: Option<PathBuf> = None;
    let mut catalog_facts_file: Option<PathBuf> = None;
    let mut artifact_root: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--intent-file" => set_unique_path(&mut intent_file, value),
            "--request-file" => set_unique_path(&mut request_file, value),
            "--projection-file" => set_unique_path(&mut projection_file, value),
            "--catalog-file" => set_unique_path(&mut catalog_file, value),
            "--remote-request-file" => set_unique_path(&mut remote_request_file, value),
            "--remote-plan-file" => set_unique_path(&mut remote_plan_file, value),
            "--cache-projection-file" => set_unique_path(&mut cache_projection_file, value),
            "--catalog-facts-file" => set_unique_path(&mut catalog_facts_file, value),
            "--artifact-root" => set_unique_path(&mut artifact_root, value),
            _ => false,
        },
        &mut want_json,
    )?;

    let planning_input = DomainPackAcquisitionPlanningInput {
        intent: read_typed(&required(intent_file)?, "acquisition intent")?,
        request: read_typed(&required(request_file)?, "domain discovery request")?,
        discovery: read_typed(&required(projection_file)?, "domain discovery projection")?,
    };
    let catalog: DomainPackAcquisitionCatalogDocument =
        read_typed(&required(catalog_file)?, "acquisition catalog")?;
    if catalog.schema_version != DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION {
        return Err(ExitError::invalid_value(
            "domain-pack: acquisition catalog has unsupported schema",
        ));
    }
    let remote_request: DomainPackRemoteAcquisitionRequestDocument = read_typed(
        &required(remote_request_file)?,
        "remote acquisition request",
    )?;
    let remote_plan: DomainPackRemoteAcquisitionPlanDocument =
        read_typed(&required(remote_plan_file)?, "remote acquisition plan")?;
    let cache_projection: DomainPackRemoteCacheProjectionDocument =
        read_typed(&required(cache_projection_file)?, "local cache projection")?;
    let catalog_facts: DomainPackRemoteCatalogFactsFile =
        read_typed(&required(catalog_facts_file)?, "host catalog facts")?;
    let artifact_root = required(artifact_root)?;

    if remote_request
        .domain_pack_remote_acquisition_request
        .catalog
        .registry
        != catalog.registry
    {
        return Err(ExitError::conflict(
            "domain-pack: remote request catalog differs from the exact C6.1 acquisition catalog",
        ));
    }

    let acquisition_plan = plan_domain_pack_acquisition(&planning_input).map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown acquisition planning error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: acquisition download planning rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    let derivation_input = DomainPackAcquisitionDerivationInput {
        planning_input: planning_input.clone(),
        plan: acquisition_plan.clone(),
        forge_core_version: catalog.forge_core_version,
        core: catalog.core,
        registry: catalog.registry,
        candidates: catalog.candidates,
    };
    let acquisition_derivation = derive_domain_pack_acquisition_inputs(&derivation_input).map_err(
        |rejection| {
            let first = rejection.issues.first().map_or_else(
                || "unknown acquisition derivation error".to_owned(),
                |issue| format!("{}: {}", issue.path, issue.message),
            );
            ExitError::invalid_value(format!(
                "domain-pack: acquisition download derivation rejected with {} issue(s); first: {first}",
                rejection.issues.len()
            ))
        },
    )?;
    let remote_planning = DomainPackRemoteAcquisitionPlanningInput {
        acquisition: planning_input,
        acquisition_plan: acquisition_plan.clone(),
        request: remote_request,
        cache_projection: Some(cache_projection),
        catalog_facts: DomainPackRemoteCatalogFacts {
            snapshot_digest: catalog_facts.snapshot_digest,
            availability: catalog_facts.availability.into(),
            host_checked_at_unix: catalog_facts.host_checked_at_unix,
        },
    };
    if !verify_domain_pack_remote_acquisition_plan(&remote_planning, &remote_plan) {
        return Err(ExitError::conflict(
            "domain-pack: remote acquisition plan is stale, replayed, or does not exactly match the supplied candidate inputs",
        ));
    }
    let remote_decision = plan_domain_pack_remote_acquisition(&remote_planning).map_err(
        |rejection| {
            let first = rejection.issues.first().map_or_else(
                || "unknown remote acquisition planning error".to_owned(),
                |issue| format!("{}: {}", issue.path, issue.message),
            );
            ExitError::invalid_value(format!(
                "domain-pack: remote acquisition planning rejected with {} issue(s); first: {first}",
                rejection.issues.len()
            ))
        },
    )?;

    let remote_plan_body = &remote_plan.domain_pack_remote_acquisition_plan;
    let attempts = if matches!(
        remote_plan_body.outcome,
        forge_core_contracts::DomainPackRemoteAcquisitionPlanOutcome::Blocked
    ) {
        Vec::new()
    } else {
        if !remote_decision.network_fetches.is_empty()
            || !remote_decision.transport_attempts.is_empty()
            || !remote_decision.local_mirror_reads.is_empty()
            || remote_decision.cache_reads.len() != remote_plan_body.artifacts.len()
        {
            return Err(ExitError::invalid_value(
                "domain-pack: local-only download requires a complete cache-only remote plan; network and transport attempts are forbidden",
            ));
        }
        read_local_remote_cache_attempts(
            &artifact_root,
            &remote_plan_body.request_digest,
            &remote_decision.cache_reads,
        )?
    };
    let handoff = verify_domain_pack_remote_fetches(&DomainPackRemoteFetchVerificationInput {
        planning: remote_planning,
        plan: remote_plan.clone(),
        attempts,
    })
    .map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown remote fetch verification error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: remote fetch verification rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    let status = match handoff.receipt.domain_pack_remote_fetch_receipt.outcome {
        forge_core_contracts::DomainPackRemoteFetchOutcome::CandidateBytesVerified => {
            DomainPackCandidateByteDownloadStatus::ReadyForTrustEvaluation
        }
        forge_core_contracts::DomainPackRemoteFetchOutcome::Blocked => {
            DomainPackCandidateByteDownloadStatus::Blocked
        }
    };
    crate::cli_util::emit_envelope(
        CliEnvelope::ok(
            "domain-pack acquire download",
            DomainPackCandidateByteDownloadPayload {
                status,
                authority: DomainPackCandidateAuthority::CandidateOnly,
                acquisition_plan,
                acquisition_derivation,
                remote_plan: handoff.plan,
                evidence: handoff.evidence,
                receipt: handoff.receipt,
                artifact_set_digest: handoff.artifact_set_digest,
                boundary: "candidate-only immutable byte verification; no trust decision, reviewed promotion, lifecycle preflight, receipt, generation, install, activation, cache mutation, anchor advancement, or operator-source binding",
            },
        ),
        want_json,
    )
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn run_acquisition_apply(args: &[String]) -> Result<(), ExitError> {
    let mut derivation_input_file: Option<PathBuf> = None;
    let mut approved_candidate: Option<String> = None;
    let mut trust_policy_file: Option<PathBuf> = None;
    let mut registry_file: Option<PathBuf> = None;
    let mut reviewer_registry_file: Option<PathBuf> = None;
    let mut reviewed_registry_file: Option<PathBuf> = None;
    let mut capability_registry_file: Option<PathBuf> = None;
    let mut sandbox_policy_file: Option<PathBuf> = None;
    let mut principal_id: Option<StableId> = None;
    let mut project_root: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut state_root = PathBuf::from(".forge-method");
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--derivation-input-file" => set_path(&mut derivation_input_file, value),
            "--operator-approve-candidate" => {
                approved_candidate = Some(value.to_owned());
                true
            }
            "--trust-policy-file" => set_path(&mut trust_policy_file, value),
            "--registry-file" => set_path(&mut registry_file, value),
            "--reviewer-registry-file" => set_path(&mut reviewer_registry_file, value),
            "--reviewed-registry-file" => set_path(&mut reviewed_registry_file, value),
            "--capability-registry-file" => set_path(&mut capability_registry_file, value),
            "--sandbox-policy-file" => set_path(&mut sandbox_policy_file, value),
            "--principal-id" => {
                principal_id = Some(StableId(value.to_owned()));
                true
            }
            "--project-root" => set_path(&mut project_root, value),
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            "--state-root" => {
                state_root = PathBuf::from(value);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let derivation_input_file = required(derivation_input_file)?;
    let approved_candidate = approved_candidate.ok_or_else(|| ExitError::usage(usage()))?;
    let trust_policy_file = required(trust_policy_file)?;
    let registry_file = required(registry_file)?;
    let reviewer_registry_file = required(reviewer_registry_file)?;
    let reviewed_registry_file = required(reviewed_registry_file)?;
    let capability_registry_file = required(capability_registry_file)?;
    let sandbox_policy_file = required(sandbox_policy_file)?;
    let principal_id = principal_id.ok_or_else(|| ExitError::usage(usage()))?;
    let project_root = required(project_root)?;

    let input: DomainPackAcquisitionDerivationInput =
        read_typed(&derivation_input_file, "acquisition derivation input")?;
    let plan = &input.plan.domain_pack_acquisition_plan;
    if approved_candidate != plan.selected.candidate_id.0 {
        return Err(ExitError::invalid_value(
            "domain-pack: explicit operator approval does not name the exact planned candidate id",
        ));
    }
    let derived = derive_domain_pack_acquisition_inputs(&input).map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown acquisition derivation error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: acquisition derivation rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    let derived = derived.domain_pack_acquisition_derived_inputs;

    let controlled_roots = canonical_lifecycle_roots(&project_root, &artifact_root, &state_root)?;
    let trust_policy_file = trusted_external_file(
        &trust_policy_file,
        "operator trust policy",
        &controlled_roots,
    )?;
    let registry_file = trusted_external_file(
        &registry_file,
        "signed supply-chain registry",
        &controlled_roots,
    )?;
    let reviewer_registry_file = trusted_external_file(
        &reviewer_registry_file,
        "signed reviewer registry",
        &controlled_roots,
    )?;
    let reviewed_registry_file = trusted_external_file(
        &reviewed_registry_file,
        "dual-signed reviewed registry",
        &controlled_roots,
    )?;
    let capability_registry_file = trusted_external_file(
        &capability_registry_file,
        "runtime capability registry",
        &controlled_roots,
    )?;
    let sandbox_policy_file = trusted_external_file(
        &sandbox_policy_file,
        "capability sandbox policy",
        &controlled_roots,
    )?;
    let trust_policy: DomainPackTrustPolicyDocument =
        read_typed(&trust_policy_file, "operator trust policy")?;
    let registry: DomainPackSupplyChainRegistryDocument =
        read_typed(&registry_file, "signed supply-chain registry")?;
    if registry != input.registry {
        return Err(ExitError::invalid_value(
            "domain-pack: operator registry differs from the exact acquisition derivation input",
        ));
    }
    let capability_registry: DomainPackRuntimeCapabilityRegistryDocument =
        read_typed(&capability_registry_file, "runtime capability registry")?;
    let sandbox_policy: DomainPackCapabilitySandboxPolicyDocument =
        read_typed(&sandbox_policy_file, "capability sandbox policy")?;
    let project_snapshot_digest = domain_pack_project_snapshot_digest(&controlled_roots.project)
        .map_err(map_lifecycle_error)?;
    if project_snapshot_digest != plan.expected_project_snapshot_digest {
        return Err(ExitError::conflict(
            "domain-pack: project snapshot changed after candidate acquisition planning",
        ));
    }

    let mut operator_anchor = lock_operator_registry_anchor(&registry_file)?;
    for (path, label) in [
        (&trust_policy_file, "operator trust policy"),
        (&reviewer_registry_file, "signed reviewer registry"),
        (&reviewed_registry_file, "dual-signed reviewed registry"),
        (&capability_registry_file, "runtime capability registry"),
        (&sandbox_policy_file, "capability sandbox policy"),
    ] {
        require_direct_operator_file(path, &operator_anchor.operator_root, label)?;
    }
    let now_unix = trusted_now_unix()?;
    let verified_snapshot =
        verify_domain_pack_supply_chain_snapshot(&trust_policy, &registry, now_unix).map_err(
            |error| {
                ExitError::with_code(
                    2,
                    format!("domain-pack: supply-chain verification rejected: {error}"),
                )
            },
        )?;
    let mut registry_anchor = load_operator_registry_anchor(&operator_anchor)?;
    let operator_root = operator_anchor.operator_root.clone();
    let anchored_snapshot = advance_operator_registry_anchor(
        &mut operator_anchor,
        &mut registry_anchor,
        verified_snapshot,
    )?;
    let reviewed_guard = crate::domain_pack_learning_cmd::lock_reviewed_snapshot_for_lifecycle(
        &operator_root,
        &reviewer_registry_file,
        &reviewed_registry_file,
        now_unix,
    )?;
    persist_domain_pack_operator_sources(
        &state_root,
        &DomainPackOperatorSourceBinding {
            schema_version: DOMAIN_PACK_OPERATOR_SOURCE_SCHEMA_VERSION.to_owned(),
            operator_root: normalized_path(&operator_root),
            trust_policy_file: normalized_path(&trust_policy_file),
            registry_file: normalized_path(&registry_file),
            reviewer_registry_file: normalized_path(&reviewer_registry_file),
            reviewed_registry_file: normalized_path(&reviewed_registry_file),
            capability_registry_file: normalized_path(&capability_registry_file),
            sandbox_policy_file: normalized_path(&sandbox_policy_file),
            artifact_root: normalized_path(&controlled_roots.artifacts),
        },
    )?;

    let resolution_request = derived.resolution_request;
    let composition_request = derived.composition_request;
    let mut resolution = resolve_domain_packs(&resolution_request, &registry);
    if resolution.domain_pack_resolution_projection.status != DomainPackResolutionStatus::Resolved
        || !resolution
            .domain_pack_resolution_projection
            .issues
            .is_empty()
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: exact acquisition resolution is no longer clean",
        ));
    }
    let reviewed_join =
        join_reviewed_registry_to_resolution(&resolution, reviewed_guard.snapshot().registry());
    if reviewed_join.reviewed_registry_digest != reviewed_guard.snapshot().registry_digest() {
        return Err(ExitError::with_code(
            2,
            "domain-pack: reviewed-registry join differs from the anchored reviewed snapshot",
        ));
    }
    let mut assessments = Vec::new();
    for selected in &mut resolution.domain_pack_resolution_projection.selected {
        let record = anchored_snapshot
            .verified_snapshot()
            .entries()
            .iter()
            .map(forge_core_authority::VerifiedDomainPackSupplyChainEntry::record)
            .find(|record| {
                record.record_digest == selected.registry_record_digest
                    && record.identity == selected.identity
                    && record.package_digest == selected.package.package_digest
                    && record.manifest_digest == selected.package.manifest.canonical_sha256
                    && record.content_digest == selected.package.content.canonical_sha256
                    && record.license_digest == selected.package.license.canonical_sha256
                    && record.fixture_digests
                        == selected
                            .package
                            .fixtures
                            .iter()
                            .map(|fixture| fixture.canonical_sha256.clone())
                            .collect::<Vec<_>>()
            })
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: selected package is absent from the verified supply-chain snapshot",
                )
            })?;
        let join = reviewed_join
            .joins
            .iter()
            .find(|join| {
                join.publisher == selected.identity.publisher.0
                    && join.name == selected.identity.name.0
                    && join.version == selected.identity.version
                    && join.package_digest == selected.package.package_digest
                    && join.registry_record_digest == selected.registry_record_digest
            })
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: selected package is absent from the reviewed-registry join",
                )
            })?;
        if join.status != DomainPackReviewedResolutionJoinStatus::EligibleReviewed {
            return Err(ExitError::with_code(
                2,
                "domain-pack: selected package is not currently eligible and reviewed",
            ));
        }
        let entry_digest = join.reviewed_entry_digest.as_ref().ok_or_else(|| {
            ExitError::with_code(2, "domain-pack: eligible reviewed join lacks entry digest")
        })?;
        let entry = reviewed_guard
            .snapshot()
            .registry()
            .domain_pack_reviewed_registry
            .entries
            .iter()
            .find(|entry| &entry.entry_digest == entry_digest)
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: reviewed entry is absent from anchored registry",
                )
            })?;
        selected.source_assurance = DomainPackSourceAssurance::SupplyChainVerified;
        selected.semantic_assurance = DomainPackSemanticAssurance::Reviewed;
        selected.reviewed_entry_digest = Some(entry.entry_digest.clone());
        selected.promotion_authorization_digest = Some(entry.authorization_digest.clone());
        assessments.push(DomainPackSupplyChainAssessment {
            package_digest: record.package_digest.clone(),
            registry_record_digest: record.record_digest.clone(),
            publisher_signature_verified: true,
            registry_signature_threshold_verified: true,
            namespace_grant_verified: true,
            revoked: false,
        });
    }
    let promoted = &mut resolution.domain_pack_resolution_projection;
    promoted.resolution_digest = domain_pack_resolution_projection_digest(
        &resolution_request,
        anchored_snapshot.verified_snapshot().snapshot_digest(),
        promoted,
    );

    let owned_materials = load_composition_materials(&composition_request, &artifact_root)?;
    let materials = material_views(&composition_request, &owned_materials);
    let composition = compose_domain_packs(&composition_request, &materials);
    if !composition
        .domain_pack_composition_projection
        .issues
        .is_empty()
        || !composition
            .domain_pack_composition_projection
            .gaps
            .is_empty()
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: exact acquisition composition is not clean and composable",
        ));
    }
    let selected_for_trust = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .zip(&assessments)
        .map(|(package, assessment)| {
            Ok(DomainPackTrustSelectedPackage {
                package: package.clone(),
                structurally_valid: true,
                supply_chain: assessment.clone(),
                capability_demands: derive_domain_pack_capability_demands(
                    package,
                    &composition_request.domain_pack_composition_request,
                )
                .map_err(map_lifecycle_error)?,
            })
        })
        .collect::<Result<Vec<_>, ExitError>>()?;
    let trust_input = DomainPackTrustEvaluationInput {
        project_id: resolution_request
            .domain_pack_resolution_request
            .project_id
            .clone(),
        selected: selected_for_trust,
        trust_policy: trust_policy.domain_pack_trust_policy.clone(),
        capability_registry: capability_registry
            .domain_pack_runtime_capability_registry
            .clone(),
        sandbox_policy: sandbox_policy.domain_pack_capability_sandbox_policy.clone(),
    };
    let trust = evaluate_domain_pack_trust(&trust_input);
    if trust.status != DomainPackTrustEvaluationStatus::Approved || !trust.issues.is_empty() {
        return Err(ExitError::with_code(
            2,
            format!(
                "domain-pack: trust/capability evaluation blocked: {:?}",
                trust.issues
            ),
        ));
    }

    let lock_payload = DomainPackExactLockPayload {
        project_id: resolution_request
            .domain_pack_resolution_request
            .project_id
            .clone(),
        core: resolution_request
            .domain_pack_resolution_request
            .core
            .clone(),
        requirements_digest: canonical_digest(
            &resolution_request
                .domain_pack_resolution_request
                .requirements,
        )?,
        roots: resolution_request
            .domain_pack_resolution_request
            .roots
            .clone(),
        registry_snapshot_digest: anchored_snapshot
            .verified_snapshot()
            .snapshot_digest()
            .to_owned(),
        reviewer_registry_digest: reviewed_guard
            .snapshot()
            .reviewer_registry_digest()
            .to_owned(),
        reviewed_registry_digest: reviewed_guard.snapshot().registry_digest().to_owned(),
        trust_policy_digest: anchored_snapshot
            .verified_snapshot()
            .trust_policy_digest()
            .to_owned(),
        capability_registry_digest: canonical_digest(&capability_registry)?,
        sandbox_policy_digest: canonical_digest(&sandbox_policy)?,
        resolution_digest: resolution
            .domain_pack_resolution_projection
            .resolution_digest
            .clone(),
        composition_digest: composition
            .domain_pack_composition_projection
            .composition_digest
            .clone(),
        packages: resolution
            .domain_pack_resolution_projection
            .selected
            .iter()
            .map(locked_package_from_resolution)
            .collect(),
        verified_capability_bindings: trust.verified_capability_bindings,
        unresolved_composition_gaps: composition.domain_pack_composition_projection.gaps.clone(),
        unresolved_capability_gaps: trust.capability_gaps.clone(),
    };
    let proposed_lock = DomainPackExactLockDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_exact_lock: DomainPackExactLock {
            lock_digest: canonical_digest(&lock_payload)?,
            payload: lock_payload,
        },
    };
    let mut lifecycle = lock_domain_pack_lifecycle(&state_root).map_err(map_lifecycle_error)?;
    if lifecycle.projection().active_pointer.is_some() {
        return Err(ExitError::conflict(
            "domain-pack: acquire apply is clean-install-only; use intent-specific lifecycle upgrade, rollback, or remove for initialized state",
        ));
    }
    let expected_state = DomainPackExpectedLifecycleState::Uninitialized {
        project_snapshot_digest: project_snapshot_digest.clone(),
    };
    let operation = DomainPackLifecycleOperation::Install {
        root: resolution_request.domain_pack_resolution_request.roots[0]
            .pack
            .clone(),
    };
    let lifecycle_request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: StableId(format!("{}.lifecycle", plan.acquisition_id.0)),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: resolution_request
                .domain_pack_resolution_request
                .project_id
                .clone(),
            principal_id,
            operation: operation.clone(),
            expected_state: expected_state.clone(),
            resolution_request_digest: canonical_digest(&resolution_request)?,
            project_snapshot_digest: project_snapshot_digest.clone(),
        },
    };
    let compatibility = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: StableId(format!("{}.compatibility", plan.acquisition_id.0)),
        operation,
        sealed_core: resolution_request
            .domain_pack_resolution_request
            .core
            .clone(),
        from_lock: None,
        to_lock: proposed_lock.clone(),
    });
    if compatibility.domain_pack_compatibility_report.status
        != DomainPackCompatibilityStatus::Compatible
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: acquisition compatibility evaluation blocked",
        ));
    }
    let mut staged_artifacts = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .flat_map(|package| {
            let content = DomainPackArtifactBinding {
                artifact_ref: package.package.content.content_ref.clone(),
                raw_sha256: package.package.content.raw_sha256.clone(),
                canonical_sha256: package.package.content.canonical_sha256.clone(),
            };
            std::iter::once(package.package.manifest.clone())
                .chain(std::iter::once(content))
                .chain(std::iter::once(package.package.license.clone()))
                .chain(package.package.fixtures.iter().cloned())
        })
        .collect::<Vec<_>>();
    staged_artifacts.sort_by(|left, right| left.artifact_ref.0.cmp(&right.artifact_ref.0));
    staged_artifacts.dedup();
    let mut preflight = DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: StableId(format!("{}.preflight", plan.acquisition_id.0)),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request_digest: canonical_digest(&lifecycle_request)?,
            request: lifecycle_request,
            observed_state: expected_state,
            resolution,
            proposed_lock,
            composition,
            supply_chain_assessments: assessments,
            trust_decisions: trust.trust_decisions,
            capability_gaps: trust.capability_gaps,
            compatibility_report: compatibility,
            staged_artifacts,
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: vec![],
            preflight_digest: String::new(),
        },
    };
    preflight.domain_pack_lifecycle_preflight.preflight_digest = canonical_digest(&preflight)?;
    let owned_artifacts = load_immutable_artifacts(&preflight, &artifact_root)?;
    let artifacts = immutable_artifact_views(&owned_artifacts);
    let project_snapshot =
        verify_domain_pack_project_snapshot(&controlled_roots.project, &project_snapshot_digest)
            .map_err(map_lifecycle_error)?;
    let prepared = lifecycle
        .prepare_candidate(preflight)
        .map_err(map_lifecycle_error)?;
    let context = DomainPackLifecycleAuthorizationContext {
        anchored_snapshot: &anchored_snapshot,
        anchored_reviewed_snapshot: reviewed_guard.snapshot(),
        project_snapshot: &project_snapshot,
        trust_policy_document: &trust_policy,
        registry_document: &registry,
        resolution_request: &resolution_request,
        composition_request: &composition_request,
        materials: &materials,
        artifacts: &artifacts,
        trust_input: &trust_input,
    };
    let authority = authorize_prepared_domain_pack_lifecycle(&prepared, &context)
        .map_err(map_lifecycle_error)?;
    let receipt = lifecycle
        .commit(prepared, authority)
        .map_err(map_lifecycle_error)?;
    drop(reviewed_guard);
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack acquire apply", receipt),
        want_json,
    )
}

fn locked_package_from_resolution(
    selected: &forge_core_contracts::DomainPackResolvedPackage,
) -> DomainPackLockedPackage {
    DomainPackLockedPackage {
        identity: selected.identity.clone(),
        package_digest: selected.package.package_digest.clone(),
        manifest_binding: selected.package.manifest.clone(),
        content_binding: selected.package.content.clone(),
        license_binding: selected.package.license.clone(),
        fixture_bindings: selected.package.fixtures.clone(),
        namespace_grant_id: selected.namespace_grant_id.clone(),
        registry_record_digest: selected.registry_record_digest.clone(),
        source_assurance: selected.source_assurance,
        semantic_assurance: selected.semantic_assurance,
        reviewed_entry_digest: selected.reviewed_entry_digest.clone(),
        promotion_authorization_digest: selected.promotion_authorization_digest.clone(),
        dependencies: selected.dependencies.clone(),
        deterministic_order: selected.deterministic_order,
    }
}

/// Commit one exact target-Core lifecycle generation from operator roots that
/// were bound during the original high-level acquisition. Workflow authority
/// is advanced separately by the joined ledger event after this returns.
#[allow(clippy::similar_names)] // Reviewer identity and reviewed-pack authority are distinct registries.
pub(crate) fn apply_domain_pack_core_rebase(
    project_root: &Path,
    state_root: &Path,
    plan: &DomainPackRebasePlanDocument,
    target_core: &DomainPackCoreBinding,
    principal_id: StableId,
) -> Result<DomainPackLifecycleReceiptDocument, ExitError> {
    if !verify_domain_pack_rebase_plan(plan) {
        return Err(ExitError::conflict(
            "domain-pack: rebase plan failed integrity verification",
        ));
    }
    let plan = &plan.domain_pack_rebase_plan;
    if &plan.target_core != target_core {
        return Err(ExitError::invalid_value(
            "domain-pack: rebase plan is not an apply-authorized target-Core plan",
        ));
    }
    let sources = load_domain_pack_operator_sources(state_root)?;
    let artifact_root = PathBuf::from(&sources.artifact_root);
    let controlled_roots = canonical_lifecycle_roots(project_root, &artifact_root, state_root)?;
    let trust_policy_file = trusted_external_file(
        Path::new(&sources.trust_policy_file),
        "operator trust policy",
        &controlled_roots,
    )?;
    let registry_file = trusted_external_file(
        Path::new(&sources.registry_file),
        "signed supply-chain registry",
        &controlled_roots,
    )?;
    let reviewer_registry_file = trusted_external_file(
        Path::new(&sources.reviewer_registry_file),
        "signed reviewer registry",
        &controlled_roots,
    )?;
    let reviewed_registry_file = trusted_external_file(
        Path::new(&sources.reviewed_registry_file),
        "dual-signed reviewed registry",
        &controlled_roots,
    )?;
    let capability_registry_file = trusted_external_file(
        Path::new(&sources.capability_registry_file),
        "runtime capability registry",
        &controlled_roots,
    )?;
    let sandbox_policy_file = trusted_external_file(
        Path::new(&sources.sandbox_policy_file),
        "capability sandbox policy",
        &controlled_roots,
    )?;
    let operator_root = std::fs::canonicalize(&sources.operator_root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve persisted operator root: {error}"
        ))
    })?;
    if normalized_path(&operator_root) != sources.operator_root {
        return Err(ExitError::invalid_value(
            "domain-pack: persisted operator root is no longer canonical",
        ));
    }
    for (path, label) in [
        (&trust_policy_file, "operator trust policy"),
        (&registry_file, "signed supply-chain registry"),
        (&reviewer_registry_file, "signed reviewer registry"),
        (&reviewed_registry_file, "dual-signed reviewed registry"),
        (&capability_registry_file, "runtime capability registry"),
        (&sandbox_policy_file, "capability sandbox policy"),
    ] {
        require_direct_operator_file(path, &operator_root, label)?;
    }
    let project_snapshot_digest = domain_pack_project_snapshot_digest(&controlled_roots.project)
        .map_err(map_lifecycle_error)?;
    if project_snapshot_digest != plan.exact_cas.expected_project_snapshot_digest {
        return Err(ExitError::conflict(
            "domain-pack: project snapshot changed after rebase planning",
        ));
    }

    let trust_policy: DomainPackTrustPolicyDocument =
        read_typed(&trust_policy_file, "operator trust policy")?;
    let registry: DomainPackSupplyChainRegistryDocument =
        read_typed(&registry_file, "signed supply-chain registry")?;
    let capability_registry: DomainPackRuntimeCapabilityRegistryDocument =
        read_typed(&capability_registry_file, "runtime capability registry")?;
    let sandbox_policy: DomainPackCapabilitySandboxPolicyDocument =
        read_typed(&sandbox_policy_file, "capability sandbox policy")?;
    let mut operator_anchor = lock_operator_registry_anchor(&registry_file)?;
    let now_unix = trusted_now_unix()?;
    let verified_snapshot =
        verify_domain_pack_supply_chain_snapshot(&trust_policy, &registry, now_unix).map_err(
            |error| {
                ExitError::with_code(
                    2,
                    format!("domain-pack: supply-chain verification rejected: {error}"),
                )
            },
        )?;
    if verified_snapshot.snapshot_digest() != plan.exact_cas.expected_supply_chain_registry_digest {
        return Err(ExitError::conflict(
            "domain-pack: supply-chain registry changed after rebase planning",
        ));
    }
    let mut registry_anchor = load_operator_registry_anchor(&operator_anchor)?;
    let anchored_snapshot = advance_operator_registry_anchor(
        &mut operator_anchor,
        &mut registry_anchor,
        verified_snapshot,
    )?;
    let reviewed_guard = crate::domain_pack_learning_cmd::lock_reviewed_snapshot_for_lifecycle(
        &operator_root,
        &reviewer_registry_file,
        &reviewed_registry_file,
        now_unix,
    )?;
    if reviewed_guard.snapshot().reviewer_registry_digest()
        != plan.exact_cas.expected_reviewer_registry_digest
        || reviewed_guard.snapshot().registry_digest()
            != plan.exact_cas.expected_reviewed_registry_digest
    {
        return Err(ExitError::conflict(
            "domain-pack: reviewer or reviewed registry changed after rebase planning",
        ));
    }

    persist_domain_pack_rebase_plan(
        state_root,
        &DomainPackRebasePlanDocument {
            schema_version: forge_core_contracts::DOMAIN_PACK_REBASE_SCHEMA_VERSION.to_owned(),
            domain_pack_rebase_plan: plan.clone(),
        },
    )?;
    let mut lifecycle = lock_domain_pack_lifecycle(state_root).map_err(map_lifecycle_error)?;
    let source = lifecycle
        .active_rebase_source()
        .map_err(map_lifecycle_error)?;
    let source_pointer = &source.pointer.domain_pack_active_pointer;
    if source_pointer.generation != plan.exact_cas.expected_generation
        || source_pointer.pointer_digest != plan.exact_cas.expected_lifecycle_pointer_digest
        || source_pointer.lifecycle_head_digest != plan.exact_cas.expected_lifecycle_head_digest
        || source_pointer.active_lock_digest != plan.exact_cas.expected_active_lock_digest
        || source.exact_lock.domain_pack_exact_lock.payload.core != plan.source_core
    {
        return Err(ExitError::conflict(
            "domain-pack: active lifecycle generation changed after rebase planning",
        ));
    }
    let source_lock = &source.exact_lock.domain_pack_exact_lock.payload;
    if source_lock.registry_snapshot_digest != plan.exact_cas.expected_supply_chain_registry_digest
        || source_lock.reviewer_registry_digest != plan.exact_cas.expected_reviewer_registry_digest
        || source_lock.reviewed_registry_digest != plan.exact_cas.expected_reviewed_registry_digest
        || canonical_digest(&trust_policy)? != source_lock.trust_policy_digest
        || canonical_digest(&capability_registry)? != source_lock.capability_registry_digest
        || canonical_digest(&sandbox_policy)? != source_lock.sandbox_policy_digest
    {
        return Err(ExitError::conflict(
            "domain-pack: fresh operator policy no longer matches the active exact lock",
        ));
    }

    let mut resolution_request = source.resolution_request.clone();
    resolution_request.domain_pack_resolution_request.request_id = StableId(format!(
        "domain-pack.rebase.resolution.{}",
        source_pointer.generation + 1
    ));
    resolution_request.domain_pack_resolution_request.core = target_core.clone();
    resolution_request
        .domain_pack_resolution_request
        .roots
        .clone_from(&source_lock.roots);
    resolution_request
        .domain_pack_resolution_request
        .current_lock = None;
    resolution_request
        .domain_pack_resolution_request
        .registry_snapshot_digest
        .clone_from(&source_lock.registry_snapshot_digest);
    let mut resolution = resolve_domain_packs(&resolution_request, &registry);
    if resolution.domain_pack_resolution_projection.status != DomainPackResolutionStatus::Resolved
        || !resolution
            .domain_pack_resolution_projection
            .issues
            .is_empty()
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: target-Core re-resolution is blocked",
        ));
    }
    let reviewed_join =
        join_reviewed_registry_to_resolution(&resolution, reviewed_guard.snapshot().registry());
    let mut assessments = Vec::new();
    for selected in &mut resolution.domain_pack_resolution_projection.selected {
        let record = anchored_snapshot
            .verified_snapshot()
            .entries()
            .iter()
            .map(forge_core_authority::VerifiedDomainPackSupplyChainEntry::record)
            .find(|record| {
                record.record_digest == selected.registry_record_digest
                    && record.identity == selected.identity
                    && record.package_digest == selected.package.package_digest
                    && record.manifest_digest == selected.package.manifest.canonical_sha256
                    && record.content_digest == selected.package.content.canonical_sha256
                    && record.license_digest == selected.package.license.canonical_sha256
                    && record.fixture_digests
                        == selected
                            .package
                            .fixtures
                            .iter()
                            .map(|fixture| fixture.canonical_sha256.clone())
                            .collect::<Vec<_>>()
            })
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: target-Core package is absent from verified supply chain",
                )
            })?;
        let join = reviewed_join
            .joins
            .iter()
            .find(|join| join.registry_record_digest == selected.registry_record_digest)
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: target-Core package is absent from reviewed join",
                )
            })?;
        if join.status != DomainPackReviewedResolutionJoinStatus::EligibleReviewed {
            return Err(ExitError::with_code(
                2,
                "domain-pack: target-Core package is not eligible-reviewed",
            ));
        }
        let entry = reviewed_guard
            .snapshot()
            .registry()
            .domain_pack_reviewed_registry
            .entries
            .iter()
            .find(|entry| join.reviewed_entry_digest.as_ref() == Some(&entry.entry_digest))
            .ok_or_else(|| {
                ExitError::with_code(2, "domain-pack: reviewed entry is absent from anchor")
            })?;
        selected.source_assurance = DomainPackSourceAssurance::SupplyChainVerified;
        selected.semantic_assurance = DomainPackSemanticAssurance::Reviewed;
        selected.reviewed_entry_digest = Some(entry.entry_digest.clone());
        selected.promotion_authorization_digest = Some(entry.authorization_digest.clone());
        assessments.push(DomainPackSupplyChainAssessment {
            package_digest: record.package_digest.clone(),
            registry_record_digest: record.record_digest.clone(),
            publisher_signature_verified: true,
            registry_signature_threshold_verified: true,
            namespace_grant_verified: true,
            revoked: false,
        });
    }
    let promoted = &mut resolution.domain_pack_resolution_projection;
    promoted.resolution_digest = domain_pack_resolution_projection_digest(
        &resolution_request,
        anchored_snapshot.verified_snapshot().snapshot_digest(),
        promoted,
    );

    let mut composition_request = source.composition_request.clone();
    composition_request
        .domain_pack_composition_request
        .request_id = StableId(format!(
        "domain-pack.rebase.composition.{}",
        source_pointer.generation + 1
    ));
    composition_request.domain_pack_composition_request.core = target_core.clone();
    let owned_materials = load_composition_materials(&composition_request, &artifact_root)?;
    let materials = material_views(&composition_request, &owned_materials);
    let composition = compose_domain_packs(&composition_request, &materials);
    let source_was_degraded_empty =
        source_lock.packages.is_empty() && !source_lock.unresolved_composition_gaps.is_empty();
    let composition_admissible = composition
        .domain_pack_composition_projection
        .issues
        .is_empty()
        && if source_was_degraded_empty {
            composition.domain_pack_composition_projection.status
                == DomainPackCompositionStatus::Blocked
                && composition.domain_pack_composition_projection.gaps
                    == source_lock.unresolved_composition_gaps
        } else {
            composition.domain_pack_composition_projection.status
                == DomainPackCompositionStatus::Composable
                && composition
                    .domain_pack_composition_projection
                    .gaps
                    .is_empty()
        };
    if !composition_admissible {
        let projection = &composition.domain_pack_composition_projection;
        return Err(ExitError::with_code(
            2,
            format!(
                "domain-pack: target-Core recomposition is incompatible: status={:?}, issues={}, gaps={}, first_issue={:?}",
                projection.status,
                projection.issues.len(),
                projection.gaps.len(),
                projection.issues.first(),
            ),
        ));
    }

    let trust_selected = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .zip(&assessments)
        .map(|(package, assessment)| {
            Ok(DomainPackTrustSelectedPackage {
                package: package.clone(),
                structurally_valid: true,
                supply_chain: assessment.clone(),
                capability_demands: derive_domain_pack_capability_demands(
                    package,
                    &composition_request.domain_pack_composition_request,
                )
                .map_err(map_lifecycle_error)?,
            })
        })
        .collect::<Result<Vec<_>, ExitError>>()?;
    let trust_input = DomainPackTrustEvaluationInput {
        project_id: source_lock.project_id.clone(),
        selected: trust_selected,
        trust_policy: trust_policy.domain_pack_trust_policy.clone(),
        capability_registry: capability_registry
            .domain_pack_runtime_capability_registry
            .clone(),
        sandbox_policy: sandbox_policy.domain_pack_capability_sandbox_policy.clone(),
    };
    let trust = evaluate_domain_pack_trust(&trust_input);
    if trust.status != DomainPackTrustEvaluationStatus::Approved || !trust.issues.is_empty() {
        return Err(ExitError::with_code(
            2,
            "domain-pack: target-Core trust/capability evaluation blocked",
        ));
    }
    let target_packages = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .map(locked_package_from_resolution)
        .collect::<Vec<_>>();
    if target_packages != source_lock.packages
        || trust.capability_gaps != source_lock.unresolved_capability_gaps
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: target-Core rebase would silently change package or capability state",
        ));
    }
    let target_payload = DomainPackExactLockPayload {
        project_id: source_lock.project_id.clone(),
        core: target_core.clone(),
        requirements_digest: source_lock.requirements_digest.clone(),
        roots: source_lock.roots.clone(),
        registry_snapshot_digest: source_lock.registry_snapshot_digest.clone(),
        reviewer_registry_digest: source_lock.reviewer_registry_digest.clone(),
        reviewed_registry_digest: source_lock.reviewed_registry_digest.clone(),
        trust_policy_digest: source_lock.trust_policy_digest.clone(),
        capability_registry_digest: source_lock.capability_registry_digest.clone(),
        sandbox_policy_digest: source_lock.sandbox_policy_digest.clone(),
        resolution_digest: resolution
            .domain_pack_resolution_projection
            .resolution_digest
            .clone(),
        composition_digest: composition
            .domain_pack_composition_projection
            .composition_digest
            .clone(),
        packages: target_packages,
        verified_capability_bindings: trust.verified_capability_bindings,
        unresolved_composition_gaps: composition.domain_pack_composition_projection.gaps.clone(),
        unresolved_capability_gaps: trust.capability_gaps.clone(),
    };
    let target_lock = DomainPackExactLockDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_exact_lock: DomainPackExactLock {
            lock_digest: canonical_digest(&target_payload)?,
            payload: target_payload,
        },
    };
    let operation = DomainPackLifecycleOperation::RebaseCore {
        target_release_id: plan.target_release.release_id.clone(),
        expected_from_core_digest: plan.source_core.bundle_digest.clone(),
        target_core_digest: target_core.bundle_digest.clone(),
    };
    let expected_state = DomainPackExpectedLifecycleState::Initialized {
        generation: source_pointer.generation,
        active_lock_digest: source_pointer.active_lock_digest.clone(),
        lifecycle_head_digest: source_pointer.lifecycle_head_digest.clone(),
        project_snapshot_digest: project_snapshot_digest.clone(),
    };
    let lifecycle_request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: StableId(format!(
                "domain-pack.rebase.lifecycle.{}",
                source_pointer.generation + 1
            )),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: source_lock.project_id.clone(),
            principal_id,
            operation: operation.clone(),
            expected_state: expected_state.clone(),
            resolution_request_digest: canonical_digest(&resolution_request)?,
            project_snapshot_digest: project_snapshot_digest.clone(),
        },
    };
    let compatibility = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: StableId(format!(
            "domain-pack.rebase.compatibility.{}",
            source_pointer.generation + 1
        )),
        operation,
        sealed_core: target_core.clone(),
        from_lock: Some(source.exact_lock.clone()),
        to_lock: target_lock.clone(),
    });
    if !matches!(
        compatibility.domain_pack_compatibility_report.status,
        DomainPackCompatibilityStatus::Compatible | DomainPackCompatibilityStatus::Degraded
    ) {
        return Err(ExitError::with_code(
            2,
            "domain-pack: target-Core compatibility report is blocked",
        ));
    }
    let mut staged_artifacts = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .flat_map(|package| {
            let content = DomainPackArtifactBinding {
                artifact_ref: package.package.content.content_ref.clone(),
                raw_sha256: package.package.content.raw_sha256.clone(),
                canonical_sha256: package.package.content.canonical_sha256.clone(),
            };
            std::iter::once(package.package.manifest.clone())
                .chain(std::iter::once(content))
                .chain(std::iter::once(package.package.license.clone()))
                .chain(package.package.fixtures.iter().cloned())
        })
        .collect::<Vec<_>>();
    staged_artifacts.sort_by(|left, right| left.artifact_ref.0.cmp(&right.artifact_ref.0));
    staged_artifacts.dedup();
    let mut preflight = DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: StableId(format!(
                "domain-pack.rebase.preflight.{}",
                source_pointer.generation + 1
            )),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request_digest: canonical_digest(&lifecycle_request)?,
            request: lifecycle_request,
            observed_state: expected_state,
            resolution,
            proposed_lock: target_lock,
            composition,
            supply_chain_assessments: assessments,
            trust_decisions: trust.trust_decisions,
            capability_gaps: trust.capability_gaps,
            compatibility_report: compatibility,
            staged_artifacts,
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: Vec::new(),
            preflight_digest: String::new(),
        },
    };
    preflight.domain_pack_lifecycle_preflight.preflight_digest = canonical_digest(&preflight)?;
    let owned_artifacts = load_immutable_artifacts(&preflight, &artifact_root)?;
    let artifacts = immutable_artifact_views(&owned_artifacts);
    let project_snapshot =
        verify_domain_pack_project_snapshot(&controlled_roots.project, &project_snapshot_digest)
            .map_err(map_lifecycle_error)?;
    let prepared = lifecycle
        .prepare_candidate(preflight)
        .map_err(map_lifecycle_error)?;
    let context = DomainPackLifecycleAuthorizationContext {
        anchored_snapshot: &anchored_snapshot,
        anchored_reviewed_snapshot: reviewed_guard.snapshot(),
        project_snapshot: &project_snapshot,
        trust_policy_document: &trust_policy,
        registry_document: &registry,
        resolution_request: &resolution_request,
        composition_request: &composition_request,
        materials: &materials,
        artifacts: &artifacts,
        trust_input: &trust_input,
    };
    let authority = authorize_prepared_domain_pack_lifecycle(&prepared, &context)
        .map_err(map_lifecycle_error)?;
    let receipt = lifecycle
        .commit(prepared, authority)
        .map_err(map_lifecycle_error)?;
    drop(reviewed_guard);
    Ok(receipt)
}

fn run_trust_provision(args: &[String]) -> Result<(), ExitError> {
    let mut operator_root: Option<PathBuf> = None;
    let mut trust_policy_file: Option<PathBuf> = None;
    let mut registry_file: Option<PathBuf> = None;
    let mut project_root: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut state_root = PathBuf::from(".forge-method");
    let mut acknowledgement: Option<String> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--operator-root" => set_path(&mut operator_root, value),
            "--trust-policy-file" => set_path(&mut trust_policy_file, value),
            "--registry-file" => set_path(&mut registry_file, value),
            "--project-root" => set_path(&mut project_root, value),
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            "--state-root" => {
                state_root = PathBuf::from(value);
                true
            }
            "--operator-acknowledge-trust-on-first-use" => {
                acknowledgement = Some(value.to_owned());
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let operator_root = required(operator_root)?;
    let trust_policy_file = required(trust_policy_file)?;
    let registry_file = required(registry_file)?;
    let project_root = required(project_root)?;
    if acknowledgement.as_deref() != Some(DOMAIN_PACK_TRUST_ON_FIRST_USE_ACKNOWLEDGEMENT) {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: trust provisioning requires explicit operator acknowledgement via --operator-acknowledge-trust-on-first-use {DOMAIN_PACK_TRUST_ON_FIRST_USE_ACKNOWLEDGEMENT}"
        )));
    }

    let controlled_roots = canonical_lifecycle_roots(&project_root, &artifact_root, &state_root)?;
    let operator_root = canonical_external_operator_root(&operator_root, &controlled_roots)?;
    let trust_policy_file = trusted_external_file(
        &trust_policy_file,
        "operator trust policy",
        &controlled_roots,
    )?;
    let registry_file = trusted_external_file(
        &registry_file,
        "signed supply-chain registry",
        &controlled_roots,
    )?;
    require_direct_operator_file(&trust_policy_file, &operator_root, "operator trust policy")?;
    require_direct_operator_file(
        &registry_file,
        &operator_root,
        "signed supply-chain registry",
    )?;

    let mut locked = lock_operator_registry_anchor(&registry_file)?;
    let now_unix = trusted_now_unix()?;
    let trust_policy: DomainPackTrustPolicyDocument =
        read_typed(&trust_policy_file, "operator trust policy")?;
    let registry: DomainPackSupplyChainRegistryDocument =
        read_typed(&registry_file, "signed supply-chain registry")?;
    let verified = verify_domain_pack_supply_chain_snapshot(&trust_policy, &registry, now_unix)
        .map_err(|error| {
            ExitError::with_code(
                2,
                format!("domain-pack: supply-chain verification rejected: {error}"),
            )
        })?;
    let anchor_previously_present = locked
        .reconciliation
        .as_ref()
        .is_some_and(|reconciliation| reconciliation.raw_bytes().is_some());
    let mut anchor = if anchor_previously_present {
        load_operator_registry_anchor(&locked)?
    } else {
        DomainPackRegistryAnchor::new_trust_on_first_use(
            registry
                .domain_pack_supply_chain_registry
                .registry_id
                .clone(),
            trust_policy.domain_pack_trust_policy.audience.clone(),
        )
        .map_err(|error| {
            ExitError::with_code(
                2,
                format!("domain-pack: operator registry anchor rejected: {error}"),
            )
        })?
    };
    let anchored = advance_operator_registry_anchor(&mut locked, &mut anchor, verified)?;
    let verified = anchored.verified_snapshot();
    let payload = DomainPackTrustProvisionPayload {
        operator_root: operator_root.to_string_lossy().replace('\\', "/"),
        registry_id: verified.registry_id().clone(),
        audience: verified.audience().clone(),
        generation: verified.generation(),
        snapshot_digest: verified.snapshot_digest().to_owned(),
        trust_policy_digest: verified.trust_policy_digest().to_owned(),
        anchor_previously_present,
        boundary: "explicit operator-approved trust-on-first-use or monotonic anchor advance; no project lifecycle activation authority",
    };
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack trust-provision", payload),
        want_json,
    )
}

fn run_resolve(args: &[String]) -> Result<(), ExitError> {
    let mut request_file: Option<PathBuf> = None;
    let mut registry_file: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--request-file" => {
                request_file = Some(PathBuf::from(value));
                true
            }
            "--registry-file" => {
                registry_file = Some(PathBuf::from(value));
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let request_file = request_file.ok_or_else(|| ExitError::usage(usage()))?;
    let registry_file = registry_file.ok_or_else(|| ExitError::usage(usage()))?;
    let request_raw = read_bounded(
        &request_file,
        "resolution request",
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )?;
    let registry_raw = read_bounded(
        &registry_file,
        "supply-chain registry",
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )?;
    let request: DomainPackResolutionRequestDocument = parse(&request_raw, &request_file)?;
    let registry: DomainPackSupplyChainRegistryDocument = parse(&registry_raw, &registry_file)?;
    let projection = resolve_domain_packs(&request, &registry);
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack resolve", projection),
        want_json,
    )
}

fn run_lifecycle_state(args: &[String], recover: bool) -> Result<(), ExitError> {
    let mut state_root = PathBuf::from(".forge-method");
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--state-root" => {
                state_root = PathBuf::from(value);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let locked = lock_domain_pack_lifecycle(&state_root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect lifecycle state '{}': {error}",
            state_root.display()
        ))
    })?;
    let projection = locked.projection();
    let recovery_report = locked.recovery_report();
    let payload = DomainPackLifecycleStatusPayload {
        state_root: std::fs::canonicalize(&state_root)
            .unwrap_or(state_root)
            .to_string_lossy()
            .replace('\\', "/"),
        active: projection.active_pointer.is_some(),
        active_pointer: projection.active_pointer.clone(),
        active_lock: projection.active_lock.clone(),
        ledger_records: projection.ledger_records.clone(),
        recovery_report,
        recovery_checked: true,
        boundary: if recover {
            "crash-recovery checked under the retained lifecycle lock; no policy or activation authority"
        } else {
            "integrity-checked state projection; interrupted pointer replacement is recovered before read"
        },
    };
    let command = if recover {
        "domain-pack recover"
    } else {
        "domain-pack status"
    };
    crate::cli_util::emit_envelope(CliEnvelope::ok(command, payload), want_json)
}

#[allow(clippy::similar_names)] // Reviewer and reviewed registries are separate required roots.
fn run_lifecycle_authorized(args: &[String], apply: bool) -> Result<(), ExitError> {
    let mut preflight_file: Option<PathBuf> = None;
    let mut trust_policy_file: Option<PathBuf> = None;
    let mut registry_file: Option<PathBuf> = None;
    let mut reviewer_registry_file: Option<PathBuf> = None;
    let mut reviewed_registry_file: Option<PathBuf> = None;
    let mut resolution_request_file: Option<PathBuf> = None;
    let mut composition_request_file: Option<PathBuf> = None;
    let mut trust_input_file: Option<PathBuf> = None;
    let mut project_root: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut state_root = PathBuf::from(".forge-method");
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--preflight-file" => set_path(&mut preflight_file, value),
            "--trust-policy-file" => set_path(&mut trust_policy_file, value),
            "--registry-file" => set_path(&mut registry_file, value),
            "--reviewer-registry-file" => set_path(&mut reviewer_registry_file, value),
            "--reviewed-registry-file" => set_path(&mut reviewed_registry_file, value),
            "--resolution-request-file" => set_path(&mut resolution_request_file, value),
            "--composition-request-file" => set_path(&mut composition_request_file, value),
            "--trust-input-file" => set_path(&mut trust_input_file, value),
            "--project-root" => set_path(&mut project_root, value),
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            "--state-root" => {
                state_root = PathBuf::from(value);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let preflight_file = required(preflight_file)?;
    let trust_policy_file = required(trust_policy_file)?;
    let registry_file = required(registry_file)?;
    let reviewer_registry_file = required(reviewer_registry_file)?;
    let reviewed_registry_file = required(reviewed_registry_file)?;
    let resolution_request_file = required(resolution_request_file)?;
    let composition_request_file = required(composition_request_file)?;
    let trust_input_file = required(trust_input_file)?;
    let project_root = required(project_root)?;
    let now_unix = trusted_now_unix()?;

    let controlled_roots = canonical_lifecycle_roots(&project_root, &artifact_root, &state_root)?;
    let trust_policy_file = trusted_external_file(
        &trust_policy_file,
        "operator trust policy",
        &controlled_roots,
    )?;
    let registry_file = trusted_external_file(
        &registry_file,
        "signed supply-chain registry",
        &controlled_roots,
    )?;
    let reviewer_registry_file = trusted_external_file(
        &reviewer_registry_file,
        "signed reviewer registry",
        &controlled_roots,
    )?;
    let reviewed_registry_file = trusted_external_file(
        &reviewed_registry_file,
        "dual-signed reviewed registry",
        &controlled_roots,
    )?;

    let preflight: DomainPackLifecyclePreflightDocument =
        read_typed(&preflight_file, "lifecycle preflight")?;
    let owned_artifacts = load_immutable_artifacts(&preflight, &artifact_root)?;
    let artifacts = immutable_artifact_views(&owned_artifacts);
    let mut operator_anchor = lock_operator_registry_anchor(&registry_file)?;
    require_direct_operator_file(
        &reviewer_registry_file,
        &operator_anchor.operator_root,
        "signed reviewer registry",
    )?;
    require_direct_operator_file(
        &reviewed_registry_file,
        &operator_anchor.operator_root,
        "dual-signed reviewed registry",
    )?;
    let trust_policy: DomainPackTrustPolicyDocument =
        read_typed(&trust_policy_file, "operator trust policy")?;
    let registry: DomainPackSupplyChainRegistryDocument =
        read_typed(&registry_file, "signed supply-chain registry")?;
    let mut registry_anchor = load_operator_registry_anchor(&operator_anchor)?;
    let resolution_request: DomainPackResolutionRequestDocument =
        read_typed(&resolution_request_file, "resolution request")?;
    let composition_request: DomainPackCompositionRequestDocument =
        read_typed(&composition_request_file, "composition request")?;
    let trust_input: DomainPackTrustEvaluationInput =
        read_typed(&trust_input_file, "trust evaluation input")?;

    let expected_project_snapshot_digest = &preflight
        .domain_pack_lifecycle_preflight
        .request
        .domain_pack_lifecycle_request
        .project_snapshot_digest;
    let project_snapshot = verify_domain_pack_project_snapshot(
        &controlled_roots.project,
        expected_project_snapshot_digest,
    )
    .map_err(map_lifecycle_error)?;

    let verified_snapshot =
        verify_domain_pack_supply_chain_snapshot(&trust_policy, &registry, now_unix).map_err(
            |error| {
                ExitError::with_code(
                    2,
                    format!("domain-pack: supply-chain verification rejected: {error}"),
                )
            },
        )?;
    let operator_root = operator_anchor.operator_root.clone();
    let anchored_snapshot = advance_operator_registry_anchor(
        &mut operator_anchor,
        &mut registry_anchor,
        verified_snapshot,
    )?;
    // Deterministic cross-anchor order: supply-chain lock first, then the
    // combined reviewer/reviewed learning lock. The guard is explicitly kept
    // through authorization and commit below.
    let reviewed_guard = crate::domain_pack_learning_cmd::lock_reviewed_snapshot_for_lifecycle(
        &operator_root,
        &reviewer_registry_file,
        &reviewed_registry_file,
        now_unix,
    )?;
    let owned = load_composition_materials(&composition_request, &artifact_root)?;
    let materials = material_views(&composition_request, &owned);
    let mut lifecycle = lock_domain_pack_lifecycle(&state_root).map_err(map_lifecycle_error)?;
    let prepared = lifecycle
        .prepare_candidate(preflight.clone())
        .map_err(map_lifecycle_error)?;
    let context = DomainPackLifecycleAuthorizationContext {
        anchored_snapshot: &anchored_snapshot,
        anchored_reviewed_snapshot: reviewed_guard.snapshot(),
        project_snapshot: &project_snapshot,
        trust_policy_document: &trust_policy,
        registry_document: &registry,
        resolution_request: &resolution_request,
        composition_request: &composition_request,
        materials: &materials,
        artifacts: &artifacts,
        trust_input: &trust_input,
    };
    let authority = authorize_prepared_domain_pack_lifecycle(&prepared, &context)
        .map_err(map_lifecycle_error)?;
    if apply {
        let receipt = lifecycle
            .commit(prepared, authority)
            .map_err(map_lifecycle_error)?;
        drop(reviewed_guard);
        crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack apply", receipt), want_json)
    } else {
        let payload = DomainPackLifecyclePreflightPayload {
            ready: true,
            preflight_digest: preflight.domain_pack_lifecycle_preflight.preflight_digest,
            supply_chain: anchored_snapshot.verified_snapshot().audit(),
            boundary: "fresh verification completed under lifecycle lock; this preflight did not activate the candidate generation",
        };
        drop(reviewed_guard);
        crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack preflight", payload), want_json)
    }
}

fn set_path(target: &mut Option<PathBuf>, value: &str) -> bool {
    *target = Some(PathBuf::from(value));
    true
}

/// Candidate-byte download flags carry document or filesystem authority. A
/// second spelling must not silently replace the first supplied binding.
fn set_unique_path(target: &mut Option<PathBuf>, value: &str) -> bool {
    if target.is_some() {
        false
    } else {
        *target = Some(PathBuf::from(value));
        true
    }
}

fn required(path: Option<PathBuf>) -> Result<PathBuf, ExitError> {
    path.ok_or_else(|| ExitError::usage(usage()))
}

fn map_lifecycle_error(error: DomainPackLifecycleStoreError) -> ExitError {
    match error {
        DomainPackLifecycleStoreError::StaleExpectedState { .. } => {
            ExitError::conflict(error.to_string())
        }
        DomainPackLifecycleStoreError::PreflightBlocked { .. } => {
            ExitError::with_code(2, error.to_string())
        }
        DomainPackLifecycleStoreError::InvalidArgument { .. }
        | DomainPackLifecycleStoreError::InvalidDigest { .. }
        | DomainPackLifecycleStoreError::InvalidDocument { .. } => {
            ExitError::invalid_value(error.to_string())
        }
        _ => ExitError::failed(error.to_string()),
    }
}

fn run_validate(args: &[String]) -> Result<(), ExitError> {
    let mut manifest_file: Option<PathBuf> = None;
    let mut content_file: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut forge_core_version = env!("CARGO_PKG_VERSION").to_owned();
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--manifest-file" => {
                manifest_file = Some(PathBuf::from(value));
                true
            }
            "--content-file" => {
                content_file = Some(PathBuf::from(value));
                true
            }
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            "--forge-core-version" => {
                value.clone_into(&mut forge_core_version);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let manifest_file = manifest_file.ok_or_else(|| ExitError::usage(usage()))?;
    let content_file = content_file.ok_or_else(|| ExitError::usage(usage()))?;
    let manifest_raw = read(&manifest_file, "manifest")?;
    let content_raw = read(&content_file, "content")?;
    let manifest: DomainPackManifestDocument = parse(&manifest_raw, &manifest_file)?;
    let content: DomainPackContentDocument = parse(&content_raw, &content_file)?;
    let license_path = safe_join(
        &artifact_root,
        &manifest
            .domain_pack_manifest
            .provenance
            .license_text
            .artifact_ref
            .0,
    )?;
    let license_raw = read(&license_path, "license artifact")?;
    let identity = manifest.domain_pack_manifest.identity.clone();
    let candidate = DomainPackCandidateInput {
        // Standalone validation receives the authored manifest as an explicit
        // CLI input rather than through a composition request. Bind those exact
        // bytes and their closed typed semantics before handing the candidate
        // to the pure validator.
        manifest_binding: DomainPackArtifactBinding {
            artifact_ref: RepoPath("manifest-input.yaml".to_owned()),
            raw_sha256: sha256_bytes(&manifest_raw),
            canonical_sha256: canonical_digest(&manifest)?,
        },
        manifest,
        content,
    };
    let material = DomainPackCandidateMaterial {
        publisher: &identity.publisher.0,
        name: &identity.name.0,
        version: &identity.version,
        manifest_raw: &manifest_raw,
        content_raw: &content_raw,
        license_raw: &license_raw,
    };
    let issues = validate_domain_pack_candidate(&candidate, &material, &forge_core_version);
    let payload = DomainPackValidationPayload {
        authority: DomainPackCandidateAuthority::CandidateOnly,
        structurally_valid: issues.is_empty(),
        publisher: identity.publisher.0,
        name: identity.name.0,
        version: identity.version,
        issues,
        boundary: "candidate_only; no install, trust, activation, execution, or mutation authority",
    };
    crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack validate", payload), want_json)
}

fn run_compose(args: &[String]) -> Result<(), ExitError> {
    let mut request_file: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--request-file" => {
                request_file = Some(PathBuf::from(value));
                true
            }
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let request_file = request_file.ok_or_else(|| ExitError::usage(usage()))?;
    let request_raw = read(&request_file, "composition request")?;
    let request: DomainPackCompositionRequestDocument = parse(&request_raw, &request_file)?;

    let owned = load_composition_materials(&request, &artifact_root)?;
    let materials = material_views(&request, &owned);
    let projection = compose_domain_packs(&request, &materials);
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack compose", projection),
        want_json,
    )
}

fn load_composition_materials(
    request: &DomainPackCompositionRequestDocument,
    artifact_root: &Path,
) -> Result<Vec<OwnedMaterial>, ExitError> {
    request
        .domain_pack_composition_request
        .candidates
        .iter()
        .map(|candidate| {
            let manifest = &candidate.manifest.domain_pack_manifest;
            let manifest_path =
                safe_join(artifact_root, &candidate.manifest_binding.artifact_ref.0)?;
            let content_path = safe_join(artifact_root, &manifest.content.content_ref.0)?;
            let license_path = safe_join(
                artifact_root,
                &manifest.provenance.license_text.artifact_ref.0,
            )?;
            Ok(OwnedMaterial {
                manifest: read(&manifest_path, "pack manifest")?,
                content: read(&content_path, "pack content")?,
                license: read(&license_path, "license artifact")?,
            })
        })
        .collect::<Result<Vec<_>, ExitError>>()
}

fn material_views<'a>(
    request: &'a DomainPackCompositionRequestDocument,
    owned: &'a [OwnedMaterial],
) -> Vec<DomainPackCandidateMaterial<'a>> {
    request
        .domain_pack_composition_request
        .candidates
        .iter()
        .zip(owned)
        .map(|(candidate, owned)| {
            let identity = &candidate.manifest.domain_pack_manifest.identity;
            DomainPackCandidateMaterial {
                publisher: &identity.publisher.0,
                name: &identity.name.0,
                version: &identity.version,
                manifest_raw: &owned.manifest,
                content_raw: &owned.content,
                license_raw: &owned.license,
            }
        })
        .collect()
}

fn load_immutable_artifacts(
    preflight: &DomainPackLifecyclePreflightDocument,
    artifact_root: &Path,
) -> Result<Vec<OwnedImmutableArtifact>, ExitError> {
    preflight
        .domain_pack_lifecycle_preflight
        .staged_artifacts
        .iter()
        .map(|binding| {
            let path = safe_join(artifact_root, &binding.artifact_ref.0)?;
            let raw_bytes = read_bounded(
                &path,
                "staged immutable artifact",
                DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            )?;
            let actual = sha256_bytes(&raw_bytes);
            if actual != binding.raw_sha256 {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: staged immutable artifact '{}' differs from raw_sha256 binding: expected '{}', found '{actual}'",
                    binding.artifact_ref.0, binding.raw_sha256
                )));
            }
            Ok(OwnedImmutableArtifact {
                binding: binding.clone(),
                raw_bytes,
            })
        })
        .collect()
}

fn immutable_artifact_views(
    owned: &[OwnedImmutableArtifact],
) -> Vec<DomainPackImmutableArtifact<'_>> {
    owned
        .iter()
        .map(|artifact| DomainPackImmutableArtifact {
            binding: &artifact.binding,
            raw_bytes: &artifact.raw_bytes,
        })
        .collect()
}

fn canonical_lifecycle_roots(
    project_root: &Path,
    artifact_root: &Path,
    state_root: &Path,
) -> Result<CanonicalLifecycleRoots, ExitError> {
    Ok(CanonicalLifecycleRoots {
        project: canonicalize_existing_root(project_root, "--project-root")?,
        artifacts: canonicalize_existing_root(artifact_root, "--artifact-root")?,
        state: canonicalize_allow_missing(state_root, "--state-root")?,
        project_lexical: absolute_lexical(project_root, "--project-root")?,
        artifacts_lexical: absolute_lexical(artifact_root, "--artifact-root")?,
        state_lexical: absolute_lexical(state_root, "--state-root")?,
    })
}

fn absolute_lexical(path: &Path, label: &str) -> Result<PathBuf, ExitError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                ExitError::failed(format!(
                    "domain-pack: cannot resolve current directory for {label}: {error}"
                ))
            })?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(ExitError::invalid_value(format!(
                        "domain-pack: {label} '{}' escapes its filesystem root",
                        path.display()
                    )));
                }
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

fn canonicalize_existing_root(path: &Path, label: &str) -> Result<PathBuf, ExitError> {
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve {label} '{}': {error}",
            path.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must resolve to a directory",
            path.display()
        )));
    }
    Ok(canonical)
}

fn canonicalize_allow_missing(path: &Path, label: &str) -> Result<PathBuf, ExitError> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        if !canonical.is_dir() {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: {label} '{}' must resolve to a directory",
                path.display()
            )));
        }
        return Ok(canonical);
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                ExitError::failed(format!(
                    "domain-pack: cannot resolve current directory for {label}: {error}"
                ))
            })?
            .join(path)
    };
    let mut cursor = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        match std::fs::canonicalize(cursor) {
            Ok(mut canonical) => {
                for component in missing.iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = cursor.file_name().ok_or_else(|| {
                    ExitError::failed(format!(
                        "domain-pack: cannot resolve missing {label} '{}'",
                        path.display()
                    ))
                })?;
                missing.push(name.to_os_string());
                cursor = cursor.parent().ok_or_else(|| {
                    ExitError::failed(format!(
                        "domain-pack: cannot resolve missing {label} '{}'",
                        path.display()
                    ))
                })?;
            }
            Err(error) => {
                return Err(ExitError::failed(format!(
                    "domain-pack: cannot resolve {label} '{}': {error}",
                    path.display()
                )));
            }
        }
    }
}

fn trusted_external_file(
    path: &Path,
    label: &str,
    roots: &CanonicalLifecycleRoots,
) -> Result<PathBuf, ExitError> {
    let link_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect {label} '{}': {error}",
            path.display()
        ))
    })?;
    if link_metadata.file_type().is_symlink() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must not be a symbolic link or junction",
            path.display()
        )));
    }
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve {label} '{}': {error}",
            path.display()
        ))
    })?;
    let metadata = std::fs::metadata(&canonical).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect resolved {label} '{}': {error}",
            canonical.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must be a regular file",
            path.display()
        )));
    }
    let lexical = absolute_lexical(path, label)?;
    for (root, lexical_root, root_label) in [
        (&roots.project, &roots.project_lexical, "--project-root"),
        (
            &roots.artifacts,
            &roots.artifacts_lexical,
            "--artifact-root",
        ),
        (&roots.state, &roots.state_lexical, "--state-root"),
    ] {
        if canonical.starts_with(root) || lexical.starts_with(lexical_root) {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: {label} '{}' must be operator-controlled and external to {root_label}",
                path.display()
            )));
        }
    }
    Ok(canonical)
}

fn canonical_external_operator_root(
    path: &Path,
    roots: &CanonicalLifecycleRoots,
) -> Result<PathBuf, ExitError> {
    let link_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect --operator-root '{}': {error}",
            path.display()
        ))
    })?;
    if link_metadata.file_type().is_symlink() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: --operator-root '{}' must not be a symbolic link or junction",
            path.display()
        )));
    }
    let canonical = canonicalize_existing_root(path, "--operator-root")?;
    let lexical = absolute_lexical(path, "--operator-root")?;
    for (root, lexical_root, root_label) in [
        (&roots.project, &roots.project_lexical, "--project-root"),
        (
            &roots.artifacts,
            &roots.artifacts_lexical,
            "--artifact-root",
        ),
        (&roots.state, &roots.state_lexical, "--state-root"),
    ] {
        if canonical.starts_with(root)
            || root.starts_with(&canonical)
            || lexical.starts_with(lexical_root)
            || lexical_root.starts_with(&lexical)
        {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: --operator-root '{}' must be operator-controlled and disjoint from {root_label}",
                path.display()
            )));
        }
    }
    Ok(canonical)
}

fn require_direct_operator_file(
    file: &Path,
    operator_root: &Path,
    label: &str,
) -> Result<(), ExitError> {
    if file.parent() != Some(operator_root) {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must be a direct child of --operator-root '{}'",
            file.display(),
            operator_root.display()
        )));
    }
    Ok(())
}

pub(crate) fn lock_domain_pack_backup_authorities(
    state_root: &Path,
    verified_at_unix: u64,
) -> Result<LockedDomainPackBackupAuthorities, ExitError> {
    let source_path = state_root.join(DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH);
    let planned_raw = match crate::io_util::read_regular_file_no_follow_bounded(
        &source_path,
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    ) {
        Ok(raw) => Some(raw),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(ExitError::failed(format!(
                "domain-pack: cannot plan backup operator authority: {error}"
            )))
        }
    };
    let planned_binding = planned_raw
        .as_deref()
        .map(|raw| {
            yaml_serde::from_slice::<DomainPackOperatorSourceBinding>(raw).map_err(|error| {
                ExitError::invalid_value(format!(
                    "domain-pack: operator-source binding is invalid: {error}"
                ))
            })
        })
        .transpose()?;
    let (supply_chain, reviewed_learning) = if let Some(binding) = &planned_binding {
        let operator_root = PathBuf::from(&binding.operator_root);
        let registry_file = PathBuf::from(&binding.registry_file);
        let reviewer_registry_file = PathBuf::from(&binding.reviewer_registry_file);
        let accepted_registry_file = PathBuf::from(&binding.reviewed_registry_file);
        let supply = lock_operator_registry_anchor(&registry_file)?;
        let reviewed = crate::domain_pack_learning_cmd::lock_reviewed_snapshot_for_lifecycle(
            &operator_root,
            &reviewer_registry_file,
            &accepted_registry_file,
            verified_at_unix,
        )?;
        (Some(supply), Some(reviewed))
    } else {
        (None, None)
    };
    let mut operator_sources = lock_domain_pack_operator_sources(state_root)?;
    let operator_snapshot = snapshot_domain_pack_operator_sources(&mut operator_sources)?;
    if operator_snapshot.raw() != planned_raw.as_deref() {
        return Err(ExitError::conflict(
            "domain-pack: operator-source binding changed during backup lock acquisition",
        ));
    }
    let mut rebase_plan = lock_domain_pack_rebase_plan(state_root)?;
    let rebase_snapshot = snapshot_domain_pack_rebase_plan(&mut rebase_plan)?;
    // These two producers participate in the Store root boundary. Drain and
    // validate them before returning, but do not retain their effect leases: a
    // retained lease would make the immediately following host-quiescence
    // acquisition a forbidden process-local upgrade. Any producer entering the
    // small handoff window is drained by that exclusive root acquisition before
    // source bytes are observed.
    let lifecycle = lock_domain_pack_lifecycle(state_root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot validate lifecycle for backup: {error}"
        ))
    })?;
    drop(lifecycle);
    let learning_capture =
        acquire_effect_store_lock(state_root, "domain-pack-learning/capture.lock").map_err(
            |error| {
                ExitError::failed(format!(
                    "domain-pack: cannot validate learning capture for backup: {error:?}"
                ))
            },
        )?;
    drop(learning_capture);
    let mut expected_members = Vec::new();
    for snapshot in [&operator_snapshot, &rebase_snapshot] {
        if let Some(raw) = snapshot.raw() {
            expected_members.push(BackupExpectedMember {
                logical_path: format!("sidecar/.forge-method/{}", snapshot.relative_path()),
                sha256: sha256_bytes(raw),
            });
        }
    }
    // Release the retained root effect lease before the caller takes host
    // quiescence. The snapshots above remain CAS-bound expected members, and
    // the exclusive boundary drains any producer entering this handoff window.
    drop(rebase_plan);
    drop(operator_sources);
    Ok(LockedDomainPackBackupAuthorities {
        expected_members,
        _supply_chain: supply_chain,
        _reviewed_learning: reviewed_learning,
    })
}

pub(crate) fn lock_operator_registry_anchor(
    registry_file: &Path,
) -> Result<LockedOperatorRegistryAnchor, ExitError> {
    let operator_root = registry_file
        .parent()
        .ok_or_else(|| {
            ExitError::invalid_value(format!(
                "domain-pack: signed supply-chain registry '{}' has no operator root",
                registry_file.display()
            ))
        })?
        .to_path_buf();
    let retained_root = forge_core_store::RetainedEffectStoreRoot::acquire(&operator_root)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot bind operator registry effect root: {error}"
            ))
        })?;
    let root_identity = crate::io_util::RetainedDirectoryIdentity::capture(&operator_root)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot bind operator registry root identity: {error}"
            ))
        })?;
    let registry_file = registry_file
        .strip_prefix(&operator_root)
        .map_err(|error| ExitError::failed(format!("domain-pack: registry path: {error}")))?
        .to_path_buf();
    let lock = crate::io_util::acquire_effect_store_lock_retained(
        &retained_root,
        DOMAIN_PACK_REGISTRY_ANCHOR_LOCK_RELATIVE_PATH,
    )
    .map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot lock operator registry anchor below '{}': {error}",
            operator_root.display()
        ))
    })?;
    let reconciliation = reconcile_file_crash_safe_under_owned_lock(
        lock,
        Path::new(DOMAIN_PACK_REGISTRY_ANCHOR_RELATIVE_PATH),
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )
    .map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot recover operator registry anchor below '{}': {error}",
            operator_root.display()
        ))
    })?;
    root_identity.validate().map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: operator registry root changed while locking: {error}"
        ))
    })?;
    Ok(LockedOperatorRegistryAnchor {
        operator_root,
        registry_file,
        root_identity,
        reconciliation: Some(reconciliation),
        exact_read: None,
    })
}

fn snapshot_reconciled_present(
    reconciliation: &mut Option<OwnedRetainedCrashReplaceSession>,
    exact_read: &mut Option<OwnedRetainedCrashReplaceRead>,
    label: &str,
) -> Result<Option<Vec<u8>>, ExitError> {
    if let Some(read) = exact_read.as_mut() {
        read.revalidate().map_err(|error| {
            ExitError::conflict(format!(
                "domain-pack: {label} changed after locked recovery: {error}"
            ))
        })?;
        return Ok(Some(read.raw_bytes().to_vec()));
    }
    let present = reconciliation
        .as_ref()
        .ok_or_else(|| ExitError::failed(format!("domain-pack: {label} authority was consumed")))?
        .raw_bytes()
        .is_some();
    if !present {
        reconciliation
            .as_mut()
            .ok_or_else(|| {
                ExitError::failed(format!("domain-pack: {label} authority was consumed"))
            })?
            .revalidate()
            .map_err(|error| {
                ExitError::conflict(format!(
                    "domain-pack: {label} absence changed after locked recovery: {error}"
                ))
            })?;
        return Ok(None);
    }
    let session = reconciliation
        .take()
        .ok_or_else(|| ExitError::failed(format!("domain-pack: {label} authority was consumed")))?;
    let mut read = session
        .read_exact()
        .map_err(|error| {
            ExitError::conflict(format!(
                "domain-pack: {label} changed after locked recovery: {error}"
            ))
        })?
        .ok_or_else(|| {
            ExitError::conflict(format!(
                "domain-pack: {label} disappeared after locked recovery"
            ))
        })?;
    read.revalidate().map_err(|error| {
        ExitError::conflict(format!(
            "domain-pack: {label} changed after locked recovery: {error}"
        ))
    })?;
    let raw = read.raw_bytes().to_vec();
    *exact_read = Some(read);
    Ok(Some(raw))
}

/// Snapshot exact supply-chain anchor bytes while retaining the producer lock.
pub(crate) fn snapshot_operator_registry_anchor(
    locked: &mut LockedOperatorRegistryAnchor,
) -> Result<OperatorRegistryAnchorSnapshot, ExitError> {
    let raw_anchor = snapshot_reconciled_present(
        &mut locked.reconciliation,
        &mut locked.exact_read,
        "operator registry anchor",
    )?
    .ok_or_else(|| {
        ExitError::with_code(
            2,
            "domain-pack: operator registry anchor is not provisioned; obtain explicit operator approval and run 'forge-core domain-pack trust-provision' first",
        )
    })?;
    let raw_registry = locked
        .root_identity
        .read_direct_file_bounded(
            &locked.registry_file,
            MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES as u64,
        )
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot snapshot signed supply-chain registry: {error}"
            ))
        })?;
    locked.root_identity.validate().map_err(|error| {
        ExitError::conflict(format!(
            "domain-pack: operator registry root changed during snapshot: {error}"
        ))
    })?;
    let raw_registry_sha256 = sha256_bytes(&raw_registry);
    let raw_anchor_sha256 = sha256_bytes(&raw_anchor);
    Ok(OperatorRegistryAnchorSnapshot {
        raw_registry,
        raw_registry_sha256,
        raw_anchor,
        raw_anchor_sha256,
    })
}

fn load_operator_registry_anchor(
    locked: &LockedOperatorRegistryAnchor,
) -> Result<DomainPackRegistryAnchor, ExitError> {
    let anchor_path = locked
        .operator_root
        .join(DOMAIN_PACK_REGISTRY_ANCHOR_RELATIVE_PATH);
    let raw_anchor = locked
        .reconciliation
        .as_ref()
        .and_then(|reconciliation| reconciliation.raw_bytes())
        .ok_or_else(|| {
            ExitError::with_code(
                2,
                "domain-pack: operator registry anchor is not provisioned; obtain explicit operator approval and run 'forge-core domain-pack trust-provision' first",
            )
        })?;
    let schema_header: DomainPackRegistryAnchorSchemaHeader = parse(raw_anchor, &anchor_path)?;
    if schema_header.schema_version != DOMAIN_PACK_REGISTRY_ANCHOR_SCHEMA_VERSION {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: unsupported operator registry anchor schema '{}'; expected '{}'",
            schema_header.schema_version, DOMAIN_PACK_REGISTRY_ANCHOR_SCHEMA_VERSION
        )));
    }
    let head: DomainPackRegistryAnchorHead = parse(raw_anchor, &anchor_path)?;
    DomainPackRegistryAnchor::from_operator_protected_head(
        head.registry_id,
        head.audience,
        head.generation,
        head.snapshot_digest,
        head.trust_policy_digest,
        head.cumulative_revocations,
        head.cumulative_revocation_digest,
    )
    .map_err(|error| {
        ExitError::with_code(
            2,
            format!("domain-pack: operator registry anchor rejected: {error}"),
        )
    })
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn bind_domain_pack_effect_directories(
    state_root: &Path,
    lock_relative_path: &str,
    target_relative_path: &str,
) -> Result<
    (
        crate::io_util::RetainedDirectoryIdentity,
        crate::io_util::RetainedDirectoryIdentity,
        crate::io_util::RetainedDirectoryIdentity,
    ),
    ExitError,
> {
    let root_identity = crate::io_util::RetainedDirectoryIdentity::capture(state_root)
        .map_err(|error| ExitError::failed(format!("domain-pack: bind state root: {error}")))?;
    let lock_parent_relative = Path::new(lock_relative_path)
        .parent()
        .ok_or_else(|| ExitError::failed("domain-pack: lock path has no parent"))?;
    let target_parent_relative = Path::new(target_relative_path)
        .parent()
        .ok_or_else(|| ExitError::failed("domain-pack: target path has no parent"))?;
    let lock_parent_identity = root_identity
        .open_or_create_direct_directory(lock_parent_relative)
        .map_err(|error| ExitError::failed(format!("domain-pack: bind lock parent: {error}")))?;
    let target_parent_identity = root_identity
        .open_or_create_direct_directory(target_parent_relative)
        .map_err(|error| ExitError::failed(format!("domain-pack: bind target parent: {error}")))?;
    Ok((root_identity, lock_parent_identity, target_parent_identity))
}

fn validate_domain_pack_effect_directories(
    root: &crate::io_util::RetainedDirectoryIdentity,
    lock_parent: &crate::io_util::RetainedDirectoryIdentity,
    target_parent: &crate::io_util::RetainedDirectoryIdentity,
) -> Result<(), ExitError> {
    root.validate()
        .and_then(|()| lock_parent.validate())
        .and_then(|()| target_parent.validate())
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: retained authority identity changed: {error}"
            ))
        })
}

fn replace_domain_pack_file_exact(
    reconciliation: OwnedRetainedCrashReplaceSession,
    raw: &[u8],
    label: &str,
) -> Result<OwnedRetainedCrashReplaceRead, ExitError> {
    let mut installed = reconciliation.replace(raw).map_err(|error| {
        ExitError::failed(format!("domain-pack: cannot persist {label}: {error}"))
    })?;
    installed.revalidate().map_err(|error| {
        ExitError::conflict(format!(
            "domain-pack: {label} selector changed while persistence completed: {error}"
        ))
    })?;
    Ok(installed)
}

pub(crate) fn lock_domain_pack_operator_sources(
    state_root: &Path,
) -> Result<LockedDomainPackOperatorSources, ExitError> {
    let retained_root =
        forge_core_store::RetainedEffectStoreRoot::acquire(state_root).map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot bind operator-source effect root: {error}"
            ))
        })?;
    let (root_identity, lock_parent_identity, target_parent_identity) =
        bind_domain_pack_effect_directories(
            state_root,
            DOMAIN_PACK_OPERATOR_SOURCE_LOCK_RELATIVE_PATH,
            DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH,
        )?;
    let lock = crate::io_util::acquire_effect_store_lock_retained(
        &retained_root,
        DOMAIN_PACK_OPERATOR_SOURCE_LOCK_RELATIVE_PATH,
    )
    .map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot lock operator-source binding: {error}"
        ))
    })?;
    validate_domain_pack_effect_directories(
        &root_identity,
        &lock_parent_identity,
        &target_parent_identity,
    )?;
    let reconciliation = reconcile_file_crash_safe_under_owned_lock(
        lock,
        Path::new(DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH),
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )
    .map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot recover operator-source binding: {error}"
        ))
    })?;
    validate_domain_pack_effect_directories(
        &root_identity,
        &lock_parent_identity,
        &target_parent_identity,
    )?;
    Ok(LockedDomainPackOperatorSources {
        root_identity,
        lock_parent_identity,
        target_parent_identity,
        reconciliation: Some(reconciliation),
        exact_read: None,
    })
}

#[allow(dead_code)]
pub(crate) fn snapshot_domain_pack_operator_sources(
    locked: &mut LockedDomainPackOperatorSources,
) -> Result<DomainPackStateFileSnapshot, ExitError> {
    snapshot_domain_pack_state_file(
        &locked.root_identity,
        &locked.lock_parent_identity,
        &locked.target_parent_identity,
        &mut locked.reconciliation,
        &mut locked.exact_read,
        DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH,
    )
}

fn persist_domain_pack_operator_sources(
    state_root: &Path,
    binding: &DomainPackOperatorSourceBinding,
) -> Result<(), ExitError> {
    let locked = lock_domain_pack_operator_sources(state_root)?;
    let raw = yaml_serde::to_string(binding)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot serialize operator-source binding: {error}"
            ))
        })?
        .into_bytes();
    let reconciliation = locked.reconciliation.ok_or_else(|| {
        ExitError::failed("domain-pack: operator-source reconciliation authority was consumed")
    })?;
    replace_domain_pack_file_exact(reconciliation, &raw, "operator-source binding").map(drop)
}

fn load_domain_pack_operator_sources(
    state_root: &Path,
) -> Result<DomainPackOperatorSourceBinding, ExitError> {
    let path = state_root.join(DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH);
    let mut locked = lock_domain_pack_operator_sources(state_root)?;
    let snapshot = snapshot_domain_pack_operator_sources(&mut locked)?;
    let raw = snapshot.raw().ok_or_else(|| {
        ExitError::invalid_value("domain-pack: operator-source binding is not provisioned")
    })?;
    let binding: DomainPackOperatorSourceBinding = parse(raw, &path)?;
    if binding.schema_version != DOMAIN_PACK_OPERATOR_SOURCE_SCHEMA_VERSION {
        return Err(ExitError::invalid_value(
            "domain-pack: operator-source binding has unsupported schema",
        ));
    }
    Ok(binding)
}

pub(crate) fn lock_domain_pack_rebase_plan(
    state_root: &Path,
) -> Result<LockedDomainPackRebasePlan, ExitError> {
    let retained_root =
        forge_core_store::RetainedEffectStoreRoot::acquire(state_root).map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot bind rebase-plan effect root: {error}"
            ))
        })?;
    let (root_identity, lock_parent_identity, target_parent_identity) =
        bind_domain_pack_effect_directories(
            state_root,
            DOMAIN_PACK_REBASE_PLAN_LOCK_RELATIVE_PATH,
            DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH,
        )?;
    let lock = crate::io_util::acquire_effect_store_lock_retained(
        &retained_root,
        DOMAIN_PACK_REBASE_PLAN_LOCK_RELATIVE_PATH,
    )
    .map_err(|error| ExitError::failed(format!("domain-pack: cannot lock rebase plan: {error}")))?;
    validate_domain_pack_effect_directories(
        &root_identity,
        &lock_parent_identity,
        &target_parent_identity,
    )?;
    let reconciliation = reconcile_file_crash_safe_under_owned_lock(
        lock,
        Path::new(DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH),
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )
    .map_err(|error| {
        ExitError::failed(format!("domain-pack: cannot recover rebase plan: {error}"))
    })?;
    validate_domain_pack_effect_directories(
        &root_identity,
        &lock_parent_identity,
        &target_parent_identity,
    )?;
    Ok(LockedDomainPackRebasePlan {
        root_identity,
        lock_parent_identity,
        target_parent_identity,
        reconciliation: Some(reconciliation),
        exact_read: None,
    })
}

#[allow(dead_code)]
pub(crate) fn snapshot_domain_pack_rebase_plan(
    locked: &mut LockedDomainPackRebasePlan,
) -> Result<DomainPackStateFileSnapshot, ExitError> {
    snapshot_domain_pack_state_file(
        &locked.root_identity,
        &locked.lock_parent_identity,
        &locked.target_parent_identity,
        &mut locked.reconciliation,
        &mut locked.exact_read,
        DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH,
    )
}

#[allow(dead_code)]
fn snapshot_domain_pack_state_file(
    root_identity: &crate::io_util::RetainedDirectoryIdentity,
    lock_parent_identity: &crate::io_util::RetainedDirectoryIdentity,
    target_parent_identity: &crate::io_util::RetainedDirectoryIdentity,
    reconciliation: &mut Option<OwnedRetainedCrashReplaceSession>,
    exact_read: &mut Option<OwnedRetainedCrashReplaceRead>,
    relative_path: &'static str,
) -> Result<DomainPackStateFileSnapshot, ExitError> {
    validate_domain_pack_effect_directories(
        root_identity,
        lock_parent_identity,
        target_parent_identity,
    )?;
    let raw = snapshot_reconciled_present(reconciliation, exact_read, relative_path)?;
    validate_domain_pack_effect_directories(
        root_identity,
        lock_parent_identity,
        target_parent_identity,
    )?;
    Ok(DomainPackStateFileSnapshot { relative_path, raw })
}

fn persist_domain_pack_rebase_plan(
    state_root: &Path,
    plan: &DomainPackRebasePlanDocument,
) -> Result<(), ExitError> {
    let locked = lock_domain_pack_rebase_plan(state_root)?;
    let raw = yaml_serde::to_string(plan)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot serialize rebase plan: {error}"
            ))
        })?
        .into_bytes();
    let reconciliation = locked.reconciliation.ok_or_else(|| {
        ExitError::failed("domain-pack: rebase-plan reconciliation authority was consumed")
    })?;
    replace_domain_pack_file_exact(reconciliation, &raw, "rebase plan").map(drop)
}

pub(crate) fn load_persisted_domain_pack_rebase_plan(
    state_root: &Path,
    expected_digest: &str,
) -> Result<DomainPackRebasePlanDocument, ExitError> {
    let path = state_root.join(DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH);
    let mut locked = lock_domain_pack_rebase_plan(state_root)?;
    let snapshot = snapshot_domain_pack_rebase_plan(&mut locked)?;
    let raw = snapshot.raw().ok_or_else(|| {
        ExitError::invalid_value("domain-pack: persisted rebase plan is not provisioned")
    })?;
    let plan: DomainPackRebasePlanDocument = parse(raw, &path)?;
    if !verify_domain_pack_rebase_plan(&plan)
        || plan.domain_pack_rebase_plan.plan_digest != expected_digest
    {
        return Err(ExitError::conflict(
            "domain-pack: persisted rebase plan failed integrity or expected CAS",
        ));
    }
    Ok(plan)
}

fn trusted_now_unix() -> Result<u64, ExitError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ExitError::failed("domain-pack: system clock is before Unix epoch"))
        .map(|duration| duration.as_secs())
}

fn advance_operator_registry_anchor(
    locked: &mut LockedOperatorRegistryAnchor,
    anchor: &mut DomainPackRegistryAnchor,
    verified: VerifiedDomainPackSupplyChainSnapshot,
) -> Result<AnchoredDomainPackSupplyChainSnapshot, ExitError> {
    let expected = anchor.version();
    let (capability, changed) =
        match anchor
            .compare_and_advance(&expected, verified)
            .map_err(|error| {
                ExitError::with_code(
                    2,
                    format!("domain-pack: monotonic registry anchor rejected: {error}"),
                )
            })? {
            DomainPackRegistryAnchorAdvance::Advanced(capability) => (capability, true),
            DomainPackRegistryAnchorAdvance::Replay { capability, .. } => (capability, false),
        };
    let verified = capability.verified_snapshot();
    let head = DomainPackRegistryAnchorHead {
        schema_version: DOMAIN_PACK_REGISTRY_ANCHOR_SCHEMA_VERSION.to_owned(),
        registry_id: verified.registry_id().clone(),
        audience: verified.audience().clone(),
        generation: verified.generation(),
        snapshot_digest: verified.snapshot_digest().to_owned(),
        trust_policy_digest: verified.trust_policy_digest().to_owned(),
        cumulative_revocations: capability.current_revocations().to_vec(),
        cumulative_revocation_digest: capability.cumulative_revocation_digest().to_owned(),
    };
    let raw = yaml_serde::to_string(&head)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot serialize operator registry anchor: {error}"
            ))
        })?
        .into_bytes();
    if locked.exact_read.is_some() {
        return Err(ExitError::failed(
            "domain-pack: operator registry reconciliation authority was already consumed",
        ));
    }
    let reconciliation = locked.reconciliation.take().ok_or_else(|| {
        ExitError::failed("domain-pack: operator registry reconciliation authority was consumed")
    })?;
    let retained = if changed {
        replace_domain_pack_file_exact(reconciliation, &raw, "operator registry anchor")?
    } else {
        let mut current = reconciliation
            .read_exact()
            .map_err(|error| {
                ExitError::conflict(format!(
                    "domain-pack: operator registry anchor changed after locked recovery: {error}"
                ))
            })?
            .ok_or_else(|| {
                ExitError::conflict(
                    "domain-pack: operator registry anchor disappeared after locked recovery",
                )
            })?;
        current.revalidate().map_err(|error| {
            ExitError::conflict(format!(
                "domain-pack: operator registry anchor changed after locked recovery: {error}"
            ))
        })?;
        current
    };
    locked.exact_read = Some(retained);
    Ok(capability)
}

fn parse_flags(
    args: &[String],
    mut set_value: impl FnMut(&str, &str) -> bool,
    want_json: &mut bool,
) -> Result<(), ExitError> {
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => *want_json = true,
            "--no-json" | "--text" => *want_json = false,
            "--help" | "-h" => return Err(ExitError::usage(usage())),
            flag if flag.starts_with("--") => {
                index += 1;
                let value = args
                    .get(index)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| ExitError::usage(usage()))?;
                if !set_value(flag, value) {
                    return Err(ExitError::usage(usage()));
                }
            }
            _ => return Err(ExitError::usage(usage())),
        }
        index += 1;
    }
    Ok(())
}

fn read(path: &Path, label: &str) -> Result<Vec<u8>, ExitError> {
    read_bounded(path, label, MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES as u64)
}

fn read_bounded(path: &Path, label: &str, maximum: u64) -> Result<Vec<u8>, ExitError> {
    let path_metadata = std::fs::metadata(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect {label} '{}': {error}",
            path.display()
        ))
    })?;
    if !path_metadata.is_file() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(path)
    };
    #[cfg(not(unix))]
    let file = std::fs::OpenOptions::new().read(true).open(path);
    let file = file.map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot open bounded regular {label} '{}': {error}",
            path.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect {label} '{}': {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: opened {label} '{}' is not a regular file",
            path.display()
        )));
    }
    if metadata.len() > maximum {
        return Err(document_too_large(path, label, metadata.len(), maximum));
    }

    // Metadata is only an early rejection. The capped reader also closes the
    // race where a regular file grows between metadata and the read, and it
    // bounds streams whose metadata does not expose a useful byte length.
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(usize::MAX)
            .min(usize::try_from(maximum).unwrap_or(usize::MAX)),
    );
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot read {label} '{}': {error}",
                path.display()
            ))
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(document_too_large(path, label, bytes.len() as u64, maximum));
    }
    Ok(bytes)
}

fn document_too_large(path: &Path, label: &str, observed: u64, maximum: u64) -> ExitError {
    ExitError::failed(format!(
        "domain-pack: {label} '{}' is {observed} bytes and exceeds maximum {maximum} bytes",
        path.display()
    ))
}

fn canonical_digest<T: serde::Serialize>(value: &T) -> Result<String, ExitError> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot canonicalize manifest input: {error}"
        ))
    })?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn parse<T: serde::de::DeserializeOwned>(raw: &[u8], path: &Path) -> Result<T, ExitError> {
    reject_yaml_alias_syntax(raw, path)?;
    let text = std::str::from_utf8(raw).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: '{}' is not UTF-8: {error}",
            path.display()
        ))
    })?;
    yaml_serde::from_str(text).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: '{}' is not a closed typed document: {error}",
            path.display()
        ))
    })
}

fn reject_yaml_alias_syntax(raw: &[u8], path: &Path) -> Result<(), ExitError> {
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut comment = false;
    let mut index = 0usize;
    while index < raw.len() {
        let byte = raw[index];
        if comment {
            if byte == b'\n' {
                comment = false;
            }
            index += 1;
            continue;
        }
        if double_quoted {
            if byte == b'\\' {
                index = index.saturating_add(2);
                continue;
            }
            if byte == b'"' {
                double_quoted = false;
            }
            index += 1;
            continue;
        }
        if single_quoted {
            if byte == b'\'' {
                if raw.get(index + 1) == Some(&b'\'') {
                    index = index.saturating_add(2);
                    continue;
                }
                single_quoted = false;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' => double_quoted = true,
            b'\'' => single_quoted = true,
            b'#' if index == 0
                || raw
                    .get(index.wrapping_sub(1))
                    .is_some_and(u8::is_ascii_whitespace) =>
            {
                comment = true;
            }
            b'&' | b'*'
                if (index == 0
                    || raw.get(index.wrapping_sub(1)).is_some_and(|previous| {
                        previous.is_ascii_whitespace()
                            || matches!(previous, b'-' | b'?' | b':' | b',' | b'[' | b'{')
                    }))
                    && raw.get(index + 1).is_some_and(|next| {
                        !next.is_ascii_whitespace()
                            && !matches!(next, b',' | b'[' | b']' | b'{' | b'}')
                    }) =>
            {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: '{}' uses YAML anchor/alias syntax; bounded authority documents must be alias-free",
                    path.display()
                )));
            }
            _ => {}
        }
        index += 1;
    }
    Ok(())
}

fn read_typed<T: serde::de::DeserializeOwned>(path: &Path, label: &str) -> Result<T, ExitError> {
    let raw = read_bounded(path, label, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
    parse(&raw, path)
}

fn read_local_remote_cache_attempts(
    artifact_root: &Path,
    request_digest: &str,
    cache_reads: &[forge_core_decisions::DomainPackRemotePlannedCacheRead],
) -> Result<Vec<DomainPackRemoteFetchAttempt>, ExitError> {
    let expected_paths = cache_reads
        .iter()
        .map(|read| read.location.object_path.0.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if expected_paths.len() != cache_reads.len() {
        return Err(ExitError::invalid_value(
            "domain-pack: local-only remote plan repeats an artifact object path",
        ));
    }
    let canonical_root = inspect_exact_local_artifact_root(artifact_root, &expected_paths)?;

    cache_reads
        .iter()
        .map(|planned| {
            let path = safe_local_artifact_path(
                &canonical_root,
                &planned.location.object_path.0,
                "remote artifact object",
            )?;
            let expected = &planned.location.artifact;
            let maximum = expected.byte_length.min(DOMAIN_PACK_MAX_DOCUMENT_BYTES);
            let raw_bytes = read_bounded(&path, "operator-provisioned local artifact", maximum)?;
            let observed_raw_sha256 = sha256_bytes(&raw_bytes);
            let observed_canonical_sha256 =
                remote_observed_canonical_digest(expected.media_type, &raw_bytes, &path)?;
            let mut observation = DomainPackRemoteUntrustedFetchObservationDocument {
                schema_version: DOMAIN_PACK_REMOTE_ACQUISITION_SCHEMA_VERSION.to_owned(),
                domain_pack_remote_untrusted_fetch_observation:
                    DomainPackRemoteUntrustedFetchObservation {
                        observation_id: StableId(format!(
                            "domain-pack.local-observation.{}",
                            &observed_raw_sha256["sha256:".len()..]
                        )),
                        authority: DomainPackCandidateAuthority::CandidateOnly,
                        request_digest: request_digest.to_owned(),
                        source: planned.source.clone(),
                        location: planned.location.clone(),
                        observed_raw_sha256,
                        observed_canonical_sha256,
                        observed_byte_length: u64::try_from(raw_bytes.len()).unwrap_or(u64::MAX),
                        observed_media_type: expected.media_type,
                        observation_digest: String::new(),
                    },
            };
            observation
                .domain_pack_remote_untrusted_fetch_observation
                .observation_digest = observation.observation_digest().map_err(|error| {
                ExitError::invalid_value(format!(
                    "domain-pack: cannot bind local artifact observation: {error:?}"
                ))
            })?;
            Ok(DomainPackRemoteFetchAttempt::Observation {
                observation,
                raw_bytes,
            })
        })
        .collect()
}

fn inspect_exact_local_artifact_root(
    artifact_root: &Path,
    expected_paths: &std::collections::BTreeSet<String>,
) -> Result<PathBuf, ExitError> {
    let root_metadata = std::fs::symlink_metadata(artifact_root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect --artifact-root '{}': {error}",
            artifact_root.display()
        ))
    })?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: --artifact-root '{}' must be a direct regular directory, not a symbolic link or special file",
            artifact_root.display()
        )));
    }
    if expected_paths.len() > MAX_LOCAL_ARTIFACT_TREE_ENTRIES {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: local artifact plan exceeds the contract maximum of {MAX_LOCAL_ARTIFACT_TREE_ENTRIES} entries",
        )));
    }
    let canonical_root = std::fs::canonicalize(artifact_root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve --artifact-root '{}': {error}",
            artifact_root.display()
        ))
    })?;
    let mut observed_paths = std::collections::BTreeSet::new();
    let mut observed_entries = 0usize;
    collect_local_artifact_paths(
        &canonical_root,
        &canonical_root,
        0,
        &mut observed_entries,
        &mut observed_paths,
    )?;
    if let Some(path) = observed_paths.difference(expected_paths).next() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: --artifact-root contains unselected artifact object '{path}'",
        )));
    }
    if let Some(path) = expected_paths.difference(&observed_paths).next() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: --artifact-root is missing selected artifact object '{path}'",
        )));
    }
    Ok(canonical_root)
}

fn collect_local_artifact_paths(
    root: &Path,
    current: &Path,
    depth: usize,
    observed_entries: &mut usize,
    observed_paths: &mut std::collections::BTreeSet<String>,
) -> Result<(), ExitError> {
    let entries = std::fs::read_dir(current).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot enumerate untrusted local artifact directory '{}': {error}",
            current.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot enumerate untrusted local artifact directory '{}': {error}",
                current.display()
            ))
        })?;
        *observed_entries = observed_entries.checked_add(1).ok_or_else(|| {
            ExitError::invalid_value(
                "domain-pack: --artifact-root local artifact entry count overflowed its limit",
            )
        })?;
        if *observed_entries > MAX_LOCAL_ARTIFACT_TREE_ENTRIES {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: --artifact-root exceeds the maximum of {MAX_LOCAL_ARTIFACT_TREE_ENTRIES} local entries",
            )));
        }
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot inspect untrusted local artifact '{}': {error}",
                path.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: --artifact-root contains symbolic link or junction '{}', which is forbidden",
                path.display()
            )));
        }
        if metadata.is_dir() {
            if depth >= MAX_LOCAL_ARTIFACT_TREE_DEPTH {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: --artifact-root exceeds the maximum directory depth of {MAX_LOCAL_ARTIFACT_TREE_DEPTH}",
                )));
            }
            collect_local_artifact_paths(root, &path, depth + 1, observed_entries, observed_paths)?;
        } else if metadata.is_file() {
            let relative = path.strip_prefix(root).map_err(|_| {
                ExitError::failed("domain-pack: local artifact enumeration escaped its root")
            })?;
            let relative = local_relative_path_string(relative)?;
            if !observed_paths.insert(relative.clone()) {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: --artifact-root repeats local artifact object '{relative}'",
                )));
            }
        } else {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: --artifact-root contains non-regular special file '{}', which is forbidden",
                path.display()
            )));
        }
    }
    Ok(())
}

fn local_relative_path_string(path: &Path) -> Result<String, ExitError> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: local artifact path is not a normalized relative path: {}",
                path.display()
            )));
        };
        components.push(component.to_str().ok_or_else(|| {
            ExitError::invalid_value(format!(
                "domain-pack: local artifact path is not valid UTF-8: {}",
                path.display()
            ))
        })?);
    }
    if components.is_empty() {
        return Err(ExitError::invalid_value(
            "domain-pack: local artifact path is empty",
        ));
    }
    Ok(components.join("/"))
}

fn safe_local_artifact_path(
    canonical_root: &Path,
    reference: &str,
    label: &str,
) -> Result<PathBuf, ExitError> {
    let reference_path = Path::new(reference);
    if reference_path.is_absolute()
        || reference_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} must be a normalized relative path beneath --artifact-root: {reference}",
        )));
    }
    let mut candidate = canonical_root.to_path_buf();
    for component in reference_path.components() {
        let Component::Normal(component) = component else {
            unreachable!("normalized components were checked above");
        };
        candidate.push(component);
        let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot inspect {label} '{}': {error}",
                candidate.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: {label} '{}' is a symbolic link or junction",
                candidate.display()
            )));
        }
    }
    let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect {label} '{}': {error}",
            candidate.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must be a regular file",
            candidate.display()
        )));
    }
    let canonical_candidate = std::fs::canonicalize(&candidate).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve {label} '{}': {error}",
            candidate.display()
        ))
    })?;
    if !canonical_candidate.starts_with(canonical_root) {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} escapes canonical --artifact-root: {reference}",
        )));
    }
    Ok(canonical_candidate)
}

fn remote_observed_canonical_digest(
    media_type: DomainPackRemoteArtifactMediaType,
    raw: &[u8],
    path: &Path,
) -> Result<String, ExitError> {
    match media_type {
        DomainPackRemoteArtifactMediaType::ApplicationYaml => {
            reject_yaml_alias_syntax(raw, path)?;
            let text = std::str::from_utf8(raw).map_err(|error| {
                ExitError::invalid_value(format!(
                    "domain-pack: local YAML artifact '{}' is not UTF-8: {error}",
                    path.display()
                ))
            })?;
            let value: serde_json::Value = yaml_serde::from_str(text).map_err(|error| {
                ExitError::invalid_value(format!(
                    "domain-pack: local YAML artifact '{}' cannot be canonicalized: {error}",
                    path.display()
                ))
            })?;
            canonical_digest(&value)
        }
        DomainPackRemoteArtifactMediaType::ApplicationJson => {
            let value: serde_json::Value = serde_json::from_slice(raw).map_err(|error| {
                ExitError::invalid_value(format!(
                    "domain-pack: local JSON artifact '{}' cannot be canonicalized: {error}",
                    path.display()
                ))
            })?;
            canonical_digest(&value)
        }
        DomainPackRemoteArtifactMediaType::TextPlain => {
            let text = std::str::from_utf8(raw).map_err(|error| {
                ExitError::invalid_value(format!(
                    "domain-pack: local text artifact '{}' is not UTF-8: {error}",
                    path.display()
                ))
            })?;
            canonical_digest(&text)
        }
        DomainPackRemoteArtifactMediaType::ApplicationOctetStream => Ok(sha256_bytes(raw)),
    }
}

/// Persist a generic authoring template only into a fresh, explicitly supplied
/// directory. Every output file is an exclusive create beneath a non-link root;
/// this prevents a skeleton command from replacing or following operator files.
fn write_author_skeleton_template(
    output_root: &Path,
    files: [(&str, &[u8]); 4],
) -> Result<(), ExitError> {
    // Validate every generated reference before reserving a new root. This
    // keeps an invalid decision output from stranding an otherwise empty root.
    let mut unique_references = std::collections::BTreeSet::new();
    let mut prepared = Vec::with_capacity(files.len());
    for (reference, bytes) in files {
        let relative = author_output_relative_path(reference)?;
        if !unique_references.insert(relative.clone()) {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: generated author template repeats output path '{reference}'"
            )));
        }
        prepared.push((relative, bytes));
    }
    for (index, (left, _)) in prepared.iter().enumerate() {
        for (right, _) in prepared.iter().skip(index + 1) {
            if left.starts_with(right) || right.starts_with(left) {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: generated author template output paths '{}' and '{}' conflict as ancestor and descendant",
                    left.display(),
                    right.display()
                )));
            }
        }
    }

    let canonical_root = prepare_author_output_root(output_root)?;
    let write_result = (|| {
        for (relative, _) in &prepared {
            preflight_author_output_target(&canonical_root, relative)?;
        }
        for (relative, bytes) in prepared {
            write_author_output_file_new(&canonical_root, &relative, bytes)?;
        }
        Ok(())
    })();
    if let Err(error) = write_result {
        if let Err(cleanup_error) = std::fs::remove_dir_all(&canonical_root) {
            if cleanup_error.kind() != std::io::ErrorKind::NotFound {
                return Err(ExitError::failed(format!(
                    "domain-pack: generated author template failed ({error}); additionally cannot remove newly-created --output-root '{}': {cleanup_error}",
                    canonical_root.display()
                )));
            }
        }
        return Err(error);
    }
    Ok(())
}

/// Reject links, special files, and missing ancestors in every component of
/// the caller-supplied root. Checking only the final root would allow a path
/// such as `linked-parent/empty-root` to redirect generated bytes.
fn validate_author_output_root_components(output_root: &Path) -> Result<(), ExitError> {
    let mut current = if output_root.is_absolute() {
        PathBuf::new()
    } else {
        std::fs::canonicalize(".").map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot resolve current directory for --output-root: {error}"
            ))
        })?
    };
    let normal_components = output_root
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count();
    let mut normal_index = 0usize;
    for component in output_root.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: --output-root must not contain parent traversal: {}",
                    output_root.display()
                )));
            }
            Component::Normal(name) => {
                if matches!(
                    name,
                    name if name == std::ffi::OsStr::new(".forge-method")
                        || name == std::ffi::OsStr::new(".git")
                ) {
                    return Err(ExitError::invalid_value(format!(
                        "domain-pack: --output-root must not be inside protected lifecycle or repository state: {}",
                        output_root.display()
                    )));
                }
                normal_index += 1;
                current.push(name);
                match std::fs::symlink_metadata(&current) {
                    Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                        return Err(ExitError::invalid_value(format!(
                            "domain-pack: --output-root component '{}' must be a direct directory, not a symbolic link or special file",
                            current.display()
                        )));
                    }
                    Ok(_) => {}
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound
                            && normal_index == normal_components =>
                    {
                        return Ok(());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        return Err(ExitError::invalid_value(format!(
                            "domain-pack: --output-root parent component '{}' does not exist",
                            current.display()
                        )));
                    }
                    Err(error) => {
                        return Err(ExitError::failed(format!(
                            "domain-pack: cannot inspect --output-root component '{}': {error}",
                            current.display()
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}

fn prepare_author_output_root(output_root: &Path) -> Result<PathBuf, ExitError> {
    validate_author_output_root_components(output_root)?;
    match std::fs::symlink_metadata(output_root) {
        Ok(_) => Err(ExitError::conflict(format!(
            "domain-pack: --output-root '{}' must not already exist",
            output_root.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = output_root
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let name = output_root
                .file_name()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    ExitError::invalid_value("domain-pack: --output-root must name a directory")
                })?;
            let parent_metadata = std::fs::symlink_metadata(parent).map_err(|error| {
                ExitError::failed(format!(
                    "domain-pack: cannot inspect --output-root parent '{}': {error}",
                    parent.display()
                ))
            })?;
            if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: --output-root parent '{}' must be a direct directory, not a symbolic link or special file",
                    parent.display()
                )));
            }
            let canonical_parent = std::fs::canonicalize(parent).map_err(|error| {
                ExitError::failed(format!(
                    "domain-pack: cannot resolve --output-root parent '{}': {error}",
                    parent.display()
                ))
            })?;
            let root = canonical_parent.join(name);
            std::fs::create_dir(&root).map_err(|error| {
                ExitError::conflict(format!(
                    "domain-pack: cannot reserve new --output-root '{}': {error}",
                    output_root.display()
                ))
            })?;
            // `create_dir` is an exclusive reservation. The parent was checked
            // above, and concurrent namespace swaps remain deferred to C3.2.
            Ok(root)
        }
        Err(error) => Err(ExitError::failed(format!(
            "domain-pack: cannot inspect --output-root '{}': {error}",
            output_root.display()
        ))),
    }
}

fn author_output_relative_path(reference: &str) -> Result<PathBuf, ExitError> {
    let path = Path::new(reference);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: generated author template path must be normalized and relative: {reference}"
        )));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::Normal(name)
                if name == std::ffi::OsStr::new(".forge-method")
                    || name == std::ffi::OsStr::new(".git")
        )
    }) {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: generated author template path must not target protected lifecycle or repository state: {reference}"
        )));
    }
    Ok(path.to_path_buf())
}

fn preflight_author_output_target(root: &Path, relative: &Path) -> Result<(), ExitError> {
    let components = relative.components().collect::<Vec<_>>();
    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            unreachable!("author output references were normalized before preflight");
        };
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(_metadata) if index + 1 == components.len() => {
                return Err(ExitError::conflict(format!(
                    "domain-pack: generated author template output '{}' already exists",
                    current.display()
                )));
            }
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: generated author template parent '{}' is a symbolic link or non-directory",
                    current.display()
                )));
            }
            Ok(_) => {
                let canonical = std::fs::canonicalize(&current).map_err(|error| {
                    ExitError::failed(format!(
                        "domain-pack: cannot resolve generated author template parent '{}': {error}",
                        current.display()
                    ))
                })?;
                if !canonical.starts_with(root) {
                    return Err(ExitError::invalid_value(format!(
                        "domain-pack: generated author template path escapes --output-root: {}",
                        relative.display()
                    )));
                }
                current = canonical;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(ExitError::failed(format!(
                    "domain-pack: cannot inspect generated author template path '{}': {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(())
}

fn write_author_output_file_new(
    root: &Path,
    relative: &Path,
    bytes: &[u8],
) -> Result<(), ExitError> {
    let components = relative.components().collect::<Vec<_>>();
    let (file_name, parents) = components.split_last().expect("author output is non-empty");
    let Component::Normal(file_name) = file_name else {
        unreachable!("author output references were normalized before writing");
    };
    let mut parent = root.to_path_buf();
    for component in parents {
        let Component::Normal(component) = component else {
            unreachable!("author output references were normalized before writing");
        };
        parent.push(component);
        match std::fs::symlink_metadata(&parent) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: generated author template parent '{}' is a symbolic link or non-directory",
                    parent.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&parent).map_err(|error| {
                    ExitError::failed(format!(
                        "domain-pack: cannot create generated author template directory '{}': {error}",
                        parent.display()
                    ))
                })?;
            }
            Err(error) => {
                return Err(ExitError::failed(format!(
                    "domain-pack: cannot inspect generated author template directory '{}': {error}",
                    parent.display()
                )));
            }
        }
        let canonical = std::fs::canonicalize(&parent).map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot resolve generated author template directory '{}': {error}",
                parent.display()
            ))
        })?;
        if !canonical.starts_with(root) {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: generated author template path escapes --output-root: {}",
                relative.display()
            )));
        }
        parent = canonical;
    }
    let target = parent.join(file_name);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let mut file = options.open(&target).map_err(|error| {
        ExitError::conflict(format!(
            "domain-pack: cannot exclusively create generated author template output '{}': {error}",
            target.display()
        ))
    })?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&target);
        return Err(ExitError::failed(format!(
            "domain-pack: cannot persist generated author template output '{}': {error}",
            target.display()
        )));
    }
    Ok(())
}

fn safe_join(root: &Path, reference: &str) -> Result<PathBuf, ExitError> {
    let reference = Path::new(reference);
    if reference.is_absolute()
        || reference.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ExitError::failed(format!(
            "domain-pack: artifact ref must remain relative to --artifact-root: {}",
            reference.display()
        )));
    }
    let canonical_root = std::fs::canonicalize(root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve --artifact-root '{}': {error}",
            root.display()
        ))
    })?;
    let candidate = root.join(reference);
    let candidate_metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect artifact '{}' below --artifact-root '{}': {error}",
            candidate.display(),
            root.display()
        ))
    })?;
    if candidate_metadata.file_type().is_symlink() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: artifact ref must name a regular file, not a symbolic link or junction: {}",
            reference.display()
        )));
    }
    let canonical_candidate = std::fs::canonicalize(&candidate).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve artifact '{}' below --artifact-root '{}': {error}",
            candidate.display(),
            root.display()
        ))
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(ExitError::failed(format!(
            "domain-pack: artifact ref escapes canonical --artifact-root: {}",
            reference.display()
        )));
    }
    if !candidate_metadata.is_file() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: artifact ref must name a regular file: {}",
            reference.display()
        )));
    }
    Ok(canonical_candidate)
}

fn usage() -> String {
    let mut output = String::from("usage:");
    for line in COMMAND_DOMAIN_PACK.usage_lines {
        output.push('\n');
        output.push_str("  ");
        output.push_str(line.trim_start());
    }
    output.push_str(
        "\n  validate, search, explain, acquire plan, compose, and resolve are read-only candidate projections; trust-provision requires explicit operator approval and mutates only the external monotonic trust anchor; status/recover integrity-check lifecycle state and may complete an interrupted pointer replacement",
    );
    output
}

#[cfg(test)]
mod backup_seam_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn state_root() -> PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "forge-domain-pack-lock-seam-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(root.join("domain-packs")).unwrap();
        root
    }

    #[test]
    fn retained_state_file_authorities_project_exact_recovered_bytes() {
        let root = state_root();
        let sources = b"schema_version: exact-sources\n# preserved\n";
        let rebase = b"schema_version: exact-rebase\n";
        std::fs::write(
            root.join(DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH),
            sources,
        )
        .unwrap();
        std::fs::write(root.join(DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH), rebase).unwrap();

        let mut source_guard = lock_domain_pack_operator_sources(&root).unwrap();
        let source_snapshot = snapshot_domain_pack_operator_sources(&mut source_guard).unwrap();
        assert_eq!(
            source_snapshot.relative_path(),
            DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH
        );
        assert_eq!(source_snapshot.raw(), Some(sources.as_slice()));
        drop(source_guard);

        let mut rebase_guard = lock_domain_pack_rebase_plan(&root).unwrap();
        let rebase_snapshot = snapshot_domain_pack_rebase_plan(&mut rebase_guard).unwrap();
        assert_eq!(
            rebase_snapshot.relative_path(),
            DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH
        );
        assert_eq!(rebase_snapshot.raw(), Some(rebase.as_slice()));
        drop(rebase_guard);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn retained_domain_pack_state_fails_closed_after_parent_swap() {
        let root = state_root();
        std::fs::write(
            root.join(DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH),
            b"trusted: true\n",
        )
        .unwrap();
        let mut retained = lock_domain_pack_operator_sources(&root).unwrap();
        let moved = root.with_extension("retained");
        std::fs::rename(&root, &moved).unwrap();
        std::fs::create_dir_all(root.join("domain-packs")).unwrap();
        std::fs::write(
            root.join(DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH),
            b"attacker: true\n",
        )
        .unwrap();
        assert!(lock_domain_pack_operator_sources(&root).is_err());

        assert!(snapshot_domain_pack_operator_sources(&mut retained).is_err());
        drop(retained);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(moved);
    }

    #[test]
    fn retained_domain_pack_state_rejects_byte_identical_post_recovery_replacement() {
        let root = state_root();
        let target = root.join(DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH);
        std::fs::write(&target, b"trusted: true\n").unwrap();
        let mut retained = lock_domain_pack_operator_sources(&root).unwrap();
        let replacement = target.with_extension("replacement");
        std::fs::write(&replacement, b"trusted: true\n").unwrap();
        std::fs::remove_file(&target).unwrap();
        std::fs::rename(&replacement, &target).unwrap();

        assert!(snapshot_domain_pack_operator_sources(&mut retained).is_err());
        drop(retained);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn operator_source_persistence_rejects_byte_identical_session_substitution() {
        let root = state_root();
        let target = root.join(DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH);
        std::fs::write(&target, b"trusted: true\n").unwrap();
        let retained = lock_domain_pack_operator_sources(&root).unwrap();
        let replacement = target.with_extension("replacement");
        std::fs::write(&replacement, b"trusted: true\n").unwrap();
        std::fs::remove_file(&target).unwrap();
        std::fs::rename(&replacement, &target).unwrap();

        let reconciliation = retained.reconciliation.unwrap();
        assert!(replace_domain_pack_file_exact(
            reconciliation,
            b"trusted: updated\n",
            "operator-source binding",
        )
        .is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"trusted: true\n");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rebase_plan_persistence_rejects_late_creation_after_recovered_absence() {
        let root = state_root();
        let target = root.join(DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH);
        let retained = lock_domain_pack_rebase_plan(&root).unwrap();
        std::fs::write(&target, b"attacker: true\n").unwrap();

        let reconciliation = retained.reconciliation.unwrap();
        assert!(
            replace_domain_pack_file_exact(reconciliation, b"trusted: true\n", "rebase plan",)
                .is_err()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"attacker: true\n");
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn retained_domain_pack_state_rejects_hard_linked_target() {
        let root = state_root();
        let target = root.join(DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH);
        std::fs::write(&target, b"trusted: true\n").unwrap();
        let mut retained = lock_domain_pack_rebase_plan(&root).unwrap();
        std::fs::hard_link(&target, target.with_extension("alias")).unwrap();

        assert!(snapshot_domain_pack_rebase_plan(&mut retained).is_err());
        drop(retained);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn retained_operator_anchor_rejects_byte_identical_replacement() {
        let root = state_root();
        let registry = root.join("supply-registry.yaml");
        let anchor = root.join(DOMAIN_PACK_REGISTRY_ANCHOR_RELATIVE_PATH);
        std::fs::write(&registry, b"public: registry\n").unwrap();
        std::fs::write(&anchor, b"protected: anchor\n").unwrap();
        let mut retained = lock_operator_registry_anchor(&registry).unwrap();
        let replacement = root.join("anchor-replacement.yaml");
        std::fs::write(&replacement, b"protected: anchor\n").unwrap();
        std::fs::remove_file(&anchor).unwrap();
        std::fs::rename(&replacement, &anchor).unwrap();

        assert!(snapshot_operator_registry_anchor(&mut retained).is_err());
        drop(retained);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn retained_operator_anchor_rejects_hard_linked_anchor() {
        let root = state_root();
        let registry = root.join("supply-registry.yaml");
        let anchor = root.join(DOMAIN_PACK_REGISTRY_ANCHOR_RELATIVE_PATH);
        std::fs::write(&registry, b"public: registry\n").unwrap();
        std::fs::write(&anchor, b"protected: anchor\n").unwrap();
        let mut retained = lock_operator_registry_anchor(&registry).unwrap();
        std::fs::hard_link(&anchor, root.join("anchor-alias.yaml")).unwrap();

        assert!(snapshot_operator_registry_anchor(&mut retained).is_err());
        drop(retained);
        let _ = std::fs::remove_dir_all(root);
    }
}
