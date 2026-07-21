#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

//! Host-neutral authority for the strict workflow broker public control plane.
//!
//! This module validates public broker/admin credential metadata, exact selected-
//! host bindings, current-registry CAS administration, canonical Ed25519
//! signatures, monotonic rotation/revocation, and rotation-stable native replay
//! identity. It owns no filesystem, transport, private key, generic signing API,
//! or mutation store. Callers must durably publish the returned registry and
//! receipt under their retained transaction authority.

use crate::workflow_origin_broker::{
    AuthorizedWorkflowBrokerRegistry, HistoricallyVerifiedWorkflowBrokerEvent,
    VerifiedWorkflowBrokerEvent, WorkflowBrokerEnrollmentDeclaration, WorkflowBrokerError,
    WorkflowBrokerEventEnvelope, WorkflowBrokerEventKind, WorkflowBrokerFreshnessPolicy,
    WorkflowBrokerIssuerEntry, WorkflowBrokerIssuerProfile, WorkflowBrokerIssuerStatus,
};
use ed25519_dalek::{Signature, VerifyingKey};
use forge_core_contracts::{
    workflow_broker_admin_operation_digest, workflow_broker_admin_operation_signing_bytes,
    workflow_broker_expected_audience, workflow_broker_native_admin_descriptor_digest,
    workflow_broker_native_admin_replay_digest, workflow_broker_native_interaction_replay_digest,
    workflow_broker_public_credential_digest, workflow_broker_public_registry_digest, StableId,
    WorkflowBrokerAdminOperation, WorkflowBrokerAdminOperationEnvelope, WorkflowBrokerAdminReceipt,
    WorkflowBrokerAdminReceiptDocument, WorkflowBrokerBoundOperation,
    WorkflowBrokerComponentStatus, WorkflowBrokerComponentStatusDocument,
    WorkflowBrokerCredentialProfile, WorkflowBrokerCredentialPurpose,
    WorkflowBrokerCredentialStatus, WorkflowBrokerHostBinding, WorkflowBrokerNativeAdminReplayKey,
    WorkflowBrokerNativeInteractionReplayKey, WorkflowBrokerPublicCredentialMetadata,
    WorkflowBrokerPublicCredentialMetadataDocument, WorkflowBrokerPublicRegistryDocument,
    WorkflowBrokerRecoveryState, WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION,
    WORKFLOW_BROKER_ADMIN_RECEIPT_SCHEMA_VERSION, WORKFLOW_BROKER_COMPONENT_STATUS_SCHEMA_VERSION,
    WORKFLOW_BROKER_PUBLIC_CREDENTIAL_SCHEMA_VERSION,
    WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION, WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAX_IDENTIFIER_BYTES: usize = 160;
const MAX_AUDIENCE_BYTES: usize = 256;
const MAX_PROTOCOL_VERSION_BYTES: usize = 128;
const MIN_OPAQUE_HANDLE_BYTES: usize = 16;
const MAX_OPAQUE_HANDLE_BYTES: usize = 192;
const MAX_ADMIN_OBSERVATION_TO_ISSUANCE_SECONDS: u64 = 300;
const DEFAULT_ADMIN_MAX_FUTURE_SKEW_SECONDS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdmittedPublicCredential {
    metadata: WorkflowBrokerPublicCredentialMetadata,
    verifying_key: VerifyingKey,
    public_key_fingerprint: String,
    metadata_digest: String,
}

/// Trusted expected binding supplied by the workflow kernel, never deserialized
/// from the broker event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowBrokerVerificationContext {
    pub audience: String,
    pub project_id: StableId,
    pub workflow_id: StableId,
    pub operation: WorkflowBrokerBoundOperation,
}

/// Narrow selected-host seam for a preconfigured external operator trust
/// anchor. The proposed registry and administration envelope cannot construct
/// or supply this capability: a future selected-host adapter must resolve it
/// from operator-controlled configuration before calling genesis authority.
pub trait WorkflowBrokerGenesisTrustAnchor {
    fn anchor_id(&self) -> &StableId;
    fn operator_subject_id(&self) -> &StableId;
    fn public_key_hex(&self) -> &str;
    fn host_binding(&self) -> &WorkflowBrokerHostBinding;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedBoundWorkflowBrokerEventAudit {
    pub registry_digest: String,
    pub registry_generation: u64,
    pub workflow_id: StableId,
    pub operation: WorkflowBrokerBoundOperation,
    pub credential_id: StableId,
    pub credential_generation: u64,
    pub credential_metadata_digest: String,
    pub broker_id: StableId,
    pub host_installation_id: StableId,
    pub native_interaction_replay_key: WorkflowBrokerNativeInteractionReplayKey,
    pub native_interaction_replay_digest: String,
}

/// Non-cloneable bound capability. Existing kernel consumers can deliberately
/// unwrap the already verified event; new consumers can retain the stronger
/// registry/host/replay audit alongside it.
pub struct VerifiedBoundWorkflowBrokerEvent {
    verified: VerifiedWorkflowBrokerEvent,
    audit: VerifiedBoundWorkflowBrokerEventAudit,
}

impl fmt::Debug for VerifiedBoundWorkflowBrokerEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedBoundWorkflowBrokerEvent")
            .field("verified", &self.verified)
            .field("audit", &self.audit)
            .finish()
    }
}

impl VerifiedBoundWorkflowBrokerEvent {
    #[must_use]
    pub const fn verified(&self) -> &VerifiedWorkflowBrokerEvent {
        &self.verified
    }

    #[must_use]
    pub const fn audit(&self) -> &VerifiedBoundWorkflowBrokerEventAudit {
        &self.audit
    }

    #[must_use]
    pub fn into_verified_event(self) -> VerifiedWorkflowBrokerEvent {
        self.verified
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        VerifiedWorkflowBrokerEvent,
        VerifiedBoundWorkflowBrokerEventAudit,
    ) {
        (self.verified, self.audit)
    }
}

/// Recovery-only bound capability. It retains the rotation-stable replay
/// identity needed to repair Store state, but cannot authorize a new ledger
/// mutation.
pub struct HistoricallyVerifiedBoundWorkflowBrokerEvent {
    verified: HistoricallyVerifiedWorkflowBrokerEvent,
    audit: VerifiedBoundWorkflowBrokerEventAudit,
}

impl fmt::Debug for HistoricallyVerifiedBoundWorkflowBrokerEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HistoricallyVerifiedBoundWorkflowBrokerEvent")
            .field("verified", &self.verified)
            .field("audit", &self.audit)
            .finish()
    }
}

impl HistoricallyVerifiedBoundWorkflowBrokerEvent {
    #[must_use]
    pub const fn audit(&self) -> &VerifiedBoundWorkflowBrokerEventAudit {
        &self.audit
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        HistoricallyVerifiedWorkflowBrokerEvent,
        VerifiedBoundWorkflowBrokerEventAudit,
    ) {
        (self.verified, self.audit)
    }
}

/// Validate one exact selected-host binding through registry-admission rules.
pub fn validate_workflow_broker_host_binding(
    binding: &WorkflowBrokerHostBinding,
) -> Result<(), WorkflowBrokerControlError> {
    validate_host_binding(binding)
}

/// Validate a standalone, versioned public credential projection through the
/// same semantic Ed25519 and metadata checks used by registry admission.
pub fn validate_workflow_broker_public_credential_document(
    document: &WorkflowBrokerPublicCredentialMetadataDocument,
) -> Result<String, WorkflowBrokerControlError> {
    if document.schema_version != WORKFLOW_BROKER_PUBLIC_CREDENTIAL_SCHEMA_VERSION {
        return Err(WorkflowBrokerControlError::UnsupportedCredentialSchema(
            document.schema_version.clone(),
        ));
    }
    let admitted = admit_credentials(std::slice::from_ref(&document.credential))?;
    Ok(admitted
        .into_iter()
        .next()
        .expect("one credential was admitted")
        .metadata_digest)
}

/// Admitted strict public registry and its semantically validated Ed25519 keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedWorkflowBrokerControlPlane {
    document: WorkflowBrokerPublicRegistryDocument,
    registry_digest: String,
    credentials: Vec<AdmittedPublicCredential>,
    event_registry: AuthorizedWorkflowBrokerRegistry,
}

impl AuthorizedWorkflowBrokerControlPlane {
    /// Admit one strict registry for an exact project/workflow binding.
    pub fn from_document_for_binding(
        document: WorkflowBrokerPublicRegistryDocument,
        expected_audience: &str,
        expected_project_id: &StableId,
        expected_workflow_id: &StableId,
    ) -> Result<Self, WorkflowBrokerControlError> {
        if document.audience != expected_audience {
            return Err(WorkflowBrokerControlError::BindingMismatch("audience"));
        }
        if &document.project_id != expected_project_id {
            return Err(WorkflowBrokerControlError::BindingMismatch("project_id"));
        }
        if &document.workflow_id != expected_workflow_id {
            return Err(WorkflowBrokerControlError::BindingMismatch("workflow_id"));
        }
        Self::from_document(document)
    }

    /// Admit one public registry. Every key is parsed as an Ed25519 verifying
    /// key, weak keys are rejected, and all lifecycle chains are validated.
    pub fn from_document(
        document: WorkflowBrokerPublicRegistryDocument,
    ) -> Result<Self, WorkflowBrokerControlError> {
        validate_registry_header(&document)?;
        let registry_digest =
            workflow_broker_public_registry_digest(&document).map_err(contract_error)?;
        let credentials = admit_credentials(&document.credentials)?;
        validate_credential_chains(&credentials)?;
        validate_active_authorities(&credentials)?;

        let legacy_issuers = credentials
            .iter()
            .filter(|credential| {
                credential.metadata.purpose == WorkflowBrokerCredentialPurpose::EventIssuer
            })
            .map(|credential| WorkflowBrokerIssuerEntry {
                issuer_id: credential.metadata.subject_id.clone(),
                profile: event_profile(credential.metadata.profile)
                    .expect("validated event credential profile"),
                public_key_hex: credential.metadata.public_key_hex.clone(),
                status: match credential.metadata.status {
                    WorkflowBrokerCredentialStatus::Active => WorkflowBrokerIssuerStatus::Active,
                    WorkflowBrokerCredentialStatus::Revoked => WorkflowBrokerIssuerStatus::Revoked,
                },
                enrollment: WorkflowBrokerEnrollmentDeclaration {
                    ceremony_ref: format!(
                        "admin-operation:{}",
                        credential.metadata.enrollment_operation_id.0
                    ),
                    ceremony_digest: credential.metadata_digest.clone(),
                    declared_at_unix: credential.metadata.not_before_unix,
                },
            })
            .collect::<Vec<_>>();
        if legacy_issuers.is_empty() {
            return Err(WorkflowBrokerControlError::MissingEventCredential);
        }
        let event_registry = AuthorizedWorkflowBrokerRegistry::from_document_for_audience(
            crate::workflow_origin_broker::WorkflowBrokerRegistryDocument {
                schema_version:
                    crate::workflow_origin_broker::WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION
                        .to_owned(),
                audience: document.audience.clone(),
                issuers: legacy_issuers,
            },
            &document.audience,
        )
        .map_err(WorkflowBrokerControlError::EventAuthority)?;

        Ok(Self {
            document,
            registry_digest,
            credentials,
            event_registry,
        })
    }

    #[must_use]
    pub const fn document(&self) -> &WorkflowBrokerPublicRegistryDocument {
        &self.document
    }

    #[must_use]
    pub fn registry_digest(&self) -> &str {
        &self.registry_digest
    }

    /// Public status derived only from admitted public metadata.
    #[must_use]
    pub fn component_status(
        &self,
        last_admin_receipt_digest: Option<String>,
        recovery: WorkflowBrokerRecoveryState,
    ) -> WorkflowBrokerComponentStatusDocument {
        let active_admin = self
            .credentials
            .iter()
            .find(|credential| {
                credential.metadata.purpose
                    == WorkflowBrokerCredentialPurpose::RegistryAdministrator
                    && credential.metadata.status == WorkflowBrokerCredentialStatus::Active
            })
            .expect("registry admission requires exactly one active administrator");
        WorkflowBrokerComponentStatusDocument {
            schema_version: WORKFLOW_BROKER_COMPONENT_STATUS_SCHEMA_VERSION.to_owned(),
            status: WorkflowBrokerComponentStatus {
                audience: self.document.audience.clone(),
                project_id: self.document.project_id.clone(),
                workflow_id: self.document.workflow_id.clone(),
                registry_generation: self.document.registry_generation,
                registry_digest: self.registry_digest.clone(),
                required_event_schema_version: self.document.required_event_schema_version.clone(),
                active_event_credential_count: self
                    .credentials
                    .iter()
                    .filter(|credential| {
                        credential.metadata.purpose == WorkflowBrokerCredentialPurpose::EventIssuer
                            && credential.metadata.status == WorkflowBrokerCredentialStatus::Active
                    })
                    .count(),
                retained_revoked_credential_count: self
                    .credentials
                    .iter()
                    .filter(|credential| {
                        credential.metadata.status == WorkflowBrokerCredentialStatus::Revoked
                    })
                    .count(),
                active_admin_credential_id: active_admin.metadata.credential_id.clone(),
                last_admin_receipt_digest,
                recovery,
            },
        }
    }

