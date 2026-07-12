use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    AttestationInput, AttestationPolicy, AttestationVerifier, AuthorizedPrincipalRegistry,
    CanonicalIntent, PrincipalCredentialStatus, PrincipalRegistryContract,
    PrincipalRegistryDocument, PrincipalRegistryEntry, VerifiedWorkflowApplicabilityAuthorization,
    VerifiedWorkflowCapabilityAuthorization, VerifiedWorkflowDecisionAuthorization,
    VerifiedWorkflowEvidenceAuthorization, VerifiedWorkflowSignalAuthorization,
    VerifiedWorkflowWaiverAuthorization, WorkflowApplicabilityAuthorizationRequest,
    WorkflowAuthorityError, WorkflowCapabilityAuthorizationRequest,
    WorkflowDecisionAuthorizationRequest, WorkflowEvidenceAuthorizationRequest,
    WorkflowSignalAuthorizationRequest, WorkflowWaiverAuthorizationRequest, WorkflowWaiverSubject,
    PRINCIPAL_REGISTRY_SCHEMA_VERSION,
};
use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::{
    NextActionKind, PrincipalId, ReadinessTarget, ReceiptRevokedEvent, StableId,
    WorkflowContentAddressedReference, WorkflowEvaluatorProvider, WorkflowEvidenceKind,
    WorkflowEvidenceOutcome, WorkflowEvidenceStrength, WorkflowEvidenceSubjectKind,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceEvent, WorkflowGovernancePolicy,
    WorkflowGovernanceSignal, WorkflowReceiptCarryover,
};
use forge_core_decisions::WorkflowClaimResultStatus;
use forge_core_kernel::{
    WorkflowGovernanceAdapterError, WorkflowGovernanceGuidance, WorkflowGovernanceGuidanceStatus,
    WorkflowGovernanceProjectAdapter, WorkflowGovernanceReleasePinOrigin,
    WorkflowGovernanceReleaseUpgradeStatus,
};
use forge_core_store::sha256_content_hash;
use forge_core_workflow_governance_tcb::{
    append_workflow_governance_event_tcb, recover_workflow_governance_ledger,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIENCE: &str = "forge-core:workflow:signed-golden-path";
const HUMAN_CREDENTIAL: &str = "credential.workflow.human";
const WORKER_CREDENTIAL: &str = "credential.workflow.worker";
const RUNTIME_CREDENTIAL: &str = "credential.workflow.runtime";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bundle() -> WorkflowGovernanceBundleDocument {
    yaml_serde::from_str(
        &fs::read_to_string(repo_root().join("contracts/workflow-governance/golden-path-v0.yaml"))
            .expect("golden bundle"),
    )
    .expect("typed golden bundle")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
            output
        },
    )
}

fn readiness_name(target: ReadinessTarget) -> String {
    match target {
        ReadinessTarget::Explore => "explore",
        ReadinessTarget::Execute => "execute",
        ReadinessTarget::Release => "release",
    }
    .to_owned()
}

struct SignedFixture {
    project_id: StableId,
    root: PathBuf,
    operator_registry_path: PathBuf,
    adapter: WorkflowGovernanceProjectAdapter,
    registry: AuthorizedPrincipalRegistry,
    human_key: SigningKey,
    worker_key: SigningKey,
    runtime_key: SigningKey,
}

impl SignedFixture {
    fn new(label: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "forge-p5c-{label}-{}-{}",
            std::process::id(),
            sequence
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".forge-method")).expect("state");
        fs::write(root.join("README.md"), "governed consumer\n").expect("basis");
        let root = root.canonicalize().expect("canonical project");
        let state = root.join(".forge-method");
        let human_key = SigningKey::from_bytes(&[71; 32]);
        let worker_key = SigningKey::from_bytes(&[72; 32]);
        let runtime_key = SigningKey::from_bytes(&[73; 32]);
        let document = PrincipalRegistryDocument {
            schema_version: PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
            principal_registry: PrincipalRegistryContract {
                audience: AUDIENCE.to_owned(),
                principals: vec![
                    principal(
                        HUMAN_CREDENTIAL,
                        "principal.workflow.human",
                        "agent.workflow.human-console",
                        CallerRole::Human,
                        &human_key,
                        &[
                            "workflow.applicability.assess",
                            "workflow.decision.resolve",
                            "workflow.waiver.authorize",
                            "workflow.evidence.authorize_human",
                        ],
                    ),
                    principal(
                        WORKER_CREDENTIAL,
                        "principal.workflow.reviewer",
                        "agent.workflow.reviewer",
                        CallerRole::Worker,
                        &worker_key,
                        &[
                            "workflow.evidence.authorize_review",
                            "workflow.evidence.authorize_external",
                        ],
                    ),
                    principal(
                        RUNTIME_CREDENTIAL,
                        "principal.workflow.runtime",
                        "agent.workflow.runtime",
                        CallerRole::Runtime,
                        &runtime_key,
                        &[
                            "workflow.capability.authorize",
                            "workflow.signal.authorize",
                            "workflow.evidence.authorize_runtime",
                            "workflow.evidence.authorize_external",
                        ],
                    ),
                ],
            },
        };
        let registry_yaml = yaml_serde::to_string(&document).expect("registry YAML");
        let registry = AuthorizedPrincipalRegistry::from_document(document).expect("registry");
        let project_id = StableId(format!("project.signed-golden-path.{sequence}"));
        let adapter = WorkflowGovernanceProjectAdapter::new(project_id.clone(), &root, &state)
            .expect("adapter");
        let operator_registry_path = adapter.trusted_principal_registry_path();
        fs::create_dir_all(
            operator_registry_path
                .parent()
                .expect("operator registry parent"),
        )
        .expect("operator trust root");
        fs::write(&operator_registry_path, registry_yaml).expect("fixed registry");
        adapter.initialize().expect("initialize");
        Self {
            project_id,
            root,
            operator_registry_path,
            adapter,
            registry,
            human_key,
            worker_key,
            runtime_key,
        }
    }

    fn key(&self, credential: &str) -> &SigningKey {
        match credential {
            HUMAN_CREDENTIAL => &self.human_key,
            WORKER_CREDENTIAL => &self.worker_key,
            RUNTIME_CREDENTIAL => &self.runtime_key,
            _ => panic!("unknown test credential"),
        }
    }

    fn attestation<T: Serialize>(
        &self,
        credential: &str,
        action: &str,
        request: &T,
    ) -> AttestationInput {
        static NONCE: AtomicU64 = AtomicU64::new(0);
        let issued = i64::try_from(now()).expect("i64 clock");
        let key = self.key(credential);
        let mut attestation = AttestationInput {
            credential_id: Some(credential.to_owned()),
            audience: Some(AUDIENCE.to_owned()),
            execution_intent_digest: None,
            nonce: format!(
                "signed-golden-{action}-{issued}-{}",
                NONCE.fetch_add(1, Ordering::SeqCst)
            ),
            ts: issued,
            signature: String::new(),
            public_key_hex: hex(&key.verifying_key().to_bytes()),
        };
        let intent = CanonicalIntent {
            tool: "workflow".to_owned(),
            arguments: serde_json::json!({
                "action": action,
                "request": serde_json::to_value(request).expect("request JSON"),
            }),
            credential_id: attestation.credential_id.clone(),
            audience: attestation.audience.clone(),
            execution_intent_digest: None,
            nonce: attestation.nonce.clone(),
            ts: attestation.ts,
        };
        attestation.signature = hex(&key
            .sign(&intent.canonical_bytes().expect("canonical intent"))
            .to_bytes());
        attestation
    }

    fn applicability(
        &self,
        request: WorkflowApplicabilityAuthorizationRequest,
    ) -> VerifiedWorkflowApplicabilityAuthorization {
        let attestation = self.attestation(HUMAN_CREDENTIAL, "applicability_assess", &request);
        self.registry
            .authorize_workflow_applicability(
                &AttestationVerifier::new(AttestationPolicy::Default),
                request,
                &attestation,
            )
            .expect("authorized applicability")
    }

    fn capability(
        &self,
        request: WorkflowCapabilityAuthorizationRequest,
    ) -> VerifiedWorkflowCapabilityAuthorization {
        let attestation = self.attestation(RUNTIME_CREDENTIAL, "capability_authorize", &request);
        self.registry
            .authorize_workflow_capability(
                &AttestationVerifier::new(AttestationPolicy::Default),
                request,
                &attestation,
            )
            .expect("authorized capability")
    }

    fn evidence(
        &self,
        request: WorkflowEvidenceAuthorizationRequest,
    ) -> VerifiedWorkflowEvidenceAuthorization {
        let credential = match request.provider {
            WorkflowEvaluatorProvider::AuthorizedHuman => HUMAN_CREDENTIAL,
            WorkflowEvaluatorProvider::IndependentReviewer
            | WorkflowEvaluatorProvider::ExternalAuthority
            | WorkflowEvaluatorProvider::ResearchSource => WORKER_CREDENTIAL,
            WorkflowEvaluatorProvider::RepositoryInspector
            | WorkflowEvaluatorProvider::DeterministicTool
            | WorkflowEvaluatorProvider::RepresentativeRuntime => RUNTIME_CREDENTIAL,
        };
        let attestation = self.attestation(credential, "evidence_authorize", &request);
        self.registry
            .authorize_workflow_evidence(
                &AttestationVerifier::new(AttestationPolicy::Default),
                request,
                &attestation,
            )
            .expect("authorized evidence")
    }

    fn decision(
        &self,
        request: WorkflowDecisionAuthorizationRequest,
    ) -> VerifiedWorkflowDecisionAuthorization {
        let attestation = self.attestation(HUMAN_CREDENTIAL, "decision_resolve", &request);
        self.registry
            .authorize_workflow_decision(
                &AttestationVerifier::new(AttestationPolicy::Default),
                request,
                &attestation,
            )
            .expect("authorized decision")
    }

    fn signal(
        &self,
        request: WorkflowSignalAuthorizationRequest,
    ) -> VerifiedWorkflowSignalAuthorization {
        let attestation = self.attestation(RUNTIME_CREDENTIAL, "signal_authorize", &request);
        self.registry
            .authorize_workflow_signal(
                &AttestationVerifier::new(AttestationPolicy::Default),
                request,
                &attestation,
            )
            .expect("authorized signal")
    }

    fn waiver(
        &self,
        request: WorkflowWaiverAuthorizationRequest,
    ) -> VerifiedWorkflowWaiverAuthorization {
        let attestation = self.attestation(HUMAN_CREDENTIAL, "waiver_authorize", &request);
        self.registry
            .authorize_workflow_waiver(
                &AttestationVerifier::new(AttestationPolicy::Default),
                request,
                &attestation,
            )
            .expect("authorized waiver")
    }
}

