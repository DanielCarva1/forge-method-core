//! Verification boundary for origin-bound external workflow brokers.
//!
//! Forge admits only broker public keys. A host broker signs a minimal closed
//! answer after observing a real inbound human, reviewer, or runtime event.
//! The kernel later combines the verified answer with its current action packet
//! to derive every authority-sensitive request field. This module has no
//! private-key, filesystem, standalone-attestation, or replay-store behavior.

use std::collections::BTreeSet;
use std::fmt;

use ed25519_dalek::{Signature, VerifyingKey};
use forge_core_contracts::{
    PrincipalId, StableId, WorkflowBrokerHostInteractionKind, WorkflowBrokerNativeHostProvenance,
    WorkflowEvidenceOutcome, WorkflowEvidenceSubjectKind,
    MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES, MAX_WORKFLOW_INTENT_ITEM_BYTES,
    MAX_WORKFLOW_INTENT_LIST_ITEMS, MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES,
    MAX_WORKFLOW_INTENT_TOTAL_BYTES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::DEFAULT_MAX_FUTURE_SKEW_SECONDS;

pub const WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BROKER_LEGACY_EVENT_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BROKER_EVENT_SCHEMA_VERSION: &str = "0.2";
pub const WORKFLOW_BROKER_SIGNATURE_DOMAIN: &[u8] = b"forge-method:workflow-origin-broker:v1\0";
pub const WORKFLOW_BROKER_SIGNATURE_DOMAIN_V2: &[u8] = b"forge-method:workflow-origin-broker:v2\0";
pub const WORKFLOW_BROKER_HOST_DESCRIPTOR_DOMAIN: &[u8] =
    b"forge-method:workflow-host-event-descriptor:v1\0";
pub const WORKFLOW_BROKER_SEMANTIC_INPUT_DOMAIN: &[u8] =
    b"forge-method:workflow-broker-semantic-input:v1\0";
const MAX_WORKFLOW_BROKER_VERSION_BYTES: usize = 128;
const MAX_WORKFLOW_BROKER_OPAQUE_REF_BYTES: usize = 256;
const MIN_WORKFLOW_BROKER_OPAQUE_REF_BYTES: usize = 16;
const MAX_HOST_OBSERVATION_TO_ISSUANCE_SECONDS: u64 = 300;

/// Operator declaration for externally performed enrollment.
///
/// This binds an audit reference; it does not pretend Forge observed user
/// presence. The operator must admit the registry only after the ceremony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerEnrollmentDeclaration {
    pub ceremony_ref: String,
    pub ceremony_digest: String,
    pub declared_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerIssuerProfile {
    Human,
    Reviewer,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerIssuerStatus {
    Active,
    Revoked,
}

/// One admitted external broker. There is intentionally no private-key field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerIssuerEntry {
    pub issuer_id: StableId,
    pub profile: WorkflowBrokerIssuerProfile,
    pub public_key_hex: String,
    pub status: WorkflowBrokerIssuerStatus,
    pub enrollment: WorkflowBrokerEnrollmentDeclaration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerRegistryDocument {
    pub schema_version: String,
    pub audience: String,
    pub issuers: Vec<WorkflowBrokerIssuerEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBrokerEventKind {
    Applicability,
    Capability,
    Decision,
    Evidence,
    IntentRevision,
    Signal,
    Waiver,
}

/// Minimal answer only. Policy, bundle, phase, evaluator, scopes, digests, and
/// authoritative timestamps are deliberately absent and must be kernel-derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowBrokerSemanticInput {
    Applicability {
        applicable: bool,
        basis_refs: Vec<String>,
    },
    Capability {
        available: bool,
        probe_ref: String,
        subject_kind: WorkflowEvidenceSubjectKind,
        subject_ref: String,
    },
    Decision {
        selected_alternative_ref: StableId,
    },
    Evidence {
        outcome: WorkflowEvidenceOutcome,
        subject_kind: WorkflowEvidenceSubjectKind,
        subject_ref: String,
        scenario_ref: String,
    },
    /// Human-authored intent content only. Intent identifiers, revision
    /// numbers, assurance epochs, policy, readiness, claim status, targets, and
    /// evaluators are deliberately absent and must be derived by the kernel
    /// from current governed state.
    IntentRevision {
        desired_outcome: String,
        constraints: Vec<String>,
        preferences: Vec<String>,
        unacceptable_outcomes: Vec<String>,
        uncertainties: Vec<String>,
        /// Stable external conversation locator, never raw transcript content.
        conversation_ref: String,
        /// Digest of the source conversation as observed by the human broker.
        conversation_digest: String,
    },
    Signal {
        active: bool,
        basis_refs: Vec<String>,
    },
    Waiver {
        reason: String,
    },
}

impl WorkflowBrokerSemanticInput {
    #[must_use]
    pub const fn kind(&self) -> WorkflowBrokerEventKind {
        match self {
            Self::Applicability { .. } => WorkflowBrokerEventKind::Applicability,
            Self::Capability { .. } => WorkflowBrokerEventKind::Capability,
            Self::Decision { .. } => WorkflowBrokerEventKind::Decision,
            Self::Evidence { .. } => WorkflowBrokerEventKind::Evidence,
            Self::IntentRevision { .. } => WorkflowBrokerEventKind::IntentRevision,
            Self::Signal { .. } => WorkflowBrokerEventKind::Signal,
            Self::Waiver { .. } => WorkflowBrokerEventKind::Waiver,
        }
    }
}

/// Signature covers every field except `signature`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerEventEnvelope {
    pub schema_version: String,
    pub audience: String,
    pub issuer_id: StableId,
    pub issuer_profile: WorkflowBrokerIssuerProfile,
    /// Subject identity vouched for by the admitted broker. Forge verifies the
    /// broker statement; it does not infer physical presence from this label.
    pub origin_principal_id: PrincipalId,
    /// Independence boundary used by downstream quorum/separation policy.
    pub separation_domain: StableId,
    pub event_kind: WorkflowBrokerEventKind,
    pub project_id: StableId,
    pub action_packet_digest: String,
    pub semantic_input: WorkflowBrokerSemanticInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_host_provenance: Option<WorkflowBrokerNativeHostProvenance>,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
    pub nonce: String,
    pub signature: String,
}

/// Explicit verification policy; verification never reads the system clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowBrokerFreshnessPolicy {
    pub max_age_seconds: u64,
    pub max_future_skew_seconds: u64,
    pub max_ttl_seconds: u64,
}

impl Default for WorkflowBrokerFreshnessPolicy {
    fn default() -> Self {
        Self {
            max_age_seconds: 300,
            max_future_skew_seconds: DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            max_ttl_seconds: 300,
        }
    }
}

/// Replay identity returned to the kernel. Authority verification is pure;
/// reserve/commit must occur atomically with the ledger mutation downstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBrokerReplayKey {
    pub issuer_id: StableId,
    pub origin_principal_id: PrincipalId,
    pub separation_domain: StableId,
    pub project_id: StableId,
    pub nonce_fingerprint: String,
    pub event_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedWorkflowBrokerEventAudit {
    pub issuer_id: StableId,
    pub issuer_profile: WorkflowBrokerIssuerProfile,
    pub origin_principal_id: PrincipalId,
    pub separation_domain: StableId,
    pub event_kind: WorkflowBrokerEventKind,
    pub project_id: StableId,
    pub action_packet_digest: String,
    pub event_digest: String,
    pub public_key_fingerprint: String,
    pub signature_fingerprint: String,
    pub enrollment_ceremony_digest: String,
    pub replay_key: WorkflowBrokerReplayKey,
    pub native_host_provenance: Option<WorkflowBrokerNativeHostProvenance>,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
}

/// Non-cloneable proof consumed by the future kernel transaction.
pub struct VerifiedWorkflowBrokerEvent {
    semantic_input: WorkflowBrokerSemanticInput,
    audit: VerifiedWorkflowBrokerEventAudit,
}

/// Cryptographically verified historical broker event. This capability can
/// only reconcile an already durable `BrokerOriginApplied` receipt; it must
/// never enter a new-mutation path.
pub struct HistoricallyVerifiedWorkflowBrokerEvent {
    semantic_input: WorkflowBrokerSemanticInput,
    audit: VerifiedWorkflowBrokerEventAudit,
}

impl fmt::Debug for HistoricallyVerifiedWorkflowBrokerEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HistoricallyVerifiedWorkflowBrokerEvent")
            .field("audit", &self.audit)
            .finish_non_exhaustive()
    }
}

impl HistoricallyVerifiedWorkflowBrokerEvent {
    #[must_use]
    pub const fn audit(&self) -> &VerifiedWorkflowBrokerEventAudit {
        &self.audit
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        WorkflowBrokerSemanticInput,
        VerifiedWorkflowBrokerEventAudit,
    ) {
        (self.semantic_input, self.audit)
    }
}

