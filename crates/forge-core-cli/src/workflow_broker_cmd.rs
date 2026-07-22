//! Public lifecycle for the strict external workflow-origin broker control plane.
//!
//! Forge persists only a versioned public registry and content-free administration
//! receipts. Every registry mutation consumes an already-signed, native-host-bound
//! administration envelope and the exact proposed registry snapshot. This module
//! exposes no key generation, private-key input, generic signing, MCP signing, or
//! caller-selected semantic signing path.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use forge_core_authority::{
    workflow_broker_public_key_fingerprint, AuthorizedWorkflowBrokerControlPlane,
    AuthorizedWorkflowBrokerRegistry, WorkflowBrokerRegistryDocument,
};
use forge_core_contracts::{
    workflow_broker_admin_operation_digest, workflow_broker_expected_audience,
    workflow_broker_native_admin_replay_digest, workflow_broker_public_registry_digest,
    CliEnvelope, StableId, WorkflowBrokerAdminOperation, WorkflowBrokerAdminOperationEnvelope,
    WorkflowBrokerAdminReceiptDocument, WorkflowBrokerComponentStatusDocument,
    WorkflowBrokerCredentialStatus, WorkflowBrokerExternalSetupBlockReason,
    WorkflowBrokerExternalSetupState, WorkflowBrokerNativeAdminReplayKey,
    WorkflowBrokerPublicCredentialMetadata, WorkflowBrokerPublicRegistryDocument,
    WorkflowBrokerRecoveryState,
};
use forge_core_kernel::WorkflowGovernanceProjectAdapter;
use forge_core_store::workflow_broker_admin::{
    WorkflowBrokerAdminStore, WorkflowBrokerAdminStoreError, MAX_WORKFLOW_BROKER_ADMIN_STATE_BYTES,
    MAX_WORKFLOW_BROKER_REGISTRY_BYTES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

const COMMAND: &str = "workflow broker";
const WORKFLOW_ID: &str = "workflow.governance";
const ADMIN_JOURNAL_SCHEMA_VERSION: &str = "0.1";
const ADMIN_STATE_FILE: &str = "workflow-broker-admin.json";
const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ADMIN_RECEIPTS: usize = 1024;

#[derive(Debug)]
struct BrokerPaths {
    project_id: StableId,
    workflow_id: StableId,
    project_root: PathBuf,
    state_root: PathBuf,
    operator_dir: PathBuf,
    registry: PathBuf,
    admin_state: PathBuf,
    audience: String,
}

/// Opaque retained producer authority used by backup and restore.
///
/// This is the same Store-owned lock and retained directory capability used by
/// broker administration, so backup cannot race registry publication or pending
/// recovery.
pub(crate) struct LockedWorkflowBrokerRegistry {
    store: WorkflowBrokerAdminStore,
    expected_audience: String,
    expected_project_id: StableId,
    expected_workflow_id: StableId,
}

/// Exact public broker-registry bytes; external private keys are structurally
/// absent from every admitted registry schema.
pub(crate) struct WorkflowBrokerRegistrySnapshot {
    raw_registry: Option<Vec<u8>>,
    raw_sha256: Option<String>,
}

impl WorkflowBrokerRegistrySnapshot {
    pub(crate) fn raw_registry(&self) -> Option<&[u8]> {
        self.raw_registry.as_deref()
    }

    pub(crate) fn raw_sha256(&self) -> Option<&str> {
        self.raw_sha256.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AppliedAdminOperation {
    envelope: WorkflowBrokerAdminOperationEnvelope,
    receipt: WorkflowBrokerAdminReceiptDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingAdminTransition {
    operation_id: StableId,
    operation_digest: String,
    expected_registry_file_digest: Option<String>,
    proposed_registry: WorkflowBrokerPublicRegistryDocument,
    proposed_registry_yaml: String,
    proposed_registry_file_digest: String,
    envelope: WorkflowBrokerAdminOperationEnvelope,
    receipt: WorkflowBrokerAdminReceiptDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdminJournal {
    schema_version: String,
    project_id: StableId,
    workflow_id: StableId,
    audience: String,
    registry_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    registry_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    registry_file_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    receipt_head_digest: Option<String>,
    receipts: Vec<AppliedAdminOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending: Option<PendingAdminTransition>,
}

#[derive(Debug)]
struct AdminSnapshot {
    journal: AdminJournal,
    registry: WorkflowBrokerPublicRegistryDocument,
    control: AuthorizedWorkflowBrokerControlPlane,
    state_file_digest: String,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct BrokerResult {
    action: String,
    component_state: String,
    registry_path: String,
    admin_state_path: String,
    audience: String,
    workflow_id: String,
    registry_generation: Option<u64>,
    registry_digest: Option<String>,
    receipt: Option<WorkflowBrokerAdminReceiptDocument>,
    credential: Option<BrokerCredentialStatus>,
    credentials: Vec<BrokerCredentialStatus>,
    component_status: Option<WorkflowBrokerComponentStatusDocument>,
    external_setup: WorkflowBrokerExternalSetupState,
    trust_boundary: String,
    claim_boundary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct BrokerCredentialStatus {
    credential_id: String,
    broker_id: String,
    subject_id: String,
    purpose: forge_core_contracts::WorkflowBrokerCredentialPurpose,
    profile: forge_core_contracts::WorkflowBrokerCredentialProfile,
    key_generation: u64,
    status: WorkflowBrokerCredentialStatus,
    custody: forge_core_contracts::WorkflowBrokerCustodyKind,
    host_binding: forge_core_contracts::WorkflowBrokerHostBinding,
    allowed_operations: Vec<forge_core_contracts::WorkflowBrokerBoundOperation>,
    public_key_fingerprint: String,
    not_before_unix: u64,
    revoked_at_unix: Option<u64>,
    predecessor_credential_id: Option<String>,
    enrollment_operation_id: String,
    revocation_operation_id: Option<String>,
}

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let action = args.first().map_or("help", String::as_str);
    if matches!(action, "help" | "--help" | "-h") {
        println!("{}", usage());
        return Ok(());
    }
    let flags = parse_flags(action, &args[1..])?;
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    match action {
        "initialize" => initialize(&flags, want_json),
        "register" => apply_transition(&flags, TransitionCommand::Enroll, want_json),
        "rotate" => apply_transition(&flags, TransitionCommand::Rotate, want_json),
        "revoke" => apply_transition(&flags, TransitionCommand::Revoke, want_json),
        "list" => read_projection(&flags, "broker_credential_list", None, want_json),
        "inspect" => {
            let credential_id = required(&flags, "--credential-id")?;
            read_projection(
                &flags,
                "broker_credential_inspection",
                Some(credential_id),
                want_json,
            )
        }
        "status" => read_projection(&flags, "broker_component_status", None, want_json),
        "conformance" => read_projection(&flags, "broker_conformance_status", None, want_json),
        _ => Err(ExitError::usage(usage())),
    }
}

#[derive(Debug, Clone, Copy)]
enum TransitionCommand {
    Enroll,
    Rotate,
    Revoke,
}

impl TransitionCommand {
    const fn action(self) -> &'static str {
        match self {
            Self::Enroll => "registered_broker_credential",
            Self::Rotate => "rotated_broker_credential",
            Self::Revoke => "revoked_broker_credential",
        }
    }

    fn matches(self, operation: &WorkflowBrokerAdminOperation) -> bool {
        matches!(
            (self, operation),
            (Self::Enroll, WorkflowBrokerAdminOperation::Enroll { .. })
                | (Self::Rotate, WorkflowBrokerAdminOperation::Rotate { .. })
                | (Self::Revoke, WorkflowBrokerAdminOperation::Revoke { .. })
        )
    }
}

fn initialize(flags: &BTreeMap<String, Vec<String>>, want_json: bool) -> Result<(), ExitError> {
    let paths = broker_paths(required_path(flags, "--root")?)?;
    let _registry_file = required_path(flags, "--registry-file")?;
    let _authorization_file = required_path(flags, "--authorization-file")?;
    emit_result(blocked_genesis_result(&paths), want_json)
}

fn apply_transition(
    flags: &BTreeMap<String, Vec<String>>,
    command: TransitionCommand,
    want_json: bool,
) -> Result<(), ExitError> {
    let paths = broker_paths(required_path(flags, "--root")?)?;
    let proposed = read_public_registry(
        &required_path(flags, "--registry-file")?,
        &paths,
        "workflow broker proposed registry",
    )?;
    let envelope: WorkflowBrokerAdminOperationEnvelope = read_json_bounded(
        &required_path(flags, "--authorization-file")?,
        "workflow broker administration authorization",
    )?;
    if !command.matches(&envelope.operation) {
        return Err(ExitError::usage(format!(
            "workflow broker command does not match signed operation {:?}",
            envelope.operation
        )));
    }
    let expected_flag = required(flags, "--expected-registry-digest")?;
    require_digest("--expected-registry-digest", expected_flag)?;
    if envelope.expected_registry_digest.as_deref() != Some(expected_flag) {
        return Err(ExitError::conflict(
            "--expected-registry-digest does not match the signed administration envelope"
                .to_owned(),
        ));
    }
    let credential_id = operation_credential_id(&envelope.operation).to_owned();
    let store = open_store(&paths)?;
    let snapshot = load_snapshot(&paths, &store)?.ok_or_else(not_initialized)?;
    if let Some(receipt) = exact_retry(&snapshot.journal, &envelope)? {
        return emit_result(
            result_from_snapshot(
                &paths,
                command.action(),
                &snapshot,
                Some(receipt),
                Some(credential_id.as_str()),
            )?,
            want_json,
        );
    }
    if snapshot.journal.registry_digest.as_deref() != Some(expected_flag) {
        return Err(ExitError::conflict(format!(
            "registry CAS mismatch: expected {expected_flag}, current {:?}",
            snapshot.journal.registry_digest
        )));
    }
    if snapshot.journal.receipts.len() >= MAX_ADMIN_RECEIPTS {
        return Err(ExitError::conflict(format!(
            "workflow broker administration receipt capacity {MAX_ADMIN_RECEIPTS} is exhausted"
        )));
    }
    let advance = snapshot
        .control
        .authorize_admin_transition(
            envelope.clone(),
            proposed,
            now_i64()?,
            snapshot.journal.receipt_head_digest.clone(),
        )
        .map_err(authority_error)?;
    let (control, receipt) = advance.into_parts();
    commit_advance(
        &paths,
        &store,
        snapshot.journal,
        Some(snapshot.state_file_digest),
        control,
        envelope,
        receipt,
        command.action(),
        Some(credential_id.as_str()),
        want_json,
    )
}

#[allow(clippy::too_many_arguments)]
fn commit_advance(
    paths: &BrokerPaths,
    store: &WorkflowBrokerAdminStore,
    mut journal: AdminJournal,
    expected_state_file_digest: Option<String>,
    proposed: AuthorizedWorkflowBrokerControlPlane,
    envelope: WorkflowBrokerAdminOperationEnvelope,
    receipt: WorkflowBrokerAdminReceiptDocument,
    action: &str,
    credential_id: Option<&str>,
    want_json: bool,
) -> Result<(), ExitError> {
    let proposed_yaml = registry_yaml(proposed.document())?;
    let proposed_file_digest = digest(proposed_yaml.as_bytes());
    let pending = PendingAdminTransition {
        operation_id: receipt.receipt.operation_id.clone(),
        operation_digest: receipt.receipt.operation_digest.clone(),
        expected_registry_file_digest: journal.registry_file_digest.clone(),
        proposed_registry: proposed.document().clone(),
        proposed_registry_yaml: proposed_yaml,
        proposed_registry_file_digest: proposed_file_digest,
        envelope,
        receipt: receipt.clone(),
    };
    journal.pending = Some(pending);
    validate_journal(&journal, paths)?;
    let prepared_bytes = journal_bytes(&journal)?;
    store
        .replace_admin_state(expected_state_file_digest.as_deref(), &prepared_bytes)
        .map_err(store_error)?;
    let recovered = load_snapshot(paths, store)?.ok_or_else(|| {
        ExitError::env_config("workflow broker state disappeared during recovery".to_owned())
    })?;
    emit_result(
        result_from_snapshot(paths, action, &recovered, Some(receipt), credential_id)?,
        want_json,
    )
}

fn load_snapshot(
    paths: &BrokerPaths,
    store: &WorkflowBrokerAdminStore,
) -> Result<Option<AdminSnapshot>, ExitError> {
    let Some(mut state_file) = store.read_admin_state().map_err(store_error)? else {
        return Ok(None);
    };
    let mut journal = parse_journal(state_file.bytes(), paths)?;
    if journal.pending.is_some() {
        recover_pending(paths, store, &mut journal, state_file.raw_sha256())?;
        state_file = store
            .read_admin_state()
            .map_err(store_error)?
            .ok_or_else(|| ExitError::env_config("broker admin state disappeared".to_owned()))?;
        journal = parse_journal(state_file.bytes(), paths)?;
    }
    let registry_file = store
        .read_registry()
        .map_err(store_error)?
        .ok_or_else(|| ExitError::env_config("broker registry is missing".to_owned()))?;
    let registry = parse_strict_registry_bytes(registry_file.bytes(), paths)?;
    let control = AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
        registry.clone(),
        &paths.audience,
        &paths.project_id,
        &paths.workflow_id,
    )
    .map_err(authority_error)?;
    verify_applied_journal(&control, &journal)?;
    if journal.registry_generation != registry.registry_generation
        || journal.registry_digest.as_deref() != Some(control.registry_digest())
        || journal.registry_file_digest.as_deref() != Some(registry_file.raw_sha256())
    {
        return Err(ExitError::env_config(
            "public broker registry differs from durable administration state".to_owned(),
        ));
    }
    Ok(Some(AdminSnapshot {
        journal,
        registry,
        control,
        state_file_digest: state_file.raw_sha256().to_owned(),
    }))
}

fn verify_applied_journal(
    control: &AuthorizedWorkflowBrokerControlPlane,
    journal: &AdminJournal,
) -> Result<(), ExitError> {
    for applied in &journal.receipts {
        control
            .verify_historical_admin_receipt(&applied.envelope, &applied.receipt)
            .map_err(authority_error)?;
    }
    Ok(())
}

fn recover_pending(
    paths: &BrokerPaths,
    store: &WorkflowBrokerAdminStore,
    journal: &mut AdminJournal,
    state_file_digest: &str,
) -> Result<(), ExitError> {
    let pending = journal.pending.clone().ok_or_else(|| {
        ExitError::env_config("pending broker recovery has no transition".to_owned())
    })?;
    if journal.registry_generation == 0 {
        return Err(ExitError::env_config(
            "blocked_external_dependency[selected_host_unavailable]: pending broker genesis recovery requires the preconfigured external operator trust anchor"
                .to_owned(),
        ));
    }
    let current = store.read_registry().map_err(store_error)?;
    let current_digest = current.as_ref().map(|file| file.raw_sha256().to_owned());
    if current_digest == pending.expected_registry_file_digest {
        let current_file = current.as_ref().ok_or_else(|| {
            ExitError::env_config(
                "prepared broker transition has no current registry bytes".to_owned(),
            )
        })?;
        let current_registry = parse_strict_registry_bytes(current_file.bytes(), paths)?;
        let current_control = AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            current_registry,
            &paths.audience,
            &paths.project_id,
            &paths.workflow_id,
        )
        .map_err(authority_error)?;
        verify_applied_journal(&current_control, journal)?;
        let recovered_receipt = current_control
            .recover_authorized_admin_transition(
                pending.envelope.clone(),
                pending.proposed_registry.clone(),
                journal.receipt_head_digest.clone(),
            )
            .map_err(authority_error)?
            .receipt()
            .clone();
        if recovered_receipt != pending.receipt {
            return Err(ExitError::env_config(
                "prepared broker transition receipt does not match recovered authority".to_owned(),
            ));
        }
    } else if current_digest.as_deref() == Some(pending.proposed_registry_file_digest.as_str()) {
        let proposed_control = AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            pending.proposed_registry.clone(),
            &paths.audience,
            &paths.project_id,
            &paths.workflow_id,
        )
        .map_err(authority_error)?;
        verify_applied_journal(&proposed_control, journal)?;
        proposed_control
            .verify_historical_admin_receipt(&pending.envelope, &pending.receipt)
            .map_err(authority_error)?;
    }
    let installed = if current_digest == pending.expected_registry_file_digest {
        store
            .replace_registry(
                current_digest.as_deref(),
                pending.proposed_registry_yaml.as_bytes(),
            )
            .map_err(store_error)?
    } else if current_digest.as_deref() == Some(pending.proposed_registry_file_digest.as_str()) {
        current.ok_or_else(|| {
            ExitError::env_config("broker registry digest exists without bytes".to_owned())
        })?
    } else {
        return Err(ExitError::conflict(format!(
            "pending broker recovery found unbound registry file digest {current_digest:?}"
        )));
    };
    if installed.raw_sha256() != pending.proposed_registry_file_digest
        || installed.bytes() != pending.proposed_registry_yaml.as_bytes()
    {
        return Err(ExitError::env_config(
            "recovered broker registry bytes differ from the authorized target".to_owned(),
        ));
    }
    let parsed = parse_strict_registry_bytes(installed.bytes(), paths)?;
    if parsed != pending.proposed_registry {
        return Err(ExitError::env_config(
            "recovered broker registry document differs from the authorized target".to_owned(),
        ));
    }
    journal.registry_generation = pending.receipt.receipt.proposed_registry_generation;
    journal.registry_digest = Some(pending.receipt.receipt.proposed_registry_digest.clone());
    journal.registry_file_digest = Some(pending.proposed_registry_file_digest);
    journal.receipt_head_digest = Some(pending.receipt.receipt.receipt_digest.clone());
    journal.receipts.push(AppliedAdminOperation {
        envelope: pending.envelope,
        receipt: pending.receipt,
    });
    journal.pending = None;
    validate_journal(journal, paths)?;
    store
        .replace_admin_state(Some(state_file_digest), &journal_bytes(journal)?)
        .map_err(store_error)?;
    Ok(())
}

fn exact_retry(
    journal: &AdminJournal,
    envelope: &WorkflowBrokerAdminOperationEnvelope,
) -> Result<Option<WorkflowBrokerAdminReceiptDocument>, ExitError> {
    let operation_digest = workflow_broker_admin_operation_digest(envelope)
        .map_err(|error| ExitError::env_config(error.to_string()))?;
    let Some(applied) = journal
        .receipts
        .iter()
        .find(|applied| applied.receipt.receipt.operation_id == envelope.operation_id)
    else {
        if journal
            .pending
            .as_ref()
            .is_some_and(|pending| pending.operation_id == envelope.operation_id)
        {
            return Err(ExitError::conflict(
                "administration operation is pending durable recovery; retry after recovery"
                    .to_owned(),
            ));
        }
        return Ok(None);
    };
    let receipt = &applied.receipt;
    let signature_fingerprint = signature_fingerprint(&envelope.signature)?;
    if receipt.receipt.operation_digest == operation_digest
        && receipt.receipt.signature_fingerprint == signature_fingerprint
        && receipt.receipt.audience == envelope.audience
        && receipt.receipt.project_id == envelope.project_id
        && receipt.receipt.workflow_id == envelope.workflow_id
        && receipt.receipt.admin_credential_id == envelope.admin_credential_id
        && receipt.receipt.admin_credential_generation == envelope.admin_credential_generation
        && receipt.receipt.expected_registry_generation == envelope.expected_registry_generation
        && receipt.receipt.expected_registry_digest == envelope.expected_registry_digest
        && receipt.receipt.proposed_registry_generation == envelope.proposed_registry_generation
        && receipt.receipt.proposed_registry_digest == envelope.proposed_registry_digest
        && receipt.receipt.native_authorization_descriptor_digest
            == envelope.native_authorization.descriptor_digest
        && receipt.receipt.authorized_at_unix == envelope.issued_at_unix
    {
        Ok(Some(receipt.clone()))
    } else {
        Err(ExitError::conflict(
            "administration operation id was already used with different canonical input"
                .to_owned(),
        ))
    }
}

fn validate_receipt_envelope_binding(
    envelope: &WorkflowBrokerAdminOperationEnvelope,
    receipt: &WorkflowBrokerAdminReceiptDocument,
) -> Result<(), ExitError> {
    let operation_digest = workflow_broker_admin_operation_digest(envelope)
        .map_err(|error| ExitError::env_config(error.to_string()))?;
    let signature_fingerprint = signature_fingerprint(&envelope.signature)?;
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
        .map_err(|error| ExitError::env_config(error.to_string()))?;
    let value = &receipt.receipt;
    if value.operation_id != envelope.operation_id
        || value.operation_digest != operation_digest
        || value.audience != envelope.audience
        || value.project_id != envelope.project_id
        || value.workflow_id != envelope.workflow_id
        || value.admin_credential_id != envelope.admin_credential_id
        || value.admin_credential_generation != envelope.admin_credential_generation
        || value.signature_fingerprint != signature_fingerprint
        || value.expected_registry_generation != envelope.expected_registry_generation
        || value.expected_registry_digest != envelope.expected_registry_digest
        || value.proposed_registry_generation != envelope.proposed_registry_generation
        || value.proposed_registry_digest != envelope.proposed_registry_digest
        || value.native_authorization_descriptor_digest
            != envelope.native_authorization.descriptor_digest
        || value.native_authorization_replay_digest != native_replay_digest
        || value.authorized_at_unix != envelope.issued_at_unix
    {
        return Err(ExitError::env_config(
            "workflow broker receipt does not match its retained signed administration envelope"
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_journal(journal: &AdminJournal, paths: &BrokerPaths) -> Result<(), ExitError> {
    if journal.schema_version != ADMIN_JOURNAL_SCHEMA_VERSION
        || journal.project_id != paths.project_id
        || journal.workflow_id != paths.workflow_id
        || journal.audience != paths.audience
        || journal.receipts.len() > MAX_ADMIN_RECEIPTS
    {
        return Err(ExitError::env_config(
            "workflow broker administration journal binding or bound is invalid".to_owned(),
        ));
    }
    let mut operation_ids = BTreeSet::new();
    let mut native_admin_replays = BTreeSet::new();
    let mut previous_receipt_digest: Option<String> = None;
    let mut previous_registry_digest: Option<String> = None;
    let mut previous_generation = 0_u64;
    for applied in &journal.receipts {
        applied
            .receipt
            .validate_self_digest()
            .map_err(|error| ExitError::env_config(error.to_string()))?;
        let value = &applied.receipt.receipt;
        validate_receipt_envelope_binding(&applied.envelope, &applied.receipt)?;
        for (field, digest_value) in [
            ("receipt.operation_digest", value.operation_digest.as_str()),
            (
                "receipt.admin_public_key_fingerprint",
                value.admin_public_key_fingerprint.as_str(),
            ),
            (
                "receipt.signature_fingerprint",
                value.signature_fingerprint.as_str(),
            ),
            (
                "receipt.proposed_registry_digest",
                value.proposed_registry_digest.as_str(),
            ),
            (
                "receipt.native_authorization_descriptor_digest",
                value.native_authorization_descriptor_digest.as_str(),
            ),
            (
                "receipt.native_authorization_replay_digest",
                value.native_authorization_replay_digest.as_str(),
            ),
            ("receipt.receipt_digest", value.receipt_digest.as_str()),
        ] {
            require_digest(field, digest_value)?;
        }
        if let Some(expected) = value.expected_registry_digest.as_deref() {
            require_digest("receipt.expected_registry_digest", expected)?;
        }
        if let Some(previous) = value.previous_receipt_digest.as_deref() {
            require_digest("receipt.previous_receipt_digest", previous)?;
        }
        if value.project_id != journal.project_id
            || value.workflow_id != journal.workflow_id
            || value.audience != journal.audience
            || value.expected_registry_generation != previous_generation
            || value.expected_registry_digest != previous_registry_digest
            || value.proposed_registry_generation != previous_generation.saturating_add(1)
            || value.previous_receipt_digest != previous_receipt_digest
            || !operation_ids.insert(value.operation_id.0.clone())
            || !native_admin_replays.insert(value.native_authorization_replay_digest.clone())
        {
            return Err(ExitError::env_config(
                "workflow broker administration receipt chain invariant failed".to_owned(),
            ));
        }
        previous_generation = value.proposed_registry_generation;
        previous_registry_digest = Some(value.proposed_registry_digest.clone());
        previous_receipt_digest = Some(value.receipt_digest.clone());
    }
    if journal.registry_generation != previous_generation
        || journal.registry_digest != previous_registry_digest
        || journal.receipt_head_digest != previous_receipt_digest
        || journal.registry_digest.is_some() != journal.registry_file_digest.is_some()
    {
        return Err(ExitError::env_config(
            "workflow broker administration journal head is inconsistent".to_owned(),
        ));
    }
    if let Some(pending) = journal.pending.as_ref() {
        pending
            .receipt
            .validate_self_digest()
            .map_err(|error| ExitError::env_config(error.to_string()))?;
        validate_receipt_envelope_binding(&pending.envelope, &pending.receipt)?;
        let value = &pending.receipt.receipt;
        for (field, digest_value) in [
            (
                "pending.operation_digest",
                pending.operation_digest.as_str(),
            ),
            (
                "pending.proposed_registry_file_digest",
                pending.proposed_registry_file_digest.as_str(),
            ),
            (
                "pending.native_authorization_replay_digest",
                value.native_authorization_replay_digest.as_str(),
            ),
        ] {
            require_digest(field, digest_value)?;
        }
        if let Some(expected_file) = pending.expected_registry_file_digest.as_deref() {
            require_digest("pending.expected_registry_file_digest", expected_file)?;
        }
        let proposed_digest = workflow_broker_public_registry_digest(&pending.proposed_registry)
            .map_err(|error| ExitError::env_config(error.to_string()))?;
        if pending.operation_id != value.operation_id
            || pending.operation_digest != value.operation_digest
            || value.expected_registry_generation != journal.registry_generation
            || value.expected_registry_digest != journal.registry_digest
            || value.proposed_registry_generation != journal.registry_generation.saturating_add(1)
            || value.previous_receipt_digest != journal.receipt_head_digest
            || value.proposed_registry_digest != proposed_digest
            || pending.expected_registry_file_digest != journal.registry_file_digest
            || digest(pending.proposed_registry_yaml.as_bytes())
                != pending.proposed_registry_file_digest
            || registry_yaml(&pending.proposed_registry)? != pending.proposed_registry_yaml
            || operation_ids.contains(&pending.operation_id.0)
            || native_admin_replays.contains(&value.native_authorization_replay_digest)
        {
            return Err(ExitError::env_config(
                "pending workflow broker administration transition is inconsistent".to_owned(),
            ));
        }
        AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            pending.proposed_registry.clone(),
            &paths.audience,
            &paths.project_id,
            &paths.workflow_id,
        )
        .map_err(authority_error)?;
    }
    Ok(())
}

fn parse_journal(raw: &[u8], paths: &BrokerPaths) -> Result<AdminJournal, ExitError> {
    let journal: AdminJournal = serde_json::from_slice(raw).map_err(|error| {
        ExitError::env_config(format!(
            "strict workflow broker administration journal parse failed: {error}"
        ))
    })?;
    validate_journal(&journal, paths)?;
    Ok(journal)
}

fn read_projection(
    flags: &BTreeMap<String, Vec<String>>,
    action: &str,
    credential_id: Option<&str>,
    want_json: bool,
) -> Result<(), ExitError> {
    let paths = broker_paths(required_path(flags, "--root")?)?;
    let store = open_store(&paths)?;
    let Some(snapshot) = load_snapshot(&paths, &store)? else {
        if store.read_registry().map_err(store_error)?.is_some() {
            return Err(ExitError::env_config(
                "broker registry exists without durable administration state".to_owned(),
            ));
        }
        return emit_result(uninitialized_result(&paths, action), want_json);
    };
    let result = result_from_snapshot(&paths, action, &snapshot, None, credential_id)?;
    if credential_id.is_some() && result.credential.is_none() {
        return Err(ExitError::env_config(format!(
            "unknown broker credential '{}'",
            credential_id.unwrap_or_default()
        )));
    }
    emit_result(result, want_json)
}

fn result_from_snapshot(
    paths: &BrokerPaths,
    action: &str,
    snapshot: &AdminSnapshot,
    receipt: Option<WorkflowBrokerAdminReceiptDocument>,
    credential_id: Option<&str>,
) -> Result<BrokerResult, ExitError> {
    let credentials = snapshot
        .registry
        .credentials
        .iter()
        .map(credential_status)
        .collect::<Result<Vec<_>, _>>()?;
    let credential = credential_id
        .and_then(|id| {
            snapshot
                .registry
                .credentials
                .iter()
                .find(|credential| credential.credential_id.0 == id)
        })
        .map(credential_status)
        .transpose()?;
    let status = snapshot.control.component_status(
        snapshot.journal.receipt_head_digest.clone(),
        WorkflowBrokerRecoveryState::Clean,
    );
    Ok(BrokerResult {
        action: action.to_owned(),
        component_state: "initialized".to_owned(),
        registry_path: display(&paths.registry),
        admin_state_path: display(&paths.admin_state),
        audience: paths.audience.clone(),
        workflow_id: paths.workflow_id.0.clone(),
        registry_generation: Some(snapshot.registry.registry_generation),
        registry_digest: Some(snapshot.control.registry_digest().to_owned()),
        receipt,
        credential,
        credentials,
        component_status: Some(status),
        external_setup: selected_host_blocked_setup(),
        trust_boundary: "Forge stores strict public broker/admin metadata, canonical digests, and durable content-free receipts only; private keys and signer invocation remain outside project, sidecar, CLI, MCP, and agent-controlled process state".to_owned(),
        claim_boundary: "implemented_pending_evidence host-neutral control plane; selected-host custody, native invocation, process isolation, and field conformance remain blocked on C1.1 and stabilization".to_owned(),
    })
}

fn blocked_genesis_result(paths: &BrokerPaths) -> BrokerResult {
    let mut result = uninitialized_result(paths, "broker_genesis_blocked");
    "blocked_external_dependency".clone_into(&mut result.component_state);
    "genesis was not attempted: the proposed registry and signed administration envelope cannot supply their own trust anchor"
        .clone_into(&mut result.trust_boundary);
    "selected_host is unresolved, so no external operator trust anchor, custody, native interaction, or host evidence is available; configure a supported selected-host adapter before retrying"
        .clone_into(&mut result.claim_boundary);
    result
}

const fn selected_host_blocked_setup() -> WorkflowBrokerExternalSetupState {
    WorkflowBrokerExternalSetupState::Blocked {
        reason: WorkflowBrokerExternalSetupBlockReason::SelectedHostUnavailable,
    }
}

fn uninitialized_result(paths: &BrokerPaths, action: &str) -> BrokerResult {
    BrokerResult {
        action: action.to_owned(),
        component_state: "uninitialized".to_owned(),
        registry_path: display(&paths.registry),
        admin_state_path: display(&paths.admin_state),
        audience: paths.audience.clone(),
        workflow_id: paths.workflow_id.0.clone(),
        registry_generation: None,
        registry_digest: None,
        receipt: None,
        credential: None,
        credentials: Vec::new(),
        component_status: None,
        external_setup: selected_host_blocked_setup(),
        trust_boundary: "no broker authority is initialized; initialization requires a preconfigured external operator trust anchor resolved by a selected-host adapter, plus an externally signed native administration envelope and exact strict public registry".to_owned(),
        claim_boundary: "uninitialized state establishes no host, custody, native-origin, or conformance assurance".to_owned(),
    }
}

fn credential_status(
    credential: &WorkflowBrokerPublicCredentialMetadata,
) -> Result<BrokerCredentialStatus, ExitError> {
    Ok(BrokerCredentialStatus {
        credential_id: credential.credential_id.0.clone(),
        broker_id: credential.broker_id.0.clone(),
        subject_id: credential.subject_id.0.clone(),
        purpose: credential.purpose,
        profile: credential.profile,
        key_generation: credential.key_generation,
        status: credential.status,
        custody: credential.custody,
        host_binding: credential.host_binding.clone(),
        allowed_operations: credential.allowed_operations.clone(),
        public_key_fingerprint: workflow_broker_public_key_fingerprint(&credential.public_key_hex)
            .map_err(|error| ExitError::env_config(error.to_string()))?,
        not_before_unix: credential.not_before_unix,
        revoked_at_unix: credential.revoked_at_unix,
        predecessor_credential_id: credential
            .predecessor_credential_id
            .as_ref()
            .map(|id| id.0.clone()),
        enrollment_operation_id: credential.enrollment_operation_id.0.clone(),
        revocation_operation_id: credential
            .revocation_operation_id
            .as_ref()
            .map(|id| id.0.clone()),
    })
}

fn operation_credential_id(operation: &WorkflowBrokerAdminOperation) -> &str {
    match operation {
        WorkflowBrokerAdminOperation::Initialize {
            active_admin_credential_id,
        } => &active_admin_credential_id.0,
        WorkflowBrokerAdminOperation::Enroll { credential_id }
        | WorkflowBrokerAdminOperation::Revoke { credential_id, .. } => &credential_id.0,
        WorkflowBrokerAdminOperation::Rotate {
            replacement_credential_id,
            ..
        } => &replacement_credential_id.0,
    }
}

fn broker_paths(root: PathBuf) -> Result<BrokerPaths, ExitError> {
    let project = crate::project_cmd::resolve_project(&root)
        .map_err(|error| ExitError::env_config(format!("cannot resolve Project Link: {error}")))?;
    if !project.state_exists {
        return Err(ExitError::env_config(
            "Forge state is missing; run forge-core start before administering a workflow broker"
                .to_owned(),
        ));
    }
    let project_root = std::fs::canonicalize(project.project_root)
        .map_err(|error| ExitError::env_config(format!("canonicalize project root: {error}")))?;
    let state_root = std::fs::canonicalize(project.state_root)
        .map_err(|error| ExitError::env_config(format!("canonicalize state root: {error}")))?;
    let project_id = StableId(project.project_id.clone());
    let adapter =
        WorkflowGovernanceProjectAdapter::new(project_id.clone(), &project_root, &state_root)
            .map_err(|error| ExitError::env_config(error.to_string()))?;
    let registry = adapter.trusted_broker_registry_path();
    let operator_dir = registry
        .parent()
        .ok_or_else(|| ExitError::env_config("broker registry has no operator parent".to_owned()))?
        .to_path_buf();
    let admin_state = operator_dir.join(ADMIN_STATE_FILE);
    reject_existing_links(&operator_dir)?;
    reject_existing_links(&registry)?;
    reject_existing_links(&admin_state)?;
    let physical = physical_candidate(&operator_dir)?;
    if physical.starts_with(&project_root) || physical.starts_with(&state_root) {
        return Err(ExitError::env_config(
            "workflow broker trust store physically overlaps project or Forge state".to_owned(),
        ));
    }
    let workflow_id = StableId(WORKFLOW_ID.to_owned());
    let audience = workflow_broker_expected_audience(&project_id, &workflow_id);
    Ok(BrokerPaths {
        project_id,
        workflow_id,
        project_root,
        state_root,
        operator_dir,
        registry,
        admin_state,
        audience,
    })
}

fn open_store(paths: &BrokerPaths) -> Result<WorkflowBrokerAdminStore, ExitError> {
    WorkflowBrokerAdminStore::open(&paths.operator_dir).map_err(store_error)
}

fn acquire_backup_lock(paths: &BrokerPaths) -> Result<LockedWorkflowBrokerRegistry, ExitError> {
    reject_existing_links(&paths.operator_dir)?;
    let physical = physical_candidate(&paths.operator_dir)?;
    if physical.starts_with(&paths.project_root) || physical.starts_with(&paths.state_root) {
        return Err(ExitError::env_config(
            "workflow broker trust store physically overlaps project or Forge state".to_owned(),
        ));
    }
    let store = open_store(paths)?;
    if store.read_admin_state().map_err(store_error)?.is_some() {
        let _ = load_snapshot(paths, &store)?.ok_or_else(|| {
            ExitError::env_config(
                "workflow broker administration state disappeared under retained lock".to_owned(),
            )
        })?;
    }
    Ok(LockedWorkflowBrokerRegistry {
        store,
        expected_audience: paths.audience.clone(),
        expected_project_id: paths.project_id.clone(),
        expected_workflow_id: paths.workflow_id.clone(),
    })
}

/// Resolve and retain the same broker-registry authority used by producers.
pub(crate) fn lock_workflow_broker_registry(
    project_root: &Path,
) -> Result<LockedWorkflowBrokerRegistry, ExitError> {
    let paths = broker_paths(project_root.to_path_buf())?;
    acquire_backup_lock(&paths)
}

/// Read exact public registry bytes under the retained producer authority.
pub(crate) fn snapshot_workflow_broker_registry(
    locked: &LockedWorkflowBrokerRegistry,
) -> Result<WorkflowBrokerRegistrySnapshot, ExitError> {
    let stored = locked.store.read_registry().map_err(store_error)?;
    let raw_registry = stored.as_ref().map(|file| file.bytes().to_vec());
    if let Some(raw) = raw_registry.as_deref() {
        validate_snapshot_registry(
            raw,
            &locked.expected_audience,
            &locked.expected_project_id,
            &locked.expected_workflow_id,
        )?;
    }
    let raw_sha256 = stored.map(|file| file.raw_sha256().to_owned());
    Ok(WorkflowBrokerRegistrySnapshot {
        raw_registry,
        raw_sha256,
    })
}

fn validate_snapshot_registry(
    raw: &[u8],
    expected_audience: &str,
    expected_project_id: &StableId,
    expected_workflow_id: &StableId,
) -> Result<(), ExitError> {
    if let Ok(document) = yaml_serde::from_slice::<WorkflowBrokerPublicRegistryDocument>(raw) {
        return AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            document,
            expected_audience,
            expected_project_id,
            expected_workflow_id,
        )
        .map(|_| ())
        .map_err(authority_error);
    }
    let legacy: WorkflowBrokerRegistryDocument = yaml_serde::from_slice(raw).map_err(|error| {
        ExitError::env_config(format!(
            "strict public workflow broker registry parse failed: {error}"
        ))
    })?;
    AuthorizedWorkflowBrokerRegistry::from_document_for_audience(legacy, expected_audience)
        .map(|_| ())
        .map_err(|error| ExitError::env_config(error.to_string()))
}

fn read_public_registry(
    path: &Path,
    paths: &BrokerPaths,
    label: &str,
) -> Result<WorkflowBrokerPublicRegistryDocument, ExitError> {
    let raw = read_bytes_bounded(path, label, MAX_INPUT_BYTES)?;
    parse_strict_registry_bytes(&raw, paths)
}

fn parse_strict_registry_bytes(
    raw: &[u8],
    paths: &BrokerPaths,
) -> Result<WorkflowBrokerPublicRegistryDocument, ExitError> {
    let document: WorkflowBrokerPublicRegistryDocument =
        yaml_serde::from_slice(raw).map_err(|error| {
            ExitError::env_config(format!(
                "strict workflow broker public registry parse failed: {error}"
            ))
        })?;
    AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
        document.clone(),
        &paths.audience,
        &paths.project_id,
        &paths.workflow_id,
    )
    .map_err(authority_error)?;
    Ok(document)
}

fn registry_yaml(document: &WorkflowBrokerPublicRegistryDocument) -> Result<String, ExitError> {
    let yaml = yaml_serde::to_string(document)
        .map_err(|error| ExitError::env_config(format!("serialize broker registry: {error}")))?;
    if u64::try_from(yaml.len()).unwrap_or(u64::MAX) > MAX_WORKFLOW_BROKER_REGISTRY_BYTES {
        return Err(ExitError::env_config(
            "serialized broker registry exceeds Store bound".to_owned(),
        ));
    }
    Ok(yaml)
}

fn journal_bytes(journal: &AdminJournal) -> Result<Vec<u8>, ExitError> {
    let mut bytes = serde_json_canonicalizer::to_vec(journal).map_err(|error| {
        ExitError::env_config(format!(
            "canonical administration journal encoding failed: {error}"
        ))
    })?;
    bytes.push(b'\n');
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_WORKFLOW_BROKER_ADMIN_STATE_BYTES {
        return Err(ExitError::env_config(
            "serialized broker administration journal exceeds Store bound".to_owned(),
        ));
    }
    Ok(bytes)
}

fn read_json_bounded<T: serde::de::DeserializeOwned>(
    path: &Path,
    label: &str,
) -> Result<T, ExitError> {
    let raw = read_bytes_bounded(path, label, MAX_INPUT_BYTES)?;
    serde_json::from_slice(&raw)
        .map_err(|error| ExitError::env_config(format!("strict {label} parse failed: {error}")))
}

fn read_bytes_bounded(path: &Path, label: &str, maximum: u64) -> Result<Vec<u8>, ExitError> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        ExitError::env_config(format!("read {label} metadata {}: {error}", path.display()))
    })?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(ExitError::env_config(format!(
            "{label} {} is not a regular file or exceeds {maximum} bytes",
            path.display()
        )));
    }
    let raw = std::fs::read(path).map_err(|error| {
        ExitError::env_config(format!("read {label} {}: {error}", path.display()))
    })?;
    if u64::try_from(raw.len()).unwrap_or(u64::MAX) > maximum {
        return Err(ExitError::env_config(format!(
            "{label} {} exceeds {maximum} bytes",
            path.display()
        )));
    }
    Ok(raw)
}