    /// Verify an existing v0.2 broker event and add exact registry, workflow,
    /// host-installation, operation, credential-generation, and stable native
    /// replay bindings.
    pub fn verify_bound_event(
        &self,
        envelope: WorkflowBrokerEventEnvelope,
        context: &WorkflowBrokerVerificationContext,
        now_unix: i64,
        freshness: WorkflowBrokerFreshnessPolicy,
    ) -> Result<VerifiedBoundWorkflowBrokerEvent, WorkflowBrokerControlError> {
        self.validate_context(context)?;
        if envelope.schema_version != self.document.required_event_schema_version {
            return Err(WorkflowBrokerControlError::EventSchemaDowngrade);
        }

        // The existing authority verifies the Ed25519 signature before semantic
        // validation. Stronger binding checks occur only after that capability
        // exists, preserving deterministic authentication-first behavior.
        let verified = self
            .event_registry
            .verify_event(envelope, &context.project_id, now_unix, freshness)
            .map_err(WorkflowBrokerControlError::EventAuthority)?;
        let operation = bound_operation(verified.audit().event_kind);
        if operation != context.operation {
            return Err(WorkflowBrokerControlError::OperationMismatch);
        }
        let credential = self
            .credentials
            .iter()
            .find(|credential| {
                credential.metadata.purpose == WorkflowBrokerCredentialPurpose::EventIssuer
                    && credential.metadata.subject_id == verified.audit().issuer_id
            })
            .ok_or(WorkflowBrokerControlError::UnknownCredential)?;
        if !credential
            .metadata
            .allowed_operations
            .contains(&context.operation)
        {
            return Err(WorkflowBrokerControlError::OperationNotAuthorized);
        }
        if verified.audit().issued_at_unix < credential.metadata.not_before_unix {
            return Err(WorkflowBrokerControlError::CredentialNotYetValid);
        }
        let provenance = verified
            .audit()
            .native_host_provenance
            .as_ref()
            .ok_or(WorkflowBrokerControlError::MissingNativeProvenance)?;
        validate_event_host_binding(provenance, &credential.metadata.host_binding)?;
        let replay_key = WorkflowBrokerNativeInteractionReplayKey {
            audience: self.document.audience.clone(),
            project_id: self.document.project_id.clone(),
            workflow_id: self.document.workflow_id.clone(),
            broker_id: credential.metadata.broker_id.clone(),
            host_kind: provenance.host_kind,
            adapter_id: provenance.adapter_id.clone(),
            host_installation_id: credential
                .metadata
                .host_binding
                .host_installation_id
                .clone(),
            host_event_ref: provenance.host_event_ref.clone(),
            host_session_ref: provenance.host_session_ref.clone(),
            host_interaction_ref: provenance.host_interaction_ref.clone(),
        };
        let replay_digest = workflow_broker_native_interaction_replay_digest(&replay_key)
            .map_err(contract_error)?;
        let audit = VerifiedBoundWorkflowBrokerEventAudit {
            registry_digest: self.registry_digest.clone(),
            registry_generation: self.document.registry_generation,
            workflow_id: self.document.workflow_id.clone(),
            operation,
            credential_id: credential.metadata.credential_id.clone(),
            credential_generation: credential.metadata.key_generation,
            credential_metadata_digest: credential.metadata_digest.clone(),
            broker_id: credential.metadata.broker_id.clone(),
            host_installation_id: credential
                .metadata
                .host_binding
                .host_installation_id
                .clone(),
            native_interaction_replay_key: replay_key,
            native_interaction_replay_digest: replay_digest,
        };
        Ok(VerifiedBoundWorkflowBrokerEvent { verified, audit })
    }

    /// Verify an expired or now-revoked strict event only for exact durable
    /// replay repair. The same host, operation, credential-generation, registry,
    /// and rotation-stable native-interaction bindings are retained.
    pub fn verify_bound_event_for_recovery(
        &self,
        envelope: WorkflowBrokerEventEnvelope,
        context: &WorkflowBrokerVerificationContext,
    ) -> Result<HistoricallyVerifiedBoundWorkflowBrokerEvent, WorkflowBrokerControlError> {
        self.validate_context(context)?;
        if envelope.schema_version != self.document.required_event_schema_version {
            return Err(WorkflowBrokerControlError::EventSchemaDowngrade);
        }
        let verified = self
            .event_registry
            .verify_event_for_recovery(envelope, &context.project_id)
            .map_err(WorkflowBrokerControlError::EventAuthority)?;
        let verified_audit = verified.audit();
        let operation = bound_operation(verified_audit.event_kind);
        if operation != context.operation {
            return Err(WorkflowBrokerControlError::OperationMismatch);
        }
        let credential = self
            .credentials
            .iter()
            .find(|credential| {
                credential.metadata.purpose == WorkflowBrokerCredentialPurpose::EventIssuer
                    && credential.metadata.subject_id == verified_audit.issuer_id
            })
            .ok_or(WorkflowBrokerControlError::UnknownCredential)?;
        if !credential
            .metadata
            .allowed_operations
            .contains(&context.operation)
        {
            return Err(WorkflowBrokerControlError::OperationNotAuthorized);
        }
        if verified_audit.issued_at_unix < credential.metadata.not_before_unix {
            return Err(WorkflowBrokerControlError::CredentialNotYetValid);
        }
        let provenance = verified_audit
            .native_host_provenance
            .as_ref()
            .ok_or(WorkflowBrokerControlError::MissingNativeProvenance)?;
        validate_event_host_binding(provenance, &credential.metadata.host_binding)?;
        let replay_key = WorkflowBrokerNativeInteractionReplayKey {
            audience: self.document.audience.clone(),
            project_id: self.document.project_id.clone(),
            workflow_id: self.document.workflow_id.clone(),
            broker_id: credential.metadata.broker_id.clone(),
            host_kind: provenance.host_kind,
            adapter_id: provenance.adapter_id.clone(),
            host_installation_id: credential
                .metadata
                .host_binding
                .host_installation_id
                .clone(),
            host_event_ref: provenance.host_event_ref.clone(),
            host_session_ref: provenance.host_session_ref.clone(),
            host_interaction_ref: provenance.host_interaction_ref.clone(),
        };
        let replay_digest = workflow_broker_native_interaction_replay_digest(&replay_key)
            .map_err(contract_error)?;
        let audit = VerifiedBoundWorkflowBrokerEventAudit {
            registry_digest: self.registry_digest.clone(),
            registry_generation: self.document.registry_generation,
            workflow_id: self.document.workflow_id.clone(),
            operation,
            credential_id: credential.metadata.credential_id.clone(),
            credential_generation: credential.metadata.key_generation,
            credential_metadata_digest: credential.metadata_digest.clone(),
            broker_id: credential.metadata.broker_id.clone(),
            host_installation_id: credential
                .metadata
                .host_binding
                .host_installation_id
                .clone(),
            native_interaction_replay_key: replay_key,
            native_interaction_replay_digest: replay_digest,
        };
        Ok(HistoricallyVerifiedBoundWorkflowBrokerEvent { verified, audit })
    }

    /// Authorize installation of the first strict public registry. Trust begins
    /// at a preconfigured external operator anchor supplied by the selected-host
    /// adapter, never at a key declared by the proposed registry or envelope.
    /// Forge neither creates nor accepts private key material during bootstrap.
    #[allow(clippy::needless_pass_by_value)]
    pub fn authorize_genesis(
        trust_anchor: &dyn WorkflowBrokerGenesisTrustAnchor,
        envelope: WorkflowBrokerAdminOperationEnvelope,
        proposed_document: WorkflowBrokerPublicRegistryDocument,
        now_unix: i64,
    ) -> Result<AuthorizedWorkflowBrokerRegistryAdvance, WorkflowBrokerControlError> {
        Self::authorize_genesis_inner(trust_anchor, &envelope, proposed_document, now_unix, true)
    }

    /// Recover a previously prepared generation-one installation. Reacquiring
    /// the same external trust anchor is mandatory; durable response state must
    /// not silently reopen the former self-trusting bootstrap path.
    #[allow(clippy::needless_pass_by_value)]
    pub fn recover_authorized_genesis(
        trust_anchor: &dyn WorkflowBrokerGenesisTrustAnchor,
        envelope: WorkflowBrokerAdminOperationEnvelope,
        proposed_document: WorkflowBrokerPublicRegistryDocument,
    ) -> Result<AuthorizedWorkflowBrokerRegistryAdvance, WorkflowBrokerControlError> {
        Self::authorize_genesis_inner(trust_anchor, &envelope, proposed_document, 0, false)
    }

    fn authorize_genesis_inner(
        trust_anchor: &dyn WorkflowBrokerGenesisTrustAnchor,
        envelope: &WorkflowBrokerAdminOperationEnvelope,
        proposed_document: WorkflowBrokerPublicRegistryDocument,
        now_unix: i64,
        enforce_freshness: bool,
    ) -> Result<AuthorizedWorkflowBrokerRegistryAdvance, WorkflowBrokerControlError> {
        let proposed = AuthorizedWorkflowBrokerControlPlane::from_document(proposed_document)?;
        let WorkflowBrokerAdminOperation::Initialize {
            active_admin_credential_id,
        } = &envelope.operation
        else {
            return Err(WorkflowBrokerControlError::InvalidAdminTransition);
        };
        let admin = proposed
            .credentials
            .iter()
            .find(|credential| credential.metadata.credential_id == envelope.admin_credential_id)
            .ok_or(WorkflowBrokerControlError::UnknownCredential)?;
        validate_genesis_trust_anchor(trust_anchor, admin, envelope)?;
        let verified_admin = proposed.verify_admin_signature(envelope)?;
        proposed.validate_admin_envelope(
            envelope,
            verified_admin,
            now_unix,
            enforce_freshness,
            false,
        )?;
        if envelope.expected_registry_generation != 0
            || envelope.expected_registry_digest.is_some()
            || envelope.proposed_registry_generation != 1
            || envelope.proposed_registry_digest != proposed.registry_digest
            || proposed.document.registry_generation != 1
            || proposed.document.previous_registry_digest.is_some()
            || &verified_admin.metadata.credential_id != active_admin_credential_id
            || proposed.document.credentials.iter().any(|credential| {
                credential.enrollment_operation_id != envelope.operation_id
                    || credential.not_before_unix > envelope.issued_at_unix
            })
        {
            return Err(WorkflowBrokerControlError::ProposedRegistryMismatch);
        }
        let (receipt, operation_digest) = build_admin_receipt(envelope, verified_admin, None)?;
        Ok(AuthorizedWorkflowBrokerRegistryAdvance {
            proposed,
            receipt,
            operation_digest,
        })
    }

    /// Authorize one exact current-registry CAS transition. Signature and native
    /// admin provenance are verified before transition semantics. The returned
    /// receipt is deterministic for exact retries because its time is the signed
    /// issuance time, not a later response clock.
    #[allow(clippy::needless_pass_by_value)]
    pub fn authorize_admin_transition(
        &self,
        envelope: WorkflowBrokerAdminOperationEnvelope,
        proposed_document: WorkflowBrokerPublicRegistryDocument,
        now_unix: i64,
        previous_receipt_digest: Option<String>,
    ) -> Result<AuthorizedWorkflowBrokerRegistryAdvance, WorkflowBrokerControlError> {
        self.authorize_admin_transition_inner(
            &envelope,
            proposed_document,
            now_unix,
            previous_receipt_digest,
            true,
        )
    }

    /// Reconstruct the exact transition authority retained in a prepared durable
    /// journal after process interruption. Signature, native descriptor, CAS,
    /// transition, and receipt bindings remain mandatory; only wall-clock
    /// freshness is not re-applied to already prepared work.
    #[allow(clippy::needless_pass_by_value)]
    pub fn recover_authorized_admin_transition(
        &self,
        envelope: WorkflowBrokerAdminOperationEnvelope,
        proposed_document: WorkflowBrokerPublicRegistryDocument,
        previous_receipt_digest: Option<String>,
    ) -> Result<AuthorizedWorkflowBrokerRegistryAdvance, WorkflowBrokerControlError> {
        self.authorize_admin_transition_inner(
            &envelope,
            proposed_document,
            0,
            previous_receipt_digest,
            false,
        )
    }

