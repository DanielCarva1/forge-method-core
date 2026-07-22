#![allow(clippy::missing_errors_doc)]

//! Strict public contracts for an origin-bound workflow broker control plane.
//!
//! These documents intentionally contain only public keys, bounded opaque
//! identifiers, canonical digests, and closed metadata. Private keys, generic
//! signing requests, raw host transcripts, environment values, and caller-
//! selected workflow semantics have no representation in this module.

use crate::{RuntimeKind, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION: &str = "0.2";
pub const WORKFLOW_BROKER_PUBLIC_CREDENTIAL_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BROKER_ADMIN_RECEIPT_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BROKER_COMPONENT_STATUS_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION: &str = "0.2";
pub const WORKFLOW_BROKER_AUDIENCE_PREFIX: &str = "forge-core:workflow:";

pub const WORKFLOW_BROKER_PUBLIC_REGISTRY_DIGEST_DOMAIN: &[u8] =
    b"forge-method:workflow-broker-public-registry:v2\0";
pub const WORKFLOW_BROKER_PUBLIC_CREDENTIAL_DIGEST_DOMAIN: &[u8] =
    b"forge-method:workflow-broker-public-credential:v1\0";
pub const WORKFLOW_BROKER_ADMIN_OPERATION_SIGNATURE_DOMAIN: &[u8] =
    b"forge-method:workflow-broker-admin-operation:v1\0";
pub const WORKFLOW_BROKER_ADMIN_RECEIPT_DIGEST_DOMAIN: &[u8] =
    b"forge-method:workflow-broker-admin-receipt:v1\0";
pub const WORKFLOW_BROKER_ADMIN_DESCRIPTOR_DIGEST_DOMAIN: &[u8] =
    b"forge-method:workflow-broker-native-admin-descriptor:v1\0";
pub const WORKFLOW_BROKER_NATIVE_REPLAY_DIGEST_DOMAIN: &[u8] =
    b"forge-method:workflow-broker-native-interaction-replay:v1\0";
pub const WORKFLOW_BROKER_NATIVE_ADMIN_REPLAY_DIGEST_DOMAIN: &[u8] =
    b"forge-method:workflow-broker-native-admin-replay:v1\0";

/// Strict public registry for one exact project/workflow audience.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerPublicRegistryDocument {
    pub schema_version: String,
    pub audience: String,
    pub project_id: StableId,
    pub workflow_id: StableId,
    /// Monotonically advances by exactly one for every admitted administration
    /// operation. Generation one is the externally enrolled genesis snapshot.
    pub registry_generation: u64,
    /// Exact canonical predecessor digest. Required after generation one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_registry_digest: Option<String>,
    /// Exact broker event wire version admitted by this registry. A newer
    /// registry cannot silently reopen a frozen legacy event wire.
    pub required_event_schema_version: String,
    pub credentials: Vec<WorkflowBrokerPublicCredentialMetadata>,
}

/// Standalone projection for enrollment, inspection, backup, and receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerPublicCredentialMetadataDocument {
    pub schema_version: String,
    pub credential: WorkflowBrokerPublicCredentialMetadata,
}

