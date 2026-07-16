//! Versioned bootstrap state-loss diagnosis and non-conflated recovery choices.
//!
//! The types in this module are diagnostic output, not authorization to mutate.
//! Restore and reinitialize-as-new remain deferred until their own durable
//! plan/apply protocols exist.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const BOOTSTRAP_STATE_LOSS_SCHEMA_VERSION: &str = "forge_bootstrap_state_loss_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StateLossKind {
    LinkedStateUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StateLossCause {
    MissingSidecar,
    MissingStateRoot,
    IncompleteState,
    SymlinkSubstitution,
    PermissionDenied,
    Uninspectable,
}

impl StateLossCause {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingSidecar => "missing_sidecar",
            Self::MissingStateRoot => "missing_state_root",
            Self::IncompleteState => "incomplete_state",
            Self::SymlinkSubstitution => "symlink_substitution",
            Self::PermissionDenied => "permission_denied",
            Self::Uninspectable => "uninspectable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StateLossReleaseStatus {
    UnavailableUntrustedState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapRecoveryAction {
    Inspect,
    RestoreVerifiedBackup,
    ReinitializeAsNew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapRecoveryAvailability {
    AvailableReadOnly,
    DeferredPendingVerifiedRestore,
    DeferredPendingReinitializePlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapRecoveryAuthorityEffect {
    None,
    RestoresPriorAuthority,
    AbandonsPriorAuthorityAndCreatesNew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapRecoveryRequirement {
    ReadOnlyMetadataInspection,
    VerifiedCompleteBackup,
    FreshDiagnosisAtApply,
    ExplicitOperatorConfirmation,
    NewProjectIdentityDistinctFromPrior,
    NewAuthorityLocationDistinctFromPrior,
    DurablePlanAndReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRecoveryChoice {
    pub action: BootstrapRecoveryAction,
    pub availability: BootstrapRecoveryAvailability,
    pub authority_effect: BootstrapRecoveryAuthorityEffect,
    pub mutates_authority: bool,
    pub automatic_allowed: bool,
    pub operator_confirmation_required: bool,
    pub requirements: Vec<BootstrapRecoveryRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRecoveryChoices {
    pub inspect: BootstrapRecoveryChoice,
    pub restore_verified_backup: BootstrapRecoveryChoice,
    pub reinitialize_as_new: BootstrapRecoveryChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapRecoveryValidationError {
    EmptyInspectionRoot,
    NonCanonicalChoices,
    UnsupportedSchemaVersion,
    InvalidDiagnosisDigest,
    EmptyProjectId,
    UnsupportedProjectLinkSchemaVersion,
    InvalidProjectLinkDigest,
    UnexpectedWorkflowReleaseIdentity,
}

impl BootstrapRecoveryChoices {
    #[must_use]
    pub fn for_project_root(project_root: &str) -> Self {
        Self {
            inspect: BootstrapRecoveryChoice {
                action: BootstrapRecoveryAction::Inspect,
                availability: BootstrapRecoveryAvailability::AvailableReadOnly,
                authority_effect: BootstrapRecoveryAuthorityEffect::None,
                mutates_authority: false,
                automatic_allowed: true,
                operator_confirmation_required: false,
                requirements: vec![BootstrapRecoveryRequirement::ReadOnlyMetadataInspection],
                argv: vec![
                    "forge-core".to_string(),
                    "project".to_string(),
                    "resolve".to_string(),
                    "--root".to_string(),
                    project_root.to_string(),
                    "--json".to_string(),
                ],
            },
            restore_verified_backup: BootstrapRecoveryChoice {
                action: BootstrapRecoveryAction::RestoreVerifiedBackup,
                availability: BootstrapRecoveryAvailability::DeferredPendingVerifiedRestore,
                authority_effect: BootstrapRecoveryAuthorityEffect::RestoresPriorAuthority,
                mutates_authority: true,
                automatic_allowed: false,
                operator_confirmation_required: true,
                requirements: vec![
                    BootstrapRecoveryRequirement::VerifiedCompleteBackup,
                    BootstrapRecoveryRequirement::FreshDiagnosisAtApply,
                    BootstrapRecoveryRequirement::ExplicitOperatorConfirmation,
                    BootstrapRecoveryRequirement::DurablePlanAndReceipt,
                ],
                argv: Vec::new(),
            },
            reinitialize_as_new: BootstrapRecoveryChoice {
                action: BootstrapRecoveryAction::ReinitializeAsNew,
                availability: BootstrapRecoveryAvailability::DeferredPendingReinitializePlan,
                authority_effect:
                    BootstrapRecoveryAuthorityEffect::AbandonsPriorAuthorityAndCreatesNew,
                mutates_authority: true,
                automatic_allowed: false,
                operator_confirmation_required: true,
                requirements: vec![
                    BootstrapRecoveryRequirement::FreshDiagnosisAtApply,
                    BootstrapRecoveryRequirement::ExplicitOperatorConfirmation,
                    BootstrapRecoveryRequirement::NewProjectIdentityDistinctFromPrior,
                    BootstrapRecoveryRequirement::NewAuthorityLocationDistinctFromPrior,
                    BootstrapRecoveryRequirement::DurablePlanAndReceipt,
                ],
                argv: Vec::new(),
            },
        }
    }

    /// Reject contradictory or caller-forged choice combinations.
    pub fn validate(&self) -> Result<(), BootstrapRecoveryValidationError> {
        let project_root = self
            .inspect
            .argv
            .get(4)
            .filter(|root| !root.trim().is_empty())
            .ok_or(BootstrapRecoveryValidationError::EmptyInspectionRoot)?;
        if self != &Self::for_project_root(project_root) {
            return Err(BootstrapRecoveryValidationError::NonCanonicalChoices);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BootstrapStateLossDiagnostic {
    pub schema_version: String,
    pub diagnosis_digest: String,
    pub kind: StateLossKind,
    pub cause: StateLossCause,
    pub project_id: String,
    pub project_link_schema_version: String,
    pub project_link_sha256: Option<String>,
    pub workflow_release_id: Option<String>,
    pub workflow_release_status: StateLossReleaseStatus,
    pub choices: BootstrapRecoveryChoices,
}

impl BootstrapStateLossDiagnostic {
    /// Validate the closed v1 output contract after deserialization.
    pub fn validate(&self) -> Result<(), BootstrapRecoveryValidationError> {
        if self.schema_version != BOOTSTRAP_STATE_LOSS_SCHEMA_VERSION {
            return Err(BootstrapRecoveryValidationError::UnsupportedSchemaVersion);
        }
        if !is_lower_sha256(&self.diagnosis_digest) {
            return Err(BootstrapRecoveryValidationError::InvalidDiagnosisDigest);
        }
        if self.project_id.trim().is_empty() {
            return Err(BootstrapRecoveryValidationError::EmptyProjectId);
        }
        if self.project_link_schema_version != crate::project_link::PROJECT_LINK_SCHEMA_VERSION {
            return Err(BootstrapRecoveryValidationError::UnsupportedProjectLinkSchemaVersion);
        }
        if self
            .project_link_sha256
            .as_deref()
            .is_some_and(|digest| !is_lower_sha256(digest))
        {
            return Err(BootstrapRecoveryValidationError::InvalidProjectLinkDigest);
        }
        if self.workflow_release_id.is_some() {
            return Err(BootstrapRecoveryValidationError::UnexpectedWorkflowReleaseIdentity);
        }
        self.choices.validate()
    }
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic() -> BootstrapStateLossDiagnostic {
        BootstrapStateLossDiagnostic {
            schema_version: BOOTSTRAP_STATE_LOSS_SCHEMA_VERSION.to_string(),
            diagnosis_digest: "a".repeat(64),
            kind: StateLossKind::LinkedStateUnavailable,
            cause: StateLossCause::MissingSidecar,
            project_id: "app".to_string(),
            project_link_schema_version: crate::project_link::PROJECT_LINK_SCHEMA_VERSION
                .to_string(),
            project_link_sha256: Some("b".repeat(64)),
            workflow_release_id: None,
            workflow_release_status: StateLossReleaseStatus::UnavailableUntrustedState,
            choices: BootstrapRecoveryChoices::for_project_root("/tmp/app"),
        }
    }

    #[test]
    fn recovery_choices_keep_restore_and_reinitialize_semantically_distinct() {
        let choices = BootstrapRecoveryChoices::for_project_root("/tmp/app");
        assert_eq!(
            choices.restore_verified_backup.authority_effect,
            BootstrapRecoveryAuthorityEffect::RestoresPriorAuthority
        );
        assert_eq!(
            choices.reinitialize_as_new.authority_effect,
            BootstrapRecoveryAuthorityEffect::AbandonsPriorAuthorityAndCreatesNew
        );
        assert!(choices.restore_verified_backup.argv.is_empty());
        assert!(choices.reinitialize_as_new.argv.is_empty());
        assert!(!choices.restore_verified_backup.automatic_allowed);
        assert!(!choices.reinitialize_as_new.automatic_allowed);
        assert!(choices
            .reinitialize_as_new
            .requirements
            .contains(&BootstrapRecoveryRequirement::NewProjectIdentityDistinctFromPrior));
    }

    #[test]
    fn inspect_is_the_only_available_read_only_choice() {
        let choices = BootstrapRecoveryChoices::for_project_root("/tmp/app with spaces");
        assert_eq!(
            choices.inspect.availability,
            BootstrapRecoveryAvailability::AvailableReadOnly
        );
        assert!(!choices.inspect.mutates_authority);
        assert_eq!(
            choices.inspect.argv,
            [
                "forge-core",
                "project",
                "resolve",
                "--root",
                "/tmp/app with spaces",
                "--json",
            ]
        );
    }

    #[test]
    fn diagnostic_round_trip_retains_a_valid_closed_contract() {
        let raw = serde_json::to_string(&diagnostic()).unwrap();
        let parsed: BootstrapStateLossDiagnostic = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed, diagnostic());
        assert_eq!(parsed.validate(), Ok(()));
    }

    #[test]
    fn validation_rejects_forged_executable_deferred_choice() {
        let mut forged = diagnostic();
        forged.choices.restore_verified_backup.argv = vec![
            "forge-core".to_string(),
            "restore".to_string(),
            "--force".to_string(),
        ];
        assert_eq!(
            forged.validate(),
            Err(BootstrapRecoveryValidationError::NonCanonicalChoices)
        );
    }
}
