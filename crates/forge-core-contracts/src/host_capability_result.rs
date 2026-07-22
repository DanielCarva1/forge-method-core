#![allow(clippy::missing_errors_doc)]

//! Closed, authority-free observations of exact host capabilities.
//!
//! A result is evidence about one observed host/version pair, not an admission
//! decision. It cannot select, support, release, install, trust, mutate, or
//! sign for a host, and contains neither private keys nor key handles.

use crate::{HostKind, StableId};
use schemars::JsonSchema;
use serde::{de, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

pub const HOST_CAPABILITY_RESULT_SCHEMA_VERSION: &str = "0.1";
pub const HOST_CAPABILITY_CASE_DIGEST_DOMAIN: &[u8] =
    b"forge-method:host-capability-conformance-case:v1\0";
pub const HOST_CAPABILITY_RESULT_DIGEST_DOMAIN: &[u8] =
    b"forge-method:host-capability-conformance-result:v1\0";

/// A closed document that records observations without changing host state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostCapabilityResultDocument {
    pub schema_version: String,
    pub host_capability_result: HostCapabilityResult,
}

impl<'de> Deserialize<'de> for HostCapabilityResultDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: String,
            host_capability_result: HostCapabilityResult,
        }

        let document = Wire::deserialize(deserializer)?;
        let document = Self {
            schema_version: document.schema_version,
            host_capability_result: document.host_capability_result,
        };
        document.validate().map_err(de::Error::custom)?;
        Ok(document)
    }
}

impl HostCapabilityResultDocument {
    pub fn validate(&self) -> Result<(), HostCapabilityResultValidationError> {
        if self.schema_version != HOST_CAPABILITY_RESULT_SCHEMA_VERSION {
            return Err(
                HostCapabilityResultValidationError::UnsupportedSchemaVersion(
                    self.schema_version.clone(),
                ),
            );
        }
        self.host_capability_result.validate()
    }
}

/// The observation is permanently candidate-only and authority-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostCapabilityResult {
    pub result_id: StableId,
    pub authority: HostCapabilityResultAuthority,
    pub observation: ExactHostObservation,
    /// This is always `None`; a result may not select a host.
    pub selected_host: Option<HostKind>,
    /// This is always false; individual observations are not a support claim.
    pub supported: bool,
    /// This is always false; individual observations are not a release claim.
    pub released: bool,
    pub manifest_recognition: HostCapabilityFinding,
    pub installability: HostCapabilityFinding,
    pub read_only_mcp: HostCapabilityFinding,
    pub native_human_origin_assurance: HostCapabilityFinding,
    pub governed_mutation: HostCapabilityFinding,
    pub signer_isolation: HostCapabilityFinding,
    pub lifecycle: HostCapabilityFinding,
    pub field_evidence: Vec<HostCapabilityFieldEvidence>,
    pub known_limitations: Vec<HostCapabilityKnownLimitation>,
}

impl<'de> Deserialize<'de> for HostCapabilityResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            result_id: StableId,
            authority: HostCapabilityResultAuthority,
            observation: ExactHostObservation,
            selected_host: Option<HostKind>,
            supported: bool,
            released: bool,
            manifest_recognition: HostCapabilityFinding,
            installability: HostCapabilityFinding,
            read_only_mcp: HostCapabilityFinding,
            native_human_origin_assurance: HostCapabilityFinding,
            governed_mutation: HostCapabilityFinding,
            signer_isolation: HostCapabilityFinding,
            lifecycle: HostCapabilityFinding,
            field_evidence: Vec<HostCapabilityFieldEvidence>,
            known_limitations: Vec<HostCapabilityKnownLimitation>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let result = Self {
            result_id: wire.result_id,
            authority: wire.authority,
            observation: wire.observation,
            selected_host: wire.selected_host,
            supported: wire.supported,
            released: wire.released,
            manifest_recognition: wire.manifest_recognition,
            installability: wire.installability,
            read_only_mcp: wire.read_only_mcp,
            native_human_origin_assurance: wire.native_human_origin_assurance,
            governed_mutation: wire.governed_mutation,
            signer_isolation: wire.signer_isolation,
            lifecycle: wire.lifecycle,
            field_evidence: wire.field_evidence,
            known_limitations: wire.known_limitations,
        };
        result.validate().map_err(de::Error::custom)?;
        Ok(result)
    }
}