impl fmt::Debug for VerifiedWorkflowBrokerEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWorkflowBrokerEvent")
            .field("audit", &self.audit)
            .finish_non_exhaustive()
    }
}

impl VerifiedWorkflowBrokerEvent {
    #[must_use]
    pub const fn semantic_input(&self) -> &WorkflowBrokerSemanticInput {
        &self.semantic_input
    }

    #[must_use]
    pub const fn audit(&self) -> &VerifiedWorkflowBrokerEventAudit {
        &self.audit
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        WorkflowBrokerSemanticInput,
        VerifiedWorkflowBrokerEventAudit,
    ) {
        (self.semantic_input, self.audit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdmittedIssuer {
    entry: WorkflowBrokerIssuerEntry,
    verifying_key: VerifyingKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedWorkflowBrokerRegistry {
    audience: String,
    issuers: Vec<AdmittedIssuer>,
}

impl AuthorizedWorkflowBrokerRegistry {
    /// Admit a registry only for the exact project-derived audience expected by
    /// the caller. This is the entrypoint for every project-bound lifecycle and
    /// apply path; a registry copied from another project must never transfer
    /// broker trust implicitly.
    ///
    /// # Errors
    /// Returns [`WorkflowBrokerError::AudienceMismatch`] when the document is
    /// not bound to `expected_audience`, in addition to the structural errors
    /// returned by [`Self::from_document`].
    pub fn from_document_for_audience(
        document: WorkflowBrokerRegistryDocument,
        expected_audience: &str,
    ) -> Result<Self, WorkflowBrokerError> {
        require_nonblank("expected_audience", expected_audience)?;
        if document.audience != expected_audience {
            return Err(WorkflowBrokerError::AudienceMismatch);
        }
        Self::from_document(document)
    }

    /// Admit an operator-provided public-key registry.
    ///
    /// # Errors
    /// Returns a typed error for unsupported, empty, duplicate, malformed, or
    /// incomplete trust declarations.
    pub fn from_document(
        document: WorkflowBrokerRegistryDocument,
    ) -> Result<Self, WorkflowBrokerError> {
        if document.schema_version != WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION {
            return Err(WorkflowBrokerError::UnsupportedRegistrySchema(
                document.schema_version,
            ));
        }
        require_nonblank("audience", &document.audience)?;
        if document.issuers.is_empty() {
            return Err(WorkflowBrokerError::EmptyRegistry);
        }
        let mut ids = BTreeSet::new();
        let mut keys = BTreeSet::new();
        let mut issuers = Vec::with_capacity(document.issuers.len());
        for entry in document.issuers {
            require_nonblank("issuer_id", &entry.issuer_id.0)?;
            require_nonblank("enrollment.ceremony_ref", &entry.enrollment.ceremony_ref)?;
            require_digest(
                "enrollment.ceremony_digest",
                &entry.enrollment.ceremony_digest,
            )?;
            if entry.enrollment.declared_at_unix == 0 {
                return Err(WorkflowBrokerError::InvalidField {
                    field: "enrollment.declared_at_unix",
                    reason: "must be greater than zero",
                });
            }
            if !ids.insert(entry.issuer_id.0.clone()) {
                return Err(WorkflowBrokerError::DuplicateIssuer(entry.issuer_id.0));
            }
            let bytes = decode_fixed::<32>(&entry.public_key_hex)
                .ok_or_else(|| WorkflowBrokerError::InvalidPublicKey(entry.issuer_id.0.clone()))?;
            let verifying_key = VerifyingKey::from_bytes(&bytes)
                .map_err(|_| WorkflowBrokerError::InvalidPublicKey(entry.issuer_id.0.clone()))?;
            if !keys.insert(verifying_key.to_bytes()) {
                return Err(WorkflowBrokerError::DuplicatePublicKey);
            }
            issuers.push(AdmittedIssuer {
                entry,
                verifying_key,
            });
        }
        Ok(Self {
            audience: document.audience,
            issuers,
        })
    }

    /// Purely verify one signed inbound event with an explicit clock.
    ///
    /// # Errors
    /// Returns a typed error for any schema, binding, semantic, profile,
    /// freshness, key, or signature failure. It never consumes replay state.
    pub fn verify_event(
        &self,
        envelope: WorkflowBrokerEventEnvelope,
        expected_project_id: &StableId,
        now_unix: i64,
        freshness: WorkflowBrokerFreshnessPolicy,
    ) -> Result<VerifiedWorkflowBrokerEvent, WorkflowBrokerError> {
        validate_event_schema_shape(&envelope, true)?;
        let issuer = self
            .issuers
            .iter()
            .find(|issuer| issuer.entry.issuer_id == envelope.issuer_id)
            .ok_or_else(|| WorkflowBrokerError::UnknownIssuer(envelope.issuer_id.0.clone()))?;
        let signature_bytes = decode_fixed::<64>(&envelope.signature)
            .ok_or(WorkflowBrokerError::InvalidSignatureEncoding)?;
        issuer
            .verifying_key
            .verify_strict(
                &workflow_broker_event_signing_bytes(&envelope)?,
                &Signature::from_bytes(&signature_bytes),
            )
            .map_err(|_| WorkflowBrokerError::InvalidSignature)?;

        if envelope.audience != self.audience {
            return Err(WorkflowBrokerError::AudienceMismatch);
        }
        if &envelope.project_id != expected_project_id {
            return Err(WorkflowBrokerError::ProjectMismatch);
        }
        require_digest("action_packet_digest", &envelope.action_packet_digest)?;
        require_nonblank("origin_principal_id", &envelope.origin_principal_id.0)?;
        require_nonblank("separation_domain", &envelope.separation_domain.0)?;
        if envelope.event_kind != envelope.semantic_input.kind() {
            return Err(WorkflowBrokerError::EventKindMismatch);
        }
        validate_nonce(&envelope.nonce)?;
        validate_freshness(&envelope, now_unix, freshness)?;
        validate_semantic_input(&envelope.semantic_input)?;
        if issuer.entry.status != WorkflowBrokerIssuerStatus::Active {
            return Err(WorkflowBrokerError::IssuerRevoked(envelope.issuer_id.0));
        }
        if issuer.entry.profile != envelope.issuer_profile {
            return Err(WorkflowBrokerError::IssuerProfileMismatch);
        }
        require_profile_kind(envelope.issuer_profile, envelope.event_kind)?;
        validate_event_native_host_provenance(&envelope)?;
        let event_digest = workflow_broker_event_digest(&envelope)?;
        let replay_key = WorkflowBrokerReplayKey {
            issuer_id: envelope.issuer_id.clone(),
            origin_principal_id: envelope.origin_principal_id.clone(),
            separation_domain: envelope.separation_domain.clone(),
            project_id: envelope.project_id.clone(),
            nonce_fingerprint: raw_digest(envelope.nonce.as_bytes()),
            event_digest: event_digest.clone(),
        };
        let audit = VerifiedWorkflowBrokerEventAudit {
            issuer_id: envelope.issuer_id,
            issuer_profile: envelope.issuer_profile,
            origin_principal_id: envelope.origin_principal_id,
            separation_domain: envelope.separation_domain,
            event_kind: envelope.event_kind,
            project_id: envelope.project_id,
            action_packet_digest: envelope.action_packet_digest,
            event_digest,
            public_key_fingerprint: raw_digest(&issuer.verifying_key.to_bytes()),
            signature_fingerprint: raw_digest(&signature_bytes),
            enrollment_ceremony_digest: issuer.entry.enrollment.ceremony_digest.clone(),
            replay_key,
            native_host_provenance: envelope.native_host_provenance,
            issued_at_unix: envelope.issued_at_unix,
            expires_at_unix: envelope.expires_at_unix,
        };
        Ok(VerifiedWorkflowBrokerEvent {
            semantic_input: envelope.semantic_input,
            audit,
        })
    }
    /// Verify an old event solely for exact durable receipt reconciliation.
    /// Freshness and active issuer status are intentionally not admission
    /// inputs here; signature, retained key ownership, audience, project,
    /// profile, semantic structure, and canonical event identity still are.
    /// The distinct return type cannot authorize a new mutation.
    ///
    /// # Errors
    /// Rejects malformed, unknown-key, wrong-project/audience/profile, or
    /// invalid-signature envelopes.
    pub fn verify_event_for_recovery(
        &self,
        envelope: WorkflowBrokerEventEnvelope,
        expected_project_id: &StableId,
    ) -> Result<HistoricallyVerifiedWorkflowBrokerEvent, WorkflowBrokerError> {
        validate_event_schema_shape(&envelope, false)?;
        let issuer = self
            .issuers
            .iter()
            .find(|issuer| issuer.entry.issuer_id == envelope.issuer_id)
            .ok_or_else(|| WorkflowBrokerError::UnknownIssuer(envelope.issuer_id.0.clone()))?;
        let signature_bytes = decode_fixed::<64>(&envelope.signature)
            .ok_or(WorkflowBrokerError::InvalidSignatureEncoding)?;
        issuer
            .verifying_key
            .verify_strict(
                &workflow_broker_event_signing_bytes(&envelope)?,
                &Signature::from_bytes(&signature_bytes),
            )
            .map_err(|_| WorkflowBrokerError::InvalidSignature)?;

        if envelope.audience != self.audience {
            return Err(WorkflowBrokerError::AudienceMismatch);
        }
        if &envelope.project_id != expected_project_id {
            return Err(WorkflowBrokerError::ProjectMismatch);
        }
        require_digest("action_packet_digest", &envelope.action_packet_digest)?;
        require_nonblank("origin_principal_id", &envelope.origin_principal_id.0)?;
        require_nonblank("separation_domain", &envelope.separation_domain.0)?;
        if envelope.event_kind != envelope.semantic_input.kind() {
            return Err(WorkflowBrokerError::EventKindMismatch);
        }
        validate_nonce(&envelope.nonce)?;
        validate_semantic_input(&envelope.semantic_input)?;
        if issuer.entry.profile != envelope.issuer_profile {
            return Err(WorkflowBrokerError::IssuerProfileMismatch);
        }
        require_profile_kind(envelope.issuer_profile, envelope.event_kind)?;
        validate_event_native_host_provenance(&envelope)?;
        let event_digest = workflow_broker_event_digest(&envelope)?;
        let replay_key = WorkflowBrokerReplayKey {
            issuer_id: envelope.issuer_id.clone(),
            origin_principal_id: envelope.origin_principal_id.clone(),
            separation_domain: envelope.separation_domain.clone(),
            project_id: envelope.project_id.clone(),
            nonce_fingerprint: raw_digest(envelope.nonce.as_bytes()),
            event_digest: event_digest.clone(),
        };
        let audit = VerifiedWorkflowBrokerEventAudit {
            issuer_id: envelope.issuer_id,
            issuer_profile: envelope.issuer_profile,
            origin_principal_id: envelope.origin_principal_id,
            separation_domain: envelope.separation_domain,
            event_kind: envelope.event_kind,
            project_id: envelope.project_id,
            action_packet_digest: envelope.action_packet_digest,
            event_digest,
            public_key_fingerprint: raw_digest(&issuer.verifying_key.to_bytes()),
            signature_fingerprint: raw_digest(&signature_bytes),
            enrollment_ceremony_digest: issuer.entry.enrollment.ceremony_digest.clone(),
            replay_key,
            native_host_provenance: envelope.native_host_provenance,
            issued_at_unix: envelope.issued_at_unix,
            expires_at_unix: envelope.expires_at_unix,
        };
        Ok(HistoricallyVerifiedWorkflowBrokerEvent {
            semantic_input: envelope.semantic_input,
            audit,
        })
    }
}

/// Domain-separated canonical bytes for the external broker to sign.
///
/// # Errors
/// Returns a canonicalization error if the typed event cannot be encoded.
pub fn workflow_broker_event_signing_bytes(
    envelope: &WorkflowBrokerEventEnvelope,
) -> Result<Vec<u8>, WorkflowBrokerError> {
    #[derive(Serialize)]
    struct SignedV1<'a> {
        schema_version: &'a str,
        audience: &'a str,
        issuer_id: &'a StableId,
        issuer_profile: WorkflowBrokerIssuerProfile,
        origin_principal_id: &'a PrincipalId,
        separation_domain: &'a StableId,
        event_kind: WorkflowBrokerEventKind,
        project_id: &'a StableId,
        action_packet_digest: &'a str,
        semantic_input: &'a WorkflowBrokerSemanticInput,
        issued_at_unix: u64,
        expires_at_unix: u64,
        nonce: &'a str,
    }

    #[derive(Serialize)]
    struct SignedV2<'a> {
        schema_version: &'a str,
        audience: &'a str,
        issuer_id: &'a StableId,
        issuer_profile: WorkflowBrokerIssuerProfile,
        origin_principal_id: &'a PrincipalId,
        separation_domain: &'a StableId,
        event_kind: WorkflowBrokerEventKind,
        project_id: &'a StableId,
        action_packet_digest: &'a str,
        semantic_input: &'a WorkflowBrokerSemanticInput,
        native_host_provenance: &'a WorkflowBrokerNativeHostProvenance,
        issued_at_unix: u64,
        expires_at_unix: u64,
        nonce: &'a str,
    }

    validate_event_schema_shape(envelope, false)?;
    let (value, domain) = match envelope.schema_version.as_str() {
        WORKFLOW_BROKER_LEGACY_EVENT_SCHEMA_VERSION => (
            serde_json::to_value(SignedV1 {
                schema_version: &envelope.schema_version,
                audience: &envelope.audience,
                issuer_id: &envelope.issuer_id,
                issuer_profile: envelope.issuer_profile,
                origin_principal_id: &envelope.origin_principal_id,
                separation_domain: &envelope.separation_domain,
                event_kind: envelope.event_kind,
                project_id: &envelope.project_id,
                action_packet_digest: &envelope.action_packet_digest,
                semantic_input: &envelope.semantic_input,
                issued_at_unix: envelope.issued_at_unix,
                expires_at_unix: envelope.expires_at_unix,
                nonce: &envelope.nonce,
            }),
            WORKFLOW_BROKER_SIGNATURE_DOMAIN,
        ),
        WORKFLOW_BROKER_EVENT_SCHEMA_VERSION => (
            serde_json::to_value(SignedV2 {
                schema_version: &envelope.schema_version,
                audience: &envelope.audience,
                issuer_id: &envelope.issuer_id,
                issuer_profile: envelope.issuer_profile,
                origin_principal_id: &envelope.origin_principal_id,
                separation_domain: &envelope.separation_domain,
                event_kind: envelope.event_kind,
                project_id: &envelope.project_id,
                action_packet_digest: &envelope.action_packet_digest,
                semantic_input: &envelope.semantic_input,
                native_host_provenance: envelope
                    .native_host_provenance
                    .as_ref()
                    .ok_or(WorkflowBrokerError::MissingNativeHostProvenance)?,
                issued_at_unix: envelope.issued_at_unix,
                expires_at_unix: envelope.expires_at_unix,
                nonce: &envelope.nonce,
            }),
            WORKFLOW_BROKER_SIGNATURE_DOMAIN_V2,
        ),
        other => {
            return Err(WorkflowBrokerError::UnsupportedEventSchema(
                other.to_owned(),
            ))
        }
    };
    let value = value.map_err(|error| WorkflowBrokerError::Canonicalization(error.to_string()))?;
    let canonical = serde_json_canonicalizer::to_vec(&value)
        .map_err(|error| WorkflowBrokerError::Canonicalization(error.to_string()))?;
    let mut bytes = Vec::with_capacity(domain.len() + canonical.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&canonical);
    Ok(bytes)
}

/// Recompute the signed, content-free descriptor for one native host event.
///
/// # Errors
/// Returns a canonicalization error if the typed descriptor cannot be encoded.
pub fn workflow_broker_host_event_descriptor_digest(
    provenance: &WorkflowBrokerNativeHostProvenance,
    project_id: &StableId,
    action_packet_digest: &str,
    semantic_input: &WorkflowBrokerSemanticInput,
) -> Result<String, WorkflowBrokerError> {
    let semantic_value = serde_json::to_value(semantic_input)
        .map_err(|error| WorkflowBrokerError::Canonicalization(error.to_string()))?;
    let semantic_digest =
        canonical_domain_digest(WORKFLOW_BROKER_SEMANTIC_INPUT_DOMAIN, &semantic_value)?;
    let descriptor = serde_json::json!({
        "schema_version": "workflow_host_event_descriptor_v1",
        "host_kind": provenance.host_kind,
        "host_version": provenance.host_version,
        "adapter_id": provenance.adapter_id,
        "adapter_version": provenance.adapter_version,
        "interaction_kind": provenance.interaction_kind,
        "host_event_ref": provenance.host_event_ref,
        "host_session_ref": provenance.host_session_ref,
        "host_interaction_ref": provenance.host_interaction_ref,
        "host_observed_at_unix": provenance.host_observed_at_unix,
        "project_id": project_id,
        "action_packet_digest": action_packet_digest,
        "semantic_input_digest": semantic_digest,
    });
    canonical_domain_digest(WORKFLOW_BROKER_HOST_DESCRIPTOR_DOMAIN, &descriptor)
}

fn canonical_domain_digest(
    domain: &[u8],
    value: &serde_json::Value,
) -> Result<String, WorkflowBrokerError> {
    let canonical = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| WorkflowBrokerError::Canonicalization(error.to_string()))?;
    let mut bytes = Vec::with_capacity(domain.len() + canonical.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&canonical);
    Ok(raw_digest(&bytes))
}

fn validate_event_schema_shape(
    envelope: &WorkflowBrokerEventEnvelope,
    new_admission: bool,
) -> Result<(), WorkflowBrokerError> {
    match envelope.schema_version.as_str() {
        WORKFLOW_BROKER_LEGACY_EVENT_SCHEMA_VERSION => {
            if new_admission {
                return Err(WorkflowBrokerError::HistoricalEventNotAdmissible);
            }
            if envelope.native_host_provenance.is_some() {
                return Err(WorkflowBrokerError::UnexpectedNativeHostProvenance);
            }
            Ok(())
        }
        WORKFLOW_BROKER_EVENT_SCHEMA_VERSION => {
            if envelope.native_host_provenance.is_none() {
                return Err(WorkflowBrokerError::MissingNativeHostProvenance);
            }
            Ok(())
        }
        other => Err(WorkflowBrokerError::UnsupportedEventSchema(
            other.to_owned(),
        )),
    }
}

fn validate_event_native_host_provenance(
    envelope: &WorkflowBrokerEventEnvelope,
) -> Result<(), WorkflowBrokerError> {
    match envelope.native_host_provenance.as_ref() {
        Some(provenance) => validate_native_host_provenance(envelope, provenance),
        None => Ok(()),
    }
}

fn validate_native_host_provenance(
    envelope: &WorkflowBrokerEventEnvelope,
    provenance: &WorkflowBrokerNativeHostProvenance,
) -> Result<(), WorkflowBrokerError> {
    validate_bounded_protocol_text(
        "native_host_provenance.host_version",
        &provenance.host_version,
        MAX_WORKFLOW_BROKER_VERSION_BYTES,
    )?;
    validate_bounded_protocol_text(
        "native_host_provenance.adapter_id",
        &provenance.adapter_id.0,
        MAX_WORKFLOW_BROKER_VERSION_BYTES,
    )?;
    validate_bounded_protocol_text(
        "native_host_provenance.adapter_version",
        &provenance.adapter_version,
        MAX_WORKFLOW_BROKER_VERSION_BYTES,
    )?;
    semver::Version::parse(&provenance.adapter_version).map_err(|_| {
        WorkflowBrokerError::InvalidField {
            field: "native_host_provenance.adapter_version",
            reason: "must be an exact SemVer version",
        }
    })?;
    validate_opaque_host_ref(
        "native_host_provenance.host_event_ref",
        &provenance.host_event_ref,
    )?;
    validate_opaque_host_ref(
        "native_host_provenance.host_session_ref",
        &provenance.host_session_ref,
    )?;
    validate_opaque_host_ref(
        "native_host_provenance.host_interaction_ref",
        &provenance.host_interaction_ref,
    )?;
    require_digest(
        "native_host_provenance.host_event_descriptor_digest",
        &provenance.host_event_descriptor_digest,
    )?;
    if provenance.host_observed_at_unix == 0
        || provenance.host_observed_at_unix > envelope.issued_at_unix
        || envelope
            .issued_at_unix
            .saturating_sub(provenance.host_observed_at_unix)
            > MAX_HOST_OBSERVATION_TO_ISSUANCE_SECONDS
    {
        return Err(WorkflowBrokerError::HostObservationOutOfBounds);
    }
    let compatible = matches!(
        (envelope.issuer_profile, provenance.interaction_kind),
        (
            WorkflowBrokerIssuerProfile::Human,
            WorkflowBrokerHostInteractionKind::NativeHumanMessage
                | WorkflowBrokerHostInteractionKind::NativeHumanConfirmation
        ) | (
            WorkflowBrokerIssuerProfile::Reviewer,
            WorkflowBrokerHostInteractionKind::NativeReviewerConfirmation
        ) | (
            WorkflowBrokerIssuerProfile::Runtime,
            WorkflowBrokerHostInteractionKind::AttestedRuntimeObservation
        )
    );
    if !compatible {
        return Err(WorkflowBrokerError::HostInteractionProfileMismatch);
    }
    let expected = workflow_broker_host_event_descriptor_digest(
        provenance,
        &envelope.project_id,
        &envelope.action_packet_digest,
        &envelope.semantic_input,
    )?;
    if provenance.host_event_descriptor_digest != expected {
        return Err(WorkflowBrokerError::HostDescriptorDigestMismatch);
    }
    Ok(())
}

fn validate_bounded_protocol_text(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), WorkflowBrokerError> {
    if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(WorkflowBrokerError::InvalidField {
            field,
            reason: "must be non-blank, bounded, and control-character-free",
        });
    }
    Ok(())
}

