//! Explicit trusted MCP activation and operation-scoped execution.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use forge_core_authority::{
    ExecutionError, ExecutionExecutor, ExecutionResult, ExecutionStatus, VerifiedExecutionCall,
};
use forge_core_kernel::{
    prepare_execution_transaction, reconcile_prepared_execution_commits, ExecutionCommitOutcome,
    ExecutionCommitStatus, ExecutionReplayReconciliationResult, LateAdmissionOutcome,
    TrustedExecutionEnvironment,
};
use forge_core_store::replay_anchor::advance_replay_anchor_for_deployment;
use forge_core_store::replay_wal::recover_replay_wal;
use serde::Serialize;

use crate::{
    EffectScopePolicy, McpDeploymentActivationState, TrustedMcpMaterialLoader,
    ValidatedMcpDeploymentPolicy,
};

/// Deliberate operator signal required in addition to a trusted policy file.
#[derive(Debug)]
pub struct ExplicitTrustedSingleEffectOptIn(());

impl ExplicitTrustedSingleEffectOptIn {
    /// Construct only in response to an explicit operator-facing enable flag.
    #[must_use]
    pub const fn from_operator_flag() -> Self {
        Self(())
    }
}

/// Deliberate operator signal for operation-wide trusted mutation.
#[derive(Debug)]
pub struct ExplicitTrustedOperationWideOptIn(());

impl ExplicitTrustedOperationWideOptIn {
    /// Construct only in response to the dedicated operator-facing enable flag.
    #[must_use]
    pub const fn from_operator_flag() -> Self {
        Self(())
    }
}

/// Proof that the configured root was reconciled after explicit opt-in.
#[derive(Debug, Clone)]
pub struct ReconciledTrustedMcpDeployment {
    policy: ValidatedMcpDeploymentPolicy,
    environment: TrustedExecutionEnvironment,
    replay_anchor_path: PathBuf,
    audit: TrustedMcpActivationAudit,
}

impl ReconciledTrustedMcpDeployment {
    /// Verify replay state and reconcile every incomplete prepared execution.
    ///
    /// # Errors
    ///
    /// Fails closed unless the policy is trusted/dormant, the root and replay
    /// WAL are provisioned, and P4b.2c reconciliation completes successfully.
    pub fn reconcile(
        policy: ValidatedMcpDeploymentPolicy,
        project_root: impl AsRef<Path>,
        state_root: impl AsRef<Path>,
        replay_anchor_path: impl AsRef<Path>,
        _explicit_opt_in: ExplicitTrustedSingleEffectOptIn,
    ) -> Result<Self, TrustedMcpActivationError> {
        Self::reconcile_for_scope(
            policy,
            project_root,
            state_root,
            replay_anchor_path,
            EffectScopePolicy::SingleEffect,
        )
    }

    /// Reconcile an explicitly enabled operation-wide trusted deployment.
    ///
    /// # Errors
    ///
    /// Fails closed under the same conditions as [`Self::reconcile`] and when
    /// the validated policy is not operation-wide.
    pub fn reconcile_operation_wide(
        policy: ValidatedMcpDeploymentPolicy,
        project_root: impl AsRef<Path>,
        state_root: impl AsRef<Path>,
        replay_anchor_path: impl AsRef<Path>,
        _explicit_opt_in: ExplicitTrustedOperationWideOptIn,
    ) -> Result<Self, TrustedMcpActivationError> {
        Self::reconcile_for_scope(
            policy,
            project_root,
            state_root,
            replay_anchor_path,
            EffectScopePolicy::OperationWide,
        )
    }