fn parse_flags(action: &str, args: &[String]) -> Result<BTreeMap<String, Vec<String>>, ExitError> {
    let allowed: &[&str] = match action {
        "initialize" => &["--root", "--registry-file", "--authorization-file"],
        "register" | "rotate" | "revoke" => &[
            "--root",
            "--registry-file",
            "--authorization-file",
            "--expected-registry-digest",
        ],
        "inspect" => &["--root", "--credential-id"],
        "list" | "status" | "conformance" => &["--root"],
        _ => return Err(ExitError::usage(usage())),
    };
    let mut flags = BTreeMap::<String, Vec<String>>::new();
    let mut index = 0usize;
    while index < args.len() {
        let flag = args[index].as_str();
        if matches!(flag, "--json" | "--no-json" | "--text") {
            index += 1;
            continue;
        }
        if !allowed.contains(&flag) {
            return Err(ExitError::usage(format!(
                "unknown flag '{flag}' for workflow broker {action}"
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

fn require_digest(field: &str, value: &str) -> Result<(), ExitError> {
    if value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) {
        Ok(())
    } else {
        Err(ExitError::usage(format!(
            "{field} must be a canonical lowercase sha256:<64-hex> digest"
        )))
    }
}

fn signature_fingerprint(signature_hex: &str) -> Result<String, ExitError> {
    let bytes = decode_fixed::<64>(signature_hex).ok_or_else(|| {
        ExitError::env_config("administration signature encoding is invalid".to_owned())
    })?;
    Ok(digest(&bytes))
}

fn decode_fixed<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut bytes = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_nibble(pair[0])?;
        let low = decode_nibble(pair[1])?;
        bytes[index] = (high << 4) | low;
    }
    Some(bytes)
}

const fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn reject_existing_links(path: &Path) -> Result<(), ExitError> {
    for current in path.ancestors() {
        match std::fs::symlink_metadata(current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ExitError::env_config(format!(
                    "workflow broker path contains a symlink, junction, or reparse-point alias: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ExitError::env_config(format!(
                    "inspect workflow broker path {}: {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(())
}

fn physical_candidate(path: &Path) -> Result<PathBuf, ExitError> {
    let mut existing = path;
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| {
            ExitError::env_config(format!(
                "workflow broker path has no existing ancestor: {}",
                path.display()
            ))
        })?;
    }
    let canonical = std::fs::canonicalize(existing).map_err(|error| {
        ExitError::env_config(format!("canonicalize {}: {error}", existing.display()))
    })?;
    let suffix = path.strip_prefix(existing).map_err(|error| {
        ExitError::env_config(format!("resolve broker path {}: {error}", path.display()))
    })?;
    Ok(canonical.join(suffix))
}

fn now_i64() -> Result<i64, ExitError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| {
            ExitError::env_config(format!("system clock is before Unix epoch: {error}"))
        })?
        .as_secs();
    i64::try_from(now).map_err(|_| ExitError::env_config("system clock exceeds i64".to_owned()))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn authority_error(error: impl std::fmt::Display) -> ExitError {
    ExitError::failed(format!("workflow broker authority rejected input: {error}"))
}

fn store_error(error: WorkflowBrokerAdminStoreError) -> ExitError {
    if matches!(&error, WorkflowBrokerAdminStoreError::CompareAndSwap { .. }) {
        ExitError::conflict(error.to_string())
    } else {
        ExitError::env_config(error.to_string())
    }
}

fn not_initialized() -> ExitError {
    ExitError::env_config(
        "workflow broker is not initialized; install a strict generation-one public registry with an externally signed native administration envelope"
            .to_owned(),
    )
}

fn display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn emit_result(result: BrokerResult, want_json: bool) -> Result<(), ExitError> {
    emit_envelope(CliEnvelope::ok("workflow.broker", result), want_json)
}

fn usage() -> String {
    format!(
        "usage:\n  forge-core {COMMAND} initialize --root <project> --registry-file <strict-registry.yaml> --authorization-file <signed-native-admin.json> [--json|--no-json]\n  forge-core {COMMAND} register --root <project> --registry-file <proposed-registry.yaml> --authorization-file <signed-native-admin.json> --expected-registry-digest <sha256:...> [--json|--no-json]\n  forge-core {COMMAND} rotate --root <project> --registry-file <proposed-registry.yaml> --authorization-file <signed-native-admin.json> --expected-registry-digest <sha256:...> [--json|--no-json]\n  forge-core {COMMAND} revoke --root <project> --registry-file <proposed-registry.yaml> --authorization-file <signed-native-admin.json> --expected-registry-digest <sha256:...> [--json|--no-json]\n  forge-core {COMMAND} list --root <project> [--json|--no-json]\n  forge-core {COMMAND} inspect --root <project> --credential-id <id> [--json|--no-json]\n  forge-core {COMMAND} status --root <project> [--json|--no-json]\n  forge-core {COMMAND} conformance --root <project> [--json|--no-json]\n\ninitialize is blocked until a selected-host adapter provides a preconfigured external operator trust anchor; registry or envelope keys cannot bootstrap themselves"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use forge_core_contracts::{
        workflow_broker_admin_operation_signing_bytes,
        workflow_broker_native_admin_descriptor_digest, RuntimeKind, WorkflowBrokerBoundOperation,
        WorkflowBrokerCredentialProfile, WorkflowBrokerCredentialPurpose,
        WorkflowBrokerCustodyKind, WorkflowBrokerHostBinding,
        WorkflowBrokerNativeAdminAuthorization, WorkflowBrokerPublicKeyAlgorithm,
        WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION,
        WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION,
        WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION,
    };

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut value = String::new();
        for byte in bytes {
            let _ = write!(value, "{byte:02x}");
        }
        value
    }

    fn host() -> WorkflowBrokerHostBinding {
        WorkflowBrokerHostBinding {
            host_kind: RuntimeKind::Custom,
            host_version: "1.0.0".to_owned(),
            adapter_id: StableId("adapter.test".to_owned()),
            adapter_version: "1.0.0".to_owned(),
            host_installation_id: StableId("host.installation.test".to_owned()),
            protocol_version: "workflow-host-origin-v1".to_owned(),
        }
    }

    struct TestGenesisTrustAnchor {
        anchor_id: StableId,
        operator_subject_id: StableId,
        public_key_hex: String,
        host_binding: WorkflowBrokerHostBinding,
    }

    impl forge_core_authority::workflow_broker_control::WorkflowBrokerGenesisTrustAnchor
        for TestGenesisTrustAnchor
    {
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
            anchor_id: StableId("operator.anchor.test".to_owned()),
            operator_subject_id: StableId("administrator.test".to_owned()),
            public_key_hex: hex(key.verifying_key().as_bytes()),
            host_binding: host(),
        }
    }

    fn credential(
        id: &str,
        subject: &str,
        key: &SigningKey,
        purpose: WorkflowBrokerCredentialPurpose,
    ) -> WorkflowBrokerPublicCredentialMetadata {
        WorkflowBrokerPublicCredentialMetadata {
            credential_id: StableId(id.to_owned()),
            broker_id: StableId(format!("broker.{id}")),
            subject_id: StableId(subject.to_owned()),
            purpose,
            profile: if purpose == WorkflowBrokerCredentialPurpose::RegistryAdministrator {
                WorkflowBrokerCredentialProfile::Administrator
            } else {
                WorkflowBrokerCredentialProfile::Human
            },
            algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
            public_key_hex: hex(key.verifying_key().as_bytes()),
            key_generation: 1,
            status: WorkflowBrokerCredentialStatus::Active,
            custody: WorkflowBrokerCustodyKind::RemoteSignerNonExportable,
            host_binding: host(),
            allowed_operations: if purpose == WorkflowBrokerCredentialPurpose::RegistryAdministrator
            {
                Vec::new()
            } else {
                vec![WorkflowBrokerBoundOperation::Decision]
            },
            not_before_unix: 1_900_000_000,
            revoked_at_unix: None,
            predecessor_credential_id: None,
            enrollment_operation_id: StableId("admin.operation.initialize".to_owned()),
            revocation_operation_id: None,
        }
    }

    fn genesis_registry(
        admin: &SigningKey,
        event: &SigningKey,
    ) -> WorkflowBrokerPublicRegistryDocument {
        let mut credentials = vec![
            credential(
                "credential.admin.1",
                "administrator.test",
                admin,
                WorkflowBrokerCredentialPurpose::RegistryAdministrator,
            ),
            credential(
                "credential.event.1",
                "issuer.human.test",
                event,
                WorkflowBrokerCredentialPurpose::EventIssuer,
            ),
        ];
        credentials.sort_by(|left, right| left.credential_id.0.cmp(&right.credential_id.0));
        WorkflowBrokerPublicRegistryDocument {
            schema_version: WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION.to_owned(),
            audience: "forge-core:workflow:project.test".to_owned(),
            project_id: StableId("project.test".to_owned()),
            workflow_id: StableId(WORKFLOW_ID.to_owned()),
            registry_generation: 1,
            previous_registry_digest: None,
            required_event_schema_version: WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION.to_owned(),
            credentials,
        }
    }

    fn genesis_envelope(
        admin: &SigningKey,
        registry: &WorkflowBrokerPublicRegistryDocument,
    ) -> WorkflowBrokerAdminOperationEnvelope {
        let mut envelope = WorkflowBrokerAdminOperationEnvelope {
            schema_version: WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION.to_owned(),
            audience: registry.audience.clone(),
            project_id: registry.project_id.clone(),
            workflow_id: registry.workflow_id.clone(),
            operation_id: StableId("admin.operation.initialize".to_owned()),
            admin_credential_id: StableId("credential.admin.1".to_owned()),
            admin_credential_generation: 1,
            expected_registry_generation: 0,
            expected_registry_digest: None,
            proposed_registry_generation: 1,
            proposed_registry_digest: workflow_broker_public_registry_digest(registry).unwrap(),
            operation: WorkflowBrokerAdminOperation::Initialize {
                active_admin_credential_id: StableId("credential.admin.1".to_owned()),
            },
            native_authorization: WorkflowBrokerNativeAdminAuthorization {
                host_kind: RuntimeKind::Custom,
                host_version: "1.0.0".to_owned(),
                adapter_id: StableId("adapter.test".to_owned()),
                adapter_version: "1.0.0".to_owned(),
                host_installation_id: StableId("host.installation.test".to_owned()),
                protocol_version: "workflow-host-origin-v1".to_owned(),
                admin_session_ref: "admin-session-reference-0001".to_owned(),
                admin_interaction_ref: "admin-interaction-ref-0001".to_owned(),
                observed_at_unix: 1_900_000_000,
                descriptor_digest: digest(b"placeholder"),
            },
            issued_at_unix: 1_900_000_000,
            expires_at_unix: 1_900_000_300,
            nonce: "admin-initialize-nonce-0001".to_owned(),
            signature: String::new(),
        };
        envelope.native_authorization.descriptor_digest =
            workflow_broker_native_admin_descriptor_digest(&envelope).unwrap();
        envelope.signature = hex(&admin
            .sign(&workflow_broker_admin_operation_signing_bytes(&envelope).unwrap())
            .to_bytes());
        envelope
    }

    #[test]
    fn genesis_authority_and_journal_are_strict_and_secret_free() {
        let admin = SigningKey::from_bytes(&[7_u8; 32]);
        let event = SigningKey::from_bytes(&[8_u8; 32]);
        let registry = genesis_registry(&admin, &event);
        let envelope = genesis_envelope(&admin, &registry);
        let trust_anchor = genesis_trust_anchor(&admin);
        let advance = AuthorizedWorkflowBrokerControlPlane::authorize_genesis(
            &trust_anchor,
            envelope.clone(),
            registry,
            1_900_000_001,
        )
        .expect("authorized genesis");
        let (control, receipt) = advance.into_parts();
        let paths = BrokerPaths {
            project_id: StableId("project.test".to_owned()),
            workflow_id: StableId(WORKFLOW_ID.to_owned()),
            project_root: PathBuf::from("/project"),
            state_root: PathBuf::from("/state"),
            operator_dir: PathBuf::from("/operator"),
            registry: PathBuf::from("/operator/workflow-broker-registry.yaml"),
            admin_state: PathBuf::from("/operator/workflow-broker-admin.json"),
            audience: "forge-core:workflow:project.test".to_owned(),
        };
        let journal = AdminJournal {
            schema_version: ADMIN_JOURNAL_SCHEMA_VERSION.to_owned(),
            project_id: paths.project_id.clone(),
            workflow_id: paths.workflow_id.clone(),
            audience: paths.audience.clone(),
            registry_generation: 0,
            registry_digest: None,
            registry_file_digest: None,
            receipt_head_digest: None,
            receipts: Vec::new(),
            pending: Some(PendingAdminTransition {
                operation_id: receipt.receipt.operation_id.clone(),
                operation_digest: receipt.receipt.operation_digest.clone(),
                expected_registry_file_digest: None,
                proposed_registry: control.document().clone(),
                proposed_registry_yaml: registry_yaml(control.document()).unwrap(),
                proposed_registry_file_digest: digest(
                    registry_yaml(control.document()).unwrap().as_bytes(),
                ),
                envelope,
                receipt,
            }),
        };
        validate_journal(&journal, &paths).expect("valid pending journal");
        let serialized = String::from_utf8(journal_bytes(&journal).unwrap()).unwrap();
        for forbidden in ["private_key", "secret_key", "signing_key", "key_handle"] {
            assert!(!serialized.contains(forbidden), "unexpected {forbidden}");
        }
    }

    #[test]
    fn administration_journal_rejects_native_interaction_reuse() {
        let admin = SigningKey::from_bytes(&[7_u8; 32]);
        let event = SigningKey::from_bytes(&[8_u8; 32]);
        let registry = genesis_registry(&admin, &event);
        let envelope = genesis_envelope(&admin, &registry);
        let trust_anchor = genesis_trust_anchor(&admin);
        let advance = AuthorizedWorkflowBrokerControlPlane::authorize_genesis(
            &trust_anchor,
            envelope.clone(),
            registry,
            1_900_000_001,
        )
        .expect("authorized genesis");
        let first = advance.receipt().clone();
        let mut reused_envelope = envelope.clone();
        reused_envelope.operation_id = StableId("admin.operation.reused".to_owned());
        reused_envelope.expected_registry_generation = 1;
        reused_envelope.expected_registry_digest =
            Some(first.receipt.proposed_registry_digest.clone());
        reused_envelope.proposed_registry_generation = 2;
        reused_envelope.proposed_registry_digest = digest(b"second registry");
        reused_envelope.operation = WorkflowBrokerAdminOperation::Enroll {
            credential_id: StableId("credential.event.reused".to_owned()),
        };
        reused_envelope.native_authorization.descriptor_digest =
            workflow_broker_native_admin_descriptor_digest(&reused_envelope)
                .expect("reused descriptor");
        reused_envelope.signature = hex(&admin
            .sign(
                &workflow_broker_admin_operation_signing_bytes(&reused_envelope)
                    .expect("reused signing bytes"),
            )
            .to_bytes());
        let mut reused = first.clone();
        reused.receipt.operation_id = reused_envelope.operation_id.clone();
        reused.receipt.operation_digest = workflow_broker_admin_operation_digest(&reused_envelope)
            .expect("reused operation digest");
        reused.receipt.signature_fingerprint =
            signature_fingerprint(&reused_envelope.signature).expect("signature fingerprint");
        reused.receipt.expected_registry_generation = 1;
        reused.receipt.expected_registry_digest = reused_envelope.expected_registry_digest.clone();
        reused.receipt.proposed_registry_generation = 2;
        reused.receipt.proposed_registry_digest = reused_envelope.proposed_registry_digest.clone();
        reused.receipt.native_authorization_descriptor_digest = reused_envelope
            .native_authorization
            .descriptor_digest
            .clone();
        reused.receipt.previous_receipt_digest = Some(first.receipt.receipt_digest.clone());
        reused.receipt.receipt_digest = reused.digest().expect("reused receipt digest");
        let paths = BrokerPaths {
            project_id: StableId("project.test".to_owned()),
            workflow_id: StableId(WORKFLOW_ID.to_owned()),
            project_root: PathBuf::from("/project"),
            state_root: PathBuf::from("/state"),
            operator_dir: PathBuf::from("/operator"),
            registry: PathBuf::from("/operator/workflow-broker-registry.yaml"),
            admin_state: PathBuf::from("/operator/workflow-broker-admin.json"),
            audience: "forge-core:workflow:project.test".to_owned(),
        };
        let journal = AdminJournal {
            schema_version: ADMIN_JOURNAL_SCHEMA_VERSION.to_owned(),
            project_id: paths.project_id.clone(),
            workflow_id: paths.workflow_id.clone(),
            audience: paths.audience.clone(),
            registry_generation: 2,
            registry_digest: Some(reused.receipt.proposed_registry_digest.clone()),
            registry_file_digest: Some(digest(b"registry file")),
            receipt_head_digest: Some(reused.receipt.receipt_digest.clone()),
            receipts: vec![
                AppliedAdminOperation {
                    envelope,
                    receipt: first,
                },
                AppliedAdminOperation {
                    envelope: reused_envelope,
                    receipt: reused,
                },
            ],
            pending: None,
        };
        assert!(validate_journal(&journal, &paths).is_err());
    }

    #[test]
    fn pending_genesis_fails_closed_without_selected_host_trust_anchor() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "forge-broker-strict-recovery-{}-{nonce}",
            std::process::id()
        ));
        let paths = BrokerPaths {
            project_id: StableId("project.test".to_owned()),
            workflow_id: StableId(WORKFLOW_ID.to_owned()),
            project_root: base.join("project"),
            state_root: base.join("state"),
            operator_dir: base.join("operator"),
            registry: base.join("operator/workflow-broker-registry.yaml"),
            admin_state: base.join("operator/workflow-broker-admin.json"),
            audience: "forge-core:workflow:project.test".to_owned(),
        };
        std::fs::create_dir_all(&paths.project_root).expect("project");
        std::fs::create_dir_all(&paths.state_root).expect("state");
        std::fs::create_dir_all(&paths.operator_dir).expect("operator");
        let admin = SigningKey::from_bytes(&[7_u8; 32]);
        let event = SigningKey::from_bytes(&[8_u8; 32]);
        let registry = genesis_registry(&admin, &event);
        let envelope = genesis_envelope(&admin, &registry);
        let trust_anchor = genesis_trust_anchor(&admin);
        let advance = AuthorizedWorkflowBrokerControlPlane::authorize_genesis(
            &trust_anchor,
            envelope.clone(),
            registry,
            1_900_000_001,
        )
        .expect("authorized genesis");
        let (control, receipt) = advance.into_parts();
        let proposed_registry_yaml = registry_yaml(control.document()).expect("registry YAML");
        let journal = AdminJournal {
            schema_version: ADMIN_JOURNAL_SCHEMA_VERSION.to_owned(),
            project_id: paths.project_id.clone(),
            workflow_id: paths.workflow_id.clone(),
            audience: paths.audience.clone(),
            registry_generation: 0,
            registry_digest: None,
            registry_file_digest: None,
            receipt_head_digest: None,
            receipts: Vec::new(),
            pending: Some(PendingAdminTransition {
                operation_id: receipt.receipt.operation_id.clone(),
                operation_digest: receipt.receipt.operation_digest.clone(),
                expected_registry_file_digest: None,
                proposed_registry: control.document().clone(),
                proposed_registry_file_digest: digest(proposed_registry_yaml.as_bytes()),
                proposed_registry_yaml: proposed_registry_yaml.clone(),
                envelope: envelope.clone(),
                receipt: receipt.clone(),
            }),
        };
        let store = open_store(&paths).expect("store");
        store
            .replace_admin_state(None, &journal_bytes(&journal).expect("journal"))
            .expect("prepare journal");
        let error = load_snapshot(&paths, &store).expect_err("selected-host anchor is absent");
        assert!(error
            .to_string()
            .contains("blocked_external_dependency[selected_host_unavailable]"));
        assert!(store.read_registry().expect("registry read").is_none());
        let retained = store
            .read_admin_state()
            .expect("admin state read")
            .expect("prepared state remains");
        let retained_journal = parse_journal(retained.bytes(), &paths).expect("retained journal");
        assert_eq!(retained_journal.registry_generation, 0);
        assert!(retained_journal.receipts.is_empty());
        assert!(retained_journal.pending.is_some());

        let installed = store
            .replace_registry(None, proposed_registry_yaml.as_bytes())
            .expect("preinstall proposed registry");
        let installed_digest = installed.raw_sha256().to_owned();
        drop(installed);
        let error = load_snapshot(&paths, &store)
            .expect_err("preinstalled proposal is not an external genesis trust anchor");
        assert!(error
            .to_string()
            .contains("blocked_external_dependency[selected_host_unavailable]"));
        let retained_registry = store
            .read_registry()
            .expect("registry read")
            .expect("preinstalled registry remains");
        assert_eq!(retained_registry.raw_sha256(), installed_digest.as_str());
        let retained = store
            .read_admin_state()
            .expect("admin state read")
            .expect("prepared state remains");
        let retained_journal = parse_journal(retained.bytes(), &paths).expect("retained journal");
        assert_eq!(retained_journal.registry_generation, 0);
        assert!(retained_journal.receipts.is_empty());
        assert!(retained_journal.pending.is_some());
        drop(store);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn strict_setup_argv_round_trips_through_the_product_parser() {
        let initialize = [
            "--root",
            "/project",
            "--registry-file",
            "/operator/genesis-registry.yaml",
            "--authorization-file",
            "/operator/genesis-authorization.json",
            "--json",
        ]
        .map(str::to_owned);
        let initialize_flags =
            parse_flags("initialize", &initialize).expect("strict initialize argv");
        assert_eq!(required(&initialize_flags, "--root").unwrap(), "/project");
        assert_eq!(
            required(&initialize_flags, "--registry-file").unwrap(),
            "/operator/genesis-registry.yaml"
        );
        assert_eq!(
            required(&initialize_flags, "--authorization-file").unwrap(),
            "/operator/genesis-authorization.json"
        );

        let register = [
            "--root",
            "/project",
            "--registry-file",
            "/operator/proposed-registry.yaml",
            "--authorization-file",
            "/operator/register-authorization.json",
            "--expected-registry-digest",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--json",
        ]
        .map(str::to_owned);
        let register_flags = parse_flags("register", &register).expect("strict register argv");
        assert_eq!(required(&register_flags, "--root").unwrap(), "/project");
        assert_eq!(
            required(&register_flags, "--expected-registry-digest").unwrap(),
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }

    #[test]
    fn legacy_setup_argv_is_rejected_and_blocked_state_is_typed() {
        let legacy = [
            "--root",
            "/project",
            "--issuer-id",
            "broker.runtime",
            "--profile",
            "runtime",
            "--public-key-file",
            "/tmp/key.pub",
            "--ceremony-ref",
            "ceremony.1",
            "--ceremony-file",
            "/tmp/ceremony.json",
        ]
        .map(str::to_owned);
        assert!(parse_flags("trust", &legacy).is_err());
        assert!(parse_flags("register", &legacy).is_err());

        let paths = BrokerPaths {
            project_id: StableId("project.test".to_owned()),
            workflow_id: StableId(WORKFLOW_ID.to_owned()),
            project_root: PathBuf::from("/project"),
            state_root: PathBuf::from("/state"),
            operator_dir: PathBuf::from("/operator"),
            registry: PathBuf::from("/operator/workflow-broker-registry.yaml"),
            admin_state: PathBuf::from("/operator/workflow-broker-admin.json"),
            audience: "forge-core:workflow:project.test".to_owned(),
        };
        let blocked = blocked_genesis_result(&paths);
        assert_eq!(blocked.component_state, "blocked_external_dependency");
        assert_eq!(blocked.external_setup, selected_host_blocked_setup());
        assert!(blocked.receipt.is_none());
        assert!(blocked.credentials.is_empty());
    }

    #[test]
    fn strict_inputs_reject_signing_oracle_fields_and_operation_confusion() {
        let admin = SigningKey::from_bytes(&[7_u8; 32]);
        let event = SigningKey::from_bytes(&[8_u8; 32]);
        let registry = genesis_registry(&admin, &event);
        let envelope = genesis_envelope(&admin, &registry);
        let mut value = serde_json::to_value(envelope).unwrap();
        value["packet_digest"] = serde_json::json!(digest(b"packet"));
        value["arbitrary_json"] = serde_json::json!({"sign": true});
        assert!(serde_json::from_value::<WorkflowBrokerAdminOperationEnvelope>(value).is_err());
        assert!(
            !TransitionCommand::Rotate.matches(&WorkflowBrokerAdminOperation::Enroll {
                credential_id: StableId("credential.event.2".to_owned()),
            })
        );
    }
}