impl Drop for SignedFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        if let Some(registry_directory) = self.operator_registry_path.parent() {
            let _ = fs::remove_dir_all(registry_directory);
        }
    }
}

fn principal(
    credential: &str,
    principal_id: &str,
    agent_id: &str,
    role: CallerRole,
    key: &SigningKey,
    grants: &[&str],
) -> PrincipalRegistryEntry {
    PrincipalRegistryEntry {
        credential_id: credential.to_owned(),
        principal_id: PrincipalId(principal_id.to_owned()),
        agent_id: StableId(agent_id.to_owned()),
        role,
        public_key_hex: hex(&key.verifying_key().to_bytes()),
        allowed_tools: vec![StableId("workflow".to_owned())],
        authority_grants: grants
            .iter()
            .map(|grant| StableId((*grant).to_owned()))
            .collect(),
        status: PrincipalCredentialStatus::Active,
    }
}

fn basis_digest(root: &Path, refs: &[String]) -> String {
    let mut basis = refs
        .iter()
        .map(|subject_ref| WorkflowContentAddressedReference {
            subject_ref: subject_ref.replace('\\', "/"),
            subject_digest: sha256_content_hash(
                &fs::read(root.join(subject_ref)).expect("applicability basis"),
            ),
        })
        .collect::<Vec<_>>();
    basis.sort_by(|left, right| {
        left.subject_ref
            .cmp(&right.subject_ref)
            .then_with(|| left.subject_digest.cmp(&right.subject_digest))
    });
    sha256_content_hash(&serde_json_canonicalizer::to_vec(&basis).expect("canonical basis"))
}

fn applicability_request(
    fixture: &SignedFixture,
    guidance: &WorkflowGovernanceGuidance,
    applicable: bool,
) -> WorkflowApplicabilityAuthorizationRequest {
    let refs = vec!["README.md".to_owned()];
    WorkflowApplicabilityAuthorizationRequest {
        project_id: guidance.project_id.clone(),
        policy_bundle_digest: guidance.bundle_digest.clone(),
        policy_ref: guidance.selected_policy_ref.clone(),
        state_version: guidance.state_version,
        current_phase: StableId(guidance.current_phase.clone()),
        snapshot_digest: guidance.snapshot_digest.clone(),
        ledger_head_digest: guidance.ledger_head_digest.clone(),
        applicable,
        evaluator_ref: StableId("evaluator.workflow.applicability.human".to_owned()),
        authority_scope: StableId("workflow.applicability.assess".to_owned()),
        basis_digest: basis_digest(&fixture.root, &refs),
        basis_refs: refs,
        observed_at_unix: now(),
        expires_at_unix: now() + 3_600,
    }
}

fn signal_request(
    fixture: &SignedFixture,
    guidance: &WorkflowGovernanceGuidance,
    active: bool,
    episode_id: &str,
    generation: u64,
) -> WorkflowSignalAuthorizationRequest {
    let refs = vec!["README.md".to_owned()];
    WorkflowSignalAuthorizationRequest {
        project_id: guidance.project_id.clone(),
        policy_bundle_digest: guidance.bundle_digest.clone(),
        state_version: guidance.state_version,
        current_phase: StableId(guidance.current_phase.clone()),
        snapshot_digest: guidance.snapshot_digest.clone(),
        ledger_head_digest: guidance.ledger_head_digest.clone(),
        signal: WorkflowGovernanceSignal::ContextRecoveryRequired,
        active,
        episode_id: StableId(episode_id.to_owned()),
        generation,
        basis_digest: basis_digest(&fixture.root, &refs),
        basis_refs: refs,
        observed_at_unix: now(),
        expires_at_unix: now() + 3_600,
    }
}

fn provider_classification(
    provider: WorkflowEvaluatorProvider,
) -> (WorkflowEvidenceKind, WorkflowEvidenceStrength) {
    match provider {
        WorkflowEvaluatorProvider::AuthorizedHuman => (
            WorkflowEvidenceKind::HumanAcceptance,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
        ),
        WorkflowEvaluatorProvider::IndependentReviewer => (
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
        WorkflowEvaluatorProvider::RepositoryInspector => (
            WorkflowEvidenceKind::ArtifactInspection,
            WorkflowEvidenceStrength::InspectedArtifact,
        ),
        WorkflowEvaluatorProvider::DeterministicTool => (
            WorkflowEvidenceKind::DeterministicCheck,
            WorkflowEvidenceStrength::DeterministicVerification,
        ),
        WorkflowEvaluatorProvider::RepresentativeRuntime => (
            WorkflowEvidenceKind::RepresentativeExecution,
            WorkflowEvidenceStrength::RepresentativeExecution,
        ),
        WorkflowEvaluatorProvider::ExternalAuthority => (
            WorkflowEvidenceKind::ExternalAuthority,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
        ),
        WorkflowEvaluatorProvider::ResearchSource => (
            WorkflowEvidenceKind::Research,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
    }
}

fn evidence_request(
    guidance: &WorkflowGovernanceGuidance,
    policy: &forge_core_contracts::WorkflowGovernancePolicy,
    claim_ref: &StableId,
    scenario_ordinal: usize,
) -> WorkflowEvidenceAuthorizationRequest {
    let claim = policy
        .claims
        .iter()
        .find(|claim| claim.id == *claim_ref)
        .expect("claim");
    let evaluator = policy
        .evaluators
        .iter()
        .find(|evaluator| evaluator.id == claim.evaluator_ref)
        .expect("evaluator");
    let (kind, strength) = provider_classification(evaluator.provider);
    let observed = now();
    WorkflowEvidenceAuthorizationRequest {
        project_id: guidance.project_id.clone(),
        policy_bundle_digest: guidance.bundle_digest.clone(),
        policy_ref: policy.id.clone(),
        claim_ref: claim.id.clone(),
        evaluator_ref: evaluator.id.clone(),
        provider: evaluator.provider,
        kind,
        strength,
        outcome: WorkflowEvidenceOutcome::Pass,
        subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
        subject_ref: guidance.project_id.0.clone(),
        subject_digest: guidance.snapshot_digest.clone(),
        scenario_digest: sha256_content_hash(
            format!("scenario:{}:{scenario_ordinal}", claim.id.0).as_bytes(),
        ),
        state_version: guidance.state_version,
        current_phase: StableId(guidance.current_phase.clone()),
        snapshot_digest: guidance.snapshot_digest.clone(),
        ledger_head_digest: guidance.ledger_head_digest.clone(),
        readiness_target: policy.routing.readiness_target,
        observed_at_unix: observed,
        expires_at_unix: Some(observed + evaluator.max_age_seconds.min(3_600)),
    }
}

fn selected_policy<'a>(
    document: &'a WorkflowGovernanceBundleDocument,
    guidance: &WorkflowGovernanceGuidance,
) -> &'a WorkflowGovernancePolicy {
    document
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id == guidance.selected_policy_ref)
        .expect("selected admitted policy")
}