    fn reconcile_for_scope(
        policy: ValidatedMcpDeploymentPolicy,
        project_root: impl AsRef<Path>,
        state_root: impl AsRef<Path>,
        replay_anchor_path: impl AsRef<Path>,
        expected_scope: EffectScopePolicy,
    ) -> Result<Self, TrustedMcpActivationError> {
        if policy.activation_state() != McpDeploymentActivationState::PolicyValidatedDormant {
            return Err(TrustedMcpActivationError::TrustedPolicyRequired);
        }
        let policy_contract = &policy.document().mcp_deployment_policy;
        if policy_contract.effect_scope != expected_scope {
            return Err(TrustedMcpActivationError::EffectScopeOptInMismatch {
                policy: policy_contract.effect_scope,
                opt_in: expected_scope,
            });
        }
        let audience = policy_contract
            .required_audience
            .as_deref()
            .ok_or(TrustedMcpActivationError::TrustedPolicyRequired)?;
        let environment = TrustedExecutionEnvironment::from_project_and_state_roots(
            project_root,
            state_root,
            audience,
        )
        .map_err(|error| TrustedMcpActivationError::Environment(error.to_string()))?;
        let anchor_before = advance_replay_anchor_for_deployment(
            environment.state_root(),
            replay_anchor_path,
            &policy_contract.id.0,
        )
        .map_err(|error| TrustedMcpActivationError::ReplayAnchor(error.to_string()))?;
        let replay_anchor_path = anchor_before.anchor_path.clone();
        let replay_before = recover_replay_wal(environment.state_root(), true)
            .map_err(|error| TrustedMcpActivationError::Replay(error.to_string()))?;
        if !replay_before.is_clean() {
            return Err(TrustedMcpActivationError::Replay(format!(
                "replay WAL stopped at {:?}",
                replay_before.stop_reason
            )));
        }
        let reconciliation = reconcile_prepared_execution_commits(&environment)
            .map_err(|error| TrustedMcpActivationError::Reconciliation(error.to_string()))?;
        let replay_after = recover_replay_wal(environment.state_root(), false)
            .map_err(|error| TrustedMcpActivationError::Replay(error.to_string()))?;
        if !replay_after.is_clean() {
            return Err(TrustedMcpActivationError::Replay(format!(
                "post-reconciliation replay WAL stopped at {:?}",
                replay_after.stop_reason
            )));
        }
        let anchor_after = advance_replay_anchor_for_deployment(
            environment.state_root(),
            &replay_anchor_path,
            &policy_contract.id.0,
        )
        .map_err(|error| TrustedMcpActivationError::ReplayAnchor(error.to_string()))?;
        let audit = TrustedMcpActivationAudit {
            policy_id: policy_contract.id.0.clone(),
            effect_scope: policy_contract.effect_scope,
            canonical_project_root: environment.project_root().to_path_buf(),
            audience: audience.to_owned(),
            replay_records_before: replay_before.valid_record_count,
            replay_records_after: replay_after.valid_record_count,
            replay_anchor_path: replay_anchor_path.clone(),
            replay_anchor_generation_before: anchor_before.anchor.generation,
            replay_anchor_generation_after: anchor_after.anchor.generation,
            replay_anchor_advanced: anchor_before.changed || anchor_after.changed,
            reconciliation,
        };
        Ok(Self {
            policy,
            environment,
            replay_anchor_path,
            audit,
        })
    }

    #[must_use]
    pub const fn policy(&self) -> &ValidatedMcpDeploymentPolicy {
        &self.policy
    }

    #[must_use]
    pub const fn environment(&self) -> &TrustedExecutionEnvironment {
        &self.environment
    }

    #[must_use]
    pub fn replay_anchor_path(&self) -> &Path {
        &self.replay_anchor_path
    }

