//! Trusted persistence boundary for governed Domain Pack lifecycle state.
//!
//! One complete immutable generation is activated by flipping exactly one
//! crash-recoverable pointer under a retained OS lock. Serialized plans,
//! locks, reports, and receipts remain evidence. The commit token has no public
//! constructor; the policy/kernel integration must mint it only after fresh
//! supply-chain, resolution, composition, trust, capability, sandbox, and
//! compatibility checks.

use forge_core_authority::{
    domain_pack_registry_snapshot_digest, AnchoredDomainPackSupplyChainSnapshot,
    AnchoredReviewedDomainPackRegistrySnapshot,
};
use forge_core_contracts::domain_pack_learning::DomainPackSemanticAssurance;
use forge_core_contracts::{
    DomainPackActivePointer, DomainPackActivePointerDocument, DomainPackArtifactBinding,
    DomainPackCandidateAuthority, DomainPackCapabilitySandboxPolicyDocument,
    DomainPackCompatibilityStatus, DomainPackCompositionRequestDocument,
    DomainPackCompositionStatus, DomainPackExactLockDocument, DomainPackExpectedLifecycleState,
    DomainPackLifecycleLedgerRecord, DomainPackLifecycleOperation,
    DomainPackLifecyclePreflightDocument, DomainPackLifecyclePreflightStatus,
    DomainPackLifecycleReceipt, DomainPackLifecycleReceiptDocument, DomainPackLockedPackage,
    DomainPackRecoveryReport, DomainPackRecoveryReportDocument, DomainPackRecoveryStatus,
    DomainPackResolutionRequestDocument, DomainPackResolutionStatus,
    DomainPackRuntimeCapabilityRegistryDocument, DomainPackSourceAssurance,
    DomainPackSupplyChainRegistryDocument, DomainPackTrustPolicyDocument, StableId,
    DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
};
use forge_core_decisions::{
    compose_domain_packs, domain_pack_resolution_projection_digest,
    evaluate_domain_pack_compatibility, evaluate_domain_pack_trust,
    join_reviewed_registry_to_resolution, resolve_domain_packs, DomainPackCandidateMaterial,
    DomainPackCapabilityDemand, DomainPackCompatibilityInput,
    DomainPackReviewedResolutionJoinStatus, DomainPackTrustEvaluationInput,
    DomainPackTrustEvaluationStatus,
};
use forge_core_store::crash_replace::{
    recover_file_crash_safe_under_lock, replace_file_crash_safe_under_lock, CrashReplaceError,
    CrashReplaceRecovery, CrashReplaceRecoveryAction,
};
use forge_core_store::{
    acquire_effect_store_lock, sha256_content_hash, EffectStoreLock, EffectStoreLockError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DOMAIN_PACK_STATE_RELATIVE_ROOT: &str = "domain-packs";
pub const DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH: &str = "domain-packs/active.lock.yaml";
pub const DOMAIN_PACK_LIFECYCLE_LOCK_RELATIVE_PATH: &str = "locks/domain-packs.lifecycle.lock";
pub const DOMAIN_PACK_MAX_LOCK_BYTES: u64 = 4 * 1024 * 1024;
pub const DOMAIN_PACK_MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
pub const DOMAIN_PACK_MAX_LEDGER_RECORDS: usize = 10_000;
pub const DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_FILES: usize = 100_000;
pub const DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
pub const DOMAIN_PACK_MAX_SUPPLY_CHAIN_VERIFICATION_AGE_SECONDS: u64 = 300;
pub const DOMAIN_PACK_MAX_CLOCK_FUTURE_SKEW_SECONDS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackLifecycleStateProjection {
    pub active_pointer: Option<DomainPackActivePointerDocument>,
    pub active_lock: Option<DomainPackExactLockDocument>,
    pub ledger_records: Vec<DomainPackLifecycleLedgerRecord>,
}

/// Move-only token required by the mechanical commit path.
///
/// Its constructor is intentionally private. A later integration function in
/// this crate mints it only from opaque verified supply-chain admission plus a
/// freshly recomputed ready preflight.
#[derive(Debug)]
pub struct DomainPackLifecycleCommitAuthority {
    preflight_digest: String,
    project_root: PathBuf,
    project_snapshot_digest: String,
    supply_chain_verified_at_unix: u64,
    supply_chain_expires_at_unix: u64,
    verified_artifacts: Vec<OwnedDomainPackImmutableArtifact>,
}

/// Exact immutable bytes admitted by the TCB for the lifecycle object store.
/// A binding without its bytes can never be activated.
pub struct DomainPackImmutableArtifact<'a> {
    pub binding: &'a DomainPackArtifactBinding,
    pub raw_bytes: &'a [u8],
}

#[derive(Debug)]
struct OwnedDomainPackImmutableArtifact {
    binding: DomainPackArtifactBinding,
    raw_bytes: Vec<u8>,
}

/// Opaque proof that one bounded project-tree snapshot matched the lifecycle
/// request. The TCB recomputes it again immediately before activation.
pub struct VerifiedDomainPackProjectSnapshot {
    project_root: PathBuf,
    snapshot_digest: String,
}

impl fmt::Debug for VerifiedDomainPackProjectSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedDomainPackProjectSnapshot")
            .field("project_root", &self.project_root)
            .field("snapshot_digest", &self.snapshot_digest)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct PreparedDomainPackLifecycleTransaction {
    preflight: DomainPackLifecyclePreflightDocument,
    previous_pointer: Option<DomainPackActivePointerDocument>,
    previous_lock: Option<DomainPackExactLockDocument>,
    previous_pointer_raw_digest: Option<String>,
    record: DomainPackLifecycleLedgerRecord,
    next_pointer: DomainPackActivePointerDocument,
    receipt: DomainPackLifecycleReceiptDocument,
    rollback_target: Option<DomainPackLifecycleReceiptDocument>,
}

/// Fresh non-authoritative inputs that the TCB recomputes before minting commit
/// authority. `anchored_snapshot` and `anchored_reviewed_snapshot` are opaque
/// monotonic capabilities; all other fields are untrusted material whose exact
/// output must match the prepared preflight.
///
/// Serialized or cloned audit evidence cannot substitute the reviewed-registry
/// capability:
///
/// ```compile_fail
/// use forge_core_authority::{
///     AnchoredReviewedDomainPackRegistrySnapshot,
///     VerifiedDomainPackPromotionAuthorizationAudit,
/// };
/// fn requires_opaque(_: &AnchoredReviewedDomainPackRegistrySnapshot) {}
/// fn audit_is_not_authority(audit: &VerifiedDomainPackPromotionAuthorizationAudit) {
///     requires_opaque(audit);
/// }
/// ```
pub struct DomainPackLifecycleAuthorizationContext<'a> {
    pub anchored_snapshot: &'a AnchoredDomainPackSupplyChainSnapshot,
    pub anchored_reviewed_snapshot: &'a AnchoredReviewedDomainPackRegistrySnapshot,
    pub project_snapshot: &'a VerifiedDomainPackProjectSnapshot,
    pub trust_policy_document: &'a DomainPackTrustPolicyDocument,
    pub registry_document: &'a DomainPackSupplyChainRegistryDocument,
    pub resolution_request: &'a DomainPackResolutionRequestDocument,
    pub composition_request: &'a DomainPackCompositionRequestDocument,
    pub materials: &'a [DomainPackCandidateMaterial<'a>],
    pub artifacts: &'a [DomainPackImmutableArtifact<'a>],
    pub trust_input: &'a DomainPackTrustEvaluationInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackGenerationManifest {
    schema_version: String,
    generation: u64,
    record_digest: String,
    lock_digest: String,
    preflight_digest: String,
    compatibility_report_digest: String,
    receipt_digest: String,
    object_raw_digests: Vec<String>,
}

/// Hash a bounded canonical project tree and return an opaque proof only when
/// it matches the exact expected snapshot. Forge state, VCS internals, build
/// output, and dependency caches are excluded consistently with the existing
/// workflow-governance Project Snapshot Adapter.
///
/// # Errors
///
/// Fails closed when the root is invalid, the tree escapes or exceeds bounds,
/// a file changes while hashing, or the resulting digest differs.
pub fn verify_domain_pack_project_snapshot(
    project_root: impl AsRef<Path>,
    expected_digest: &str,
) -> Result<VerifiedDomainPackProjectSnapshot, DomainPackLifecycleStoreError> {
    let project_root = fs::canonicalize(project_root.as_ref())
        .map_err(|error| io_error(project_root.as_ref(), error))?;
    if !project_root.is_dir() {
        return Err(invalid("project_root", "must be an existing directory"));
    }
    let snapshot_digest = project_snapshot_digest(&project_root)?;
    if snapshot_digest != expected_digest {
        return Err(DomainPackLifecycleStoreError::StaleExpectedState {
            expected: expected_digest.to_owned(),
            actual: snapshot_digest,
        });
    }
    Ok(VerifiedDomainPackProjectSnapshot {
        project_root,
        snapshot_digest,
    })
}

