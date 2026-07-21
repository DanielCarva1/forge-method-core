//! Closed contracts for common borrowed-shell host and client surfaces.
//!
//! These documents describe candidate integrations only. Host recognition and
//! manifest projection are read-only inputs; neither can manufacture Forge Core
//! authority, installability, human-origin assurance, governed mutation, or a
//! support claim.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const HOST_CONFORMANCE_SCHEMA_VERSION: &str = "0.1";
pub const CLI_JSON_SURFACE_SCHEMA_VERSION: &str = "0.1";
pub const MCP_SURFACE_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostConformanceDocument {
    pub schema_version: String,
    pub host_conformance: HostConformanceContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostConformanceContract {
    pub contract_id: String,
    pub authority: HostContractAuthority,
    pub selected_host: Option<HostKind>,
    pub released: bool,
    pub field_verified: bool,
    pub exact_host_execution: CapabilityOutcome,
    pub read_only_inputs: HostReadOnlyInputs,
    pub claim_separation: Vec<HostClaimSeparation>,
    pub candidates: Vec<HostCandidate>,
    pub journeys: Vec<HostJourneyExpectation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostContractAuthority {
    CandidateOnly,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    Codex,
    Cursor,
    Opencode,
    Claude,
    Pidev,
    ForgeApp,
}

impl HostKind {
    pub const ALL: [Self; 6] = [
        Self::Codex,
        Self::Cursor,
        Self::Opencode,
        Self::Claude,
        Self::Pidev,
        Self::ForgeApp,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityOutcome {
    Candidate,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostReadOnlyInputs {
    pub host_origin_broker_conformance_ref: String,
    pub adapter_projection_policy_ref: String,
    pub treatment: InputAuthorityTreatment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InputAuthorityTreatment {
    ReadOnlyNonAuthoritative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostClaimSeparation {
    pub claim: HostClaimKind,
    pub independent_from: Vec<HostClaimKind>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HostClaimKind {
    RuntimeRecognition,
    ReadOnlyMcp,
    Installability,
    HumanOriginAssurance,
    GovernedMutation,
    Support,
}

impl HostClaimKind {
    pub const ALL: [Self; 6] = [
        Self::RuntimeRecognition,
        Self::ReadOnlyMcp,
        Self::Installability,
        Self::HumanOriginAssurance,
        Self::GovernedMutation,
        Self::Support,
    ];
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostCandidate {
    pub host: HostKind,
    pub disposition: CapabilityOutcome,
    pub selected: bool,
    pub supported: bool,
    pub released: bool,
    pub field_verified: bool,
    pub claims: Vec<HostCapabilityClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostCapabilityClaim {
    pub claim: HostClaimKind,
    pub outcome: CapabilityOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostJourneyExpectation {
    pub journey: HostJourney,
    pub outcome: CapabilityOutcome,
    pub host_client_responsibilities: Vec<HostClientResponsibility>,
    pub forge_core_responsibilities: Vec<ForgeCoreResponsibility>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HostJourney {
    Install,
    Invoke,
    Update,
    Diagnose,
    Recover,
    UnauthorizedMutation,
    FreshChat,
}

impl HostJourney {
    pub const ALL: [Self; 7] = [
        Self::Install,
        Self::Invoke,
        Self::Update,
        Self::Diagnose,
        Self::Recover,
        Self::UnauthorizedMutation,
        Self::FreshChat,
    ];
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HostClientResponsibility {
    DiscoverCandidate,
    TranslateManifest,
    InstallOwnedFiles,
    InvokeArgvWithoutShellString,
    PreservePriorVersionDuringUpdate,
    RenderTypedDiagnostics,
    RemoveOwnedIntegrationOnly,
    StartWithoutHiddenChatState,
    RejectUnauthorizedMutationRequest,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ForgeCoreResponsibility {
    ValidateContracts,
    ClassifyMutation,
    ClassifyAuthority,
    ReturnTypedSetupGap,
    AdmitGovernedMutation,
    PreserveProjectAuthority,
    RejectUnknownOrUnauthorizedInput,
    RequireExplicitContextEachChat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CliJsonSurfaceDocument {
    pub schema_version: String,
    pub cli_json_surface: CliJsonSurfaceContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CliJsonSurfaceContract {
    pub contract_id: String,
    pub transport: SurfaceTransport,
    pub authority: SurfaceAuthority,
    pub derived_from_manifest: bool,
    pub projection_authoritative: bool,
    pub preserved_fields: Vec<ProjectedField>,
    pub setup_gap_types: Vec<SetupGapKind>,
    pub projections_must_not: Vec<ProjectionProhibition>,
    pub ocsp_delegated_responder: OcspDelegatedResponderSurfaceBoundary,
    pub invocation: CliJsonInvocationPolicy,
    pub recognition: RecognitionBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpSurfaceDocument {
    pub schema_version: String,
    pub mcp_surface: McpSurfaceContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpSurfaceContract {
    pub contract_id: String,
    pub transport: SurfaceTransport,
    pub authority: SurfaceAuthority,
    pub derived_from_manifest: bool,
    pub projection_authoritative: bool,
    pub preserved_fields: Vec<ProjectedField>,
    pub setup_gap_types: Vec<SetupGapKind>,
    pub projections_must_not: Vec<ProjectionProhibition>,
    pub ocsp_delegated_responder: OcspDelegatedResponderSurfaceBoundary,
    pub default_mode: McpDefaultMode,
    pub mutation_authority: DefaultAuthorityGrant,
    pub signing_authority: DefaultAuthorityGrant,
    pub signer_tool_exposed: bool,
    pub mutation_policy: McpMutationPolicy,
    pub recognition: RecognitionBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceTransport {
    CliJson,
    Mcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceAuthority {
    CoreOnly,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProjectedField {
    CommandKind,
    MutationClass,
    AuthorityClass,
    SafeAutoInvocationTriggers,
    OutputTreatment,
    RequiredContracts,
    SetupGaps,
}

impl ProjectedField {
    pub const ALL: [Self; 7] = [
        Self::CommandKind,
        Self::MutationClass,
        Self::AuthorityClass,
        Self::SafeAutoInvocationTriggers,
        Self::OutputTreatment,
        Self::RequiredContracts,
        Self::SetupGaps,
    ];
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SetupGapKind {
    MissingExecutable,
    MissingAdapter,
    UnsupportedHostVersion,
    InvalidConfiguration,
    MissingRequiredContract,
    ExactHostExecutionUnavailable,
}

impl SetupGapKind {
    pub const ALL: [Self; 6] = [
        Self::MissingExecutable,
        Self::MissingAdapter,
        Self::UnsupportedHostVersion,
        Self::InvalidConfiguration,
        Self::MissingRequiredContract,
        Self::ExactHostExecutionUnavailable,
    ];
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionProhibition {
    NetworkRetrievalAuthority,
    InstallAuthority,
    UpdateAuthority,
    CrlAuthority,
    CertificateTransparencyAuthority,
    RekorAuthority,
    TufAuthority,
    SigningAuthority,
    MutationAuthority,
    HostSelection,
    HostSupportClaim,
    HostReleaseClaim,
    ProjectionAuthorityPromotion,
}

impl ProjectionProhibition {
    pub const ALL: [Self; 13] = [
        Self::NetworkRetrievalAuthority,
        Self::InstallAuthority,
        Self::UpdateAuthority,
        Self::CrlAuthority,
        Self::CertificateTransparencyAuthority,
        Self::RekorAuthority,
        Self::TufAuthority,
        Self::SigningAuthority,
        Self::MutationAuthority,
        Self::HostSelection,
        Self::HostSupportClaim,
        Self::HostReleaseClaim,
        Self::ProjectionAuthorityPromotion,
    ];
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OcspDelegatedResponderSurfaceBoundary {
    pub supplied_responder_certificate_input: bool,
    pub ordered_issuer_chain_input: bool,
    pub selected_authority_identity_output: bool,
    pub verified_authority_evidence_output: bool,
    pub network_authority: bool,
    pub install_authority: bool,
    pub update_authority: bool,
    pub crl_authority: bool,
    pub certificate_transparency_authority: bool,
    pub rekor_authority: bool,
    pub tuf_authority: bool,
    pub signing_authority: bool,
    pub mutation_authority: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CliJsonInvocationPolicy {
    pub argv_only: bool,
    pub shell_strings_allowed: bool,
    pub mutation_requires_core_admission: bool,
    pub signing_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpDefaultMode {
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DefaultAuthorityGrant {
    Forbidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpMutationPolicy {
    pub default_tools_read_only: bool,
    pub explicit_core_admission_required: bool,
    pub client_annotations_are_advisory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecognitionBoundary {
    pub runtime_kind_sufficient: bool,
    pub manifest_recognition_sufficient: bool,
    pub cannot_establish: Vec<RecognitionCannotEstablish>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RecognitionCannotEstablish {
    Installability,
    HumanOriginAssurance,
    GovernedMutation,
    Support,
}

impl RecognitionCannotEstablish {
    pub const ALL: [Self; 4] = [
        Self::Installability,
        Self::HumanOriginAssurance,
        Self::GovernedMutation,
        Self::Support,
    ];
}
