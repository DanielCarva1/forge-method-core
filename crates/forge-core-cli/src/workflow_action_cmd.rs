//! One-call application of a host-authenticated workflow action packet.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_authority::{
    AttestationPolicy, AttestationVerifier, AuthorizedWorkflowBrokerRegistry,
    HistoricallyVerifiedWorkflowBrokerEvent, WorkflowAuthorizationKind, WorkflowBrokerControlError,
    WorkflowBrokerError, WorkflowBrokerEventEnvelope, WorkflowBrokerEventKind,
    WorkflowBrokerFreshnessPolicy, WorkflowBrokerVerificationContext,
    WORKFLOW_BROKER_LEGACY_EVENT_SCHEMA_VERSION,
};
use forge_core_contracts::{
    workflow_broker_expected_audience, CliEnvelope, StableId, WorkflowBrokerBoundOperation,
};
use forge_core_kernel::{
    PreparedWorkflowAuthorization, WorkflowAuthorizationApprovalBoundary,
    WorkflowAuthorizationClosedInput, WorkflowGovernanceProjectAdapter,
};

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

const MAX_AUTHORITY_DOCUMENT_BYTES: u64 = 1024 * 1024;

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let action = args.first().map_or("help", String::as_str);
    if matches!(action, "help" | "--help" | "-h") {
        println!("{}", usage());
        return Ok(());
    }
    if !matches!(action, "apply" | "authorize") {
        return Err(ExitError::usage(usage()));
    }
    let flags = parse_flags(action, &args[1..])?;
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    match action {
        "apply" => apply(&flags, want_json),
        "authorize" => authorize(&flags, want_json),
        _ => unreachable!("action validated above"),
    }
}

/// Prepare, sign, verify, and commit one local authority action without ever
/// materializing the derived request or attestation as an intermediate file.
fn authorize(flags: &BTreeMap<String, Vec<String>>, want_json: bool) -> Result<(), ExitError> {
    let root = required_path(flags, "--root")?;
    let packet_digest = required(flags, "--packet-digest")?;
    let input_path = required_path(flags, "--input-file")?;
    let credential_id = required(flags, "--credential-id")?;
    let input_raw = read_bounded(&input_path, "workflow action closed input")?;
    let input: WorkflowAuthorizationClosedInput =
        serde_json::from_str(&input_raw).map_err(|error| {
            ExitError::env_config(format!(
                "invalid workflow action closed input {}: {error}",
                input_path.display()
            ))
        })?;
    let adapter = resolve_adapter(&root)?;
    let now = now_unix()?;
    let prepared = adapter
        .prepare_authorization(packet_digest, input, now)
        .map_err(|error| {
            ExitError::failed(format!("workflow action preparation rejected: {error}"))
        })?;
    if prepared_approval_boundary(&prepared)
        != WorkflowAuthorizationApprovalBoundary::OperatorCredentialBroker
    {
        return Err(ExitError::failed(
            "this action packet requires the external human, independent-reviewer, or runtime broker; local credentials may authorize only operator_credential_broker packets"
                .to_owned(),
        ));
    }
    let verifier = AttestationVerifier::new(AttestationPolicy::Default);
    let record = match prepared {
        PreparedWorkflowAuthorization::Applicability { request, .. } => {
            let signed = crate::workflow_credential_cmd::sign_typed_request(
                &root,
                credential_id,
                WorkflowAuthorizationKind::Applicability,
                serde_json::to_value(&request).map_err(serialize_request)?,
            )?;
            let authorization = signed
                .registry
                .authorize_workflow_applicability(&verifier, request, &signed.attestation)
                .map_err(authority_rejected)?;
            adapter.record_authorized_applicability(authorization)
        }
        PreparedWorkflowAuthorization::Capability { request, .. } => {
            let signed = crate::workflow_credential_cmd::sign_typed_request(
                &root,
                credential_id,
                WorkflowAuthorizationKind::Capability,
                serde_json::to_value(&request).map_err(serialize_request)?,
            )?;
            let authorization = signed
                .registry
                .authorize_workflow_capability(&verifier, request, &signed.attestation)
                .map_err(authority_rejected)?;
            adapter.record_authorized_capability(authorization)
        }
        PreparedWorkflowAuthorization::Decision { request, .. } => {
            let signed = crate::workflow_credential_cmd::sign_typed_request(
                &root,
                credential_id,
                WorkflowAuthorizationKind::Decision,
                serde_json::to_value(&request).map_err(serialize_request)?,
            )?;
            let authorization = signed
                .registry
                .authorize_workflow_decision(&verifier, request, &signed.attestation)
                .map_err(authority_rejected)?;
            adapter.record_authorized_decision(authorization)
        }
        PreparedWorkflowAuthorization::Evidence { request, .. } => {
            let signed = crate::workflow_credential_cmd::sign_typed_request(
                &root,
                credential_id,
                WorkflowAuthorizationKind::Evidence,
                serde_json::to_value(&request).map_err(serialize_request)?,
            )?;
            let authorization = signed
                .registry
                .authorize_workflow_evidence(&verifier, request, &signed.attestation)
                .map_err(authority_rejected)?;
            adapter.record_authorized_evidence(authorization)
        }
        PreparedWorkflowAuthorization::Signal { request, .. } => {
            let signed = crate::workflow_credential_cmd::sign_typed_request(
                &root,
                credential_id,
                WorkflowAuthorizationKind::Signal,
                serde_json::to_value(&request).map_err(serialize_request)?,
            )?;
            let authorization = signed
                .registry
                .authorize_workflow_signal(&verifier, request, &signed.attestation)
                .map_err(authority_rejected)?;
            adapter.record_authorized_signal(authorization)
        }
        PreparedWorkflowAuthorization::Waiver { request, .. } => {
            let signed = crate::workflow_credential_cmd::sign_typed_request(
                &root,
                credential_id,
                WorkflowAuthorizationKind::Waiver,
                serde_json::to_value(&request).map_err(serialize_request)?,
            )?;
            let authorization = signed
                .registry
                .authorize_workflow_waiver(&verifier, request, &signed.attestation)
                .map_err(authority_rejected)?;
            adapter.record_authorized_waiver(authorization)
        }
    }
    .map_err(|error| ExitError::failed(format!("workflow action commit rejected: {error}")))?;
    emit_envelope(
        CliEnvelope::ok("workflow.action.authorize", record),
        want_json,
    )
}