    #[must_use]
    pub const fn audit(&self) -> &TrustedMcpActivationAudit {
        &self.audit
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedMcpActivationAudit {
    pub policy_id: String,
    pub effect_scope: EffectScopePolicy,
    pub canonical_project_root: PathBuf,
    pub audience: String,
    pub replay_records_before: usize,
    pub replay_records_after: usize,
    pub replay_anchor_path: PathBuf,
    pub replay_anchor_generation_before: u64,
    pub replay_anchor_generation_after: u64,
    pub replay_anchor_advanced: bool,
    pub reconciliation: ExecutionReplayReconciliationResult,
}

/// Executor available only after [`ReconciledTrustedMcpDeployment`] exists.
#[derive(Debug)]
pub struct TrustedMcpExecutor {
    loader: TrustedMcpMaterialLoader,
    activation: Arc<ReconciledTrustedMcpDeployment>,
}

impl TrustedMcpExecutor {
    /// Bind a loader to the exact reconciled root and policy.
    ///
    /// # Errors
    ///
    /// Rejects policy or canonical-root drift between loading and activation.
    pub fn new(
        loader: TrustedMcpMaterialLoader,
        activation: Arc<ReconciledTrustedMcpDeployment>,
    ) -> Result<Self, TrustedMcpActivationError> {
        if loader.policy().document() != activation.policy().document() {
            return Err(TrustedMcpActivationError::PolicyMismatch);
        }
        if loader.project_root() != activation.environment().project_root() {
            return Err(TrustedMcpActivationError::RootMismatch);
        }
        Ok(Self { loader, activation })
    }
}

impl ExecutionExecutor for TrustedMcpExecutor {
    fn execute(&self, call: VerifiedExecutionCall) -> Result<ExecutionResult, ExecutionError> {
        advance_replay_anchor_for_deployment(
            self.activation.environment().state_root(),
            self.activation.replay_anchor_path(),
            &self.activation.audit().policy_id,
        )
        .map_err(|error| {
            ExecutionError::Rejected(format!("external replay anchor rejected call: {error}"))
        })?;
        let execution = self.execute_without_anchor(call);
        let anchored = advance_replay_anchor_for_deployment(
            self.activation.environment().state_root(),
            self.activation.replay_anchor_path(),
            &self.activation.audit().policy_id,
        );
        match (execution, anchored) {
            (Ok(result), Ok(_)) => Ok(result),
            (Ok(result), Err(error)) => Ok(ExecutionResult::new(
                ExecutionStatus::RecoveryRequired,
                serde_json::json!({
                    "stage": "external_replay_anchor",
                    "execution_status": format!("{:?}", result.status()).to_ascii_lowercase(),
                    "execution_result": result.payload(),
                    "anchor_error": error.to_string(),
                }),
            )),
            (Err(error), Ok(_)) => Err(error),
            (Err(error), Err(anchor_error)) => Err(ExecutionError::Internal(format!(
                "{error}; external replay anchor also requires recovery: {anchor_error}"
            ))),
        }
    }
}

impl TrustedMcpExecutor {
    fn execute_without_anchor(
        &self,
        call: VerifiedExecutionCall,
    ) -> Result<ExecutionResult, ExecutionError> {
        let loaded = self
            .loader
            .load(call)
            .map_err(|error| ExecutionError::Rejected(error.to_string()))?;
        let prepared = prepare_execution_transaction(
            loaded.into_kernel_material(),
            self.activation.environment().clone(),
        )
        .map_err(|error| ExecutionError::Rejected(error.to_string()))?;
        let admitted = match prepared
            .evaluate_late(self.loader.snapshot_source())
            .map_err(|error| ExecutionError::Rejected(error.to_string()))?
        {
            LateAdmissionOutcome::Admitted(admitted) => admitted,
            LateAdmissionOutcome::Blocked {
                decision,
                final_preflight,
            } => {
                return Ok(ExecutionResult::new(
                    ExecutionStatus::Blocked,
                    serde_json::json!({
                        "stage": "late_admission",
                        "decision": decision,
                        "final_preflight": final_preflight,
                    }),
                ));
            }
            _ => {
                return Err(ExecutionError::Internal(
                    "unsupported late Admission outcome".to_owned(),
                ));
            }
        };
        let outcome = admitted
            .commit(self.loader.snapshot_source())
            .map_err(|error| ExecutionError::Internal(error.to_string()))?;
        let status = commit_execution_status(&outcome);
        let payload = serde_json::to_value(&outcome)
            .map_err(|error| ExecutionError::Internal(error.to_string()))?;
        Ok(ExecutionResult::new(status, payload))
    }
}

/// Backward-compatible name for the original single-effect activation path.
pub type TrustedSingleEffectMcpExecutor = TrustedMcpExecutor;

/// Executor name used by operation-wide trusted deployments.
pub type TrustedOperationWideMcpExecutor = TrustedMcpExecutor;

fn commit_execution_status(outcome: &ExecutionCommitOutcome) -> ExecutionStatus {
    match outcome {
        ExecutionCommitOutcome::Committed { receipt } => match receipt.status {
            ExecutionCommitStatus::Committed => ExecutionStatus::Applied,
            ExecutionCommitStatus::EffectCommittedReplayPending
            | ExecutionCommitStatus::EffectCommittedCompletionPending => {
                ExecutionStatus::RecoveryRequired
            }
            _ => ExecutionStatus::RecoveryRequired,
        },
        ExecutionCommitOutcome::NotCommitted { .. } | ExecutionCommitOutcome::Blocked { .. } => {
            ExecutionStatus::Blocked
        }
        _ => ExecutionStatus::Blocked,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustedMcpActivationError {
    TrustedPolicyRequired,
    EffectScopeOptInMismatch {
        policy: EffectScopePolicy,
        opt_in: EffectScopePolicy,
    },
    Environment(String),
    Replay(String),
    Reconciliation(String),
    ReplayAnchor(String),
    PolicyMismatch,
    RootMismatch,
}

impl fmt::Display for TrustedMcpActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrustedPolicyRequired => {
                formatter.write_str("validated dormant trusted policy required")
            }
            Self::EffectScopeOptInMismatch { policy, opt_in } => write!(
                formatter,
                "trusted policy effect scope {policy:?} does not match explicit {opt_in:?} opt-in"
            ),
            Self::Environment(source) => write!(formatter, "trusted environment failed: {source}"),
            Self::Replay(source) => {
                write!(formatter, "startup replay verification failed: {source}")
            }
            Self::Reconciliation(source) => {
                write!(
                    formatter,
                    "startup execution reconciliation failed: {source}"
                )
            }
            Self::ReplayAnchor(source) => {
                write!(formatter, "external replay anchor failed: {source}")
            }
            Self::PolicyMismatch => {
                formatter.write_str("trusted loader policy differs from reconciled policy")
            }
            Self::RootMismatch => {
                formatter.write_str("trusted loader root differs from reconciled root")
            }
        }
    }
}

impl std::error::Error for TrustedMcpActivationError {}