#[derive(Debug)]
pub struct LockedDomainPackLifecycle {
    state_root: PathBuf,
    lock: EffectStoreLock,
    state: DomainPackLifecycleStateProjection,
    recovery: CrashReplaceRecovery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DomainPackLifecycleStoreError {
    InvalidArgument {
        field: &'static str,
        reason: String,
    },
    InvalidDigest {
        field: &'static str,
        value: String,
    },
    InvalidDocument {
        path: PathBuf,
        reason: String,
    },
    StaleExpectedState {
        expected: String,
        actual: String,
    },
    PreflightBlocked {
        reason: String,
    },
    ResourceLimit {
        resource: &'static str,
        maximum: u64,
    },
    Lock {
        reason: String,
    },
    CrashReplace {
        reason: String,
    },
    Io {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for DomainPackLifecycleStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument { field, reason } => {
                write!(formatter, "invalid domain-pack lifecycle {field}: {reason}")
            }
            Self::InvalidDigest { field, value } => {
                write!(
                    formatter,
                    "invalid domain-pack lifecycle digest {field}: {value}"
                )
            }
            Self::InvalidDocument { path, reason } => write!(
                formatter,
                "invalid domain-pack lifecycle document {}: {reason}",
                path.display()
            ),
            Self::StaleExpectedState { expected, actual } => write!(
                formatter,
                "stale domain-pack lifecycle state: expected {expected}; actual {actual}"
            ),
            Self::PreflightBlocked { reason } => {
                write!(
                    formatter,
                    "domain-pack lifecycle preflight blocked: {reason}"
                )
            }
            Self::ResourceLimit { resource, maximum } => write!(
                formatter,
                "domain-pack lifecycle {resource} exceeds limit {maximum}"
            ),
            Self::Lock { reason } => {
                write!(formatter, "domain-pack lifecycle lock failed: {reason}")
            }
            Self::CrashReplace { reason } => {
                write!(
                    formatter,
                    "domain-pack lifecycle pointer replacement failed: {reason}"
                )
            }
            Self::Io { path, reason } => write!(
                formatter,
                "domain-pack lifecycle I/O {} failed: {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DomainPackLifecycleStoreError {}

impl From<EffectStoreLockError> for DomainPackLifecycleStoreError {
    fn from(value: EffectStoreLockError) -> Self {
        Self::Lock {
            reason: value.to_string(),
        }
    }
}

impl From<CrashReplaceError> for DomainPackLifecycleStoreError {
    fn from(value: CrashReplaceError) -> Self {
        Self::CrashReplace {
            reason: value.to_string(),
        }
    }
}

/// Acquire the fixed lifecycle lock, reconcile interrupted pointer replacement,
/// and verify the complete active generation and immutable ledger chain.
///
/// # Errors
///
/// Returns a typed lock, recovery, confinement, integrity, or I/O error when
/// the retained state cannot be proven complete.
pub fn lock_domain_pack_lifecycle(
    state_root: impl AsRef<Path>,
) -> Result<LockedDomainPackLifecycle, DomainPackLifecycleStoreError> {
    let state_root = canonical_state_root(state_root.as_ref())?;
    let lock = acquire_effect_store_lock(&state_root, DOMAIN_PACK_LIFECYCLE_LOCK_RELATIVE_PATH)?;
    let recovery = recover_file_crash_safe_under_lock(
        &state_root,
        &lock,
        DOMAIN_PACK_LIFECYCLE_LOCK_RELATIVE_PATH,
        DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH,
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )?;
    let state = load_state_under_lock(&state_root)?;
    Ok(LockedDomainPackLifecycle {
        state_root,
        lock,
        state,
        recovery,
    })
}

impl LockedDomainPackLifecycle {
    #[must_use]
    pub fn projection(&self) -> &DomainPackLifecycleStateProjection {
        &self.state
    }

    /// Typed non-authoritative account of the pointer recovery performed while
    /// acquiring this retained lifecycle lock.
    #[must_use]
    pub fn recovery_report(&self) -> DomainPackRecoveryReportDocument {
        let status = match self.recovery.action {
            CrashReplaceRecoveryAction::Noop => DomainPackRecoveryStatus::Clean,
            CrashReplaceRecoveryAction::RemovedUncommittedNext
            | CrashReplaceRecoveryAction::AbortedToPrevious
            | CrashReplaceRecoveryAction::RestoredPrevious => {
                DomainPackRecoveryStatus::RecoveredPrior
            }
            CrashReplaceRecoveryAction::CommittedInitial
            | CrashReplaceRecoveryAction::CleanedCommitted => {
                DomainPackRecoveryStatus::RecoveredTarget
            }
            _ => DomainPackRecoveryStatus::BlockedAmbiguous,
        };
        let repaired_artifact_refs = if status == DomainPackRecoveryStatus::Clean {
            Vec::new()
        } else {
            vec![DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH.to_owned()]
        };
        DomainPackRecoveryReportDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_recovery_report: DomainPackRecoveryReport {
                authority: DomainPackCandidateAuthority::CandidateOnly,
                status,
                active_state: self
                    .state
                    .active_pointer
                    .as_ref()
                    .map(|pointer| pointer.domain_pack_active_pointer.clone()),
                lifecycle_head_digest: self.state.active_pointer.as_ref().map(|pointer| {
                    pointer
                        .domain_pack_active_pointer
                        .lifecycle_head_digest
                        .clone()
                }),
                repaired_artifact_refs,
                issues: Vec::new(),
            },
        }
    }

    /// Structurally prepare a transaction while the exact lifecycle lock is
    /// retained. This does not mint commit authority.
    ///
    /// # Errors
    ///
    /// Fails when the preflight is blocked, state is stale, rollback evidence
    /// is unreachable, or any digest/integrity check fails.
    #[allow(clippy::too_many_lines)] // Linear construction keeps receipt/ledger bindings auditable.
    pub fn prepare_candidate(
        &self,
        preflight: DomainPackLifecyclePreflightDocument,
    ) -> Result<PreparedDomainPackLifecycleTransaction, DomainPackLifecycleStoreError> {
        validate_preflight(&preflight)?;
        let body = &preflight.domain_pack_lifecycle_preflight;
        let request = &body.request.domain_pack_lifecycle_request;
        if request.project_id != body.proposed_lock.domain_pack_exact_lock.payload.project_id {
            return Err(invalid("project_id", "request and proposed lock differ"));
        }
        verify_expected_state(&request.expected_state, &self.state)?;
        let observed_at_unix = trusted_now_unix()?;
        let rollback_target = match &request.operation {
            DomainPackLifecycleOperation::Rollback {
                target_receipt_digest,
                target_lock_digest,
            } => Some(load_committed_receipt(
                &self.state_root,
                &self.state,
                target_receipt_digest,
                target_lock_digest,
            )?),
            _ => None,
        };

        let previous_pointer = self.state.active_pointer.clone();
        let previous_pointer_raw_digest = read_optional_bytes(
            &self.state_root.join(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH),
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
        )?
        .map(|bytes| sha256_content_hash(&bytes));
        let generation = previous_pointer.as_ref().map_or(0, |pointer| {
            pointer.domain_pack_active_pointer.generation + 1
        });
        let prior_head = previous_pointer.as_ref().map(|pointer| {
            pointer
                .domain_pack_active_pointer
                .lifecycle_head_digest
                .clone()
        });
        let prior_pointer_digest = previous_pointer
            .as_ref()
            .map(|pointer| pointer.domain_pack_active_pointer.pointer_digest.clone());
        let mut record = DomainPackLifecycleLedgerRecord {
            sequence: generation,
            previous_record_digest: prior_head.clone(),
            operation: request.operation.clone(),
            request_digest: body.request_digest.clone(),
            preflight_digest: body.preflight_digest.clone(),
            from_pointer_digest: prior_pointer_digest,
            to_generation: generation,
            active_lock_digest: body
                .proposed_lock
                .domain_pack_exact_lock
                .lock_digest
                .clone(),
            compatibility_report_digest: body
                .compatibility_report
                .domain_pack_compatibility_report
                .report_digest
                .clone(),
            principal_id: request.principal_id.clone(),
            observed_at_unix,
            record_digest: String::new(),
        };
        record.record_digest = digest_record(&record)?;
        let mut pointer = DomainPackActivePointer {
            project_id: request.project_id.clone(),
            generation,
            active_lock_digest: record.active_lock_digest.clone(),
            lifecycle_head_digest: record.record_digest.clone(),
            pointer_digest: String::new(),
        };
        pointer.pointer_digest = digest_pointer(&pointer)?;
        let next_pointer = DomainPackActivePointerDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_active_pointer: pointer.clone(),
        };
        let lock = &body.proposed_lock.domain_pack_exact_lock.payload;
        let mut receipt = DomainPackLifecycleReceipt {
            receipt_id: StableId(format!("domain-pack.lifecycle.receipt.{generation}")),
            operation: request.operation.clone(),
            principal_id: request.principal_id.clone(),
            request_digest: body.request_digest.clone(),
            preflight_digest: body.preflight_digest.clone(),
            resolution_digest: lock.resolution_digest.clone(),
            composition_digest: lock.composition_digest.clone(),
            compatibility_report_digest: record.compatibility_report_digest.clone(),
            trust_policy_digest: lock.trust_policy_digest.clone(),
            reviewer_registry_digest: lock.reviewer_registry_digest.clone(),
            reviewed_registry_digest: lock.reviewed_registry_digest.clone(),
            capability_registry_digest: lock.capability_registry_digest.clone(),
            sandbox_policy_digest: lock.sandbox_policy_digest.clone(),
            from_state: previous_pointer
                .as_ref()
                .map(|value| value.domain_pack_active_pointer.clone()),
            to_state: pointer,
            prior_ledger_head_digest: prior_head,
            new_ledger_head_digest: record.record_digest.clone(),
            applied_object_digests: staged_digests(body),
            observed_at_unix,
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = digest_receipt(&receipt)?;
        let receipt = DomainPackLifecycleReceiptDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_lifecycle_receipt: receipt,
        };
        Ok(PreparedDomainPackLifecycleTransaction {
            preflight,
            previous_pointer,
            previous_lock: self.state.active_lock.clone(),
            previous_pointer_raw_digest,
            record,
            next_pointer,
            receipt,
            rollback_target,
        })
    }

    /// Consume an opaque authorization and activate the complete prepared
    /// generation. All immutable objects are durable before the pointer flip.
    ///
    /// # Errors
    ///
    /// Fails closed on stale authority/project/state, persistence/recovery
    /// failure, or any post-commit integrity mismatch.
    #[allow(clippy::needless_pass_by_value)] // Move-only authority must be consumed exactly once.
    pub fn commit(
        &mut self,
        prepared: PreparedDomainPackLifecycleTransaction,
        authority: DomainPackLifecycleCommitAuthority,
    ) -> Result<DomainPackLifecycleReceiptDocument, DomainPackLifecycleStoreError> {
        let body = &prepared.preflight.domain_pack_lifecycle_preflight;
        if authority.preflight_digest != body.preflight_digest {
            return Err(invalid(
                "commit_authority",
                "authority does not bind the prepared preflight",
            ));
        }
        validate_supply_chain_freshness(
            authority.supply_chain_verified_at_unix,
            authority.supply_chain_expires_at_unix,
        )?;
        if project_snapshot_digest(&authority.project_root)? != authority.project_snapshot_digest {
            return Err(stale_project_snapshot(
                &authority.project_snapshot_digest,
                "project changed before lifecycle commit",
            ));
        }
        // Re-read after all policy work and immediately before persistence.
        self.state = load_state_under_lock(&self.state_root)?;
        verify_expected_state(
            &body.request.domain_pack_lifecycle_request.expected_state,
            &self.state,
        )?;
        if self.state.active_pointer != prepared.previous_pointer {
            return Err(stale(
                "prepared pointer changed under retained lock",
                &self.state,
            ));
        }

        materialize_generation(&self.state_root, &prepared, &authority.verified_artifacts)?;
        if project_snapshot_digest(&authority.project_root)? != authority.project_snapshot_digest {
            return Err(stale_project_snapshot(
                &authority.project_snapshot_digest,
                "project changed while materializing lifecycle generation",
            ));
        }
        let pointer_bytes = yaml_bytes(&prepared.next_pointer)?;
        replace_file_crash_safe_under_lock(
            &self.state_root,
            &self.lock,
            DOMAIN_PACK_LIFECYCLE_LOCK_RELATIVE_PATH,
            DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH,
            prepared.previous_pointer_raw_digest.as_deref(),
            &pointer_bytes,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
        )?;
        publish_committed_receipt(&self.state_root, &prepared.receipt)?;
        self.state = load_state_under_lock(&self.state_root)?;
        if self.state.active_pointer.as_ref() != Some(&prepared.next_pointer) {
            return Err(invalid(
                "active_pointer",
                "post-commit active pointer differs from prepared target",
            ));
        }
        Ok(prepared.receipt)
    }
}

/// Mint the move-only commit capability from an opaque verified registry
/// snapshot and the exact deterministic policy result bound into `prepared`.
///
/// This is intentionally the only public authorization path. Serialized
/// supply-chain assessments and preflight documents cannot manufacture the
/// opaque snapshot, and every selected/locked package is joined to one exact
/// verified registry record before authority is created.
///
/// # Errors
///
/// Returns a blocked or integrity error unless every supply-chain, monotonic
/// anchor, operation-intent, resolution, composition, trust, capability,
/// project, artifact, and compatibility binding recomputes exactly.
#[allow(clippy::too_many_lines)] // One explicit TCB proof chain is preferable to hidden partial authority.
pub fn authorize_prepared_domain_pack_lifecycle(
    prepared: &PreparedDomainPackLifecycleTransaction,
    context: &DomainPackLifecycleAuthorizationContext<'_>,
) -> Result<DomainPackLifecycleCommitAuthority, DomainPackLifecycleStoreError> {
    validate_preflight(&prepared.preflight)?;
    let body = &prepared.preflight.domain_pack_lifecycle_preflight;
    let verified_snapshot = context.anchored_snapshot.verified_snapshot();
    let reviewed_snapshot = context.anchored_reviewed_snapshot;
    let resolution = &body.resolution.domain_pack_resolution_projection;
    let composition = &body.composition.domain_pack_composition_projection;
    let lock = &body.proposed_lock.domain_pack_exact_lock;
    validate_supply_chain_freshness(
        verified_snapshot.verified_at_unix(),
        verified_snapshot.expires_at_unix(),
    )?;

    if body.authority != DomainPackCandidateAuthority::CandidateOnly
        || resolution.authority != DomainPackCandidateAuthority::CandidateOnly
        || composition.authority != DomainPackCandidateAuthority::CandidateOnly
    {
        return Err(blocked("candidate projections use an unknown authority"));
    }
    if resolution.status != DomainPackResolutionStatus::Resolved || !resolution.issues.is_empty() {
        return Err(blocked("resolution is not clean and resolved"));
    }
    let is_remove = matches!(
        body.request.domain_pack_lifecycle_request.operation,
        DomainPackLifecycleOperation::Remove { .. }
    );
    let is_historical_empty_rollback = matches!(
        body.request.domain_pack_lifecycle_request.operation,
        DomainPackLifecycleOperation::Rollback { .. }
    ) && lock.payload.packages.is_empty()
        && prepared.rollback_target.is_some();
    let composition_allowed = composition.issues.is_empty()
        && if is_remove || is_historical_empty_rollback {
            matches!(
                composition.status,
                DomainPackCompositionStatus::Composable | DomainPackCompositionStatus::Blocked
            )
        } else {
            composition.status == DomainPackCompositionStatus::Composable
                && composition.gaps.is_empty()
        };
    if !composition_allowed {
        return Err(blocked("composition is not clean and composable"));
    }
    let registry_digest = domain_pack_registry_snapshot_digest(context.registry_document)
        .map_err(|error| blocked(&format!("registry digest verification failed: {error}")))?;
    if registry_digest != verified_snapshot.snapshot_digest()
        || lock.payload.registry_snapshot_digest != verified_snapshot.snapshot_digest()
        || lock.payload.reviewer_registry_digest != reviewed_snapshot.reviewer_registry_digest()
        || lock.payload.reviewed_registry_digest != reviewed_snapshot.registry_digest()
        || lock.payload.trust_policy_digest != verified_snapshot.trust_policy_digest()
    {
        return Err(blocked(
            "verified supply-chain, reviewed, reviewer, or trust-policy digest differs from exact lock",
        ));
    }
    if context.trust_policy_document.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION
        || context.trust_policy_document.domain_pack_trust_policy
            != context.trust_input.trust_policy
        || canonical_digest(context.trust_policy_document)?
            != verified_snapshot.trust_policy_digest()
    {
        return Err(blocked(
            "fresh trust input differs from the operator-verified trust policy",
        ));
    }
    let capability_registry_document = DomainPackRuntimeCapabilityRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_runtime_capability_registry: context.trust_input.capability_registry.clone(),
    };
    let sandbox_policy_document = DomainPackCapabilitySandboxPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_capability_sandbox_policy: context.trust_input.sandbox_policy.clone(),
    };
    if canonical_digest(&capability_registry_document)? != lock.payload.capability_registry_digest
        || canonical_digest(&sandbox_policy_document)? != lock.payload.sandbox_policy_digest
    {
        return Err(blocked(
            "runtime capability registry or sandbox policy differs from exact lock",
        ));
    }
    if lock.payload.resolution_digest != resolution.resolution_digest
        || lock.payload.composition_digest != composition.composition_digest
    {
        return Err(blocked(
            "resolution or composition digest differs from exact lock",
        ));
    }
    let resolution_input = &context.resolution_request.domain_pack_resolution_request;
    let composition_input = &context.composition_request.domain_pack_composition_request;
    let lifecycle_request = &body.request.domain_pack_lifecycle_request;
    validate_operation_intent(
        &lifecycle_request.operation,
        prepared.previous_lock.as_ref(),
        &body.proposed_lock,
        resolution_input,
        prepared.rollback_target.as_ref(),
    )?;
    if canonical_digest(&body.request)? != body.request_digest
        || body.observed_state != lifecycle_request.expected_state
        || resolution.request_id != resolution_input.request_id
        || composition.request_id != composition_input.request_id
        || resolution_input.project_id != lock.payload.project_id
        || resolution_input.core != lock.payload.core
        || composition_input.core != lock.payload.core
        || composition_input.requirements
            != resolution_input
                .requirements
                .domain_pack_project_requirements
        || lock.payload.roots != resolution_input.roots
        || resolution_input.registry_snapshot_digest != lock.payload.registry_snapshot_digest
        || canonical_digest(&resolution_input.requirements)? != lock.payload.requirements_digest
        || canonical_digest(context.resolution_request)?
            != lifecycle_request.resolution_request_digest
    {
        return Err(blocked(
            "resolution/composition inputs differ from exact lock or lifecycle request",
        ));
    }
    let expected_project_snapshot = match &lifecycle_request.expected_state {
        DomainPackExpectedLifecycleState::Uninitialized {
            project_snapshot_digest,
        }
        | DomainPackExpectedLifecycleState::Initialized {
            project_snapshot_digest,
            ..
        } => project_snapshot_digest,
    };
    if expected_project_snapshot != &lifecycle_request.project_snapshot_digest {
        return Err(blocked(
            "lifecycle request and expected state bind different project snapshots",
        ));
    }
    if context.project_snapshot.snapshot_digest != lifecycle_request.project_snapshot_digest
        || project_snapshot_digest(&context.project_snapshot.project_root)?
            != context.project_snapshot.snapshot_digest
    {
        return Err(blocked(
            "fresh project snapshot differs from lifecycle request",
        ));
    }
    let report = &body.compatibility_report.domain_pack_compatibility_report;
    if report.to_lock_digest != lock.lock_digest
        || report.operation != body.request.domain_pack_lifecycle_request.operation
    {
        return Err(blocked(
            "compatibility report does not bind the exact lifecycle operation and lock",
        ));
    }

