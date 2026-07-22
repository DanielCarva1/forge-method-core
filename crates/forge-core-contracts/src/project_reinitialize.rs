#![allow(clippy::missing_errors_doc)]

//! Closed contracts for reinitializing an irrecoverably lost project as new.
//!
//! These documents bind evidence and intent for a future plan/apply protocol.
//! They are not an admission, signing, installation, activation, lifecycle, or
//! host-selection capability. In particular, `selected_host` is always `None`.
//! No field can carry a private external broker key.

use crate::bootstrap_recovery::{StateLossCause, StateLossKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Component, Path};

pub const PROJECT_REINITIALIZE_PLAN_SCHEMA_VERSION: &str = "forge_project_reinitialize_plan_v1";
pub const PROJECT_REINITIALIZE_APPLY_SCHEMA_VERSION: &str = "forge_project_reinitialize_apply_v1";

const MAX_ID_LENGTH: usize = 128;
const MAX_PATH_LENGTH: usize = 1_024;
const MAX_CHALLENGE_LENGTH: usize = 256;
const MAX_STATE_VERSION_LENGTH: usize = 128;
const MAX_DIAGNOSIS_EVIDENCE: usize = 16;

/// Exact state-loss observation that must remain unchanged between planning and
/// applying. This is a binding, not a replacement for a fresh diagnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeDiagnosisBinding {
    pub diagnosis_schema_version: String,
    pub diagnosis_digest: String,
    pub kind: StateLossKind,
    pub cause: StateLossCause,
    pub expected_project_link_schema_version: String,
    pub expected_project_link_sha256: String,
    pub expected_state_schema_version: String,
    pub expected_state_sha256: String,
    /// Canonically sorted, bounded public evidence references.
    pub evidence: Vec<ReinitializeDiagnosisEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeDiagnosisEvidence {
    pub relative_path: String,
    pub sha256: String,
}

/// The project and authority being abandoned, identified without granting them
/// any continuing authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AbandonedProjectIdentity {
    pub project_id: String,
    pub authority_id: String,
    pub authority_root: String,
}

/// A fresh destination that cannot reuse the abandoned location or identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeDestination {
    pub project_root: String,
    pub new_project_id: String,
    pub new_authority_id: String,
    pub new_authority_root: String,
}

/// A challenge displayed to and explicitly echoed by the operator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeConfirmation {
    pub challenge: String,
    pub challenge_sha256: String,
    pub confirmation_digest: String,
}

/// Durable identities for append-only operation evidence. They contain only
/// public identifiers and digests, never key material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeWalIdentity {
    pub wal_id: String,
    pub wal_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeReceiptIdentity {
    pub receipt_id: String,
    pub receipt_sha256: String,
}

/// Non-negotiable, create-only semantics. `selected_host` has no value because
/// this candidate document must not select a host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeSemantics {
    pub destination_must_be_absent: bool,
    pub overwrite_allowed: bool,
    pub restore_allowed: bool,
    pub selected_host: Option<String>,
}

/// Durable plan for a future reinitialize-as-new operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectReinitializePlan {
    pub schema_version: String,
    pub plan_id: String,
    pub operation_id: String,
    pub diagnosis: ReinitializeDiagnosisBinding,
    pub abandoned: AbandonedProjectIdentity,
    pub destination: ReinitializeDestination,
    pub confirmation: ReinitializeConfirmation,
    pub wal: ReinitializeWalIdentity,
    pub receipt: ReinitializeReceiptIdentity,
    pub semantics: ReinitializeSemantics,
}

/// Apply record which repeats every mutation-critical binding from its plan.
/// `expected_plan_sha256` binds the record to the serialized durable plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectReinitializeApply {
    pub schema_version: String,
    pub apply_id: String,
    pub plan_id: String,
    pub operation_id: String,
    pub expected_plan_sha256: String,
    pub diagnosis: ReinitializeDiagnosisBinding,
    pub abandoned: AbandonedProjectIdentity,
    pub destination: ReinitializeDestination,
    pub confirmation: ReinitializeConfirmation,
    pub wal: ReinitializeWalIdentity,
    pub receipt: ReinitializeReceiptIdentity,
    pub semantics: ReinitializeSemantics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectReinitializeValidationError {
    UnsupportedPlanSchemaVersion,
    UnsupportedApplySchemaVersion,
    InvalidIdentifier,
    DuplicateIdentifier,
    InvalidDigest,
    InvalidDiagnosis,
    InvalidPath,
    ReusedIdentity,
    ReusedLocation,
    InvalidConfirmation,
    InvalidSemantics,
    PlanBindingMismatch,
}