fn capability_request(
    guidance: &WorkflowGovernanceGuidance,
    policy: &WorkflowGovernancePolicy,
    available: bool,
    expires_at_unix: Option<u64>,
) -> WorkflowCapabilityAuthorizationRequest {
    let requirement = policy
        .capability_requirements
        .first()
        .expect("selected capability requirement");
    let observed = now();
    WorkflowCapabilityAuthorizationRequest {
        project_id: guidance.project_id.clone(),
        policy_bundle_digest: guidance.bundle_digest.clone(),
        policy_ref: policy.id.clone(),
        capability_ref: requirement.id.clone(),
        state_version: guidance.state_version,
        current_phase: StableId(guidance.current_phase.clone()),
        snapshot_digest: guidance.snapshot_digest.clone(),
        ledger_head_digest: guidance.ledger_head_digest.clone(),
        probe_kind: requirement.probe_kind,
        available,
        authority_scope: StableId("workflow.capability.authorize".to_owned()),
        probe_ref: format!("runtime:adversarial:{}", requirement.id.0),
        probe_digest: sha256_content_hash(requirement.id.0.as_bytes()),
        subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
        subject_ref: guidance.project_id.0.clone(),
        subject_digest: guidance.snapshot_digest.clone(),
        observed_at_unix: observed,
        expires_at_unix,
    }
}

fn waiver_request(
    guidance: &WorkflowGovernanceGuidance,
    policy: &WorkflowGovernancePolicy,
    subject: WorkflowWaiverSubject,
    maximum_readiness_target: &str,
    expires_at_unix: i64,
) -> WorkflowWaiverAuthorizationRequest {
    WorkflowWaiverAuthorizationRequest {
        project_id: guidance.project_id.clone(),
        policy_bundle_digest: guidance.bundle_digest.clone(),
        policy_ref: policy.id.clone(),
        subject,
        state_version: guidance.state_version,
        current_phase: StableId(guidance.current_phase.clone()),
        snapshot_digest: guidance.snapshot_digest.clone(),
        ledger_head_digest: guidance.ledger_head_digest.clone(),
        maximum_readiness_target: maximum_readiness_target.to_owned(),
        reason: "adversarial waiver boundary test".to_owned(),
        consequences_ack_digest: sha256_content_hash(b"adversarial waiver consequences"),
        expires_at_unix,
    }
}

fn assess_current_applicability(fixture: &SignedFixture, applicable: bool) {
    let guidance = fixture.adapter.next().expect("applicability guidance");
    assert_eq!(
        guidance.status,
        WorkflowGovernanceGuidanceStatus::ApplicabilityRequired
    );
    let request = applicability_request(fixture, &guidance, applicable);
    fixture
        .adapter
        .record_authorized_applicability(fixture.applicability(request))
        .expect("signed applicability receipt");
}

fn advance_to_applicable_domain_scan(
    fixture: &SignedFixture,
) -> (WorkflowGovernanceBundleDocument, WorkflowGovernanceGuidance) {
    complete_discover_intent(fixture);
    assess_current_applicability(fixture, true);
    let document = bundle();
    let guidance = fixture.adapter.next().expect("applicable domain guidance");
    assert_eq!(
        guidance.selected_policy_ref.0,
        "policy.workflow.domain-scan"
    );
    assert_eq!(guidance.applicability, Some(true));
    (document, guidance)
}

fn complete_discover_intent(fixture: &SignedFixture) {
    let guidance = fixture.adapter.next().expect("discover guidance");
    let document = bundle();
    let policy = document
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id == guidance.selected_policy_ref)
        .expect("discover policy");
    let request = evidence_request(&guidance, policy, &policy.claims[0].id, 0);
    fixture
        .adapter
        .record_authorized_evidence(fixture.evidence(request))
        .expect("signed discover evidence");
    let prepared = fixture
        .adapter
        .prepare_completion()
        .expect("prepare discover");
    fixture
        .adapter
        .consume_completion(
            prepared,
            PrincipalId("principal.workflow.replacement-agent".to_owned()),
        )
        .expect("complete discover");
}

fn upgrade_to_foundation(fixture: &SignedFixture) -> String {
    let status = fixture.adapter.release_status().expect("release status");
    let target = status
        .available_successor
        .expect("foundation successor available");
    let target_digest = target.release_digest.clone();
    let receipt = fixture
        .adapter
        .release_upgrade(
            &target.release_id,
            &status.active.release.release_digest,
            &status.ledger_head_digest,
            &status.snapshot_digest,
        )
        .expect("foundation upgrade");
    assert_eq!(
        receipt.status,
        WorkflowGovernanceReleaseUpgradeStatus::Upgraded
    );
    assert_eq!(receipt.active.release, target);
    target_digest
}

#[test]
fn release_status_ignores_local_override_and_does_not_rewrite_p5c_ledger() {
    let fixture = SignedFixture::new("release-status-fixed-embedded");
    let wal = fixture
        .root
        .join(".forge-method/wal/workflow-governance.ndjson");
    let before = fs::read(&wal).expect("P5c WAL");
    let local = fixture
        .root
        .join("contracts/migration/workflow-governance-release-registry-v0.yaml");
    fs::create_dir_all(local.parent().expect("local parent")).expect("local contracts");
    fs::write(&local, "authority: caller_override\n").expect("hostile local override");

    let status = fixture.adapter.release_status().expect("embedded status");
    assert_eq!(
        status.active.pin_origin,
        WorkflowGovernanceReleasePinOrigin::ImplicitP5cGenesis
    );
    assert_eq!(
        status.active.release.release_id.0,
        "workflow-governance.release.p5c-implicit-v0"
    );
    assert!(status.available_successor.is_some());
    assert_eq!(status.upgrade_argv.as_ref().map(Vec::len), Some(13));
    assert_eq!(fs::read(&wal).expect("unchanged WAL"), before);
}

#[test]
fn release_upgrade_is_cas_bound_resumable_and_idempotent() {
    let fixture = SignedFixture::new("release-upgrade-success");
    let initial = fixture.adapter.release_status().expect("initial status");
    let target = initial
        .available_successor
        .clone()
        .expect("foundation successor");
    let wal = fixture
        .root
        .join(".forge-method/wal/workflow-governance.ndjson");
    let before = fs::read(&wal).expect("initial WAL");

    assert!(matches!(
        fixture.adapter.release_upgrade(
            &target.release_id,
            &initial.active.release.release_digest,
            "sha256:stale-head",
            &initial.snapshot_digest,
        ),
        Err(WorkflowGovernanceAdapterError::ReleaseCasMismatch)
    ));
    assert_eq!(fs::read(&wal).expect("CAS WAL"), before);

    let target_digest = upgrade_to_foundation(&fixture);
    let after_upgrade = fs::read(&wal).expect("upgraded WAL");
    assert_ne!(after_upgrade, before);
    let resumed = fixture.adapter.resume().expect("replacement-agent resume");
    assert_eq!(resumed.release.release.release_digest, target_digest);
    assert_eq!(
        resumed.release.pin_origin,
        WorkflowGovernanceReleasePinOrigin::LedgerTransition
    );

    let replay = fixture
        .adapter
        .release_upgrade(
            &target.release_id,
            "sha256:intentionally-stale-release",
            "sha256:intentionally-stale-head",
            "sha256:intentionally-stale-snapshot",
        )
        .expect("idempotent replay");
    assert_eq!(
        replay.status,
        WorkflowGovernanceReleaseUpgradeStatus::AlreadyPinned
    );
    assert!(replay.transition_record.is_none());
    assert_eq!(fs::read(&wal).expect("replay WAL"), after_upgrade);

    assert!(matches!(
        fixture.adapter.release_upgrade(
            &initial.active.release.release_id,
            &target_digest,
            &replay.ledger_head_digest,
            &replay.snapshot_digest,
        ),
        Err(WorkflowGovernanceAdapterError::ReleaseNotAdjacent)
    ));
}

