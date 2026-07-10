//! P4a execution-admission policy decision point.
//!
//! This module is deliberately pure: it accepts a commit-time snapshot of the
//! Assurance Case, operation contracts, principal proof, replay reservation,
//! claims, gates, and commit guarantees, then returns a typed verdict. It does
//! not trust host confidence, a caller-supplied public key, or the mere
//! presence of a contract path.
//!
//! P4a does **not** mutate state and does not itself authenticate a transport.
//! The MCP/CLI adapter must derive the observations from trusted runtime state,
//! and the kernel must evaluate this decision immediately before mutation.

use crate::{check_write_against_claims, is_live, WriteCheck};
use forge_core_contracts::{
    claim::{ActorRole, ClaimContractDocument},
    command::{CommandSideEffectPolicy, NetworkPolicy},
    gate::{GateContractDocument, GateStatus},
    operation::{CallerRole, MutationPolicy, OperationGateStatus, OperationSideEffectPolicy},
    tool_effect::{EffectTargetKind, InverseKind, ToolEffectContractDocument},
    AssuranceCaseDocument, CommandContractDocument, ObligationCriticality, ObligationStatus,
    OperationContractDocument, PrincipalId, ReadinessTarget, ReadinessVerdict, RepoPath, StableId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt::Write as _;

pub const EXECUTION_ADMISSION_SCHEMA_VERSION: &str = "0.1";
pub const EXECUTION_AUTHORITY_SCOPE: &str = "operation.execute";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionAdmissionInputDocument {
    pub schema_version: String,
    pub execution_admission: ExecutionAdmissionInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionAdmissionInput {
    pub request: ExecutionAdmissionRequest,
    pub assurance_case: AssuranceCaseDocument,
    pub operation: OperationContractDocument,
    pub command_contracts: Vec<CommandContractDocument>,
    pub effect_contracts: Vec<EffectContractBinding>,
    pub principal: ExecutionPrincipalObservation,
    pub replay: ReplayProtectionObservation,
    pub claim_snapshot: ClaimSnapshotObservation,
    pub gate_snapshot: GateSnapshotObservation,
    pub commit: CommitAssuranceObservation,
    pub current_state_version: u64,
    pub now_unix: i64,
    pub max_attestation_age_seconds: u64,
    pub max_future_skew_seconds: u64,
}

/// The exact intent that an Adapter must authenticate and reserve against
/// replay. The digest covers every authority-bearing reference and revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionAdmissionRequest {
    pub id: StableId,
    pub principal_id: PrincipalId,
    pub agent_id: StableId,
    pub principal_role: CallerRole,
    pub operation_id: StableId,
    pub operation_token: String,
    pub assurance_case_id: StableId,
    pub assurance_case_token: String,
    pub command_bindings: Vec<ContentAddressedBinding>,
    pub effect_bindings: Vec<ContentAddressedBinding>,
    pub expected_claim_snapshot_revision: u64,
    pub expected_claim_revisions: Vec<RevisionExpectation>,
    pub expected_gate_snapshot_revision: u64,
    pub expected_gate_revisions: Vec<RevisionExpectation>,
    /// Canonical digest of every mutable authority observation consumed by
    /// Admission. The transport attestation covers this token through the
    /// complete execution-intent digest.
    #[serde(default)]
    pub authority_snapshot_token: String,
    pub expected_replay_reservation_revision: u64,
    pub nonce: String,
    pub issued_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentAddressedBinding {
    #[serde(rename = "ref")]
    pub reference: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionExpectation {
    #[serde(rename = "ref")]
    pub reference: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectContractBinding {
    pub effect_ref: RepoPath,
    pub document: ToolEffectContractDocument,
}

/// Authentication and authorization facts produced by the trusted Adapter.
/// `SignatureVerified` is intentionally insufficient: a caller-chosen key can
/// prove origin without proving that the origin is authorized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPrincipalObservation {
    pub principal_id: PrincipalId,
    pub agent_id: StableId,
    pub role: CallerRole,
    pub trust: ExecutionPrincipalTrust,
    pub credential_id: String,
    pub audience: String,
    pub required_audience: String,
    pub authority_grants: Vec<StableId>,
    pub attested_intent_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPrincipalTrust {
    SelfAsserted,
    SignatureVerified,
    AuthorizedKeyRegistry,
    OAuthResourceServer,
}

impl ExecutionPrincipalTrust {
    const fn authorizes_mutation(self) -> bool {
        matches!(
            self,
            Self::AuthorizedKeyRegistry | Self::OAuthResourceServer
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayProtectionObservation {
    pub status: ReplayReservationStatus,
    pub nonce: String,
    pub reserved_intent_digest: String,
    pub reservation_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayReservationStatus {
    Missing,
    FreshReserved,
    AlreadySeen,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimSnapshotObservation {
    pub revision: u64,
    pub completeness: SnapshotCompleteness,
    pub claims: Vec<ClaimRevisionObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRevisionObservation {
    pub claim_ref: RepoPath,
    pub revision: u64,
    pub document: ClaimContractDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateSnapshotObservation {
    pub revision: u64,
    pub completeness: SnapshotCompleteness,
    pub gates: Vec<GateRevisionObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateRevisionObservation {
    pub gate_ref: RepoPath,
    pub revision: u64,
    pub observed_state_version: u64,
    pub document: GateContractDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotCompleteness {
    Complete,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitAssuranceObservation {
    pub strategy: ExecutionCommitStrategy,
    pub scope: ExecutionCommitScope,
    pub wal_lock: GuaranteeStatus,
    pub rollback_recovery: GuaranteeStatus,
    pub durable_commit_record: GuaranteeStatus,
    pub compensation: CompensationCoverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionCommitStrategy {
    SingleEffectWal,
    OperationWideWal,
    Saga,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionCommitScope {
    SingleEffect,
    WholeOperation,
    PerEffectOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuaranteeStatus {
    Verified,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationCoverage {
    NotApplicable,
    Complete,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionAdmissionDecision {
    pub status: ExecutionAdmissionStatus,
    pub request_id: StableId,
    pub intent_digest: String,
    pub issues: Vec<ExecutionAdmissionIssue>,
    pub validated_claim_revisions: Vec<RevisionExpectation>,
    pub validated_gate_revisions: Vec<RevisionExpectation>,
    pub commit_strategy: ExecutionCommitStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionAdmissionStatus {
    Admitted,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionAdmissionIssue {
    pub code: ExecutionAdmissionIssueCode,
    pub subject: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionAdmissionIssueCode {
    AssuranceCaseMismatch,
    AssuranceTokenMismatch,
    AssuranceNotReady,
    AssuranceSnapshotStale,
    AssuranceObligationUnresolved,
    AssuranceDecisionPending,
    AssuranceCapabilityGap,
    OperationMismatch,
    OperationTokenMismatch,
    OperationStateVersionStale,
    OperationMutationForbidden,
    OperationAuthorityIncomplete,
    OperationDiagnosticsPresent,
    OperationGateSummaryNotPassed,
    CommandBindingMismatch,
    CommandTokenMismatch,
    RequiredCommandMissing,
    CommandNotSafelyReadOnly,
    EffectBindingMismatch,
    EffectTokenMismatch,
    EffectOperationMismatch,
    EffectPrincipalMismatch,
    PrincipalMismatch,
    PrincipalRoleMismatch,
    PrincipalNotTrusted,
    PrincipalCredentialMissing,
    PrincipalAudienceMismatch,
    PrincipalScopeMissing,
    AttestedIntentMismatch,
    AuthoritySnapshotTokenMismatch,
    InvocationNonceMissing,
    InvocationReplayRejected,
    InvocationExpired,
    InvocationFromFuture,
    ReplayIntentMismatch,
    ReplayRevisionMismatch,
    DuplicateBinding,
    ClaimSnapshotIncomplete,
    ClaimSnapshotRevisionMismatch,
    ClaimMissing,
    ClaimRevisionMismatch,
    ClaimNotLive,
    ClaimPrincipalMismatch,
    ClaimRoleMismatch,
    ClaimStateVersionMismatch,
    ClaimConflict,
    ClaimCoverageMissing,
    GateSnapshotIncomplete,
    GateSnapshotRevisionMismatch,
    GateMissing,
    GateRevisionMismatch,
    GateNotPassed,
    GateEvidenceMissing,
    GateStateVersionMismatch,
    CommitStrategyUnsupported,
    CommitScopeInsufficient,
    CommitGuaranteeMissing,
    SagaCompensationIncomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionAdmissionRejection {
    UnsupportedSchemaVersion {
        found: String,
    },
    Canonicalization {
        subject: &'static str,
        source: String,
    },
}

impl std::fmt::Display for ExecutionAdmissionRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found } => write!(
                formatter,
                "unsupported execution admission schema version '{found}'; expected {EXECUTION_ADMISSION_SCHEMA_VERSION}"
            ),
            Self::Canonicalization { subject, source } => {
                write!(formatter, "cannot canonicalize {subject}: {source}")
            }
        }
    }
}

impl std::error::Error for ExecutionAdmissionRejection {}

/// Content-address an Assurance Case using the same token format exposed by
/// the P3 conversational Adapter.
///
/// # Errors
///
/// Returns [`ExecutionAdmissionRejection::Canonicalization`] if the typed case
/// cannot be serialized into canonical JSON.
pub fn assurance_case_token(
    document: &AssuranceCaseDocument,
) -> Result<String, ExecutionAdmissionRejection> {
    canonical_digest(document, "Assurance Case")
}

/// Content-address an Operation Contract for an execution request binding.
///
/// # Errors
///
/// Returns [`ExecutionAdmissionRejection::Canonicalization`] if the typed
/// contract cannot be serialized into canonical JSON.
pub fn operation_contract_token(
    document: &OperationContractDocument,
) -> Result<String, ExecutionAdmissionRejection> {
    canonical_digest(document, "Operation Contract")
}

/// Content-address a Command Contract for an execution request binding.
///
/// # Errors
///
/// Returns [`ExecutionAdmissionRejection::Canonicalization`] if the typed
/// contract cannot be serialized into canonical JSON.
pub fn command_contract_token(
    document: &CommandContractDocument,
) -> Result<String, ExecutionAdmissionRejection> {
    canonical_digest(document, "Command Contract")
}

/// Content-address a Tool Effect Contract for an execution request binding.
///
/// # Errors
///
/// Returns [`ExecutionAdmissionRejection::Canonicalization`] if the typed
/// contract cannot be serialized into canonical JSON.
pub fn effect_contract_token(
    document: &ToolEffectContractDocument,
) -> Result<String, ExecutionAdmissionRejection> {
    canonical_digest(document, "Tool Effect Contract")
}

/// Content-address the complete authority-bearing execution intent.
///
/// # Errors
///
/// Returns [`ExecutionAdmissionRejection::Canonicalization`] if the typed
/// request cannot be serialized into canonical JSON.
pub fn execution_intent_digest(
    request: &ExecutionAdmissionRequest,
) -> Result<String, ExecutionAdmissionRejection> {
    canonical_digest(request, "execution admission request")
}

/// Content-address all mutable authority observations consumed by Admission.
///
/// The Assurance Case has its own token in the request. This token covers the
/// claim and gate snapshots, current state version, and trusted clock so none
/// can be changed after the execution intent is signed.
///
/// # Errors
///
/// Returns [`ExecutionAdmissionRejection::Canonicalization`] if the snapshot
/// cannot be serialized into canonical JSON.
pub fn authority_snapshot_token(
    claim_snapshot: &ClaimSnapshotObservation,
    gate_snapshot: &GateSnapshotObservation,
    current_state_version: u64,
    now_unix: i64,
) -> Result<String, ExecutionAdmissionRejection> {
    #[derive(Serialize)]
    #[serde(deny_unknown_fields)]
    struct AuthoritySnapshotBinding<'a> {
        claim_snapshot: &'a ClaimSnapshotObservation,
        gate_snapshot: &'a GateSnapshotObservation,
        current_state_version: u64,
        now_unix: i64,
    }

    canonical_digest(
        &AuthoritySnapshotBinding {
            claim_snapshot,
            gate_snapshot,
            current_state_version,
            now_unix,
        },
        "execution authority snapshot",
    )
}

fn canonical_digest<T: Serialize>(
    value: &T,
    subject: &'static str,
) -> Result<String, ExecutionAdmissionRejection> {
    let canonical = serde_json_canonicalizer::to_vec(value).map_err(|error| {
        ExecutionAdmissionRejection::Canonicalization {
            subject,
            source: error.to_string(),
        }
    })?;
    let digest = Sha256::digest(canonical);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(format!("sha256:{hex}"))
}

/// Evaluate a commit-time execution snapshot. Any issue blocks admission; the
/// caller gets every deterministic reason so the host agent can self-correct.
///
/// # Errors
///
/// Returns [`ExecutionAdmissionRejection::UnsupportedSchemaVersion`] for an
/// incompatible input document, or
/// [`ExecutionAdmissionRejection::Canonicalization`] if a content-addressed
/// binding cannot be produced.
pub fn evaluate_execution_admission(
    document: &ExecutionAdmissionInputDocument,
) -> Result<ExecutionAdmissionDecision, ExecutionAdmissionRejection> {
    if document.schema_version != EXECUTION_ADMISSION_SCHEMA_VERSION {
        return Err(ExecutionAdmissionRejection::UnsupportedSchemaVersion {
            found: document.schema_version.clone(),
        });
    }

    let input = &document.execution_admission;
    let request = &input.request;
    let intent_digest = execution_intent_digest(request)?;
    let mut issues = Vec::new();

    evaluate_assurance(input, &mut issues)?;
    evaluate_authority_snapshot(input, &mut issues)?;
    evaluate_operation_and_bindings(input, &mut issues)?;
    evaluate_principal_and_replay(input, &intent_digest, &mut issues);
    let validated_claim_revisions = evaluate_claims(input, &mut issues);
    let validated_gate_revisions = evaluate_gates(input, &mut issues);
    evaluate_commit(input, &mut issues);

    issues.sort();
    issues.dedup();
    Ok(ExecutionAdmissionDecision {
        status: if issues.is_empty() {
            ExecutionAdmissionStatus::Admitted
        } else {
            ExecutionAdmissionStatus::Blocked
        },
        request_id: request.id.clone(),
        intent_digest,
        issues,
        validated_claim_revisions,
        validated_gate_revisions,
        commit_strategy: input.commit.strategy,
    })
}

fn evaluate_authority_snapshot(
    input: &ExecutionAdmissionInput,
    issues: &mut Vec<ExecutionAdmissionIssue>,
) -> Result<(), ExecutionAdmissionRejection> {
    let computed = authority_snapshot_token(
        &input.claim_snapshot,
        &input.gate_snapshot,
        input.current_state_version,
        input.now_unix,
    )?;
    if input.request.authority_snapshot_token != computed {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::AuthoritySnapshotTokenMismatch,
            "authority_snapshot",
        );
    }
    Ok(())
}

fn evaluate_assurance(
    input: &ExecutionAdmissionInput,
    issues: &mut Vec<ExecutionAdmissionIssue>,
) -> Result<(), ExecutionAdmissionRejection> {
    let request = &input.request;
    let case = &input.assurance_case.assurance_case;
    if request.assurance_case_id != case.id {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::AssuranceCaseMismatch,
            &case.id.0,
        );
    }
    let computed_token = assurance_case_token(&input.assurance_case)?;
    if request.assurance_case_token != computed_token {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::AssuranceTokenMismatch,
            &case.id.0,
        );
    }
    if case.readiness.target.rank() < ReadinessTarget::Execute.rank()
        || case.readiness.verdict != ReadinessVerdict::Ready
        || !case.readiness.blocker_refs.is_empty()
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::AssuranceNotReady,
            &case.id.0,
        );
    }
    if case.project_snapshot.state_version != input.current_state_version {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::AssuranceSnapshotStale,
            &case.project_snapshot.id.0,
        );
    }
    for obligation in &case.obligations {
        if obligation.required_before.rank() <= ReadinessTarget::Execute.rank()
            && obligation.criticality != ObligationCriticality::Advisory
            && obligation.status != ObligationStatus::Satisfied
        {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::AssuranceObligationUnresolved,
                &obligation.id.0,
            );
        }
    }
    for decision in &case.decision_requests {
        if decision.blocking && decision.blocks_before.rank() <= ReadinessTarget::Execute.rank() {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::AssuranceDecisionPending,
                &decision.id.0,
            );
        }
    }
    for gap in &case.capability_gaps {
        if gap.blocking && gap.blocks_before.rank() <= ReadinessTarget::Execute.rank() {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::AssuranceCapabilityGap,
                &gap.id.0,
            );
        }
    }
    Ok(())
}

fn evaluate_operation_and_bindings(
    input: &ExecutionAdmissionInput,
    issues: &mut Vec<ExecutionAdmissionIssue>,
) -> Result<(), ExecutionAdmissionRejection> {
    let request = &input.request;
    let operation = &input.operation.operation_contract;
    if request.operation_id != operation.contract_id {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::OperationMismatch,
            &operation.contract_id.0,
        );
    }
    if request.operation_token != operation_contract_token(&input.operation)? {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::OperationTokenMismatch,
            &operation.contract_id.0,
        );
    }
    if operation.project_ref.state_version != input.current_state_version
        || operation
            .coordination_scope
            .concurrency
            .expected_state_version
            != input.current_state_version
        || operation.project_ref.state_version
            != input
                .assurance_case
                .assurance_case
                .project_snapshot
                .state_version
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::OperationStateVersionStale,
            &operation.contract_id.0,
        );
    }
    if operation.authority.mutation_policy != MutationPolicy::Allowed
        || operation.authority.side_effect_policy == OperationSideEffectPolicy::ReadOnly
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::OperationMutationForbidden,
            &operation.contract_id.0,
        );
    }
    if !operation.authority.missing_authority.is_empty() {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::OperationAuthorityIncomplete,
            &operation.contract_id.0,
        );
    }
    if !operation.diagnostics.errors.is_empty() {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::OperationDiagnosticsPresent,
            &operation.contract_id.0,
        );
    }
    if operation.gates.current_gate_status != OperationGateStatus::Pass {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::OperationGateSummaryNotPassed,
            &operation.contract_id.0,
        );
    }

    let request_commands = binding_refs(&request.command_bindings);
    let observed_command_ids = input
        .command_contracts
        .iter()
        .map(|document| document.command_contract.id.clone())
        .collect::<Vec<_>>();
    let observed_commands = stable_ids(&observed_command_ids);
    if has_duplicate_bindings(&request.command_bindings)
        || has_duplicate_stable_ids(&observed_command_ids)
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::DuplicateBinding,
            "command_bindings",
        );
    }
    if request_commands != observed_commands {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::CommandBindingMismatch,
            &operation.contract_id.0,
        );
    }
    let declared_commands: BTreeSet<String> = operation
        .command_refs
        .iter()
        .map(|reference| reference.id.0.clone())
        .collect();
    for required in operation.command_refs.iter().filter(|item| item.required) {
        if !observed_commands.contains(&required.id.0) {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::RequiredCommandMissing,
                &required.id.0,
            );
        }
    }
    for command in &input.command_contracts {
        let contract = &command.command_contract;
        if !declared_commands.contains(&contract.id.0) {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::CommandBindingMismatch,
                &contract.id.0,
            );
        }
        let computed_token = command_contract_token(command)?;
        let token_matches = request
            .command_bindings
            .iter()
            .find(|binding| binding.reference == contract.id.0)
            .is_some_and(|binding| binding.token == computed_token);
        if !token_matches {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::CommandTokenMismatch,
                &contract.id.0,
            );
        }
        if contract.side_effect_policy != CommandSideEffectPolicy::ReadOnly
            || contract.network_policy != NetworkPolicy::Disabled
            || contract.safety.shell_string_allowed
            || contract.safety.writes_files
            || contract.safety.publishes
            || contract.safety.installs_packages
        {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::CommandNotSafelyReadOnly,
                &contract.id.0,
            );
        }
    }

    let request_effects = binding_refs(&request.effect_bindings);
    let observed_effect_refs = input
        .effect_contracts
        .iter()
        .map(|binding| binding.effect_ref.clone())
        .collect::<Vec<_>>();
    let observed_effects = repo_paths(&observed_effect_refs);
    if has_duplicate_bindings(&request.effect_bindings)
        || has_duplicate_repo_paths(&observed_effect_refs)
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::DuplicateBinding,
            "effect_bindings",
        );
    }
    let declared_effects = repo_paths(&operation.effect_contract_refs);
    if request_effects != observed_effects || observed_effects != declared_effects {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::EffectBindingMismatch,
            &operation.contract_id.0,
        );
    }
    for binding in &input.effect_contracts {
        let effect = &binding.document.tool_effect_contract;
        let computed_token = effect_contract_token(&binding.document)?;
        let token_matches = request
            .effect_bindings
            .iter()
            .find(|request_binding| request_binding.reference == binding.effect_ref.0)
            .is_some_and(|request_binding| request_binding.token == computed_token);
        if !token_matches {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::EffectTokenMismatch,
                &binding.effect_ref.0,
            );
        }
        if effect.operation_ref != operation.contract_id {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::EffectOperationMismatch,
                &binding.effect_ref.0,
            );
        }
        if effect.actor.agent_id != input.principal.agent_id
            || !caller_role_matches_actor(input.principal.role, effect.actor.role)
        {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::EffectPrincipalMismatch,
                &binding.effect_ref.0,
            );
        }
    }
    Ok(())
}

