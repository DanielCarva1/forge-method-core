#![allow(clippy::missing_errors_doc)]

//! Closed C4.2 candidate-only host support matrix contracts.
//!
//! A matrix record reports an exact runtime kind and version, but is never a
//! selection, admission, installation, release, or authority decision. In
//! particular, serializing this document cannot grant support, release,
//! install, mutation, signing, trust, or host-selection authority.

use crate::RuntimeKind;
use schemars::JsonSchema;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub const HOST_SUPPORT_MATRIX_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostSupportMatrixDocument {
    pub schema_version: String,
    pub host_support_matrix: HostSupportMatrix,
}

impl<'de> Deserialize<'de> for HostSupportMatrixDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: String,
            host_support_matrix: HostSupportMatrix,
        }

        let wire = Wire::deserialize(deserializer)?;
        let document = Self {
            schema_version: wire.schema_version,
            host_support_matrix: wire.host_support_matrix,
        };
        document.validate().map_err(de::Error::custom)?;
        Ok(document)
    }
}

impl HostSupportMatrixDocument {
    pub fn validate(&self) -> Result<(), HostSupportMatrixValidationError> {
        if self.schema_version != HOST_SUPPORT_MATRIX_SCHEMA_VERSION {
            return Err(HostSupportMatrixValidationError::SchemaVersion);
        }
        self.host_support_matrix.validate()
    }
}

/// A closed, read-only candidate assessment. `selected_host` is deliberately
/// present to make the prohibition observable and must always be `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostSupportMatrix {
    pub matrix_id: String,
    pub authority: HostSupportMatrixAuthority,
    pub selected_host: Option<RuntimeKind>,
    pub serialization_boundary: SerializationAuthorityBoundary,
    pub records: Vec<HostSupportRecord>,
}