/// Public metadata for one non-exportable broker or administrator credential.
/// There is deliberately no secret/private-key/key-handle serialization field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerPublicCredentialMetadata {
    pub credential_id: StableId,
    /// Stable broker installation identity retained across key rotation. Event
    /// replay identity uses this value rather than the rotating credential id.
    pub broker_id: StableId,
    /// Event issuer id, or administrator subject id for administration keys.
    pub subject_id: StableId,
    pub purpose: WorkflowBrokerCredentialPurpose,
    pub profile: WorkflowBrokerCredentialProfile,
    pub algorithm: WorkflowBrokerPublicKeyAlgorithm,
    pub public_key_hex: String,
    pub key_generation: u64,
    pub status: WorkflowBrokerCredentialStatus,
    pub custody: WorkflowBrokerCustodyKind,
    pub host_binding: WorkflowBrokerHostBinding,
    /// Closed event operations. Administrator credentials must carry an empty
    /// list; they can authorize only the closed administration envelope below.
    pub allowed_operations: Vec<WorkflowBrokerBoundOperation>,
    pub not_before_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predecessor_credential_id: Option<StableId>,
    pub enrollment_operation_id: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revocation_operation_id: Option<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerCredentialPurpose {
    EventIssuer,
    RegistryAdministrator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerCredentialProfile {
    Human,
    Reviewer,
    Runtime,
    Administrator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerPublicKeyAlgorithm {
    Ed25519,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerCredentialStatus {
    Active,
    Revoked,
}

/// Closed, public custody assertion. Every admitted variant excludes exportable
/// file keys and agent-controlled environment/argv/configuration material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerCustodyKind {
    OsKeystoreNonExportable,
    HardwareBackedNonExportable,
    RemoteSignerNonExportable,
    HostIsolatedNonExportable,
}

/// Exact selected-host and adapter installation binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerHostBinding {
    pub host_kind: RuntimeKind,
    pub host_version: String,
    pub adapter_id: StableId,
    pub adapter_version: String,
    pub host_installation_id: StableId,
    pub protocol_version: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerBoundOperation {
    Applicability,
    Capability,
    Decision,
    Evidence,
    IntentRevision,
    Signal,
    Waiver,
}

/// Signed lifecycle operation. It cannot carry an arbitrary packet, JSON
/// payload, transcript, or generic `sign` request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerAdminOperationEnvelope {
    pub schema_version: String,
    pub audience: String,
    pub project_id: StableId,
    pub workflow_id: StableId,
    pub operation_id: StableId,
    pub admin_credential_id: StableId,
    pub admin_credential_generation: u64,
    pub expected_registry_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_registry_digest: Option<String>,
    pub proposed_registry_generation: u64,
    pub proposed_registry_digest: String,
    pub operation: WorkflowBrokerAdminOperation,
    pub native_authorization: WorkflowBrokerNativeAdminAuthorization,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
    pub nonce: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowBrokerAdminOperation {
    /// Install the first strict public registry. The active administrator named
    /// here signs the exact generation-one snapshot through a native admin
    /// interaction; no Forge signing or bootstrap key-generation path exists.
    Initialize {
        active_admin_credential_id: StableId,
    },
    Enroll {
        credential_id: StableId,
    },
    Rotate {
        current_credential_id: StableId,
        replacement_credential_id: StableId,
    },
    Revoke {
        credential_id: StableId,
        reason_code: StableId,
    },
}

/// Opaque native operator/admin authorization. These references are host-local
/// handles, never transcript or credential material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerNativeAdminAuthorization {
    pub host_kind: RuntimeKind,
    pub host_version: String,
    pub adapter_id: StableId,
    pub adapter_version: String,
    pub host_installation_id: StableId,
    pub protocol_version: String,
    pub admin_session_ref: String,
    pub admin_interaction_ref: String,
    pub observed_at_unix: u64,
    pub descriptor_digest: String,
}

/// Typed availability of the external selected-host setup boundary. This is a
/// control-plane status only: `Ready` does not itself prove custody, user
/// presence, or a native interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowBrokerExternalSetupState {
    Ready,
    Blocked {
        reason: WorkflowBrokerExternalSetupBlockReason,
    },
}

/// Closed reasons why Forge cannot currently perform an external broker setup
/// operation. Callers must not replace either reason with agent-created keys or
/// synthetic host evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerExternalSetupBlockReason {
    SelectedHostUnavailable,
    ExternalOperatorTrustAnchorUnavailable,
}