fn evaluate_principal_and_replay(
    input: &ExecutionAdmissionInput,
    intent_digest: &str,
    issues: &mut Vec<ExecutionAdmissionIssue>,
) {
    let request = &input.request;
    let principal = &input.principal;
    if request.principal_id != principal.principal_id
        || request.agent_id != principal.agent_id
        || input
            .operation
            .operation_contract
            .coordination_scope
            .concurrency
            .agent_id
            .as_ref()
            .is_some_and(|agent| agent != &principal.agent_id)
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::PrincipalMismatch,
            &principal.agent_id.0,
        );
    }
    if request.principal_role != principal.role
        || input
            .operation
            .operation_contract
            .coordination_scope
            .concurrency
            .caller_role
            != principal.role
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::PrincipalRoleMismatch,
            &principal.agent_id.0,
        );
    }
    if !principal.trust.authorizes_mutation() {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::PrincipalNotTrusted,
            &principal.agent_id.0,
        );
    }
    if principal.credential_id.trim().is_empty() {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::PrincipalCredentialMissing,
            &principal.agent_id.0,
        );
    }
    if principal.audience.trim().is_empty()
        || principal.required_audience.trim().is_empty()
        || principal.audience != principal.required_audience
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::PrincipalAudienceMismatch,
            &principal.agent_id.0,
        );
    }
    if !principal
        .authority_grants
        .iter()
        .any(|grant| grant.0 == EXECUTION_AUTHORITY_SCOPE)
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::PrincipalScopeMissing,
            &principal.agent_id.0,
        );
    }
    if principal.attested_intent_digest != intent_digest {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::AttestedIntentMismatch,
            &request.id.0,
        );
    }

    if request.nonce.trim().is_empty() || input.replay.nonce.trim().is_empty() {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::InvocationNonceMissing,
            &request.id.0,
        );
    }
    if input.replay.status != ReplayReservationStatus::FreshReserved {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::InvocationReplayRejected,
            &request.id.0,
        );
    }
    if input.replay.reservation_revision == 0
        || request.expected_replay_reservation_revision == 0
        || input.replay.reservation_revision != request.expected_replay_reservation_revision
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::ReplayRevisionMismatch,
            &request.id.0,
        );
    }
    if input.replay.nonce != request.nonce || input.replay.reserved_intent_digest != intent_digest {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::ReplayIntentMismatch,
            &request.id.0,
        );
    }
    let max_age = i64::try_from(input.max_attestation_age_seconds).unwrap_or(i64::MAX);
    let max_future = i64::try_from(input.max_future_skew_seconds).unwrap_or(i64::MAX);
    if request.issued_at_unix > input.now_unix.saturating_add(max_future) {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::InvocationFromFuture,
            &request.id.0,
        );
    } else if input.now_unix.saturating_sub(request.issued_at_unix) > max_age {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::InvocationExpired,
            &request.id.0,
        );
    }
}