    fn authorize_admin_transition_inner(
        &self,
        envelope: &WorkflowBrokerAdminOperationEnvelope,
        proposed_document: WorkflowBrokerPublicRegistryDocument,
        now_unix: i64,
        previous_receipt_digest: Option<String>,
        enforce_freshness: bool,
    ) -> Result<AuthorizedWorkflowBrokerRegistryAdvance, WorkflowBrokerControlError> {
        if matches!(
            &envelope.operation,
            WorkflowBrokerAdminOperation::Initialize { .. }
        ) {
            return Err(WorkflowBrokerControlError::InvalidAdminTransition);
        }
        let admin = self.verify_admin_signature(envelope)?;
        self.validate_admin_envelope(envelope, admin, now_unix, enforce_freshness, false)?;
        let proposed = AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            proposed_document,
            &self.document.audience,
            &self.document.project_id,
            &self.document.workflow_id,
        )?;
        if envelope.expected_registry_generation != self.document.registry_generation
            || envelope.expected_registry_digest.as_deref() != Some(self.registry_digest.as_str())
        {
            return Err(WorkflowBrokerControlError::RegistryCasMismatch);
        }
        let expected_next = self
            .document
            .registry_generation
            .checked_add(1)
            .ok_or(WorkflowBrokerControlError::RegistryGenerationOverflow)?;
        if envelope.proposed_registry_generation != expected_next
            || proposed.document.registry_generation != expected_next
            || proposed.document.previous_registry_digest.as_deref()
                != Some(self.registry_digest.as_str())
            || envelope.proposed_registry_digest != proposed.registry_digest
        {
            return Err(WorkflowBrokerControlError::ProposedRegistryMismatch);
        }
        validate_admin_transition(&self.document, &proposed.document, envelope)?;

        let (receipt, operation_digest) =
            build_admin_receipt(envelope, admin, previous_receipt_digest)?;
        Ok(AuthorizedWorkflowBrokerRegistryAdvance {
            proposed,
            receipt,
            operation_digest,
        })
    }

    /// Verify an exact response-loss retry against the already-published
    /// proposed registry and durable receipt. This returns no mutation authority.
    pub fn verify_applied_admin_retry(
        &self,
        envelope: &WorkflowBrokerAdminOperationEnvelope,
        receipt: &WorkflowBrokerAdminReceiptDocument,
    ) -> Result<VerifiedWorkflowBrokerAdminRetry, WorkflowBrokerControlError> {
        receipt.validate_self_digest().map_err(contract_error)?;
        let admin = self.verify_admin_signature(envelope)?;
        self.validate_admin_envelope(envelope, admin, 0, false, true)?;
        if envelope.proposed_registry_generation != self.document.registry_generation
            || envelope.proposed_registry_digest != self.registry_digest
            || self.document.previous_registry_digest.as_deref()
                != envelope.expected_registry_digest.as_deref()
        {
            return Err(WorkflowBrokerControlError::AdminRetryMismatch);
        }
        validate_applied_operation_projection(&self.document, envelope)?;
        let operation_digest =
            workflow_broker_admin_operation_digest(envelope).map_err(contract_error)?;
        let native = &envelope.native_authorization;
        let native_replay_digest =
            workflow_broker_native_admin_replay_digest(&WorkflowBrokerNativeAdminReplayKey {
                audience: envelope.audience.clone(),
                project_id: envelope.project_id.clone(),
                workflow_id: envelope.workflow_id.clone(),
                host_kind: native.host_kind,
                adapter_id: native.adapter_id.clone(),
                host_installation_id: native.host_installation_id.clone(),
                admin_session_ref: native.admin_session_ref.clone(),
                admin_interaction_ref: native.admin_interaction_ref.clone(),
            })
            .map_err(contract_error)?;
        let expected = &receipt.receipt;
        if expected.operation_id != envelope.operation_id
            || expected.operation_digest != operation_digest
            || expected.audience != envelope.audience
            || expected.project_id != envelope.project_id
            || expected.workflow_id != envelope.workflow_id
            || expected.admin_credential_id != admin.metadata.credential_id
            || expected.admin_credential_generation != admin.metadata.key_generation
            || expected.admin_public_key_fingerprint != admin.public_key_fingerprint
            || expected.expected_registry_generation != envelope.expected_registry_generation
            || expected.expected_registry_digest != envelope.expected_registry_digest
            || expected.proposed_registry_generation != envelope.proposed_registry_generation
            || expected.proposed_registry_digest != envelope.proposed_registry_digest
            || expected.native_authorization_descriptor_digest
                != envelope.native_authorization.descriptor_digest
            || expected.native_authorization_replay_digest != native_replay_digest
            || expected.authorized_at_unix != envelope.issued_at_unix
        {
            return Err(WorkflowBrokerControlError::AdminRetryMismatch);
        }
        let signature = decode_lower_hex_fixed::<64>(&envelope.signature)
            .ok_or(WorkflowBrokerControlError::InvalidSignatureEncoding)?;
        if expected.signature_fingerprint != raw_digest(&signature) {
            return Err(WorkflowBrokerControlError::AdminRetryMismatch);
        }
        Ok(VerifiedWorkflowBrokerAdminRetry {
            receipt: receipt.clone(),
        })
    }

    /// Re-verify one retained signed administration envelope and receipt against
    /// the current registry's retained public credential history. This grants no
    /// mutation authority and is used to fail closed when reopening the durable
    /// administration journal after later registry generations.
    pub fn verify_historical_admin_receipt(
        &self,
        envelope: &WorkflowBrokerAdminOperationEnvelope,
        receipt: &WorkflowBrokerAdminReceiptDocument,
    ) -> Result<VerifiedWorkflowBrokerAdminRetry, WorkflowBrokerControlError> {
        receipt.validate_self_digest().map_err(contract_error)?;
        let admin = self.verify_admin_signature(envelope)?;
        self.validate_admin_envelope(envelope, admin, 0, false, true)?;
        validate_applied_operation_projection(&self.document, envelope)?;
        let (expected, _) = build_admin_receipt(
            envelope,
            admin,
            receipt.receipt.previous_receipt_digest.clone(),
        )?;
        if &expected != receipt {
            return Err(WorkflowBrokerControlError::AdminRetryMismatch);
        }
        Ok(VerifiedWorkflowBrokerAdminRetry {
            receipt: receipt.clone(),
        })
    }

    fn validate_context(
        &self,
        context: &WorkflowBrokerVerificationContext,
    ) -> Result<(), WorkflowBrokerControlError> {
        if context.audience != self.document.audience {
            return Err(WorkflowBrokerControlError::BindingMismatch("audience"));
        }
        if context.project_id != self.document.project_id {
            return Err(WorkflowBrokerControlError::BindingMismatch("project_id"));
        }
        if context.workflow_id != self.document.workflow_id {
            return Err(WorkflowBrokerControlError::BindingMismatch("workflow_id"));
        }
        Ok(())
    }

    fn verify_admin_signature<'a>(
        &'a self,
        envelope: &WorkflowBrokerAdminOperationEnvelope,
    ) -> Result<&'a AdmittedPublicCredential, WorkflowBrokerControlError> {
        if envelope.schema_version != WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION {
            return Err(WorkflowBrokerControlError::UnsupportedAdminSchema(
                envelope.schema_version.clone(),
            ));
        }
        let admin = self
            .credentials
            .iter()
            .find(|credential| credential.metadata.credential_id == envelope.admin_credential_id)
            .ok_or(WorkflowBrokerControlError::UnknownCredential)?;
        let signature = decode_lower_hex_fixed::<64>(&envelope.signature)
            .ok_or(WorkflowBrokerControlError::InvalidSignatureEncoding)?;
        admin
            .verifying_key
            .verify_strict(
                &workflow_broker_admin_operation_signing_bytes(envelope).map_err(contract_error)?,
                &Signature::from_bytes(&signature),
            )
            .map_err(|_| WorkflowBrokerControlError::InvalidSignature)?;
        Ok(admin)
    }

    fn validate_admin_envelope(
        &self,
        envelope: &WorkflowBrokerAdminOperationEnvelope,
        admin: &AdmittedPublicCredential,
        now_unix: i64,
        enforce_freshness: bool,
        allow_historical_admin: bool,
    ) -> Result<(), WorkflowBrokerControlError> {
        if admin.metadata.purpose != WorkflowBrokerCredentialPurpose::RegistryAdministrator {
            return Err(WorkflowBrokerControlError::WrongCredentialPurpose);
        }
        if admin.metadata.status != WorkflowBrokerCredentialStatus::Active
            && !(allow_historical_admin
                && admin.metadata.status == WorkflowBrokerCredentialStatus::Revoked
                && admin
                    .metadata
                    .revoked_at_unix
                    .is_some_and(|revoked_at| envelope.issued_at_unix <= revoked_at))
        {
            return Err(WorkflowBrokerControlError::CredentialRevoked);
        }
        if admin.metadata.key_generation != envelope.admin_credential_generation {
            return Err(WorkflowBrokerControlError::CredentialGenerationMismatch);
        }
        if envelope.audience != self.document.audience {
            return Err(WorkflowBrokerControlError::BindingMismatch("audience"));
        }
        if envelope.project_id != self.document.project_id {
            return Err(WorkflowBrokerControlError::BindingMismatch("project_id"));
        }
        if envelope.workflow_id != self.document.workflow_id {
            return Err(WorkflowBrokerControlError::BindingMismatch("workflow_id"));
        }
        validate_identifier("operation_id", &envelope.operation_id.0)?;
        validate_nonce(&envelope.nonce)?;
        if let Some(expected) = envelope.expected_registry_digest.as_deref() {
            require_digest("expected_registry_digest", expected)?;
        }
        require_digest(
            "proposed_registry_digest",
            &envelope.proposed_registry_digest,
        )?;
        if envelope.proposed_registry_generation == 0 {
            return Err(WorkflowBrokerControlError::InvalidField {
                field: "proposed_registry_generation",
                reason: "must be greater than zero",
            });
        }
        if enforce_freshness {
            validate_admin_freshness(envelope, now_unix)?;
        } else if envelope.issued_at_unix == 0
            || envelope.expires_at_unix <= envelope.issued_at_unix
        {
            return Err(WorkflowBrokerControlError::AdminFreshnessOutOfBounds);
        }
        validate_native_admin_authorization(envelope, &admin.metadata.host_binding)?;
        match &envelope.operation {
            WorkflowBrokerAdminOperation::Initialize {
                active_admin_credential_id,
            } => {
                validate_identifier(
                    "operation.active_admin_credential_id",
                    &active_admin_credential_id.0,
                )?;
            }
            WorkflowBrokerAdminOperation::Enroll { credential_id } => {
                validate_identifier("operation.credential_id", &credential_id.0)?;
            }
            WorkflowBrokerAdminOperation::Rotate {
                current_credential_id,
                replacement_credential_id,
            } => {
                validate_identifier("operation.current_credential_id", &current_credential_id.0)?;
                validate_identifier(
                    "operation.replacement_credential_id",
                    &replacement_credential_id.0,
                )?;
                if current_credential_id == replacement_credential_id {
                    return Err(WorkflowBrokerControlError::InvalidAdminTransition);
                }
            }
            WorkflowBrokerAdminOperation::Revoke {
                credential_id,
                reason_code,
            } => {
                validate_identifier("operation.credential_id", &credential_id.0)?;
                validate_identifier("operation.reason_code", &reason_code.0)?;
            }
        }
        Ok(())
    }
}