#[test]
fn unknown_and_genesis_self_upgrade_fail_without_mutation() {
    let fixture = SignedFixture::new("release-upgrade-invalid");
    let status = fixture.adapter.release_status().expect("status");
    let wal = fixture
        .root
        .join(".forge-method/wal/workflow-governance.ndjson");
    let before = fs::read(&wal).expect("WAL");
    let invoke = |target: StableId| {
        fixture.adapter.release_upgrade(
            &target,
            &status.active.release.release_digest,
            &status.ledger_head_digest,
            &status.snapshot_digest,
        )
    };
    assert!(matches!(
        invoke(StableId("workflow-governance.release.unknown".to_owned())),
        Err(WorkflowGovernanceAdapterError::UnknownRelease(_))
    ));
    assert!(matches!(
        invoke(status.active.release.release_id.clone()),
        Err(WorkflowGovernanceAdapterError::ReleaseNotAdjacent)
    ));
    assert_eq!(fs::read(&wal).expect("unchanged WAL"), before);
}

#[test]
fn release_upgrade_invalidates_prepared_completion_authority() {
    let fixture = SignedFixture::new("release-upgrade-prepared-drift");
    let guidance = fixture.adapter.next().expect("discover guidance");
    let document = bundle();
    let policy = selected_policy(&document, &guidance);
    let request = evidence_request(&guidance, policy, &policy.claims[0].id, 0);
    fixture
        .adapter
        .record_authorized_evidence(fixture.evidence(request))
        .expect("signed evidence");
    let prepared = fixture
        .adapter
        .prepare_completion()
        .expect("prepared under P5c release");

    upgrade_to_foundation(&fixture);
    assert!(matches!(
        fixture.adapter.consume_completion(
            prepared,
            PrincipalId("principal.workflow.replacement-agent".to_owned()),
        ),
        Err(WorkflowGovernanceAdapterError::CompletionDrift)
    ));
}

#[test]
fn core_assurance_upgrade_invalidates_receipts_and_foundation_prepared_authority() {
    let fixture = SignedFixture::new("core-assurance-upgrade-invalidation");
    let document = bundle();
    let genesis = fixture.adapter.next().expect("genesis guidance");
    let policy = selected_policy(&document, &genesis);
    let request = evidence_request(&genesis, policy, &policy.claims[0].id, 0);
    fixture
        .adapter
        .record_authorized_evidence(fixture.evidence(request))
        .expect("signed evidence under P5c");
    assert_eq!(
        fixture.adapter.next().expect("P5c ready").status,
        WorkflowGovernanceGuidanceStatus::ReadyToComplete
    );

    upgrade_to_foundation(&fixture);
    assert_eq!(
        fixture.adapter.next().expect("foundation carryover").status,
        WorkflowGovernanceGuidanceStatus::ReadyToComplete,
        "policy-equivalent foundation preserves the exact receipt window"
    );
    let prepared = fixture
        .adapter
        .prepare_completion()
        .expect("prepared under foundation");
    let foundation = fixture.adapter.release_status().expect("foundation status");
    let target = foundation
        .available_successor
        .clone()
        .expect("reviewed core-assurance successor");
    assert_eq!(
        target.release_id.0,
        "workflow-governance.release.core-assurance-v0"
    );
    let receipt = fixture
        .adapter
        .release_upgrade(
            &target.release_id,
            &foundation.active.release.release_digest,
            &foundation.ledger_head_digest,
            &foundation.snapshot_digest,
        )
        .expect("core-assurance upgrade");
    let WorkflowGovernanceEvent::ReleaseUpgraded(transition) = &receipt
        .transition_record
        .as_ref()
        .expect("core-assurance transition record")
        .event
    else {
        panic!("expected release-upgraded event");
    };
    assert_eq!(
        transition.receipt_carryover,
        WorkflowReceiptCarryover::InvalidateAll
    );
    assert_eq!(receipt.active.release, target);

    let resumed = fixture.adapter.resume().expect("replacement-agent resume");
    assert_eq!(resumed.release.release, target);
    let assurance_successor = fixture
        .adapter
        .release_status()
        .expect("core-assurance status")
        .available_successor
        .expect("assurance-operations successor");
    assert_eq!(
        assurance_successor.release_id.0,
        "workflow-governance.release.assurance-operations-v0"
    );
    assert_eq!(assurance_successor.release_version, "0.3.0");
    let invalidated = fixture.adapter.next().expect("invalidated guidance");
    assert_ne!(
        invalidated.status,
        WorkflowGovernanceGuidanceStatus::ReadyToComplete
    );
    assert!(invalidated
        .simulation
        .candidate_claim_results
        .iter()
        .all(|result| !matches!(
            result.status,
            WorkflowClaimResultStatus::Verified | WorkflowClaimResultStatus::Waived
        )));
    assert!(matches!(
        fixture.adapter.consume_completion(
            prepared,
            PrincipalId("principal.workflow.replacement-agent".to_owned()),
        ),
        Err(WorkflowGovernanceAdapterError::CompletionDrift)
    ));
}

#[test]
// This single end-to-end narrative intentionally keeps CAS rejection, receipt
// invalidation, prepared-authority drift, and replacement-agent recovery on
// the same persisted project so no assertion silently uses a fresh fixture.
#[allow(clippy::too_many_lines)]
fn assurance_operations_upgrade_is_adjacent_cas_bound_and_resumable() {
    let fixture = SignedFixture::new("assurance-operations-upgrade");
    let assurance_operations =
        StableId("workflow-governance.release.assurance-operations-v0".to_owned());
    let initial = fixture.adapter.release_status().expect("initial status");
    assert!(matches!(
        fixture.adapter.release_upgrade(
            &assurance_operations,
            &initial.active.release.release_digest,
            &initial.ledger_head_digest,
            &initial.snapshot_digest,
        ),
        Err(WorkflowGovernanceAdapterError::ReleaseNotAdjacent)
    ));

    upgrade_to_foundation(&fixture);
    let foundation = fixture.adapter.release_status().expect("foundation status");
    let core = foundation
        .available_successor
        .clone()
        .expect("core-assurance successor");
    fixture
        .adapter
        .release_upgrade(
            &core.release_id,
            &foundation.active.release.release_digest,
            &foundation.ledger_head_digest,
            &foundation.snapshot_digest,
        )
        .expect("core-assurance upgrade");

    let core_status = fixture.adapter.release_status().expect("core status");
    let target = core_status
        .available_successor
        .clone()
        .expect("assurance-operations successor");
    assert_eq!(target.release_id, assurance_operations);

    let guidance = fixture.adapter.next().expect("core guidance");
    let document = bundle();
    let policy = selected_policy(&document, &guidance);
    let request = evidence_request(&guidance, policy, &policy.claims[0].id, 0);
    fixture
        .adapter
        .record_authorized_evidence(fixture.evidence(request))
        .expect("fresh core-assurance evidence");
    let prepared = fixture
        .adapter
        .prepare_completion()
        .expect("prepared under core-assurance");
    let upgrade_status = fixture
        .adapter
        .release_status()
        .expect("fresh core status after evidence");

    let wal = fixture
        .root
        .join(".forge-method/wal/workflow-governance.ndjson");
    let before = fs::read(&wal).expect("core WAL");
    assert!(matches!(
        fixture.adapter.release_upgrade(
            &target.release_id,
            &upgrade_status.active.release.release_digest,
            "sha256:stale-ledger-head",
            &upgrade_status.snapshot_digest,
        ),
        Err(WorkflowGovernanceAdapterError::ReleaseCasMismatch)
    ));
    assert_eq!(fs::read(&wal).expect("unchanged CAS WAL"), before);

    let receipt = fixture
        .adapter
        .release_upgrade(
            &target.release_id,
            &upgrade_status.active.release.release_digest,
            &upgrade_status.ledger_head_digest,
            &upgrade_status.snapshot_digest,
        )
        .expect("assurance-operations upgrade");
    let WorkflowGovernanceEvent::ReleaseUpgraded(transition) = &receipt
        .transition_record
        .as_ref()
        .expect("assurance-operations transition")
        .event
    else {
        panic!("expected release-upgraded event");
    };
    assert_eq!(
        transition.receipt_carryover,
        WorkflowReceiptCarryover::InvalidateAll
    );
    assert_eq!(receipt.active.release, target);

    let replacement = WorkflowGovernanceProjectAdapter::new(
        fixture.project_id.clone(),
        &fixture.root,
        fixture.root.join(".forge-method"),
    )
    .expect("replacement adapter");
    let resumed = replacement.resume().expect("replacement-agent resume");
    assert_eq!(resumed.release.release, target);
    assert!(replacement
        .release_status()
        .expect("assurance-operations status")
        .available_successor
        .is_none());
    let invalidated = replacement.next().expect("invalidated successor guidance");
    assert_ne!(
        invalidated.status,
        WorkflowGovernanceGuidanceStatus::ReadyToComplete
    );
    assert!(invalidated
        .simulation
        .candidate_claim_results
        .iter()
        .all(|result| !matches!(
            result.status,
            WorkflowClaimResultStatus::Verified | WorkflowClaimResultStatus::Waived
        )));
    assert!(matches!(
        replacement.consume_completion(
            prepared,
            PrincipalId("principal.workflow.replacement-agent".to_owned()),
        ),
        Err(WorkflowGovernanceAdapterError::CompletionDrift)
    ));
}

