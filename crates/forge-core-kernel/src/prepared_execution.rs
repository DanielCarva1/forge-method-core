//! P4b.2b/P4b.2c prepared Admission, provenance commit, and recovery boundary.
//!
//! This internal path deliberately stops before the first effect-WAL record.
//! It consumes verified authority, derives every runtime authority knob,
//! reserves replay, retains effect -> replay locks, preflights the exact effect,
//! then rebuilds and evaluates Execution Admission from a fresh trusted
//! assurance/claim/gate snapshot. P4b.2c then revalidates mutable authority at
//! the immediate commit call, persists redacted complete provenance, commits
//! one effect, consumes replay, and reconciles the cross-WAL crash window.
//! Public MCP mutation remains disabled pending explicit deployment policy.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use forge_core_authority::{
    VerifiedExecutionAuthorization, VerifiedExecutionAuthorizationAudit, VerifiedExecutionCall,
};
use forge_core_contracts::{
    AssuranceCaseDocument, CommandContractDocument, OperationContractDocument, RepoPath, StableId,
    ToolEffectContractDocument,
};
use forge_core_decisions::{
    command_contract_token, effect_contract_token, evaluate_execution_admission,
    execution_intent_digest, operation_contract_token, ClaimSnapshotObservation,
    CommitAssuranceObservation, CompensationCoverage, ContentAddressedBinding,
    EffectContractBinding, ExecutionAdmissionDecision, ExecutionAdmissionInput,
    ExecutionAdmissionInputDocument, ExecutionAdmissionRejection, ExecutionAdmissionRequest,
    ExecutionAdmissionStatus, ExecutionCommitScope, ExecutionCommitStrategy,
    ExecutionPrincipalObservation, ExecutionPrincipalTrust, GateSnapshotObservation,
    GuaranteeStatus, ReplayProtectionObservation, ReplayReservationStatus,
    EXECUTION_ADMISSION_SCHEMA_VERSION,
};
use forge_core_store::replay_wal::{
    acquire_owned_replay_commit_guard, consume_replay_key_hash_under_effect_lock,
    reserve_replay_nonce, OwnedReplayCommitGuard, ReplayConsumeResult, ReplayReservation,
    ReplayWalError,
};
use forge_core_store::{
    acquire_effect_store_lock, append_effect_replay_completion_under_lock,
    apply_file_effect_transaction_with_provenance_under_lock,
    pending_effect_replay_commits_under_lock, preflight_file_effect_transaction_under_lock,
    recover_effect_wal, repair_effect_wal_tail_under_lock, sha256_content_hash,
    EffectApplicationPayload, EffectApplicationResult, EffectApplicationStatus,
    EffectExecutionProvenance, EffectExecutionProvenanceError, EffectPreflightResult,
    EffectPreflightStatus, EffectReplayCommitBinding, EffectReplayCompletionResult,
    EffectReplayReconciliationError, EffectStoreLockError, EffectWalRecoveryStatus,
};
use serde::{Deserialize, Serialize};

use crate::{RuntimeEffectPayloadKind, RuntimeOperationEffectPayload};

pub const PREPARED_EXECUTION_SCHEMA_VERSION: &str = "0.1";
pub const EXECUTION_COMMIT_PROVENANCE_SCHEMA_VERSION: &str = "0.1";
pub const PREPARED_EFFECT_LOCK_RELATIVE_PATH: &str = ".forge-method/locks/effects.lock";
pub const PREPARED_EFFECT_LOCK_FROM_STATE_ROOT: &str = "locks/effects.lock";
pub const PREPARED_EFFECT_WAL_RELATIVE_PATH: &str = ".forge-method/wal/effects.ndjson";
const FORGE_STATE_DIR: &str = ".forge-method";

/// Trusted host configuration. No adapter request field can override these
/// paths or the synchronous durability policy recorded by the descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedExecutionEnvironment {
    project_root: PathBuf,
    state_root: PathBuf,
    required_audience: String,
}

impl TrustedExecutionEnvironment {
    /// Canonicalize an existing project root and require its Forge state root.
    ///
    /// # Errors
    ///
    /// Returns [`PrepareExecutionError::ProjectRootUnavailable`] or
    /// [`PrepareExecutionError::StateRootUnavailable`] when the trusted host
    /// has not provisioned an existing canonical boundary.
    pub fn from_project_root(
        project_root: impl AsRef<Path>,
        required_audience: impl Into<String>,
    ) -> Result<Self, PrepareExecutionError> {
        let requested = project_root.as_ref();
        let project_root = requested.canonicalize().map_err(|error| {
            PrepareExecutionError::ProjectRootUnavailable {
                path: requested.to_path_buf(),
                source: error.to_string(),
            }
        })?;
        if !project_root.is_dir() {
            return Err(PrepareExecutionError::ProjectRootUnavailable {
                path: project_root,
                source: "project root is not a directory".to_owned(),
            });
        }
        let required_audience = required_audience.into();
        if required_audience.trim().is_empty() {
            return Err(PrepareExecutionError::InvalidRequiredAudience);
        }
        let state_root_path = project_root.join(FORGE_STATE_DIR);
        let state_root = state_root_path.canonicalize().map_err(|error| {
            PrepareExecutionError::StateRootUnavailable {
                path: state_root_path,
                source: error.to_string(),
            }
        })?;
        if !state_root.is_dir() {
            return Err(PrepareExecutionError::StateRootUnavailable {
                path: state_root,
                source: "Forge state root is not a directory".to_owned(),
            });
        }
        if !state_root.starts_with(&project_root) {
            return Err(PrepareExecutionError::StateRootUnavailable {
                path: state_root,
                source: "canonical Forge state root escapes the project root".to_owned(),
            });
        }
        Ok(Self {
            project_root,
            state_root,
            required_audience,
        })
    }

    #[must_use]
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    #[must_use]
    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    #[must_use]
    pub fn required_audience(&self) -> &str {
        &self.required_audience
    }
}

/// Exact typed material supplied by a trusted host after loading the refs in
/// [`forge_core_authority::ExecutionRequest`]. Fields stay private so the
/// prepared kernel path can validate the whole bundle before reserving replay.
pub struct PreparedExecutionMaterial {
    call: VerifiedExecutionCall,
    admission_request: ExecutionAdmissionRequest,
    operation: OperationContractDocument,
    commands: Vec<CommandContractDocument>,
    effect: ToolEffectContractDocument,
    payloads: Vec<RuntimeOperationEffectPayload>,
}

impl fmt::Debug for PreparedExecutionMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedExecutionMaterial")
            .field("call", &self.call)
            .field("admission_request_id", &self.admission_request.id)
            .field(
                "operation_id",
                &self.operation.operation_contract.contract_id,
            )
            .field("command_count", &self.commands.len())
            .field("effect_id", &self.effect.tool_effect_contract.id)
            .field("payload_count", &self.payloads.len())
            .finish()
    }
}