fn validate_opaque_host_ref(field: &'static str, value: &str) -> Result<(), WorkflowBrokerError> {
    if !(MIN_WORKFLOW_BROKER_OPAQUE_REF_BYTES..=MAX_WORKFLOW_BROKER_OPAQUE_REF_BYTES)
        .contains(&value.len())
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
        || !value.is_ascii()
    {
        return Err(WorkflowBrokerError::InvalidField {
            field,
            reason: "must be a bounded opaque ASCII handle without whitespace",
        });
    }
    Ok(())
}
/// Canonical identity of the complete event, including its signature.
///
/// # Errors
/// Returns a canonicalization error if the typed event cannot be encoded.
pub fn workflow_broker_event_digest(
    envelope: &WorkflowBrokerEventEnvelope,
) -> Result<String, WorkflowBrokerError> {
    let value = serde_json::to_value(envelope)
        .map_err(|error| WorkflowBrokerError::Canonicalization(error.to_string()))?;
    let canonical = serde_json_canonicalizer::to_vec(&value)
        .map_err(|error| WorkflowBrokerError::Canonicalization(error.to_string()))?;
    Ok(raw_digest(&canonical))
}

fn validate_semantic_input(input: &WorkflowBrokerSemanticInput) -> Result<(), WorkflowBrokerError> {
    match input {
        WorkflowBrokerSemanticInput::Applicability { basis_refs, .. }
        | WorkflowBrokerSemanticInput::Signal { basis_refs, .. } => {
            require_nonempty_refs("semantic_input.basis_refs", basis_refs)
        }
        WorkflowBrokerSemanticInput::Capability {
            probe_ref,
            subject_ref,
            ..
        } => {
            require_nonblank("semantic_input.probe_ref", probe_ref)?;
            require_nonblank("semantic_input.subject_ref", subject_ref)
        }
        WorkflowBrokerSemanticInput::Decision {
            selected_alternative_ref,
        } => require_nonblank(
            "semantic_input.selected_alternative_ref",
            &selected_alternative_ref.0,
        ),
        WorkflowBrokerSemanticInput::Evidence {
            subject_ref,
            scenario_ref,
            ..
        } => {
            require_nonblank("semantic_input.subject_ref", subject_ref)?;
            require_nonblank("semantic_input.scenario_ref", scenario_ref)
        }
        WorkflowBrokerSemanticInput::IntentRevision {
            desired_outcome,
            constraints,
            preferences,
            unacceptable_outcomes,
            uncertainties,
            conversation_ref,
            conversation_digest,
        } => {
            validate_bounded_text(
                "semantic_input.desired_outcome",
                desired_outcome,
                MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES,
            )?;
            validate_intent_items("semantic_input.constraints", constraints)?;
            validate_intent_items("semantic_input.preferences", preferences)?;
            validate_intent_items(
                "semantic_input.unacceptable_outcomes",
                unacceptable_outcomes,
            )?;
            validate_intent_items("semantic_input.uncertainties", uncertainties)?;
            validate_bounded_text(
                "semantic_input.conversation_ref",
                conversation_ref,
                MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES,
            )?;
            require_digest("semantic_input.conversation_digest", conversation_digest)?;
            validate_intent_total_size(input)
        }
        WorkflowBrokerSemanticInput::Waiver { reason } => {
            require_nonblank("semantic_input.reason", reason)
        }
    }
}

