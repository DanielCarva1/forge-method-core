use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    workflow_broker_control::WorkflowBrokerGenesisTrustAnchor, AuthorizedWorkflowBrokerControlPlane,
};
use forge_core_contracts::{
    workflow_broker_admin_operation_signing_bytes, workflow_broker_native_admin_descriptor_digest,
    workflow_broker_public_registry_digest, StableId, WorkflowBrokerAdminOperation,
    WorkflowBrokerAdminOperationEnvelope, WorkflowBrokerAdminReceiptDocument,
    WorkflowBrokerCredentialPurpose, WorkflowBrokerHostBinding,
    WorkflowBrokerNativeAdminAuthorization, WorkflowBrokerPublicRegistryDocument,
    WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const ADMIN_JOURNAL_SCHEMA_VERSION: &str = "0.1";

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

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct AppliedAdminOperation {
    envelope: WorkflowBrokerAdminOperationEnvelope,
    receipt: WorkflowBrokerAdminReceiptDocument,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct AdminJournal {
    schema_version: String,
    project_id: StableId,
    workflow_id: StableId,
    audience: String,
    registry_generation: u64,
    registry_digest: String,
    registry_file_digest: String,
    receipt_head_digest: String,
    receipts: Vec<AppliedAdminOperation>,
}

/// Installs a selected-host-anchored strict broker fixture with a durable genesis journal.
///
/// # Panics
///
/// Panics if the fixture registry lacks one consistent administrator enrollment, if
/// genesis authorization fails, or if the public registry and journal cannot be written.
pub fn install_strict_broker_genesis(
    operator_dir: &Path,
    registry: WorkflowBrokerPublicRegistryDocument,
    admin_key: &SigningKey,
) {
    let admin = registry
        .credentials
        .iter()
        .find(|credential| {
            credential.purpose == WorkflowBrokerCredentialPurpose::RegistryAdministrator
        })
        .expect("strict registry administrator");
    assert!(registry
        .credentials
        .iter()
        .all(|credential| { credential.enrollment_operation_id == admin.enrollment_operation_id }));

    let issued_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let mut envelope = WorkflowBrokerAdminOperationEnvelope {
        schema_version: WORKFLOW_BROKER_ADMIN_OPERATION_SCHEMA_VERSION.to_owned(),
        audience: registry.audience.clone(),
        project_id: registry.project_id.clone(),
        workflow_id: registry.workflow_id.clone(),
        operation_id: admin.enrollment_operation_id.clone(),
        admin_credential_id: admin.credential_id.clone(),
        admin_credential_generation: admin.key_generation,
        expected_registry_generation: 0,
        expected_registry_digest: None,
        proposed_registry_generation: 1,
        proposed_registry_digest: workflow_broker_public_registry_digest(&registry)
            .expect("strict registry digest"),
        operation: WorkflowBrokerAdminOperation::Initialize {
            active_admin_credential_id: admin.credential_id.clone(),
        },
        native_authorization: WorkflowBrokerNativeAdminAuthorization {
            host_kind: admin.host_binding.host_kind,
            host_version: admin.host_binding.host_version.clone(),
            adapter_id: admin.host_binding.adapter_id.clone(),
            adapter_version: admin.host_binding.adapter_version.clone(),
            host_installation_id: admin.host_binding.host_installation_id.clone(),
            protocol_version: admin.host_binding.protocol_version.clone(),
            admin_session_ref: "test-selected-host-admin-session-0001".to_owned(),
            admin_interaction_ref: "test-selected-host-admin-interaction-0001".to_owned(),
            observed_at_unix: issued_at_unix,
            descriptor_digest: format!("sha256:{}", "0".repeat(64)),
        },
        issued_at_unix,
        expires_at_unix: issued_at_unix + 300,
        nonce: "test-selected-host-admin-genesis-nonce-0001".to_owned(),
        signature: String::new(),
    };
    envelope.native_authorization.descriptor_digest =
        workflow_broker_native_admin_descriptor_digest(&envelope).expect("admin descriptor digest");
    envelope.signature = hex(&admin_key
        .sign(
            &workflow_broker_admin_operation_signing_bytes(&envelope)
                .expect("admin operation signing bytes"),
        )
        .to_bytes());

    let trust_anchor = TestGenesisTrustAnchor {
        anchor_id: StableId("operator.anchor.selected-host-test".to_owned()),
        operator_subject_id: admin.subject_id.clone(),
        public_key_hex: hex(admin_key.verifying_key().as_bytes()),
        host_binding: admin.host_binding.clone(),
    };
    let advance = AuthorizedWorkflowBrokerControlPlane::authorize_genesis(
        &trust_anchor,
        envelope.clone(),
        registry,
        i64::try_from(issued_at_unix).expect("test clock fits i64"),
    )
    .expect("selected-host anchored broker genesis");
    let (control, receipt) = advance.into_parts();
    let registry_yaml =
        yaml_serde::to_string(control.document()).expect("strict broker registry YAML");
    let registry_file_digest = digest(registry_yaml.as_bytes());
    let journal = AdminJournal {
        schema_version: ADMIN_JOURNAL_SCHEMA_VERSION.to_owned(),
        project_id: control.document().project_id.clone(),
        workflow_id: control.document().workflow_id.clone(),
        audience: control.document().audience.clone(),
        registry_generation: control.document().registry_generation,
        registry_digest: control.registry_digest().to_owned(),
        registry_file_digest,
        receipt_head_digest: receipt.receipt.receipt_digest.clone(),
        receipts: vec![AppliedAdminOperation { envelope, receipt }],
    };
    let mut journal_bytes =
        serde_json_canonicalizer::to_vec(&journal).expect("canonical broker admin journal");
    journal_bytes.push(b'\n');

    fs::create_dir_all(operator_dir).expect("broker operator directory");
    fs::write(
        operator_dir.join("workflow-broker-registry.yaml"),
        registry_yaml,
    )
    .expect("preconfigured external broker registry");
    fs::write(
        operator_dir.join("workflow-broker-admin.json"),
        journal_bytes,
    )
    .expect("preconfigured broker administration journal");
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
