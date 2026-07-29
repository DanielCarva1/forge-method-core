//! Open-world conformance contracts for the solo-developer + agent journey.
//!
//! Host and adapter identifiers are intentionally strings. Forge evaluates a
//! public corpus; it never grants support because a known product name appears.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SOLO_HOST_CONFORMANCE_SCHEMA_VERSION: &str = "solo_host_conformance_v1";
pub const SOLO_HOST_CONFORMANCE_PROTOCOL_VERSION: &str = "solo_host_conformance_protocol_v1";
pub const SOLO_HOST_CONFORMANCE_BUNDLE_VERSION: &str = "solo_host_conformance_bundle_v1";
pub const SOLO_HOST_CONFORMANCE_CORPUS_PATH: &str =
    "contracts/hosts/solo-host-conformance-v1/corpus.json";

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SoloHostCapability {
    Activation,
    CanonicalProjectRoot,
    ReadOnlyGuidance,
    ConversationDerivedIntent,
    CooperativeEvidence,
    IsolatedWork,
    GovernedPromotion,
    ReplacementAgentRecovery,
}

impl SoloHostCapability {
    pub const ALL: [Self; 8] = [
        Self::Activation,
        Self::CanonicalProjectRoot,
        Self::ReadOnlyGuidance,
        Self::ConversationDerivedIntent,
        Self::CooperativeEvidence,
        Self::IsolatedWork,
        Self::GovernedPromotion,
        Self::ReplacementAgentRecovery,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SoloHostConformanceOutcome {
    Supported,
    PartiallySupported,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostConformanceBindings {
    /// Labels supplied by the caller. These are not treated as observed facts.
    pub declared: SoloHostDeclaredBindings,
    /// Facts measured by the Forge process that ran the adapter.
    pub observed: SoloHostObservedBindings,
    pub corpus_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostDeclaredBindings {
    pub host_id: String,
    pub host_version: String,
    pub adapter_id: String,
    pub adapter_version: String,
    pub platform_label: String,
    pub environment_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostObservedBindings {
    pub forge_package: String,
    pub forge_version: String,
    pub forge_executable_sha256: String,
    pub platform: SoloHostObservedPlatform,
    pub canonical_root: SoloHostObservedCanonicalRoot,
    pub adapter_invocation: SoloHostAdapterInvocationBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostObservedPlatform {
    pub os: String,
    pub architecture: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SoloHostCanonicalRootKind {
    WslNetworkShare,
    NativeOrOther,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SoloHostBridgeApplicability {
    Applicable,
    NotApplicable,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostObservedCanonicalRoot {
    /// Digest of the resolved root representation. The personal path is not disclosed.
    pub resolved_path_sha256: String,
    pub kind: SoloHostCanonicalRootKind,
    pub exists: bool,
    pub is_directory: bool,
    pub windows_to_wsl_bridge: SoloHostBridgeApplicability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostFileIdentity {
    /// Basename only. Absolute personal paths are never stored in the bundle.
    pub file_name: String,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SoloHostAdapterArgumentKind {
    File,
    LiteralDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostAdapterArgumentBinding {
    pub position: u32,
    pub kind: SoloHostAdapterArgumentKind,
    /// A basename for files or the fixed marker `<literal-digest>`.
    pub safe_display: String,
    /// Digest of the exact argument bytes, preserving ordering without disclosure.
    pub argument_sha256: String,
    pub file_identity: Option<SoloHostFileIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostAdapterInvocationBinding {
    pub executable: SoloHostFileIdentity,
    pub arguments: Vec<SoloHostAdapterArgumentBinding>,
    /// Digest of canonical JSON for the separated, ordered argument bindings.
    pub argv_sha256: String,
    pub timeout_ms: u64,
    pub output_limit_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostConformanceCorpusDocument {
    pub schema_version: String,
    pub corpus_id: String,
    pub cases: Vec<SoloHostConformanceCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostConformanceCase {
    pub case_id: String,
    pub capability: SoloHostCapability,
    pub description: String,
    pub required_assertions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostConformanceRequestDocument {
    pub schema_version: String,
    pub bindings: SoloHostConformanceBindings,
    pub accepted_native_proof_schemes: Vec<String>,
    pub cases: Vec<SoloHostConformanceCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostConformanceResponseDocument {
    pub schema_version: String,
    pub bindings: SoloHostConformanceBindings,
    pub cases: Vec<SoloHostCaseObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostCaseObservation {
    pub case_id: String,
    pub assertions: BTreeMap<String, SoloHostAssertionClaim>,
    pub gaps: Vec<SoloHostConformanceGap>,
    pub evidence: SoloHostClosedEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostAssertionClaim {
    pub passed: bool,
    pub native_proof_claim: Option<SoloHostNativeProofClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostNativeProofClaim {
    pub scheme: String,
    pub proof_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostClosedEvidence {
    /// Closed, non-secret fact codes. Raw chat, environment, logs, and transcripts are forbidden.
    pub fact_codes: Vec<String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SoloHostGapKind {
    MissingHostApi,
    PlatformBoundaryUnavailable,
    CanonicalRootUnavailable,
    InvocationUnavailable,
    EvidenceUnavailable,
    IsolationUnavailable,
    PromotionUnavailable,
    RecoveryUnavailable,
    NativeAuthenticityUnavailable,
    AdapterFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostConformanceGap {
    pub kind: SoloHostGapKind,
    pub code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SoloHostAssertionStatus {
    Passed,
    Failed,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SoloHostProofState {
    AdapterReportedUnverified,
    ForgeObserved,
    ForgeVerified,
    NativeAuthenticated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostAssertionResult {
    pub assertion: String,
    pub status: SoloHostAssertionStatus,
    pub proof_state: SoloHostProofState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostConformanceResultDocument {
    pub schema_version: String,
    pub bindings: SoloHostConformanceBindings,
    pub capabilities: Vec<SoloHostCapabilityResult>,
    pub integrity_proves_authenticity: bool,
    pub authenticity_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostCapabilityResult {
    pub capability: SoloHostCapability,
    pub outcome: SoloHostConformanceOutcome,
    pub assertions: Vec<SoloHostAssertionResult>,
    pub gaps: Vec<SoloHostConformanceGap>,
    pub artifact_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostConformanceBundleManifest {
    pub schema_version: String,
    pub bindings: SoloHostConformanceBindings,
    /// SHA-256 of canonical manifest JSON with this field set to the empty string.
    pub bundle_digest: String,
    /// Every payload file. manifest.json is deliberately not an entry.
    pub files: Vec<SoloHostBundleFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoloHostBundleFile {
    pub path: String,
    pub role: SoloHostBundleFileRole,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SoloHostBundleFileRole {
    ProtocolRequest,
    ProtocolResponse,
    DerivedResult,
    EvidenceArtifact,
}