fn prepared_approval_boundary(
    prepared: &PreparedWorkflowAuthorization,
) -> WorkflowAuthorizationApprovalBoundary {
    match prepared {
        PreparedWorkflowAuthorization::Applicability { packet, .. }
        | PreparedWorkflowAuthorization::Capability { packet, .. }
        | PreparedWorkflowAuthorization::Decision { packet, .. }
        | PreparedWorkflowAuthorization::Evidence { packet, .. }
        | PreparedWorkflowAuthorization::Signal { packet, .. }
        | PreparedWorkflowAuthorization::Waiver { packet, .. } => {
            packet.required_authority.approval_boundary
        }
    }
}

fn serialize_request(error: serde_json::Error) -> ExitError {
    ExitError::env_config(format!(
        "serialize kernel-prepared workflow request: {error}"
    ))
}

fn authority_rejected(error: impl std::fmt::Display) -> ExitError {
    ExitError::failed(format!(
        "workflow authority rejected prepared request: {error}"
    ))
}

fn apply(flags: &BTreeMap<String, Vec<String>>, want_json: bool) -> Result<(), ExitError> {
    let root = required_path(flags, "--root")?;
    let envelope_path = required_path(flags, "--origin-envelope-file")?;
    apply_origin_envelope(
        &root,
        &envelope_path,
        None,
        "workflow.action.apply",
        want_json,
    )
}