#[test]
fn expired_evidence_is_recomputed_as_stale_and_blocks_completion() {
    let fixture = SignedFixture::new("expired-evidence");
    let document = bundle();
    let guidance = fixture.adapter.next().expect("discover guidance");
    let policy = selected_policy(&document, &guidance);
    let mut request = evidence_request(&guidance, policy, &policy.claims[0].id, 0);
    // Keep enough headroom for parallel Windows CI filesystem scans before
    // proving that the exact receipt later becomes stale.
    request.expires_at_unix = Some(now() + 10);
    fixture
        .adapter
        .record_authorized_evidence(fixture.evidence(request))
        .expect("short-lived signed evidence");
    assert_eq!(
        fixture.adapter.next().expect("fresh evidence").status,
        WorkflowGovernanceGuidanceStatus::ReadyToComplete
    );

    thread::sleep(Duration::from_secs(11));
    let stale = fixture
        .adapter
        .next()
        .expect("stale evidence re-evaluation");
    assert_ne!(
        stale.status,
        WorkflowGovernanceGuidanceStatus::ReadyToComplete
    );
    assert!(stale
        .simulation
        .candidate_claim_results
        .iter()
        .any(|result| {
            result.claim_id == policy.claims[0].id.0
                && !matches!(
                    result.status,
                    WorkflowClaimResultStatus::Verified | WorkflowClaimResultStatus::Waived
                )
        }));
    assert!(matches!(
        fixture.adapter.prepare_completion(),
        Err(WorkflowGovernanceAdapterError::PolicyIncomplete)
    ));
}

#[test]
fn pass_plus_fail_is_contested_and_cannot_complete() {
    let fixture = SignedFixture::new("contradictory-evidence");
    let document = bundle();
    let guidance = fixture.adapter.next().expect("discover guidance");
    let policy = selected_policy(&document, &guidance);
    let pass = evidence_request(&guidance, policy, &policy.claims[0].id, 0);
    fixture
        .adapter
        .record_authorized_evidence(fixture.evidence(pass))
        .expect("signed pass");

    let after_pass = fixture.adapter.next().expect("ready after pass");
    assert_eq!(
        after_pass.status,
        WorkflowGovernanceGuidanceStatus::ReadyToComplete
    );
    let mut fail = evidence_request(&after_pass, policy, &policy.claims[0].id, 1);
    fail.outcome = WorkflowEvidenceOutcome::Fail;
    fail.scenario_digest = sha256_content_hash(b"independent-disproof-scenario");
    fixture
        .adapter
        .record_authorized_evidence(fixture.evidence(fail))
        .expect("signed disproof");

    let contested = fixture.adapter.next().expect("contested guidance");
    assert!(contested
        .simulation
        .candidate_claim_results
        .iter()
        .any(|result| {
            result.claim_id == policy.claims[0].id.0
                && matches!(
                    result.status,
                    WorkflowClaimResultStatus::Contradictory | WorkflowClaimResultStatus::Disproven
                )
        }));
    assert!(matches!(
        fixture.adapter.prepare_completion(),
        Err(WorkflowGovernanceAdapterError::PolicyIncomplete)
    ));
}

#[test]
fn revoking_the_supporting_receipt_recomputes_completion() {
    let fixture = SignedFixture::new("evidence-revocation");
    let document = bundle();
    let guidance = fixture.adapter.next().expect("discover guidance");
    let policy = selected_policy(&document, &guidance);
    let request = evidence_request(&guidance, policy, &policy.claims[0].id, 0);
    let evidence_record = fixture
        .adapter
        .record_authorized_evidence(fixture.evidence(request))
        .expect("signed evidence");
    assert_eq!(
        fixture
            .adapter
            .next()
            .expect("ready before revocation")
            .status,
        WorkflowGovernanceGuidanceStatus::ReadyToComplete
    );

    let state_root = fixture.root.join(".forge-method");
    let projection = recover_workflow_governance_ledger(&state_root).expect("ledger projection");
    let identity = projection.identity().expect("ledger identity");
    let head = projection.head_digest.as_deref().expect("ledger head");
    let state_version = projection
        .current_state_version()
        .expect("ledger state version");
    append_workflow_governance_event_tcb(
        &state_root,
        head,
        &identity,
        state_version,
        WorkflowGovernanceEvent::ReceiptRevoked(ReceiptRevokedEvent {
            revoked_record_id: evidence_record.record_id,
            revoked_record_digest: evidence_record.record_digest,
            principal: PrincipalId("principal.workflow.operator".to_owned()),
            authority_scope: StableId("workflow.receipt.revoke".to_owned()),
            reason: "adversarial evidence invalidation".to_owned(),
            revoked_at_unix: now(),
        }),
    )
    .expect("append trusted revocation");

    let revoked = fixture.adapter.next().expect("recomputed after revocation");
    assert_ne!(
        revoked.status,
        WorkflowGovernanceGuidanceStatus::ReadyToComplete
    );
    assert!(matches!(
        fixture.adapter.prepare_completion(),
        Err(WorkflowGovernanceAdapterError::PolicyIncomplete)
    ));
}