fn build_admin_receipt(
    envelope: &WorkflowBrokerAdminOperationEnvelope,
    admin: &AdmittedPublicCredential,
    previous_receipt_digest: Option<String>,
) -> Result<(WorkflowBrokerAdminReceiptDocument, String), WorkflowBrokerControlError> {
    if let Some(previous) = previous_receipt_digest.as_deref() {
        require_digest("previous_receipt_digest", previous)?;
    }
    let operation_digest =
        workflow_broker_admin_operation_digest(envelope).map_err(contract_error)?;
    let signature_bytes = decode_lower_hex_fixed::<64>(&envelope.signature)
        .ok_or(WorkflowBrokerControlError::InvalidSignatureEncoding)?;
    let native = &envelope.native_authorization;
    let native_replay_digest =
        workflow_broker_native_admin_replay_digest(&WorkflowBrokerNativeAdminReplayKey {
            audience: envelope.audience.clone(),
            project_id: envelope.project_id.clone(),
            workflow_id: envelope.workflow_id.clone(),
            host_kind: native.host_kind,
            adapter_id: native.adapter_id.clone(),
            host_installation_id: native.host_installation_id.clone(),
            admin_session_ref: native.admin_session_ref.clone(),
            admin_interaction_ref: native.admin_interaction_ref.clone(),
        })
        .map_err(contract_error)?;
    let mut receipt = WorkflowBrokerAdminReceiptDocument {
        schema_version: WORKFLOW_BROKER_ADMIN_RECEIPT_SCHEMA_VERSION.to_owned(),
        receipt: WorkflowBrokerAdminReceipt {
            operation_id: envelope.operation_id.clone(),
            operation_digest: operation_digest.clone(),
            audience: envelope.audience.clone(),
            project_id: envelope.project_id.clone(),
            workflow_id: envelope.workflow_id.clone(),
            admin_credential_id: admin.metadata.credential_id.clone(),
            admin_credential_generation: admin.metadata.key_generation,
            admin_public_key_fingerprint: admin.public_key_fingerprint.clone(),
            signature_fingerprint: raw_digest(&signature_bytes),
            expected_registry_generation: envelope.expected_registry_generation,
            expected_registry_digest: envelope.expected_registry_digest.clone(),
            proposed_registry_generation: envelope.proposed_registry_generation,
            proposed_registry_digest: envelope.proposed_registry_digest.clone(),
            native_authorization_descriptor_digest: envelope
                .native_authorization
                .descriptor_digest
                .clone(),
            native_authorization_replay_digest: native_replay_digest,
            authorized_at_unix: envelope.issued_at_unix,
            previous_receipt_digest,
            receipt_digest: String::new(),
        },
    };
    receipt.receipt.receipt_digest = receipt.digest().map_err(contract_error)?;
    Ok((receipt, operation_digest))
}

/// Opaque result that pairs the exact admitted successor registry and receipt.
pub struct AuthorizedWorkflowBrokerRegistryAdvance {
    proposed: AuthorizedWorkflowBrokerControlPlane,
    receipt: WorkflowBrokerAdminReceiptDocument,
    operation_digest: String,
}

impl fmt::Debug for AuthorizedWorkflowBrokerRegistryAdvance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthorizedWorkflowBrokerRegistryAdvance")
            .field("proposed_registry_digest", &self.proposed.registry_digest)
            .field("receipt", &self.receipt)
            .field("operation_digest", &self.operation_digest)
            .finish()
    }
}

impl AuthorizedWorkflowBrokerRegistryAdvance {
    #[must_use]
    pub const fn proposed(&self) -> &AuthorizedWorkflowBrokerControlPlane {
        &self.proposed
    }

    #[must_use]
    pub const fn receipt(&self) -> &WorkflowBrokerAdminReceiptDocument {
        &self.receipt
    }

    #[must_use]
    pub fn operation_digest(&self) -> &str {
        &self.operation_digest
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        AuthorizedWorkflowBrokerControlPlane,
        WorkflowBrokerAdminReceiptDocument,
    ) {
        (self.proposed, self.receipt)
    }
}

/// Recovery-only proof for an exact already-applied operation.
pub struct VerifiedWorkflowBrokerAdminRetry {
    receipt: WorkflowBrokerAdminReceiptDocument,
}

impl fmt::Debug for VerifiedWorkflowBrokerAdminRetry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWorkflowBrokerAdminRetry")
            .field("receipt", &self.receipt)
            .finish()
    }
}

impl VerifiedWorkflowBrokerAdminRetry {
    #[must_use]
    pub const fn receipt(&self) -> &WorkflowBrokerAdminReceiptDocument {
        &self.receipt
    }
}

fn validate_genesis_trust_anchor(
    trust_anchor: &dyn WorkflowBrokerGenesisTrustAnchor,
    proposed_admin: &AdmittedPublicCredential,
    envelope: &WorkflowBrokerAdminOperationEnvelope,
) -> Result<(), WorkflowBrokerControlError> {
    validate_identifier(
        "genesis_trust_anchor.anchor_id",
        &trust_anchor.anchor_id().0,
    )?;
    validate_identifier(
        "genesis_trust_anchor.operator_subject_id",
        &trust_anchor.operator_subject_id().0,
    )?;
    validate_host_binding(trust_anchor.host_binding())?;
    let anchor_key_bytes = decode_lower_hex_fixed::<32>(trust_anchor.public_key_hex())
        .ok_or(WorkflowBrokerControlError::InvalidGenesisTrustAnchor)?;
    let anchor_key = VerifyingKey::from_bytes(&anchor_key_bytes)
        .map_err(|_| WorkflowBrokerControlError::InvalidGenesisTrustAnchor)?;
    if anchor_key.is_weak() {
        return Err(WorkflowBrokerControlError::InvalidGenesisTrustAnchor);
    }
    if proposed_admin.metadata.purpose != WorkflowBrokerCredentialPurpose::RegistryAdministrator {
        return Err(WorkflowBrokerControlError::WrongCredentialPurpose);
    }
    if &proposed_admin.metadata.subject_id != trust_anchor.operator_subject_id()
        || &proposed_admin.metadata.host_binding != trust_anchor.host_binding()
        || proposed_admin.verifying_key != anchor_key
    {
        return Err(WorkflowBrokerControlError::GenesisTrustAnchorMismatch);
    }
    let signature = decode_lower_hex_fixed::<64>(&envelope.signature)
        .ok_or(WorkflowBrokerControlError::InvalidSignatureEncoding)?;
    anchor_key
        .verify_strict(
            &workflow_broker_admin_operation_signing_bytes(envelope).map_err(contract_error)?,
            &Signature::from_bytes(&signature),
        )
        .map_err(|_| WorkflowBrokerControlError::InvalidSignature)
}

fn validate_registry_header(
    document: &WorkflowBrokerPublicRegistryDocument,
) -> Result<(), WorkflowBrokerControlError> {
    if document.schema_version != WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION {
        return Err(WorkflowBrokerControlError::UnsupportedRegistrySchema(
            document.schema_version.clone(),
        ));
    }
    validate_bounded_ascii_text("audience", &document.audience, MAX_AUDIENCE_BYTES)?;
    validate_identifier("project_id", &document.project_id.0)?;
    validate_identifier("workflow_id", &document.workflow_id.0)?;
    if document.audience
        != workflow_broker_expected_audience(&document.project_id, &document.workflow_id)
    {
        return Err(WorkflowBrokerControlError::BindingMismatch("audience"));
    }
    if document.registry_generation == 0 {
        return Err(WorkflowBrokerControlError::InvalidField {
            field: "registry_generation",
            reason: "must be greater than zero",
        });
    }
    match (
        document.registry_generation,
        document.previous_registry_digest.as_deref(),
    ) {
        (1, None) => {}
        (1, Some(_)) => return Err(WorkflowBrokerControlError::UnexpectedPredecessorDigest),
        (_, Some(digest)) => require_digest("previous_registry_digest", digest)?,
        (_, None) => return Err(WorkflowBrokerControlError::MissingPredecessorDigest),
    }
    if document.required_event_schema_version != WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION {
        return Err(WorkflowBrokerControlError::EventSchemaDowngrade);
    }
    if document.credentials.is_empty() {
        return Err(WorkflowBrokerControlError::EmptyRegistry);
    }
    Ok(())
}

fn admit_credentials(
    metadata: &[WorkflowBrokerPublicCredentialMetadata],
) -> Result<Vec<AdmittedPublicCredential>, WorkflowBrokerControlError> {
    let mut credential_ids = BTreeSet::new();
    let mut subject_ids = BTreeSet::new();
    let mut public_keys = BTreeSet::new();
    let mut admitted = Vec::with_capacity(metadata.len());
    let mut previous_id: Option<&str> = None;
    for credential in metadata {
        if previous_id.is_some_and(|previous| previous >= credential.credential_id.0.as_str()) {
            return Err(WorkflowBrokerControlError::CredentialOrderNotCanonical);
        }
        previous_id = Some(&credential.credential_id.0);
        validate_credential_shape(credential)?;
        if !credential_ids.insert(credential.credential_id.0.clone()) {
            return Err(WorkflowBrokerControlError::DuplicateCredentialId(
                credential.credential_id.0.clone(),
            ));
        }
        if !subject_ids.insert(credential.subject_id.0.clone()) {
            return Err(WorkflowBrokerControlError::DuplicateSubjectId(
                credential.subject_id.0.clone(),
            ));
        }
        let key_bytes =
            decode_lower_hex_fixed::<32>(&credential.public_key_hex).ok_or_else(|| {
                WorkflowBrokerControlError::InvalidPublicKey(credential.credential_id.0.clone())
            })?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| {
            WorkflowBrokerControlError::InvalidPublicKey(credential.credential_id.0.clone())
        })?;
        if verifying_key.is_weak() {
            return Err(WorkflowBrokerControlError::WeakPublicKey(
                credential.credential_id.0.clone(),
            ));
        }
        if !public_keys.insert(verifying_key.to_bytes()) {
            return Err(WorkflowBrokerControlError::DuplicatePublicKey);
        }
        let metadata_digest =
            workflow_broker_public_credential_digest(credential).map_err(contract_error)?;
        admitted.push(AdmittedPublicCredential {
            metadata: credential.clone(),
            verifying_key,
            public_key_fingerprint: raw_digest(&key_bytes),
            metadata_digest,
        });
    }
    Ok(admitted)
}

fn validate_credential_shape(
    credential: &WorkflowBrokerPublicCredentialMetadata,
) -> Result<(), WorkflowBrokerControlError> {
    validate_identifier("credential_id", &credential.credential_id.0)?;
    validate_identifier("broker_id", &credential.broker_id.0)?;
    validate_identifier("subject_id", &credential.subject_id.0)?;
    validate_identifier(
        "enrollment_operation_id",
        &credential.enrollment_operation_id.0,
    )?;
    if let Some(operation_id) = &credential.revocation_operation_id {
        validate_identifier("revocation_operation_id", &operation_id.0)?;
    }
    validate_host_binding(&credential.host_binding)?;
    if credential.key_generation == 0 || credential.not_before_unix == 0 {
        return Err(WorkflowBrokerControlError::InvalidField {
            field: "credential_generation_or_time",
            reason: "must be greater than zero",
        });
    }
    match (
        credential.status,
        credential.revoked_at_unix,
        credential.revocation_operation_id.as_ref(),
    ) {
        (WorkflowBrokerCredentialStatus::Active, None, None) => {}
        (WorkflowBrokerCredentialStatus::Revoked, Some(revoked_at), Some(_))
            if revoked_at >= credential.not_before_unix => {}
        _ => return Err(WorkflowBrokerControlError::InvalidCredentialStatus),
    }
    match credential.purpose {
        WorkflowBrokerCredentialPurpose::RegistryAdministrator => {
            if credential.profile != WorkflowBrokerCredentialProfile::Administrator
                || !credential.allowed_operations.is_empty()
            {
                return Err(WorkflowBrokerControlError::InvalidCredentialRole);
            }
        }
        WorkflowBrokerCredentialPurpose::EventIssuer => {
            if credential.profile == WorkflowBrokerCredentialProfile::Administrator
                || credential.allowed_operations.is_empty()
            {
                return Err(WorkflowBrokerControlError::InvalidCredentialRole);
            }
            let mut previous = None;
            for operation in &credential.allowed_operations {
                if previous.is_some_and(|prior| prior >= *operation) {
                    return Err(WorkflowBrokerControlError::OperationOrderNotCanonical);
                }
                previous = Some(*operation);
                if !profile_allows_operation(credential.profile, *operation) {
                    return Err(WorkflowBrokerControlError::ProfileOperationMismatch);
                }
            }
        }
    }
    Ok(())
}