/// Verify one broker envelope through the shared public action path. A strict
/// registry may admit a current mutation only when its complete administration
/// journal is verified under the retained Store lock; a standalone legacy
/// registry can present only typed historical evidence for exact recovery. A
/// specialized caller may require an exact semantic kind before any mutation.
pub(crate) fn apply_origin_envelope(
    root: &Path,
    envelope_path: &Path,
    required_kind: Option<WorkflowBrokerEventKind>,
    command: &'static str,
    want_json: bool,
) -> Result<(), ExitError> {
    let envelope_raw = read_bounded(envelope_path, "workflow broker origin envelope")?;
    let envelope: WorkflowBrokerEventEnvelope =
        serde_json::from_str(&envelope_raw).map_err(|error| {
            ExitError::env_config(format!(
                "invalid workflow broker envelope {}: {error}",
                envelope_path.display()
            ))
        })?;
    if required_kind
        .is_some_and(|kind| envelope.event_kind != kind || envelope.semantic_input.kind() != kind)
    {
        return Err(ExitError::failed(
            "workflow intent record accepts only an intent_revision envelope from an external human broker"
                .to_owned(),
        ));
    }
    let adapter = resolve_adapter(root)?;
    let expected_audience = workflow_broker_expected_audience(
        &adapter.binding().project_id,
        &StableId("workflow.governance".to_owned()),
    );
    let authority = crate::workflow_broker_cmd::lock_workflow_broker_action_authority(root)?;
    let receipt = match &authority {
        crate::workflow_broker_cmd::LockedWorkflowBrokerActionAuthority::Strict {
            control,
            ..
        } => {
            let now = now_unix()?;
            let now_i64 = i64::try_from(now)
                .map_err(|_| ExitError::env_config("system clock exceeds i64".to_owned()))?;
            let context = WorkflowBrokerVerificationContext {
                audience: expected_audience,
                project_id: adapter.binding().project_id.clone(),
                workflow_id: StableId("workflow.governance".to_owned()),
                operation: bound_operation(envelope.event_kind),
            };
            match control.verify_bound_event(
                envelope.clone(),
                &context,
                now_i64,
                WorkflowBrokerFreshnessPolicy::default(),
            ) {
                Ok(verified) => adapter.apply_verified_bound_broker_action(verified, now),
                Err(current_error @ WorkflowBrokerControlError::EventAuthority(
                    WorkflowBrokerError::FreshnessOutOfBounds
                    | WorkflowBrokerError::IssuerRevoked(_)
                    | WorkflowBrokerError::HistoricalEventNotAdmissible,
                )) => {
                    let historical = control
                        .verify_bound_event_for_recovery(envelope, &context)
                        .map_err(|historical_error| {
                            ExitError::failed(format!(
                                "workflow broker event rejected: {current_error}; historical verification also failed: {historical_error}"
                            ))
                        })?;
                    adapter.recover_historically_verified_bound_broker_action(historical)
                }
                Err(error) => {
                    return Err(ExitError::failed(format!(
                        "workflow broker event rejected: {error}"
                    )))
                }
            }
        }
        crate::workflow_broker_cmd::LockedWorkflowBrokerActionAuthority::LegacyRecoveryOnly {
            registry,
            ..
        } => {
            let historical = verify_legacy_registry_event_for_recovery(
                registry,
                envelope,
                &adapter.binding().project_id,
            )?;
            adapter.recover_historically_verified_broker_action(historical)
        }
    }
    .map_err(|error| ExitError::failed(format!("workflow action rejected: {error}")))?;
    emit_envelope(CliEnvelope::ok(command, receipt), want_json)
}

