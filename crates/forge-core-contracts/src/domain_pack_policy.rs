//! Closed P6b supply-chain, trust, runtime-capability, and sandbox policy wire types.
//!
//! These documents are inputs and evidence only. Deserializing one never
//! creates trust, runtime availability, execution permission, or lifecycle
//! authority.

use crate::{
    DomainPackArtifactBinding, DomainPackCandidateAuthority, DomainPackCapabilityKind,
    DomainPackCoordinate, DomainPackVersionReference, StableId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION: &str = "0.4";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackTrustPolicyDocument {
    pub schema_version: String,
    pub domain_pack_trust_policy: DomainPackTrustPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackTrustPolicy {
    pub policy_id: StableId,
    pub policy_version: String,
    pub audience: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub registry_keys: Vec<DomainPackRegistryTrustKey>,
    pub required_registry_signature_threshold: u16,
    pub minimum_activation_assurance: DomainPackSourceAssurance,
    pub rules: Vec<DomainPackTrustRule>,
    pub default_disposition: DomainPackTrustDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRegistryTrustKey {
    pub key_id: StableId,
    pub role: DomainPackRegistryTrustRole,
    pub public_key_hex: String,
    pub status: DomainPackCredentialStatus,
    pub valid_from_unix: u64,
    pub valid_until_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRegistryTrustRole {
    RegistrySigner,
    RegistryRevoker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCredentialStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackTrustRule {
    pub rule_id: StableId,
    pub pack: DomainPackCoordinate,
    pub package_digest: Option<String>,
    pub content_digest: Option<String>,
    pub disposition: DomainPackTrustDisposition,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackSourceAssurance {
    ExplicitlyUntrusted,
    LocalExplicit,
    SupplyChainVerified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackTrustDisposition {
    Reject,
    InspectOnly,
    ActivateDeclarativeKnowledge,
    ActivateDeclarativeKnowledgeAndBoundBuiltIns,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRuntimeCapabilityRegistryDocument {
    pub schema_version: String,
    pub domain_pack_runtime_capability_registry: DomainPackRuntimeCapabilityRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRuntimeCapabilityRegistry {
    pub registry_id: StableId,
    pub registry_version: String,
    pub project_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub bindings: Vec<DomainPackRuntimeCapabilityBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRuntimeCapabilityBinding {
    pub binding_id: StableId,
    pub pack: DomainPackVersionReference,
    pub package_digest: String,
    pub subject_ref: StableId,
    pub capability_ref: StableId,
    pub kind: DomainPackCapabilityKind,
    pub provider: DomainPackRuntimeProvider,
    pub implementation_digest: String,
    pub status: DomainPackRuntimeCapabilityStatus,
    pub evidence: DomainPackArtifactBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackRuntimeProvider {
    CoreBuiltin { provider_id: StableId },
    LocalProcess { provider_id: StableId },
    Mcp { provider_id: StableId },
    RuntimeHandshake { provider_id: StableId },
    ExternalConnector { provider_id: StableId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRuntimeCapabilityStatus {
    Available,
    Unavailable,
    Disabled,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCapabilitySandboxPolicyDocument {
    pub schema_version: String,
    pub domain_pack_capability_sandbox_policy: DomainPackCapabilitySandboxPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCapabilitySandboxPolicy {
    pub policy_id: StableId,
    pub policy_version: String,
    pub authority: DomainPackCandidateAuthority,
    pub default_decision: DomainPackSandboxDefaultDecision,
    pub allowed_builtin_binding_ids: Vec<StableId>,
    pub external_execution: DomainPackExternalExecutionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackSandboxDefaultDecision {
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackExternalExecutionPolicy {
    DenyAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackSandboxDecision {
    AllowedBoundBuiltin,
    DeniedUndeclared,
    DeniedByPolicy,
    Unavailable,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLockedCapabilityBinding {
    pub binding_id: StableId,
    pub pack: DomainPackVersionReference,
    pub package_digest: String,
    pub subject_ref: StableId,
    pub capability_ref: StableId,
    pub provider_id: StableId,
    pub implementation_digest: String,
    pub decision: DomainPackSandboxDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRuntimeCapabilityGap {
    pub code: DomainPackRuntimeCapabilityGapCode,
    pub pack: DomainPackVersionReference,
    pub subject_ref: StableId,
    pub capability_ref: StableId,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRuntimeCapabilityGapCode {
    MissingBinding,
    UndeclaredBinding,
    PackageDigestMismatch,
    KindMismatch,
    ExternalProviderDenied,
    Disabled,
    Unavailable,
    Revoked,
}