/// Stable replay identity for one native administrator interaction. Operation,
/// credential generation, signature, and proposed-registry fields are absent so
/// the same host interaction cannot authorize a second lifecycle operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerNativeAdminReplayKey {
    pub audience: String,
    pub project_id: StableId,
    pub workflow_id: StableId,
    pub host_kind: RuntimeKind,
    pub adapter_id: StableId,
    pub host_installation_id: StableId,
    pub admin_session_ref: String,
    pub admin_interaction_ref: String,
}

/// Durable, content-free receipt for one exact registry CAS transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerAdminReceiptDocument {
    pub schema_version: String,
    pub receipt: WorkflowBrokerAdminReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerAdminReceipt {
    pub operation_id: StableId,
    pub operation_digest: String,
    pub audience: String,
    pub project_id: StableId,
    pub workflow_id: StableId,
    pub admin_credential_id: StableId,
    pub admin_credential_generation: u64,
    pub admin_public_key_fingerprint: String,
    pub signature_fingerprint: String,
    pub expected_registry_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_registry_digest: Option<String>,
    pub proposed_registry_generation: u64,
    pub proposed_registry_digest: String,
    pub native_authorization_descriptor_digest: String,
    pub native_authorization_replay_digest: String,
    pub authorized_at_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_receipt_digest: Option<String>,
    pub receipt_digest: String,
}

/// Public health/status projection. It carries no signer handle or key bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerComponentStatusDocument {
    pub schema_version: String,
    pub status: WorkflowBrokerComponentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerComponentStatus {
    pub audience: String,
    pub project_id: StableId,
    pub workflow_id: StableId,
    pub registry_generation: u64,
    pub registry_digest: String,
    pub required_event_schema_version: String,
    pub active_event_credential_count: usize,
    pub retained_revoked_credential_count: usize,
    pub active_admin_credential_id: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_admin_receipt_digest: Option<String>,
    pub recovery: WorkflowBrokerRecoveryState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowBrokerRecoveryState {
    Clean,
    PendingAdminReceipt {
        operation_id: StableId,
        operation_digest: String,
        proposed_registry_digest: String,
    },
}

/// Rotation-stable replay identity. Credential/issuer ids and key generations
/// are intentionally absent; a native interaction remains consumed after key
/// rotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerNativeInteractionReplayKey {
    pub audience: String,
    pub project_id: StableId,
    pub workflow_id: StableId,
    pub broker_id: StableId,
    pub host_kind: RuntimeKind,
    pub adapter_id: StableId,
    pub host_installation_id: StableId,
    pub host_event_ref: String,
    pub host_session_ref: String,
    pub host_interaction_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowBrokerContractError {
    UnsupportedReceiptSchema(String),
    MissingReceiptDigest,
    ReceiptDigestMismatch,
    Canonicalization(String),
}

impl fmt::Display for WorkflowBrokerContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedReceiptSchema(version) => {
                write!(
                    formatter,
                    "unsupported broker admin receipt schema '{version}'"
                )
            }
            Self::MissingReceiptDigest => {
                formatter.write_str("broker admin receipt digest missing")
            }
            Self::ReceiptDigestMismatch => {
                formatter.write_str("broker admin receipt digest mismatch")
            }
            Self::Canonicalization(error) => {
                write!(
                    formatter,
                    "broker contract canonicalization failed: {error}"
                )
            }
        }
    }
}

impl std::error::Error for WorkflowBrokerContractError {}

/// Canonical audience retained by the existing broker registry and v0.2 event
/// wire. The strict control-plane documents bind `workflow_id` separately, so
/// workflow confusion is rejected without reinterpreting the frozen event wire.
#[must_use]
pub fn workflow_broker_expected_audience(project_id: &StableId, _workflow_id: &StableId) -> String {
    format!("{WORKFLOW_BROKER_AUDIENCE_PREFIX}{}", project_id.0)
}

/// Canonical bytes for one exact public registry snapshot.
pub fn workflow_broker_public_registry_canonical_bytes(
    document: &WorkflowBrokerPublicRegistryDocument,
) -> Result<Vec<u8>, WorkflowBrokerContractError> {
    canonical_bytes(document)
}

