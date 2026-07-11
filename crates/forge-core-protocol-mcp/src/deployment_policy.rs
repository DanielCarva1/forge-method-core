//! Typed, fail-closed deployment policy for the MCP adapter.
//!
//! Validation is deliberately separate from activation. In P4b.3a a trusted
//! mutation policy can be proven coherent, but it remains dormant: no value in
//! this module can enable the public stdio mutation path.

use std::fmt;

use forge_core_contracts::StableId;
use serde::{Deserialize, Serialize};

use crate::MCP_EXECUTE_OPERATION_TOOL;

pub const MCP_DEPLOYMENT_POLICY_SCHEMA_VERSION: &str = "0.1";
pub const MCP_EXECUTION_COMMIT_PROTOCOL: &str = "execution_provenance_commit_v0@0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpDeploymentPolicyDocument {
    pub schema_version: String,
    pub mcp_deployment_policy: McpDeploymentPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpDeploymentPolicy {
    pub id: StableId,
    pub mode: McpDeploymentMode,
    pub required_audience: Option<String>,
    #[serde(default)]
    pub mutating_tools: Vec<StableId>,
    pub startup_reconciliation: StartupReconciliationPolicy,
    pub material_loading: MaterialLoadingPolicy,
    pub snapshot_loading: SnapshotLoadingPolicy,
    pub effect_scope: EffectScopePolicy,
    pub public_mutation: PublicMutationPolicy,
    pub root_binding: RootBindingPolicy,
    pub state_root_binding: StateRootBindingPolicy,
    #[serde(default)]
    pub replay_rollback_protection: ReplayRollbackProtectionPolicy,
    pub required_commit_protocol: Option<String>,
    pub same_user_boundary_acknowledged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpDeploymentMode {
    ReadOnly,
    TrustedSingleEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupReconciliationPolicy {
    Disabled,
    RequiredBeforeListen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialLoadingPolicy {
    Disabled,
    CanonicalProjectBound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotLoadingPolicy {
    Disabled,
    BoundedLocalReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectScopePolicy {
    None,
    SingleEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicMutationPolicy {
    Disabled,
    ExplicitOptIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootBindingPolicy {
    CanonicalConfiguredRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateRootBindingPolicy {
    Disabled,
    ProjectLinkResolved,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayRollbackProtectionPolicy {
    #[default]
    Disabled,
    ExternalMonotonicHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpDeploymentActivationState {
    ActiveReadOnly,
    PolicyValidatedDormant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMcpDeploymentPolicy {
    document: McpDeploymentPolicyDocument,
    activation_state: McpDeploymentActivationState,
}

impl ValidatedMcpDeploymentPolicy {
    /// Parses and validates a policy. Trusted policies remain dormant.
    ///
    /// # Errors
    ///
    /// Returns [`McpDeploymentPolicyError::Parse`] for malformed or open YAML
    /// and [`McpDeploymentPolicyError::Invalid`] for incoherent policy values.
    pub fn from_yaml(yaml: &str) -> Result<Self, McpDeploymentPolicyError> {
        let document = yaml_serde::from_str(yaml)
            .map_err(|error| McpDeploymentPolicyError::Parse(error.to_string()))?;
        Self::from_document(document)
    }

    /// Validates a typed policy document without activating mutation.
    ///
    /// # Errors
    ///
    /// Returns [`McpDeploymentPolicyError::Invalid`] with every detected
    /// cross-field invariant violation.
    pub fn from_document(
        document: McpDeploymentPolicyDocument,
    ) -> Result<Self, McpDeploymentPolicyError> {
        let issues = validate_document(&document);
        if !issues.is_empty() {
            return Err(McpDeploymentPolicyError::Invalid(issues));
        }
        let activation_state = match document.mcp_deployment_policy.mode {
            McpDeploymentMode::ReadOnly => McpDeploymentActivationState::ActiveReadOnly,
            McpDeploymentMode::TrustedSingleEffect => {
                McpDeploymentActivationState::PolicyValidatedDormant
            }
        };
        Ok(Self {
            document,
            activation_state,
        })
    }

    #[must_use]
    pub const fn document(&self) -> &McpDeploymentPolicyDocument {
        &self.document
    }

    #[must_use]
    pub const fn activation_state(&self) -> McpDeploymentActivationState {
        self.activation_state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpDeploymentPolicyError {
    Parse(String),
    Invalid(Vec<McpDeploymentPolicyIssue>),
}

impl fmt::Display for McpDeploymentPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "invalid MCP deployment policy YAML: {error}"),
            Self::Invalid(issues) => write!(
                formatter,
                "MCP deployment policy failed with {} issue(s)",
                issues.len()
            ),
        }
    }
}

impl std::error::Error for McpDeploymentPolicyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpDeploymentPolicyIssue {
    pub code: McpDeploymentPolicyIssueCode,
    pub field: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpDeploymentPolicyIssueCode {
    UnsupportedSchemaVersion,
    BlankValue,
    ReadOnlyInvariantViolated,
    TrustedMutationInvariantViolated,
}

fn validate_document(document: &McpDeploymentPolicyDocument) -> Vec<McpDeploymentPolicyIssue> {
    let mut issues = Vec::new();
    if document.schema_version != MCP_DEPLOYMENT_POLICY_SCHEMA_VERSION {
        issue(
            &mut issues,
            McpDeploymentPolicyIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            format!("expected schema version {MCP_DEPLOYMENT_POLICY_SCHEMA_VERSION}"),
        );
    }
    let policy = &document.mcp_deployment_policy;
    if policy.id.0.trim().is_empty() {
        issue(
            &mut issues,
            McpDeploymentPolicyIssueCode::BlankValue,
            "mcp_deployment_policy.id",
            "policy id must not be blank",
        );
    }
    if policy
        .required_audience
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        issue(
            &mut issues,
            McpDeploymentPolicyIssueCode::BlankValue,
            "mcp_deployment_policy.required_audience",
            "required audience must not be blank",
        );
    }
    if policy
        .required_commit_protocol
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        issue(
            &mut issues,
            McpDeploymentPolicyIssueCode::BlankValue,
            "mcp_deployment_policy.required_commit_protocol",
            "required commit protocol must not be blank",
        );
    }

    match policy.mode {
        McpDeploymentMode::ReadOnly => validate_read_only(policy, &mut issues),
        McpDeploymentMode::TrustedSingleEffect => validate_trusted(policy, &mut issues),
    }
    issues
}

fn validate_read_only(policy: &McpDeploymentPolicy, issues: &mut Vec<McpDeploymentPolicyIssue>) {
    let code = McpDeploymentPolicyIssueCode::ReadOnlyInvariantViolated;
    require(
        issues,
        policy.required_audience.is_none(),
        code,
        "mcp_deployment_policy.required_audience",
        "read-only mode must not declare a mutation audience",
    );
    require(
        issues,
        policy.mutating_tools.is_empty(),
        code,
        "mcp_deployment_policy.mutating_tools",
        "read-only mode must not expose mutating tools",
    );
    require(
        issues,
        policy.startup_reconciliation == StartupReconciliationPolicy::Disabled,
        code,
        "mcp_deployment_policy.startup_reconciliation",
        "read-only mode must keep mutation reconciliation disabled",
    );
    require(
        issues,
        policy.material_loading == MaterialLoadingPolicy::Disabled,
        code,
        "mcp_deployment_policy.material_loading",
        "read-only mode must keep trusted material loading disabled",
    );
    require(
        issues,
        policy.snapshot_loading == SnapshotLoadingPolicy::Disabled,
        code,
        "mcp_deployment_policy.snapshot_loading",
        "read-only mode must keep snapshot loading disabled",
    );
    require(
        issues,
        policy.effect_scope == EffectScopePolicy::None,
        code,
        "mcp_deployment_policy.effect_scope",
        "read-only mode must have no effect scope",
    );
    require(
        issues,
        policy.public_mutation == PublicMutationPolicy::Disabled,
        code,
        "mcp_deployment_policy.public_mutation",
        "read-only mode must keep public mutation disabled",
    );
    require(
        issues,
        policy.required_commit_protocol.is_none(),
        code,
        "mcp_deployment_policy.required_commit_protocol",
        "read-only mode must not declare a mutation commit protocol",
    );
    require(
        issues,
        policy.state_root_binding == StateRootBindingPolicy::Disabled,
        code,
        "mcp_deployment_policy.state_root_binding",
        "read-only mode must keep mutation state-root binding disabled",
    );
    require(
        issues,
        policy.replay_rollback_protection == ReplayRollbackProtectionPolicy::Disabled,
        code,
        "mcp_deployment_policy.replay_rollback_protection",
        "read-only mode must keep replay rollback protection disabled",
    );
    require(
        issues,
        !policy.same_user_boundary_acknowledged,
        code,
        "mcp_deployment_policy.same_user_boundary_acknowledged",
        "read-only mode must not claim a mutation trust-boundary acknowledgement",
    );
}

fn validate_trusted(policy: &McpDeploymentPolicy, issues: &mut Vec<McpDeploymentPolicyIssue>) {
    let code = McpDeploymentPolicyIssueCode::TrustedMutationInvariantViolated;
    require(
        issues,
        policy
            .required_audience
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        code,
        "mcp_deployment_policy.required_audience",
        "trusted mode requires a non-blank audience",
    );
    let exact_tool =
        policy.mutating_tools.as_slice() == [StableId(MCP_EXECUTE_OPERATION_TOOL.to_owned())];
    require(
        issues,
        exact_tool,
        code,
        "mcp_deployment_policy.mutating_tools",
        "trusted mode admits exactly one execute-operation tool",
    );
    require(
        issues,
        policy.startup_reconciliation == StartupReconciliationPolicy::RequiredBeforeListen,
        code,
        "mcp_deployment_policy.startup_reconciliation",
        "trusted mode requires reconciliation before listen",
    );
    require(
        issues,
        policy.material_loading == MaterialLoadingPolicy::CanonicalProjectBound,
        code,
        "mcp_deployment_policy.material_loading",
        "trusted mode requires canonical project-bound material loading",
    );
    require(
        issues,
        policy.snapshot_loading == SnapshotLoadingPolicy::BoundedLocalReadOnly,
        code,
        "mcp_deployment_policy.snapshot_loading",
        "trusted mode requires bounded local read-only snapshots",
    );
    require(
        issues,
        policy.effect_scope == EffectScopePolicy::SingleEffect,
        code,
        "mcp_deployment_policy.effect_scope",
        "trusted mode is limited to one effect",
    );
    require(
        issues,
        policy.public_mutation == PublicMutationPolicy::ExplicitOptIn,
        code,
        "mcp_deployment_policy.public_mutation",
        "trusted mode requires explicit operator opt-in",
    );
    require(
        issues,
        policy.required_commit_protocol.as_deref() == Some(MCP_EXECUTION_COMMIT_PROTOCOL),
        code,
        "mcp_deployment_policy.required_commit_protocol",
        "trusted mode requires the pinned provenance commit protocol",
    );
    require(
        issues,
        policy.state_root_binding == StateRootBindingPolicy::ProjectLinkResolved,
        code,
        "mcp_deployment_policy.state_root_binding",
        "trusted mode requires a project-link-resolved state root",
    );
    require(
        issues,
        policy.replay_rollback_protection == ReplayRollbackProtectionPolicy::ExternalMonotonicHead,
        code,
        "mcp_deployment_policy.replay_rollback_protection",
        "trusted mode requires an external monotonic replay head",
    );
    require(
        issues,
        policy.same_user_boundary_acknowledged,
        code,
        "mcp_deployment_policy.same_user_boundary_acknowledged",
        "trusted mode requires acknowledgement of the same-user trust boundary",
    );
}

fn require(
    issues: &mut Vec<McpDeploymentPolicyIssue>,
    condition: bool,
    code: McpDeploymentPolicyIssueCode,
    field: &'static str,
    message: &'static str,
) {
    if !condition {
        issue(issues, code, field, message);
    }
}

fn issue(
    issues: &mut Vec<McpDeploymentPolicyIssue>,
    code: McpDeploymentPolicyIssueCode,
    field: &'static str,
    message: impl Into<String>,
) {
    issues.push(McpDeploymentPolicyIssue {
        code,
        field,
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_only_document() -> McpDeploymentPolicyDocument {
        McpDeploymentPolicyDocument {
            schema_version: MCP_DEPLOYMENT_POLICY_SCHEMA_VERSION.to_owned(),
            mcp_deployment_policy: McpDeploymentPolicy {
                id: StableId("local-read-only".to_owned()),
                mode: McpDeploymentMode::ReadOnly,
                required_audience: None,
                mutating_tools: Vec::new(),
                startup_reconciliation: StartupReconciliationPolicy::Disabled,
                material_loading: MaterialLoadingPolicy::Disabled,
                snapshot_loading: SnapshotLoadingPolicy::Disabled,
                effect_scope: EffectScopePolicy::None,
                public_mutation: PublicMutationPolicy::Disabled,
                root_binding: RootBindingPolicy::CanonicalConfiguredRoot,
                state_root_binding: StateRootBindingPolicy::Disabled,
                replay_rollback_protection: ReplayRollbackProtectionPolicy::Disabled,
                required_commit_protocol: None,
                same_user_boundary_acknowledged: false,
            },
        }
    }

    fn trusted_document() -> McpDeploymentPolicyDocument {
        let mut document = read_only_document();
        let policy = &mut document.mcp_deployment_policy;
        policy.id = StableId("trusted-local-single-effect".to_owned());
        policy.mode = McpDeploymentMode::TrustedSingleEffect;
        policy.required_audience = Some("forge-local".to_owned());
        policy.mutating_tools = vec![StableId(MCP_EXECUTE_OPERATION_TOOL.to_owned())];
        policy.startup_reconciliation = StartupReconciliationPolicy::RequiredBeforeListen;
        policy.material_loading = MaterialLoadingPolicy::CanonicalProjectBound;
        policy.snapshot_loading = SnapshotLoadingPolicy::BoundedLocalReadOnly;
        policy.effect_scope = EffectScopePolicy::SingleEffect;
        policy.public_mutation = PublicMutationPolicy::ExplicitOptIn;
        policy.required_commit_protocol = Some(MCP_EXECUTION_COMMIT_PROTOCOL.to_owned());
        policy.state_root_binding = StateRootBindingPolicy::ProjectLinkResolved;
        policy.replay_rollback_protection = ReplayRollbackProtectionPolicy::ExternalMonotonicHead;
        policy.same_user_boundary_acknowledged = true;
        document
    }

    #[test]
    fn read_only_policy_is_active_and_safe_by_construction() {
        let validated =
            ValidatedMcpDeploymentPolicy::from_document(read_only_document()).expect("safe policy");
        assert_eq!(
            validated.activation_state(),
            McpDeploymentActivationState::ActiveReadOnly
        );
    }

    #[test]
    fn published_read_only_example_matches_the_typed_contract() {
        let yaml = include_str!("../../../contracts/examples/mcp-deployment-policy.yaml");
        let validated =
            ValidatedMcpDeploymentPolicy::from_yaml(yaml).expect("published safe policy");
        assert_eq!(
            validated.activation_state(),
            McpDeploymentActivationState::ActiveReadOnly
        );
    }

    #[test]
    fn published_trusted_example_is_validated_but_not_activated() {
        let yaml = include_str!(
            "../../../contracts/examples/mcp-trusted-single-effect-deployment-policy.yaml"
        );
        let validated =
            ValidatedMcpDeploymentPolicy::from_yaml(yaml).expect("published trusted policy");
        assert_eq!(
            validated.activation_state(),
            McpDeploymentActivationState::PolicyValidatedDormant
        );
    }

    #[test]
    fn trusted_policy_validates_but_remains_dormant() {
        let validated = ValidatedMcpDeploymentPolicy::from_document(trusted_document())
            .expect("coherent trusted policy");
        assert_eq!(
            validated.activation_state(),
            McpDeploymentActivationState::PolicyValidatedDormant
        );
    }

    #[test]
    fn trusted_policy_reports_all_missing_safety_requirements() {
        let mut document = read_only_document();
        document.mcp_deployment_policy.mode = McpDeploymentMode::TrustedSingleEffect;
        let McpDeploymentPolicyError::Invalid(issues) =
            ValidatedMcpDeploymentPolicy::from_document(document).expect_err("must reject")
        else {
            panic!("expected typed validation issues");
        };
        assert_eq!(issues.len(), 11);
        assert!(issues
            .iter()
            .any(|issue| { issue.field == "mcp_deployment_policy.replay_rollback_protection" }));
    }

    #[test]
    fn trusted_policy_rejects_duplicate_or_extra_mutating_tools() {
        for tools in [
            vec![
                StableId(MCP_EXECUTE_OPERATION_TOOL.to_owned()),
                StableId(MCP_EXECUTE_OPERATION_TOOL.to_owned()),
            ],
            vec![StableId("claim".to_owned())],
        ] {
            let mut document = trusted_document();
            document.mcp_deployment_policy.mutating_tools = tools;
            assert!(matches!(
                ValidatedMcpDeploymentPolicy::from_document(document),
                Err(McpDeploymentPolicyError::Invalid(_))
            ));
        }
    }

    #[test]
    fn read_only_policy_rejects_mutation_settings() {
        let mut document = read_only_document();
        document.mcp_deployment_policy.public_mutation = PublicMutationPolicy::ExplicitOptIn;
        document.mcp_deployment_policy.mutating_tools =
            vec![StableId(MCP_EXECUTE_OPERATION_TOOL.to_owned())];
        let McpDeploymentPolicyError::Invalid(issues) =
            ValidatedMcpDeploymentPolicy::from_document(document).expect_err("must reject")
        else {
            panic!("expected typed validation issues");
        };
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn yaml_is_closed_to_unknown_fields_and_broader_effect_scopes() {
        let yaml = yaml_serde::to_string(&read_only_document()).expect("serialize fixture");
        let unknown = format!("{yaml}unknown_field: true\n");
        assert!(matches!(
            ValidatedMcpDeploymentPolicy::from_yaml(&unknown),
            Err(McpDeploymentPolicyError::Parse(_))
        ));

        let broader = yaml.replace("effect_scope: none", "effect_scope: operation_wide");
        assert!(matches!(
            ValidatedMcpDeploymentPolicy::from_yaml(&broader),
            Err(McpDeploymentPolicyError::Parse(_))
        ));
    }
}