    let mut recomputed_resolution =
        resolve_domain_packs(context.resolution_request, context.registry_document);
    let reviewed_join =
        join_reviewed_registry_to_resolution(&recomputed_resolution, reviewed_snapshot.registry());
    if reviewed_join.reviewed_registry_digest != reviewed_snapshot.registry_digest() {
        return Err(blocked(
            "reviewed eligibility join differs from opaque reviewed registry",
        ));
    }
    for selected in &mut recomputed_resolution
        .domain_pack_resolution_projection
        .selected
    {
        let verified = verified_snapshot.entries().iter().find(|entry| {
            let record = entry.record();
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
                && record.namespace_grant_id == selected.namespace_grant_id
        });
        if verified.is_none() {
            return Err(blocked(
                "structurally resolved package is absent from opaque verified snapshot",
            ));
        }
        selected.source_assurance = DomainPackSourceAssurance::SupplyChainVerified;
        let Some(review) = reviewed_join.joins.iter().find(|review| {
            review.publisher == selected.identity.publisher.0
                && review.name == selected.identity.name.0
                && review.version == selected.identity.version
                && review.package_digest == selected.package.package_digest
                && review.registry_record_digest == selected.registry_record_digest
        }) else {
            return Err(blocked(
                "resolved package is absent from exact reviewed-registry join",
            ));
        };
        match review.status {
            DomainPackReviewedResolutionJoinStatus::EligibleReviewed => {
                let Some(entry_digest) = review.reviewed_entry_digest.as_ref() else {
                    return Err(blocked("eligible reviewed join lacks an entry digest"));
                };
                let Some(entry) = reviewed_snapshot
                    .registry()
                    .domain_pack_reviewed_registry
                    .entries
                    .iter()
                    .find(|entry| &entry.entry_digest == entry_digest)
                else {
                    return Err(blocked(
                        "eligible reviewed join is absent from opaque reviewed registry",
                    ));
                };
                selected.semantic_assurance = DomainPackSemanticAssurance::Reviewed;
                selected.reviewed_entry_digest = Some(entry.entry_digest.clone());
                selected.promotion_authorization_digest = Some(entry.authorization_digest.clone());
            }
            DomainPackReviewedResolutionJoinStatus::IneligibleDeprecated
            | DomainPackReviewedResolutionJoinStatus::IneligibleRevoked
            | DomainPackReviewedResolutionJoinStatus::IneligibleSuperseded
                if is_remove =>
            {
                let previous = prepared
                    .previous_lock
                    .as_ref()
                    .and_then(|lock| {
                        lock.domain_pack_exact_lock
                            .payload
                            .packages
                            .iter()
                            .find(|package| {
                                package.identity == selected.identity
                                    && package.package_digest == selected.package.package_digest
                                    && package.registry_record_digest
                                        == selected.registry_record_digest
                            })
                    })
                    .ok_or_else(|| {
                        blocked(
                            "remove may retain an ineligible package only from the exact previous lock",
                        )
                    })?;
                if previous.source_assurance != DomainPackSourceAssurance::SupplyChainVerified
                    || previous.semantic_assurance != DomainPackSemanticAssurance::Reviewed
                    || previous.reviewed_entry_digest.is_none()
                    || previous.promotion_authorization_digest.is_none()
                {
                    return Err(blocked(
                        "retained ineligible package lacks prior dual-axis assurance",
                    ));
                }
                selected.semantic_assurance = previous.semantic_assurance;
                selected.reviewed_entry_digest = previous.reviewed_entry_digest.clone();
                selected.promotion_authorization_digest =
                    previous.promotion_authorization_digest.clone();
            }
            _ => {
                return Err(blocked(
                    "selected package is not exactly eligible in the reviewed registry",
                ));
            }
        }
    }
    let promoted = &mut recomputed_resolution.domain_pack_resolution_projection;
    promoted.resolution_digest = domain_pack_resolution_projection_digest(
        context.resolution_request,
        verified_snapshot.snapshot_digest(),
        promoted,
    );
    if recomputed_resolution != body.resolution {
        return Err(blocked(
            "fresh deterministic resolution differs from prepared preflight",
        ));
    }
    validate_reviewed_operation_transition(
        &lifecycle_request.operation,
        prepared.previous_lock.as_ref(),
        &lock.payload.packages,
        &reviewed_join,
    )?;
    let recomputed_composition =
        compose_domain_packs(context.composition_request, context.materials);
    if recomputed_composition != body.composition {
        return Err(blocked(
            "fresh raw-sidecar composition differs from prepared preflight",
        ));
    }
    let compatibility_input = DomainPackCompatibilityInput {
        report_id: report.report_id.clone(),
        operation: body.request.domain_pack_lifecycle_request.operation.clone(),
        sealed_core: lock.payload.core.clone(),
        from_lock: prepared.previous_lock.clone(),
        to_lock: body.proposed_lock.clone(),
    };
    if evaluate_domain_pack_compatibility(&compatibility_input) != body.compatibility_report {
        return Err(blocked(
            "fresh compatibility evaluation differs from prepared preflight",
        ));
    }