fn require_profile_kind(
    profile: WorkflowBrokerIssuerProfile,
    kind: WorkflowBrokerEventKind,
) -> Result<(), WorkflowBrokerError> {
    let allowed = matches!(
        (profile, kind),
        (
            WorkflowBrokerIssuerProfile::Human,
            WorkflowBrokerEventKind::Applicability
                | WorkflowBrokerEventKind::Decision
                | WorkflowBrokerEventKind::Evidence
                | WorkflowBrokerEventKind::IntentRevision
                | WorkflowBrokerEventKind::Waiver
        ) | (
            WorkflowBrokerIssuerProfile::Reviewer,
            WorkflowBrokerEventKind::Evidence | WorkflowBrokerEventKind::Signal
        ) | (
            WorkflowBrokerIssuerProfile::Runtime,
            WorkflowBrokerEventKind::Capability
                | WorkflowBrokerEventKind::Evidence
                | WorkflowBrokerEventKind::Signal
        )
    );
    if allowed {
        Ok(())
    } else {
        Err(WorkflowBrokerError::ProfileKindMismatch)
    }
}

fn validate_bounded_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), WorkflowBrokerError> {
    require_nonblank(field, value)?;
    if value.len() > max_bytes {
        Err(WorkflowBrokerError::InvalidField {
            field,
            reason: "exceeds the byte limit",
        })
    } else {
        Ok(())
    }
}