fn validate_credential_chains(
    credentials: &[AdmittedPublicCredential],
) -> Result<(), WorkflowBrokerControlError> {
    let by_id = credentials
        .iter()
        .map(|credential| (credential.metadata.credential_id.0.as_str(), credential))
        .collect::<BTreeMap<_, _>>();
    let mut generations = BTreeSet::new();
    let mut predecessor_successors = BTreeSet::new();
    let mut maximum_generation = BTreeMap::<(String, &'static str), u64>::new();
    for credential in credentials {
        let chain = (
            credential.metadata.broker_id.0.clone(),
            purpose_label(credential.metadata.purpose),
        );
        if !generations.insert((chain.0.clone(), chain.1, credential.metadata.key_generation)) {
            return Err(WorkflowBrokerControlError::InvalidRotationChain);
        }
        maximum_generation
            .entry(chain)
            .and_modify(|generation| {
                *generation = (*generation).max(credential.metadata.key_generation);
            })
            .or_insert(credential.metadata.key_generation);
        match (
            credential.metadata.key_generation,
            credential.metadata.predecessor_credential_id.as_ref(),
        ) {
            (1, None) => {}
            (1, Some(_)) | (_, None) => {
                return Err(WorkflowBrokerControlError::InvalidRotationChain)
            }
            (generation, Some(predecessor_id)) => {
                if !predecessor_successors.insert(predecessor_id.0.clone()) {
                    return Err(WorkflowBrokerControlError::InvalidRotationChain);
                }
                let predecessor = by_id
                    .get(predecessor_id.0.as_str())
                    .ok_or(WorkflowBrokerControlError::InvalidRotationChain)?;
                if predecessor.metadata.key_generation.checked_add(1) != Some(generation)
                    || predecessor.metadata.status != WorkflowBrokerCredentialStatus::Revoked
                    || predecessor.metadata.revoked_at_unix
                        > Some(credential.metadata.not_before_unix)
                    || predecessor.metadata.broker_id != credential.metadata.broker_id
                    || predecessor.metadata.purpose != credential.metadata.purpose
                    || predecessor.metadata.profile != credential.metadata.profile
                    || predecessor.metadata.host_binding != credential.metadata.host_binding
                    || predecessor.metadata.custody != credential.metadata.custody
                    || predecessor.metadata.allowed_operations
                        != credential.metadata.allowed_operations
                    || predecessor.verifying_key == credential.verifying_key
                {
                    return Err(WorkflowBrokerControlError::InvalidRotationChain);
                }
            }
        }
    }
    for credential in credentials
        .iter()
        .filter(|credential| credential.metadata.status == WorkflowBrokerCredentialStatus::Active)
    {
        let chain = (
            credential.metadata.broker_id.0.clone(),
            purpose_label(credential.metadata.purpose),
        );
        if maximum_generation.get(&chain) != Some(&credential.metadata.key_generation) {
            return Err(WorkflowBrokerControlError::InvalidRotationChain);
        }
    }
    Ok(())
}

fn validate_active_authorities(
    credentials: &[AdmittedPublicCredential],
) -> Result<(), WorkflowBrokerControlError> {
    let active_admins = credentials
        .iter()
        .filter(|credential| {
            credential.metadata.purpose == WorkflowBrokerCredentialPurpose::RegistryAdministrator
                && credential.metadata.status == WorkflowBrokerCredentialStatus::Active
        })
        .count();
    match active_admins {
        0 => return Err(WorkflowBrokerControlError::MissingActiveAdministrator),
        1 => {}
        _ => return Err(WorkflowBrokerControlError::MultipleActiveAdministrators),
    }
    let mut active_brokers = BTreeSet::new();
    for credential in credentials.iter().filter(|credential| {
        credential.metadata.purpose == WorkflowBrokerCredentialPurpose::EventIssuer
            && credential.metadata.status == WorkflowBrokerCredentialStatus::Active
    }) {
        if !active_brokers.insert(credential.metadata.broker_id.0.clone()) {
            return Err(WorkflowBrokerControlError::MultipleActiveBrokerCredentials(
                credential.metadata.broker_id.0.clone(),
            ));
        }
    }
    Ok(())
}

fn validate_host_binding(
    binding: &WorkflowBrokerHostBinding,
) -> Result<(), WorkflowBrokerControlError> {
    validate_exact_semver("host_binding.host_version", &binding.host_version)?;
    validate_identifier("host_binding.adapter_id", &binding.adapter_id.0)?;
    validate_exact_semver("host_binding.adapter_version", &binding.adapter_version)?;
    validate_identifier(
        "host_binding.host_installation_id",
        &binding.host_installation_id.0,
    )?;
    validate_identifier("host_binding.protocol_version", &binding.protocol_version)?;
    Ok(())
}

fn validate_event_host_binding(
    provenance: &forge_core_contracts::WorkflowBrokerNativeHostProvenance,
    binding: &WorkflowBrokerHostBinding,
) -> Result<(), WorkflowBrokerControlError> {
    if provenance.host_kind != binding.host_kind
        || provenance.host_version != binding.host_version
        || provenance.adapter_id != binding.adapter_id
        || provenance.adapter_version != binding.adapter_version
    {
        return Err(WorkflowBrokerControlError::HostBindingMismatch);
    }
    Ok(())
}

fn validate_native_admin_authorization(
    envelope: &WorkflowBrokerAdminOperationEnvelope,
    binding: &WorkflowBrokerHostBinding,
) -> Result<(), WorkflowBrokerControlError> {
    let native = &envelope.native_authorization;
    if native.host_kind != binding.host_kind
        || native.host_version != binding.host_version
        || native.adapter_id != binding.adapter_id
        || native.adapter_version != binding.adapter_version
        || native.host_installation_id != binding.host_installation_id
        || native.protocol_version != binding.protocol_version
    {
        return Err(WorkflowBrokerControlError::HostBindingMismatch);
    }
    validate_opaque_handle(
        "native_authorization.admin_session_ref",
        &native.admin_session_ref,
    )?;
    validate_opaque_handle(
        "native_authorization.admin_interaction_ref",
        &native.admin_interaction_ref,
    )?;
    if native.observed_at_unix == 0
        || native.observed_at_unix > envelope.issued_at_unix
        || envelope
            .issued_at_unix
            .saturating_sub(native.observed_at_unix)
            > MAX_ADMIN_OBSERVATION_TO_ISSUANCE_SECONDS
    {
        return Err(WorkflowBrokerControlError::AdminObservationOutOfBounds);
    }
    require_digest(
        "native_authorization.descriptor_digest",
        &native.descriptor_digest,
    )?;
    let expected =
        workflow_broker_native_admin_descriptor_digest(envelope).map_err(contract_error)?;
    if native.descriptor_digest != expected {
        return Err(WorkflowBrokerControlError::AdminDescriptorMismatch);
    }
    Ok(())
}

fn validate_admin_freshness(
    envelope: &WorkflowBrokerAdminOperationEnvelope,
    now_unix: i64,
) -> Result<(), WorkflowBrokerControlError> {
    let now = u64::try_from(now_unix).map_err(|_| WorkflowBrokerControlError::InvalidClock)?;
    if envelope.issued_at_unix == 0
        || envelope.expires_at_unix <= envelope.issued_at_unix
        || envelope
            .expires_at_unix
            .saturating_sub(envelope.issued_at_unix)
            > 300
        || envelope.issued_at_unix > now.saturating_add(DEFAULT_ADMIN_MAX_FUTURE_SKEW_SECONDS)
        || envelope.expires_at_unix <= now
    {
        return Err(WorkflowBrokerControlError::AdminFreshnessOutOfBounds);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_admin_transition(
    current: &WorkflowBrokerPublicRegistryDocument,
    proposed: &WorkflowBrokerPublicRegistryDocument,
    envelope: &WorkflowBrokerAdminOperationEnvelope,
) -> Result<(), WorkflowBrokerControlError> {
    if current.audience != proposed.audience
        || current.project_id != proposed.project_id
        || current.workflow_id != proposed.workflow_id
        || current.required_event_schema_version != proposed.required_event_schema_version
    {
        return Err(WorkflowBrokerControlError::ProposedRegistryMismatch);
    }
    let current_by_id = credential_map(&current.credentials)?;
    let proposed_by_id = credential_map(&proposed.credentials)?;
    match &envelope.operation {
        WorkflowBrokerAdminOperation::Initialize { .. } => {
            return Err(WorkflowBrokerControlError::InvalidAdminTransition);
        }
        WorkflowBrokerAdminOperation::Enroll { credential_id } => {
            if current_by_id.contains_key(credential_id.0.as_str()) {
                return Err(WorkflowBrokerControlError::InvalidAdminTransition);
            }
            let enrolled = proposed_by_id
                .get(credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::InvalidAdminTransition)?;
            if enrolled.purpose != WorkflowBrokerCredentialPurpose::EventIssuer
                || enrolled.status != WorkflowBrokerCredentialStatus::Active
                || enrolled.key_generation != 1
                || enrolled.predecessor_credential_id.is_some()
                || enrolled.enrollment_operation_id != envelope.operation_id
                || proposed_by_id.len() != current_by_id.len() + 1
                || !all_other_credentials_unchanged(
                    &current_by_id,
                    &proposed_by_id,
                    &[credential_id.0.as_str()],
                )
            {
                return Err(WorkflowBrokerControlError::InvalidAdminTransition);
            }
        }
        WorkflowBrokerAdminOperation::Rotate {
            current_credential_id,
            replacement_credential_id,
        } => {
            let old = current_by_id
                .get(current_credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::InvalidAdminTransition)?;
            let revoked = proposed_by_id
                .get(current_credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::InvalidAdminTransition)?;
            let replacement = proposed_by_id
                .get(replacement_credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::InvalidAdminTransition)?;
            let mut expected_revoked = (**old).clone();
            expected_revoked.status = WorkflowBrokerCredentialStatus::Revoked;
            expected_revoked.revoked_at_unix = Some(envelope.issued_at_unix);
            expected_revoked.revocation_operation_id = Some(envelope.operation_id.clone());
            if old.status != WorkflowBrokerCredentialStatus::Active
                || current_by_id.contains_key(replacement_credential_id.0.as_str())
                || *revoked != &expected_revoked
                || replacement.purpose != old.purpose
                || replacement.status != WorkflowBrokerCredentialStatus::Active
                || replacement.broker_id != old.broker_id
                || replacement.profile != old.profile
                || replacement.host_binding != old.host_binding
                || replacement.custody != old.custody
                || replacement.allowed_operations != old.allowed_operations
                || replacement.key_generation != old.key_generation.saturating_add(1)
                || replacement.predecessor_credential_id.as_ref() != Some(current_credential_id)
                || replacement.enrollment_operation_id != envelope.operation_id
                || replacement.not_before_unix != envelope.issued_at_unix
                || replacement.public_key_hex == old.public_key_hex
                || proposed_by_id.len() != current_by_id.len() + 1
                || !all_other_credentials_unchanged(
                    &current_by_id,
                    &proposed_by_id,
                    &[
                        current_credential_id.0.as_str(),
                        replacement_credential_id.0.as_str(),
                    ],
                )
            {
                return Err(WorkflowBrokerControlError::InvalidAdminTransition);
            }
        }
        WorkflowBrokerAdminOperation::Revoke { credential_id, .. } => {
            let current_credential = current_by_id
                .get(credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::InvalidAdminTransition)?;
            let revoked = proposed_by_id
                .get(credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::InvalidAdminTransition)?;
            let mut expected_revoked = (**current_credential).clone();
            expected_revoked.status = WorkflowBrokerCredentialStatus::Revoked;
            expected_revoked.revoked_at_unix = Some(envelope.issued_at_unix);
            expected_revoked.revocation_operation_id = Some(envelope.operation_id.clone());
            if current_credential.purpose != WorkflowBrokerCredentialPurpose::EventIssuer
                || current_credential.status != WorkflowBrokerCredentialStatus::Active
                || *revoked != &expected_revoked
                || proposed_by_id.len() != current_by_id.len()
                || !all_other_credentials_unchanged(
                    &current_by_id,
                    &proposed_by_id,
                    &[credential_id.0.as_str()],
                )
            {
                return Err(WorkflowBrokerControlError::InvalidAdminTransition);
            }
        }
    }
    Ok(())
}

fn validate_applied_operation_projection(
    document: &WorkflowBrokerPublicRegistryDocument,
    envelope: &WorkflowBrokerAdminOperationEnvelope,
) -> Result<(), WorkflowBrokerControlError> {
    let credentials = credential_map(&document.credentials)?;
    match &envelope.operation {
        WorkflowBrokerAdminOperation::Initialize {
            active_admin_credential_id,
        } => {
            let credential = credentials
                .get(active_admin_credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::AdminRetryMismatch)?;
            if credential.purpose != WorkflowBrokerCredentialPurpose::RegistryAdministrator
                || credential.enrollment_operation_id != envelope.operation_id
            {
                return Err(WorkflowBrokerControlError::AdminRetryMismatch);
            }
        }
        WorkflowBrokerAdminOperation::Enroll { credential_id } => {
            let credential = credentials
                .get(credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::AdminRetryMismatch)?;
            if credential.key_generation != 1
                || credential.enrollment_operation_id != envelope.operation_id
            {
                return Err(WorkflowBrokerControlError::AdminRetryMismatch);
            }
        }
        WorkflowBrokerAdminOperation::Rotate {
            current_credential_id,
            replacement_credential_id,
        } => {
            let old = credentials
                .get(current_credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::AdminRetryMismatch)?;
            let replacement = credentials
                .get(replacement_credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::AdminRetryMismatch)?;
            if old.status != WorkflowBrokerCredentialStatus::Revoked
                || old.revocation_operation_id.as_ref() != Some(&envelope.operation_id)
                || replacement.predecessor_credential_id.as_ref() != Some(current_credential_id)
                || replacement.enrollment_operation_id != envelope.operation_id
            {
                return Err(WorkflowBrokerControlError::AdminRetryMismatch);
            }
        }
        WorkflowBrokerAdminOperation::Revoke { credential_id, .. } => {
            let credential = credentials
                .get(credential_id.0.as_str())
                .ok_or(WorkflowBrokerControlError::AdminRetryMismatch)?;
            if credential.status != WorkflowBrokerCredentialStatus::Revoked
                || credential.revocation_operation_id.as_ref() != Some(&envelope.operation_id)
            {
                return Err(WorkflowBrokerControlError::AdminRetryMismatch);
            }
        }
    }
    Ok(())
}

fn credential_map(
    credentials: &[WorkflowBrokerPublicCredentialMetadata],
) -> Result<BTreeMap<&str, &WorkflowBrokerPublicCredentialMetadata>, WorkflowBrokerControlError> {
    let mut map = BTreeMap::new();
    for credential in credentials {
        if map
            .insert(credential.credential_id.0.as_str(), credential)
            .is_some()
        {
            return Err(WorkflowBrokerControlError::DuplicateCredentialId(
                credential.credential_id.0.clone(),
            ));
        }
    }
    Ok(map)
}

fn all_other_credentials_unchanged(
    current: &BTreeMap<&str, &WorkflowBrokerPublicCredentialMetadata>,
    proposed: &BTreeMap<&str, &WorkflowBrokerPublicCredentialMetadata>,
    changed: &[&str],
) -> bool {
    current.iter().all(|(credential_id, metadata)| {
        changed.contains(credential_id)
            || proposed
                .get(credential_id)
                .is_some_and(|proposed_metadata| *proposed_metadata == *metadata)
    })
}

const fn purpose_label(purpose: WorkflowBrokerCredentialPurpose) -> &'static str {
    match purpose {
        WorkflowBrokerCredentialPurpose::EventIssuer => "event_issuer",
        WorkflowBrokerCredentialPurpose::RegistryAdministrator => "registry_administrator",
    }
}

fn event_profile(profile: WorkflowBrokerCredentialProfile) -> Option<WorkflowBrokerIssuerProfile> {
    match profile {
        WorkflowBrokerCredentialProfile::Human => Some(WorkflowBrokerIssuerProfile::Human),
        WorkflowBrokerCredentialProfile::Reviewer => Some(WorkflowBrokerIssuerProfile::Reviewer),
        WorkflowBrokerCredentialProfile::Runtime => Some(WorkflowBrokerIssuerProfile::Runtime),
        WorkflowBrokerCredentialProfile::Administrator => None,
    }
}

const fn bound_operation(kind: WorkflowBrokerEventKind) -> WorkflowBrokerBoundOperation {
    match kind {
        WorkflowBrokerEventKind::Applicability => WorkflowBrokerBoundOperation::Applicability,
        WorkflowBrokerEventKind::Capability => WorkflowBrokerBoundOperation::Capability,
        WorkflowBrokerEventKind::Decision => WorkflowBrokerBoundOperation::Decision,
        WorkflowBrokerEventKind::Evidence => WorkflowBrokerBoundOperation::Evidence,
        WorkflowBrokerEventKind::IntentRevision => WorkflowBrokerBoundOperation::IntentRevision,
        WorkflowBrokerEventKind::Signal => WorkflowBrokerBoundOperation::Signal,
        WorkflowBrokerEventKind::Waiver => WorkflowBrokerBoundOperation::Waiver,
    }
}

const fn profile_allows_operation(
    profile: WorkflowBrokerCredentialProfile,
    operation: WorkflowBrokerBoundOperation,
) -> bool {
    matches!(
        (profile, operation),
        (
            WorkflowBrokerCredentialProfile::Human,
            WorkflowBrokerBoundOperation::Applicability
                | WorkflowBrokerBoundOperation::Decision
                | WorkflowBrokerBoundOperation::Evidence
                | WorkflowBrokerBoundOperation::IntentRevision
                | WorkflowBrokerBoundOperation::Waiver
        ) | (
            WorkflowBrokerCredentialProfile::Reviewer,
            WorkflowBrokerBoundOperation::Evidence | WorkflowBrokerBoundOperation::Signal
        ) | (
            WorkflowBrokerCredentialProfile::Runtime,
            WorkflowBrokerBoundOperation::Capability
                | WorkflowBrokerBoundOperation::Evidence
                | WorkflowBrokerBoundOperation::Signal
        )
    )
}

fn validate_exact_semver(
    field: &'static str,
    value: &str,
) -> Result<(), WorkflowBrokerControlError> {
    validate_bounded_ascii_text(field, value, MAX_PROTOCOL_VERSION_BYTES)?;
    let parsed =
        semver::Version::parse(value).map_err(|_| WorkflowBrokerControlError::InvalidField {
            field,
            reason: "must be an exact SemVer version",
        })?;
    if parsed.to_string() != value {
        return Err(WorkflowBrokerControlError::InvalidField {
            field,
            reason: "must use canonical exact SemVer spelling",
        });
    }
    Ok(())
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), WorkflowBrokerControlError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value.is_ascii()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':'))
    {
        return Err(WorkflowBrokerControlError::InvalidField {
            field,
            reason: "must be a bounded opaque ASCII identifier",
        });
    }
    Ok(())
}

