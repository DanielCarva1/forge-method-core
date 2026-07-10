//! Explicit P4b.3c activation and the trusted single-effect MCP executor.

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
use forge_core_store::replay_wal::recover_replay_wal;
use serde::Serialize;

use crate::{McpDeploymentActivationState, TrustedMcpMaterialLoader, ValidatedMcpDeploymentPolicy};

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

/// Proof that the configured root was reconciled after explicit opt-in.
#[derive(Debug, Clone)]
pub struct ReconciledTrustedMcpDeployment {
    policy: ValidatedMcpDeploymentPolicy,
    environment: TrustedExecutionEnvironment,
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
        _explicit_opt_in: ExplicitTrustedSingleEffectOptIn,
    ) -> Result<Self, TrustedMcpActivationError> {
        if policy.activation_state() != McpDeploymentActivationState::PolicyValidatedDormant {
            return Err(TrustedMcpActivationError::TrustedPolicyRequired);
        }
        let policy_contract = &policy.document().mcp_deployment_policy;
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
        let audit = TrustedMcpActivationAudit {
            policy_id: policy_contract.id.0.clone(),
            canonical_project_root: environment.project_root().to_path_buf(),
            audience: audience.to_owned(),
            replay_records_before: replay_before.valid_record_count,
            replay_records_after: replay_after.valid_record_count,
            reconciliation,
        };
        Ok(Self {
            policy,
            environment,
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
    pub const fn audit(&self) -> &TrustedMcpActivationAudit {
        &self.audit
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedMcpActivationAudit {
    pub policy_id: String,
    pub canonical_project_root: PathBuf,
    pub audience: String,
    pub replay_records_before: usize,
    pub replay_records_after: usize,
    pub reconciliation: ExecutionReplayReconciliationResult,
}

/// Executor available only after [`ReconciledTrustedMcpDeployment`] exists.
#[derive(Debug)]
pub struct TrustedSingleEffectMcpExecutor {
    loader: TrustedMcpMaterialLoader,
    activation: Arc<ReconciledTrustedMcpDeployment>,
}

impl TrustedSingleEffectMcpExecutor {
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

impl ExecutionExecutor for TrustedSingleEffectMcpExecutor {
    fn execute(&self, call: VerifiedExecutionCall) -> Result<ExecutionResult, ExecutionError> {
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
    Environment(String),
    Replay(String),
    Reconciliation(String),
    PolicyMismatch,
    RootMismatch,
}

impl fmt::Display for TrustedMcpActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrustedPolicyRequired => {
                formatter.write_str("validated dormant trusted policy required")
            }
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