fn validate_intent_items(
    field: &'static str,
    values: &[String],
) -> Result<(), WorkflowBrokerError> {
    if values.len() > MAX_WORKFLOW_INTENT_LIST_ITEMS {
        return Err(WorkflowBrokerError::InvalidField {
            field,
            reason: "contains too many items",
        });
    }
    for value in values {
        validate_bounded_text(field, value, MAX_WORKFLOW_INTENT_ITEM_BYTES)?;
    }
    Ok(())
}

fn validate_intent_total_size(
    input: &WorkflowBrokerSemanticInput,
) -> Result<(), WorkflowBrokerError> {
    let encoded = serde_json_canonicalizer::to_vec(
        &serde_json::to_value(input)
            .map_err(|error| WorkflowBrokerError::Canonicalization(error.to_string()))?,
    )
    .map_err(|error| WorkflowBrokerError::Canonicalization(error.to_string()))?;
    if encoded.len() > MAX_WORKFLOW_INTENT_TOTAL_BYTES {
        Err(WorkflowBrokerError::InvalidField {
            field: "semantic_input.intent_revision",
            reason: "exceeds the total byte limit",
        })
    } else {
        Ok(())
    }
}

fn validate_freshness(
    envelope: &WorkflowBrokerEventEnvelope,
    now_unix: i64,
    policy: WorkflowBrokerFreshnessPolicy,
) -> Result<(), WorkflowBrokerError> {
    let now = u64::try_from(now_unix).map_err(|_| WorkflowBrokerError::InvalidClock)?;
    if envelope.issued_at_unix == 0
        || envelope.expires_at_unix <= envelope.issued_at_unix
        || envelope
            .expires_at_unix
            .saturating_sub(envelope.issued_at_unix)
            > policy.max_ttl_seconds
        || envelope.issued_at_unix > now.saturating_add(policy.max_future_skew_seconds)
        || now.saturating_sub(envelope.issued_at_unix) > policy.max_age_seconds
        || envelope.expires_at_unix <= now
    {
        return Err(WorkflowBrokerError::FreshnessOutOfBounds);
    }
    Ok(())
}

fn validate_nonce(value: &str) -> Result<(), WorkflowBrokerError> {
    if !(16..=256).contains(&value.len()) || value.chars().any(char::is_control) {
        Err(WorkflowBrokerError::InvalidNonce)
    } else {
        Ok(())
    }
}

fn require_nonblank(field: &'static str, value: &str) -> Result<(), WorkflowBrokerError> {
    if value.trim().is_empty() {
        Err(WorkflowBrokerError::InvalidField {
            field,
            reason: "must not be blank",
        })
    } else {
        Ok(())
    }
}

fn require_nonempty_refs(
    field: &'static str,
    values: &[String],
) -> Result<(), WorkflowBrokerError> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        Err(WorkflowBrokerError::InvalidField {
            field,
            reason: "must contain only non-blank references",
        })
    } else {
        Ok(())
    }
}

fn require_digest(field: &'static str, value: &str) -> Result<(), WorkflowBrokerError> {
    let valid = value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    });
    if valid {
        Ok(())
    } else {
        Err(WorkflowBrokerError::InvalidField {
            field,
            reason: "must be a lowercase sha256 digest",
        })
    }
}

fn decode_fixed<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowBrokerError {
    UnsupportedRegistrySchema(String),
    UnsupportedEventSchema(String),
    HistoricalEventNotAdmissible,
    MissingNativeHostProvenance,
    UnexpectedNativeHostProvenance,
    HostObservationOutOfBounds,
    HostInteractionProfileMismatch,
    HostDescriptorDigestMismatch,
    EmptyRegistry,
    DuplicateIssuer(String),
    DuplicatePublicKey,
    InvalidPublicKey(String),
    InvalidField {
        field: &'static str,
        reason: &'static str,
    },
    UnknownIssuer(String),
    IssuerRevoked(String),
    AudienceMismatch,
    ProjectMismatch,
    IssuerProfileMismatch,
    EventKindMismatch,
    ProfileKindMismatch,
    InvalidNonce,
    InvalidClock,
    FreshnessOutOfBounds,
    InvalidSignatureEncoding,
    InvalidSignature,
    Canonicalization(String),
}

impl fmt::Display for WorkflowBrokerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedRegistrySchema(found) => {
                write!(formatter, "unsupported broker registry schema '{found}'")
            }
            Self::UnsupportedEventSchema(found) => {
                write!(formatter, "unsupported broker event schema '{found}'")
            }
            Self::HistoricalEventNotAdmissible => formatter.write_str(
                "legacy broker events are recovery-only and cannot authorize a new mutation",
            ),
            Self::MissingNativeHostProvenance => {
                formatter.write_str("broker event is missing native host provenance")
            }
            Self::UnexpectedNativeHostProvenance => {
                formatter.write_str("legacy broker event cannot contain native host provenance")
            }
            Self::HostObservationOutOfBounds => {
                formatter.write_str("native host observation time is out of bounds")
            }
            Self::HostInteractionProfileMismatch => formatter
                .write_str("native host interaction kind is incompatible with broker profile"),
            Self::HostDescriptorDigestMismatch => {
                formatter.write_str("native host event descriptor digest mismatch")
            }
            Self::EmptyRegistry => formatter.write_str("broker registry is empty"),
            Self::DuplicateIssuer(value) => write!(formatter, "duplicate broker issuer '{value}'"),
            Self::DuplicatePublicKey => formatter.write_str("broker public keys must be unique"),
            Self::InvalidPublicKey(value) => write!(formatter, "invalid key for issuer '{value}'"),
            Self::InvalidField { field, reason } => write!(formatter, "invalid {field}: {reason}"),
            Self::UnknownIssuer(value) => write!(formatter, "unknown broker issuer '{value}'"),
            Self::IssuerRevoked(value) => write!(formatter, "broker issuer '{value}' is revoked"),
            Self::AudienceMismatch => formatter.write_str("broker audience mismatch"),
            Self::ProjectMismatch => formatter.write_str("broker project mismatch"),
            Self::IssuerProfileMismatch => formatter.write_str("broker profile mismatch"),
            Self::EventKindMismatch => formatter.write_str("broker event kind mismatch"),
            Self::ProfileKindMismatch => formatter.write_str("broker profile cannot assert kind"),
            Self::InvalidNonce => formatter.write_str("broker nonce is invalid"),
            Self::InvalidClock => formatter.write_str("verification clock is before Unix epoch"),
            Self::FreshnessOutOfBounds => formatter.write_str("broker freshness is out of bounds"),
            Self::InvalidSignatureEncoding => formatter.write_str("invalid signature encoding"),
            Self::InvalidSignature => formatter.write_str("broker signature verification failed"),
            Self::Canonicalization(error) => write!(formatter, "canonicalization failed: {error}"),
        }
    }
}