fn verify_legacy_registry_event_for_recovery(
    registry: &AuthorizedWorkflowBrokerRegistry,
    envelope: WorkflowBrokerEventEnvelope,
    expected_project_id: &StableId,
) -> Result<HistoricallyVerifiedWorkflowBrokerEvent, ExitError> {
    if envelope.schema_version != WORKFLOW_BROKER_LEGACY_EVENT_SCHEMA_VERSION {
        return Err(ExitError::failed(format!(
            "legacy workflow broker registry is recovery-only for {WORKFLOW_BROKER_LEGACY_EVENT_SCHEMA_VERSION} events; {} events require a strict registry with exact registry-generation, workflow, selected-host, and operation bindings",
            envelope.schema_version
        )));
    }
    registry
        .verify_event_for_recovery(envelope, expected_project_id)
        .map_err(|error| {
            ExitError::failed(format!(
                "historical workflow broker event rejected: {error}"
            ))
        })
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

fn resolve_adapter(root: &Path) -> Result<WorkflowGovernanceProjectAdapter, ExitError> {
    let project = crate::project_cmd::resolve_project(root)
        .map_err(|error| ExitError::env_config(format!("project resolve failed: {error}")))?;
    if !project.state_exists {
        return Err(ExitError::env_config(format!(
            "resolved state root {} does not exist; run forge-core start first",
            project.state_root
        )));
    }
    WorkflowGovernanceProjectAdapter::new(
        StableId(project.project_id),
        PathBuf::from(project.project_root),
        PathBuf::from(project.state_root),
    )
    .map_err(|error| ExitError::env_config(error.to_string()))
}

fn read_bounded(path: &Path, label: &str) -> Result<String, ExitError> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        ExitError::env_config(format!("read {label} metadata {}: {error}", path.display()))
    })?;
    if metadata.len() > MAX_AUTHORITY_DOCUMENT_BYTES {
        return Err(ExitError::env_config(format!(
            "{label} {} exceeds {} bytes",
            path.display(),
            MAX_AUTHORITY_DOCUMENT_BYTES
        )));
    }
    std::fs::read_to_string(path)
        .map_err(|error| ExitError::env_config(format!("read {label} {}: {error}", path.display())))
}

fn parse_flags(action: &str, args: &[String]) -> Result<BTreeMap<String, Vec<String>>, ExitError> {
    let mut flags = BTreeMap::<String, Vec<String>>::new();
    let mut index = 0usize;
    while index < args.len() {
        let flag = args[index].as_str();
        if matches!(flag, "--json" | "--no-json" | "--text") {
            index += 1;
            continue;
        }
        let accepted = match action {
            "apply" => matches!(flag, "--root" | "--origin-envelope-file"),
            "authorize" => matches!(
                flag,
                "--root" | "--packet-digest" | "--input-file" | "--credential-id"
            ),
            _ => false,
        };
        if !accepted {
            return Err(ExitError::usage(format!(
                "unknown flag '{flag}' for workflow action apply"
            )));
        }
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| ExitError::usage(format!("{flag} requires a value")))?;
        if value.starts_with('-') {
            return Err(ExitError::usage(format!(
                "{flag} requires a value, got flag '{value}'"
            )));
        }
        flags
            .entry(flag.to_owned())
            .or_default()
            .push(value.clone());
        index += 1;
    }
    if let Some((flag, _)) = flags.iter().find(|(_, values)| values.len() != 1) {
        return Err(ExitError::usage(format!(
            "{flag} may be supplied only once"
        )));
    }
    Ok(flags)
}

fn required<'a>(
    flags: &'a BTreeMap<String, Vec<String>>,
    flag: &str,
) -> Result<&'a str, ExitError> {
    flags
        .get(flag)
        .and_then(|values| values.first())
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(format!("{flag} is required")))
}

fn required_path(flags: &BTreeMap<String, Vec<String>>, flag: &str) -> Result<PathBuf, ExitError> {
    required(flags, flag).map(PathBuf::from)
}

fn now_unix() -> Result<u64, ExitError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| {
            ExitError::env_config(format!("system clock is before Unix epoch: {error}"))
        })
}