impl HostCapabilityResult {
    pub fn validate(&self) -> Result<(), HostCapabilityResultValidationError> {
        validate_stable_id("result_id", &self.result_id)?;
        if self.authority != HostCapabilityResultAuthority::ObservationOnly {
            return Err(HostCapabilityResultValidationError::AuthorityMustBeObservationOnly);
        }
        self.observation.validate()?;
        if self.selected_host.is_some() {
            return Err(HostCapabilityResultValidationError::SelectedHostMustBeNone);
        }
        if self.supported {
            return Err(HostCapabilityResultValidationError::SupportClaimForbidden);
        }
        if self.released {
            return Err(HostCapabilityResultValidationError::ReleaseClaimForbidden);
        }

        for evidence in &self.field_evidence {
            evidence.validate()?;
        }
        for limitation in &self.known_limitations {
            limitation.validate()?;
        }

        self.validate_finding(
            HostCapabilityKind::ManifestRecognition,
            &self.manifest_recognition,
        )?;
        self.validate_finding(HostCapabilityKind::Installability, &self.installability)?;
        self.validate_finding(HostCapabilityKind::ReadOnlyMcp, &self.read_only_mcp)?;
        self.validate_finding(
            HostCapabilityKind::NativeHumanOriginAssurance,
            &self.native_human_origin_assurance,
        )?;
        self.validate_finding(
            HostCapabilityKind::GovernedMutation,
            &self.governed_mutation,
        )?;
        self.validate_finding(HostCapabilityKind::SignerIsolation, &self.signer_isolation)?;
        self.validate_finding(HostCapabilityKind::Lifecycle, &self.lifecycle)?;
        Ok(())
    }