impl std::error::Error for WorkflowBrokerError {}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use forge_core_contracts::RuntimeKind;

    const NOW: i64 = 1_900_000_000;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut value = String::new();
        for byte in bytes {
            let _ = write!(value, "{byte:02x}");
        }
        value
    }

    fn assert_no_forbidden_keys(value: &serde_json::Value, forbidden: &[&str]) {
        match value {
            serde_json::Value::Object(fields) => {
                for key in fields.keys() {
                    assert!(!forbidden.contains(&key.as_str()), "unexpected key {key}");
                }
                for value in fields.values() {
                    assert_no_forbidden_keys(value, forbidden);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    assert_no_forbidden_keys(value, forbidden);
                }
            }
            _ => {}
        }
    }

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7_u8; 32])
    }

    fn registry(
        key: &SigningKey,
        profile: WorkflowBrokerIssuerProfile,
    ) -> AuthorizedWorkflowBrokerRegistry {
        AuthorizedWorkflowBrokerRegistry::from_document(WorkflowBrokerRegistryDocument {
            schema_version: WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION.to_owned(),
            audience: "forge-host:test".to_owned(),
            issuers: vec![WorkflowBrokerIssuerEntry {
                issuer_id: StableId("broker.host".to_owned()),
                profile,
                public_key_hex: hex(key.verifying_key().as_bytes()),
                status: WorkflowBrokerIssuerStatus::Active,
                enrollment: WorkflowBrokerEnrollmentDeclaration {
                    ceremony_ref: "operator://ceremony/1".to_owned(),
                    ceremony_digest: digest('a'),
                    declared_at_unix: NOW as u64 - 60,
                },
            }],
        })
        .expect("registry")
    }

    #[test]
    fn project_bound_admission_rejects_a_registry_copied_from_another_audience() {
        let key = key();
        let document = WorkflowBrokerRegistryDocument {
            schema_version: WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION.to_owned(),
            audience: "forge-core:workflow:project.source".to_owned(),
            issuers: vec![WorkflowBrokerIssuerEntry {
                issuer_id: StableId("broker.host".to_owned()),
                profile: WorkflowBrokerIssuerProfile::Human,
                public_key_hex: hex(key.verifying_key().as_bytes()),
                status: WorkflowBrokerIssuerStatus::Active,
                enrollment: WorkflowBrokerEnrollmentDeclaration {
                    ceremony_ref: "operator://ceremony/source".to_owned(),
                    ceremony_digest: digest('a'),
                    declared_at_unix: NOW as u64 - 60,
                },
            }],
        };

        assert_eq!(
            AuthorizedWorkflowBrokerRegistry::from_document_for_audience(
                document,
                "forge-core:workflow:project.target",
            )
            .expect_err("cross-project registry"),
            WorkflowBrokerError::AudienceMismatch
        );
    }

    fn refresh_host_descriptor(envelope: &mut WorkflowBrokerEventEnvelope) {
        let provenance = envelope
            .native_host_provenance
            .as_mut()
            .expect("native host provenance");
        provenance.interaction_kind = match envelope.issuer_profile {
            WorkflowBrokerIssuerProfile::Human => {
                WorkflowBrokerHostInteractionKind::NativeHumanConfirmation
            }
            WorkflowBrokerIssuerProfile::Reviewer => {
                WorkflowBrokerHostInteractionKind::NativeReviewerConfirmation
            }
            WorkflowBrokerIssuerProfile::Runtime => {
                WorkflowBrokerHostInteractionKind::AttestedRuntimeObservation
            }
        };
        provenance.host_observed_at_unix = envelope.issued_at_unix;
        provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
            provenance,
            &envelope.project_id,
            &envelope.action_packet_digest,
            &envelope.semantic_input,
        )
        .expect("host descriptor");
    }

    fn unsigned() -> WorkflowBrokerEventEnvelope {
        let mut envelope = WorkflowBrokerEventEnvelope {
            schema_version: WORKFLOW_BROKER_EVENT_SCHEMA_VERSION.to_owned(),
            audience: "forge-host:test".to_owned(),
            issuer_id: StableId("broker.host".to_owned()),
            issuer_profile: WorkflowBrokerIssuerProfile::Human,
            origin_principal_id: PrincipalId("principal.human.owner".to_owned()),
            separation_domain: StableId("human.owner.session".to_owned()),
            event_kind: WorkflowBrokerEventKind::Decision,
            project_id: StableId("project.test".to_owned()),
            action_packet_digest: digest('b'),
            semantic_input: WorkflowBrokerSemanticInput::Decision {
                selected_alternative_ref: StableId("alternative.safe".to_owned()),
            },
            native_host_provenance: Some(WorkflowBrokerNativeHostProvenance {
                host_kind: RuntimeKind::ForgeStandalone,
                host_version: "0.12.0".to_owned(),
                adapter_id: StableId("adapter.forge-standalone".to_owned()),
                adapter_version: "0.1.0".to_owned(),
                interaction_kind: WorkflowBrokerHostInteractionKind::NativeHumanConfirmation,
                host_event_ref: "event-ref-0000000001".to_owned(),
                host_session_ref: "session-ref-00000001".to_owned(),
                host_interaction_ref: "interaction-ref-000001".to_owned(),
                host_event_descriptor_digest: digest('0'),
                host_observed_at_unix: NOW as u64 - 5,
            }),
            issued_at_unix: NOW as u64 - 5,
            expires_at_unix: NOW as u64 + 120,
            nonce: "origin-event-nonce-0001".to_owned(),
            signature: String::new(),
        };
        refresh_host_descriptor(&mut envelope);
        envelope
    }

    fn unsigned_intent_revision() -> WorkflowBrokerEventEnvelope {
        WorkflowBrokerEventEnvelope {
            event_kind: WorkflowBrokerEventKind::IntentRevision,
            semantic_input: WorkflowBrokerSemanticInput::IntentRevision {
                desired_outcome: "Enable a novice to build a dependable product".to_owned(),
                constraints: vec!["The human interacts only through conversation".to_owned()],
                preferences: vec!["Prefer reversible technical choices".to_owned()],
                unacceptable_outcomes: vec![
                    "Do not claim readiness without representative evidence".to_owned(),
                ],
                uncertainties: vec!["The delivery domain is not known yet".to_owned()],
                conversation_ref: "conversation://thread/intent-turn-42".to_owned(),
                conversation_digest: digest('d'),
            },
            nonce: "intent-revision-nonce-0001".to_owned(),
            ..unsigned()
        }
    }

    fn sign(
        mut envelope: WorkflowBrokerEventEnvelope,
        key: &SigningKey,
    ) -> WorkflowBrokerEventEnvelope {
        refresh_host_descriptor(&mut envelope);
        let bytes = workflow_broker_event_signing_bytes(&envelope).expect("bytes");
        envelope.signature = hex(&key.sign(&bytes).to_bytes());
        envelope
    }

    fn sign_exact_provenance(
        mut envelope: WorkflowBrokerEventEnvelope,
        key: &SigningKey,
    ) -> WorkflowBrokerEventEnvelope {
        let provenance = envelope
            .native_host_provenance
            .as_mut()
            .expect("native host provenance");
        provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
            provenance,
            &envelope.project_id,
            &envelope.action_packet_digest,
            &envelope.semantic_input,
        )
        .expect("host descriptor");
        let bytes = workflow_broker_event_signing_bytes(&envelope).expect("bytes");
        envelope.signature = hex(&key.sign(&bytes).to_bytes());
        envelope
    }

    fn verify(
        registry: &AuthorizedWorkflowBrokerRegistry,
        envelope: WorkflowBrokerEventEnvelope,
        now: i64,
    ) -> Result<VerifiedWorkflowBrokerEvent, WorkflowBrokerError> {
        registry.verify_event(
            envelope,
            &StableId("project.test".to_owned()),
            now,
            WorkflowBrokerFreshnessPolicy::default(),
        )
    }

    #[test]
    fn v0_2_requires_valid_opaque_native_host_provenance() {
        let key = key();
        let registry = registry(&key, WorkflowBrokerIssuerProfile::Human);

        let mut missing = unsigned();
        missing.native_host_provenance = None;
        assert_eq!(
            workflow_broker_event_signing_bytes(&missing).expect_err("missing provenance"),
            WorkflowBrokerError::MissingNativeHostProvenance
        );

        let mut mismatched = unsigned();
        mismatched
            .native_host_provenance
            .as_mut()
            .expect("provenance")
            .host_event_descriptor_digest = digest('f');
        let bytes = workflow_broker_event_signing_bytes(&mismatched).expect("shape-valid bytes");
        mismatched.signature = hex(&key.sign(&bytes).to_bytes());
        assert_eq!(
            verify(&registry, mismatched, NOW).expect_err("descriptor mismatch"),
            WorkflowBrokerError::HostDescriptorDigestMismatch
        );

        let mut malformed_version = unsigned();
        malformed_version
            .native_host_provenance
            .as_mut()
            .expect("provenance")
            .adapter_version = "v0.1".to_owned();
        refresh_host_descriptor(&mut malformed_version);
        malformed_version
            .native_host_provenance
            .as_mut()
            .expect("provenance")
            .adapter_version = "v0.1".to_owned();
        let provenance = malformed_version
            .native_host_provenance
            .as_mut()
            .expect("provenance");
        provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
            provenance,
            &malformed_version.project_id,
            &malformed_version.action_packet_digest,
            &malformed_version.semantic_input,
        )
        .expect("descriptor");
        let bytes = workflow_broker_event_signing_bytes(&malformed_version).expect("signed shape");
        malformed_version.signature = hex(&key.sign(&bytes).to_bytes());
        assert!(matches!(
            verify(&registry, malformed_version, NOW),
            Err(WorkflowBrokerError::InvalidField {
                field: "native_host_provenance.adapter_version",
                ..
            })
        ));

        let mut raw = serde_json::to_value(unsigned()).expect("event JSON");
        raw.as_object_mut().expect("event object").insert(
            "raw_transcript".to_owned(),
            serde_json::json!("human said yes"),
        );
        assert!(serde_json::from_value::<WorkflowBrokerEventEnvelope>(raw).is_err());
    }

    #[test]
    fn v0_2_rejects_profile_time_reference_and_tamper_attacks() {
        let key = key();
        let registry = registry(&key, WorkflowBrokerIssuerProfile::Human);

        let mut incompatible = unsigned();
        incompatible
            .native_host_provenance
            .as_mut()
            .expect("provenance")
            .interaction_kind = WorkflowBrokerHostInteractionKind::NativeReviewerConfirmation;
        assert_eq!(
            verify(&registry, sign_exact_provenance(incompatible, &key), NOW)
                .expect_err("human profile cannot assert reviewer interaction"),
            WorkflowBrokerError::HostInteractionProfileMismatch
        );

        let issued_at = unsigned().issued_at_unix;
        for observed_at in [
            0,
            issued_at + 1,
            issued_at - MAX_HOST_OBSERVATION_TO_ISSUANCE_SECONDS - 1,
        ] {
            let mut invalid_time = unsigned();
            invalid_time
                .native_host_provenance
                .as_mut()
                .expect("provenance")
                .host_observed_at_unix = observed_at;
            assert_eq!(
                verify(&registry, sign_exact_provenance(invalid_time, &key), NOW)
                    .expect_err("host observation time must be bounded"),
                WorkflowBrokerError::HostObservationOutOfBounds
            );
        }

        for invalid_ref in [
            "short",
            "opaque ref contains whitespace",
            "referência-não-ascii-0001",
            "opaque-control-\u{1b}-ref-0001",
        ] {
            let mut invalid_reference = unsigned();
            invalid_reference
                .native_host_provenance
                .as_mut()
                .expect("provenance")
                .host_event_ref = invalid_ref.to_owned();
            assert!(matches!(
                verify(
                    &registry,
                    sign_exact_provenance(invalid_reference, &key),
                    NOW,
                ),
                Err(WorkflowBrokerError::InvalidField {
                    field: "native_host_provenance.host_event_ref",
                    ..
                })
            ));
        }

        let mut tampered_after_signing = sign(unsigned(), &key);
        tampered_after_signing
            .native_host_provenance
            .as_mut()
            .expect("provenance")
            .interaction_kind = WorkflowBrokerHostInteractionKind::NativeReviewerConfirmation;
        assert_eq!(
            verify(&registry, tampered_after_signing, NOW)
                .expect_err("signed provenance tamper must fail before content validation"),
            WorkflowBrokerError::InvalidSignature
        );

        let mut semantic_tamper = sign(unsigned(), &key);
        let WorkflowBrokerSemanticInput::Decision {
            selected_alternative_ref,
        } = &mut semantic_tamper.semantic_input
        else {
            panic!("decision semantic fixture");
        };
        selected_alternative_ref.0.clear();
        assert_eq!(
            verify(&registry, semantic_tamper.clone(), NOW)
                .expect_err("live verification must authenticate before semantic validation"),
            WorkflowBrokerError::InvalidSignature
        );
        assert_eq!(
            registry
                .verify_event_for_recovery(semantic_tamper, &StableId("project.test".to_owned()),)
                .expect_err("recovery verification must authenticate before semantic validation"),
            WorkflowBrokerError::InvalidSignature
        );
    }
    #[test]
    fn frozen_v0_1_bytes_remain_recovery_only() {
        let key = key();
        let registry = registry(&key, WorkflowBrokerIssuerProfile::Human);
        let mut envelope = unsigned();
        envelope.schema_version = WORKFLOW_BROKER_LEGACY_EVENT_SCHEMA_VERSION.to_owned();
        envelope.native_host_provenance = None;
        envelope.signature.clear();

        let signing_bytes = workflow_broker_event_signing_bytes(&envelope).expect("v0.1 bytes");
        assert_eq!(
            &signing_bytes[..WORKFLOW_BROKER_SIGNATURE_DOMAIN.len()],
            WORKFLOW_BROKER_SIGNATURE_DOMAIN
        );
        assert_eq!(
            std::str::from_utf8(&signing_bytes[WORKFLOW_BROKER_SIGNATURE_DOMAIN.len()..])
                .expect("canonical UTF-8"),
            r#"{"action_packet_digest":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","audience":"forge-host:test","event_kind":"decision","expires_at_unix":1900000120,"issued_at_unix":1899999995,"issuer_id":"broker.host","issuer_profile":"human","nonce":"origin-event-nonce-0001","origin_principal_id":"principal.human.owner","project_id":"project.test","schema_version":"0.1","semantic_input":{"kind":"decision","selected_alternative_ref":"alternative.safe"},"separation_domain":"human.owner.session"}"#
        );
        envelope.signature = hex(&key.sign(&signing_bytes).to_bytes());
        assert_eq!(
            envelope.signature,
            "20b52d86c9b0629392d82d9b8f58b73957b9eee2bba6c0623339afe84b03fc35d6d14b5157e910937bda434ad57317015bc771a93a891ac35079ae586ccb430d"
        );
        assert_eq!(
            workflow_broker_event_digest(&envelope).expect("v0.1 digest"),
            "sha256:46c466f5c032c8d5b722d4cef8034a0fe7572df804c260d5b39d948db2d1cdc6"
        );
        assert_eq!(
            verify(&registry, envelope.clone(), NOW).expect_err("v0.1 new admission"),
            WorkflowBrokerError::HistoricalEventNotAdmissible
        );
        registry
            .verify_event_for_recovery(envelope, &StableId("project.test".to_owned()))
            .expect("v0.1 exact historical recovery");
    }
    #[test]
    fn verifies_external_signature_and_returns_kernel_replay_identity() {
        let key = key();
        let envelope = sign(unsigned(), &key);
        let verified = verify(
            &registry(&key, WorkflowBrokerIssuerProfile::Human),
            envelope,
            NOW,
        )
        .expect("verified");
        assert_eq!(
            verified.audit().event_kind,
            WorkflowBrokerEventKind::Decision
        );
        assert!(verified
            .audit()
            .replay_key
            .event_digest
            .starts_with("sha256:"));
        assert!(matches!(
            verified.semantic_input(),
            WorkflowBrokerSemanticInput::Decision { .. }
        ));
    }

    #[test]
    fn verifies_closed_human_intent_revision_and_binds_source_provenance() {
        let key = key();
        let envelope = sign(unsigned_intent_revision(), &key);
        let verified = verify(
            &registry(&key, WorkflowBrokerIssuerProfile::Human),
            envelope,
            NOW,
        )
        .expect("verified intent revision");
        assert_eq!(
            verified.audit().event_kind,
            WorkflowBrokerEventKind::IntentRevision
        );
        assert!(matches!(
            verified.semantic_input(),
            WorkflowBrokerSemanticInput::IntentRevision {
                conversation_ref,
                conversation_digest,
                ..
            } if conversation_ref == "conversation://thread/intent-turn-42"
                && conversation_digest == &digest('d')
        ));
    }

    #[test]
    fn intent_revision_is_human_only_and_tampering_fails_closed() {
        let key = key();
        let reviewer = registry(&key, WorkflowBrokerIssuerProfile::Reviewer);
        let mut reviewer_event = unsigned_intent_revision();
        reviewer_event.issuer_profile = WorkflowBrokerIssuerProfile::Reviewer;
        assert_eq!(
            verify(&reviewer, sign(reviewer_event, &key), NOW)
                .expect_err("reviewer cannot revise human intent"),
            WorkflowBrokerError::ProfileKindMismatch
        );

        let human = registry(&key, WorkflowBrokerIssuerProfile::Human);
        let mut tampered = sign(unsigned_intent_revision(), &key);
        let WorkflowBrokerSemanticInput::IntentRevision {
            desired_outcome, ..
        } = &mut tampered.semantic_input
        else {
            panic!("intent input");
        };
        desired_outcome.push_str(" but with an unapproved changed outcome");
        assert_eq!(
            verify(&human, tampered, NOW).expect_err("content tamper"),
            WorkflowBrokerError::InvalidSignature
        );

        let mut provenance_tamper = sign(unsigned_intent_revision(), &key);
        let WorkflowBrokerSemanticInput::IntentRevision {
            conversation_digest,
            ..
        } = &mut provenance_tamper.semantic_input
        else {
            panic!("intent input");
        };
        *conversation_digest = digest('e');
        assert_eq!(
            verify(&human, provenance_tamper, NOW).expect_err("provenance tamper"),
            WorkflowBrokerError::InvalidSignature
        );
    }

    #[test]
    fn intent_revision_enforces_canonical_bounds_and_exposes_no_authority_fields() {
        let key = key();
        let human = registry(&key, WorkflowBrokerIssuerProfile::Human);

        let mut oversized = unsigned_intent_revision();
        let WorkflowBrokerSemanticInput::IntentRevision {
            desired_outcome, ..
        } = &mut oversized.semantic_input
        else {
            panic!("intent input");
        };
        *desired_outcome = "x".repeat(MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES + 1);
        assert!(matches!(
            verify(&human, sign(oversized, &key), NOW),
            Err(WorkflowBrokerError::InvalidField {
                field: "semantic_input.desired_outcome",
                ..
            })
        ));

        let mut too_many = unsigned_intent_revision();
        let WorkflowBrokerSemanticInput::IntentRevision { constraints, .. } =
            &mut too_many.semantic_input
        else {
            panic!("intent input");
        };
        *constraints = vec!["bounded".to_owned(); MAX_WORKFLOW_INTENT_LIST_ITEMS + 1];
        assert!(matches!(
            verify(&human, sign(too_many, &key), NOW),
            Err(WorkflowBrokerError::InvalidField {
                field: "semantic_input.constraints",
                ..
            })
        ));

        let mut oversized_total = unsigned_intent_revision();
        let WorkflowBrokerSemanticInput::IntentRevision {
            constraints,
            preferences,
            unacceptable_outcomes,
            uncertainties,
            ..
        } = &mut oversized_total.semantic_input
        else {
            panic!("intent input");
        };
        for values in [
            constraints,
            preferences,
            unacceptable_outcomes,
            uncertainties,
        ] {
            *values = vec!["z".repeat(300); MAX_WORKFLOW_INTENT_LIST_ITEMS];
        }
        assert!(matches!(
            verify(&human, sign(oversized_total, &key), NOW),
            Err(WorkflowBrokerError::InvalidField {
                field: "semantic_input.intent_revision",
                ..
            })
        ));

        let json = serde_json::to_value(unsigned_intent_revision()).expect("intent json");
        let forbidden = [
            "raw_chat",
            "transcript",
            "intent_id",
            "revision",
            "assurance_epoch",
            "policy_ref",
            "claim_status",
            "readiness",
            "target_ref",
            "evaluator_ref",
        ];
        assert_no_forbidden_keys(&json, &forbidden);
    }

    #[test]
    fn historical_intent_revision_recovery_preserves_signature_without_reopening_admission() {
        let key = key();
        let mut expired = unsigned_intent_revision();
        expired.issued_at_unix = u64::try_from(NOW - 600).expect("positive time");
        expired.expires_at_unix = u64::try_from(NOW - 300).expect("positive time");
        let expired = sign(expired, &key);
        let active = registry(&key, WorkflowBrokerIssuerProfile::Human);
        assert!(matches!(
            verify(&active, expired.clone(), NOW),
            Err(WorkflowBrokerError::FreshnessOutOfBounds)
        ));
        let historical = active
            .verify_event_for_recovery(expired, &StableId("project.test".to_owned()))
            .expect("historical intent signature");
        assert_eq!(
            historical.audit().event_kind,
            WorkflowBrokerEventKind::IntentRevision
        );
    }

    #[test]
    fn verification_is_pure_and_does_not_consume_replay() {
        let key = key();
        let envelope = sign(unsigned(), &key);
        let registry = registry(&key, WorkflowBrokerIssuerProfile::Human);
        let first = verify(&registry, envelope.clone(), NOW).expect("first");
        let second = verify(&registry, envelope, NOW).expect("second");
        assert_eq!(first.audit().replay_key, second.audit().replay_key);
    }

    #[test]
    fn historical_verification_keeps_signature_checks_but_skips_freshness_and_active_status() {
        let key = key();
        let mut expired = unsigned();
        expired.issued_at_unix = u64::try_from(NOW - 600).expect("positive time");
        expired.expires_at_unix = u64::try_from(NOW - 300).expect("positive time");
        let expired = sign(expired, &key);
        let active = registry(&key, WorkflowBrokerIssuerProfile::Human);
        assert!(matches!(
            verify(&active, expired.clone(), NOW),
            Err(WorkflowBrokerError::FreshnessOutOfBounds)
        ));
        assert_eq!(
            active
                .verify_event_for_recovery(expired.clone(), &StableId("project.test".to_owned()))
                .expect("historical expired signature")
                .audit()
                .action_packet_digest,
            expired.action_packet_digest
        );

        let revoked =
            AuthorizedWorkflowBrokerRegistry::from_document(WorkflowBrokerRegistryDocument {
                schema_version: WORKFLOW_BROKER_REGISTRY_SCHEMA_VERSION.to_owned(),
                audience: "forge-host:test".to_owned(),
                issuers: vec![WorkflowBrokerIssuerEntry {
                    issuer_id: StableId("broker.host".to_owned()),
                    profile: WorkflowBrokerIssuerProfile::Human,
                    public_key_hex: hex(key.verifying_key().as_bytes()),
                    status: WorkflowBrokerIssuerStatus::Revoked,
                    enrollment: WorkflowBrokerEnrollmentDeclaration {
                        ceremony_ref: "operator://ceremony/1".to_owned(),
                        ceremony_digest: digest('a'),
                        declared_at_unix: NOW as u64 - 60,
                    },
                }],
            })
            .expect("retained revoked key");
        let current = sign(unsigned(), &key);
        assert!(matches!(
            verify(&revoked, current.clone(), NOW),
            Err(WorkflowBrokerError::IssuerRevoked(_))
        ));
        revoked
            .verify_event_for_recovery(current, &StableId("project.test".to_owned()))
            .expect("revoked key remains usable only for historical verification");

        let mut tampered = expired;
        tampered.action_packet_digest = digest('c');
        assert!(matches!(
            active.verify_event_for_recovery(tampered, &StableId("project.test".to_owned())),
            Err(WorkflowBrokerError::InvalidSignature)
        ));
    }

    #[test]
    fn tamper_profile_and_kind_fail_closed() {
        let key = key();
        let human = registry(&key, WorkflowBrokerIssuerProfile::Human);
        let mut tampered = sign(unsigned(), &key);
        tampered.action_packet_digest = digest('c');
        assert_eq!(
            verify(&human, tampered, NOW).expect_err("tamper"),
            WorkflowBrokerError::InvalidSignature
        );

        let mut identity_tamper = sign(unsigned(), &key);
        identity_tamper.origin_principal_id = PrincipalId("principal.agent.fake".to_owned());
        assert_eq!(
            verify(&human, identity_tamper, NOW).expect_err("identity tamper"),
            WorkflowBrokerError::InvalidSignature
        );

        let mut domain_tamper = sign(unsigned(), &key);
        domain_tamper.separation_domain = StableId("same-agent-domain".to_owned());
        assert_eq!(
            verify(&human, domain_tamper, NOW).expect_err("domain tamper"),
            WorkflowBrokerError::InvalidSignature
        );

        let signed = sign(unsigned(), &key);
        assert_eq!(
            verify(
                &registry(&key, WorkflowBrokerIssuerProfile::Reviewer),
                signed,
                NOW,
            )
            .expect_err("profile"),
            WorkflowBrokerError::IssuerProfileMismatch
        );

        let mut mismatch = unsigned();
        mismatch.event_kind = WorkflowBrokerEventKind::Waiver;
        let mismatch = sign(mismatch, &key);
        assert_eq!(
            verify(&human, mismatch, NOW).expect_err("kind"),
            WorkflowBrokerError::EventKindMismatch
        );
    }

    #[test]
    fn explicit_clock_rejects_stale_future_expired_and_negative_now() {
        let key = key();
        let registry = registry(&key, WorkflowBrokerIssuerProfile::Human);
        for (issued, expires, now, expected) in [
            (
                NOW as u64 - 301,
                NOW as u64 + 1,
                NOW,
                WorkflowBrokerError::FreshnessOutOfBounds,
            ),
            (
                NOW as u64 + 31,
                NOW as u64 + 60,
                NOW,
                WorkflowBrokerError::FreshnessOutOfBounds,
            ),
            (
                NOW as u64 - 5,
                NOW as u64,
                NOW,
                WorkflowBrokerError::FreshnessOutOfBounds,
            ),
            (
                NOW as u64 - 5,
                NOW as u64 + 1,
                -1,
                WorkflowBrokerError::InvalidClock,
            ),
        ] {
            let mut envelope = unsigned();
            envelope.issued_at_unix = issued;
            envelope.expires_at_unix = expires;
            let envelope = sign(envelope, &key);
            assert_eq!(
                verify(&registry, envelope, now).expect_err("clock"),
                expected
            );
        }
    }

    #[test]
    fn minimal_semantic_answers_reject_blank_values_and_carry_no_request_fields() {
        let key = key();
        let registry = registry(&key, WorkflowBrokerIssuerProfile::Human);
        let mut envelope = unsigned();
        envelope.semantic_input = WorkflowBrokerSemanticInput::Waiver {
            reason: " ".to_owned(),
        };
        envelope.event_kind = WorkflowBrokerEventKind::Waiver;
        let envelope = sign(envelope, &key);
        assert!(matches!(
            verify(&registry, envelope, NOW),
            Err(WorkflowBrokerError::InvalidField {
                field: "semantic_input.reason",
                ..
            })
        ));

        let json = serde_json::to_string(&unsigned()).expect("json");
        let value = serde_json::to_value(&unsigned().semantic_input).expect("legacy value");
        assert_eq!(
            value,
            serde_json::json!({
                "kind": "decision",
                "selected_alternative_ref": "alternative.safe"
            }),
            "the pre-intent decision wire shape must remain unchanged"
        );
        for forbidden in [
            "policy_bundle_digest",
            "policy_ref",
            "current_phase",
            "evaluator_ref",
            "snapshot_digest",
            "ledger_head_digest",
        ] {
            assert!(!json.contains(forbidden), "unexpected {forbidden}");
        }
    }
}