#[test]
fn unavailable_expired_and_misbound_capabilities_keep_the_gap_visible_across_handoff() {
    let fixture = SignedFixture::new("capability-adversarial");
    let (document, guidance) = advance_to_applicable_domain_scan(&fixture);
    let policy = selected_policy(&document, &guidance);
    let capability_id = policy.capability_requirements[0].id.clone();

    let mut misbound = capability_request(&guidance, policy, true, Some(now() + 3_600));
    misbound.snapshot_digest = sha256_content_hash(b"wrong project snapshot");
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_capability(fixture.capability(misbound)),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));

    let current = fixture.adapter.next().expect("unchanged after rejection");
    let unavailable = capability_request(&current, policy, false, Some(now() + 3_600));
    fixture
        .adapter
        .record_authorized_capability(fixture.capability(unavailable))
        .expect("signed unavailable capability");
    let gap = fixture.adapter.next().expect("unavailable gap");
    assert!(gap
        .simulation
        .candidate_capability_gaps
        .iter()
        .any(|candidate| candidate.id == capability_id));

    let replacement = WorkflowGovernanceProjectAdapter::new(
        gap.project_id.clone(),
        &fixture.root,
        fixture.root.join(".forge-method"),
    )
    .expect("replacement adapter");
    assert_eq!(
        serde_json::to_value(replacement.resume().expect("replacement resume"))
            .expect("resumed guidance JSON"),
        serde_json::to_value(&gap).expect("original guidance JSON")
    );

    // Leave enough headroom for a loaded Windows runner to recompute the
    // expanded reviewed registry before the first assertion. The previous
    // two-second window became timing-dependent after the fourth release.
    let expiring = capability_request(&gap, policy, true, Some(now() + 10));
    fixture
        .adapter
        .record_authorized_capability(fixture.capability(expiring))
        .expect("short-lived available capability");
    assert!(!fixture
        .adapter
        .next()
        .expect("fresh capability")
        .simulation
        .candidate_capability_gaps
        .iter()
        .any(|candidate| candidate.id == capability_id));
    thread::sleep(Duration::from_secs(11));
    assert!(fixture
        .adapter
        .next()
        .expect("expired capability")
        .simulation
        .candidate_capability_gaps
        .iter()
        .any(|candidate| candidate.id == capability_id));
}

#[test]
fn waiver_boundaries_reject_nonwaivable_target_subject_snapshot_and_expiry() {
    let nonwaivable_fixture = SignedFixture::new("waiver-not-allowed");
    let document = bundle();
    let guidance = nonwaivable_fixture
        .adapter
        .next()
        .expect("discover guidance");
    let policy = selected_policy(&document, &guidance);
    let request = waiver_request(
        &guidance,
        policy,
        WorkflowWaiverSubject::Claim {
            claim_ref: policy.claims[0].id.clone(),
        },
        "explore",
        i64::try_from(now() + 3_600).expect("expiry"),
    );
    assert!(matches!(
        nonwaivable_fixture
            .adapter
            .record_authorized_waiver(nonwaivable_fixture.waiver(request)),
        Err(WorkflowGovernanceAdapterError::WaiverNotAllowed)
    ));

    let fixture = SignedFixture::new("waiver-bounds");
    let (document, guidance) = advance_to_applicable_domain_scan(&fixture);
    let policy = selected_policy(&document, &guidance);
    let claim_subject = WorkflowWaiverSubject::Claim {
        claim_ref: policy.claims[0].id.clone(),
    };
    let expiry = i64::try_from(now() + 3_600).expect("expiry");

    let above_target = waiver_request(&guidance, policy, claim_subject.clone(), "execute", expiry);
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_waiver(fixture.waiver(above_target)),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));

    let wrong_subject = waiver_request(
        &guidance,
        policy,
        WorkflowWaiverSubject::Obligation {
            obligation_ref: policy.obligations[0].id.clone(),
        },
        "explore",
        expiry,
    );
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_waiver(fixture.waiver(wrong_subject)),
        Err(WorkflowGovernanceAdapterError::InvalidObservation(_))
    ));

    let mut wrong_snapshot =
        waiver_request(&guidance, policy, claim_subject.clone(), "explore", expiry);
    wrong_snapshot.snapshot_digest = sha256_content_hash(b"wrong waiver snapshot");
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_waiver(fixture.waiver(wrong_snapshot)),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));

    let expired = waiver_request(
        &guidance,
        policy,
        claim_subject.clone(),
        "explore",
        i64::try_from(now().saturating_sub(1)).expect("expired timestamp"),
    );
    let expired_attestation = fixture.attestation(HUMAN_CREDENTIAL, "waiver_authorize", &expired);
    assert!(matches!(
        fixture.registry.authorize_workflow_waiver(
            &AttestationVerifier::new(AttestationPolicy::Default),
            expired,
            &expired_attestation,
        ),
        Err(WorkflowAuthorityError::WaiverExpired { .. })
    ));

    let valid = waiver_request(&guidance, policy, claim_subject, "explore", expiry);
    let record = fixture
        .adapter
        .record_authorized_waiver(fixture.waiver(valid))
        .expect("bounded waiver");
    let WorkflowGovernanceEvent::WaiverAuthorized(event) = record.event else {
        panic!("expected waiver receipt");
    };
    assert_eq!(event.authority_scope.0, "project.domain");
    assert_eq!(event.subject.subject_digest, guidance.snapshot_digest);
}

#[test]
fn waiver_cannot_be_pre_authorized_for_an_unselected_future_policy() {
    let fixture = SignedFixture::new("waiver-future-policy-scope");
    let (document, guidance) = advance_to_applicable_domain_scan(&fixture);
    let future_policy = document
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id.0 == "policy.workflow.write-spec")
        .expect("future waivable policy");
    let request = waiver_request(
        &guidance,
        future_policy,
        WorkflowWaiverSubject::Claim {
            claim_ref: future_policy.claims[0].id.clone(),
        },
        "execute",
        i64::try_from(now() + 3_600).expect("expiry"),
    );
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_waiver(fixture.waiver(request)),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));
}

#[test]
fn future_representative_execution_cannot_be_asserted_without_running_it() {
    let fixture = SignedFixture::new("future-representative-evidence");
    let document = bundle();
    let guidance = fixture.adapter.next().expect("initial guidance");
    let build_policy = document
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build policy");
    let representative_claim = build_policy
        .claims
        .iter()
        .find(|claim| claim.id.0 == "claim.workflow.build-story.representative-execution")
        .expect("representative execution claim");
    let request = evidence_request(&guidance, build_policy, &representative_claim.id, 0);

    assert!(matches!(
        fixture
            .adapter
            .record_authorized_evidence(fixture.evidence(request)),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));
}

#[test]
fn replacement_agent_recovers_pending_decision_and_ranked_next_action() {
    let fixture = SignedFixture::new("decision-handoff");
    complete_discover_intent(&fixture);
    assess_current_applicability(&fixture, false);
    assess_current_applicability(&fixture, false);

    let pending = fixture.adapter.next().expect("product decision guidance");
    assert_eq!(
        pending.selected_policy_ref.0,
        "policy.workflow.product-requirements"
    );
    assert!(!pending.simulation.candidate_decision_requests.is_empty());
    assert!(pending
        .simulation
        .candidate_next_actions
        .iter()
        .any(|action| matches!(action.kind, NextActionKind::AskHuman)));

    let replacement = WorkflowGovernanceProjectAdapter::new(
        pending.project_id.clone(),
        &fixture.root,
        fixture.root.join(".forge-method"),
    )
    .expect("replacement adapter");
    assert_eq!(
        serde_json::to_value(replacement.resume().expect("replacement decision resume"))
            .expect("resumed JSON"),
        serde_json::to_value(pending).expect("pending JSON")
    );
}

#[test]
fn signed_snapshot_and_head_bound_authorizations_reject_replay_and_drift() {
    let fixture = SignedFixture::new("signed-replay");
    complete_discover_intent(&fixture);

    let applicability_guidance = fixture.adapter.next().expect("domain applicability");
    assert_eq!(
        applicability_guidance.status,
        WorkflowGovernanceGuidanceStatus::ApplicabilityRequired
    );
    let applicability_request = applicability_request(&fixture, &applicability_guidance, true);
    let signed_before_drift = fixture.applicability(applicability_request.clone());
    fs::write(fixture.root.join("README.md"), "snapshot drift\n").expect("drift");
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_applicability(signed_before_drift),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));

    fs::write(fixture.root.join("README.md"), "governed consumer\n").expect("restore");
    fixture
        .adapter
        .record_authorized_applicability(fixture.applicability(applicability_request.clone()))
        .expect("current signed applicability");
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_applicability(fixture.applicability(applicability_request)),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));

    let capability_guidance = fixture.adapter.next().expect("domain capability");
    let policy = bundle()
        .workflow_governance_bundle
        .policies
        .into_iter()
        .find(|policy| policy.id == capability_guidance.selected_policy_ref)
        .expect("domain policy");
    let requirement = policy.capability_requirements.first().expect("capability");
    let observed = now();
    let capability_request = WorkflowCapabilityAuthorizationRequest {
        project_id: capability_guidance.project_id.clone(),
        policy_bundle_digest: capability_guidance.bundle_digest.clone(),
        policy_ref: policy.id,
        capability_ref: requirement.id.clone(),
        state_version: capability_guidance.state_version,
        current_phase: StableId(capability_guidance.current_phase.clone()),
        snapshot_digest: capability_guidance.snapshot_digest.clone(),
        ledger_head_digest: capability_guidance.ledger_head_digest.clone(),
        probe_kind: requirement.probe_kind,
        available: true,
        authority_scope: StableId("workflow.capability.authorize".to_owned()),
        probe_ref: "runtime:test-domain-review".to_owned(),
        probe_digest: sha256_content_hash(b"runtime:test-domain-review"),
        subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
        subject_ref: capability_guidance.project_id.0.clone(),
        subject_digest: capability_guidance.snapshot_digest,
        observed_at_unix: observed,
        expires_at_unix: Some(observed + 3_600),
    };
    fixture
        .adapter
        .record_authorized_capability(fixture.capability(capability_request.clone()))
        .expect("current signed capability");
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_capability(fixture.capability(capability_request)),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));
}