fn evaluate_claims(
    input: &ExecutionAdmissionInput,
    issues: &mut Vec<ExecutionAdmissionIssue>,
) -> Vec<RevisionExpectation> {
    let request = &input.request;
    let snapshot = &input.claim_snapshot;
    if snapshot.completeness != SnapshotCompleteness::Complete {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::ClaimSnapshotIncomplete,
            "claim_snapshot",
        );
    }
    if snapshot.revision != request.expected_claim_snapshot_revision {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::ClaimSnapshotRevisionMismatch,
            "claim_snapshot",
        );
    }
    if has_duplicate_revision_expectations(&request.expected_claim_revisions)
        || has_duplicate_claim_observations(&snapshot.claims)
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::DuplicateBinding,
            "claim_revisions",
        );
    }
    let mut validated = Vec::new();
    for expectation in &request.expected_claim_revisions {
        let Some(observation) = snapshot
            .claims
            .iter()
            .find(|claim| claim.claim_ref.0 == expectation.reference)
        else {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimMissing,
                &expectation.reference,
            );
            continue;
        };
        if observation.revision != expectation.revision {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimRevisionMismatch,
                &expectation.reference,
            );
            continue;
        }
        validated.push(expectation.clone());
    }

    let write_authority = &input
        .operation
        .operation_contract
        .coordination_scope
        .write_authority;
    if write_authority.requires_driver_claim || write_authority.requires_lane_claim {
        let Some(required_ref) = write_authority.claim_contract_ref.as_ref() else {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimMissing,
                "operation.claim_contract_ref",
            );
            return validated;
        };
        let Some(required) = snapshot
            .claims
            .iter()
            .find(|claim| claim.claim_ref == *required_ref)
        else {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimMissing,
                &required_ref.0,
            );
            return validated;
        };
        let claim = &required.document.claim_contract;
        if !is_live(claim, input.now_unix) {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimNotLive,
                &required_ref.0,
            );
        }
        if claim.claim.claimant_agent_id != input.principal.agent_id {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimPrincipalMismatch,
                &required_ref.0,
            );
        }
        if !caller_role_matches_actor(input.principal.role, claim.claim.claimant_role) {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimRoleMismatch,
                &required_ref.0,
            );
        }
        if claim.lease.expected_state_version != input.current_state_version {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimStateVersionMismatch,
                &required_ref.0,
            );
        }
        if !request.expected_claim_revisions.iter().any(|expectation| {
            expectation.reference == required_ref.0 && expectation.revision == required.revision
        }) {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimRevisionMismatch,
                &required_ref.0,
            );
        }
    }

    let file_targets: Vec<RepoPath> = input
        .effect_contracts
        .iter()
        .flat_map(|binding| &binding.document.tool_effect_contract.write_set)
        .filter(|write| file_backed_effect_target(write.target_kind))
        .map(|write| RepoPath(write.reference.clone()))
        .collect();
    if !file_targets.is_empty() {
        let claims = snapshot
            .claims
            .iter()
            .map(|claim| claim.document.claim_contract.clone())
            .collect::<Vec<_>>();
        match check_write_against_claims(
            &file_targets,
            &input.principal.agent_id,
            &claims,
            input.now_unix,
        ) {
            WriteCheck::Blocked { .. } => push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimConflict,
                "effect_write_set",
            ),
            WriteCheck::Ok { ungoverned, .. } if !ungoverned.is_empty() => push_issue(
                issues,
                ExecutionAdmissionIssueCode::ClaimCoverageMissing,
                "effect_write_set",
            ),
            WriteCheck::Ok { .. } => {}
        }
    }
    validated.sort_by(|left, right| left.reference.cmp(&right.reference));
    validated
}