    let mut verified_records = verified_snapshot
        .entries()
        .iter()
        .map(|entry| (entry.record().record_digest.as_str(), entry.record()))
        .collect::<std::collections::BTreeMap<_, _>>();
    if verified_records.len() != verified_snapshot.entries().len() {
        return Err(blocked(
            "verified snapshot contains duplicate record digests",
        ));
    }
    if resolution.selected.len() != lock.payload.packages.len()
        || body.supply_chain_assessments.len() != lock.payload.packages.len()
    {
        return Err(blocked(
            "resolution, supply-chain assessments, and exact lock differ in cardinality",
        ));
    }

    let mut assessments = body
        .supply_chain_assessments
        .iter()
        .map(|assessment| (assessment.registry_record_digest.as_str(), assessment))
        .collect::<std::collections::BTreeMap<_, _>>();
    if assessments.len() != body.supply_chain_assessments.len() {
        return Err(blocked("supply-chain assessment is duplicated"));
    }
    for locked in &lock.payload.packages {
        if locked.source_assurance != DomainPackSourceAssurance::SupplyChainVerified {
            return Err(blocked("locked package is not supply-chain verified"));
        }
        if locked.semantic_assurance != DomainPackSemanticAssurance::Reviewed
            || locked.reviewed_entry_digest.is_none()
            || locked.promotion_authorization_digest.is_none()
        {
            return Err(blocked(
                "locked package lacks exact reviewed semantic assurance",
            ));
        }
        let Some(record) = verified_records.remove(locked.registry_record_digest.as_str()) else {
            return Err(blocked(
                "locked package has no exact verified registry record",
            ));
        };
        if record.identity != locked.identity
            || record.package_digest != locked.package_digest
            || record.manifest_digest != locked.manifest_binding.canonical_sha256
            || record.content_digest != locked.content_binding.canonical_sha256
            || record.license_digest != locked.license_binding.canonical_sha256
            || record.namespace_grant_id != locked.namespace_grant_id
        {
            return Err(blocked(
                "locked package differs from verified registry record",
            ));
        }
        let fixture_digests = locked
            .fixture_bindings
            .iter()
            .map(|binding| binding.canonical_sha256.clone())
            .collect::<Vec<_>>();
        if fixture_digests != record.fixture_digests {
            return Err(blocked(
                "locked fixtures differ from verified registry record",
            ));
        }
        let Some(assessment) = assessments.remove(locked.registry_record_digest.as_str()) else {
            return Err(blocked(
                "locked package has no exact supply-chain assessment",
            ));
        };
        if assessment.package_digest != locked.package_digest
            || !assessment.publisher_signature_verified
            || !assessment.registry_signature_threshold_verified
            || !assessment.namespace_grant_verified
            || assessment.revoked
        {
            return Err(blocked("supply-chain assessment is not fully accepted"));
        }
        let selected = resolution
            .selected
            .iter()
            .find(|selected| selected.registry_record_digest == locked.registry_record_digest);
        let Some(selected) = selected else {
            return Err(blocked("locked package was not selected by resolution"));
        };
        if selected.identity != locked.identity
            || selected.package.package_digest != locked.package_digest
            || selected.package.manifest != locked.manifest_binding
            || selected.package.content != locked.content_binding
            || selected.package.license != locked.license_binding
            || selected.package.fixtures != locked.fixture_bindings
            || selected.namespace_grant_id != locked.namespace_grant_id
            || selected.dependencies != locked.dependencies
            || selected.deterministic_order != locked.deterministic_order
            || selected.source_assurance != DomainPackSourceAssurance::SupplyChainVerified
            || selected.semantic_assurance != locked.semantic_assurance
            || selected.reviewed_entry_digest != locked.reviewed_entry_digest
            || selected.promotion_authorization_digest != locked.promotion_authorization_digest
        {
            return Err(blocked("resolved and locked package bindings differ"));
        }
        let composed = composition.ordered_packs.iter().find(|composed| {
            composed.identity == locked.identity
                && composed.deterministic_order == locked.deterministic_order
        });
        let Some(composed) = composed else {
            return Err(blocked("locked package was not included in composition"));
        };
        if composed.manifest_digest != locked.manifest_binding.canonical_sha256
            || composed.content_digest != locked.content_binding.canonical_sha256
        {
            return Err(blocked("composed and locked artifact bindings differ"));
        }
    }
    if composition.ordered_packs.len() != lock.payload.packages.len() {
        return Err(blocked("composition and exact lock differ in cardinality"));
    }
    if lock.payload.unresolved_composition_gaps != composition.gaps {
        return Err(blocked(
            "exact lock does not preserve all unresolved composition gaps",
        ));
    }
    if normalized_artifact_bindings(&body.staged_artifacts)
        != expected_staged_artifact_bindings(&lock.payload.packages)
    {
        return Err(blocked(
            "staged artifact bindings do not exactly cover the locked package set",
        ));
    }
    if !assessments.is_empty() {
        return Err(blocked(
            "preflight contains an unselected supply-chain assessment",
        ));
    }

    if context.trust_input.project_id != lock.payload.project_id
        || context.trust_input.selected.len() != resolution.selected.len()
    {
        return Err(blocked(
            "fresh trust input does not cover the exact resolved project",
        ));
    }
    for selected in &context.trust_input.selected {
        let Some(resolved) = resolution.selected.iter().find(|resolved| {
            resolved.registry_record_digest == selected.package.registry_record_digest
        }) else {
            return Err(blocked("trust input contains an unresolved package"));
        };
        let Some(assessment) = body.supply_chain_assessments.iter().find(|assessment| {
            assessment.registry_record_digest == selected.package.registry_record_digest
        }) else {
            return Err(blocked(
                "trust input package lacks verified supply-chain assessment",
            ));
        };
        if !selected.structurally_valid
            || &selected.package != resolved
            || &selected.supply_chain != assessment
        {
            return Err(blocked(
                "trust input package differs from resolution or verified assessment",
            ));
        }
        let expected_demands = expected_capability_demands(&selected.package, composition_input)?;
        if normalized_capability_demands(&selected.capability_demands)
            != normalized_capability_demands(&expected_demands)
        {
            return Err(blocked(
                "trust input capability demands differ from raw composed package semantics",
            ));
        }
    }

    let trust_evaluation = evaluate_domain_pack_trust(context.trust_input);
    if trust_evaluation.status != DomainPackTrustEvaluationStatus::Approved
        || !trust_evaluation.issues.is_empty()
        || trust_evaluation.trust_decisions != body.trust_decisions
        || trust_evaluation.verified_capability_bindings
            != lock.payload.verified_capability_bindings
        || trust_evaluation.capability_gaps != body.capability_gaps
        || trust_evaluation.capability_gaps != lock.payload.unresolved_capability_gaps
    {
        return Err(blocked(
            "fresh trust, capability, or sandbox evaluation differs from preflight lock",
        ));
    }

    let verified_artifacts = verify_immutable_artifacts(context.artifacts, &body.staged_artifacts)?;
    Ok(DomainPackLifecycleCommitAuthority {
        preflight_digest: body.preflight_digest.clone(),
        project_root: context.project_snapshot.project_root.clone(),
        project_snapshot_digest: context.project_snapshot.snapshot_digest.clone(),
        supply_chain_verified_at_unix: verified_snapshot.verified_at_unix(),
        supply_chain_expires_at_unix: verified_snapshot.expires_at_unix(),
        verified_artifacts,
    })
}