fn usage() -> String {
    "usage:\n  forge-core workflow action authorize --root <project> --packet-digest <sha256> --input-file <closed-input.json> --credential-id <id> [--json|--no-json]\n  forge-core workflow action apply --root <project> --origin-envelope-file <signed-json> [--json|--no-json]"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use forge_core_authority::{
        workflow_broker_event_signing_bytes, workflow_broker_host_event_descriptor_digest,
        WorkflowBrokerEnrollmentDeclaration, WorkflowBrokerIssuerEntry,
        WorkflowBrokerIssuerProfile, WorkflowBrokerIssuerStatus, WorkflowBrokerRegistryDocument,
        WorkflowBrokerSemanticInput, WORKFLOW_BROKER_EVENT_SCHEMA_VERSION,
    };
    use forge_core_contracts::{
        PrincipalId, RuntimeKind, WorkflowBrokerHostInteractionKind,
        WorkflowBrokerNativeHostProvenance,
    };

    const NOW: i64 = 1_900_000_000;

    #[test]
    fn legacy_registry_yields_only_typed_historical_evidence_for_v0_1() {
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let registry = legacy_registry(&key);
        let historical: HistoricallyVerifiedWorkflowBrokerEvent =
            verify_legacy_registry_event_for_recovery(
                &registry,
                signed_event(WORKFLOW_BROKER_LEGACY_EVENT_SCHEMA_VERSION, &key),
                &StableId("project.test".to_owned()),
            )
            .expect("legacy wire must produce recovery-only evidence");
        assert_eq!(
            historical.audit().event_kind,
            WorkflowBrokerEventKind::Decision
        );
    }

    #[test]
    fn legacy_registry_rejects_new_v0_2_instead_of_downgrading_strict_bindings() {
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let registry = legacy_registry(&key);
        let envelope = signed_event(WORKFLOW_BROKER_EVENT_SCHEMA_VERSION, &key);
        assert!(
            envelope.native_host_provenance.is_some(),
            "fixture must carry the v0.2 native-host evidence that requires strict binding"
        );

        let error = verify_legacy_registry_event_for_recovery(
            &registry,
            envelope,
            &StableId("project.test".to_owned()),
        )
        .expect_err("v0.2 must require the strict control plane");
        assert!(
            error.message().contains(
                "strict registry with exact registry-generation, workflow, selected-host, and operation bindings"
            ),
            "{error:?}"
        );
    }

    fn legacy_registry(key: &SigningKey) -> AuthorizedWorkflowBrokerRegistry {
        AuthorizedWorkflowBrokerRegistry::from_document_for_audience(
            WorkflowBrokerRegistryDocument {
                schema_version: "0.1".to_owned(),
                audience: "forge-core:workflow:project.test".to_owned(),
                issuers: vec![WorkflowBrokerIssuerEntry {
                    issuer_id: StableId("issuer.human.test".to_owned()),
                    profile: WorkflowBrokerIssuerProfile::Human,
                    public_key_hex: lower_hex(key.verifying_key().as_bytes()),
                    status: WorkflowBrokerIssuerStatus::Active,
                    enrollment: WorkflowBrokerEnrollmentDeclaration {
                        ceremony_ref: "ceremony:test:0001".to_owned(),
                        ceremony_digest: digest('b'),
                        declared_at_unix: 1,
                    },
                }],
            },
            "forge-core:workflow:project.test",
        )
        .expect("legacy registry fixture")
    }

    fn signed_event(schema_version: &str, key: &SigningKey) -> WorkflowBrokerEventEnvelope {
        let mut envelope = WorkflowBrokerEventEnvelope {
            schema_version: schema_version.to_owned(),
            audience: "forge-core:workflow:project.test".to_owned(),
            issuer_id: StableId("issuer.human.test".to_owned()),
            issuer_profile: WorkflowBrokerIssuerProfile::Human,
            origin_principal_id: PrincipalId("principal.human.test".to_owned()),
            separation_domain: StableId("human.test.session".to_owned()),
            event_kind: WorkflowBrokerEventKind::Decision,
            project_id: StableId("project.test".to_owned()),
            action_packet_digest: digest('a'),
            semantic_input: WorkflowBrokerSemanticInput::Decision {
                selected_alternative_ref: StableId("alternative.safe".to_owned()),
            },
            native_host_provenance: (schema_version == WORKFLOW_BROKER_EVENT_SCHEMA_VERSION).then(
                || WorkflowBrokerNativeHostProvenance {
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
                },
            ),
            issued_at_unix: NOW as u64 - 5,
            expires_at_unix: NOW as u64 + 120,
            nonce: "event-operation-nonce-0001".to_owned(),
            signature: String::new(),
        };
        if let Some(provenance) = envelope.native_host_provenance.as_mut() {
            provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
                provenance,
                &envelope.project_id,
                &envelope.action_packet_digest,
                &envelope.semantic_input,
            )
            .expect("host event descriptor");
        }
        envelope.signature = lower_hex(
            &key.sign(&workflow_broker_event_signing_bytes(&envelope).expect("signing bytes"))
                .to_bytes(),
        );
        envelope
    }

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn lower_hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;

        let mut value = String::new();
        for byte in bytes {
            let _ = write!(value, "{byte:02x}");
        }
        value
    }
}