impl ReinitializeDiagnosisBinding {
    fn validate(&self) -> Result<(), ProjectReinitializeValidationError> {
        if !is_bounded_text(&self.diagnosis_schema_version, MAX_STATE_VERSION_LENGTH)
            || !is_bounded_text(
                &self.expected_project_link_schema_version,
                MAX_STATE_VERSION_LENGTH,
            )
            || !is_bounded_text(
                &self.expected_state_schema_version,
                MAX_STATE_VERSION_LENGTH,
            )
        {
            return Err(ProjectReinitializeValidationError::InvalidDiagnosis);
        }
        if !is_sha256(&self.diagnosis_digest)
            || !is_sha256(&self.expected_project_link_sha256)
            || !is_sha256(&self.expected_state_sha256)
        {
            return Err(ProjectReinitializeValidationError::InvalidDigest);
        }
        if self.evidence.len() > MAX_DIAGNOSIS_EVIDENCE {
            return Err(ProjectReinitializeValidationError::InvalidDiagnosis);
        }
        let mut previous_path = None;
        for evidence in &self.evidence {
            validate_relative_path(&evidence.relative_path)?;
            validate_digest(&evidence.sha256)?;
            if previous_path.is_some_and(|previous| previous >= evidence.relative_path.as_str()) {
                return Err(ProjectReinitializeValidationError::InvalidDiagnosis);
            }
            previous_path = Some(evidence.relative_path.as_str());
        }
        Ok(())
    }
}

impl ReinitializeConfirmation {
    fn validate(&self) -> Result<(), ProjectReinitializeValidationError> {
        if !is_bounded_text(&self.challenge, MAX_CHALLENGE_LENGTH)
            || self.challenge_sha256 != sha256_of(&self.challenge)
            || self.confirmation_digest != self.challenge_sha256
        {
            return Err(ProjectReinitializeValidationError::InvalidConfirmation);
        }
        Ok(())
    }
}

impl ReinitializeSemantics {
    fn validate(&self) -> Result<(), ProjectReinitializeValidationError> {
        if !self.destination_must_be_absent
            || self.overwrite_allowed
            || self.restore_allowed
            || self.selected_host.is_some()
        {
            return Err(ProjectReinitializeValidationError::InvalidSemantics);
        }
        Ok(())
    }
}

impl ProjectReinitializePlan {
    /// Validate every closed v1 plan invariant after deserialization.
    pub fn validate(&self) -> Result<(), ProjectReinitializeValidationError> {
        if self.schema_version != PROJECT_REINITIALIZE_PLAN_SCHEMA_VERSION {
            return Err(ProjectReinitializeValidationError::UnsupportedPlanSchemaVersion);
        }
        if self.abandoned.project_id == self.destination.new_project_id
            || self.abandoned.authority_id == self.destination.new_authority_id
        {
            return Err(ProjectReinitializeValidationError::ReusedIdentity);
        }
        validate_ids(&[
            &self.plan_id,
            &self.operation_id,
            &self.abandoned.project_id,
            &self.abandoned.authority_id,
            &self.destination.new_project_id,
            &self.destination.new_authority_id,
            &self.wal.wal_id,
            &self.receipt.receipt_id,
        ])?;
        self.diagnosis.validate()?;
        self.confirmation.validate()?;
        validate_digest(&self.wal.wal_sha256)?;
        validate_digest(&self.receipt.receipt_sha256)?;
        validate_absolute_path(&self.abandoned.authority_root)?;
        validate_absolute_path(&self.destination.project_root)?;
        validate_absolute_path(&self.destination.new_authority_root)?;
        if self.abandoned.authority_root == self.destination.project_root
            || self.abandoned.authority_root == self.destination.new_authority_root
            || self.destination.project_root == self.destination.new_authority_root
        {
            return Err(ProjectReinitializeValidationError::ReusedLocation);
        }
        self.semantics.validate()
    }
}

impl ProjectReinitializeApply {
    /// Validate the closed apply record before comparing it with a plan.
    pub fn validate(&self) -> Result<(), ProjectReinitializeValidationError> {
        if self.schema_version != PROJECT_REINITIALIZE_APPLY_SCHEMA_VERSION {
            return Err(ProjectReinitializeValidationError::UnsupportedApplySchemaVersion);
        }
        validate_ids(&[
            &self.apply_id,
            &self.plan_id,
            &self.operation_id,
            &self.abandoned.project_id,
            &self.abandoned.authority_id,
            &self.destination.new_project_id,
            &self.destination.new_authority_id,
            &self.wal.wal_id,
            &self.receipt.receipt_id,
        ])?;
        validate_digest(&self.expected_plan_sha256)?;
        let plan = ProjectReinitializePlan {
            schema_version: PROJECT_REINITIALIZE_PLAN_SCHEMA_VERSION.to_string(),
            plan_id: self.plan_id.clone(),
            operation_id: self.operation_id.clone(),
            diagnosis: self.diagnosis.clone(),
            abandoned: self.abandoned.clone(),
            destination: self.destination.clone(),
            confirmation: self.confirmation.clone(),
            wal: self.wal.clone(),
            receipt: self.receipt.clone(),
            semantics: self.semantics.clone(),
        };
        plan.validate()?;
        if self.apply_id == self.plan_id || self.apply_id == self.operation_id {
            return Err(ProjectReinitializeValidationError::DuplicateIdentifier);
        }
        Ok(())
    }

