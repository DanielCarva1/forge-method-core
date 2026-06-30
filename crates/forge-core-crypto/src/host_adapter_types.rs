//! Host adapter types — structs and enums that describe the contract surface
//! between the Forge core and host adapters (manifest, projection, process
//! security, distribution, verification inputs/outputs).
//!
//! Extracted from `lib.rs` in R1.HostAdapterTypes (2026-06-29). Pure data
//! types — no logic lives here. All items are `pub` and re-exported at the
//! crate root via `pub use host_adapter_types::*;` so existing call sites
//! (`main.rs`, `tests/validate.rs`) keep importing from `forge_core_cli`.

use forge_core_contracts::RuntimeKind;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::ops::Deref;
use std::path::PathBuf;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// OCSP nonce as lowercase hex. Wrapped in a newtype because nonces are
/// client secrets that must be zeroized on drop. Serializes transparently
/// as a string so existing JSON consumers are unaffected.
#[derive(Clone, Debug, Zeroize, ZeroizeOnDrop)]
pub struct OcspNonceHex(pub String);

impl OcspNonceHex {
    /// Build from any string-like value. Caller is responsible for
    /// formatting (typically lowercase hex).
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Access the inner hex string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for OcspNonceHex {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for OcspNonceHex {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for OcspNonceHex {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Serialize for OcspNonceHex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterManifest {
    pub schema_version: String,
    pub manifest_id: String,
    pub supported_runtime_kinds: Vec<RuntimeKind>,
    pub authority_boundary: HostAdapterAuthorityBoundary,
    pub commands: Vec<HostAdapterCommand>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterAuthorityBoundary {
    pub source_of_truth: String,
    pub adapters_may: Vec<String>,
    pub adapters_must_not: Vec<String>,
    pub mutation_rule: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterCommand {
    pub name: String,
    pub command_kind: HostAdapterCommandKind,
    pub mutation_class: HostAdapterMutationClass,
    pub authority_class: HostAdapterAuthorityClass,
    pub json_supported: bool,
    pub required_contracts: Vec<String>,
    pub safe_auto_invocation_triggers: Vec<HostAdapterAutoTrigger>,
    pub output_treatment: Vec<HostAdapterOutputTreatment>,
    pub policy_refs: Vec<String>,
    pub adapters_must_not: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterCommandKind {
    Validation,
    OperationExecution,
    OperationalRepair,
    AdvisoryLookup,
    CapabilityManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterMutationClass {
    ReadOnly,
    AppendOnlyOperational,
    MutatingOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterAuthorityClass {
    NoWorkflowAuthority,
    RequiresOperationAuthority,
    OperationalMaintenanceOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterAutoTrigger {
    EvidenceDiscovery,
    Diagnostics,
    HandoffPreparation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterOutputTreatment {
    ValidationEvidence,
    RuntimeAuthorityResponse,
    AdvisoryContext,
    OperationalMaintenanceEvidence,
    HostCapabilityMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterProjectionTarget {
    McpTools,
    BorrowedShell,
    AppUi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterProcessTarget {
    McpStdio,
    BorrowedShell,
    AppBridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterInvocationAdmissionStatus {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterUpdateChannel {
    Stable,
    Canary,
    Dev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterDistributionAdmissionStatus {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterArtifactVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterProvenanceVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterRekorVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterSigstoreTrustPolicyVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterFulcioCertificateIdentityVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterSigstoreBundleSubjectVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterSigstoreDsseInTotoSubjectVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterSigstoreTimestampAuthorityVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterCertificateTransparencySctVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterCertificateRevocationPolicyVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterTufTrustedRootFreshnessVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterCertificateCrlStatusVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapterCertificateOcspStatusVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterProjection {
    pub schema_version: String,
    pub projection_id: String,
    pub target: HostAdapterProjectionTarget,
    pub derived_from_manifest: String,
    pub projection_authoritative: bool,
    pub authority_boundary: HostAdapterProjectionAuthorityBoundary,
    pub commands: Vec<HostAdapterProjectedCommand>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterProjectionAuthorityBoundary {
    pub source_of_truth: String,
    pub projection_rule: String,
    pub projected_metadata_must_preserve: Vec<String>,
    pub projections_must_not: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterProcessSecurityPolicy {
    pub schema_version: String,
    pub policy_id: String,
    pub target: HostAdapterProcessTarget,
    pub derived_from_manifest: String,
    pub default_admission: HostAdapterInvocationAdmissionStatus,
    pub argv_policy: HostAdapterArgvPolicy,
    pub cwd_policy: HostAdapterCwdPolicy,
    pub env_policy: HostAdapterEnvPolicy,
    pub stdio_policy: HostAdapterStdioPolicy,
    pub command_admissions: Vec<HostAdapterCommandProcessAdmission>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterArgvPolicy {
    pub shell_strings_allowed: bool,
    pub argv_must_start_with: Vec<String>,
    pub unknown_commands_allowed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterCwdPolicy {
    pub repo_root_scoped: bool,
    pub outside_root_allowed_by_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterEnvPolicy {
    pub inherit_full_environment: bool,
    pub allowed_keys: Vec<String>,
    pub forbidden_key_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterStdioPolicy {
    pub stdin_format: String,
    pub stdout_format: String,
    pub stderr_policy: String,
    pub raw_payload_bytes_allowed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterCommandProcessAdmission {
    pub command_name: String,
    pub mutation_class: HostAdapterMutationClass,
    pub authority_class: HostAdapterAuthorityClass,
    pub automatic_invocation_allowed: bool,
    pub explicit_invocation_required: bool,
    pub mcp_stdio_enabled: bool,
    pub required_controls: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HostAdapterInvocationRequest {
    pub command_name: String,
    pub target: HostAdapterProcessTarget,
    pub explicit_invocation: bool,
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub env_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterInvocationAdmission {
    pub status: HostAdapterInvocationAdmissionStatus,
    pub command_name: String,
    pub target: HostAdapterProcessTarget,
    pub mutation_class: Option<HostAdapterMutationClass>,
    pub authority_class: Option<HostAdapterAuthorityClass>,
    pub reasons: Vec<String>,
    pub required_controls: Vec<String>,
    pub allowed_argv_prefix: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterDistributionPolicy {
    pub schema_version: String,
    pub policy_id: String,
    pub supported_runtime_targets: Vec<RuntimeKind>,
    pub default_admission: HostAdapterDistributionAdmissionStatus,
    pub required_evidence: HostAdapterDistributionRequiredEvidence,
    pub channel_policy: HostAdapterDistributionChannelPolicy,
    pub updater_policy: HostAdapterUpdaterPolicy,
}

// Each field is an independent evidence requirement that distribution
// admission checks individually. Keeping them as bools preserves the JSON
// schema that agents and operators inspect.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterDistributionRequiredEvidence {
    pub immutable_source_ref: bool,
    pub artifact_checksum_or_signature: bool,
    pub provenance_ref: bool,
    pub version_compatibility: bool,
    pub rollback_ref: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterDistributionChannelPolicy {
    pub stable_allowed: bool,
    pub canary_allowed_with_explicit_opt_in: bool,
    pub dev_allowed_for_general_install: bool,
}

// Each field is an independent updater-policy switch that distribution
// admission evaluates separately.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterUpdaterPolicy {
    pub update_summary_required: bool,
    pub rollback_metadata_required: bool,
    pub preserve_local_project_state: bool,
    pub self_update_may_bypass_admission: bool,
}

#[derive(Debug, Clone)]
pub struct HostAdapterDistributionEvidence {
    pub target: RuntimeKind,
    pub channel: HostAdapterUpdateChannel,
    pub artifact_name: String,
    pub artifact_sha256: Option<String>,
    pub signature_ref: Option<String>,
    pub provenance_ref: Option<String>,
    pub source_ref: Option<String>,
    pub version: Option<String>,
    pub compatible_core_version: Option<String>,
    pub rollback_ref: Option<String>,
    pub update_summary_ref: Option<String>,
    pub explicit_canary_opt_in: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterDistributionAdmission {
    pub status: HostAdapterDistributionAdmissionStatus,
    pub target: RuntimeKind,
    pub channel: HostAdapterUpdateChannel,
    pub artifact_name: String,
    pub reasons: Vec<String>,
    pub required_evidence: HostAdapterDistributionRequiredEvidence,
    pub accepted_evidence: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HostAdapterArtifactVerificationInput {
    pub artifact_path: PathBuf,
    pub expected_sha256: String,
    pub signature_ref: Option<String>,
    pub provenance_ref: Option<String>,
    pub source_ref: Option<String>,
    pub version: Option<String>,
    pub compatible_core_version: Option<String>,
    pub rollback_ref: Option<String>,
    pub update_summary_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterArtifactVerification {
    pub status: HostAdapterArtifactVerificationStatus,
    pub artifact_path: String,
    pub byte_len: Option<usize>,
    pub expected_sha256: String,
    pub computed_sha256: Option<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub deferred_verification: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HostAdapterProvenanceVerificationInput {
    pub artifact_path: PathBuf,
    pub provenance_path: PathBuf,
    pub signature_path: PathBuf,
    pub public_key_path: PathBuf,
    pub transparency_log_path: PathBuf,
    pub expected_sha256: String,
    pub expected_builder_id: String,
    pub expected_source_uri: String,
    pub expected_source_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterProvenanceVerification {
    pub status: HostAdapterProvenanceVerificationStatus,
    pub artifact_path: String,
    pub provenance_path: String,
    pub signature_path: String,
    pub public_key_path: String,
    pub transparency_log_path: String,
    pub computed_artifact_sha256: Option<String>,
    pub provenance_sha256: Option<String>,
    pub signature_sha256: Option<String>,
    pub predicate_type: Option<String>,
    pub builder_id: Option<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterRekorVerificationInput {
    pub log_entry_path: PathBuf,
    pub public_key_path: PathBuf,
    pub expected_log_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterRekorVerification {
    pub status: HostAdapterRekorVerificationStatus,
    pub log_entry_path: String,
    pub public_key_path: String,
    pub expected_log_id: String,
    pub observed_log_id: Option<String>,
    pub log_index: Option<i64>,
    pub integrated_time: Option<i64>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterSigstoreTrustPolicyVerificationInput {
    pub policy_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterSigstoreTrustPolicyVerification {
    pub status: HostAdapterSigstoreTrustPolicyVerificationStatus,
    pub policy_path: String,
    pub schema_version: Option<String>,
    pub root_source: Option<String>,
    pub trusted_root_ref: Option<String>,
    pub timestamp_mode: Option<String>,
    pub expected_oidc_issuer: Option<String>,
    pub expected_certificate_identity: Option<String>,
    pub expected_github_repository: Option<String>,
    pub expected_github_ref: Option<String>,
    pub expected_github_sha: Option<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterFulcioCertificateIdentityVerificationInput {
    pub trust_policy_path: PathBuf,
    pub certificate_path: PathBuf,
    pub issuer_certificate_paths: Vec<PathBuf>,
    pub verification_time_unix: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterFulcioCertificateIdentityVerification {
    pub status: HostAdapterFulcioCertificateIdentityVerificationStatus,
    pub trust_policy_path: String,
    pub certificate_path: String,
    pub issuer_certificate_paths: Vec<String>,
    pub verification_time_unix: i64,
    pub expected_oidc_issuer: Option<String>,
    pub expected_certificate_identity: Option<String>,
    pub expected_github_repository: Option<String>,
    pub expected_github_ref: Option<String>,
    pub expected_github_sha: Option<String>,
    pub observed_subject_alt_names: Vec<String>,
    pub observed_oidc_issuer: Option<String>,
    pub observed_build_signer_uri: Option<String>,
    pub observed_build_signer_digest: Option<String>,
    pub observed_source_repository_uri: Option<String>,
    pub observed_source_repository_digest: Option<String>,
    pub observed_source_repository_ref: Option<String>,
    pub observed_token_subject: Option<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterSigstoreBundleSubjectVerificationInput {
    pub bundle_path: PathBuf,
    pub artifact_path: PathBuf,
    pub trust_policy_path: PathBuf,
    pub certificate_path: PathBuf,
    pub issuer_certificate_paths: Vec<PathBuf>,
    pub rekor_log_entry_path: PathBuf,
    pub rekor_public_key_path: PathBuf,
    pub expected_rekor_log_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterSigstoreBundleSubjectVerification {
    pub status: HostAdapterSigstoreBundleSubjectVerificationStatus,
    pub bundle_path: String,
    pub artifact_path: String,
    pub trust_policy_path: String,
    pub certificate_path: String,
    pub issuer_certificate_paths: Vec<String>,
    pub rekor_log_entry_path: String,
    pub rekor_public_key_path: String,
    pub expected_rekor_log_id: String,
    pub media_type: Option<String>,
    pub computed_artifact_sha256: Option<String>,
    pub bundle_message_digest_sha256: Option<String>,
    pub bundle_signature_sha256: Option<String>,
    pub rekor_integrated_time: Option<i64>,
    pub fulcio_status: Option<HostAdapterFulcioCertificateIdentityVerificationStatus>,
    pub rekor_status: Option<HostAdapterRekorVerificationStatus>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterSigstoreDsseInTotoSubjectVerificationInput {
    pub bundle_path: PathBuf,
    pub artifact_path: PathBuf,
    pub trust_policy_path: PathBuf,
    pub certificate_path: PathBuf,
    pub issuer_certificate_paths: Vec<PathBuf>,
    pub rekor_log_entry_path: PathBuf,
    pub rekor_public_key_path: PathBuf,
    pub expected_rekor_log_id: String,
    pub expected_predicate_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterSigstoreDsseInTotoSubjectVerification {
    pub status: HostAdapterSigstoreDsseInTotoSubjectVerificationStatus,
    pub bundle_path: String,
    pub artifact_path: String,
    pub trust_policy_path: String,
    pub certificate_path: String,
    pub issuer_certificate_paths: Vec<String>,
    pub rekor_log_entry_path: String,
    pub rekor_public_key_path: String,
    pub expected_rekor_log_id: String,
    pub expected_predicate_type: Option<String>,
    pub media_type: Option<String>,
    pub payload_type: Option<String>,
    pub computed_artifact_sha256: Option<String>,
    pub dsse_payload_sha256: Option<String>,
    pub dsse_envelope_sha256: Option<String>,
    pub dsse_signature_sha256: Option<String>,
    pub statement_type: Option<String>,
    pub predicate_type: Option<String>,
    pub rekor_integrated_time: Option<i64>,
    pub fulcio_status: Option<HostAdapterFulcioCertificateIdentityVerificationStatus>,
    pub rekor_status: Option<HostAdapterRekorVerificationStatus>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterSigstoreTimestampAuthorityVerificationInput {
    pub trust_policy_path: PathBuf,
    pub certificate_path: PathBuf,
    pub rekor_log_entry_path: Option<PathBuf>,
    pub rekor_public_key_path: Option<PathBuf>,
    pub expected_rekor_log_id: Option<String>,
    pub rfc3161_timestamp_token_path: Option<PathBuf>,
    pub rfc3161_timestamped_signature_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterSigstoreTimestampAuthorityVerification {
    pub status: HostAdapterSigstoreTimestampAuthorityVerificationStatus,
    pub trust_policy_path: String,
    pub certificate_path: String,
    pub rekor_log_entry_path: Option<String>,
    pub rekor_public_key_path: Option<String>,
    pub expected_rekor_log_id: Option<String>,
    pub rfc3161_timestamp_token_path: Option<String>,
    pub rfc3161_timestamped_signature_path: Option<String>,
    pub rfc3161_tsa_certificate_refs: Vec<String>,
    pub policy_mode: Option<String>,
    pub selected_timestamp_source: Option<String>,
    pub observed_timestamp_unix: Option<i64>,
    pub certificate_not_before_unix: Option<i64>,
    pub certificate_not_after_unix: Option<i64>,
    pub rekor_status: Option<HostAdapterRekorVerificationStatus>,
    pub deferred_verification: Vec<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterCertificateTransparencySctVerificationInput {
    pub trust_policy_path: PathBuf,
    pub certificate_path: PathBuf,
    pub sct_paths: Vec<PathBuf>,
    pub verification_time_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterCertificateTransparencySctVerification {
    pub status: HostAdapterCertificateTransparencySctVerificationStatus,
    pub trust_policy_path: String,
    pub certificate_path: String,
    pub sct_paths: Vec<String>,
    pub verification_time_unix_ms: u64,
    pub policy_log_ids: Vec<String>,
    pub ct_public_key_refs: Vec<String>,
    pub verified_log_ids: Vec<String>,
    pub verified_sct_count: usize,
    pub deferred_verification: Vec<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterCertificateRevocationPolicyVerificationInput {
    pub trust_policy_path: PathBuf,
    pub certificate_path: PathBuf,
    pub trusted_signing_time_unix: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterCertificateRevocationPolicyVerification {
    pub status: HostAdapterCertificateRevocationPolicyVerificationStatus,
    pub trust_policy_path: String,
    pub certificate_path: String,
    pub trusted_signing_time_unix: i64,
    pub policy_mode: Option<String>,
    pub max_certificate_lifetime_seconds: Option<i64>,
    pub certificate_not_before_unix: Option<i64>,
    pub certificate_not_after_unix: Option<i64>,
    pub certificate_lifetime_seconds: Option<i64>,
    pub revocation_strategy: Option<String>,
    pub revocation_status: Option<String>,
    pub deferred_verification: Vec<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterTufTrustedRootFreshnessVerificationInput {
    pub trust_policy_path: PathBuf,
    pub root_metadata_path: PathBuf,
    pub timestamp_metadata_path: Option<PathBuf>,
    pub snapshot_metadata_path: Option<PathBuf>,
    pub targets_metadata_path: Option<PathBuf>,
    pub update_start_time_unix: i64,
    pub min_root_version: Option<i64>,
    pub min_timestamp_version: Option<i64>,
    pub min_snapshot_version: Option<i64>,
    pub min_targets_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterTufMetadataFreshnessRole {
    pub role: String,
    pub metadata_path: String,
    pub version: Option<i64>,
    pub min_version: Option<i64>,
    pub expires: Option<String>,
    pub expires_unix: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterTufTrustedRootFreshnessVerification {
    pub status: HostAdapterTufTrustedRootFreshnessVerificationStatus,
    pub trust_policy_path: String,
    pub root_metadata_path: String,
    pub timestamp_metadata_path: Option<String>,
    pub snapshot_metadata_path: Option<String>,
    pub targets_metadata_path: Option<String>,
    pub update_start_time_unix: i64,
    pub root_source: Option<String>,
    pub trusted_root_ref: Option<String>,
    pub verified_roles: Vec<HostAdapterTufMetadataFreshnessRole>,
    pub deferred_verification: Vec<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterCertificateCrlStatusVerificationInput {
    pub trust_policy_path: PathBuf,
    pub certificate_path: PathBuf,
    pub issuer_certificate_path: PathBuf,
    pub crl_path: PathBuf,
    pub verification_time_unix: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterCertificateCrlStatusVerification {
    pub status: HostAdapterCertificateCrlStatusVerificationStatus,
    pub trust_policy_path: String,
    pub certificate_path: String,
    pub issuer_certificate_path: String,
    pub crl_path: String,
    pub verification_time_unix: i64,
    pub policy_mode: Option<String>,
    pub certificate_serial_hex: Option<String>,
    pub issuer_subject: Option<String>,
    pub crl_issuer: Option<String>,
    pub crl_this_update_unix: Option<i64>,
    pub crl_next_update_unix: Option<i64>,
    pub revocation_status: Option<String>,
    pub revoked_at_unix: Option<i64>,
    pub revocation_reason: Option<String>,
    pub deferred_verification: Vec<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone)]
pub struct HostAdapterCertificateOcspStatusVerificationInput {
    pub trust_policy_path: PathBuf,
    pub certificate_path: PathBuf,
    pub issuer_certificate_path: PathBuf,
    pub ocsp_response_path: PathBuf,
    pub verification_time_unix: i64,
    pub expected_nonce_hex: Option<OcspNonceHex>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterCertificateOcspStatusVerification {
    pub status: HostAdapterCertificateOcspStatusVerificationStatus,
    pub trust_policy_path: String,
    pub certificate_path: String,
    pub issuer_certificate_path: String,
    pub ocsp_response_path: String,
    pub verification_time_unix: i64,
    pub expected_nonce_hex: Option<OcspNonceHex>,
    pub observed_nonce_hex: Option<OcspNonceHex>,
    pub policy_mode: Option<String>,
    pub certificate_serial_hex: Option<String>,
    pub issuer_subject: Option<String>,
    pub ocsp_response_status: Option<String>,
    pub responder_authority: Option<String>,
    pub ocsp_produced_at_unix: Option<i64>,
    pub ocsp_this_update_unix: Option<i64>,
    pub ocsp_next_update_unix: Option<i64>,
    pub revocation_status: Option<String>,
    pub revoked_at_unix: Option<i64>,
    pub revocation_reason: Option<String>,
    pub deferred_verification: Vec<String>,
    pub reasons: Vec<String>,
    pub verified_evidence: Vec<String>,
    pub inference_boundary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterProjectedCommand {
    pub name: String,
    pub source_command: String,
    pub title: String,
    pub description: String,
    pub mutation_class: HostAdapterMutationClass,
    pub authority_class: HostAdapterAuthorityClass,
    pub safe_auto_invocation_triggers: Vec<HostAdapterAutoTrigger>,
    pub output_treatment: Vec<HostAdapterOutputTreatment>,
    pub mcp_tool: Option<HostAdapterMcpToolProjection>,
    pub borrowed_shell: Option<HostAdapterBorrowedShellProjection>,
    pub app_ui: Option<HostAdapterAppUiProjection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAdapterMcpToolProjection {
    pub name: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub annotations: HostAdapterMcpToolAnnotations,
}

// Mirrors the MCP `ToolAnnotations` schema (read_only/destructive/idempotent/
// open_world). Keeping the bools matches the upstream spec byte-for-byte so
// the host adapter can roundtrip them without remapping.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAdapterMcpToolAnnotations {
    pub title: String,
    pub read_only_hint: bool,
    pub destructive_hint: bool,
    pub idempotent_hint: bool,
    pub open_world_hint: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterBorrowedShellProjection {
    pub argv_prefix: Vec<String>,
    pub json_flag: String,
    pub explicit_invocation_required: bool,
    pub may_auto_invoke: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAdapterAppUiProjection {
    pub surface_group: String,
    pub confirmation_required: bool,
    pub display_authority_badge: String,
}