pub fn workflow_broker_public_registry_digest(
    document: &WorkflowBrokerPublicRegistryDocument,
) -> Result<String, WorkflowBrokerContractError> {
    domain_digest(
        WORKFLOW_BROKER_PUBLIC_REGISTRY_DIGEST_DOMAIN,
        &workflow_broker_public_registry_canonical_bytes(document)?,
    )
}

pub fn workflow_broker_public_credential_digest(
    credential: &WorkflowBrokerPublicCredentialMetadata,
) -> Result<String, WorkflowBrokerContractError> {
    domain_digest(
        WORKFLOW_BROKER_PUBLIC_CREDENTIAL_DIGEST_DOMAIN,
        &canonical_bytes(credential)?,
    )
}

/// Canonical signed bytes excluding only the signature field.
pub fn workflow_broker_admin_operation_signing_bytes(
    envelope: &WorkflowBrokerAdminOperationEnvelope,
) -> Result<Vec<u8>, WorkflowBrokerContractError> {
    #[derive(Serialize)]
    struct Signed<'a> {
        schema_version: &'a str,
        audience: &'a str,
        project_id: &'a StableId,
        workflow_id: &'a StableId,
        operation_id: &'a StableId,
        admin_credential_id: &'a StableId,
        admin_credential_generation: u64,
        expected_registry_generation: u64,
        expected_registry_digest: &'a Option<String>,
        proposed_registry_generation: u64,
        proposed_registry_digest: &'a str,
        operation: &'a WorkflowBrokerAdminOperation,
        native_authorization: &'a WorkflowBrokerNativeAdminAuthorization,
        issued_at_unix: u64,
        expires_at_unix: u64,
        nonce: &'a str,
    }

    let canonical = canonical_bytes(&Signed {
        schema_version: &envelope.schema_version,
        audience: &envelope.audience,
        project_id: &envelope.project_id,
        workflow_id: &envelope.workflow_id,
        operation_id: &envelope.operation_id,
        admin_credential_id: &envelope.admin_credential_id,
        admin_credential_generation: envelope.admin_credential_generation,
        expected_registry_generation: envelope.expected_registry_generation,
        expected_registry_digest: &envelope.expected_registry_digest,
        proposed_registry_generation: envelope.proposed_registry_generation,
        proposed_registry_digest: &envelope.proposed_registry_digest,
        operation: &envelope.operation,
        native_authorization: &envelope.native_authorization,
        issued_at_unix: envelope.issued_at_unix,
        expires_at_unix: envelope.expires_at_unix,
        nonce: &envelope.nonce,
    })?;
    domain_bytes(WORKFLOW_BROKER_ADMIN_OPERATION_SIGNATURE_DOMAIN, &canonical)
}

pub fn workflow_broker_admin_operation_digest(
    envelope: &WorkflowBrokerAdminOperationEnvelope,
) -> Result<String, WorkflowBrokerContractError> {
    Ok(raw_digest(&workflow_broker_admin_operation_signing_bytes(
        envelope,
    )?))
}