fn validate_bounded_ascii_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), WorkflowBrokerControlError> {
    if value.trim().is_empty()
        || value.len() > max_bytes
        || !value.is_ascii()
        || value.chars().any(char::is_control)
    {
        return Err(WorkflowBrokerControlError::InvalidField {
            field,
            reason: "must be non-blank, bounded, control-free ASCII",
        });
    }
    Ok(())
}

fn validate_opaque_handle(
    field: &'static str,
    value: &str,
) -> Result<(), WorkflowBrokerControlError> {
    if !(MIN_OPAQUE_HANDLE_BYTES..=MAX_OPAQUE_HANDLE_BYTES).contains(&value.len())
        || !value.is_ascii()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':'))
        || value.contains("://")
    {
        return Err(WorkflowBrokerControlError::InvalidField {
            field,
            reason: "must be a bounded content-free opaque handle",
        });
    }
    Ok(())
}

fn validate_nonce(value: &str) -> Result<(), WorkflowBrokerControlError> {
    validate_opaque_handle("nonce", value)
}

fn require_digest(field: &'static str, value: &str) -> Result<(), WorkflowBrokerControlError> {
    let valid = value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    });
    if valid {
        Ok(())
    } else {
        Err(WorkflowBrokerControlError::InvalidField {
            field,
            reason: "must be a lowercase sha256 digest",
        })
    }
}