impl PreparedExecutionMaterial {
    #[must_use]
    pub fn new(
        call: VerifiedExecutionCall,
        admission_request: ExecutionAdmissionRequest,
        operation: OperationContractDocument,
        commands: Vec<CommandContractDocument>,
        effect: ToolEffectContractDocument,
        payloads: Vec<RuntimeOperationEffectPayload>,
    ) -> Self {
        Self {
            call,
            admission_request,
            operation,
            commands,
            effect,
            payloads,
        }
    }
}

/// Mutable authority facts captured by a trusted source while the prepared
/// transaction retains effect and replay locks. Principal, replay, contracts,
/// commit guarantees, freshness limits, and intent digest are intentionally
/// absent: the kernel derives them itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateExecutionSnapshot {
    pub assurance_case: AssuranceCaseDocument,
    pub claim_snapshot: ClaimSnapshotObservation,
    pub gate_snapshot: GateSnapshotObservation,
    pub current_state_version: u64,
    pub now_unix: i64,
}

pub trait LateExecutionSnapshotSource: fmt::Debug + Send + Sync {
    /// Capture one complete commit-time snapshot.
    ///
    /// Implementations are part of the trusted kernel host and must perform a
    /// bounded local read only: no model call, network request, subprocess, or
    /// mutation is allowed while authority locks are retained.
    ///
    /// # Errors
    ///
    /// Returns [`LateSnapshotError`] when the trusted source cannot produce a
    /// complete observation. The kernel fails closed without effect-WAL writes.
    fn capture(&self) -> Result<LateExecutionSnapshot, LateSnapshotError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateSnapshotError {
    message: String,
}

impl LateSnapshotError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for LateSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LateSnapshotError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PreparedCommitDescriptor {
    pub schema_version: String,
    pub project_root: String,
    pub audience: String,
    pub operation_ref: String,
    pub operation_id: StableId,
    pub operation_token: String,
    pub commands: Vec<PreparedCommandBinding>,
    pub effect: PreparedEffectBinding,
    pub payloads: Vec<PreparedPayloadBinding>,
    pub effect_lock_relative_path: String,
    pub effect_wal_relative_path: String,
    pub tx_id: String,
    pub durability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PreparedCommandBinding {
    pub source_ref: String,
    pub command_id: StableId,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PreparedEffectBinding {
    pub source_ref: String,
    pub effect_id: StableId,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PreparedPayloadBinding {
    pub target_ref: String,
    pub source_ref: String,
    pub content_hash: String,
    pub byte_len: u64,
    pub payload_kind: RuntimeEffectPayloadKind,
}

/// Complete, content-addressed authority and Admission evidence written into
/// the effect WAL before the first project mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ExecutionCommitProvenanceDocument {
    pub schema_version: String,
    pub tx_id: String,
    pub project_root: String,
    pub audience: String,
    pub authorization: VerifiedExecutionAuthorizationAudit,
    pub late_admission_input: serde_json::Value,
    pub late_admission_decision: ExecutionAdmissionDecision,
    pub commit_admission_input: serde_json::Value,
    pub commit_admission_decision: ExecutionAdmissionDecision,
    pub initial_preflight: EffectPreflightResult,
    pub late_preflight: EffectPreflightResult,
    pub commit_preflight: EffectPreflightResult,
    pub commit_descriptor: PreparedCommitDescriptor,
    pub commit_digest: String,
    pub replay_reservation: ReplayReservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExecutionCommitStatus {
    Committed,
    EffectCommittedReplayPending,
    EffectCommittedCompletionPending,
}

/// A durable effect commit receipt. Pending statuses are not safe to retry as
/// a new execution; they must be passed through P4b.2c reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ExecutionCommitReceipt {
    pub status: ExecutionCommitStatus,
    pub tx_id: String,
    pub effect_id: StableId,
    pub commit_digest: String,
    pub provenance_digest: String,
    pub application: EffectApplicationResult,
    pub replay: Option<ReplayConsumeResult>,
    pub completion: Option<EffectReplayCompletionResult>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExecutionCommitOutcome {
    Committed {
        receipt: Box<ExecutionCommitReceipt>,
    },
    NotCommitted {
        application: Box<EffectApplicationResult>,
        provenance_digest: String,
    },
    Blocked {
        decision: Box<ExecutionAdmissionDecision>,
        commit_preflight: Box<EffectPreflightResult>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExecutionReplayReconciliationStatus {
    Noop,
    Reconciled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ExecutionReplayReconciliationResult {
    pub status: ExecutionReplayReconciliationStatus,
    pub recovered_effect_transactions: Vec<String>,
    pub reconciled_transactions: Vec<String>,
    pub replay_results: Vec<ReplayConsumeResult>,
    pub completion_records: Vec<EffectReplayCompletionResult>,
}

/// Linear transaction retaining verified identity, exact material, initial
/// filesystem preflight, the effect lock, and the replay lock/reservation.
/// It has no serde or clone implementation. Only the admitted typestate can
/// enter the P4b.2c effect-commit boundary.
///
/// ```compile_fail
/// use forge_core_kernel::PreparedExecutionTransaction;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<PreparedExecutionTransaction>();
/// ```
///
/// ```compile_fail
/// use forge_core_kernel::PreparedExecutionTransaction;
/// let _: PreparedExecutionTransaction = serde_json::from_str("{}").unwrap();
/// ```
pub struct PreparedExecutionTransaction {
    environment: TrustedExecutionEnvironment,
    authorization: VerifiedExecutionAuthorization,
    admission_request: ExecutionAdmissionRequest,
    _operation_ref: String,
    operation: OperationContractDocument,
    _command_refs: Vec<String>,
    commands: Vec<CommandContractDocument>,
    effect_ref: RepoPath,
    effect: ToolEffectContractDocument,
    _payloads: Vec<RuntimeOperationEffectPayload>,
    store_payloads: Vec<EffectApplicationPayload>,
    commit_descriptor: PreparedCommitDescriptor,
    commit_digest: String,
    initial_preflight: EffectPreflightResult,
    replay_guard: OwnedReplayCommitGuard,
}

impl fmt::Debug for PreparedExecutionTransaction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedExecutionTransaction")
            .field("authorization", &self.authorization)
            .field("admission_request", &self.admission_request.id)
            .field("commit_digest", &self.commit_digest)
            .field("initial_preflight", &self.initial_preflight)
            .field("replay_guard", &self.replay_guard)
            .finish_non_exhaustive()
    }
}

impl PreparedExecutionTransaction {
    #[must_use]
    pub fn commit_descriptor(&self) -> &PreparedCommitDescriptor {
        &self.commit_descriptor
    }

    #[must_use]
    pub fn commit_digest(&self) -> &str {
        &self.commit_digest
    }

    #[must_use]
    pub fn replay_reservation(&self) -> &forge_core_store::replay_wal::ReplayReservation {
        self.replay_guard.reservation()
    }

    #[must_use]
    pub fn initial_preflight(&self) -> &EffectPreflightResult {
        &self.initial_preflight
    }

    /// Revalidate the exact effect under retained locks, capture a fresh
    /// mutable authority snapshot, and evaluate P4a immediately at the future
    /// pre-WAL boundary.
    ///
    /// # Errors
    ///
    /// Returns [`LateAdmissionError`] for snapshot failure, filesystem drift,
    /// or an evaluator-level schema/canonicalization failure. A policy block is
    /// returned as [`LateAdmissionOutcome::Blocked`], not an operational error.
    pub fn evaluate_late(
        self,
        source: &dyn LateExecutionSnapshotSource,
    ) -> Result<LateAdmissionOutcome, LateAdmissionError> {
        let final_preflight = preflight_file_effect_transaction_under_lock(
            self.environment.project_root(),
            self.replay_guard.effect_lock(),
            PREPARED_EFFECT_LOCK_RELATIVE_PATH,
            &self.effect,
            &self.store_payloads,
        );
        if final_preflight.status != EffectPreflightStatus::Ready
            || final_preflight != self.initial_preflight
        {
            return Err(LateAdmissionError::EffectPreflightChanged {
                initial: Box::new(self.initial_preflight.clone()),
                current: Box::new(final_preflight),
            });
        }

        let snapshot = source
            .capture()
            .map_err(LateAdmissionError::SnapshotCapture)?;
        let admission_document = self.admission_document(snapshot);
        let decision = evaluate_execution_admission(&admission_document)
            .map_err(LateAdmissionError::Evaluation)?;
        if decision.status == ExecutionAdmissionStatus::Admitted {
            Ok(LateAdmissionOutcome::Admitted(Box::new(
                LateAdmittedExecutionTransaction {
                    prepared: self,
                    admission_document,
                    decision,
                    final_preflight,
                },
            )))
        } else {
            Ok(LateAdmissionOutcome::Blocked {
                decision: Box::new(decision),
                final_preflight: Box::new(final_preflight),
            })
        }
    }

    fn admission_document(
        &self,
        snapshot: LateExecutionSnapshot,
    ) -> ExecutionAdmissionInputDocument {
        let principal = self.authorization.principal();
        let reservation = self.replay_guard.reservation();
        ExecutionAdmissionInputDocument {
            schema_version: EXECUTION_ADMISSION_SCHEMA_VERSION.to_owned(),
            execution_admission: ExecutionAdmissionInput {
                request: self.admission_request.clone(),
                assurance_case: snapshot.assurance_case,
                operation: self.operation.clone(),
                command_contracts: self.commands.clone(),
                effect_contracts: vec![EffectContractBinding {
                    effect_ref: self.effect_ref.clone(),
                    document: self.effect.clone(),
                }],
                principal: ExecutionPrincipalObservation {
                    principal_id: principal.principal_id().clone(),
                    agent_id: principal.agent_id().clone(),
                    role: principal.role(),
                    trust: ExecutionPrincipalTrust::AuthorizedKeyRegistry,
                    credential_id: principal.credential_id().to_owned(),
                    audience: principal.audience().to_owned(),
                    required_audience: principal.audience().to_owned(),
                    authority_grants: principal.authority_grants().to_vec(),
                    attested_intent_digest: self.authorization.execution_intent_digest().to_owned(),
                },
                replay: ReplayProtectionObservation {
                    status: ReplayReservationStatus::FreshReserved,
                    nonce: self.authorization.nonce().to_owned(),
                    reserved_intent_digest: reservation.intent_digest.clone(),
                    reservation_revision: reservation.revision,
                },
                claim_snapshot: snapshot.claim_snapshot,
                gate_snapshot: snapshot.gate_snapshot,
                commit: CommitAssuranceObservation {
                    strategy: ExecutionCommitStrategy::SingleEffectWal,
                    scope: ExecutionCommitScope::SingleEffect,
                    wal_lock: GuaranteeStatus::Verified,
                    rollback_recovery: GuaranteeStatus::Verified,
                    durable_commit_record: GuaranteeStatus::Verified,
                    compensation: CompensationCoverage::NotApplicable,
                },
                current_state_version: snapshot.current_state_version,
                now_unix: snapshot.now_unix,
                max_attestation_age_seconds: self.authorization.max_attestation_age_seconds(),
                max_future_skew_seconds: self.authorization.max_future_skew_seconds(),
            },
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum LateAdmissionOutcome {
    Admitted(Box<LateAdmittedExecutionTransaction>),
    Blocked {
        decision: Box<ExecutionAdmissionDecision>,
        final_preflight: Box<EffectPreflightResult>,
    },
}

/// Opaque typestate proving late Admission passed while all prepared authority
/// remains owned. Its P4b.2c commit method consumes this value directly, repeats
/// mutable-authority Admission, and cannot be invoked from a deserialized DTO.
///
/// ```compile_fail
/// use forge_core_kernel::LateAdmittedExecutionTransaction;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<LateAdmittedExecutionTransaction>();
/// ```
///
/// ```compile_fail
/// use forge_core_kernel::LateAdmittedExecutionTransaction;
/// let _: LateAdmittedExecutionTransaction = serde_json::from_str("{}").unwrap();
/// ```
pub struct LateAdmittedExecutionTransaction {
    prepared: PreparedExecutionTransaction,
    admission_document: ExecutionAdmissionInputDocument,
    decision: ExecutionAdmissionDecision,
    final_preflight: EffectPreflightResult,
}

impl fmt::Debug for LateAdmittedExecutionTransaction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LateAdmittedExecutionTransaction")
            .field("request_id", &self.decision.request_id)
            .field(
                "admission_schema_version",
                &self.admission_document.schema_version,
            )
            .field("intent_digest", &self.decision.intent_digest)
            .field("commit_digest", &self.prepared.commit_digest)
            .field("final_preflight", &self.final_preflight)
            .finish_non_exhaustive()
    }
}

impl LateAdmittedExecutionTransaction {
    #[must_use]
    pub fn decision(&self) -> &ExecutionAdmissionDecision {
        &self.decision
    }

    #[must_use]
    pub fn commit_descriptor(&self) -> &PreparedCommitDescriptor {
        &self.prepared.commit_descriptor
    }

    #[must_use]
    pub fn commit_digest(&self) -> &str {
        &self.prepared.commit_digest
    }

    #[must_use]
    pub fn final_preflight(&self) -> &EffectPreflightResult {
        &self.final_preflight
    }

    /// Revalidate every mutable observation at the immediate commit call,
    /// persist complete provenance, commit exactly one effect, consume replay,
    /// and acknowledge replay completion in the effect WAL.
    ///
    /// A receipt with a pending status means the effect commit is already
    /// durable and must be reconciled, never retried as a new execution.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionCommitError`] before the first effect-WAL record for
    /// snapshot, preflight, evaluation, or provenance-construction failure.
    pub fn commit(
        self,
        source: &dyn LateExecutionSnapshotSource,
    ) -> Result<ExecutionCommitOutcome, ExecutionCommitError> {
        let commit_preflight = preflight_file_effect_transaction_under_lock(
            self.prepared.environment.project_root(),
            self.prepared.replay_guard.effect_lock(),
            PREPARED_EFFECT_LOCK_RELATIVE_PATH,
            &self.prepared.effect,
            &self.prepared.store_payloads,
        );
        if commit_preflight.status != EffectPreflightStatus::Ready
            || commit_preflight != self.final_preflight
        {
            return Err(ExecutionCommitError::EffectPreflightChanged {
                admitted: Box::new(self.final_preflight),
                current: Box::new(commit_preflight),
            });
        }

        let snapshot = source
            .capture()
            .map_err(ExecutionCommitError::SnapshotCapture)?;
        let commit_admission_input = self.prepared.admission_document(snapshot);
        let commit_admission_decision = evaluate_execution_admission(&commit_admission_input)
            .map_err(ExecutionCommitError::Evaluation)?;
        if commit_admission_decision.status != ExecutionAdmissionStatus::Admitted {
            return Ok(ExecutionCommitOutcome::Blocked {
                decision: Box::new(commit_admission_decision),
                commit_preflight: Box::new(commit_preflight),
            });
        }

        let tx_id = self.prepared.commit_descriptor.tx_id.clone();
        let effect_id = self.prepared.effect.tool_effect_contract.id.clone();
        let commit_digest = self.prepared.commit_digest.clone();
        let replay_reservation = self.prepared.replay_guard.reservation().clone();
        let authorization = self.prepared.authorization.audit();
        let late_admission_input = redacted_admission_projection(
            &self.admission_document,
            &authorization.nonce_fingerprint,
        )?;
        let commit_admission_projection = redacted_admission_projection(
            &commit_admission_input,
            &authorization.nonce_fingerprint,
        )?;
        let provenance_document = ExecutionCommitProvenanceDocument {
            schema_version: EXECUTION_COMMIT_PROVENANCE_SCHEMA_VERSION.to_owned(),
            tx_id: tx_id.clone(),
            project_root: path_string(self.prepared.environment.project_root())
                .map_err(|error| ExecutionCommitError::Provenance(error.to_string()))?,
            audience: self.prepared.environment.required_audience().to_owned(),
            authorization,
            late_admission_input,
            late_admission_decision: self.decision,
            commit_admission_input: commit_admission_projection,
            commit_admission_decision,
            initial_preflight: self.prepared.initial_preflight.clone(),
            late_preflight: self.final_preflight,
            commit_preflight: commit_preflight.clone(),
            commit_descriptor: self.prepared.commit_descriptor.clone(),
            commit_digest: commit_digest.clone(),
            replay_reservation: replay_reservation.clone(),
        };
        let provenance_value = serde_json::to_value(&provenance_document)
            .map_err(|error| ExecutionCommitError::Provenance(error.to_string()))?;
        let provenance = EffectExecutionProvenance::new(provenance_value)
            .map_err(ExecutionCommitError::ProvenanceEnvelope)?;
        let provenance_digest = provenance.digest.clone();
        let replay_binding = EffectReplayCommitBinding::new(
            replay_reservation.key_hash,
            replay_reservation.intent_digest,
            replay_reservation.commit_digest,
            replay_reservation.revision,
        );
        let application = apply_file_effect_transaction_with_provenance_under_lock(
            self.prepared.environment.project_root(),
            self.prepared.replay_guard.effect_lock(),
            PREPARED_EFFECT_LOCK_RELATIVE_PATH,
            &self.prepared.effect,
            &self.prepared.store_payloads,
            PREPARED_EFFECT_WAL_RELATIVE_PATH,
            tx_id.clone(),
            provenance,
            replay_binding.clone(),
        );
        if application.status != EffectApplicationStatus::Applied {
            return Ok(ExecutionCommitOutcome::NotCommitted {
                application: Box::new(application),
                provenance_digest,
            });
        }

        let replay_guard = self.prepared.replay_guard;
        let consumed = match replay_guard.consume_retaining_effect_lock() {
            Ok(consumed) => consumed,
            Err(error) => {
                return Ok(ExecutionCommitOutcome::Committed {
                    receipt: Box::new(ExecutionCommitReceipt {
                        status: ExecutionCommitStatus::EffectCommittedReplayPending,
                        tx_id,
                        effect_id,
                        commit_digest,
                        provenance_digest,
                        application,
                        replay: None,
                        completion: None,
                        diagnostics: vec![format!(
                            "effect committed but replay consume failed: {error}"
                        )],
                    }),
                });
            }
        };
        let replay = consumed.result().clone();
        let completion = append_effect_replay_completion_under_lock(
            self.prepared.environment.project_root(),
            consumed.effect_lock(),
            PREPARED_EFFECT_LOCK_RELATIVE_PATH,
            PREPARED_EFFECT_WAL_RELATIVE_PATH,
            &tx_id,
            &effect_id,
            &replay_binding,
            &replay,
            false,
        );
        match completion {
            Ok(completion) => Ok(ExecutionCommitOutcome::Committed {
                receipt: Box::new(ExecutionCommitReceipt {
                    status: ExecutionCommitStatus::Committed,
                    tx_id,
                    effect_id,
                    commit_digest,
                    provenance_digest,
                    application,
                    replay: Some(replay),
                    completion: Some(completion),
                    diagnostics: Vec::new(),
                }),
            }),
            Err(error) => Ok(ExecutionCommitOutcome::Committed {
                receipt: Box::new(ExecutionCommitReceipt {
                    status: ExecutionCommitStatus::EffectCommittedCompletionPending,
                    tx_id,
                    effect_id,
                    commit_digest,
                    provenance_digest,
                    application,
                    replay: Some(replay),
                    completion: None,
                    diagnostics: vec![format!(
                        "effect and replay committed but completion marker failed: {error}"
                    )],
                }),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrepareExecutionError {
    ProjectRootUnavailable { path: PathBuf, source: String },
    StateRootUnavailable { path: PathBuf, source: String },
    NonUtf8Path(PathBuf),
    InvalidRequiredAudience,
    AudienceMismatch,
    IntentDigest(String),
    ContractBinding(String),
    AuthorityIntentMismatch,
    PrincipalMismatch,
    NonceMismatch,
    IssuedAtMismatch,
    ReplayRevisionMustStartAtOne(u64),
    OperationBindingMismatch,
    CommandBindingMismatch,
    EffectBindingMismatch,
    SingleEffectRequired,
    PayloadBindingMismatch,
    AdapterRequirementNotIntegrated(&'static str),
    CommitDescriptor(String),
    EffectLock(String),
    EffectPreflightBlocked(Box<EffectPreflightResult>),
    Replay(String),
}

impl fmt::Display for PrepareExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRootUnavailable { path, source } => {
                write!(
                    formatter,
                    "project root {} unavailable: {source}",
                    path.display()
                )
            }
            Self::StateRootUnavailable { path, source } => {
                write!(
                    formatter,
                    "state root {} unavailable: {source}",
                    path.display()
                )
            }
            Self::NonUtf8Path(path) => write!(
                formatter,
                "authority-bearing path is not valid UTF-8: {}",
                path.display()
            ),
            Self::InvalidRequiredAudience => {
                formatter.write_str("trusted environment audience must not be blank")
            }
            Self::AudienceMismatch => formatter
                .write_str("verified principal audience does not match trusted environment"),
            Self::IntentDigest(source) => write!(formatter, "intent digest failed: {source}"),
            Self::ContractBinding(source) => {
                write!(formatter, "contract binding failed: {source}")
            }
            Self::AuthorityIntentMismatch => formatter.write_str(
                "verified authorization digest does not match the typed admission request",
            ),
            Self::PrincipalMismatch => {
                formatter.write_str("admission request principal does not match verified authority")
            }
            Self::NonceMismatch => {
                formatter.write_str("admission request nonce does not match verified authority")
            }
            Self::IssuedAtMismatch => {
                formatter.write_str("admission request issued-at does not match verified authority")
            }
            Self::ReplayRevisionMustStartAtOne(found) => write!(
                formatter,
                "kernel-owned replay reservation must start at revision 1, found {found}"
            ),
            Self::OperationBindingMismatch => {
                formatter.write_str("operation binding does not match exact typed material")
            }
            Self::CommandBindingMismatch => {
                formatter.write_str("command bindings do not match exact typed material")
            }
            Self::EffectBindingMismatch => {
                formatter.write_str("effect binding does not match exact typed material")
            }
            Self::SingleEffectRequired => {
                formatter.write_str("prepared execution requires exactly one effect")
            }
            Self::PayloadBindingMismatch => {
                formatter.write_str("payload bindings do not match exact effect material")
            }
            Self::AdapterRequirementNotIntegrated(requirement) => write!(
                formatter,
                "adapter requirement '{requirement}' has no trusted P4b.2c kernel projection"
            ),
            Self::CommitDescriptor(source) => {
                write!(formatter, "commit descriptor failed: {source}")
            }
            Self::EffectLock(source) => write!(formatter, "effect lock failed: {source}"),
            Self::EffectPreflightBlocked(result) => {
                write!(formatter, "effect preflight blocked: {:?}", result.reasons)
            }
            Self::Replay(source) => write!(formatter, "replay authority failed: {source}"),
        }
    }
}

impl std::error::Error for PrepareExecutionError {}

impl From<EffectStoreLockError> for PrepareExecutionError {
    fn from(error: EffectStoreLockError) -> Self {
        Self::EffectLock(error.to_string())
    }
}

impl From<ReplayWalError> for PrepareExecutionError {
    fn from(error: ReplayWalError) -> Self {
        Self::Replay(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LateAdmissionError {
    SnapshotCapture(LateSnapshotError),
    EffectPreflightChanged {
        initial: Box<EffectPreflightResult>,
        current: Box<EffectPreflightResult>,
    },
    Evaluation(ExecutionAdmissionRejection),
}

impl fmt::Display for LateAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SnapshotCapture(error) => write!(formatter, "late snapshot failed: {error}"),
            Self::EffectPreflightChanged { .. } => formatter.write_str(
                "effect preflight changed while the prepared transaction retained locks",
            ),
            Self::Evaluation(error) => write!(formatter, "late admission failed: {error}"),
        }
    }
}

impl std::error::Error for LateAdmissionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExecutionCommitError {
    SnapshotCapture(LateSnapshotError),
    EffectPreflightChanged {
        admitted: Box<EffectPreflightResult>,
        current: Box<EffectPreflightResult>,
    },
    Evaluation(ExecutionAdmissionRejection),
    Provenance(String),
    ProvenanceEnvelope(EffectExecutionProvenanceError),
}

impl fmt::Display for ExecutionCommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SnapshotCapture(error) => write!(formatter, "commit snapshot failed: {error}"),
            Self::EffectPreflightChanged { .. } => formatter
                .write_str("effect preflight changed after late Admission and before commit"),
            Self::Evaluation(error) => write!(formatter, "commit admission failed: {error}"),
            Self::Provenance(error) => {
                write!(formatter, "commit provenance construction failed: {error}")
            }
            Self::ProvenanceEnvelope(error) => {
                write!(formatter, "commit provenance failed: {error}")
            }
        }
    }
}

impl std::error::Error for ExecutionCommitError {}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExecutionReplayReconciliationError {
    EffectLock(String),
    EffectRecovery(Vec<String>),
    EffectWal(EffectReplayReconciliationError),
    Provenance {
        tx_id: String,
        reason: String,
    },
    Replay {
        tx_id: String,
        source: ReplayWalError,
    },
    Completion {
        tx_id: String,
        source: EffectReplayReconciliationError,
    },
}

impl fmt::Display for ExecutionReplayReconciliationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EffectLock(error) => write!(formatter, "effect recovery lock failed: {error}"),
            Self::EffectRecovery(diagnostics) => write!(
                formatter,
                "effect WAL recovery failed: {}",
                diagnostics.join("; ")
            ),
            Self::EffectWal(error) => write!(formatter, "effect WAL inspection failed: {error}"),
            Self::Provenance { tx_id, reason } => {
                write!(
                    formatter,
                    "transaction {tx_id} provenance rejected: {reason}"
                )
            }
            Self::Replay { tx_id, source } => {
                write!(
                    formatter,
                    "transaction {tx_id} replay reconciliation failed: {source}"
                )
            }
            Self::Completion { tx_id, source } => write!(
                formatter,
                "transaction {tx_id} replay completion marker failed: {source}"
            ),
        }
    }
}

impl std::error::Error for ExecutionReplayReconciliationError {}

/// Consume verified authority and prepare one replay-bound single-effect
/// transaction without writing the effect WAL.
///
/// # Errors
///
/// Fails closed for any authority/material mismatch, unsupported adapter
/// requirement, preflight issue, lock failure, or replay failure.
pub fn prepare_execution_transaction(
    material: PreparedExecutionMaterial,
    environment: TrustedExecutionEnvironment,
) -> Result<PreparedExecutionTransaction, PrepareExecutionError> {
    let PreparedExecutionMaterial {
        call,
        admission_request,
        operation,
        commands,
        effect,
        payloads,
    } = material;
    let (authorization, execution_request) = call.into_parts();

    if execution_request.risk_audit_rules_ref().is_some() {
        return Err(PrepareExecutionError::AdapterRequirementNotIntegrated(
            "risk_audit_rules_ref",
        ));
    }
    if execution_request.require_citation() {
        return Err(PrepareExecutionError::AdapterRequirementNotIntegrated(
            "require_citation",
        ));
    }

    let intent_digest = execution_intent_digest(&admission_request)
        .map_err(|error| PrepareExecutionError::IntentDigest(error.to_string()))?;
    if authorization.execution_intent_digest() != intent_digest {
        return Err(PrepareExecutionError::AuthorityIntentMismatch);
    }
    let principal = authorization.principal();
    if principal.audience() != environment.required_audience() {
        return Err(PrepareExecutionError::AudienceMismatch);
    }
    if admission_request.principal_id != *principal.principal_id()
        || admission_request.agent_id != *principal.agent_id()
        || admission_request.principal_role != principal.role()
    {
        return Err(PrepareExecutionError::PrincipalMismatch);
    }
    if admission_request.nonce != authorization.nonce() {
        return Err(PrepareExecutionError::NonceMismatch);
    }
    if admission_request.issued_at_unix != authorization.issued_at_unix() {
        return Err(PrepareExecutionError::IssuedAtMismatch);
    }
    if admission_request.expected_replay_reservation_revision != 1 {
        return Err(PrepareExecutionError::ReplayRevisionMustStartAtOne(
            admission_request.expected_replay_reservation_revision,
        ));
    }

    let operation_token = operation_contract_token(&operation)
        .map_err(|error| PrepareExecutionError::ContractBinding(error.to_string()))?;
    if admission_request.operation_id != operation.operation_contract.contract_id
        || admission_request.operation_token != operation_token
    {
        return Err(PrepareExecutionError::OperationBindingMismatch);
    }

    let command_refs = execution_request.command_contract_refs();
    let distinct_command_refs = command_refs
        .iter()
        .map(PathBuf::as_path)
        .collect::<BTreeSet<_>>();
    if command_refs.len() != commands.len()
        || distinct_command_refs.len() != command_refs.len()
        || !command_bindings_match(&admission_request.command_bindings, &commands)?
    {
        return Err(PrepareExecutionError::CommandBindingMismatch);
    }

    let Some(effect_path) = execution_request.effect_contract_ref() else {
        return Err(PrepareExecutionError::SingleEffectRequired);
    };
    if admission_request.effect_bindings.len() != 1
        || operation.operation_contract.effect_contract_refs.len() != 1
    {
        return Err(PrepareExecutionError::SingleEffectRequired);
    }
    let effect_ref = RepoPath(path_string(effect_path)?);
    let effect_token = effect_contract_token(&effect)
        .map_err(|error| PrepareExecutionError::ContractBinding(error.to_string()))?;
    let request_effect = &admission_request.effect_bindings[0];
    if request_effect.reference != effect_ref.0
        || request_effect.token != effect_token
        || operation.operation_contract.effect_contract_refs[0] != effect_ref
    {
        return Err(PrepareExecutionError::EffectBindingMismatch);
    }

    if !payload_bindings_match(&execution_request, &effect, &payloads) {
        return Err(PrepareExecutionError::PayloadBindingMismatch);
    }
    let store_payloads = store_effect_payloads(&payloads);
    let command_source_refs: Vec<String> = command_refs
        .iter()
        .map(|path| path_string(path))
        .collect::<Result<_, _>>()?;
    let operation_ref = path_string(execution_request.operation_contract_ref())?;
    let descriptor = build_commit_descriptor(
        &environment,
        &operation_ref,
        &operation,
        &operation_token,
        &command_source_refs,
        &commands,
        &effect_ref,
        &effect,
        &effect_token,
        execution_request.payloads(),
        &payloads,
        &intent_digest,
        &admission_request.id,
    )?;
    let canonical = serde_json_canonicalizer::to_vec(&descriptor)
        .map_err(|error| PrepareExecutionError::CommitDescriptor(error.to_string()))?;
    let commit_digest = sha256_content_hash(&canonical);

    let effect_lock = acquire_effect_store_lock(
        environment.project_root(),
        PREPARED_EFFECT_LOCK_RELATIVE_PATH,
    )?;
    let initial_preflight = preflight_file_effect_transaction_under_lock(
        environment.project_root(),
        &effect_lock,
        PREPARED_EFFECT_LOCK_RELATIVE_PATH,
        &effect,
        &store_payloads,
    );
    if initial_preflight.status != EffectPreflightStatus::Ready {
        return Err(PrepareExecutionError::EffectPreflightBlocked(Box::new(
            initial_preflight,
        )));
    }

    let reservation = reserve_replay_nonce(
        environment.state_root(),
        principal.principal_id(),
        principal.audience(),
        authorization.nonce(),
        &intent_digest,
        &commit_digest,
    )?;
    let replay_guard = acquire_owned_replay_commit_guard(
        environment.state_root(),
        effect_lock,
        PREPARED_EFFECT_LOCK_FROM_STATE_ROOT,
        principal.principal_id(),
        principal.audience(),
        authorization.nonce(),
        &intent_digest,
        &commit_digest,
        reservation.reservation.revision,
    )?;

    Ok(PreparedExecutionTransaction {
        environment,
        authorization,
        admission_request,
        _operation_ref: operation_ref,
        operation,
        _command_refs: command_source_refs,
        commands,
        effect_ref,
        effect,
        _payloads: payloads,
        store_payloads,
        commit_descriptor: descriptor,
        commit_digest,
        initial_preflight,
        replay_guard,
    })
}

/// Recover incomplete effects, then reconcile every provenance-bound effect
/// commit whose replay consume or effect-WAL completion marker was interrupted
/// by a crash. The exact effect lock is retained for the entire pass.
///
/// # Errors
///
/// Returns [`ExecutionReplayReconciliationError`] and stops fail-closed on any
/// corrupt provenance, WAL inconsistency, replay mismatch, or durable append
/// failure. Successfully reconciled earlier candidates remain idempotent.
pub fn reconcile_prepared_execution_commits(
    environment: &TrustedExecutionEnvironment,
) -> Result<ExecutionReplayReconciliationResult, ExecutionReplayReconciliationError> {
    let effect_lock = acquire_effect_store_lock(
        environment.project_root(),
        PREPARED_EFFECT_LOCK_RELATIVE_PATH,
    )
    .map_err(|error| ExecutionReplayReconciliationError::EffectLock(error.to_string()))?;
    repair_effect_wal_tail_under_lock(
        environment.project_root(),
        &effect_lock,
        PREPARED_EFFECT_LOCK_RELATIVE_PATH,
        PREPARED_EFFECT_WAL_RELATIVE_PATH,
    )
    .map_err(ExecutionReplayReconciliationError::EffectWal)?;
    let effect_recovery = recover_effect_wal(
        environment.project_root(),
        PREPARED_EFFECT_WAL_RELATIVE_PATH,
    );
    if effect_recovery.status == EffectWalRecoveryStatus::RecoveryFailed {
        return Err(ExecutionReplayReconciliationError::EffectRecovery(
            effect_recovery.diagnostics,
        ));
    }
    let recovered_effect_transactions = effect_recovery.recovered_transactions;
    let pending = pending_effect_replay_commits_under_lock(
        environment.project_root(),
        &effect_lock,
        PREPARED_EFFECT_LOCK_RELATIVE_PATH,
        PREPARED_EFFECT_WAL_RELATIVE_PATH,
    )
    .map_err(ExecutionReplayReconciliationError::EffectWal)?;

    let expected_project_root = path_string(environment.project_root()).map_err(|error| {
        ExecutionReplayReconciliationError::Provenance {
            tx_id: "<environment>".to_owned(),
            reason: error.to_string(),
        }
    })?;
    let mut reconciled_transactions = Vec::with_capacity(pending.len());
    let mut replay_results = Vec::with_capacity(pending.len());
    let mut completion_records = Vec::with_capacity(pending.len());
    for candidate in pending {
        validate_recovery_provenance(
            &candidate.tx_id,
            &candidate.provenance.document,
            &candidate.replay_binding,
            &expected_project_root,
            environment.required_audience(),
        )?;
        let replay = consume_replay_key_hash_under_effect_lock(
            environment.state_root(),
            &effect_lock,
            PREPARED_EFFECT_LOCK_FROM_STATE_ROOT,
            &candidate.replay_binding.key_hash,
            &candidate.replay_binding.intent_digest,
            &candidate.replay_binding.commit_digest,
            candidate.replay_binding.reservation_revision,
        )
        .map_err(|source| ExecutionReplayReconciliationError::Replay {
            tx_id: candidate.tx_id.clone(),
            source,
        })?;
        let completion = append_effect_replay_completion_under_lock(
            environment.project_root(),
            &effect_lock,
            PREPARED_EFFECT_LOCK_RELATIVE_PATH,
            PREPARED_EFFECT_WAL_RELATIVE_PATH,
            &candidate.tx_id,
            &candidate.effect_id,
            &candidate.replay_binding,
            &replay,
            true,
        )
        .map_err(|source| ExecutionReplayReconciliationError::Completion {
            tx_id: candidate.tx_id.clone(),
            source,
        })?;
        reconciled_transactions.push(candidate.tx_id);
        replay_results.push(replay);
        completion_records.push(completion);
    }

    let status = if recovered_effect_transactions.is_empty() && reconciled_transactions.is_empty() {
        ExecutionReplayReconciliationStatus::Noop
    } else {
        ExecutionReplayReconciliationStatus::Reconciled
    };
    Ok(ExecutionReplayReconciliationResult {
        status,
        recovered_effect_transactions,
        reconciled_transactions,
        replay_results,
        completion_records,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryProvenanceView {
    schema_version: String,
    tx_id: String,
    project_root: String,
    audience: String,
    #[serde(rename = "authorization")]
    _authorization: serde_json::Value,
    #[serde(rename = "late_admission_input")]
    _late_admission_input: serde_json::Value,
    #[serde(rename = "late_admission_decision")]
    _late_admission_decision: serde_json::Value,
    #[serde(rename = "commit_admission_input")]
    _commit_admission_input: serde_json::Value,
    #[serde(rename = "commit_admission_decision")]
    _commit_admission_decision: serde_json::Value,
    #[serde(rename = "initial_preflight")]
    _initial_preflight: serde_json::Value,
    #[serde(rename = "late_preflight")]
    _late_preflight: serde_json::Value,
    #[serde(rename = "commit_preflight")]
    _commit_preflight: serde_json::Value,
    commit_descriptor: RecoveryCommitDescriptor,
    commit_digest: String,
    replay_reservation: RecoveryReplayReservation,
}

#[derive(Debug, Deserialize)]
struct RecoveryCommitDescriptor {
    project_root: String,
    audience: String,
    tx_id: String,
    effect_lock_relative_path: String,
    effect_wal_relative_path: String,
}

#[derive(Debug, Deserialize)]
struct RecoveryReplayReservation {
    key_hash: String,
    intent_digest: String,
    commit_digest: String,
    revision: u64,
    state: String,
}

fn validate_recovery_provenance(
    tx_id: &str,
    document: &serde_json::Value,
    replay_binding: &EffectReplayCommitBinding,
    expected_project_root: &str,
    expected_audience: &str,
) -> Result<(), ExecutionReplayReconciliationError> {
    let view: RecoveryProvenanceView =
        serde_json::from_value(document.clone()).map_err(|error| {
            ExecutionReplayReconciliationError::Provenance {
                tx_id: tx_id.to_owned(),
                reason: format!("complete P4b.2c provenance shape is invalid: {error}"),
            }
        })?;
    let reservation = &view.replay_reservation;
    let descriptor = &view.commit_descriptor;
    let valid = view.schema_version == EXECUTION_COMMIT_PROVENANCE_SCHEMA_VERSION
        && view.tx_id == tx_id
        && view.project_root == expected_project_root
        && view.audience == expected_audience
        && view.commit_digest == replay_binding.commit_digest
        && descriptor.project_root == expected_project_root
        && descriptor.audience == expected_audience
        && descriptor.tx_id == tx_id
        && descriptor.effect_lock_relative_path == PREPARED_EFFECT_LOCK_RELATIVE_PATH
        && descriptor.effect_wal_relative_path == PREPARED_EFFECT_WAL_RELATIVE_PATH
        && reservation.key_hash == replay_binding.key_hash
        && reservation.intent_digest == replay_binding.intent_digest
        && reservation.commit_digest == replay_binding.commit_digest
        && reservation.revision == replay_binding.reservation_revision
        && reservation.state == "reserved";
    if !valid {
        return Err(ExecutionReplayReconciliationError::Provenance {
            tx_id: tx_id.to_owned(),
            reason: "root, audience, descriptor, or replay reservation binding changed".to_owned(),
        });
    }
    Ok(())
}

fn command_bindings_match(
    requested: &[ContentAddressedBinding],
    commands: &[CommandContractDocument],
) -> Result<bool, PrepareExecutionError> {
    if requested.len() != commands.len() {
        return Ok(false);
    }
    let requested_refs = requested
        .iter()
        .map(|binding| binding.reference.as_str())
        .collect::<BTreeSet<_>>();
    if requested_refs.len() != requested.len() {
        return Ok(false);
    }
    for command in commands {
        let id = &command.command_contract.id.0;
        let token = command_contract_token(command)
            .map_err(|error| PrepareExecutionError::ContractBinding(error.to_string()))?;
        if !requested
            .iter()
            .any(|binding| binding.reference == *id && binding.token == token)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn payload_bindings_match(
    request: &forge_core_authority::ExecutionRequest,
    effect: &ToolEffectContractDocument,
    payloads: &[RuntimeOperationEffectPayload],
) -> bool {
    let requested = request
        .payloads()
        .iter()
        .map(forge_core_authority::ExecutionPayloadBinding::target_ref)
        .collect::<BTreeSet<_>>();
    let supplied = payloads
        .iter()
        .map(|payload| payload.target_ref.as_str())
        .collect::<BTreeSet<_>>();
    let required = effect
        .tool_effect_contract
        .write_set
        .iter()
        .filter(|write| write.access_mode != forge_core_contracts::tool_effect::AccessMode::Delete)
        .map(|write| write.reference.as_str())
        .collect::<BTreeSet<_>>();
    requested.len() == request.payloads().len()
        && supplied.len() == payloads.len()
        && requested == supplied
        && supplied == required
}

#[allow(clippy::too_many_arguments)]
fn build_commit_descriptor(
    environment: &TrustedExecutionEnvironment,
    operation_ref: &str,
    operation: &OperationContractDocument,
    operation_token: &str,
    command_refs: &[String],
    commands: &[CommandContractDocument],
    effect_ref: &RepoPath,
    effect: &ToolEffectContractDocument,
    effect_token: &str,
    payload_bindings: &[forge_core_authority::ExecutionPayloadBinding],
    payloads: &[RuntimeOperationEffectPayload],
    intent_digest: &str,
    request_id: &StableId,
) -> Result<PreparedCommitDescriptor, PrepareExecutionError> {
    let mut prepared_commands = Vec::with_capacity(commands.len());
    for (source_ref, command) in command_refs.iter().zip(commands) {
        prepared_commands.push(PreparedCommandBinding {
            source_ref: source_ref.clone(),
            command_id: command.command_contract.id.clone(),
            token: command_contract_token(command)
                .map_err(|error| PrepareExecutionError::CommitDescriptor(error.to_string()))?,
        });
    }
    let mut prepared_payloads = Vec::with_capacity(payloads.len());
    for payload in payloads {
        let source_ref = payload_bindings
            .iter()
            .find(|binding| binding.target_ref() == payload.target_ref)
            .map(|binding| path_string(binding.path()))
            .transpose()?
            .ok_or(PrepareExecutionError::PayloadBindingMismatch)?;
        prepared_payloads.push(PreparedPayloadBinding {
            target_ref: payload.target_ref.clone(),
            source_ref,
            content_hash: payload.content_hash.clone(),
            byte_len: u64::try_from(payload.content.len()).unwrap_or(u64::MAX),
            payload_kind: payload.payload_kind,
        });
    }
    prepared_commands.sort_by(|left, right| left.command_id.0.cmp(&right.command_id.0));
    prepared_payloads.sort_by(|left, right| left.target_ref.cmp(&right.target_ref));

    Ok(PreparedCommitDescriptor {
        schema_version: PREPARED_EXECUTION_SCHEMA_VERSION.to_owned(),
        project_root: path_string(environment.project_root())?,
        audience: environment.required_audience().to_owned(),
        operation_ref: operation_ref.to_owned(),
        operation_id: operation.operation_contract.contract_id.clone(),
        operation_token: operation_token.to_owned(),
        commands: prepared_commands,
        effect: PreparedEffectBinding {
            source_ref: effect_ref.0.clone(),
            effect_id: effect.tool_effect_contract.id.clone(),
            token: effect_token.to_owned(),
        },
        payloads: prepared_payloads,
        effect_lock_relative_path: PREPARED_EFFECT_LOCK_RELATIVE_PATH.to_owned(),
        effect_wal_relative_path: PREPARED_EFFECT_WAL_RELATIVE_PATH.to_owned(),
        tx_id: derived_tx_id(request_id, &effect.tool_effect_contract.id, intent_digest),
        durability: "sync_on_append".to_owned(),
    })
}

fn store_effect_payloads(
    payloads: &[RuntimeOperationEffectPayload],
) -> Vec<EffectApplicationPayload> {
    payloads
        .iter()
        .map(|payload| EffectApplicationPayload {
            target_ref: payload.target_ref.clone(),
            content: payload.content.clone(),
            content_hash: payload.content_hash.clone(),
        })
        .collect()
}

fn derived_tx_id(request_id: &StableId, effect_id: &StableId, intent_digest: &str) -> String {
    let sanitize = |value: &str| {
        value
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>()
    };
    let suffix = intent_digest
        .strip_prefix("sha256:")
        .unwrap_or(intent_digest)
        .chars()
        .take(12)
        .collect::<String>();
    format!(
        "prepared-{}-{}-{suffix}",
        sanitize(&request_id.0),
        sanitize(&effect_id.0)
    )
}

fn redacted_admission_projection(
    document: &ExecutionAdmissionInputDocument,
    nonce_fingerprint: &str,
) -> Result<serde_json::Value, ExecutionCommitError> {
    let mut value = serde_json::to_value(document)
        .map_err(|error| ExecutionCommitError::Provenance(error.to_string()))?;
    let admission = value
        .get_mut("execution_admission")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| {
            ExecutionCommitError::Provenance(
                "serialized Admission input has no execution_admission object".to_owned(),
            )
        })?;
    for section in ["request", "replay"] {
        let object = admission
            .get_mut(section)
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| {
                ExecutionCommitError::Provenance(format!(
                    "serialized Admission input has no {section} object"
                ))
            })?;
        let nonce = object.get_mut("nonce").ok_or_else(|| {
            ExecutionCommitError::Provenance(format!("serialized Admission {section} has no nonce"))
        })?;
        *nonce = serde_json::Value::String(format!("redacted:{nonce_fingerprint}"));
    }
    Ok(value)
}

fn path_string(path: &Path) -> Result<String, PrepareExecutionError> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| PrepareExecutionError::NonUtf8Path(path.to_path_buf()))
}