/// Recompute the descriptor signed by a native administration interaction.
pub fn workflow_broker_native_admin_descriptor_digest(
    envelope: &WorkflowBrokerAdminOperationEnvelope,
) -> Result<String, WorkflowBrokerContractError> {
    #[derive(Serialize)]
    struct Descriptor<'a> {
        schema_version: &'static str,
        audience: &'a str,
        project_id: &'a StableId,
        workflow_id: &'a StableId,
        operation_id: &'a StableId,
        admin_credential_id: &'a StableId,
        admin_credential_generation: u64,
        expected_registry_generation: u64,
        expected_registry_digest: &'a Option<String>,
        proposed_registry_generation: u64,
        proposed_registry_digest: &'a str,
        operation: &'a WorkflowBrokerAdminOperation,
        host_kind: RuntimeKind,
        host_version: &'a str,
        adapter_id: &'a StableId,
        adapter_version: &'a str,
        host_installation_id: &'a StableId,
        protocol_version: &'a str,
        admin_session_ref: &'a str,
        admin_interaction_ref: &'a str,
        observed_at_unix: u64,
    }

    let native = &envelope.native_authorization;
    let canonical = canonical_bytes(&Descriptor {
        schema_version: "workflow_broker_native_admin_descriptor_v1",
        audience: &envelope.audience,
        project_id: &envelope.project_id,
        workflow_id: &envelope.workflow_id,
        operation_id: &envelope.operation_id,
        admin_credential_id: &envelope.admin_credential_id,
        admin_credential_generation: envelope.admin_credential_generation,
        expected_registry_generation: envelope.expected_registry_generation,
        expected_registry_digest: &envelope.expected_registry_digest,
        proposed_registry_generation: envelope.proposed_registry_generation,
        proposed_registry_digest: &envelope.proposed_registry_digest,
        operation: &envelope.operation,
        host_kind: native.host_kind,
        host_version: &native.host_version,
        adapter_id: &native.adapter_id,
        adapter_version: &native.adapter_version,
        host_installation_id: &native.host_installation_id,
        protocol_version: &native.protocol_version,
        admin_session_ref: &native.admin_session_ref,
        admin_interaction_ref: &native.admin_interaction_ref,
        observed_at_unix: native.observed_at_unix,
    })?;
    domain_digest(WORKFLOW_BROKER_ADMIN_DESCRIPTOR_DIGEST_DOMAIN, &canonical)
}

pub fn workflow_broker_native_admin_replay_digest(
    replay: &WorkflowBrokerNativeAdminReplayKey,
) -> Result<String, WorkflowBrokerContractError> {
    domain_digest(
        WORKFLOW_BROKER_NATIVE_ADMIN_REPLAY_DIGEST_DOMAIN,
        &canonical_bytes(replay)?,
    )
}

pub fn workflow_broker_native_interaction_replay_digest(
    replay: &WorkflowBrokerNativeInteractionReplayKey,
) -> Result<String, WorkflowBrokerContractError> {
    domain_digest(
        WORKFLOW_BROKER_NATIVE_REPLAY_DIGEST_DOMAIN,
        &canonical_bytes(replay)?,
    )
}

impl WorkflowBrokerAdminReceiptDocument {
    pub fn canonical_receipt_bytes(&self) -> Result<Vec<u8>, WorkflowBrokerContractError> {
        let mut value = serde_json::to_value(self)
            .map_err(|error| WorkflowBrokerContractError::Canonicalization(error.to_string()))?;
        value
            .get_mut("receipt")
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|receipt| receipt.remove("receipt_digest"))
            .ok_or(WorkflowBrokerContractError::MissingReceiptDigest)?;
        serde_json_canonicalizer::to_vec(&value)
            .map_err(|error| WorkflowBrokerContractError::Canonicalization(error.to_string()))
    }

    pub fn digest(&self) -> Result<String, WorkflowBrokerContractError> {
        domain_digest(
            WORKFLOW_BROKER_ADMIN_RECEIPT_DIGEST_DOMAIN,
            &self.canonical_receipt_bytes()?,
        )
    }

    pub fn validate_self_digest(&self) -> Result<(), WorkflowBrokerContractError> {
        if self.schema_version != WORKFLOW_BROKER_ADMIN_RECEIPT_SCHEMA_VERSION {
            return Err(WorkflowBrokerContractError::UnsupportedReceiptSchema(
                self.schema_version.clone(),
            ));
        }
        if self.receipt.receipt_digest != self.digest()? {
            return Err(WorkflowBrokerContractError::ReceiptDigestMismatch);
        }
        Ok(())
    }
}

fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, WorkflowBrokerContractError> {
    let value = serde_json::to_value(value)
        .map_err(|error| WorkflowBrokerContractError::Canonicalization(error.to_string()))?;
    serde_json_canonicalizer::to_vec(&value)
        .map_err(|error| WorkflowBrokerContractError::Canonicalization(error.to_string()))
}

fn domain_bytes(domain: &[u8], canonical: &[u8]) -> Result<Vec<u8>, WorkflowBrokerContractError> {
    let length = u64::try_from(canonical.len()).map_err(|_| {
        WorkflowBrokerContractError::Canonicalization("canonical value is too large".to_owned())
    })?;
    let mut bytes = Vec::with_capacity(domain.len() + 8 + canonical.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(canonical);
    Ok(bytes)
}

fn domain_digest(domain: &[u8], canonical: &[u8]) -> Result<String, WorkflowBrokerContractError> {
    Ok(raw_digest(&domain_bytes(domain, canonical)?))
}

fn raw_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn host() -> WorkflowBrokerHostBinding {
        WorkflowBrokerHostBinding {
            host_kind: RuntimeKind::ForgeStandalone,
            host_version: "1.2.3".to_owned(),
            adapter_id: StableId("adapter.host".to_owned()),
            adapter_version: "2.3.4".to_owned(),
            host_installation_id: StableId("host.installation.alpha".to_owned()),
            protocol_version: "workflow-host-origin-v1".to_owned(),
        }
    }

    fn credential() -> WorkflowBrokerPublicCredentialMetadata {
        WorkflowBrokerPublicCredentialMetadata {
            credential_id: StableId("credential.event.1".to_owned()),
            broker_id: StableId("broker.installation.alpha".to_owned()),
            subject_id: StableId("issuer.human.1".to_owned()),
            purpose: WorkflowBrokerCredentialPurpose::EventIssuer,
            profile: WorkflowBrokerCredentialProfile::Human,
            algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
            public_key_hex: "11".repeat(32),
            key_generation: 1,
            status: WorkflowBrokerCredentialStatus::Active,
            custody: WorkflowBrokerCustodyKind::OsKeystoreNonExportable,
            host_binding: host(),
            allowed_operations: vec![WorkflowBrokerBoundOperation::Decision],
            not_before_unix: 1_900_000_000,
            revoked_at_unix: None,
            predecessor_credential_id: None,
            enrollment_operation_id: StableId("admin.operation.genesis".to_owned()),
            revocation_operation_id: None,
        }
    }

    fn registry() -> WorkflowBrokerPublicRegistryDocument {
        WorkflowBrokerPublicRegistryDocument {
            schema_version: WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION.to_owned(),
            audience: "forge-core:workflow:project.alpha".to_owned(),
            project_id: StableId("project.alpha".to_owned()),
            workflow_id: StableId("workflow.governance".to_owned()),
            registry_generation: 1,
            previous_registry_digest: None,
            required_event_schema_version: WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION.to_owned(),
            credentials: vec![credential()],
        }
    }

    fn admin_envelope() -> WorkflowBrokerAdminOperationEnvelope {
        WorkflowBrokerAdminOperationEnvelope {
            schema_version: WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION.to_owned(),
            audience: "forge-core:workflow:project.alpha".to_owned(),
            project_id: StableId("project.alpha".to_owned()),
            workflow_id: StableId("workflow.governance".to_owned()),
            operation_id: StableId("admin.operation.rotate.2".to_owned()),
            admin_credential_id: StableId("credential.admin.1".to_owned()),
            admin_credential_generation: 1,
            expected_registry_generation: 1,
            expected_registry_digest: Some(digest('a')),
            proposed_registry_generation: 2,
            proposed_registry_digest: digest('b'),
            operation: WorkflowBrokerAdminOperation::Rotate {
                current_credential_id: StableId("credential.event.1".to_owned()),
                replacement_credential_id: StableId("credential.event.2".to_owned()),
            },
            native_authorization: WorkflowBrokerNativeAdminAuthorization {
                host_kind: RuntimeKind::ForgeStandalone,
                host_version: "1.2.3".to_owned(),
                adapter_id: StableId("adapter.host".to_owned()),
                adapter_version: "2.3.4".to_owned(),
                host_installation_id: StableId("host.installation.alpha".to_owned()),
                protocol_version: "workflow-host-origin-v1".to_owned(),
                admin_session_ref: "admin-session-00000001".to_owned(),
                admin_interaction_ref: "admin-interaction-0001".to_owned(),
                observed_at_unix: 1_900_000_100,
                descriptor_digest: digest('c'),
            },
            issued_at_unix: 1_900_000_100,
            expires_at_unix: 1_900_000_220,
            nonce: "admin-operation-nonce-0001".to_owned(),
            signature: "22".repeat(64),
        }
    }

    #[test]
    fn public_documents_structurally_reject_private_material() {
        let mut registry = serde_json::to_value(registry()).expect("registry JSON");
        registry["credentials"][0]["private_key_hex"] = serde_json::json!("secret");
        assert!(serde_json::from_value::<WorkflowBrokerPublicRegistryDocument>(registry).is_err());
        let mut oracle_request =
            serde_json::to_value(admin_envelope()).expect("admin operation JSON");
        oracle_request["packet_digest"] = serde_json::json!(digest('f'));
        oracle_request["arbitrary_json"] = serde_json::json!({"sign": true});
        assert!(
            serde_json::from_value::<WorkflowBrokerAdminOperationEnvelope>(oracle_request).is_err()
        );

        let document = WorkflowBrokerPublicCredentialMetadataDocument {
            schema_version: WORKFLOW_BROKER_PUBLIC_CREDENTIAL_SCHEMA_VERSION.to_owned(),
            credential: credential(),
        };
        let encoded = serde_json::to_string(&document).expect("credential JSON");
        for forbidden in [
            "private_key",
            "secret_key",
            "signing_key",
            "key_handle",
            "environment",
            "argv",
            "transcript",
        ] {
            assert!(!encoded.contains(forbidden), "unexpected {forbidden}");
        }
    }

    #[test]
    fn canonical_registry_and_credential_digests_bind_every_public_field() {
        let first = registry();
        assert_eq!(
            first.audience,
            workflow_broker_expected_audience(&first.project_id, &first.workflow_id)
        );
        let first_digest = workflow_broker_public_registry_digest(&first).expect("digest");
        let mut changed = first.clone();
        changed.credentials[0].host_binding.host_version = "1.2.4".to_owned();
        assert_ne!(
            first_digest,
            workflow_broker_public_registry_digest(&changed).expect("changed digest")
        );
        assert_ne!(
            workflow_broker_public_credential_digest(&first.credentials[0]).expect("credential"),
            workflow_broker_public_credential_digest(&changed.credentials[0])
                .expect("changed credential")
        );
    }

    #[test]
    fn admin_signing_bytes_exclude_only_signature_and_bind_cas_host_and_operation() {
        let envelope = admin_envelope();
        let bytes = workflow_broker_admin_operation_signing_bytes(&envelope).expect("bytes");
        let mut signature_changed = envelope.clone();
        signature_changed.signature = "ff".repeat(64);
        assert_eq!(
            bytes,
            workflow_broker_admin_operation_signing_bytes(&signature_changed)
                .expect("signature-independent bytes")
        );

        let mut operation_changed = envelope.clone();
        operation_changed.proposed_registry_digest = digest('d');
        assert_ne!(
            bytes,
            workflow_broker_admin_operation_signing_bytes(&operation_changed)
                .expect("operation changed")
        );
        let mut host_changed = envelope;
        host_changed.native_authorization.host_installation_id =
            StableId("host.installation.other".to_owned());
        assert_ne!(
            bytes,
            workflow_broker_admin_operation_signing_bytes(&host_changed).expect("host changed")
        );
    }

    #[test]
    fn replay_identity_is_stable_across_credential_rotation_but_not_host_interactions() {
        let replay = WorkflowBrokerNativeInteractionReplayKey {
            audience: "forge-core:workflow:project.alpha".to_owned(),
            project_id: StableId("project.alpha".to_owned()),
            workflow_id: StableId("workflow.governance".to_owned()),
            broker_id: StableId("broker.installation.alpha".to_owned()),
            host_kind: RuntimeKind::ForgeStandalone,
            adapter_id: StableId("adapter.host".to_owned()),
            host_installation_id: StableId("host.installation.alpha".to_owned()),
            host_event_ref: "event-reference-000001".to_owned(),
            host_session_ref: "session-reference-0001".to_owned(),
            host_interaction_ref: "interaction-reference-1".to_owned(),
        };
        let first = workflow_broker_native_interaction_replay_digest(&replay).expect("first");
        let same_after_rotation = replay.clone();
        assert_eq!(
            first,
            workflow_broker_native_interaction_replay_digest(&same_after_rotation)
                .expect("rotation-stable")
        );
        let mut switched = replay;
        switched.host_interaction_ref = "interaction-reference-2".to_owned();
        assert_ne!(
            first,
            workflow_broker_native_interaction_replay_digest(&switched).expect("switched")
        );
    }

    #[test]
    fn external_setup_block_is_typed_and_strict() {
        let blocked = WorkflowBrokerExternalSetupState::Blocked {
            reason: WorkflowBrokerExternalSetupBlockReason::SelectedHostUnavailable,
        };
        let value = serde_json::to_value(blocked).expect("blocked setup JSON");
        assert_eq!(value["state"], "blocked");
        assert_eq!(value["reason"], "selected_host_unavailable");
        assert_eq!(
            serde_json::from_value::<WorkflowBrokerExternalSetupState>(value.clone())
                .expect("blocked setup round-trip"),
            blocked
        );
        let mut unknown = value;
        unknown["host_evidence"] = serde_json::json!("fabricated");
        assert!(serde_json::from_value::<WorkflowBrokerExternalSetupState>(unknown).is_err());
    }

    #[test]
    fn admin_receipt_self_digest_is_canonical_and_fail_closed() {
        let mut document = WorkflowBrokerAdminReceiptDocument {
            schema_version: WORKFLOW_BROKER_ADMIN_RECEIPT_SCHEMA_VERSION.to_owned(),
            receipt: WorkflowBrokerAdminReceipt {
                operation_id: StableId("admin.operation.rotate.2".to_owned()),
                operation_digest: digest('d'),
                audience: "forge-core:workflow:project.alpha".to_owned(),
                project_id: StableId("project.alpha".to_owned()),
                workflow_id: StableId("workflow.governance".to_owned()),
                admin_credential_id: StableId("credential.admin.1".to_owned()),
                admin_credential_generation: 1,
                admin_public_key_fingerprint: digest('e'),
                signature_fingerprint: digest('f'),
                expected_registry_generation: 1,
                expected_registry_digest: Some(digest('a')),
                proposed_registry_generation: 2,
                proposed_registry_digest: digest('b'),
                native_authorization_descriptor_digest: digest('c'),
                native_authorization_replay_digest: digest('8'),
                authorized_at_unix: 1_900_000_100,
                previous_receipt_digest: Some(digest('9')),
                receipt_digest: String::new(),
            },
        };
        document.receipt.receipt_digest = document.digest().expect("receipt digest");
        document.validate_self_digest().expect("valid receipt");
        document.receipt.proposed_registry_generation = 3;
        assert_eq!(
            document
                .validate_self_digest()
                .expect_err("tampered receipt"),
            WorkflowBrokerContractError::ReceiptDigestMismatch
        );
    }
}