fn evaluate_gates(
    input: &ExecutionAdmissionInput,
    issues: &mut Vec<ExecutionAdmissionIssue>,
) -> Vec<RevisionExpectation> {
    let request = &input.request;
    let snapshot = &input.gate_snapshot;
    if snapshot.completeness != SnapshotCompleteness::Complete {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::GateSnapshotIncomplete,
            "gate_snapshot",
        );
    }
    if snapshot.revision != request.expected_gate_snapshot_revision {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::GateSnapshotRevisionMismatch,
            "gate_snapshot",
        );
    }
    if has_duplicate_revision_expectations(&request.expected_gate_revisions)
        || has_duplicate_gate_observations(&snapshot.gates)
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::DuplicateBinding,
            "gate_revisions",
        );
    }
    let mut validated = Vec::new();
    for required in &input
        .operation
        .operation_contract
        .gates
        .required_before_mutation
    {
        let reference = &required.gate_contract_ref.0;
        let Some(expectation) = request
            .expected_gate_revisions
            .iter()
            .find(|expectation| expectation.reference == *reference)
        else {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::GateRevisionMismatch,
                reference,
            );
            continue;
        };
        let Some(observation) = snapshot
            .gates
            .iter()
            .find(|gate| gate.gate_ref.0 == *reference)
        else {
            push_issue(issues, ExecutionAdmissionIssueCode::GateMissing, reference);
            continue;
        };
        if observation.revision != expectation.revision {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::GateRevisionMismatch,
                reference,
            );
            continue;
        }
        let mut gate_valid = true;
        if observation.document.gate_contract.gate.status != GateStatus::Pass {
            gate_valid = false;
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::GateNotPassed,
                reference,
            );
        }
        if observation.document.gate_contract.evidence_refs.is_empty()
            || observation
                .document
                .gate_contract
                .gate
                .checked_by
                .0
                .trim()
                .is_empty()
        {
            gate_valid = false;
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::GateEvidenceMissing,
                reference,
            );
        }
        if observation.observed_state_version != input.current_state_version {
            gate_valid = false;
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::GateStateVersionMismatch,
                reference,
            );
        }
        if gate_valid {
            validated.push(expectation.clone());
        }
    }
    validated.sort_by(|left, right| left.reference.cmp(&right.reference));
    validated
}