    /// Require the apply record to repeat the exact durable plan bindings.
    pub fn validate_against_plan(
        &self,
        plan: &ProjectReinitializePlan,
    ) -> Result<(), ProjectReinitializeValidationError> {
        self.validate()?;
        plan.validate()?;
        if self.expected_plan_sha256 != plan_sha256(plan)
            || self.plan_id != plan.plan_id
            || self.operation_id != plan.operation_id
            || self.diagnosis != plan.diagnosis
            || self.abandoned != plan.abandoned
            || self.destination != plan.destination
            || self.confirmation != plan.confirmation
            || self.wal != plan.wal
            || self.receipt != plan.receipt
            || self.semantics != plan.semantics
        {
            return Err(ProjectReinitializeValidationError::PlanBindingMismatch);
        }
        Ok(())
    }
}

fn validate_ids(values: &[&str]) -> Result<(), ProjectReinitializeValidationError> {
    for value in values {
        validate_identifier(value)?;
    }
    for (index, value) in values.iter().enumerate() {
        if values[index + 1..].contains(value) {
            return Err(ProjectReinitializeValidationError::DuplicateIdentifier);
        }
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), ProjectReinitializeValidationError> {
    let mut bytes = value.bytes();
    if value.is_empty()
        || value.len() > MAX_ID_LENGTH
        || !bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || !bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(ProjectReinitializeValidationError::InvalidIdentifier);
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), ProjectReinitializeValidationError> {
    if is_sha256(value) {
        Ok(())
    } else {
        Err(ProjectReinitializeValidationError::InvalidDigest)
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_of(value: &str) -> String {
    sha256_of_bytes(value.as_bytes())
}

/// SHA-256 of the single canonical byte representation of a durable plan.
///
/// The closed v1 protocol uses canonical JSON bytes, not an independently
/// formatted serialization, as its single durable plan encoding.
fn plan_sha256(plan: &ProjectReinitializePlan) -> String {
    let bytes = serde_json_canonicalizer::to_vec(plan)
        .expect("closed reinitialize plan canonically serializes");
    sha256_of_bytes(&bytes)
}

fn sha256_of_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn is_bounded_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.contains('\0')
}

fn validate_relative_path(value: &str) -> Result<(), ProjectReinitializeValidationError> {
    if value.is_empty()
        || value.len() > MAX_PATH_LENGTH
        || value.contains('\0')
        || value.contains('\\')
    {
        return Err(ProjectReinitializeValidationError::InvalidPath);
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ProjectReinitializeValidationError::InvalidPath);
    }
    Ok(())
}

fn validate_absolute_path(value: &str) -> Result<(), ProjectReinitializeValidationError> {
    if value.len() > MAX_PATH_LENGTH || value.contains('\0') || value.contains('\\') {
        return Err(ProjectReinitializeValidationError::InvalidPath);
    }
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(ProjectReinitializeValidationError::InvalidPath);
    }
    let mut normal_components = 0usize;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(_) => normal_components += 1,
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(ProjectReinitializeValidationError::InvalidPath);
            }
        }
    }
    if normal_components == 0 {
        return Err(ProjectReinitializeValidationError::InvalidPath);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(byte: char) -> String {
        byte.to_string().repeat(64)
    }

    fn plan() -> ProjectReinitializePlan {
        ProjectReinitializePlan {
            schema_version: PROJECT_REINITIALIZE_PLAN_SCHEMA_VERSION.to_string(),
            plan_id: "plan.reinitialize-1".to_string(),
            operation_id: "operation.reinitialize-1".to_string(),
            diagnosis: ReinitializeDiagnosisBinding {
                diagnosis_schema_version: "forge_bootstrap_state_loss_v1".to_string(),
                diagnosis_digest: sha('a'),
                kind: StateLossKind::LinkedStateUnavailable,
                cause: StateLossCause::MissingStateRoot,
                expected_project_link_schema_version: "forge_project_link_v1".to_string(),
                expected_project_link_sha256: sha('b'),
                expected_state_schema_version: "forge_state_v1".to_string(),
                expected_state_sha256: sha('c'),
                evidence: vec![ReinitializeDiagnosisEvidence {
                    relative_path: "state/manifest.json".to_string(),
                    sha256: sha('3'),
                }],
            },
            abandoned: AbandonedProjectIdentity {
                project_id: "project.old".to_string(),
                authority_id: "authority.old".to_string(),
                authority_root: "/srv/forge-old".to_string(),
            },
            destination: ReinitializeDestination {
                project_root: "/srv/project-new".to_string(),
                new_project_id: "project.new".to_string(),
                new_authority_id: "authority.new".to_string(),
                new_authority_root: "/srv/forge-new".to_string(),
            },
            confirmation: ReinitializeConfirmation {
                challenge: "REINITIALIZE project.old AS project.new".to_string(),
                challenge_sha256: sha256_of("REINITIALIZE project.old AS project.new"),
                confirmation_digest: sha256_of("REINITIALIZE project.old AS project.new"),
            },
            wal: ReinitializeWalIdentity {
                wal_id: "wal.reinitialize-1".to_string(),
                wal_sha256: sha('f'),
            },
            receipt: ReinitializeReceiptIdentity {
                receipt_id: "receipt.reinitialize-1".to_string(),
                receipt_sha256: sha('1'),
            },
            semantics: ReinitializeSemantics {
                destination_must_be_absent: true,
                overwrite_allowed: false,
                restore_allowed: false,
                selected_host: None,
            },
        }
    }

    fn apply() -> ProjectReinitializeApply {
        let plan = plan();
        let expected_plan_sha256 = plan_sha256(&plan);
        ProjectReinitializeApply {
            schema_version: PROJECT_REINITIALIZE_APPLY_SCHEMA_VERSION.to_string(),
            apply_id: "apply.reinitialize-1".to_string(),
            plan_id: plan.plan_id,
            operation_id: plan.operation_id,
            expected_plan_sha256,
            diagnosis: plan.diagnosis,
            abandoned: plan.abandoned,
            destination: plan.destination,
            confirmation: plan.confirmation,
            wal: plan.wal,
            receipt: plan.receipt,
            semantics: plan.semantics,
        }
    }

    #[test]
    fn closed_contract_rejects_unknown_authority_fields() {
        let mut value = serde_json::to_value(plan()).unwrap();
        value["abandoned"]["authority_override"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ProjectReinitializePlan>(value).is_err());
    }

    #[test]
    fn rejects_identity_and_location_reuse() {
        let mut reused_identity = plan();
        reused_identity.destination.new_authority_id =
            reused_identity.abandoned.authority_id.clone();
        assert_eq!(
            reused_identity.validate(),
            Err(ProjectReinitializeValidationError::ReusedIdentity)
        );

        let mut reused_location = plan();
        reused_location.destination.new_authority_root =
            reused_location.abandoned.authority_root.clone();
        assert_eq!(
            reused_location.validate(),
            Err(ProjectReinitializeValidationError::ReusedLocation)
        );
    }

    #[test]
    fn apply_rejects_stale_or_substituted_diagnosis() {
        let plan = plan();
        let mut stale = apply();
        stale.diagnosis.diagnosis_digest = sha('9');
        assert_eq!(
            stale.validate_against_plan(&plan),
            Err(ProjectReinitializeValidationError::PlanBindingMismatch)
        );
    }

    #[test]
    fn apply_rejects_a_digest_that_does_not_bind_the_plan_bytes() {
        let plan = plan();
        let mut substituted = apply();
        substituted.expected_plan_sha256 = sha('2');
        assert_eq!(
            substituted.validate_against_plan(&plan),
            Err(ProjectReinitializeValidationError::PlanBindingMismatch)
        );
    }

    #[test]
    fn rejects_invalid_confirmation_and_host_selection() {
        let mut invalid_confirmation = plan();
        invalid_confirmation.confirmation.challenge = "different challenge".to_string();
        assert_eq!(
            invalid_confirmation.validate(),
            Err(ProjectReinitializeValidationError::InvalidConfirmation)
        );

        let mut host_selected = plan();
        host_selected.semantics.selected_host = Some("host-a".to_string());
        assert_eq!(
            host_selected.validate(),
            Err(ProjectReinitializeValidationError::InvalidSemantics)
        );
    }

    #[test]
    fn rejects_invalid_digest_and_unsafe_path() {
        let mut invalid_digest = plan();
        invalid_digest.diagnosis.expected_state_sha256 = "ABC".to_string();
        assert_eq!(
            invalid_digest.validate(),
            Err(ProjectReinitializeValidationError::InvalidDigest)
        );

        let mut invalid_path = plan();
        invalid_path.destination.project_root = "/srv/../other".to_string();
        assert_eq!(
            invalid_path.validate(),
            Err(ProjectReinitializeValidationError::InvalidPath)
        );
    }
}