    fn validate_finding(
        &self,
        expected_capability: HostCapabilityKind,
        finding: &HostCapabilityFinding,
    ) -> Result<(), HostCapabilityResultValidationError> {
        if finding.capability != expected_capability {
            return Err(HostCapabilityResultValidationError::FindingCapabilityMismatch);
        }
        finding.validate(&self.observation)?;
        for evidence_id in finding.evidence_ids.iter().chain(
            finding
                .conformance_case_results
                .iter()
                .flat_map(|case| case.evidence_ids.iter()),
        ) {
            if !self
                .field_evidence
                .iter()
                .any(|evidence| &evidence.evidence_id == evidence_id)
            {
                return Err(HostCapabilityResultValidationError::ReferencedEvidenceMissing);
            }
        }
        for limitation_id in finding.limitation_ids.iter().chain(
            finding
                .conformance_case_results
                .iter()
                .flat_map(|case| case.limitation_ids.iter()),
        ) {
            if !self
                .known_limitations
                .iter()
                .any(|limitation| &limitation.limitation_id == limitation_id)
            {
                return Err(HostCapabilityResultValidationError::ReferencedLimitationMissing);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostCapabilityResultAuthority {
    ObservationOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExactHostObservation {
    pub host: HostKind,
    pub host_version: String,
    pub observation_id: StableId,
    pub observed_at_unix: u64,
}

impl ExactHostObservation {
    fn validate(&self) -> Result<(), HostCapabilityResultValidationError> {
        validate_stable_id("observation_id", &self.observation_id)?;
        if !crate::is_exact_host_version(&self.host_version) {
            return Err(HostCapabilityResultValidationError::EmptyHostVersion);
        }
        Ok(())
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HostCapabilityKind {
    ManifestRecognition,
    Installability,
    ReadOnlyMcp,
    NativeHumanOriginAssurance,
    GovernedMutation,
    SignerIsolation,
    Lifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostCapabilityOutcome {
    Supported,
    Unsupported,
    Unknown,
}

/// The minimum provenance needed to make an outcome truthful rather than a
/// prediction or an implicit admission decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostCapabilityOutcomeBasis {
    DirectFieldEvidence,
    DeclaredHostLimitation,
    NotObserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostCapabilityFinding {
    pub capability: HostCapabilityKind,
    pub outcome: HostCapabilityOutcome,
    pub outcome_basis: HostCapabilityOutcomeBasis,
    pub evidence_ids: Vec<StableId>,
    pub limitation_ids: Vec<StableId>,
    pub conformance_case_results: Vec<HostConformanceCaseResult>,
}

impl HostCapabilityFinding {
    fn validate(
        &self,
        observation: &ExactHostObservation,
    ) -> Result<(), HostCapabilityResultValidationError> {
        for evidence_id in &self.evidence_ids {
            validate_stable_id("evidence_id", evidence_id)?;
        }
        for limitation_id in &self.limitation_ids {
            validate_stable_id("limitation_id", limitation_id)?;
        }
        match (self.outcome, self.outcome_basis) {
            (HostCapabilityOutcome::Supported, HostCapabilityOutcomeBasis::DirectFieldEvidence) => {
                if self.evidence_ids.is_empty() {
                    return Err(HostCapabilityResultValidationError::SupportedRequiresEvidence);
                }
            }
            (
                HostCapabilityOutcome::Unsupported,
                HostCapabilityOutcomeBasis::DirectFieldEvidence,
            ) => {
                if self.evidence_ids.is_empty() {
                    return Err(HostCapabilityResultValidationError::UnsupportedRequiresEvidence);
                }
            }
            (
                HostCapabilityOutcome::Unsupported,
                HostCapabilityOutcomeBasis::DeclaredHostLimitation,
            ) => {
                if self.limitation_ids.is_empty() {
                    return Err(HostCapabilityResultValidationError::UnsupportedRequiresLimitation);
                }
            }
            (HostCapabilityOutcome::Unknown, HostCapabilityOutcomeBasis::NotObserved) => {}
            _ => return Err(HostCapabilityResultValidationError::UntruthfulOutcomeBasis),
        }
        for result in &self.conformance_case_results {
            if result.capability != self.capability {
                return Err(HostCapabilityResultValidationError::CaseCapabilityMismatch);
            }
            result.validate(observation)?;
        }
        Ok(())
    }
}

/// Content-free field evidence. References identify observed fields without
/// retaining host transcripts, environment values, credentials, or key data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostCapabilityFieldEvidence {
    pub evidence_id: StableId,
    pub source: HostCapabilityEvidenceSource,
    pub field: HostCapabilityEvidenceField,
    pub observed_at_unix: u64,
    pub source_digest: String,
}

impl HostCapabilityFieldEvidence {
    fn validate(&self) -> Result<(), HostCapabilityResultValidationError> {
        validate_stable_id("evidence_id", &self.evidence_id)?;
        validate_sha256("source_digest", &self.source_digest)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostCapabilityEvidenceSource {
    Manifest,
    ReadOnlyMcp,
    NativeHumanOriginBoundary,
    GovernedMutationBoundary,
    SignerIsolationBoundary,
    LifecycleBoundary,
    FieldInspection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostCapabilityEvidenceField {
    ManifestIdentity,
    ManifestVersion,
    InstallInstruction,
    McpReadOnlyMode,
    NativeHumanOriginSignal,
    GovernedMutationAdmissionBoundary,
    NonExportableSignerBoundary,
    LifecycleState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostCapabilityKnownLimitation {
    pub limitation_id: StableId,
    pub kind: HostCapabilityLimitationKind,
    pub observed_at_unix: u64,
    pub evidence_digest: String,
}

impl HostCapabilityKnownLimitation {
    fn validate(&self) -> Result<(), HostCapabilityResultValidationError> {
        validate_stable_id("limitation_id", &self.limitation_id)?;
        validate_sha256("evidence_digest", &self.evidence_digest)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostCapabilityLimitationKind {
    ManifestNotRecognized,
    InstallationNotAvailable,
    McpNotReadOnly,
    HumanOriginNotAssured,
    GovernedMutationNotAvailable,
    SignerNotIsolated,
    LifecycleNotObserved,
    ExactVersionNotObserved,
}

/// One content-addressed conformance result for an exact host observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostConformanceCaseResult {
    pub case_id: StableId,
    pub result_id: StableId,
    pub capability: HostCapabilityKind,
    pub expected_outcome: HostCapabilityOutcome,
    pub observed_outcome: HostCapabilityOutcome,
    pub observed_at_unix: u64,
    pub evidence_ids: Vec<StableId>,
    pub limitation_ids: Vec<StableId>,
    /// `sha256:` followed by exactly 64 lowercase hexadecimal characters.
    pub case_digest: String,
    /// `sha256:` followed by exactly 64 lowercase hexadecimal characters.
    pub result_digest: String,
}

impl HostConformanceCaseResult {
    pub fn canonical_case_digest(
        &self,
        observation: &ExactHostObservation,
    ) -> Result<String, HostCapabilityResultValidationError> {
        #[derive(Serialize)]
        struct CanonicalCaseInput<'a> {
            case_id: &'a StableId,
            host: HostKind,
            host_version: &'a str,
            capability: HostCapabilityKind,
            expected_outcome: HostCapabilityOutcome,
        }
        canonical_digest(
            HOST_CAPABILITY_CASE_DIGEST_DOMAIN,
            &CanonicalCaseInput {
                case_id: &self.case_id,
                host: observation.host,
                host_version: &observation.host_version,
                capability: self.capability,
                expected_outcome: self.expected_outcome,
            },
        )
    }

    pub fn canonical_result_digest(
        &self,
        observation: &ExactHostObservation,
    ) -> Result<String, HostCapabilityResultValidationError> {
        #[derive(Serialize)]
        struct CanonicalResultInput<'a> {
            case_digest: &'a str,
            result_id: &'a StableId,
            host: HostKind,
            host_version: &'a str,
            capability: HostCapabilityKind,
            expected_outcome: HostCapabilityOutcome,
            observed_outcome: HostCapabilityOutcome,
            observed_at_unix: u64,
            evidence_ids: &'a [StableId],
            limitation_ids: &'a [StableId],
        }
        canonical_digest(
            HOST_CAPABILITY_RESULT_DIGEST_DOMAIN,
            &CanonicalResultInput {
                case_digest: &self.case_digest,
                result_id: &self.result_id,
                host: observation.host,
                host_version: &observation.host_version,
                capability: self.capability,
                expected_outcome: self.expected_outcome,
                observed_outcome: self.observed_outcome,
                observed_at_unix: self.observed_at_unix,
                evidence_ids: &self.evidence_ids,
                limitation_ids: &self.limitation_ids,
            },
        )
    }

    fn validate(
        &self,
        observation: &ExactHostObservation,
    ) -> Result<(), HostCapabilityResultValidationError> {
        validate_stable_id("case_id", &self.case_id)?;
        validate_stable_id("conformance result_id", &self.result_id)?;
        for evidence_id in &self.evidence_ids {
            validate_stable_id("case evidence_id", evidence_id)?;
        }
        for limitation_id in &self.limitation_ids {
            validate_stable_id("case limitation_id", limitation_id)?;
        }
        validate_sha256("case_digest", &self.case_digest)?;
        validate_sha256("result_digest", &self.result_digest)?;
        if self.case_digest != self.canonical_case_digest(observation)? {
            return Err(HostCapabilityResultValidationError::CaseDigestMismatch);
        }
        if self.result_digest != self.canonical_result_digest(observation)? {
            return Err(HostCapabilityResultValidationError::ResultDigestMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCapabilityResultValidationError {
    UnsupportedSchemaVersion(String),
    AuthorityMustBeObservationOnly,
    SelectedHostMustBeNone,
    SupportClaimForbidden,
    ReleaseClaimForbidden,
    EmptyHostVersion,
    InvalidStableId(&'static str),
    InvalidSha256Digest(&'static str),
    FindingCapabilityMismatch,
    CaseCapabilityMismatch,
    ReferencedEvidenceMissing,
    ReferencedLimitationMissing,
    SupportedRequiresEvidence,
    UnsupportedRequiresEvidence,
    UnsupportedRequiresLimitation,
    UntruthfulOutcomeBasis,
    CaseDigestMismatch,
    ResultDigestMismatch,
    Canonicalization(String),
}

impl std::fmt::Display for HostCapabilityResultValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported host capability result schema '{version}'"
                )
            }
            Self::AuthorityMustBeObservationOnly => {
                formatter.write_str("authority must be observation_only")
            }
            Self::SelectedHostMustBeNone => formatter.write_str("selected_host must be none"),
            Self::SupportClaimForbidden => formatter.write_str("host support claim is forbidden"),
            Self::ReleaseClaimForbidden => formatter.write_str("host release claim is forbidden"),
            Self::EmptyHostVersion => {
                formatter.write_str("host_version must be exact and non-empty")
            }
            Self::InvalidStableId(field) => write!(formatter, "{field} must be a stable id"),
            Self::InvalidSha256Digest(field) => {
                write!(
                    formatter,
                    "{field} must be an exact lowercase sha256 digest"
                )
            }
            Self::FindingCapabilityMismatch => {
                formatter.write_str("finding capability does not match field")
            }
            Self::CaseCapabilityMismatch => {
                formatter.write_str("case capability does not match finding")
            }
            Self::ReferencedEvidenceMissing => {
                formatter.write_str("referenced field evidence is absent")
            }
            Self::ReferencedLimitationMissing => {
                formatter.write_str("referenced known limitation is absent")
            }
            Self::SupportedRequiresEvidence => {
                formatter.write_str("supported outcome requires field evidence")
            }
            Self::UnsupportedRequiresEvidence => {
                formatter.write_str("direct unsupported outcome requires field evidence")
            }
            Self::UnsupportedRequiresLimitation => {
                formatter.write_str("declared unsupported outcome requires a limitation")
            }
            Self::UntruthfulOutcomeBasis => {
                formatter.write_str("outcome and basis are inconsistent")
            }
            Self::CaseDigestMismatch => formatter.write_str("conformance case digest mismatch"),
            Self::ResultDigestMismatch => formatter.write_str("conformance result digest mismatch"),
            Self::Canonicalization(error) => write!(formatter, "canonicalization failed: {error}"),
        }
    }
}

impl std::error::Error for HostCapabilityResultValidationError {}

fn validate_stable_id(
    field: &'static str,
    stable_id: &StableId,
) -> Result<(), HostCapabilityResultValidationError> {
    let value = &stable_id.0;
    let valid = !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-')
                || (index > 0 && byte == b':')
        });
    if valid {
        Ok(())
    } else {
        Err(HostCapabilityResultValidationError::InvalidStableId(field))
    }
}

fn validate_sha256(
    field: &'static str,
    digest: &str,
) -> Result<(), HostCapabilityResultValidationError> {
    if crate::is_lowercase_sha256_digest(digest) {
        Ok(())
    } else {
        Err(HostCapabilityResultValidationError::InvalidSha256Digest(
            field,
        ))
    }
}

fn canonical_digest<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<String, HostCapabilityResultValidationError> {
    let value = serde_json::to_value(value).map_err(|error| {
        HostCapabilityResultValidationError::Canonicalization(error.to_string())
    })?;
    let canonical = serde_json_canonicalizer::to_vec(&value).map_err(|error| {
        HostCapabilityResultValidationError::Canonicalization(error.to_string())
    })?;
    let length = u64::try_from(canonical.len()).map_err(|_| {
        HostCapabilityResultValidationError::Canonicalization(
            "canonical value is too large".to_owned(),
        )
    })?;
    let mut bytes = Vec::with_capacity(domain.len() + 8 + canonical.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(&canonical);
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stable_id(value: &str) -> StableId {
        StableId(value.to_owned())
    }

    fn observation() -> ExactHostObservation {
        ExactHostObservation {
            host: HostKind::ForgeApp,
            host_version: "1.2.3".to_owned(),
            observation_id: stable_id("host-observation.alpha"),
            observed_at_unix: 1_900_000_000,
        }
    }

    fn finding(capability: HostCapabilityKind) -> HostCapabilityFinding {
        HostCapabilityFinding {
            capability,
            outcome: HostCapabilityOutcome::Unknown,
            outcome_basis: HostCapabilityOutcomeBasis::NotObserved,
            evidence_ids: Vec::new(),
            limitation_ids: Vec::new(),
            conformance_case_results: Vec::new(),
        }
    }

    fn result() -> HostCapabilityResult {
        HostCapabilityResult {
            result_id: stable_id("host-capability-result.alpha"),
            authority: HostCapabilityResultAuthority::ObservationOnly,
            observation: observation(),
            selected_host: None,
            supported: false,
            released: false,
            manifest_recognition: finding(HostCapabilityKind::ManifestRecognition),
            installability: finding(HostCapabilityKind::Installability),
            read_only_mcp: finding(HostCapabilityKind::ReadOnlyMcp),
            native_human_origin_assurance: finding(HostCapabilityKind::NativeHumanOriginAssurance),
            governed_mutation: finding(HostCapabilityKind::GovernedMutation),
            signer_isolation: finding(HostCapabilityKind::SignerIsolation),
            lifecycle: finding(HostCapabilityKind::Lifecycle),
            field_evidence: Vec::new(),
            known_limitations: Vec::new(),
        }
    }

    #[test]
    fn document_deserialization_rejects_host_admission_claims() {
        let document = HostCapabilityResultDocument {
            schema_version: HOST_CAPABILITY_RESULT_SCHEMA_VERSION.to_owned(),
            host_capability_result: result(),
        };
        let mut selected = serde_json::to_value(&document).expect("document JSON");
        selected["host_capability_result"]["selected_host"] = serde_json::json!("forge_app");
        assert!(serde_json::from_value::<HostCapabilityResultDocument>(selected).is_err());

        let mut supported = serde_json::to_value(&document).expect("document JSON");
        supported["host_capability_result"]["supported"] = serde_json::json!(true);
        assert!(serde_json::from_value::<HostCapabilityResultDocument>(supported).is_err());

        let mut released = serde_json::to_value(&document).expect("document JSON");
        released["host_capability_result"]["released"] = serde_json::json!(true);
        assert!(serde_json::from_value::<HostCapabilityResultDocument>(released).is_err());
    }

    #[test]
    fn conformance_digests_are_canonical_lowercase_sha256_and_bind_host_version() {
        let observed = observation();
        let mut case = HostConformanceCaseResult {
            case_id: stable_id("host-case.manifest-recognition"),
            result_id: stable_id("host-case-result.manifest-recognition"),
            capability: HostCapabilityKind::ManifestRecognition,
            expected_outcome: HostCapabilityOutcome::Supported,
            observed_outcome: HostCapabilityOutcome::Supported,
            observed_at_unix: 1_900_000_001,
            evidence_ids: vec![stable_id("field-evidence.manifest")],
            limitation_ids: Vec::new(),
            case_digest: String::new(),
            result_digest: String::new(),
        };
        case.case_digest = case.canonical_case_digest(&observed).expect("case digest");
        case.result_digest = case
            .canonical_result_digest(&observed)
            .expect("result digest");
        case.validate(&observed).expect("canonical case result");
        assert!(case.case_digest.starts_with("sha256:"));
        assert!(case.result_digest.starts_with("sha256:"));

        let mut other_version = observed;
        other_version.host_version = "1.2.4".to_owned();
        assert_ne!(
            case.case_digest,
            case.canonical_case_digest(&other_version)
                .expect("changed case digest")
        );

        case.result_digest = case.result_digest.to_uppercase();
        assert!(matches!(
            case.validate(&observation()),
            Err(HostCapabilityResultValidationError::InvalidSha256Digest(
                "result_digest"
            ))
        ));
    }

    #[test]
    fn closed_deserialization_rejects_private_or_mutation_authority_fields() {
        let document = HostCapabilityResultDocument {
            schema_version: HOST_CAPABILITY_RESULT_SCHEMA_VERSION.to_owned(),
            host_capability_result: result(),
        };
        for forbidden_field in ["private_key", "private_key_grant", "mutation_authority"] {
            let mut value = serde_json::to_value(&document).expect("document JSON");
            value["host_capability_result"][forbidden_field] = serde_json::json!(true);
            assert!(
                serde_json::from_value::<HostCapabilityResultDocument>(value).is_err(),
                "{forbidden_field} must fail closed"
            );
        }
    }

    #[test]
    fn deserialization_rejects_non_exact_host_versions() {
        let document = HostCapabilityResultDocument {
            schema_version: HOST_CAPABILITY_RESULT_SCHEMA_VERSION.to_owned(),
            host_capability_result: result(),
        };
        for version in ["^1.2", "unknown", "unobserved", "latest", "current"] {
            let mut value = serde_json::to_value(&document).expect("document JSON");
            value["host_capability_result"]["observation"]["host_version"] =
                serde_json::json!(version);
            assert!(
                serde_json::from_value::<HostCapabilityResultDocument>(value).is_err(),
                "{version} must not identify an exact observed host"
            );
        }
    }
}