fn evaluate_commit(input: &ExecutionAdmissionInput, issues: &mut Vec<ExecutionAdmissionIssue>) {
    let commit = &input.commit;
    let effects = &input.effect_contracts;
    match commit.strategy {
        ExecutionCommitStrategy::SingleEffectWal => {
            if effects.len() != 1 || commit.scope != ExecutionCommitScope::SingleEffect {
                push_issue(
                    issues,
                    ExecutionAdmissionIssueCode::CommitScopeInsufficient,
                    "single_effect_wal",
                );
            }
            require_wal_guarantees(commit, issues);
        }
        ExecutionCommitStrategy::OperationWideWal => {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::CommitStrategyUnsupported,
                "operation_wide_wal",
            );
            if commit.scope != ExecutionCommitScope::WholeOperation {
                push_issue(
                    issues,
                    ExecutionAdmissionIssueCode::CommitScopeInsufficient,
                    "operation_wide_wal",
                );
            }
            require_wal_guarantees(commit, issues);
        }
        ExecutionCommitStrategy::Saga => {
            push_issue(
                issues,
                ExecutionAdmissionIssueCode::CommitStrategyUnsupported,
                "saga",
            );
            if commit.scope != ExecutionCommitScope::WholeOperation {
                push_issue(
                    issues,
                    ExecutionAdmissionIssueCode::CommitScopeInsufficient,
                    "saga",
                );
            }
            if commit.compensation != CompensationCoverage::Complete
                || effects.iter().any(|binding| {
                    let repair = &binding.document.tool_effect_contract.repair;
                    repair.inverse.kind == InverseKind::None || !repair.stop_if_inverse_missing
                })
            {
                push_issue(
                    issues,
                    ExecutionAdmissionIssueCode::SagaCompensationIncomplete,
                    "saga",
                );
            }
        }
    }
}

