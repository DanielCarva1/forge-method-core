//! One-call application of a host-authenticated workflow action packet.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_authority::{
    AttestationPolicy, AttestationVerifier, AuthorizedWorkflowBrokerRegistry,
    WorkflowAuthorizationKind, WorkflowBrokerError, WorkflowBrokerEventEnvelope,
    WorkflowBrokerEventKind, WorkflowBrokerFreshnessPolicy, WorkflowBrokerRegistryDocument,
};
use forge_core_contracts::{CliEnvelope, StableId};
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

/// Verify and apply one broker envelope through the shared public mutation
/// path. A specialized caller may require an exact semantic kind; that check
/// happens immediately after bounded parsing and before any kernel mutation.
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
    let registry_path = adapter.trusted_broker_registry_path();
    reject_existing_links(&registry_path)?;
    let registry_raw = read_bounded(&registry_path, "workflow broker registry")?;
    let registry_document: WorkflowBrokerRegistryDocument = yaml_serde::from_str(&registry_raw)
        .map_err(|error| {
            ExitError::env_config(format!(
                "invalid workflow broker registry {}: {error}",
                registry_path.display()
            ))
        })?;
    let registry =
        AuthorizedWorkflowBrokerRegistry::from_document(registry_document).map_err(|error| {
            ExitError::env_config(format!("invalid workflow broker registry: {error}"))
        })?;
    let now = now_unix()?;
    let current = registry.verify_event(
        envelope.clone(),
        &adapter.binding().project_id,
        i64::try_from(now)
            .map_err(|_| ExitError::env_config("system clock exceeds i64".to_owned()))?,
        WorkflowBrokerFreshnessPolicy::default(),
    );
    let receipt = match current {
        Ok(verified) => adapter.apply_verified_broker_action(verified, now),
        Err(
            current_error @ (WorkflowBrokerError::FreshnessOutOfBounds
            | WorkflowBrokerError::IssuerRevoked(_)),
        ) => {
            let historical = registry
                .verify_event_for_recovery(envelope, &adapter.binding().project_id)
                .map_err(|historical_error| {
                    ExitError::failed(format!(
                        "workflow broker event rejected: {current_error}; historical verification also failed: {historical_error}"
                    ))
                })?;
            adapter.recover_historically_verified_broker_action(historical)
        }
        Err(error) => {
            return Err(ExitError::failed(format!(
                "workflow broker event rejected: {error}"
            )))
        }
    }
    .map_err(|error| ExitError::failed(format!("workflow action rejected: {error}")))?;
    emit_envelope(CliEnvelope::ok(command, receipt), want_json)
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

fn reject_existing_links(path: &Path) -> Result<(), ExitError> {
    for current in path.ancestors() {
        match std::fs::symlink_metadata(current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ExitError::env_config(format!(
                    "workflow broker registry path contains a symlink, junction, or reparse-point alias: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ExitError::env_config(format!(
                    "inspect workflow broker registry path {}: {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(())
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