fn decode_lower_hex_fixed<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return None;
    }
    let mut bytes = [0_u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

fn raw_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[allow(clippy::needless_pass_by_value)]
fn contract_error(
    error: forge_core_contracts::WorkflowBrokerContractError,
) -> WorkflowBrokerControlError {
    WorkflowBrokerControlError::Contract(error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowBrokerControlError {
    UnsupportedRegistrySchema(String),
    UnsupportedCredentialSchema(String),
    UnsupportedAdminSchema(String),
    BindingMismatch(&'static str),
    EventSchemaDowngrade,
    EmptyRegistry,
    MissingEventCredential,
    MissingPredecessorDigest,
    UnexpectedPredecessorDigest,
    CredentialOrderNotCanonical,
    OperationOrderNotCanonical,
    DuplicateCredentialId(String),
    DuplicateSubjectId(String),
    DuplicatePublicKey,
    InvalidPublicKey(String),
    WeakPublicKey(String),
    InvalidCredentialRole,
    InvalidCredentialStatus,
    InvalidRotationChain,
    ProfileOperationMismatch,
    MissingActiveAdministrator,
    MultipleActiveAdministrators,
    MultipleActiveBrokerCredentials(String),
    UnknownCredential,
    WrongCredentialPurpose,
    CredentialRevoked,
    CredentialNotYetValid,
    CredentialGenerationMismatch,
    OperationMismatch,
    OperationNotAuthorized,
    HostBindingMismatch,
    MissingNativeProvenance,
    RegistryCasMismatch,
    RegistryGenerationOverflow,
    ProposedRegistryMismatch,
    InvalidAdminTransition,
    InvalidGenesisTrustAnchor,
    GenesisTrustAnchorMismatch,
    AdminObservationOutOfBounds,
    AdminDescriptorMismatch,
    AdminFreshnessOutOfBounds,
    AdminRetryMismatch,
    InvalidClock,
    InvalidSignatureEncoding,
    InvalidSignature,
    InvalidField {
        field: &'static str,
        reason: &'static str,
    },
    EventAuthority(WorkflowBrokerError),
    Contract(String),
}

impl fmt::Display for WorkflowBrokerControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedRegistrySchema(version) => {
                write!(formatter, "unsupported strict broker registry schema '{version}'")
            }
            Self::UnsupportedCredentialSchema(version) => {
                write!(formatter, "unsupported broker public credential schema '{version}'")
            }
            Self::UnsupportedAdminSchema(version) => {
                write!(formatter, "unsupported broker admin operation schema '{version}'")
            }
            Self::BindingMismatch(field) => write!(formatter, "broker {field} binding mismatch"),
            Self::EventSchemaDowngrade => formatter.write_str("broker event schema downgrade refused"),
            Self::EmptyRegistry => formatter.write_str("strict broker registry is empty"),
            Self::MissingEventCredential => formatter.write_str("broker registry has no event credential"),
            Self::MissingPredecessorDigest => formatter.write_str("registry predecessor digest missing"),
            Self::UnexpectedPredecessorDigest => formatter.write_str("genesis registry has a predecessor digest"),
            Self::CredentialOrderNotCanonical => formatter.write_str("broker credentials are not in canonical id order"),
            Self::OperationOrderNotCanonical => formatter.write_str("broker operations are not in canonical order"),
            Self::DuplicateCredentialId(id) => write!(formatter, "duplicate broker credential '{id}'"),
            Self::DuplicateSubjectId(id) => write!(formatter, "duplicate broker subject '{id}'"),
            Self::DuplicatePublicKey => formatter.write_str("broker public keys must be unique"),
            Self::InvalidPublicKey(id) => write!(formatter, "invalid Ed25519 public key for '{id}'"),
            Self::WeakPublicKey(id) => write!(formatter, "weak Ed25519 public key for '{id}'"),
            Self::InvalidCredentialRole => formatter.write_str("invalid broker credential role metadata"),
            Self::InvalidCredentialStatus => formatter.write_str("invalid broker credential status metadata"),
            Self::InvalidRotationChain => formatter.write_str("invalid broker credential rotation chain"),
            Self::ProfileOperationMismatch => formatter.write_str("broker profile cannot assert an allowed operation"),
            Self::MissingActiveAdministrator => formatter.write_str("broker registry has no active administrator"),
            Self::MultipleActiveAdministrators => formatter.write_str("broker registry has multiple active administrators"),
            Self::MultipleActiveBrokerCredentials(id) => write!(formatter, "broker '{id}' has multiple active credentials"),
            Self::UnknownCredential => formatter.write_str("unknown broker credential"),
            Self::WrongCredentialPurpose => formatter.write_str("credential purpose cannot authorize this operation"),
            Self::CredentialRevoked => formatter.write_str("broker credential is revoked"),
            Self::CredentialNotYetValid => formatter.write_str("broker credential was not valid at event issuance"),
            Self::CredentialGenerationMismatch => formatter.write_str("broker credential generation mismatch"),
            Self::OperationMismatch => formatter.write_str("broker operation binding mismatch"),
            Self::OperationNotAuthorized => formatter.write_str("broker credential is not authorized for operation"),
            Self::HostBindingMismatch => formatter.write_str("broker host or adapter binding mismatch"),
            Self::MissingNativeProvenance => formatter.write_str("broker native provenance missing"),
            Self::RegistryCasMismatch => formatter.write_str("broker registry compare-and-swap mismatch"),
            Self::RegistryGenerationOverflow => formatter.write_str("broker registry generation overflow"),
            Self::ProposedRegistryMismatch => formatter.write_str("proposed broker registry binding mismatch"),
            Self::InvalidAdminTransition => formatter.write_str("invalid broker administration transition"),
            Self::InvalidGenesisTrustAnchor => {
                formatter.write_str("external broker genesis trust anchor is invalid")
            }
            Self::GenesisTrustAnchorMismatch => formatter.write_str(
                "proposed broker administrator does not match the preconfigured external trust anchor",
            ),
            Self::AdminObservationOutOfBounds => formatter.write_str("native admin observation time is out of bounds"),
            Self::AdminDescriptorMismatch => formatter.write_str("native admin descriptor digest mismatch"),
            Self::AdminFreshnessOutOfBounds => formatter.write_str("broker admin operation freshness is out of bounds"),
            Self::AdminRetryMismatch => formatter.write_str("broker admin retry does not match durable state"),
            Self::InvalidClock => formatter.write_str("broker verification clock is before Unix epoch"),
            Self::InvalidSignatureEncoding => formatter.write_str("invalid broker admin signature encoding"),
            Self::InvalidSignature => formatter.write_str("broker admin signature verification failed"),
            Self::InvalidField { field, reason } => write!(formatter, "invalid {field}: {reason}"),
            Self::EventAuthority(error) => write!(formatter, "broker event authority rejected input: {error}"),
            Self::Contract(error) => write!(formatter, "broker contract rejected input: {error}"),
        }
    }
}

impl std::error::Error for WorkflowBrokerControlError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        workflow_broker_event_signing_bytes, workflow_broker_host_event_descriptor_digest,
        WorkflowBrokerSemanticInput, WORKFLOW_BROKER_EVENT_SCHEMA_VERSION,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use forge_core_contracts::{
        PrincipalId, RuntimeKind, WorkflowBrokerHostInteractionKind,
        WorkflowBrokerNativeAdminAuthorization, WorkflowBrokerNativeHostProvenance,
        WorkflowBrokerPublicKeyAlgorithm,
    };

    const NOW: i64 = 1_900_000_000;

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut value = String::new();
        for byte in bytes {
            let _ = write!(value, "{byte:02x}");
        }
        value
    }

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn event_key() -> SigningKey {
        SigningKey::from_bytes(&[7_u8; 32])
    }

    fn replacement_key() -> SigningKey {
        SigningKey::from_bytes(&[8_u8; 32])
    }

    fn admin_key() -> SigningKey {
        SigningKey::from_bytes(&[9_u8; 32])
    }

    struct TestGenesisTrustAnchor {
        anchor_id: StableId,
        operator_subject_id: StableId,
        public_key_hex: String,
        host_binding: WorkflowBrokerHostBinding,
    }

    impl WorkflowBrokerGenesisTrustAnchor for TestGenesisTrustAnchor {
        fn anchor_id(&self) -> &StableId {
            &self.anchor_id
        }

        fn operator_subject_id(&self) -> &StableId {
            &self.operator_subject_id
        }

        fn public_key_hex(&self) -> &str {
            &self.public_key_hex
        }

        fn host_binding(&self) -> &WorkflowBrokerHostBinding {
            &self.host_binding
        }
    }

    fn genesis_trust_anchor(key: &SigningKey) -> TestGenesisTrustAnchor {
        TestGenesisTrustAnchor {
            anchor_id: StableId("operator.anchor.preconfigured".to_owned()),
            operator_subject_id: StableId("administrator.operator.alpha".to_owned()),
            public_key_hex: hex(key.verifying_key().as_bytes()),
            host_binding: host_binding(),
        }
    }

    fn host_binding() -> WorkflowBrokerHostBinding {
        WorkflowBrokerHostBinding {
            host_kind: RuntimeKind::ForgeStandalone,
            host_version: "1.2.3".to_owned(),
            adapter_id: StableId("adapter.host".to_owned()),
            adapter_version: "2.3.4".to_owned(),
            host_installation_id: StableId("host.installation.alpha".to_owned()),
            protocol_version: "workflow-host-origin-v1".to_owned(),
        }
    }

    fn admin_credential(key: &SigningKey) -> WorkflowBrokerPublicCredentialMetadata {
        WorkflowBrokerPublicCredentialMetadata {
            credential_id: StableId("credential.admin.1".to_owned()),
            broker_id: StableId("broker.admin.alpha".to_owned()),
            subject_id: StableId("administrator.operator.alpha".to_owned()),
            purpose: WorkflowBrokerCredentialPurpose::RegistryAdministrator,
            profile: WorkflowBrokerCredentialProfile::Administrator,
            algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
            public_key_hex: hex(key.verifying_key().as_bytes()),
            key_generation: 1,
            status: WorkflowBrokerCredentialStatus::Active,
            custody: forge_core_contracts::WorkflowBrokerCustodyKind::OsKeystoreNonExportable,
            host_binding: host_binding(),
            allowed_operations: vec![],
            not_before_unix: NOW as u64 - 600,
            revoked_at_unix: None,
            predecessor_credential_id: None,
            enrollment_operation_id: StableId("admin.operation.genesis".to_owned()),
            revocation_operation_id: None,
        }
    }

    fn event_credential(
        credential_id: &str,
        subject_id: &str,
        generation: u64,
        key: &SigningKey,
    ) -> WorkflowBrokerPublicCredentialMetadata {
        WorkflowBrokerPublicCredentialMetadata {
            credential_id: StableId(credential_id.to_owned()),
            broker_id: StableId("broker.event.alpha".to_owned()),
            subject_id: StableId(subject_id.to_owned()),
            purpose: WorkflowBrokerCredentialPurpose::EventIssuer,
            profile: WorkflowBrokerCredentialProfile::Human,
            algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
            public_key_hex: hex(key.verifying_key().as_bytes()),
            key_generation: generation,
            status: WorkflowBrokerCredentialStatus::Active,
            custody: forge_core_contracts::WorkflowBrokerCustodyKind::OsKeystoreNonExportable,
            host_binding: host_binding(),
            allowed_operations: vec![WorkflowBrokerBoundOperation::Decision],
            not_before_unix: if generation == 1 {
                NOW as u64 - 600
            } else {
                NOW as u64 - 5
            },
            revoked_at_unix: None,
            predecessor_credential_id: None,
            enrollment_operation_id: StableId(if generation == 1 {
                "admin.operation.genesis".to_owned()
            } else {
                "admin.operation.rotate.2".to_owned()
            }),
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
            credentials: vec![
                admin_credential(&admin_key()),
                event_credential("credential.event.1", "issuer.human.1", 1, &event_key()),
            ],
        }
    }

    fn context(operation: WorkflowBrokerBoundOperation) -> WorkflowBrokerVerificationContext {
        WorkflowBrokerVerificationContext {
            audience: "forge-core:workflow:project.alpha".to_owned(),
            project_id: StableId("project.alpha".to_owned()),
            workflow_id: StableId("workflow.governance".to_owned()),
            operation,
        }
    }

    fn event_envelope(issuer_id: &str, key: &SigningKey) -> WorkflowBrokerEventEnvelope {
        let mut envelope = WorkflowBrokerEventEnvelope {
            schema_version: WORKFLOW_BROKER_EVENT_SCHEMA_VERSION.to_owned(),
            audience: "forge-core:workflow:project.alpha".to_owned(),
            issuer_id: StableId(issuer_id.to_owned()),
            issuer_profile: WorkflowBrokerIssuerProfile::Human,
            origin_principal_id: PrincipalId("principal.human.owner".to_owned()),
            separation_domain: StableId("human.owner.session".to_owned()),
            event_kind: WorkflowBrokerEventKind::Decision,
            project_id: StableId("project.alpha".to_owned()),
            action_packet_digest: digest('a'),
            semantic_input: WorkflowBrokerSemanticInput::Decision {
                selected_alternative_ref: StableId("alternative.safe".to_owned()),
            },
            native_host_provenance: Some(WorkflowBrokerNativeHostProvenance {
                host_kind: RuntimeKind::ForgeStandalone,
                host_version: "1.2.3".to_owned(),
                adapter_id: StableId("adapter.host".to_owned()),
                adapter_version: "2.3.4".to_owned(),
                interaction_kind: WorkflowBrokerHostInteractionKind::NativeHumanConfirmation,
                host_event_ref: "event-reference-000001".to_owned(),
                host_session_ref: "session-reference-0001".to_owned(),
                host_interaction_ref: "interaction-reference-1".to_owned(),
                host_event_descriptor_digest: digest('0'),
                host_observed_at_unix: NOW as u64 - 5,
            }),
            issued_at_unix: NOW as u64 - 5,
            expires_at_unix: NOW as u64 + 120,
            nonce: "event-operation-nonce-0001".to_owned(),
            signature: String::new(),
        };
        let provenance = envelope
            .native_host_provenance
            .as_mut()
            .expect("provenance");
        provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
            provenance,
            &envelope.project_id,
            &envelope.action_packet_digest,
            &envelope.semantic_input,
        )
        .expect("descriptor");
        envelope.signature = hex(&key
            .sign(&workflow_broker_event_signing_bytes(&envelope).expect("bytes"))
            .to_bytes());
        envelope
    }

    fn rotated_registry(
        current: &AuthorizedWorkflowBrokerControlPlane,
    ) -> (
        WorkflowBrokerPublicRegistryDocument,
        WorkflowBrokerAdminOperationEnvelope,
    ) {
        let operation_id = StableId("admin.operation.rotate.2".to_owned());
        let mut proposed = current.document().clone();
        proposed.registry_generation = 2;
        proposed.previous_registry_digest = Some(current.registry_digest().to_owned());
        let old = proposed
            .credentials
            .iter_mut()
            .find(|credential| credential.credential_id.0 == "credential.event.1")
            .expect("old credential");
        old.status = WorkflowBrokerCredentialStatus::Revoked;
        old.revoked_at_unix = Some(NOW as u64 - 5);
        old.revocation_operation_id = Some(operation_id.clone());
        let mut replacement = event_credential(
            "credential.event.2",
            "issuer.human.2",
            2,
            &replacement_key(),
        );
        replacement.predecessor_credential_id = Some(StableId("credential.event.1".to_owned()));
        proposed.credentials.push(replacement);
        proposed
            .credentials
            .sort_by(|left, right| left.credential_id.0.cmp(&right.credential_id.0));
        let proposed_digest =
            workflow_broker_public_registry_digest(&proposed).expect("proposed digest");
        let mut envelope = WorkflowBrokerAdminOperationEnvelope {
            schema_version: WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION.to_owned(),
            audience: proposed.audience.clone(),
            project_id: proposed.project_id.clone(),
            workflow_id: proposed.workflow_id.clone(),
            operation_id,
            admin_credential_id: StableId("credential.admin.1".to_owned()),
            admin_credential_generation: 1,
            expected_registry_generation: 1,
            expected_registry_digest: Some(current.registry_digest().to_owned()),
            proposed_registry_generation: 2,
            proposed_registry_digest: proposed_digest,
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
                admin_session_ref: "admin-session-reference-1".to_owned(),
                admin_interaction_ref: "admin-interaction-ref-1".to_owned(),
                observed_at_unix: NOW as u64 - 5,
                descriptor_digest: digest('0'),
            },
            issued_at_unix: NOW as u64 - 5,
            expires_at_unix: NOW as u64 + 120,
            nonce: "admin-operation-nonce-0001".to_owned(),
            signature: String::new(),
        };
        envelope.native_authorization.descriptor_digest =
            workflow_broker_native_admin_descriptor_digest(&envelope).expect("admin descriptor");
        envelope.signature = hex(&admin_key()
            .sign(&workflow_broker_admin_operation_signing_bytes(&envelope).expect("admin bytes"))
            .to_bytes());
        (proposed, envelope)
    }

    #[test]
    fn registry_requires_semantic_ed25519_keys_and_exact_project_workflow_binding() {
        let document = registry();
        let admitted = AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            document.clone(),
            &document.audience,
            &document.project_id,
            &document.workflow_id,
        )
        .expect("admitted registry");
        assert!(admitted.registry_digest().starts_with("sha256:"));
        let credential_document = WorkflowBrokerPublicCredentialMetadataDocument {
            schema_version: WORKFLOW_BROKER_PUBLIC_CREDENTIAL_SCHEMA_VERSION.to_owned(),
            credential: document.credentials[1].clone(),
        };
        assert_eq!(
            validate_workflow_broker_public_credential_document(&credential_document)
                .expect("standalone credential"),
            workflow_broker_public_credential_digest(&credential_document.credential)
                .expect("credential digest")
        );
        let mut wrong_version = credential_document;
        wrong_version.schema_version = "0.0".to_owned();
        assert!(matches!(
            validate_workflow_broker_public_credential_document(&wrong_version),
            Err(WorkflowBrokerControlError::UnsupportedCredentialSchema(_))
        ));

        assert_eq!(
            AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
                document.clone(),
                &document.audience,
                &document.project_id,
                &StableId("workflow.other".to_owned()),
            )
            .expect_err("workflow confusion"),
            WorkflowBrokerControlError::BindingMismatch("workflow_id")
        );

        let mut weak = document;
        weak.credentials[1].public_key_hex = "00".repeat(32);
        assert!(matches!(
            AuthorizedWorkflowBrokerControlPlane::from_document(weak),
            Err(WorkflowBrokerControlError::InvalidPublicKey(_)
                | WorkflowBrokerControlError::WeakPublicKey(_))
        ));
    }

    #[test]
    fn bound_event_enforces_exact_operation_and_selected_host_metadata() {
        let control =
            AuthorizedWorkflowBrokerControlPlane::from_document(registry()).expect("control plane");
        let verified = control
            .verify_bound_event(
                event_envelope("issuer.human.1", &event_key()),
                &context(WorkflowBrokerBoundOperation::Decision),
                NOW,
                WorkflowBrokerFreshnessPolicy::default(),
            )
            .expect("bound event");
        assert_eq!(verified.audit().credential_generation, 1);
        assert_eq!(
            verified.audit().operation,
            WorkflowBrokerBoundOperation::Decision
        );

        assert_eq!(
            control
                .verify_bound_event(
                    event_envelope("issuer.human.1", &event_key()),
                    &context(WorkflowBrokerBoundOperation::Evidence),
                    NOW,
                    WorkflowBrokerFreshnessPolicy::default(),
                )
                .expect_err("operation confusion"),
            WorkflowBrokerControlError::OperationMismatch
        );

        let mut host_confusion = event_envelope("issuer.human.1", &event_key());
        host_confusion
            .native_host_provenance
            .as_mut()
            .expect("provenance")
            .host_version = "1.2.4".to_owned();
        let provenance = host_confusion
            .native_host_provenance
            .as_mut()
            .expect("provenance");
        provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
            provenance,
            &host_confusion.project_id,
            &host_confusion.action_packet_digest,
            &host_confusion.semantic_input,
        )
        .expect("descriptor");
        host_confusion.signature = hex(&event_key()
            .sign(
                &workflow_broker_event_signing_bytes(&host_confusion).expect("host-confused bytes"),
            )
            .to_bytes());
        assert_eq!(
            control
                .verify_bound_event(
                    host_confusion,
                    &context(WorkflowBrokerBoundOperation::Decision),
                    NOW,
                    WorkflowBrokerFreshnessPolicy::default(),
                )
                .expect_err("host confusion"),
            WorkflowBrokerControlError::HostBindingMismatch
        );
    }

    #[test]
    fn genesis_requires_preconfigured_external_anchor_and_binds_exact_registry() {
        let proposed = registry();
        let proposed_digest =
            workflow_broker_public_registry_digest(&proposed).expect("genesis digest");
        let mut envelope = WorkflowBrokerAdminOperationEnvelope {
            schema_version: WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION.to_owned(),
            audience: proposed.audience.clone(),
            project_id: proposed.project_id.clone(),
            workflow_id: proposed.workflow_id.clone(),
            operation_id: StableId("admin.operation.genesis".to_owned()),
            admin_credential_id: StableId("credential.admin.1".to_owned()),
            admin_credential_generation: 1,
            expected_registry_generation: 0,
            expected_registry_digest: None,
            proposed_registry_generation: 1,
            proposed_registry_digest: proposed_digest,
            operation: WorkflowBrokerAdminOperation::Initialize {
                active_admin_credential_id: StableId("credential.admin.1".to_owned()),
            },
            native_authorization: WorkflowBrokerNativeAdminAuthorization {
                host_kind: RuntimeKind::ForgeStandalone,
                host_version: "1.2.3".to_owned(),
                adapter_id: StableId("adapter.host".to_owned()),
                adapter_version: "2.3.4".to_owned(),
                host_installation_id: StableId("host.installation.alpha".to_owned()),
                protocol_version: "workflow-host-origin-v1".to_owned(),
                admin_session_ref: "admin-session-reference-1".to_owned(),
                admin_interaction_ref: "admin-interaction-ref-1".to_owned(),
                observed_at_unix: NOW as u64 - 5,
                descriptor_digest: digest('0'),
            },
            issued_at_unix: NOW as u64 - 5,
            expires_at_unix: NOW as u64 + 120,
            nonce: "admin-genesis-nonce-0001".to_owned(),
            signature: String::new(),
        };
        envelope.native_authorization.descriptor_digest =
            workflow_broker_native_admin_descriptor_digest(&envelope).expect("descriptor");
        envelope.signature = hex(&admin_key()
            .sign(&workflow_broker_admin_operation_signing_bytes(&envelope).expect("signing bytes"))
            .to_bytes());
        let trust_anchor = genesis_trust_anchor(&admin_key());
        let advance = AuthorizedWorkflowBrokerControlPlane::authorize_genesis(
            &trust_anchor,
            envelope.clone(),
            proposed.clone(),
            NOW,
        )
        .expect("externally anchored genesis");
        assert_eq!(
            advance.proposed().registry_digest(),
            envelope.proposed_registry_digest.as_str()
        );
        assert_eq!(advance.receipt().receipt.expected_registry_digest, None);

        let untrusted_self_anchor = genesis_trust_anchor(&replacement_key());
        assert_eq!(
            AuthorizedWorkflowBrokerControlPlane::authorize_genesis(
                &untrusted_self_anchor,
                envelope.clone(),
                proposed.clone(),
                NOW,
            )
            .expect_err("a registry key cannot establish its own genesis trust"),
            WorkflowBrokerControlError::GenesisTrustAnchorMismatch
        );

        let mut confused = envelope;
        confused.proposed_registry_digest = digest('f');
        confused.native_authorization.descriptor_digest =
            workflow_broker_native_admin_descriptor_digest(&confused).expect("descriptor");
        confused.signature = hex(&admin_key()
            .sign(&workflow_broker_admin_operation_signing_bytes(&confused).expect("signing bytes"))
            .to_bytes());
        assert_eq!(
            AuthorizedWorkflowBrokerControlPlane::authorize_genesis(
                &trust_anchor,
                confused,
                proposed,
                NOW,
            )
            .expect_err("wrong proposed digest"),
            WorkflowBrokerControlError::ProposedRegistryMismatch
        );
    }

    #[test]
    fn signed_admin_rotation_is_cas_bound_monotonic_and_retry_recoverable() {
        let current = AuthorizedWorkflowBrokerControlPlane::from_document(registry())
            .expect("current registry");
        let (proposed, envelope) = rotated_registry(&current);
        let advance = current
            .authorize_admin_transition(envelope.clone(), proposed, NOW, None)
            .expect("authorized rotation");
        advance
            .receipt()
            .validate_self_digest()
            .expect("receipt digest");
        assert_eq!(advance.proposed().document().registry_generation, 2);
        assert_eq!(
            advance.proposed().document().credentials[1].status,
            WorkflowBrokerCredentialStatus::Revoked
        );
        advance
            .proposed()
            .verify_applied_admin_retry(&envelope, advance.receipt())
            .expect("exact response-loss retry");

        let mut stale = envelope;
        stale.expected_registry_digest = Some(digest('f'));
        stale.native_authorization.descriptor_digest =
            workflow_broker_native_admin_descriptor_digest(&stale).expect("descriptor");
        stale.signature = hex(&admin_key()
            .sign(&workflow_broker_admin_operation_signing_bytes(&stale).expect("stale bytes"))
            .to_bytes());
        let proposed = advance.proposed().document().clone();
        assert_eq!(
            current
                .authorize_admin_transition(stale, proposed, NOW, None)
                .expect_err("stale CAS"),
            WorkflowBrokerControlError::RegistryCasMismatch
        );
    }

    #[test]
    fn event_key_cannot_authorize_admin_and_admin_host_tamper_fails_before_transition() {
        let current = AuthorizedWorkflowBrokerControlPlane::from_document(registry())
            .expect("current registry");
        let (proposed, mut envelope) = rotated_registry(&current);
        envelope.admin_credential_id = StableId("credential.event.1".to_owned());
        envelope.admin_credential_generation = 1;
        envelope.native_authorization.descriptor_digest =
            workflow_broker_native_admin_descriptor_digest(&envelope).expect("descriptor");
        envelope.signature = hex(&event_key()
            .sign(
                &workflow_broker_admin_operation_signing_bytes(&envelope)
                    .expect("event-key admin bytes"),
            )
            .to_bytes());
        assert_eq!(
            current
                .authorize_admin_transition(envelope, proposed, NOW, None)
                .expect_err("event key is not admin authority"),
            WorkflowBrokerControlError::WrongCredentialPurpose
        );

        let (proposed, mut host_tamper) = rotated_registry(&current);
        host_tamper.native_authorization.host_installation_id =
            StableId("host.installation.other".to_owned());
        host_tamper.native_authorization.descriptor_digest =
            workflow_broker_native_admin_descriptor_digest(&host_tamper)
                .expect("tampered descriptor");
        host_tamper.signature = hex(&admin_key()
            .sign(
                &workflow_broker_admin_operation_signing_bytes(&host_tamper)
                    .expect("tampered bytes"),
            )
            .to_bytes());
        assert_eq!(
            current
                .authorize_admin_transition(host_tamper, proposed, NOW, None)
                .expect_err("host installation confusion"),
            WorkflowBrokerControlError::HostBindingMismatch
        );
    }

    #[test]
    fn native_replay_identity_survives_rotation_and_revoked_key_cannot_admit_new_events() {
        let current = AuthorizedWorkflowBrokerControlPlane::from_document(registry())
            .expect("current registry");
        let before = current
            .verify_bound_event(
                event_envelope("issuer.human.1", &event_key()),
                &context(WorkflowBrokerBoundOperation::Decision),
                NOW,
                WorkflowBrokerFreshnessPolicy::default(),
            )
            .expect("pre-rotation event");
        let (proposed, envelope) = rotated_registry(&current);
        let advance = current
            .authorize_admin_transition(envelope, proposed, NOW, None)
            .expect("rotation");
        let after = advance
            .proposed()
            .verify_bound_event(
                event_envelope("issuer.human.2", &replacement_key()),
                &context(WorkflowBrokerBoundOperation::Decision),
                NOW,
                WorkflowBrokerFreshnessPolicy::default(),
            )
            .expect("post-rotation event");
        assert_eq!(
            before.audit().native_interaction_replay_digest,
            after.audit().native_interaction_replay_digest
        );
        assert!(matches!(
            advance.proposed().verify_bound_event(
                event_envelope("issuer.human.1", &event_key()),
                &context(WorkflowBrokerBoundOperation::Decision),
                NOW,
                WorkflowBrokerFreshnessPolicy::default(),
            ),
            Err(WorkflowBrokerControlError::EventAuthority(
                WorkflowBrokerError::IssuerRevoked(_)
            ))
        ));
    }

    #[test]
    fn registry_rejects_downgrade_duplicate_active_and_noncanonical_metadata() {
        let mut wrong_audience = registry();
        wrong_audience.audience = "forge-core:workflow:project.other".to_owned();
        assert_eq!(
            AuthorizedWorkflowBrokerControlPlane::from_document(wrong_audience)
                .expect_err("project audience confusion"),
            WorkflowBrokerControlError::BindingMismatch("audience")
        );

        let mut downgrade = registry();
        downgrade.required_event_schema_version = "0.1".to_owned();
        assert_eq!(
            AuthorizedWorkflowBrokerControlPlane::from_document(downgrade).expect_err("downgrade"),
            WorkflowBrokerControlError::EventSchemaDowngrade
        );

        let mut duplicate_active = registry();
        let mut second = event_credential(
            "credential.event.2",
            "issuer.human.2",
            1,
            &replacement_key(),
        );
        second.enrollment_operation_id = StableId("admin.operation.enroll.2".to_owned());
        duplicate_active.credentials.push(second);
        duplicate_active
            .credentials
            .sort_by(|left, right| left.credential_id.0.cmp(&right.credential_id.0));
        assert_eq!(
            AuthorizedWorkflowBrokerControlPlane::from_document(duplicate_active)
                .expect_err("duplicate active generation"),
            WorkflowBrokerControlError::InvalidRotationChain
        );

        let mut noncanonical = registry();
        noncanonical.credentials.reverse();
        assert_eq!(
            AuthorizedWorkflowBrokerControlPlane::from_document(noncanonical)
                .expect_err("credential order"),
            WorkflowBrokerControlError::CredentialOrderNotCanonical
        );
    }
}