fn require_wal_guarantees(
    commit: &CommitAssuranceObservation,
    issues: &mut Vec<ExecutionAdmissionIssue>,
) {
    if commit.wal_lock != GuaranteeStatus::Verified
        || commit.rollback_recovery != GuaranteeStatus::Verified
        || commit.durable_commit_record != GuaranteeStatus::Verified
    {
        push_issue(
            issues,
            ExecutionAdmissionIssueCode::CommitGuaranteeMissing,
            "wal",
        );
    }
}

fn push_issue(
    issues: &mut Vec<ExecutionAdmissionIssue>,
    code: ExecutionAdmissionIssueCode,
    subject: &str,
) {
    issues.push(ExecutionAdmissionIssue {
        code,
        subject: subject.to_owned(),
    });
}

fn stable_ids(values: &[StableId]) -> BTreeSet<String> {
    values.iter().map(|value| value.0.clone()).collect()
}

fn binding_refs(values: &[ContentAddressedBinding]) -> BTreeSet<String> {
    values.iter().map(|value| value.reference.clone()).collect()
}

fn has_duplicate_bindings(values: &[ContentAddressedBinding]) -> bool {
    binding_refs(values).len() != values.len()
}

fn has_duplicate_stable_ids(values: &[StableId]) -> bool {
    stable_ids(values).len() != values.len()
}