impl HostSupportMatrix {
    pub fn validate(&self) -> Result<(), HostSupportMatrixValidationError> {
        if self.matrix_id.trim().is_empty() {
            return Err(HostSupportMatrixValidationError::EmptyMatrixId);
        }
        if self.authority != HostSupportMatrixAuthority::CandidateOnly {
            return Err(HostSupportMatrixValidationError::NonCandidateAuthority);
        }
        if self.selected_host.is_some() {
            return Err(HostSupportMatrixValidationError::SelectedHostMustBeNone);
        }
        if !self.serialization_boundary.is_non_authoritative() {
            return Err(HostSupportMatrixValidationError::SerializationAuthority);
        }

        let mut targets = BTreeSet::new();
        for record in &self.records {
            record.validate()?;
            let key = format!("{:?}\u{0}{}", record.runtime.kind, record.runtime.version);
            if !targets.insert(key) {
                return Err(HostSupportMatrixValidationError::DuplicateRuntime);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostSupportMatrixAuthority {
    CandidateOnly,
}

/// Explicitly records that a wire representation is not an authority grant.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SerializationAuthorityBoundary {
    pub grants_support_authority: bool,
    pub grants_release_authority: bool,
    pub grants_install_authority: bool,
    pub grants_mutation_authority: bool,
    pub grants_signing_authority: bool,
    pub grants_trust_authority: bool,
    /// Private broker keys are never retained in candidate records or backups.
    pub grants_private_key_authority: bool,
    pub grants_host_selection_authority: bool,
}

impl SerializationAuthorityBoundary {
    #[must_use]
    pub const fn non_authoritative() -> Self {
        Self {
            grants_support_authority: false,
            grants_release_authority: false,
            grants_install_authority: false,
            grants_mutation_authority: false,
            grants_signing_authority: false,
            grants_trust_authority: false,
            grants_private_key_authority: false,
            grants_host_selection_authority: false,
        }
    }

    const fn is_non_authoritative(&self) -> bool {
        !self.grants_support_authority
            && !self.grants_release_authority
            && !self.grants_install_authority
            && !self.grants_mutation_authority
            && !self.grants_signing_authority
            && !self.grants_trust_authority
            && !self.grants_private_key_authority
            && !self.grants_host_selection_authority
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostSupportRecord {
    pub runtime: ExactRuntime,
    pub status: HostSupportStatus,
    pub recognition: CapabilityAssessment,
    pub installability: CapabilityAssessment,
    pub read_only_mcp: CapabilityAssessment,
    pub native_human_origin_assurance: CapabilityAssessment,
    pub governed_mutation: CapabilityAssessment,
    pub conformance: CapabilityAssessment,
    pub release_asset: CapabilityAssessment,
    pub field_evidence: CapabilityAssessment,
    pub known_limits: KnownLimitsReport,
}

impl HostSupportRecord {
    fn validate(&self) -> Result<(), HostSupportMatrixValidationError> {
        self.runtime.validate()?;

        let assessments = [
            (HostSupportDimension::Recognition, &self.recognition),
            (HostSupportDimension::Installability, &self.installability),
            (HostSupportDimension::ReadOnlyMcp, &self.read_only_mcp),
            (
                HostSupportDimension::NativeHumanOriginAssurance,
                &self.native_human_origin_assurance,
            ),
            (
                HostSupportDimension::GovernedMutation,
                &self.governed_mutation,
            ),
            (HostSupportDimension::Conformance, &self.conformance),
            (HostSupportDimension::ReleaseAsset, &self.release_asset),
            (HostSupportDimension::FieldEvidence, &self.field_evidence),
        ];

        let mut evidence_ids = BTreeSet::new();
        for (dimension, assessment) in assessments {
            assessment.validate(dimension, &self.runtime)?;
            if let Some(evidence) = &assessment.evidence {
                if !evidence_ids.insert(evidence.identity()) {
                    return Err(HostSupportMatrixValidationError::EvidenceNotIndependent);
                }
            }
        }
        self.known_limits
            .validate(&self.runtime, &mut evidence_ids)?;

        if self.status == HostSupportStatus::Supported
            && assessments.iter().any(|(_, assessment)| {
                assessment.status != HostSupportStatus::Supported || assessment.evidence.is_none()
            })
        {
            return Err(HostSupportMatrixValidationError::SupportedWithoutCompleteEvidence);
        }
        Ok(())
    }
}

/// The exact runtime target of a candidate record. Version ranges, wildcards,
/// and surrounding whitespace are rejected so evidence cannot drift between
/// host versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExactRuntime {
    pub kind: RuntimeKind,
    pub version: String,
}

impl ExactRuntime {
    fn validate(&self) -> Result<(), HostSupportMatrixValidationError> {
        if !crate::is_exact_host_version(&self.version) {
            return Err(HostSupportMatrixValidationError::NonExactVersion);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostSupportStatus {
    Candidate,
    Unsupported,
    Unknown,
    Supported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostSupportDimension {
    Recognition,
    Installability,
    ReadOnlyMcp,
    NativeHumanOriginAssurance,
    GovernedMutation,
    Conformance,
    ReleaseAsset,
    FieldEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityAssessment {
    pub status: HostSupportStatus,
    pub evidence: Option<ExactHostEvidence>,
}

impl CapabilityAssessment {
    fn validate(
        &self,
        _dimension: HostSupportDimension,
        runtime: &ExactRuntime,
    ) -> Result<(), HostSupportMatrixValidationError> {
        if self.status == HostSupportStatus::Supported && self.evidence.is_none() {
            return Err(HostSupportMatrixValidationError::SupportedWithoutDimensionEvidence);
        }
        if let Some(evidence) = &self.evidence {
            evidence.validate_for(runtime)?;
        }
        Ok(())
    }
}

/// Independently supplied evidence bound to one exact runtime target. An
/// external broker's private key is intentionally not representable here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExactHostEvidence {
    pub evidence_id: String,
    pub observed_runtime: ExactRuntime,
    pub evidence_ref: String,
    pub evidence_digest: String,
}

impl ExactHostEvidence {
    fn validate_for(&self, runtime: &ExactRuntime) -> Result<(), HostSupportMatrixValidationError> {
        if self.evidence_id.trim().is_empty() || self.evidence_ref.trim().is_empty() {
            return Err(HostSupportMatrixValidationError::EmptyEvidenceField);
        }
        if !crate::is_lowercase_sha256_digest(&self.evidence_digest) {
            return Err(HostSupportMatrixValidationError::InvalidEvidenceDigest);
        }
        self.observed_runtime.validate()?;
        if &self.observed_runtime != runtime {
            return Err(HostSupportMatrixValidationError::EvidenceRuntimeMismatch);
        }
        Ok(())
    }

    fn identity(&self) -> String {
        // A reference identifies the independently supplied artifact. Changing
        // its display id or digest cannot make one artifact satisfy two
        // dimensions.
        self.evidence_ref.clone()
    }
}

/// A distinct report prevents unbounded optimism: it states whether limits are
/// still unknown and, when known, carries exact-target disclosure evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KnownLimitsReport {
    pub status: KnownLimitsStatus,
    pub limits: Vec<String>,
    pub disclosure_evidence: Option<ExactHostEvidence>,
}

impl KnownLimitsReport {
    fn validate(
        &self,
        runtime: &ExactRuntime,
        evidence_ids: &mut BTreeSet<String>,
    ) -> Result<(), HostSupportMatrixValidationError> {
        if self.status == KnownLimitsStatus::Known && self.disclosure_evidence.is_none() {
            return Err(HostSupportMatrixValidationError::KnownLimitsWithoutEvidence);
        }
        if self.status == KnownLimitsStatus::NoneKnown && !self.limits.is_empty() {
            return Err(HostSupportMatrixValidationError::LimitsContradictNoneKnown);
        }
        if let Some(evidence) = &self.disclosure_evidence {
            evidence.validate_for(runtime)?;
            if !evidence_ids.insert(evidence.identity()) {
                return Err(HostSupportMatrixValidationError::EvidenceNotIndependent);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KnownLimitsStatus {
    Unknown,
    Known,
    NoneKnown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostSupportMatrixValidationError {
    SchemaVersion,
    EmptyMatrixId,
    NonCandidateAuthority,
    SelectedHostMustBeNone,
    SerializationAuthority,
    DuplicateRuntime,
    NonExactVersion,
    SupportedWithoutDimensionEvidence,
    SupportedWithoutCompleteEvidence,
    EmptyEvidenceField,
    InvalidEvidenceDigest,
    EvidenceRuntimeMismatch,
    EvidenceNotIndependent,
    KnownLimitsWithoutEvidence,
    LimitsContradictNoneKnown,
}

impl fmt::Display for HostSupportMatrixValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::SchemaVersion => "unsupported host support matrix schema version",
            Self::EmptyMatrixId => "host support matrix id must not be empty",
            Self::NonCandidateAuthority => "host support matrix authority must be candidate only",
            Self::SelectedHostMustBeNone => "host support matrix selected_host must be none",
            Self::SerializationAuthority => "serialization must not grant host authority",
            Self::DuplicateRuntime => "host support matrix has a duplicate exact runtime",
            Self::NonExactVersion => "runtime version must be an exact version",
            Self::SupportedWithoutDimensionEvidence => {
                "a supported dimension requires exact host evidence"
            }
            Self::SupportedWithoutCompleteEvidence => {
                "a supported host requires independent exact evidence for every dimension"
            }
            Self::EmptyEvidenceField => "host evidence fields must not be empty",
            Self::InvalidEvidenceDigest => {
                "host evidence digest must be an exact lowercase sha256 digest"
            }
            Self::EvidenceRuntimeMismatch => "host evidence does not match the exact runtime",
            Self::EvidenceNotIndependent => "host evidence must be independent per dimension",
            Self::KnownLimitsWithoutEvidence => "known limits require exact host evidence",
            Self::LimitsContradictNoneKnown => "none_known limits cannot include entries",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for HostSupportMatrixValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> ExactRuntime {
        ExactRuntime {
            kind: RuntimeKind::Claude,
            version: "1.2.3".to_owned(),
        }
    }

    fn evidence(id: &str) -> ExactHostEvidence {
        ExactHostEvidence {
            evidence_id: id.to_owned(),
            observed_runtime: target(),
            evidence_ref: format!("evidence/{id}.json"),
            evidence_digest: format!("sha256:{}", "a".repeat(64)),
        }
    }

    fn assessment(status: HostSupportStatus, evidence_id: Option<&str>) -> CapabilityAssessment {
        CapabilityAssessment {
            status,
            evidence: evidence_id.map(evidence),
        }
    }

    fn record(status: HostSupportStatus) -> HostSupportRecord {
        HostSupportRecord {
            runtime: target(),
            status,
            recognition: assessment(status, Some("recognition")),
            installability: assessment(status, Some("installability")),
            read_only_mcp: assessment(status, Some("read-only-mcp")),
            native_human_origin_assurance: assessment(status, Some("human-origin")),
            governed_mutation: assessment(status, Some("governed-mutation")),
            conformance: assessment(status, Some("conformance")),
            release_asset: assessment(status, Some("release-asset")),
            field_evidence: assessment(status, Some("field-evidence")),
            known_limits: KnownLimitsReport {
                status: KnownLimitsStatus::Unknown,
                limits: Vec::new(),
                disclosure_evidence: None,
            },
        }
    }

    fn document(record: HostSupportRecord) -> HostSupportMatrixDocument {
        HostSupportMatrixDocument {
            schema_version: HOST_SUPPORT_MATRIX_SCHEMA_VERSION.to_owned(),
            host_support_matrix: HostSupportMatrix {
                matrix_id: "candidate-host-support".to_owned(),
                authority: HostSupportMatrixAuthority::CandidateOnly,
                selected_host: None,
                serialization_boundary: SerializationAuthorityBoundary::non_authoritative(),
                records: vec![record],
            },
        }
    }

    #[test]
    fn candidate_record_is_closed_and_non_authoritative() {
        let value = serde_json::to_value(document(record(HostSupportStatus::Candidate)))
            .expect("serialize candidate matrix");
        let parsed: HostSupportMatrixDocument =
            serde_json::from_value(value).expect("deserialize candidate matrix");
        parsed.validate().expect("candidate matrix remains valid");
    }

    #[test]
    fn supported_requires_independent_exact_evidence_for_every_dimension() {
        let mut supported = record(HostSupportStatus::Supported);
        supported.conformance.evidence = supported.recognition.evidence.clone();
        let error = document(supported)
            .validate()
            .expect_err("reused evidence is not independent");
        assert_eq!(
            error,
            HostSupportMatrixValidationError::EvidenceNotIndependent
        );
    }

    #[test]
    fn supported_rejects_evidence_for_another_exact_version() {
        let mut supported = record(HostSupportStatus::Supported);
        supported
            .release_asset
            .evidence
            .as_mut()
            .expect("evidence")
            .observed_runtime
            .version = "1.2.4".to_owned();
        let error = document(supported)
            .validate()
            .expect_err("different version must fail");
        assert_eq!(
            error,
            HostSupportMatrixValidationError::EvidenceRuntimeMismatch
        );
    }

    #[test]
    fn placeholder_versions_are_not_exact_runtime_targets() {
        for placeholder in ["unknown", "unobserved", "latest", "current", "n/a"] {
            let mut candidate = record(HostSupportStatus::Candidate);
            candidate.runtime.version = placeholder.to_owned();
            let error = document(candidate)
                .validate()
                .expect_err("placeholder version must fail closed");
            assert_eq!(error, HostSupportMatrixValidationError::NonExactVersion);
        }
    }

    #[test]
    fn evidence_requires_canonical_lowercase_sha256() {
        let mut candidate = record(HostSupportStatus::Candidate);
        candidate
            .recognition
            .evidence
            .as_mut()
            .expect("recognition evidence")
            .evidence_digest = "sha256:not-a-digest".to_owned();
        let error = document(candidate)
            .validate()
            .expect_err("symbolic evidence digest must fail closed");
        assert_eq!(
            error,
            HostSupportMatrixValidationError::InvalidEvidenceDigest
        );
    }

    #[test]
    fn selected_host_cannot_be_serialized_as_a_matrix_decision() {
        let mut document = document(record(HostSupportStatus::Unknown));
        document.host_support_matrix.selected_host = Some(RuntimeKind::Claude);
        let error = document.validate().expect_err("selection must fail closed");
        assert_eq!(
            error,
            HostSupportMatrixValidationError::SelectedHostMustBeNone
        );
    }

    #[test]
    fn deserialization_rejects_selection_and_all_authority_grants() {
        let document = document(record(HostSupportStatus::Candidate));
        let mut selected = serde_json::to_value(&document).expect("serialize matrix");
        selected["host_support_matrix"]["selected_host"] = serde_json::json!("claude");
        assert!(serde_json::from_value::<HostSupportMatrixDocument>(selected).is_err());

        for grant in [
            "grants_support_authority",
            "grants_release_authority",
            "grants_private_key_authority",
        ] {
            let mut value = serde_json::to_value(&document).expect("serialize matrix");
            value["host_support_matrix"]["serialization_boundary"][grant] = serde_json::json!(true);
            assert!(
                serde_json::from_value::<HostSupportMatrixDocument>(value).is_err(),
                "{grant} must be rejected during deserialization"
            );
        }
    }

    #[test]
    fn unknown_fields_fail_closed() {
        let mut value = serde_json::to_value(document(record(HostSupportStatus::Unsupported)))
            .expect("serialize matrix");
        value["host_support_matrix"]["forbidden_signing_authority"] = serde_json::json!(true);
        assert!(serde_json::from_value::<HostSupportMatrixDocument>(value).is_err());
    }
}