fn materialize_generation(
    state_root: &Path,
    prepared: &PreparedDomainPackLifecycleTransaction,
    artifacts: &[OwnedDomainPackImmutableArtifact],
) -> Result<(), DomainPackLifecycleStoreError> {
    let body = &prepared.preflight.domain_pack_lifecycle_preflight;
    let record_token = digest_token(&prepared.record.record_digest, "record.record_digest")?;

    for artifact in artifacts {
        let object_token = digest_token(&artifact.binding.raw_sha256, "artifact.raw_sha256")?;
        let object_path = state_root
            .join(DOMAIN_PACK_STATE_RELATIVE_ROOT)
            .join("objects")
            .join(object_token);
        write_immutable_under_root(state_root, &object_path, &artifact.raw_bytes)?;
        let stored =
            read_required_state_bytes(state_root, &object_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
        if sha256_content_hash(&stored) != artifact.binding.raw_sha256 {
            return Err(invalid(
                "artifact.raw_sha256",
                "object store post-write digest mismatch",
            ));
        }
    }

    let staging_root = state_root
        .join(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("staging")
        .join(record_token);
    ensure_secure_directory(state_root, &staging_root)?;
    write_immutable_under_root(
        state_root,
        &staging_root.join("lock.yaml"),
        &yaml_bytes(&body.proposed_lock)?,
    )?;
    write_immutable_under_root(
        state_root,
        &staging_root.join("preflight.yaml"),
        &yaml_bytes(&prepared.preflight)?,
    )?;
    write_immutable_under_root(
        state_root,
        &staging_root.join("compatibility.yaml"),
        &yaml_bytes(&body.compatibility_report)?,
    )?;
    write_immutable_under_root(
        state_root,
        &staging_root.join("receipt.yaml"),
        &yaml_bytes(&prepared.receipt)?,
    )?;
    let mut object_raw_digests = artifacts
        .iter()
        .map(|artifact| artifact.binding.raw_sha256.clone())
        .collect::<Vec<_>>();
    object_raw_digests.sort();
    object_raw_digests.dedup();
    let manifest = DomainPackGenerationManifest {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        generation: prepared.next_pointer.domain_pack_active_pointer.generation,
        record_digest: prepared.record.record_digest.clone(),
        lock_digest: body
            .proposed_lock
            .domain_pack_exact_lock
            .lock_digest
            .clone(),
        preflight_digest: body.preflight_digest.clone(),
        compatibility_report_digest: body
            .compatibility_report
            .domain_pack_compatibility_report
            .report_digest
            .clone(),
        receipt_digest: prepared
            .receipt
            .domain_pack_lifecycle_receipt
            .receipt_digest
            .clone(),
        object_raw_digests,
    };
    write_immutable_under_root(
        state_root,
        &staging_root.join("generation.yaml"),
        &yaml_bytes(&manifest)?,
    )?;

    let generation_root = generation_root(
        state_root,
        prepared.next_pointer.domain_pack_active_pointer.generation,
        &prepared.record.record_digest,
    )?;
    if generation_root.exists() {
        validate_generation_directory(state_root, &generation_root, &manifest)?;
        fs::remove_dir_all(&staging_root).map_err(|error| io_error(&staging_root, error))?;
    } else {
        let parent = generation_root
            .parent()
            .ok_or_else(|| invalid("generation_root", "missing parent"))?;
        ensure_secure_directory(state_root, parent)?;
        fs::rename(&staging_root, &generation_root)
            .map_err(|error| io_error(&generation_root, error))?;
        validate_generation_directory(state_root, &generation_root, &manifest)?;
    }

    let ledger_path = state_root
        .join(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("ledger")
        .join(format!("{record_token}.yaml"));
    write_immutable_under_root(state_root, &ledger_path, &yaml_bytes(&prepared.record)?)?;
    validate_prepared_generation(state_root, &generation_root, prepared, &manifest)?;
    Ok(())
}

fn validate_prepared_generation(
    state_root: &Path,
    generation_root: &Path,
    prepared: &PreparedDomainPackLifecycleTransaction,
    manifest: &DomainPackGenerationManifest,
) -> Result<(), DomainPackLifecycleStoreError> {
    let body = &prepared.preflight.domain_pack_lifecycle_preflight;
    for (name, expected) in [
        ("generation.yaml", yaml_bytes(manifest)?),
        ("lock.yaml", yaml_bytes(&body.proposed_lock)?),
        ("preflight.yaml", yaml_bytes(&prepared.preflight)?),
        (
            "compatibility.yaml",
            yaml_bytes(&body.compatibility_report)?,
        ),
        ("receipt.yaml", yaml_bytes(&prepared.receipt)?),
    ] {
        let path = generation_root.join(name);
        let actual = read_required_state_bytes(state_root, &path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
        if actual != expected {
            return Err(DomainPackLifecycleStoreError::InvalidDocument {
                path,
                reason: "published generation file differs from admitted bytes".to_owned(),
            });
        }
    }
    Ok(())
}

fn publish_committed_receipt(
    state_root: &Path,
    receipt: &DomainPackLifecycleReceiptDocument,
) -> Result<(), DomainPackLifecycleStoreError> {
    let token = digest_token(
        &receipt.domain_pack_lifecycle_receipt.receipt_digest,
        "receipt.receipt_digest",
    )?;
    let receipt_path = state_root
        .join(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("receipts")
        .join(format!("{token}.yaml"));
    write_immutable_under_root(state_root, &receipt_path, &yaml_bytes(receipt)?)
}

fn generation_root(
    state_root: &Path,
    generation: u64,
    record_digest: &str,
) -> Result<PathBuf, DomainPackLifecycleStoreError> {
    let token = digest_token(record_digest, "generation.record_digest")?;
    Ok(state_root
        .join(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("generations")
        .join(format!("{generation:020}-{token}")))
}

fn validate_generation_directory(
    state_root: &Path,
    generation_root: &Path,
    expected: &DomainPackGenerationManifest,
) -> Result<(), DomainPackLifecycleStoreError> {
    assert_confined_state_path(state_root, generation_root)?;
    let path = generation_root.join("generation.yaml");
    let bytes = read_required_state_bytes(state_root, &path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
    let actual: DomainPackGenerationManifest = parse_yaml(&path, &bytes)?;
    if &actual != expected {
        return Err(invalid(
            "generation_manifest",
            "published generation differs from prepared manifest",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Full-generation cross-link validation is deliberately linear.
fn load_state_under_lock(
    state_root: &Path,
) -> Result<DomainPackLifecycleStateProjection, DomainPackLifecycleStoreError> {
    let pointer_path = state_root.join(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH);
    let Some(pointer_bytes) =
        read_optional_state_bytes(state_root, &pointer_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?
    else {
        return Ok(DomainPackLifecycleStateProjection {
            active_pointer: None,
            active_lock: None,
            ledger_records: Vec::new(),
        });
    };
    let pointer: DomainPackActivePointerDocument = parse_yaml(&pointer_path, &pointer_bytes)?;
    validate_schema(&pointer.schema_version, "active_pointer.schema_version")?;
    let pointer_value = &pointer.domain_pack_active_pointer;
    if pointer_value.pointer_digest != digest_pointer(pointer_value)? {
        return Err(invalid("active_pointer.pointer_digest", "digest mismatch"));
    }
    let records = load_ledger_chain(
        state_root,
        &pointer_value.lifecycle_head_digest,
        pointer_value.generation,
    )?;
    let head = records
        .last()
        .ok_or_else(|| invalid("ledger.head", "active pointer has no ledger head"))?;
    if head.record_digest != pointer_value.lifecycle_head_digest
        || head.active_lock_digest != pointer_value.active_lock_digest
        || head.to_generation != pointer_value.generation
    {
        return Err(invalid(
            "ledger.head",
            "head record does not bind the active pointer generation and lock",
        ));
    }

    let root = generation_root(state_root, pointer_value.generation, &head.record_digest)?;
    let manifest_path = root.join("generation.yaml");
    let manifest: DomainPackGenerationManifest = parse_yaml(
        &manifest_path,
        &read_required_state_bytes(state_root, &manifest_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    if manifest.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION
        || manifest.generation != pointer_value.generation
        || manifest.record_digest != head.record_digest
        || manifest.lock_digest != pointer_value.active_lock_digest
        || manifest.preflight_digest != head.preflight_digest
        || manifest.compatibility_report_digest != head.compatibility_report_digest
    {
        return Err(invalid(
            "generation_manifest",
            "manifest does not bind pointer and ledger head",
        ));
    }

    let lock_path = root.join("lock.yaml");
    let lock: DomainPackExactLockDocument = parse_yaml(
        &lock_path,
        &read_required_state_bytes(state_root, &lock_path, DOMAIN_PACK_MAX_LOCK_BYTES)?,
    )?;
    validate_exact_lock(&lock)?;
    if lock.domain_pack_exact_lock.lock_digest != pointer_value.active_lock_digest
        || lock.domain_pack_exact_lock.payload.project_id != pointer_value.project_id
    {
        return Err(invalid(
            "active_lock",
            "generation lock differs from pointer",
        ));
    }

    let preflight_path = root.join("preflight.yaml");
    let preflight: DomainPackLifecyclePreflightDocument = parse_yaml(
        &preflight_path,
        &read_required_state_bytes(state_root, &preflight_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    validate_preflight(&preflight)?;
    let preflight_value = &preflight.domain_pack_lifecycle_preflight;
    if preflight_value.preflight_digest != manifest.preflight_digest
        || preflight_value.proposed_lock != lock
        || preflight_value.request_digest != head.request_digest
        || preflight_value
            .request
            .domain_pack_lifecycle_request
            .operation
            != head.operation
    {
        return Err(invalid(
            "generation.preflight",
            "preflight does not bind lock, request, and ledger head",
        ));
    }

    let compatibility_path = root.join("compatibility.yaml");
    let compatibility: forge_core_contracts::DomainPackCompatibilityReportDocument = parse_yaml(
        &compatibility_path,
        &read_required_state_bytes(
            state_root,
            &compatibility_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
        )?,
    )?;
    if compatibility != preflight_value.compatibility_report
        || compatibility.domain_pack_compatibility_report.report_digest
            != manifest.compatibility_report_digest
        || compatibility.domain_pack_compatibility_report.operation != head.operation
    {
        return Err(invalid(
            "generation.compatibility",
            "compatibility report does not bind preflight and ledger head",
        ));
    }

    let receipt_path = root.join("receipt.yaml");
    let receipt: DomainPackLifecycleReceiptDocument = parse_yaml(
        &receipt_path,
        &read_required_state_bytes(state_root, &receipt_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    let receipt_value = &receipt.domain_pack_lifecycle_receipt;
    if receipt_value.receipt_digest != manifest.receipt_digest
        || digest_receipt(receipt_value)? != manifest.receipt_digest
        || receipt_value.new_ledger_head_digest != head.record_digest
        || receipt_value.to_state != *pointer_value
        || receipt_value.operation != head.operation
        || receipt_value.preflight_digest != head.preflight_digest
        || receipt_value.trust_policy_digest
            != lock.domain_pack_exact_lock.payload.trust_policy_digest
        || receipt_value.reviewer_registry_digest
            != lock.domain_pack_exact_lock.payload.reviewer_registry_digest
        || receipt_value.reviewed_registry_digest
            != lock.domain_pack_exact_lock.payload.reviewed_registry_digest
        || receipt_value.capability_registry_digest
            != lock
                .domain_pack_exact_lock
                .payload
                .capability_registry_digest
        || receipt_value.sandbox_policy_digest
            != lock.domain_pack_exact_lock.payload.sandbox_policy_digest
    {
        return Err(invalid(
            "generation.receipt",
            "receipt is not the completion evidence for the active pointer",
        ));
    }

    let mut expected_objects = preflight_value
        .staged_artifacts
        .iter()
        .map(|binding| binding.raw_sha256.clone())
        .collect::<Vec<_>>();
    expected_objects.sort();
    expected_objects.dedup();
    if manifest.object_raw_digests != expected_objects {
        return Err(invalid(
            "generation.objects",
            "generation object manifest differs from staged artifacts",
        ));
    }
    for digest in &manifest.object_raw_digests {
        let token = digest_token(digest, "generation.object_raw_digest")?;
        let object_path = state_root
            .join(DOMAIN_PACK_STATE_RELATIVE_ROOT)
            .join("objects")
            .join(token);
        let bytes =
            read_required_state_bytes(state_root, &object_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
        if sha256_content_hash(&bytes) != *digest {
            return Err(invalid("generation.objects", "object digest mismatch"));
        }
    }

    // The active pointer is the commit authority. A crash after its atomic
    // flip but before publishing the receipt is repaired idempotently from the
    // immutable generation envelope.
    publish_committed_receipt(state_root, &receipt)?;
    Ok(DomainPackLifecycleStateProjection {
        active_pointer: Some(pointer),
        active_lock: Some(lock),
        ledger_records: records,
    })
}

fn load_ledger_chain(
    state_root: &Path,
    head: &str,
    expected_generation: u64,
) -> Result<Vec<DomainPackLifecycleLedgerRecord>, DomainPackLifecycleStoreError> {
    let mut reverse = Vec::new();
    let mut cursor = Some(head.to_owned());
    while let Some(digest) = cursor.take() {
        if reverse.len() >= DOMAIN_PACK_MAX_LEDGER_RECORDS {
            return Err(DomainPackLifecycleStoreError::ResourceLimit {
                resource: "ledger records",
                maximum: DOMAIN_PACK_MAX_LEDGER_RECORDS as u64,
            });
        }
        let token = digest_token(&digest, "ledger.record_digest")?;
        let path = state_root
            .join(DOMAIN_PACK_STATE_RELATIVE_ROOT)
            .join("ledger")
            .join(format!("{token}.yaml"));
        let bytes = read_required_state_bytes(state_root, &path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
        let record: DomainPackLifecycleLedgerRecord = parse_yaml(&path, &bytes)?;
        if record.record_digest != digest || digest_record(&record)? != digest {
            return Err(invalid("ledger.record_digest", "record digest mismatch"));
        }
        cursor.clone_from(&record.previous_record_digest);
        reverse.push(record);
    }
    reverse.reverse();
    for (index, record) in reverse.iter().enumerate() {
        let sequence = u64::try_from(index).unwrap_or(u64::MAX);
        if record.sequence != sequence {
            return Err(invalid("ledger.sequence", "sequence is not contiguous"));
        }
        if index == 0 && record.previous_record_digest.is_some() {
            return Err(invalid(
                "ledger.previous_record_digest",
                "genesis has predecessor",
            ));
        }
        if index > 0
            && record.previous_record_digest.as_deref()
                != Some(reverse[index - 1].record_digest.as_str())
        {
            return Err(invalid("ledger.previous_record_digest", "chain mismatch"));
        }
    }
    if reverse.last().map(|record| record.sequence) != Some(expected_generation) {
        return Err(invalid("ledger.head", "generation and ledger head differ"));
    }
    Ok(reverse)
}

fn validate_preflight(
    document: &DomainPackLifecyclePreflightDocument,
) -> Result<(), DomainPackLifecycleStoreError> {
    validate_schema(&document.schema_version, "preflight.schema_version")?;
    let body = &document.domain_pack_lifecycle_preflight;
    if body.status != DomainPackLifecyclePreflightStatus::Ready || !body.issues.is_empty() {
        return Err(DomainPackLifecycleStoreError::PreflightBlocked {
            reason: "status must be ready with zero issues".to_owned(),
        });
    }
    validate_exact_lock(&body.proposed_lock)?;
    let expected = digest_preflight(document)?;
    if body.preflight_digest != expected {
        return Err(invalid("preflight.preflight_digest", "digest mismatch"));
    }
    let report = &body.compatibility_report.domain_pack_compatibility_report;
    let operation = &body.request.domain_pack_lifecycle_request.operation;
    let historical_empty_rollback =
        matches!(operation, DomainPackLifecycleOperation::Rollback { .. })
            && body
                .proposed_lock
                .domain_pack_exact_lock
                .payload
                .packages
                .is_empty();
    let allowed = match operation {
        DomainPackLifecycleOperation::Remove { .. } => matches!(
            report.status,
            DomainPackCompatibilityStatus::Compatible | DomainPackCompatibilityStatus::Degraded
        ),
        DomainPackLifecycleOperation::Rollback { .. } if historical_empty_rollback => true,
        _ => report.status == DomainPackCompatibilityStatus::Compatible,
    };
    if !allowed || !report.universal_core_unchanged {
        return Err(DomainPackLifecycleStoreError::PreflightBlocked {
            reason: "compatibility or sealed-core invariant blocks operation".to_owned(),
        });
    }
    Ok(())
}

fn validate_exact_lock(
    document: &DomainPackExactLockDocument,
) -> Result<(), DomainPackLifecycleStoreError> {
    validate_schema(&document.schema_version, "exact_lock.schema_version")?;
    let expected = canonical_digest(&document.domain_pack_exact_lock.payload)?;
    if document.domain_pack_exact_lock.lock_digest != expected {
        return Err(invalid("exact_lock.lock_digest", "payload digest mismatch"));
    }
    Ok(())
}

fn validate_operation_intent(
    operation: &DomainPackLifecycleOperation,
    from_lock: Option<&DomainPackExactLockDocument>,
    to_lock: &DomainPackExactLockDocument,
    resolution: &forge_core_contracts::DomainPackResolutionRequest,
    rollback_target: Option<&DomainPackLifecycleReceiptDocument>,
) -> Result<(), DomainPackLifecycleStoreError> {
    let from_packages = from_lock
        .map(|lock| lock.domain_pack_exact_lock.payload.packages.as_slice())
        .unwrap_or_default();
    let to_packages = &to_lock.domain_pack_exact_lock.payload.packages;
    match operation {
        DomainPackLifecycleOperation::Install { root } => {
            if find_locked_package(from_packages, root).is_some()
                || find_locked_package(to_packages, root).is_none()
                || !resolution.roots.iter().any(|candidate| {
                    candidate.pack == *root
                        && candidate.reason
                            == forge_core_contracts::DomainPackResolutionRootReason::InstallIntent
                })
            {
                return Err(blocked(
                    "install must add an absent pack and select the exact install-intent root",
                ));
            }
        }
        DomainPackLifecycleOperation::Upgrade {
            pack,
            expected_from,
            target_requirement,
            required_content_digest,
        } => {
            let Some(previous) = find_locked_package(from_packages, pack) else {
                return Err(blocked("upgrade target is absent from the active lock"));
            };
            let Some(target) = find_locked_package(to_packages, pack) else {
                return Err(blocked("upgrade target is absent from the proposed lock"));
            };
            let root_matches = resolution.roots.iter().any(|root| {
                root.pack == *pack
                    && root.version_requirement == *target_requirement
                    && root.required_content_digest == *required_content_digest
                    && root.reason
                        == forge_core_contracts::DomainPackResolutionRootReason::UpgradeIntent
            });
            if previous.identity.version != *expected_from
                || !root_matches
                || previous.package_digest == target.package_digest
                || required_content_digest
                    .as_ref()
                    .is_some_and(|digest| target.content_binding.canonical_sha256 != *digest)
            {
                return Err(blocked(
                    "upgrade intent does not match the active version, resolution root, or target content",
                ));
            }
        }
        DomainPackLifecycleOperation::Remove { pack } => {
            if find_locked_package(from_packages, pack).is_none()
                || find_locked_package(to_packages, pack).is_some()
                || resolution.roots.iter().any(|root| root.pack == *pack)
            {
                return Err(blocked(
                    "remove must delete an active pack from both roots and proposed lock",
                ));
            }
        }
        DomainPackLifecycleOperation::Rollback {
            target_receipt_digest,
            target_lock_digest,
        } => {
            let Some(target) = rollback_target else {
                return Err(blocked("rollback lacks a reachable committed receipt"));
            };
            if target.domain_pack_lifecycle_receipt.receipt_digest != *target_receipt_digest
                || target
                    .domain_pack_lifecycle_receipt
                    .to_state
                    .active_lock_digest
                    != *target_lock_digest
                || to_lock.domain_pack_exact_lock.lock_digest != *target_lock_digest
            {
                return Err(blocked(
                    "rollback target receipt and proposed lock are not the exact historical state",
                ));
            }
        }
    }
    Ok(())
}

fn validate_reviewed_operation_transition(
    operation: &DomainPackLifecycleOperation,
    from_lock: Option<&DomainPackExactLockDocument>,
    to_packages: &[DomainPackLockedPackage],
    reviewed_join: &forge_core_decisions::DomainPackReviewedResolutionProjection,
) -> Result<(), DomainPackLifecycleStoreError> {
    if matches!(operation, DomainPackLifecycleOperation::Rollback { .. })
        && to_packages.is_empty()
        && reviewed_join.joins.is_empty()
    {
        // The exact historical remove-last lock contains no package that could
        // require semantic review. Operation-intent validation has already
        // bound this lock to a reachable immutable receipt, so the empty set is
        // vacuously eligible without creating an install/upgrade shortcut.
        return Ok(());
    }
    if !matches!(operation, DomainPackLifecycleOperation::Remove { .. }) {
        if !reviewed_join.all_selected_eligible
            || to_packages.is_empty()
            || reviewed_join
                .joins
                .iter()
                .any(|join| join.status != DomainPackReviewedResolutionJoinStatus::EligibleReviewed)
        {
            return Err(blocked(
                "install, upgrade, and rollback require every selected package to be eligible-reviewed",
            ));
        }
        return Ok(());
    }

    let Some(from_lock) = from_lock else {
        return Err(blocked("remove requires an initialized exact lock"));
    };
    let previous = &from_lock.domain_pack_exact_lock.payload.packages;
    for target in to_packages {
        let Some(prior) = previous.iter().find(|prior| {
            prior.identity == target.identity
                && prior.package_digest == target.package_digest
                && prior.registry_record_digest == target.registry_record_digest
        }) else {
            return Err(blocked(
                "remove cannot introduce a package absent from the previous lock",
            ));
        };
        if prior != target {
            return Err(blocked(
                "remove must preserve every remaining package byte-exactly",
            ));
        }
        let Some(join) = reviewed_join.joins.iter().find(|join| {
            join.publisher == target.identity.publisher.0
                && join.name == target.identity.name.0
                && join.version == target.identity.version
                && join.package_digest == target.package_digest
                && join.registry_record_digest == target.registry_record_digest
        }) else {
            return Err(blocked(
                "remaining remove package lacks an exact reviewed join",
            ));
        };
        if !matches!(
            join.status,
            DomainPackReviewedResolutionJoinStatus::EligibleReviewed
                | DomainPackReviewedResolutionJoinStatus::IneligibleDeprecated
                | DomainPackReviewedResolutionJoinStatus::IneligibleRevoked
                | DomainPackReviewedResolutionJoinStatus::IneligibleSuperseded
        ) {
            return Err(blocked(
                "remove cannot retain a package without reviewed history",
            ));
        }
    }
    Ok(())
}

fn find_locked_package<'a>(
    packages: &'a [DomainPackLockedPackage],
    pack: &forge_core_contracts::DomainPackCoordinate,
) -> Option<&'a DomainPackLockedPackage> {
    packages.iter().find(|candidate| {
        candidate.identity.publisher == pack.publisher && candidate.identity.name == pack.name
    })
}

fn verify_immutable_artifacts(
    artifacts: &[DomainPackImmutableArtifact<'_>],
    expected: &[DomainPackArtifactBinding],
) -> Result<Vec<OwnedDomainPackImmutableArtifact>, DomainPackLifecycleStoreError> {
    let expected = normalized_artifact_bindings(expected);
    let mut supplied = artifacts
        .iter()
        .map(|artifact| artifact.binding.clone())
        .collect::<Vec<_>>();
    supplied = normalized_artifact_bindings(&supplied);
    if artifacts.len() != expected.len() || supplied != expected {
        return Err(blocked(
            "immutable artifact bytes do not exactly cover staged bindings",
        ));
    }
    let mut owned = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        if u64::try_from(artifact.raw_bytes.len()).unwrap_or(u64::MAX)
            > DOMAIN_PACK_MAX_DOCUMENT_BYTES
            || sha256_content_hash(artifact.raw_bytes) != artifact.binding.raw_sha256
        {
            return Err(blocked(
                "immutable artifact bytes exceed bounds or differ from raw binding",
            ));
        }
        let text = std::str::from_utf8(artifact.raw_bytes)
            .map_err(|_| blocked("immutable artifact is not UTF-8 YAML"))?;
        let semantic: serde_json::Value = yaml_serde::from_str(text)
            .map_err(|_| blocked("immutable artifact is not valid bounded YAML"))?;
        if canonical_digest(&semantic)? != artifact.binding.canonical_sha256 {
            return Err(blocked(
                "immutable artifact canonical semantics differ from staged binding",
            ));
        }
        owned.push(OwnedDomainPackImmutableArtifact {
            binding: artifact.binding.clone(),
            raw_bytes: artifact.raw_bytes.to_vec(),
        });
    }
    owned.sort_by(|left, right| {
        left.binding
            .artifact_ref
            .0
            .cmp(&right.binding.artifact_ref.0)
    });
    Ok(owned)
}

fn load_committed_receipt(
    state_root: &Path,
    state: &DomainPackLifecycleStateProjection,
    receipt_digest: &str,
    target_lock_digest: &str,
) -> Result<DomainPackLifecycleReceiptDocument, DomainPackLifecycleStoreError> {
    let token = digest_token(receipt_digest, "rollback.target_receipt_digest")?;
    let path = state_root
        .join(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("receipts")
        .join(format!("{token}.yaml"));
    let receipt: DomainPackLifecycleReceiptDocument = parse_yaml(
        &path,
        &read_required_state_bytes(state_root, &path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    let value = &receipt.domain_pack_lifecycle_receipt;
    let historical_record = state.ledger_records.iter().find(|record| {
        record.record_digest == value.new_ledger_head_digest
            && record.active_lock_digest == target_lock_digest
            && record.to_generation == value.to_state.generation
    });
    if value.receipt_digest != receipt_digest
        || digest_receipt(value)? != receipt_digest
        || digest_pointer(&value.to_state)? != value.to_state.pointer_digest
        || value.to_state.lifecycle_head_digest != value.new_ledger_head_digest
        || value.to_state.active_lock_digest != target_lock_digest
        || historical_record.is_none()
        || state.active_pointer.as_ref().is_some_and(|active| {
            active.domain_pack_active_pointer.pointer_digest == value.to_state.pointer_digest
        })
    {
        return Err(blocked(
            "rollback receipt is invalid or unreachable from committed history",
        ));
    }
    let historical_record = historical_record.expect("checked above");
    if historical_record.operation != value.operation
        || historical_record.preflight_digest != value.preflight_digest
        || historical_record.request_digest != value.request_digest
        || historical_record.compatibility_report_digest != value.compatibility_report_digest
    {
        return Err(blocked(
            "rollback receipt fields differ from their reachable ledger record",
        ));
    }
    let generation = generation_root(
        state_root,
        historical_record.to_generation,
        &historical_record.record_digest,
    )?;
    let canonical_path = generation.join("receipt.yaml");
    let canonical: DomainPackLifecycleReceiptDocument = parse_yaml(
        &canonical_path,
        &read_required_state_bytes(state_root, &canonical_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    if canonical != receipt {
        return Err(blocked(
            "rollback receipt differs from the exact immutable historical generation",
        ));
    }
    Ok(receipt)
}

fn verify_expected_state(
    expected: &DomainPackExpectedLifecycleState,
    actual: &DomainPackLifecycleStateProjection,
) -> Result<(), DomainPackLifecycleStoreError> {
    let matches = match (expected, actual.active_pointer.as_ref()) {
        (DomainPackExpectedLifecycleState::Uninitialized { .. }, None) => true,
        (
            DomainPackExpectedLifecycleState::Initialized {
                generation,
                active_lock_digest,
                lifecycle_head_digest,
                ..
            },
            Some(pointer),
        ) => {
            let value = &pointer.domain_pack_active_pointer;
            value.generation == *generation
                && value.active_lock_digest == *active_lock_digest
                && value.lifecycle_head_digest == *lifecycle_head_digest
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(stale(&format!("{expected:?}"), actual))
    }
}

fn staged_digests(body: &forge_core_contracts::DomainPackLifecyclePreflight) -> Vec<String> {
    let mut values = body
        .staged_artifacts
        .iter()
        .flat_map(|binding| [binding.raw_sha256.clone(), binding.canonical_sha256.clone()])
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn normalized_artifact_bindings(
    bindings: &[DomainPackArtifactBinding],
) -> Vec<DomainPackArtifactBinding> {
    let mut normalized = bindings.to_vec();
    normalized.sort_by(|left, right| {
        left.artifact_ref
            .0
            .cmp(&right.artifact_ref.0)
            .then(left.raw_sha256.cmp(&right.raw_sha256))
            .then(left.canonical_sha256.cmp(&right.canonical_sha256))
    });
    normalized.dedup();
    normalized
}

fn expected_staged_artifact_bindings(
    packages: &[DomainPackLockedPackage],
) -> Vec<DomainPackArtifactBinding> {
    let bindings = packages
        .iter()
        .flat_map(|package| {
            let content = DomainPackArtifactBinding {
                artifact_ref: package.content_binding.content_ref.clone(),
                raw_sha256: package.content_binding.raw_sha256.clone(),
                canonical_sha256: package.content_binding.canonical_sha256.clone(),
            };
            std::iter::once(package.manifest_binding.clone())
                .chain(std::iter::once(content))
                .chain(std::iter::once(package.license_binding.clone()))
                .chain(package.fixture_bindings.iter().cloned())
        })
        .collect::<Vec<_>>();
    normalized_artifact_bindings(&bindings)
}

fn expected_capability_demands(
    package: &forge_core_contracts::DomainPackResolvedPackage,
    composition: &forge_core_contracts::DomainPackCompositionRequest,
) -> Result<Vec<DomainPackCapabilityDemand>, DomainPackLifecycleStoreError> {
    let candidate = composition
        .candidates
        .iter()
        .find(|candidate| {
            let pack = &candidate.content.domain_pack_content.pack;
            pack.publisher == package.identity.publisher
                && pack.name == package.identity.name
                && pack.version == package.identity.version
        })
        .ok_or_else(|| blocked("resolved package has no exact raw composition candidate"))?;
    let content = &candidate.content.domain_pack_content;
    let capability_kinds = content
        .provided_capabilities
        .iter()
        .map(|capability| (capability.id.0.as_str(), capability.kind))
        .collect::<BTreeMap<_, _>>();
    let mut demands = Vec::new();
    let mut add = |subject_ref: &StableId,
                   capability_ref: &StableId|
     -> Result<(), DomainPackLifecycleStoreError> {
        let Some(kind) = capability_kinds.get(capability_ref.0.as_str()).copied() else {
            return Err(blocked(
                "raw package semantics demand an undeclared capability",
            ));
        };
        demands.push(DomainPackCapabilityDemand {
            subject_ref: subject_ref.clone(),
            capability_ref: capability_ref.clone(),
            kind,
        });
        Ok(())
    };
    // Workflow capability requirements are abstract governance probes and do
    // not carry a Domain Pack `capability_ref`; treating their own ids as
    // runtime bindings would fabricate a cross-vocabulary mapping. P6b derives
    // executable demands only from fields with explicit capability refs.
    for lifecycle in &content.lifecycle_models {
        for transition in &lifecycle.transitions {
            for capability_ref in &transition.required_capability_refs {
                add(&transition.id, capability_ref)?;
            }
        }
    }
    for adapter in &content.adapters {
        for capability_ref in &adapter.required_capability_refs {
            add(&adapter.id, capability_ref)?;
        }
    }
    for requirement in &composition.requirements.required_domains {
        for capability_ref in &requirement.required_capability_refs {
            if capability_kinds.contains_key(capability_ref.0.as_str()) {
                add(&requirement.id, capability_ref)?;
            }
        }
    }
    Ok(normalized_capability_demands(&demands))
}

fn normalized_capability_demands(
    demands: &[DomainPackCapabilityDemand],
) -> Vec<DomainPackCapabilityDemand> {
    let mut normalized = demands.to_vec();
    normalized.sort_by(|left, right| {
        left.subject_ref
            .0
            .cmp(&right.subject_ref.0)
            .then(left.capability_ref.0.cmp(&right.capability_ref.0))
            .then(format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
    });
    normalized.dedup();
    normalized
}

fn digest_preflight(
    document: &DomainPackLifecyclePreflightDocument,
) -> Result<String, DomainPackLifecycleStoreError> {
    let mut subject = document.clone();
    subject
        .domain_pack_lifecycle_preflight
        .preflight_digest
        .clear();
    canonical_digest(&subject)
}

fn digest_pointer(
    pointer: &DomainPackActivePointer,
) -> Result<String, DomainPackLifecycleStoreError> {
    let mut subject = pointer.clone();
    subject.pointer_digest.clear();
    canonical_digest(&subject)
}

fn digest_record(
    record: &DomainPackLifecycleLedgerRecord,
) -> Result<String, DomainPackLifecycleStoreError> {
    let mut subject = record.clone();
    subject.record_digest.clear();
    canonical_digest(&subject)
}

fn digest_receipt(
    receipt: &DomainPackLifecycleReceipt,
) -> Result<String, DomainPackLifecycleStoreError> {
    let mut subject = receipt.clone();
    subject.receipt_digest.clear();
    canonical_digest(&subject)
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, DomainPackLifecycleStoreError> {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| invalid("canonicalization", &error.to_string()))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn yaml_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, DomainPackLifecycleStoreError> {
    yaml_serde::to_string(value)
        .map(String::into_bytes)
        .map_err(|error| invalid("serialization", &error.to_string()))
}

fn parse_yaml<T: serde::de::DeserializeOwned>(
    path: &Path,
    bytes: &[u8],
) -> Result<T, DomainPackLifecycleStoreError> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        DomainPackLifecycleStoreError::InvalidDocument {
            path: path.to_path_buf(),
            reason: error.to_string(),
        }
    })?;
    yaml_serde::from_str(text).map_err(|error| DomainPackLifecycleStoreError::InvalidDocument {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })
}

fn write_immutable(path: &Path, content: &[u8]) -> Result<(), DomainPackLifecycleStoreError> {
    if u64::try_from(content.len()).unwrap_or(u64::MAX) > DOMAIN_PACK_MAX_DOCUMENT_BYTES {
        return Err(DomainPackLifecycleStoreError::ResourceLimit {
            resource: "immutable document bytes",
            maximum: DOMAIN_PACK_MAX_DOCUMENT_BYTES,
        });
    }
    let parent = path
        .parent()
        .ok_or_else(|| invalid("path", "missing parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(content)
                .map_err(|error| io_error(path, error))?;
            file.sync_all().map_err(|error| io_error(path, error))?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = read_required_bytes(path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
            if existing == content {
                Ok(())
            } else {
                Err(DomainPackLifecycleStoreError::InvalidDocument {
                    path: path.to_path_buf(),
                    reason: "content-addressed collision with different bytes".to_owned(),
                })
            }
        }
        Err(error) => Err(io_error(path, error)),
    }
}

fn write_immutable_under_root(
    state_root: &Path,
    path: &Path,
    content: &[u8],
) -> Result<(), DomainPackLifecycleStoreError> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("path", "missing parent"))?;
    ensure_secure_directory(state_root, parent)?;
    assert_confined_state_path(state_root, path)?;
    write_immutable(path, content)
}

fn ensure_secure_directory(
    state_root: &Path,
    directory: &Path,
) -> Result<(), DomainPackLifecycleStoreError> {
    let relative = directory
        .strip_prefix(state_root)
        .map_err(|_| invalid("state_path", "directory escapes canonical state root"))?;
    let mut current = state_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(invalid(
                "state_path",
                "directory is not a normalized child path",
            ));
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(DomainPackLifecycleStoreError::InvalidDocument {
                        path: current,
                        reason: "state directory is a link/reparse point or non-directory"
                            .to_owned(),
                    });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|error| io_error(&current, error))?;
                let metadata =
                    fs::symlink_metadata(&current).map_err(|error| io_error(&current, error))?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(invalid(
                        "state_path",
                        "created directory became a link or non-directory",
                    ));
                }
            }
            Err(error) => return Err(io_error(&current, error)),
        }
        let canonical = fs::canonicalize(&current).map_err(|error| io_error(&current, error))?;
        if !canonical.starts_with(state_root) {
            return Err(DomainPackLifecycleStoreError::InvalidDocument {
                path: canonical,
                reason: "state directory escapes canonical state root".to_owned(),
            });
        }
    }
    Ok(())
}

fn assert_confined_state_path(
    state_root: &Path,
    path: &Path,
) -> Result<(), DomainPackLifecycleStoreError> {
    let relative = path
        .strip_prefix(state_root)
        .map_err(|_| invalid("state_path", "path escapes canonical state root"))?;
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid("state_path", "path is not a normalized child path"));
    }
    if let Some(parent) = path.parent() {
        ensure_secure_directory(state_root, parent)?;
    }
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(DomainPackLifecycleStoreError::InvalidDocument {
                path: path.to_path_buf(),
                reason: "state file is a link/reparse point".to_owned(),
            });
        }
    }
    Ok(())
}

fn read_optional_state_bytes(
    state_root: &Path,
    path: &Path,
    maximum: u64,
) -> Result<Option<Vec<u8>>, DomainPackLifecycleStoreError> {
    assert_confined_state_path(state_root, path)?;
    read_optional_bytes(path, maximum)
}

fn read_required_state_bytes(
    state_root: &Path,
    path: &Path,
    maximum: u64,
) -> Result<Vec<u8>, DomainPackLifecycleStoreError> {
    read_optional_state_bytes(state_root, path, maximum)?.ok_or_else(|| {
        DomainPackLifecycleStoreError::InvalidDocument {
            path: path.to_path_buf(),
            reason: "required file is missing".to_owned(),
        }
    })
}

fn read_optional_bytes(
    path: &Path,
    maximum: u64,
) -> Result<Option<Vec<u8>>, DomainPackLifecycleStoreError> {
    let file = match OpenOptions::new().read(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(path, error)),
    };
    let metadata = file.metadata().map_err(|error| io_error(path, error))?;
    if !metadata.is_file() {
        return Err(DomainPackLifecycleStoreError::InvalidDocument {
            path: path.to_path_buf(),
            reason: "expected regular file".to_owned(),
        });
    }
    if metadata.len() > maximum {
        return Err(DomainPackLifecycleStoreError::ResourceLimit {
            resource: "document bytes",
            maximum,
        });
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(DomainPackLifecycleStoreError::ResourceLimit {
            resource: "document bytes",
            maximum,
        });
    }
    Ok(Some(bytes))
}

fn read_required_bytes(
    path: &Path,
    maximum: u64,
) -> Result<Vec<u8>, DomainPackLifecycleStoreError> {
    read_optional_bytes(path, maximum)?.ok_or_else(|| {
        DomainPackLifecycleStoreError::InvalidDocument {
            path: path.to_path_buf(),
            reason: "required file is missing".to_owned(),
        }
    })
}

fn project_snapshot_digest(root: &Path) -> Result<String, DomainPackLifecycleStoreError> {
    let mut stack = vec![root.to_path_buf()];
    let mut entries = Vec::new();
    let mut files = 0usize;
    let mut bytes_total = 0u64;
    while let Some(directory) = stack.pop() {
        let mut children = fs::read_dir(&directory)
            .map_err(|error| io_error(&directory, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| io_error(&directory, error))?;
        children.sort_by_key(std::fs::DirEntry::file_name);
        for child in children.into_iter().rev() {
            let path = child.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| invalid("project_snapshot", "path escaped project root"))?;
            let top_level = relative
                .components()
                .next()
                .and_then(|component| component.as_os_str().to_str())
                .unwrap_or_default();
            if matches!(
                top_level,
                ".git" | ".forge-method" | "target" | "node_modules"
            ) {
                continue;
            }
            let metadata = fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
            if metadata.file_type().is_symlink() {
                let target = fs::read_link(&path).map_err(|error| io_error(&path, error))?;
                entries.push((
                    relative.to_string_lossy().replace('\\', "/"),
                    format!("symlink:{}", target.display()),
                ));
            } else if metadata.is_dir() {
                let canonical = fs::canonicalize(&path).map_err(|error| io_error(&path, error))?;
                if !canonical.starts_with(root) {
                    return Err(DomainPackLifecycleStoreError::InvalidDocument {
                        path: canonical,
                        reason: "project snapshot directory escapes canonical root".to_owned(),
                    });
                }
                stack.push(path);
            } else if metadata.is_file() {
                files = files.saturating_add(1);
                bytes_total = bytes_total.saturating_add(metadata.len());
                if files > DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_FILES {
                    return Err(DomainPackLifecycleStoreError::ResourceLimit {
                        resource: "project snapshot files",
                        maximum: DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_FILES as u64,
                    });
                }
                if bytes_total > DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_BYTES {
                    return Err(DomainPackLifecycleStoreError::ResourceLimit {
                        resource: "project snapshot bytes",
                        maximum: DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_BYTES,
                    });
                }
                let mut file = OpenOptions::new()
                    .read(true)
                    .open(&path)
                    .map_err(|error| io_error(&path, error))?;
                let opened = file.metadata().map_err(|error| io_error(&path, error))?;
                if !opened.is_file() || opened.len() != metadata.len() {
                    return Err(stale_project_snapshot(
                        "stable file handle",
                        "project file changed while opening the snapshot",
                    ));
                }
                let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
                std::io::Read::by_ref(&mut file)
                    .take(opened.len().saturating_add(1))
                    .read_to_end(&mut bytes)
                    .map_err(|error| io_error(&path, error))?;
                if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != opened.len() {
                    return Err(stale_project_snapshot(
                        "stable file bytes",
                        "project file changed while hashing the snapshot",
                    ));
                }
                entries.push((
                    relative.to_string_lossy().replace('\\', "/"),
                    sha256_content_hash(&bytes),
                ));
            } else {
                return Err(DomainPackLifecycleStoreError::InvalidDocument {
                    path,
                    reason: "project snapshot contains a special filesystem object".to_owned(),
                });
            }
        }
    }
    entries.sort();
    canonical_digest(&entries)
}

fn validate_supply_chain_freshness(
    verified_at_unix: u64,
    expires_at_unix: u64,
) -> Result<(), DomainPackLifecycleStoreError> {
    let now_unix = trusted_now_unix()?;
    if verified_at_unix > now_unix.saturating_add(DOMAIN_PACK_MAX_CLOCK_FUTURE_SKEW_SECONDS) {
        return Err(blocked("supply-chain verification time is in the future"));
    }
    if now_unix.saturating_sub(verified_at_unix)
        > DOMAIN_PACK_MAX_SUPPLY_CHAIN_VERIFICATION_AGE_SECONDS
    {
        return Err(blocked("supply-chain verification is stale"));
    }
    if now_unix > expires_at_unix {
        return Err(blocked("verified supply-chain snapshot has expired"));
    }
    Ok(())
}

fn trusted_now_unix() -> Result<u64, DomainPackLifecycleStoreError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("system_clock", "time is before Unix epoch"))
        .map(|duration| duration.as_secs())
}

fn canonical_state_root(path: &Path) -> Result<PathBuf, DomainPackLifecycleStoreError> {
    let canonical = fs::canonicalize(path).map_err(|error| io_error(path, error))?;
    if !canonical.is_dir()
        || canonical
            .file_name()
            .is_none_or(|name| name != std::ffi::OsStr::new(".forge-method"))
    {
        return Err(invalid(
            "state_root",
            "must be an existing canonical .forge-method directory",
        ));
    }
    Ok(canonical)
}

fn digest_token<'a>(
    digest: &'a str,
    field: &'static str,
) -> Result<&'a str, DomainPackLifecycleStoreError> {
    let Some(token) = digest.strip_prefix("sha256:") else {
        return Err(DomainPackLifecycleStoreError::InvalidDigest {
            field,
            value: digest.to_owned(),
        });
    };
    if token.len() != 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DomainPackLifecycleStoreError::InvalidDigest {
            field,
            value: digest.to_owned(),
        });
    }
    Ok(token)
}

fn validate_schema(value: &str, field: &'static str) -> Result<(), DomainPackLifecycleStoreError> {
    if value == DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(DomainPackLifecycleStoreError::InvalidArgument {
            field,
            reason: format!("expected {DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION}, found {value}"),
        })
    }
}

fn stale(
    expected: &str,
    actual: &DomainPackLifecycleStateProjection,
) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::StaleExpectedState {
        expected: expected.to_owned(),
        actual: format!("{:?}", actual.active_pointer),
    }
}

fn stale_project_snapshot(expected: &str, actual: &str) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::StaleExpectedState {
        expected: expected.to_owned(),
        actual: actual.to_owned(),
    }
}

fn invalid(field: &'static str, reason: &str) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::InvalidArgument {
        field,
        reason: reason.to_owned(),
    }
}

fn blocked(reason: &str) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::PreflightBlocked {
        reason: reason.to_owned(),
    }
}

#[allow(clippy::needless_pass_by_value)] // Call sites transfer the terminal I/O error into text.
fn io_error(path: &Path, error: io::Error) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::Io {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

#[allow(dead_code)]
fn is_safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}