#[test]
fn signed_signal_transitions_enforce_monotonic_episodes_and_reject_replay() {
    let fixture = SignedFixture::new("signed-signal-episodes");
    let initial = fixture.adapter.next().expect("initial guidance");
    let open = signal_request(&fixture, &initial, true, "signal.episode.context.1", 1);
    fixture
        .adapter
        .record_authorized_signal(fixture.signal(open.clone()))
        .expect("open first signal episode");
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_signal(fixture.signal(open)),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));

    let active = fixture.adapter.next().expect("active signal guidance");
    assert_eq!(
        active.selected_policy_ref.0,
        "policy.workflow.context-recovery"
    );
    fs::write(
        fixture.root.join("README.md"),
        "signal basis snapshot drift\n",
    )
    .expect("drift signal snapshot");
    let drifted = fixture.adapter.next().expect("signal drift guidance");
    let close = signal_request(&fixture, &drifted, false, "signal.episode.context.1", 1);
    fixture
        .adapter
        .record_authorized_signal(fixture.signal(close))
        .expect("close first signal episode");

    let closed = fixture.adapter.next().expect("closed signal guidance");
    let regressed = signal_request(&fixture, &closed, true, "signal.episode.context.1", 1);
    assert!(matches!(
        fixture
            .adapter
            .record_authorized_signal(fixture.signal(regressed)),
        Err(WorkflowGovernanceAdapterError::AuthorizationBindingMismatch)
    ));

    let reopen = signal_request(&fixture, &closed, true, "signal.episode.context.2", 2);
    fixture
        .adapter
        .record_authorized_signal(fixture.signal(reopen))
        .expect("open next monotonic signal episode");
    let reopened = fixture.adapter.next().expect("reopened signal guidance");
    assert_eq!(
        reopened.selected_policy_ref.0,
        "policy.workflow.context-recovery"
    );
}

#[test]
fn private_registry_cannot_escalate_fixed_principal_grants() {
    let fixture = SignedFixture::new("signed-registry-escalation");
    let guidance = fixture.adapter.next().expect("initial guidance");
    let request = signal_request(&fixture, &guidance, true, "signal.episode.escalated.1", 1);
    let attestation = fixture.attestation(WORKER_CREDENTIAL, "signal_authorize", &request);
    let escalated = AuthorizedPrincipalRegistry::from_document(PrincipalRegistryDocument {
        schema_version: PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
        principal_registry: PrincipalRegistryContract {
            audience: AUDIENCE.to_owned(),
            principals: vec![principal(
                WORKER_CREDENTIAL,
                "principal.workflow.reviewer",
                "agent.workflow.reviewer",
                CallerRole::Worker,
                &fixture.worker_key,
                &[
                    "workflow.evidence.authorize_review",
                    "workflow.evidence.authorize_external",
                    "workflow.signal.authorize",
                ],
            )],
        },
    })
    .expect("attacker-controlled registry is internally valid");
    let authorization = escalated
        .authorize_workflow_signal(
            &AttestationVerifier::new(AttestationPolicy::Default),
            request,
            &attestation,
        )
        .expect("private registry can mint only a locally opaque proof");
    assert!(matches!(
        fixture.adapter.record_authorized_signal(authorization),
        Err(WorkflowGovernanceAdapterError::TrustedRegistry { .. })
    ));
}