fn repo_paths(values: &[RepoPath]) -> BTreeSet<String> {
    values.iter().map(|value| value.0.clone()).collect()
}

fn has_duplicate_repo_paths(values: &[RepoPath]) -> bool {
    repo_paths(values).len() != values.len()
}

fn has_duplicate_revision_expectations(values: &[RevisionExpectation]) -> bool {
    values
        .iter()
        .map(|value| value.reference.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != values.len()
}

fn has_duplicate_claim_observations(values: &[ClaimRevisionObservation]) -> bool {
    values
        .iter()
        .map(|value| value.claim_ref.0.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != values.len()
}

fn has_duplicate_gate_observations(values: &[GateRevisionObservation]) -> bool {
    values
        .iter()
        .map(|value| value.gate_ref.0.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != values.len()
}

const fn caller_role_matches_actor(caller: CallerRole, actor: ActorRole) -> bool {
    matches!(
        (caller, actor),
        (CallerRole::Driver, ActorRole::Driver)
            | (CallerRole::Worker, ActorRole::Worker)
            | (CallerRole::Runtime, ActorRole::Runtime)
            | (CallerRole::Unknown, ActorRole::Unknown)
    )
}

const fn file_backed_effect_target(target_kind: EffectTargetKind) -> bool {
    // Keep this authority boundary aligned with the kernel effect-store set.
    // The decisions crate intentionally cannot depend on the mutation kernel.
    matches!(
        target_kind,
        EffectTargetKind::FilePath
            | EffectTargetKind::ArtifactId
            | EffectTargetKind::EvidenceId
            | EffectTargetKind::LedgerStream
            | EffectTargetKind::RequestStream
    )
}