#[test]
// This is the executable 15-policy acceptance story. Keeping it contiguous
// makes cross-policy routing, refresh, and terminal invariants auditable.
#[allow(clippy::too_many_lines)]
fn all_admitted_required_policies_use_signed_authority_and_reach_terminal_resume() {
    let fixture = SignedFixture::new("signed-full-golden");
    let document = bundle();
    let mut completed = Vec::new();
    let mut decision_authorized = false;
    let mut waiver_authorized = false;
    let mut waived_claims = BTreeSet::new();
    let mut observation_counts = BTreeMap::<String, usize>::new();

    for _step in 0..180 {
        let guidance = fixture.adapter.next().expect("governed next");
        if guidance.status == WorkflowGovernanceGuidanceStatus::PhaseComplete {
            assert_eq!(guidance.current_phase, "4-build-verify");
            assert_eq!(document.workflow_governance_bundle.policies.len(), 15);
            assert!(
                decision_authorized,
                "signed decision path was not exercised"
            );
            assert!(waiver_authorized, "signed waiver path was not exercised");
            let resumed = fixture.adapter.resume().expect("replacement-agent resume");
            assert_eq!(resumed.ledger_head_digest, guidance.ledger_head_digest);
            assert_eq!(resumed.state_version, guidance.state_version);
            fs::write(
                fixture.root.join("README.md"),
                "post-release project drift\n",
            )
            .expect("drift terminal project snapshot");
            let drifted = fixture.adapter.next().expect("terminal drift guidance");
            assert_ne!(
                drifted.status,
                WorkflowGovernanceGuidanceStatus::PhaseComplete,
                "release completion must be consumed again after snapshot drift"
            );
            return;
        }

        let policy = document
            .workflow_governance_bundle
            .policies
            .iter()
            .find(|policy| policy.id == guidance.selected_policy_ref)
            .expect("selected admitted policy");

        if guidance.status == WorkflowGovernanceGuidanceStatus::ApplicabilityRequired {
            let request = applicability_request(&fixture, &guidance, false);
            fixture
                .adapter
                .record_authorized_applicability(fixture.applicability(request))
                .expect("signed not-applicable receipt");
            continue;
        }

        if policy.id.0 == "policy.workflow.product-requirements" && !decision_authorized {
            let rule = policy.decision_rules.first().expect("decision rule");
            let request = WorkflowDecisionAuthorizationRequest {
                project_id: guidance.project_id.clone(),
                policy_bundle_digest: guidance.bundle_digest.clone(),
                policy_ref: policy.id.clone(),
                decision_ref: rule.id.clone(),
                selected_alternative_ref: rule.alternatives[0].id.clone(),
                state_version: guidance.state_version,
                current_phase: StableId(guidance.current_phase.clone()),
                snapshot_digest: guidance.snapshot_digest.clone(),
                ledger_head_digest: guidance.ledger_head_digest.clone(),
                readiness_target: readiness_name(guidance.target),
                consequences_ack_digest: sha256_content_hash(
                    &serde_json_canonicalizer::to_vec(&rule.alternatives[0].consequences)
                        .expect("canonical decision consequences"),
                ),
            };
            fixture
                .adapter
                .record_authorized_decision(fixture.decision(request))
                .expect("signed product decision");
            decision_authorized = true;
            continue;
        }

        if policy.id.0 == "policy.workflow.write-spec" && !waiver_authorized {
            let claim = policy.claims.first().expect("waivable claim");
            let request = WorkflowWaiverAuthorizationRequest {
                project_id: guidance.project_id.clone(),
                policy_bundle_digest: guidance.bundle_digest.clone(),
                policy_ref: policy.id.clone(),
                subject: WorkflowWaiverSubject::Claim {
                    claim_ref: claim.id.clone(),
                },
                state_version: guidance.state_version,
                current_phase: StableId(guidance.current_phase.clone()),
                snapshot_digest: guidance.snapshot_digest.clone(),
                ledger_head_digest: guidance.ledger_head_digest.clone(),
                maximum_readiness_target: readiness_name(guidance.target),
                reason: "bounded golden-path waiver exercise".to_owned(),
                consequences_ack_digest: sha256_content_hash(b"waiver consequences accepted"),
                expires_at_unix: i64::try_from(now() + 3_600).expect("expiry"),
            };
            fixture
                .adapter
                .record_authorized_waiver(fixture.waiver(request))
                .expect("signed claim waiver");
            waiver_authorized = true;
            waived_claims.insert(claim.id.0.clone());
            continue;
        }

        if guidance.status == WorkflowGovernanceGuidanceStatus::Blocked {
            if let Some((boundary, gap)) = guidance.boundary_rechecks.iter().find_map(|boundary| {
                boundary
                    .simulation
                    .candidate_capability_gaps
                    .iter()
                    .find(|gap| gap.blocking)
                    .map(|gap| (boundary, gap))
            }) {
                let boundary_policy = document
                    .workflow_governance_bundle
                    .policies
                    .iter()
                    .find(|policy| policy.id == boundary.policy_ref)
                    .expect("boundary policy");
                let requirement = boundary_policy
                    .capability_requirements
                    .iter()
                    .find(|requirement| requirement.id == gap.id)
                    .expect("boundary capability");
                let observed = now();
                let request = WorkflowCapabilityAuthorizationRequest {
                    project_id: guidance.project_id.clone(),
                    policy_bundle_digest: guidance.bundle_digest.clone(),
                    policy_ref: boundary_policy.id.clone(),
                    capability_ref: requirement.id.clone(),
                    state_version: guidance.state_version,
                    current_phase: StableId(guidance.current_phase.clone()),
                    snapshot_digest: guidance.snapshot_digest.clone(),
                    ledger_head_digest: guidance.ledger_head_digest.clone(),
                    probe_kind: requirement.probe_kind,
                    available: true,
                    authority_scope: StableId("workflow.capability.authorize".to_owned()),
                    probe_ref: format!("runtime:boundary:{}", requirement.id.0),
                    probe_digest: sha256_content_hash(requirement.id.0.as_bytes()),
                    subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
                    subject_ref: guidance.project_id.0.clone(),
                    subject_digest: guidance.snapshot_digest.clone(),
                    observed_at_unix: observed,
                    expires_at_unix: Some(observed + 3_600),
                };
                fixture
                    .adapter
                    .record_authorized_capability(fixture.capability(request))
                    .expect("signed boundary capability refresh");
                continue;
            }

            if let Some((boundary, result)) =
                guidance.boundary_rechecks.iter().find_map(|boundary| {
                    boundary
                        .simulation
                        .candidate_claim_results
                        .iter()
                        .find(|result| {
                            !matches!(
                                result.status,
                                WorkflowClaimResultStatus::Verified
                                    | WorkflowClaimResultStatus::Waived
                            )
                        })
                        .map(|result| (boundary, result))
                })
            {
                let boundary_policy = document
                    .workflow_governance_bundle
                    .policies
                    .iter()
                    .find(|policy| policy.id == boundary.policy_ref)
                    .expect("boundary policy");
                let claim_ref = StableId(result.claim_id.clone());
                let observation = *observation_counts.get(&claim_ref.0).unwrap_or(&0);
                let mut request =
                    evidence_request(&guidance, boundary_policy, &claim_ref, observation);
                request.readiness_target = boundary.requested_target;
                fixture
                    .adapter
                    .record_authorized_evidence(fixture.evidence(request))
                    .unwrap_or_else(|error| {
                        panic!(
                            "signed boundary evidence for {} / {}: {error}",
                            boundary_policy.id.0, claim_ref.0
                        )
                    });
                observation_counts.insert(claim_ref.0, observation + 1);
                continue;
            }
        }

        if let Some(gap) = guidance
            .simulation
            .candidate_capability_gaps
            .iter()
            .find(|gap| gap.blocking)
        {
            let requirement = policy
                .capability_requirements
                .iter()
                .find(|requirement| requirement.id == gap.id)
                .expect("selected capability requirement");
            let observed = now();
            let request = WorkflowCapabilityAuthorizationRequest {
                project_id: guidance.project_id.clone(),
                policy_bundle_digest: guidance.bundle_digest.clone(),
                policy_ref: policy.id.clone(),
                capability_ref: requirement.id.clone(),
                state_version: guidance.state_version,
                current_phase: StableId(guidance.current_phase.clone()),
                snapshot_digest: guidance.snapshot_digest.clone(),
                ledger_head_digest: guidance.ledger_head_digest.clone(),
                probe_kind: requirement.probe_kind,
                available: true,
                authority_scope: StableId("workflow.capability.authorize".to_owned()),
                probe_ref: format!("runtime:{}", requirement.id.0),
                probe_digest: sha256_content_hash(requirement.id.0.as_bytes()),
                subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
                subject_ref: guidance.project_id.0.clone(),
                subject_digest: guidance.snapshot_digest.clone(),
                observed_at_unix: observed,
                expires_at_unix: Some(observed + 3_600),
            };
            fixture
                .adapter
                .record_authorized_capability(fixture.capability(request))
                .expect("signed required capability");
            continue;
        }

        if let Some(result) = guidance
            .simulation
            .candidate_claim_results
            .iter()
            .find(|result| {
                !matches!(
                    result.status,
                    WorkflowClaimResultStatus::Verified | WorkflowClaimResultStatus::Waived
                )
            })
        {
            let claim_ref = StableId(result.claim_id.clone());
            if claim_ref.0 == "claim.workflow.build-story.implementation-conforms"
                && !waived_claims.contains(&claim_ref.0)
            {
                let request = WorkflowWaiverAuthorizationRequest {
                    project_id: guidance.project_id.clone(),
                    policy_bundle_digest: guidance.bundle_digest.clone(),
                    policy_ref: policy.id.clone(),
                    subject: WorkflowWaiverSubject::Claim {
                        claim_ref: claim_ref.clone(),
                    },
                    state_version: guidance.state_version,
                    current_phase: StableId(guidance.current_phase.clone()),
                    snapshot_digest: guidance.snapshot_digest.clone(),
                    ledger_head_digest: guidance.ledger_head_digest.clone(),
                    maximum_readiness_target: "execute".to_owned(),
                    reason: "repository-inspector evaluator requires a stronger admitted path"
                        .to_owned(),
                    consequences_ack_digest: sha256_content_hash(
                        b"build-story inspection waiver consequences accepted",
                    ),
                    expires_at_unix: i64::try_from(now() + 3_600).expect("expiry"),
                };
                fixture
                    .adapter
                    .record_authorized_waiver(fixture.waiver(request))
                    .expect("signed build-story inspection waiver");
                waived_claims.insert(claim_ref.0);
                continue;
            }
            let observation = *observation_counts.get(&claim_ref.0).unwrap_or(&0);
            let request = evidence_request(&guidance, policy, &claim_ref, observation);
            fixture
                .adapter
                .record_authorized_evidence(fixture.evidence(request))
                .unwrap_or_else(|error| {
                    panic!(
                        "signed evaluator evidence for {} / {}: {error}",
                        policy.id.0, claim_ref.0
                    )
                });
            observation_counts.insert(claim_ref.0, observation + 1);
            continue;
        }

        assert_eq!(
            guidance.status,
            WorkflowGovernanceGuidanceStatus::ReadyToComplete,
            "simulation={:#?}\nboundary_rechecks={}",
            guidance.simulation,
            serde_json::to_string_pretty(&guidance.boundary_rechecks)
                .expect("serialize boundary rechecks")
        );
        let policy_id = guidance.selected_policy_ref.0.clone();
        let prepared = fixture
            .adapter
            .prepare_completion()
            .expect("prepare completion");
        let receipt = fixture
            .adapter
            .consume_completion(
                prepared,
                PrincipalId("principal.workflow.replacement-agent".to_owned()),
            )
            .expect("late-rechecked completion");
        assert!(receipt.completed_record.state_version > guidance.state_version);
        completed.push(policy_id);
    }

    panic!("signed golden path did not converge: {completed:?}");
}
