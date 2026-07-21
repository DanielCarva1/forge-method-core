use forge_core_contracts::gate::GateStatus;
use forge_core_contracts::request::RequestStatus;
use forge_core_contracts::{
    CoordinationMutationHandoff, CoordinationRequestState, CoordinationStateAppliedEvent,
    CoordinationStateRecord, Phase, PhaseAdvancedEvent, PostBuildVerifyAdmittedGateResult,
    PostBuildVerifyContinuityBinding, PostBuildVerifyEpisode, PostBuildVerifyEpisodeAppliedEvent,
    PostBuildVerifyEpisodeAuthority, PostBuildVerifyEpisodeDocument, PostBuildVerifyEpisodeOutcome,
    PostBuildVerifyEvolutionIdentity, PostBuildVerifyEvolutionStatus,
    PostBuildVerifyEvolutionTrigger, PostBuildVerifyGateKind, PostBuildVerifyPolicyReference,
    PostBuildVerifyPolicyRole, PostBuildVerifyRollbackBaseline, ProjectImportedEvent,
    ReleaseUpgradedEvent, RepoPath, RequestContractDocument, StableId,
    WorkflowContentAddressedReference, WorkflowGovernanceEvent, WorkflowGovernanceLedgerRecord,
    WorkflowGovernanceReceiptDocument, WorkflowGovernanceReleaseIdentity, WorkflowReceiptCarryover,
    WorkflowReleaseAdmissionProof, WorkflowReleaseRegistryProvenance,
    WorkflowRuntimeBundleIdentity, POST_BUILD_VERIFY_EPISODE_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_POST_BUILD_VERIFY_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_REPLACEMENT_CONTINUITY_LEDGER_SCHEMA_VERSION,
};
use forge_core_workflow_governance_tcb::{
    append_workflow_governance_event_tcb, initialize_workflow_governance_ledger_tcb,
    lock_workflow_governance_ledger_tcb, recover_workflow_governance_ledger,
    transition_workflow_governance_release_tcb, workflow_governance_record_digest,
    WorkflowGovernanceLedgerError, WorkflowGovernanceLedgerIdentity,
    WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES, WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn identity() -> WorkflowGovernanceLedgerIdentity {
    WorkflowGovernanceLedgerIdentity {
        project_id: id("project-alpha"),
        bundle_id: id("bundle-core"),
        bundle_digest: named_digest("bundle-core"),
    }
}

fn target_identity() -> WorkflowGovernanceLedgerIdentity {
    WorkflowGovernanceLedgerIdentity {
        project_id: id("project-alpha"),
        bundle_id: id("bundle-next"),
        bundle_digest: named_digest("bundle-next"),
    }
}

fn named_digest(value: &str) -> String {
    bytes_digest(value.as_bytes())
}

fn release_identity(version: &str) -> WorkflowGovernanceReleaseIdentity {
    WorkflowGovernanceReleaseIdentity {
        lineage_id: id("forge-core-governance"),
        release_id: id(&format!("release-{version}")),
        release_version: version.to_owned(),
        release_digest: named_digest(&format!("release-{version}")),
    }
}

fn runtime_identity(
    identity: &WorkflowGovernanceLedgerIdentity,
    policy_set: &str,
) -> WorkflowRuntimeBundleIdentity {
    WorkflowRuntimeBundleIdentity {
        bundle_id: identity.bundle_id.clone(),
        bundle_digest: identity.bundle_digest.clone(),
        policy_set_digest: named_digest(policy_set),
    }
}

fn release_upgraded(
    prior_head: &str,
    source: &WorkflowGovernanceLedgerIdentity,
    target: &WorkflowGovernanceLedgerIdentity,
) -> ReleaseUpgradedEvent {
    let from_runtime_bundle = runtime_identity(source, "policy-set-v1");
    let to_runtime_bundle = runtime_identity(target, "policy-set-v2");
    ReleaseUpgradedEvent {
        from_release: release_identity("1.0.0"),
        to_release: release_identity("2.0.0"),
        from_runtime_bundle: from_runtime_bundle.clone(),
        to_runtime_bundle: to_runtime_bundle.clone(),
        registry_provenance: WorkflowReleaseRegistryProvenance {
            registry_id: id("release-registry"),
            registry_version: "1.0.0".to_owned(),
            registry_digest: named_digest("release-registry"),
        },
        admission_proof: WorkflowReleaseAdmissionProof {
            proof_id: id("release-admission-proof"),
            proof_digest: named_digest("release-admission-proof"),
            snapshot_digest: named_digest("release-snapshot"),
            from_policy_set_digest: from_runtime_bundle.policy_set_digest,
            to_policy_set_digest: to_runtime_bundle.policy_set_digest,
        },
        receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
        prior_ledger_head_digest: prior_head.to_owned(),
    }
}

fn imported() -> WorkflowGovernanceEvent {
    WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
        source_ref: "project/state.yaml".to_owned(),
        source_digest: "sha256:source".to_owned(),
        snapshot_digest: "sha256:snapshot-0".to_owned(),
        initial_phase: id("discover"),
    })
}

fn advanced_between(from_phase: &str, to_phase: &str, index: usize) -> WorkflowGovernanceEvent {
    WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
        from_phase: Some(id(from_phase)),
        to_phase: id(to_phase),
        snapshot_digest: format!("sha256:snapshot-{index}"),
    })
}

fn advanced(index: usize) -> WorkflowGovernanceEvent {
    advanced_between("discover", "define", index)
}

fn post_build_verify_imported() -> WorkflowGovernanceEvent {
    WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
        source_ref: "project/state.yaml".to_owned(),
        source_digest: named_digest("source"),
        snapshot_digest: named_digest("snapshot"),
        initial_phase: id("4-build-verify"),
    })
}

fn episode_reference(name: &str, digest: String) -> WorkflowContentAddressedReference {
    WorkflowContentAddressedReference {
        subject_ref: name.to_owned(),
        subject_digest: digest,
    }
}

fn episode_policies() -> Vec<PostBuildVerifyPolicyReference> {
    [
        (PostBuildVerifyPolicyRole::Readiness, "release-readiness"),
        (PostBuildVerifyPolicyRole::ReadyRelease, "ready-release"),
        (
            PostBuildVerifyPolicyRole::RealityEvidence,
            "reality-evidence",
        ),
        (
            PostBuildVerifyPolicyRole::ContextRecovery,
            "context-recovery",
        ),
        (PostBuildVerifyPolicyRole::EvolveProject, "evolve-project"),
    ]
    .into_iter()
    .map(|(role, name)| PostBuildVerifyPolicyReference {
        role,
        policy_id: id(name),
        policy_ref: RepoPath(format!("contracts/{name}.yaml")),
    })
    .collect()
}

fn sync_episode_snapshot(event: &mut PostBuildVerifyEpisodeAppliedEvent) {
    let release_digest = event.release_subject.release_digest.clone();
    let snapshot = episode_reference("build-verify/current", event.snapshot_digest.clone());
    let mut document = PostBuildVerifyEpisodeDocument {
        schema_version: POST_BUILD_VERIFY_EPISODE_SCHEMA_VERSION.to_owned(),
        post_build_verify_episode: PostBuildVerifyEpisode {
            episode_id: event.episode_id.clone(),
            generation: event.generation,
            previous_episode_digest: event.previous_episode_digest.clone(),
            authority: PostBuildVerifyEpisodeAuthority::CandidateOnly,
            release_subject: event.release_subject.clone(),
            build_verify_snapshot: snapshot.clone(),
            rollback_baseline: PostBuildVerifyRollbackBaseline::BuildVerifySnapshot { snapshot },
            policy_references: episode_policies(),
            deployment_observations: Vec::new(),
            operational_evidence: Vec::new(),
            feedback: Vec::new(),
            intake: Vec::new(),
            evolution: PostBuildVerifyEvolutionIdentity {
                evolution_episode_id: id("evolution.release.current"),
                generation: event.generation,
                release_digest,
                status: PostBuildVerifyEvolutionStatus::Dormant,
                trigger: PostBuildVerifyEvolutionTrigger::PlannedFollowUp,
                proposed_entry_phase: Phase::Plan,
                continuity_subject: episode_reference(
                    "continuity/evolution.current",
                    named_digest("continuity-evolution"),
                ),
            },
            continuity: PostBuildVerifyContinuityBinding {
                context_recovery_subject: episode_reference(
                    "recovery/release.current",
                    named_digest("context-recovery"),
                ),
                next_action_ref: id("action.monitor-release"),
            },
            episode_digest: String::new(),
        },
    };
    let digest = document.episode_digest().expect("episode canonicalizes");
    document
        .post_build_verify_episode
        .episode_digest
        .clone_from(&digest);
    event.episode_digest = digest;
    event.episode_snapshot = Some(document);
}

fn post_build_verify_event(head: &str, state_version: u64) -> PostBuildVerifyEpisodeAppliedEvent {
    let mut event = PostBuildVerifyEpisodeAppliedEvent {
        episode_id: id("episode.release.current"),
        generation: 1,
        previous_episode_digest: None,
        episode_digest: named_digest("episode"),
        release_subject: release_identity("1.0.0"),
        decision_digest: named_digest("decision"),
        from_phase: id("4-build-verify"),
        to_phase: Some(id("5-ready-operate")),
        outcome: PostBuildVerifyEpisodeOutcome::AdvancedToReadyOperate,
        snapshot_digest: named_digest("snapshot"),
        prior_ledger_head_digest: head.to_owned(),
        prior_state_version: state_version,
        admitted_gate: Some(PostBuildVerifyAdmittedGateResult {
            kind: PostBuildVerifyGateKind::Readiness,
            status: GateStatus::Pass,
            effective_bundle_digest: named_digest("effective-bundle"),
        }),
        episode_snapshot: None,
    };
    sync_episode_snapshot(&mut event);
    event
}

fn coordination_request_document() -> RequestContractDocument {
    yaml_serde::from_str(include_str!(
        "../../../contracts/requests/worker-state-transition-request.yaml"
    ))
    .expect("request fixture")
}

fn coordination_request_state(
    status: RequestStatus,
    previous_status: Option<RequestStatus>,
    actor: &str,
) -> CoordinationStateRecord {
    let mut request = coordination_request_document();
    request.request_contract.status = status;
    CoordinationStateRecord::Request(CoordinationRequestState {
        request,
        previous_status,
        actor_agent_id: id(actor),
        response_evidence_refs: vec!["contracts/gates/story-ready-lane-gate.yaml".to_owned()],
        mutation_handoff: None,
    })
}

fn coordination_event(
    head: &str,
    state_version: u64,
    state: CoordinationStateRecord,
) -> CoordinationStateAppliedEvent {
    CoordinationStateAppliedEvent {
        prior_ledger_head_digest: head.to_owned(),
        prior_state_version: state_version,
        state,
    }
}

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-governance-ledger-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create test root");
    root
}

fn wal_path(root: &Path) -> PathBuf {
    root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)
}

fn replacement_paths(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let parent = wal_path(root).parent().expect("WAL parent").to_path_buf();
    (
        parent.join(".workflow-governance.ndjson.forge-next"),
        parent.join(".workflow-governance.ndjson.forge-previous"),
        parent.join(".workflow-governance.ndjson.forge-transaction"),
    )
}

fn bytes_digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    format!("sha256:{hex}")
}

fn marker_bytes(previous: Option<&[u8]>, next: &[u8]) -> Vec<u8> {
    let previous = previous.map_or_else(|| "absent".to_owned(), bytes_digest);
    format!(
        "forge-wal-replacement-v1\nprevious={previous}\nnext={}\n",
        bytes_digest(next)
    )
    .into_bytes()
}

fn assert_protocol_artifacts_absent(root: &Path) {
    let (next, previous, transaction) = replacement_paths(root);
    for path in [next, previous, transaction] {
        assert!(
            fs::symlink_metadata(path).is_err(),
            "reconciled protocol artifact must be absent"
        );
    }
}

fn read_documents(root: &Path) -> Vec<WorkflowGovernanceReceiptDocument> {
    fs::read_to_string(wal_path(root))
        .expect("read ledger")
        .lines()
        .map(|line| serde_json::from_str(line).expect("parse receipt"))
        .collect()
}

fn write_documents(root: &Path, documents: &[WorkflowGovernanceReceiptDocument]) {
    let mut bytes = Vec::new();
    for document in documents {
        bytes.extend(serde_json::to_vec(document).expect("encode receipt"));
        bytes.push(b'\n');
    }
    fs::write(wal_path(root), bytes).expect("rewrite ledger");
}

#[test]
fn published_all_events_fixture_has_a_valid_canonical_hash_chain() {
    let raw = include_str!(
        "../../../docs/fixtures/workflow-governance-golden-path-v0/ledger-all-events.yaml"
    );
    let document: forge_core_contracts::WorkflowGovernanceLedgerDocument =
        yaml_serde::from_str(raw).expect("published ledger fixture");
    let mut expected_previous: Option<String> = None;
    for record in &document.workflow_governance_ledger.records {
        assert!(
            !matches!(record.event, WorkflowGovernanceEvent::ReleaseUpgraded(_)),
            "the published P5c fixture must remain byte/hash compatible"
        );
        assert_eq!(record.previous_record_digest, expected_previous);
        assert_eq!(
            workflow_governance_record_digest(record).expect("canonical record digest"),
            record.record_digest
        );
        expected_previous = Some(record.record_digest.clone());
    }
}

#[test]
fn initialize_append_and_recover_ordered_chain() {
    let root = temp_root("init-recover");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    assert_eq!(first.sequence, 1);
    assert_eq!(first.previous_record_digest, None);
    assert!(!first.record_id.0.is_empty());
    assert!(!first.record_digest.is_empty());

    let second = append_workflow_governance_event_tcb(
        &root,
        &first.record_digest,
        &identity(),
        1,
        advanced(1),
    )
    .expect("append");
    let projection = recover_workflow_governance_ledger(&root).expect("recover");
    assert_eq!(projection.records, vec![first.clone(), second.clone()]);
    assert_eq!(
        projection.head_digest.as_deref(),
        Some(second.record_digest.as_str())
    );
    assert_eq!(projection.next_sequence, 3);
    assert_eq!(projection.next_state_version, 2);
    assert!(root.join("locks/workflow-governance.lock").is_file());
    fs::remove_dir_all(root).expect("cleanup");

    let blocked = temp_root("init-atomic-failure");
    fs::create_dir_all(wal_path(&blocked)).expect("block WAL target with directory");
    assert!(
        initialize_workflow_governance_ledger_tcb(&blocked, &identity(), 0, imported()).is_err()
    );
    assert!(
        wal_path(&blocked).is_dir(),
        "failed initialization must not replace the target"
    );
    let wal_parent = wal_path(&blocked)
        .parent()
        .expect("WAL parent")
        .to_path_buf();
    assert!(
        fs::read_dir(wal_parent)
            .expect("read WAL parent")
            .all(|entry| {
                let name = entry.expect("directory entry").file_name();
                let name = name.to_string_lossy();
                !name.contains(".forge-tmp") && !name.contains(".forge-bak")
            }),
        "failed initialization must not leave replacement artifacts"
    );
    fs::remove_dir_all(blocked).expect("cleanup blocked initialization");
}

#[test]
fn retained_guard_snapshots_exact_wal_and_projection_without_relocking() {
    let root = temp_root("raw-snapshot");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let guard = lock_workflow_governance_ledger_tcb(&root).expect("retain producer lock");

    let snapshot = guard.snapshot_raw_wal().expect("snapshot exact WAL");
    assert_eq!(
        snapshot.raw_wal_bytes().expect("present WAL"),
        fs::read(wal_path(&root)).expect("read WAL").as_slice()
    );
    assert_eq!(snapshot.projection().records, vec![first]);
    assert!(matches!(
        lock_workflow_governance_ledger_tcb(&root),
        Err(WorkflowGovernanceLedgerError::Lock { .. })
    ));

    drop(guard);
    lock_workflow_governance_ledger_tcb(&root)
        .expect("dropping retained guard releases producer lock");
    fs::remove_dir_all(root).expect("cleanup");
}
#[cfg(unix)]
#[test]
fn raw_wal_snapshot_rejects_renamed_symlink_namespace_substitution() {
    let root = temp_root("raw-wal-symlink");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let guard = lock_workflow_governance_ledger_tcb(&root).expect("retain producer lock");
    let path = wal_path(&root);
    let moved = path.with_extension("ndjson.moved");
    fs::rename(&path, &moved).expect("rename WAL");
    std::os::unix::fs::symlink(&moved, &path).expect("substitute WAL symlink");

    guard
        .snapshot_raw_wal()
        .expect_err("no-follow snapshot must reject WAL symlink substitution");
    fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn raw_wal_snapshot_rejects_state_root_rename_and_replacement() {
    let root = temp_root("raw-wal-root-swap");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize original WAL");
    let replacement = temp_root("raw-wal-root-replacement");
    initialize_workflow_governance_ledger_tcb(&replacement, &identity(), 0, imported())
        .expect("initialize replacement WAL");
    let guard = lock_workflow_governance_ledger_tcb(&root).expect("pin original root");
    let moved = root.with_extension("moved");
    fs::rename(&root, &moved).expect("rename guarded root");
    fs::rename(&replacement, &root).expect("install well-formed replacement root");

    guard
        .snapshot_raw_wal()
        .expect_err("snapshot must reject a replacement root outside the retained lock namespace");
    drop(guard);
    fs::remove_dir_all(root).expect("remove replacement root");
    fs::remove_dir_all(moved).expect("remove original moved root");
}
#[test]
fn batch_commits_two_events_together_and_exposes_prepared_projection() {
    let root = temp_root("batch-two-events");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let original_wal = fs::read(wal_path(&root)).expect("read original WAL");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");

    let mut batch = ledger
        .begin_unchecked_tcb_batch(&first.record_digest, &identity())
        .expect("begin batch");
    let second = batch.push_event(1, advanced(1)).expect("prepare second");
    assert_eq!(batch.projection().records.len(), 2);
    assert_eq!(batch.projection().head_digest, Some(second.record_digest));
    let third = batch
        .push_event(2, advanced_between("define", "plan", 2))
        .expect("prepare third");
    assert_eq!(batch.projection().records.len(), 3);
    assert_eq!(batch.projection().head_digest, Some(third.record_digest));
    assert_eq!(
        fs::read(wal_path(&root)).expect("read uncommitted WAL"),
        original_wal,
        "preparation must not change durable bytes"
    );

    let committed = batch.commit().expect("commit batch");
    assert_eq!(committed.len(), 2);
    let projection = ledger.recover().expect("recover committed batch");
    assert_eq!(projection.records.len(), 3);
    assert_eq!(&projection.records[1..], committed.as_slice());
    drop(ledger);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_transition_keeps_source_envelope_then_activates_target_for_next_append() {
    let root = temp_root("release-transition-success");
    let source = identity();
    let target = target_identity();
    let first = initialize_workflow_governance_ledger_tcb(&root, &source, 0, imported())
        .expect("initialize");
    let transition = transition_workflow_governance_release_tcb(
        &root,
        &first.record_digest,
        &source,
        &target,
        1,
        release_upgraded(&first.record_digest, &source, &target),
    )
    .expect("transition release");
    assert_eq!(transition.bundle_id, source.bundle_id);
    assert_eq!(transition.bundle_digest, source.bundle_digest);

    let after_transition = recover_workflow_governance_ledger(&root).expect("recover transition");
    assert_eq!(after_transition.genesis_identity(), Some(source.clone()));
    assert_eq!(after_transition.identity(), Some(source.clone()));
    assert_eq!(after_transition.active_identity(), Some(target.clone()));
    assert_eq!(
        after_transition.active_runtime_bundle_identity(),
        Some(runtime_identity(&target, "policy-set-v2"))
    );
    assert!(matches!(
        append_workflow_governance_event_tcb(
            &root,
            &transition.record_digest,
            &source,
            2,
            advanced(2)
        ),
        Err(WorkflowGovernanceLedgerError::BundleMismatch { .. })
    ));
    let next = append_workflow_governance_event_tcb(
        &root,
        &transition.record_digest,
        &target,
        2,
        advanced(2),
    )
    .expect("append under target identity");
    let projection = recover_workflow_governance_ledger(&root).expect("recover target append");
    assert_eq!(projection.records, vec![first, transition, next]);
    assert_eq!(projection.active_identity(), Some(target));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn generic_append_and_batch_reject_caller_injected_release_event() {
    let root = temp_root("release-transition-generic-reject");
    let source = identity();
    let target = target_identity();
    let first = initialize_workflow_governance_ledger_tcb(&root, &source, 0, imported())
        .expect("initialize");
    let original = fs::read(wal_path(&root)).expect("read source WAL");
    let event = release_upgraded(&first.record_digest, &source, &target);
    assert!(matches!(
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &source,
            1,
            WorkflowGovernanceEvent::ReleaseUpgraded(event.clone())
        ),
        Err(WorkflowGovernanceLedgerError::ReleaseUpgradeRequiresDedicatedAuthority)
    ));

    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");
    let mut batch = ledger
        .begin_unchecked_tcb_batch(&first.record_digest, &source)
        .expect("begin batch");
    for _ in 0..2 {
        assert!(matches!(
            batch.push_event(1, WorkflowGovernanceEvent::ReleaseUpgraded(event.clone())),
            Err(WorkflowGovernanceLedgerError::ReleaseUpgradeRequiresDedicatedAuthority)
        ));
    }
    drop(batch);
    drop(ledger);
    assert_eq!(fs::read(wal_path(&root)).expect("unchanged WAL"), original);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_transition_rejects_stale_self_reverse_and_non_contiguous_inputs_without_commit() {
    let source = identity();
    let target = target_identity();
    for scenario in ["stale", "self", "reverse", "state"] {
        let root = temp_root(&format!("release-{scenario}"));
        let first = initialize_workflow_governance_ledger_tcb(&root, &source, 0, imported())
            .expect("initialize");
        let original = fs::read(wal_path(&root)).expect("read source WAL");
        let mut event = release_upgraded(&first.record_digest, &source, &target);
        let (head, requested_target, state_version) = match scenario {
            "stale" => (named_digest("stale-head"), target.clone(), 1),
            "self" => {
                event.to_release = event.from_release.clone();
                event.to_runtime_bundle = event.from_runtime_bundle.clone();
                event.admission_proof.to_policy_set_digest =
                    event.from_runtime_bundle.policy_set_digest.clone();
                (first.record_digest.clone(), source.clone(), 1)
            }
            "reverse" => {
                std::mem::swap(&mut event.from_runtime_bundle, &mut event.to_runtime_bundle);
                (first.record_digest.clone(), target.clone(), 1)
            }
            "state" => (first.record_digest.clone(), target.clone(), 2),
            _ => unreachable!(),
        };
        let error = transition_workflow_governance_release_tcb(
            &root,
            &head,
            &source,
            &requested_target,
            state_version,
            event,
        )
        .expect_err("transition must reject malformed input");
        assert!(matches!(
            (scenario, error),
            ("stale", WorkflowGovernanceLedgerError::HeadMismatch { .. })
                | (
                    "self" | "reverse",
                    WorkflowGovernanceLedgerError::ReleaseTransitionInvalid { .. }
                )
                | (
                    "state",
                    WorkflowGovernanceLedgerError::ReleaseTransitionStateVersionMismatch { .. }
                )
        ));
        assert_eq!(
            fs::read(wal_path(&root)).expect("source WAL preserved"),
            original,
            "failure before commit must preserve the source WAL"
        );
        assert_eq!(
            recover_workflow_governance_ledger(&root)
                .expect("recover source")
                .active_identity(),
            Some(source.clone())
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn recovery_rejects_rehashed_release_event_with_tampered_head_or_binding() {
    for scenario in ["head", "from", "policy"] {
        let root = temp_root(&format!("release-tamper-{scenario}"));
        let source = identity();
        let target = target_identity();
        let first = initialize_workflow_governance_ledger_tcb(&root, &source, 0, imported())
            .expect("initialize");
        transition_workflow_governance_release_tcb(
            &root,
            &first.record_digest,
            &source,
            &target,
            1,
            release_upgraded(&first.record_digest, &source, &target),
        )
        .expect("transition");
        let mut documents = read_documents(&root);
        let WorkflowGovernanceEvent::ReleaseUpgraded(event) =
            &mut documents[1].workflow_governance_receipt.event
        else {
            panic!("second record must be release_upgraded")
        };
        match scenario {
            "head" => event.prior_ledger_head_digest = named_digest("wrong-head"),
            "from" => event.from_runtime_bundle.bundle_id = id("reversed-source"),
            "policy" => event.admission_proof.from_policy_set_digest = named_digest("wrong-policy"),
            _ => unreachable!(),
        }
        documents[1].workflow_governance_receipt.record_digest =
            workflow_governance_record_digest(&documents[1].workflow_governance_receipt)
                .expect("rehash tampered transition");
        write_documents(&root, &documents);
        assert!(matches!(
            recover_workflow_governance_ledger(&root),
            Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid { .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn recovery_restores_marker_bound_previous_wal_when_target_is_missing() {
    let root = temp_root("replacement-restore-previous");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let target = wal_path(&root);
    let old = fs::read(&target).expect("read old WAL");
    let candidate = b"candidate bytes are discarded during rollback\n";
    let (next, previous, transaction) = replacement_paths(&root);
    fs::rename(&target, &previous).expect("simulate installed previous WAL");
    fs::write(&next, candidate).expect("simulate synced next WAL");
    fs::write(&transaction, marker_bytes(Some(&old), candidate)).expect("write marker");

    let projection = recover_workflow_governance_ledger(&root).expect("restore old WAL");
    assert_eq!(projection.records, vec![first]);
    assert_eq!(fs::read(&target).expect("restored WAL"), old);
    assert_protocol_artifacts_absent(&root);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_accepts_marker_bound_committed_wal_and_cleans_residue() {
    let root = temp_root("replacement-clean-committed");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let target = wal_path(&root);
    let old = fs::read(&target).expect("read old WAL");
    let second = append_workflow_governance_event_tcb(
        &root,
        &first.record_digest,
        &identity(),
        1,
        advanced(1),
    )
    .expect("append");
    let committed = fs::read(&target).expect("read committed WAL");
    let (_, previous, transaction) = replacement_paths(&root);
    fs::write(&previous, &old).expect("simulate previous cleanup residue");
    fs::write(&transaction, marker_bytes(Some(&old), &committed)).expect("write marker");

    let projection = recover_workflow_governance_ledger(&root).expect("accept committed WAL");
    assert_eq!(projection.records, vec![first, second]);
    assert_eq!(fs::read(&target).expect("committed WAL"), committed);
    assert_protocol_artifacts_absent(&root);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_fails_closed_on_corrupt_ambiguous_or_non_regular_protocol_state() {
    for scenario in [
        "previous-without-marker",
        "corrupt-marker",
        "multiple",
        "directory",
    ] {
        let root = temp_root(scenario);
        initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
            .expect("initialize");
        let target = wal_path(&root);
        let old = fs::read(&target).expect("read old WAL");
        let (next, previous, transaction) = replacement_paths(&root);
        match scenario {
            "previous-without-marker" => fs::write(&previous, &old).expect("write previous"),
            "corrupt-marker" => fs::write(&transaction, b"torn marker").expect("write marker"),
            "multiple" => {
                fs::write(&next, b"candidate\n").expect("write next");
                fs::write(&previous, &old).expect("write previous");
                fs::write(&transaction, marker_bytes(Some(&old), b"candidate\n"))
                    .expect("write marker");
            }
            "directory" => fs::create_dir(&next).expect("create non-regular next path"),
            _ => unreachable!(),
        }

        let error = recover_workflow_governance_ledger(&root).expect_err("must fail closed");
        assert!(
            matches!(error, WorkflowGovernanceLedgerError::Io { .. }),
            "unexpected error for {scenario}: {error:?}"
        );
        assert_eq!(
            fs::read(&target).expect("old WAL remains"),
            old,
            "failed reconciliation must not rewrite the authoritative target"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[cfg(unix)]
#[test]
fn recovery_rejects_symlinked_protocol_artifact() {
    use std::os::unix::fs::symlink;

    let root = temp_root("replacement-symlink");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let (next, _, _) = replacement_paths(&root);
    symlink(wal_path(&root), next).expect("create protocol symlink");
    assert!(matches!(
        recover_workflow_governance_ledger(&root),
        Err(WorkflowGovernanceLedgerError::Io { .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn batch_error_on_second_event_does_not_persist_first() {
    let root = temp_root("batch-second-error");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 2, imported())
        .expect("initialize");
    let original_wal = fs::read(wal_path(&root)).expect("read original WAL");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");
    {
        let mut batch = ledger
            .begin_unchecked_tcb_batch(&first.record_digest, &identity())
            .expect("begin batch");
        batch.push_event(3, advanced(1)).expect("prepare first");
        assert!(matches!(
            batch.push_event(1, advanced(2)),
            Err(WorkflowGovernanceLedgerError::StateVersionRegression {
                previous: 3,
                found: 1
            })
        ));
        assert_eq!(batch.projection().records.len(), 2);
    }
    assert_eq!(
        fs::read(wal_path(&root)).expect("read WAL after dropped batch"),
        original_wal
    );
    assert_eq!(ledger.recover().expect("recover").records.len(), 1);
    drop(ledger);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn batch_stale_head_fails_before_any_wal_mutation() {
    let root = temp_root("batch-stale-head");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let original_wal = fs::read(wal_path(&root)).expect("read original WAL");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");
    assert!(matches!(
        ledger.begin_unchecked_tcb_batch("sha256:stale", &identity()),
        Err(WorkflowGovernanceLedgerError::HeadMismatch { .. })
    ));
    assert_eq!(
        fs::read(wal_path(&root)).expect("read WAL after rejected batch"),
        original_wal
    );
    drop(ledger);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn batch_capacity_preflight_leaves_wal_unchanged() {
    let root = temp_root("batch-capacity-preflight");
    let mut near_capacity_import = imported();
    let WorkflowGovernanceEvent::ProjectImported(event) = &mut near_capacity_import else {
        unreachable!("imported helper must return project_imported")
    };
    event.source_ref = "x".repeat(
        usize::try_from(WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES).expect("capacity fits usize") - 2_048,
    );
    let first =
        initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, near_capacity_import)
            .expect("initialize near byte capacity");
    let original_wal = fs::read(wal_path(&root)).expect("read original WAL");
    assert!(
        WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES
            - u64::try_from(original_wal.len()).expect("WAL length fits u64")
            < 8_192,
        "fixture must leave less space than the prepared event requires"
    );
    let mut oversized_event = advanced(1);
    let WorkflowGovernanceEvent::PhaseAdvanced(event) = &mut oversized_event else {
        unreachable!("advanced helper must return phase_advanced")
    };
    event.snapshot_digest = "y".repeat(8_192);

    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");
    let mut batch = ledger
        .begin_unchecked_tcb_batch(&first.record_digest, &identity())
        .expect("begin batch");
    assert!(matches!(
        batch.push_event(1, oversized_event),
        Err(WorkflowGovernanceLedgerError::CapacityBytes { .. })
    ));
    assert_eq!(batch.projection().records.len(), 1);
    assert!(matches!(
        batch.commit(),
        Err(WorkflowGovernanceLedgerError::EmptyBatch)
    ));
    assert_eq!(
        fs::read(wal_path(&root)).expect("read WAL after capacity rejection"),
        original_wal
    );
    drop(ledger);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn digest_is_deterministic_and_ignores_only_record_digest() {
    let root = temp_root("digest");
    let record = initialize_workflow_governance_ledger_tcb(&root, &identity(), 7, imported())
        .expect("initialize");
    let expected = workflow_governance_record_digest(&record).expect("digest");
    let mut with_other_stored_digest = record.clone();
    with_other_stored_digest.record_digest = "untrusted-cache".to_owned();
    assert_eq!(
        workflow_governance_record_digest(&with_other_stored_digest).expect("digest"),
        expected
    );
    with_other_stored_digest.state_version += 1;
    assert_ne!(
        workflow_governance_record_digest(&with_other_stored_digest).expect("digest"),
        expected
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_rejects_payload_tamper() {
    let root = temp_root("tamper");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let mut documents = read_documents(&root);
    if let WorkflowGovernanceEvent::ProjectImported(event) =
        &mut documents[0].workflow_governance_receipt.event
    {
        event.source_ref = "attacker.yaml".to_owned();
    }
    write_documents(&root, &documents);
    assert!(matches!(
        recover_workflow_governance_ledger(&root),
        Err(WorkflowGovernanceLedgerError::RecordDigestMismatch { line: 1, .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_rejects_torn_blank_malformed_and_unknown_schema_lines() {
    for (name, mutation, expected) in [
        ("torn", "torn", "torn"),
        ("blank", "blank", "blank"),
        ("malformed", "malformed", "malformed"),
        ("schema", "schema", "schema"),
    ] {
        let root = temp_root(name);
        initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
            .expect("initialize");
        let path = wal_path(&root);
        match mutation {
            "torn" => {
                let mut bytes = fs::read(&path).expect("read");
                assert_eq!(bytes.pop(), Some(b'\n'));
                fs::write(&path, bytes).expect("write");
            }
            "blank" => fs::write(&path, b" \t\n").expect("write"),
            "malformed" => fs::write(&path, b"{not-json}\n").expect("write"),
            "schema" => {
                let mut docs = read_documents(&root);
                docs[0].schema_version = "999".to_owned();
                write_documents(&root, &docs);
            }
            _ => unreachable!(),
        }
        let error = recover_workflow_governance_ledger(&root).expect_err("must reject");
        assert!(
            matches!(
                (&error, expected),
                (WorkflowGovernanceLedgerError::TornTail { .. }, "torn")
                    | (WorkflowGovernanceLedgerError::BlankLine { .. }, "blank")
                    | (
                        WorkflowGovernanceLedgerError::MalformedRecord { .. },
                        "malformed"
                    )
                    | (
                        WorkflowGovernanceLedgerError::UnsupportedSchema { .. },
                        "schema"
                    )
            ),
            "unexpected {error:?}"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn recovery_rejects_sequence_gap_and_wrong_previous_digest() {
    for wrong_sequence in [true, false] {
        let root = temp_root(if wrong_sequence { "gap" } else { "previous" });
        let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
            .expect("initialize");
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &identity(),
            1,
            advanced(1),
        )
        .expect("append");
        let mut docs = read_documents(&root);
        if wrong_sequence {
            docs[1].workflow_governance_receipt.sequence = 3;
        } else {
            docs[1].workflow_governance_receipt.previous_record_digest =
                Some("sha256:wrong".to_owned());
        }
        write_documents(&root, &docs);
        let error = recover_workflow_governance_ledger(&root).expect_err("must reject");
        assert!(if wrong_sequence {
            matches!(error, WorkflowGovernanceLedgerError::SequenceGap { .. })
        } else {
            matches!(
                error,
                WorkflowGovernanceLedgerError::PreviousDigestMismatch { .. }
            )
        });
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn append_rejects_stale_head_state_rollback_and_identity_mismatch() {
    let root = temp_root("cas-identity");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 2, imported())
        .expect("initialize");
    assert!(matches!(
        append_workflow_governance_event_tcb(&root, "sha256:stale", &identity(), 2, advanced(1)),
        Err(WorkflowGovernanceLedgerError::HeadMismatch { .. })
    ));
    assert!(matches!(
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &identity(),
            1,
            advanced(2)
        ),
        Err(WorkflowGovernanceLedgerError::StateVersionRegression { .. })
    ));

    let mut wrong_project = identity();
    wrong_project.project_id = id("other-project");
    assert!(matches!(
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &wrong_project,
            2,
            advanced(3)
        ),
        Err(WorkflowGovernanceLedgerError::ProjectMismatch { .. })
    ));
    let mut wrong_bundle = identity();
    wrong_bundle.bundle_digest = "sha256:other-bundle".to_owned();
    assert!(matches!(
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &wrong_bundle,
            2,
            advanced(4)
        ),
        Err(WorkflowGovernanceLedgerError::BundleMismatch { .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_rejects_duplicate_record_id_after_valid_rehash() {
    let root = temp_root("duplicate");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    append_workflow_governance_event_tcb(&root, &first.record_digest, &identity(), 1, advanced(1))
        .expect("append");
    let mut docs = read_documents(&root);
    docs[1].workflow_governance_receipt.record_id =
        docs[0].workflow_governance_receipt.record_id.clone();
    docs[1].workflow_governance_receipt.record_digest =
        workflow_governance_record_digest(&docs[1].workflow_governance_receipt).expect("rehash");
    write_documents(&root, &docs);
    assert!(matches!(
        recover_workflow_governance_ledger(&root),
        Err(WorkflowGovernanceLedgerError::DuplicateRecordId { line: 2, .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_rejects_project_bundle_and_state_regression_even_when_rehashed() {
    for mutation in ["project", "bundle", "state"] {
        let root = temp_root(mutation);
        let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 2, imported())
            .expect("initialize");
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &identity(),
            3,
            advanced(1),
        )
        .expect("append");
        let mut docs = read_documents(&root);
        match mutation {
            "project" => docs[1].workflow_governance_receipt.project_id = id("evil"),
            "bundle" => docs[1].workflow_governance_receipt.bundle_id = id("evil-bundle"),
            "state" => docs[1].workflow_governance_receipt.state_version = 1,
            _ => unreachable!(),
        }
        docs[1].workflow_governance_receipt.record_digest =
            workflow_governance_record_digest(&docs[1].workflow_governance_receipt)
                .expect("rehash");
        write_documents(&root, &docs);
        let error = recover_workflow_governance_ledger(&root).expect_err("must reject");
        assert!(matches!(
            (mutation, error),
            (
                "project",
                WorkflowGovernanceLedgerError::ProjectMismatch { .. }
            ) | (
                "bundle",
                WorkflowGovernanceLedgerError::BundleMismatch { .. }
            ) | (
                "state",
                WorkflowGovernanceLedgerError::StateVersionRegression { .. }
            )
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn generic_append_cannot_smuggle_a_post_build_verify_episode() {
    let root = temp_root("episode-generic-authority");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, post_build_verify_imported())
        .expect("initialize");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock");
    let projection = ledger.recover().expect("recover");
    let head = projection.head_digest.expect("head");
    let event = post_build_verify_event(&head, 0);

    assert!(matches!(
        ledger.append_unchecked_tcb_event(
            &head,
            &identity(),
            1,
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event),
        ),
        Err(WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeRequiresDedicatedAuthority)
    ));
    drop(ledger);
    assert_eq!(
        recover_workflow_governance_ledger(&root)
            .expect("recover unchanged ledger")
            .records
            .len(),
        1
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn dedicated_episode_route_starts_and_retains_the_zero_eight_epoch() {
    let root = temp_root("episode-zero-eight");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, post_build_verify_imported())
        .expect("initialize");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock");
    let projection = ledger.recover().expect("recover");
    let head = projection.head_digest.expect("head");
    let event = post_build_verify_event(&head, 0);
    let record = ledger
        .apply_post_build_verify_episode_unchecked_tcb(&head, &identity(), 1, event.clone())
        .expect("apply dedicated episode route");
    assert_eq!(
        record.event,
        WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event)
    );

    let successor = WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
        from_phase: Some(id("5-ready-operate")),
        to_phase: id("6-evolve"),
        snapshot_digest: named_digest("snapshot-evolve"),
    });
    ledger
        .append_unchecked_tcb_event(&record.record_digest, &identity(), 2, successor)
        .expect("append successor in retained epoch");
    drop(ledger);

    let documents = read_documents(&root);
    assert_eq!(documents.len(), 3);
    assert_eq!(
        documents[1].schema_version,
        WORKFLOW_GOVERNANCE_REPLACEMENT_CONTINUITY_LEDGER_SCHEMA_VERSION
    );
    assert_eq!(
        documents[2].schema_version,
        WORKFLOW_GOVERNANCE_REPLACEMENT_CONTINUITY_LEDGER_SCHEMA_VERSION
    );
    recover_workflow_governance_ledger(&root).expect("recover 0.8 history");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn episode_route_rejects_stale_bindings_malformed_gate_and_broken_generation() {
    let root = temp_root("episode-invalid");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, post_build_verify_imported())
        .expect("initialize");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock");
    let projection = ledger.recover().expect("recover");
    let head = projection.head_digest.expect("head");

    assert!(matches!(
        ledger.apply_post_build_verify_episode_unchecked_tcb(
            &named_digest("stale-head"),
            &identity(),
            1,
            post_build_verify_event(&head, 0),
        ),
        Err(WorkflowGovernanceLedgerError::HeadMismatch { .. })
    ));

    assert!(matches!(
        ledger.apply_post_build_verify_episode_unchecked_tcb(
            &head,
            &identity(),
            2,
            post_build_verify_event(&head, 0),
        ),
        Err(
            WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                reason: "episode route state version is not contiguous"
            }
        )
    ));

    let mut wrong_phase = post_build_verify_event(&head, 0);
    wrong_phase.from_phase = id("5-ready-operate");
    assert!(matches!(
        ledger.apply_post_build_verify_episode_unchecked_tcb(&head, &identity(), 1, wrong_phase,),
        Err(WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid { .. })
    ));

    let mut malformed_gate = post_build_verify_event(&head, 0);
    malformed_gate.admitted_gate.as_mut().expect("gate").status = GateStatus::Fail;
    assert!(matches!(
        ledger
            .apply_post_build_verify_episode_unchecked_tcb(&head, &identity(), 1, malformed_gate,),
        Err(
            WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                reason: "episode outcome, phase transition, or admitted gate is invalid"
            }
        )
    ));

    let mut broken_generation = post_build_verify_event(&head, 0);
    broken_generation.generation = 2;
    broken_generation.previous_episode_digest = Some(named_digest("missing-predecessor"));
    sync_episode_snapshot(&mut broken_generation);
    assert!(matches!(
        ledger.apply_post_build_verify_episode_unchecked_tcb(
            &head,
            &identity(),
            1,
            broken_generation,
        ),
        Err(
            WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                reason: "episode generation does not extend the exact predecessor"
            }
        )
    ));
    drop(ledger);
    assert_eq!(
        recover_workflow_governance_ledger(&root)
            .expect("recover unchanged ledger")
            .records
            .len(),
        1
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn follow_on_episode_preserves_phase_and_extends_exact_predecessor() {
    let root = temp_root("episode-follow-on");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, post_build_verify_imported())
        .expect("initialize");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock");
    let projection = ledger.recover().expect("recover");
    let head = projection.head_digest.expect("head");
    let first = post_build_verify_event(&head, 0);
    let first_record = ledger
        .apply_post_build_verify_episode_unchecked_tcb(&head, &identity(), 1, first.clone())
        .expect("advance to ready-operate");

    let mut follow_on = post_build_verify_event(&first_record.record_digest, 1);
    follow_on.generation = 2;
    follow_on.previous_episode_digest = Some(first.episode_digest);
    follow_on.episode_digest = named_digest("episode-rollback");
    follow_on.from_phase = id("5-ready-operate");
    follow_on.to_phase = None;
    follow_on.outcome = PostBuildVerifyEpisodeOutcome::RollbackAssessmentOpened;
    follow_on.admitted_gate = None;
    sync_episode_snapshot(&mut follow_on);
    let follow_on_record = ledger
        .apply_post_build_verify_episode_unchecked_tcb(
            &first_record.record_digest,
            &identity(),
            2,
            follow_on.clone(),
        )
        .expect("open rollback assessment");
    assert_eq!(
        follow_on_record.event,
        WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(follow_on.clone())
    );
    assert!(follow_on.to_phase.is_none());
    assert!(follow_on.admitted_gate.is_none());

    let mut evolve = post_build_verify_event(&follow_on_record.record_digest, 2);
    evolve.generation = 3;
    evolve.previous_episode_digest = Some(follow_on.episode_digest);
    evolve.episode_digest = named_digest("episode-evolve");
    evolve.from_phase = id("5-ready-operate");
    evolve.to_phase = Some(id("6-evolve"));
    evolve.outcome = PostBuildVerifyEpisodeOutcome::AdvancedToEvolve;
    evolve.admitted_gate = Some(PostBuildVerifyAdmittedGateResult {
        kind: PostBuildVerifyGateKind::Release,
        status: GateStatus::Pass,
        effective_bundle_digest: named_digest("effective-bundle"),
    });
    sync_episode_snapshot(&mut evolve);
    ledger
        .apply_post_build_verify_episode_unchecked_tcb(
            &follow_on_record.record_digest,
            &identity(),
            3,
            evolve,
        )
        .expect("follow-on retained ready-operate phase");
    drop(ledger);
    recover_workflow_governance_ledger(&root).expect("recover exact episode chain");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn distinct_episode_identity_can_start_at_generation_one() {
    let root = temp_root("episode-distinct-generation");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, post_build_verify_imported())
        .expect("initialize");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock");
    let projection = ledger.recover().expect("recover");
    let head = projection.head_digest.expect("head");
    let first = post_build_verify_event(&head, 0);
    let first_record = ledger
        .apply_post_build_verify_episode_unchecked_tcb(&head, &identity(), 1, first)
        .expect("advance first episode to ready-operate");

    let mut distinct = post_build_verify_event(&first_record.record_digest, 1);
    distinct.episode_id = id("episode.release.current.rollback");
    distinct.episode_digest = named_digest("episode-distinct-rollback");
    distinct.from_phase = id("5-ready-operate");
    distinct.to_phase = None;
    distinct.outcome = PostBuildVerifyEpisodeOutcome::RollbackAssessmentOpened;
    distinct.admitted_gate = None;
    sync_episode_snapshot(&mut distinct);
    ledger
        .apply_post_build_verify_episode_unchecked_tcb(
            &first_record.record_digest,
            &identity(),
            2,
            distinct,
        )
        .expect("start distinct episode at generation one");
    drop(ledger);
    recover_workflow_governance_ledger(&root).expect("recover distinct episode identities");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_keeps_historical_zero_seven_episode_summaries_readable() {
    let root = temp_root("historical-zero-seven-episode");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, post_build_verify_imported())
        .expect("initialize");
    let mut documents = read_documents(&root);
    let head = documents[0]
        .workflow_governance_receipt
        .record_digest
        .clone();
    let mut event = post_build_verify_event(&head, 0);
    event.episode_snapshot = None;
    let mut record = WorkflowGovernanceLedgerRecord {
        record_id: id("historical-zero-seven-episode"),
        sequence: 2,
        project_id: identity().project_id,
        bundle_id: identity().bundle_id,
        bundle_digest: identity().bundle_digest,
        state_version: 1,
        previous_record_digest: Some(head),
        record_digest: String::new(),
        recorded_at_unix: 1,
        event: WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event),
    };
    record.record_digest = workflow_governance_record_digest(&record).expect("record digest");
    documents.push(WorkflowGovernanceReceiptDocument {
        schema_version: WORKFLOW_GOVERNANCE_POST_BUILD_VERIFY_LEDGER_SCHEMA_VERSION.to_owned(),
        workflow_governance_receipt: record,
    });
    write_documents(&root, &documents);

    let projection = recover_workflow_governance_ledger(&root).expect("recover historical 0.7");
    assert!(matches!(
        &projection.records[1].event,
        WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event)
            if event.episode_snapshot.is_none()
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn generic_append_rejects_coordination_state() {
    let root = temp_root("generic-coordination-rejected");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock");
    let projection = ledger.recover().expect("recover");
    let head = projection.head_digest.expect("head");
    let event = coordination_event(
        &head,
        0,
        coordination_request_state(RequestStatus::Pending, None, "cursor-worker-1"),
    );

    assert!(matches!(
        ledger.append_unchecked_tcb_event(
            &head,
            &identity(),
            1,
            WorkflowGovernanceEvent::CoordinationStateApplied(event),
        ),
        Err(WorkflowGovernanceLedgerError::CoordinationStateRequiresDedicatedAuthority)
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
#[allow(clippy::too_many_lines)]
fn coordination_request_lifecycle_is_owner_bound_immutable_and_terminal() {
    let root = temp_root("coordination-request-lifecycle");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock");
    let projection = ledger.recover().expect("recover");
    let genesis_head = projection.head_digest.expect("head");

    let pending = coordination_request_state(RequestStatus::Pending, None, "cursor-worker-1");
    let pending_record = ledger
        .apply_coordination_state_unchecked_tcb(
            &genesis_head,
            &identity(),
            1,
            coordination_event(&genesis_head, 0, pending),
        )
        .expect("sender appends pending request");

    let wrong_actor = coordination_request_state(
        RequestStatus::Accepted,
        Some(RequestStatus::Pending),
        "cursor-worker-1",
    );
    assert!(matches!(
        ledger.apply_coordination_state_unchecked_tcb(
            &pending_record.record_digest,
            &identity(),
            2,
            coordination_event(&pending_record.record_digest, 1, wrong_actor),
        ),
        Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
            reason: "request status does not extend the exact durable predecessor"
        })
    ));

    let mut changed = coordination_request_state(
        RequestStatus::Accepted,
        Some(RequestStatus::Pending),
        "codex-main",
    );
    let CoordinationStateRecord::Request(changed) = &mut changed else {
        unreachable!();
    };
    changed.request.request_contract.requested_operation = id("invented-operation");
    assert!(matches!(
        ledger.apply_coordination_state_unchecked_tcb(
            &pending_record.record_digest,
            &identity(),
            2,
            coordination_event(
                &pending_record.record_digest,
                1,
                CoordinationStateRecord::Request(changed.clone()),
            ),
        ),
        Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
            reason: "request transition changed immutable request fields"
        })
    ));

    let accepted = coordination_request_state(
        RequestStatus::Accepted,
        Some(RequestStatus::Pending),
        "codex-main",
    );
    let accepted_record = ledger
        .apply_coordination_state_unchecked_tcb(
            &pending_record.record_digest,
            &identity(),
            2,
            coordination_event(&pending_record.record_digest, 1, accepted),
        )
        .expect("driver accepts request");

    let duplicate_transition = coordination_request_state(
        RequestStatus::Accepted,
        Some(RequestStatus::Accepted),
        "codex-main",
    );
    assert!(matches!(
        ledger.apply_coordination_state_unchecked_tcb(
            &accepted_record.record_digest,
            &identity(),
            3,
            coordination_event(&accepted_record.record_digest, 2, duplicate_transition),
        ),
        Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
            reason: "request status does not extend the exact durable predecessor"
        })
    ));

    let missing_handoff = coordination_request_state(
        RequestStatus::Applied,
        Some(RequestStatus::Accepted),
        "codex-main",
    );
    assert!(matches!(
        ledger.apply_coordination_state_unchecked_tcb(
            &accepted_record.record_digest,
            &identity(),
            3,
            coordination_event(&accepted_record.record_digest, 2, missing_handoff),
        ),
        Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
            reason: "driver-applied request is missing its authority/effect handoff evidence"
        })
    ));

    let mut applied = coordination_request_state(
        RequestStatus::Applied,
        Some(RequestStatus::Accepted),
        "codex-main",
    );
    let CoordinationStateRecord::Request(applied_request) = &mut applied else {
        unreachable!();
    };
    applied_request.mutation_handoff = Some(CoordinationMutationHandoff {
        driver_agent_id: id("codex-main"),
        requested_operation: id("apply_transition"),
        claim_contract_ref: RepoPath("contracts/claims/driver-active-claim.yaml".to_owned()),
        authority_refs: vec!["contracts/gates/story-ready-lane-gate.yaml".to_owned()],
        effect_contract_refs: vec!["contracts/effects/story-artifact-write-effect.yaml".to_owned()],
    });
    let applied_record = ledger
        .apply_coordination_state_unchecked_tcb(
            &accepted_record.record_digest,
            &identity(),
            3,
            coordination_event(&accepted_record.record_digest, 2, applied),
        )
        .expect("driver applies with evidence-only handoff");

    let terminal_rewrite = coordination_request_state(
        RequestStatus::Rejected,
        Some(RequestStatus::Applied),
        "codex-main",
    );
    assert!(matches!(
        ledger.apply_coordination_state_unchecked_tcb(
            &applied_record.record_digest,
            &identity(),
            4,
            coordination_event(&applied_record.record_digest, 3, terminal_rewrite),
        ),
        Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
            reason: "request status does not extend the exact durable predecessor"
        })
    ));

    drop(ledger);
    recover_workflow_governance_ledger(&root).expect("recover coordination lifecycle");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn oversized_ledger_fails_before_unbounded_read() {
    let root = temp_root("capacity");
    let path = wal_path(&root);
    fs::create_dir_all(path.parent().expect("wal parent")).expect("create wal parent");
    let file = fs::File::create(&path).expect("create wal");
    file.set_len(WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES + 1)
        .expect("create sparse oversized WAL");
    assert!(matches!(
        recover_workflow_governance_ledger(&root),
        Err(WorkflowGovernanceLedgerError::CapacityBytes { .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn record_count_capacity_is_enforced_on_an_otherwise_valid_chain() {
    let root = temp_root("record-capacity");
    let path = wal_path(&root);
    fs::create_dir_all(path.parent().expect("wal parent")).expect("create wal parent");
    let mut output = Vec::new();
    let mut previous = None;
    for index in 1..=10_001_u64 {
        let event = if index == 1 {
            imported()
        } else if index % 2 == 0 {
            advanced_between("discover", "define", usize::try_from(index).expect("index"))
        } else {
            advanced_between("define", "discover", usize::try_from(index).expect("index"))
        };
        let mut record = WorkflowGovernanceLedgerRecord {
            record_id: id(&format!("r{index}")),
            sequence: index,
            project_id: id("p"),
            bundle_id: id("b"),
            bundle_digest: "d".to_owned(),
            state_version: index - 1,
            previous_record_digest: previous,
            record_digest: String::new(),
            recorded_at_unix: 0,
            event,
        };
        record.record_digest = workflow_governance_record_digest(&record).expect("digest");
        previous = Some(record.record_digest.clone());
        let document = WorkflowGovernanceReceiptDocument {
            schema_version: WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION.to_owned(),
            workflow_governance_receipt: record,
        };
        output.extend(serde_json::to_vec(&document).expect("encode"));
        output.push(b'\n');
    }
    assert!(
        u64::try_from(output.len()).expect("fixture length")
            <= WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES,
        "fixture should exercise record count rather than byte capacity"
    );
    fs::write(&path, output).expect("write generated chain");
    assert!(matches!(
        recover_workflow_governance_ledger(&root),
        Err(WorkflowGovernanceLedgerError::CapacityRecords { found: 10_001, .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn concurrent_under_lock_appends_are_serialized_without_lost_updates() {
    let root = Arc::new(temp_root("concurrent"));
    initialize_workflow_governance_ledger_tcb(root.as_ref(), &identity(), 0, imported())
        .expect("initialize");
    let barrier = Arc::new(Barrier::new(9));
    let mut threads = Vec::new();
    for index in 0..8 {
        let root = Arc::clone(&root);
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            let mut ledger = (0..400)
                .find_map(
                    |_| match lock_workflow_governance_ledger_tcb(root.as_ref()) {
                        Ok(ledger) => Some(ledger),
                        Err(WorkflowGovernanceLedgerError::Lock { .. }) => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                            None
                        }
                        Err(error) => panic!("unexpected lock failure: {error}"),
                    },
                )
                .expect("acquire lock within bounded retry window");
            let projection = ledger.recover().expect("recover under lock");
            let head = projection.head_digest.expect("initialized head");
            let state_version = projection.next_state_version;
            let event = if projection.records.len() % 2 == 1 {
                advanced_between("discover", "define", index + 1)
            } else {
                advanced_between("define", "discover", index + 1)
            };
            ledger
                .append_unchecked_tcb_event(&head, &identity(), state_version, event)
                .expect("serialized append")
        }));
    }
    barrier.wait();
    let appended: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().expect("thread"))
        .collect();
    let projection = recover_workflow_governance_ledger(root.as_ref()).expect("recover");
    assert_eq!(projection.records.len(), 9);
    assert_eq!(
        appended
            .iter()
            .map(|record| &record.record_id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        8
    );
    for (index, record) in projection.records.iter().enumerate() {
        assert_eq!(record.sequence, u64::try_from(index + 1).expect("sequence"));
    }
    fs::remove_dir_all(root.as_ref()).expect("cleanup");
}

#[test]
fn receipt_schema_constant_is_the_store_wire_schema() {
    let root = temp_root("schema-wire");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    assert_eq!(
        read_documents(&root)[0].schema_version,
        WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
    );
    fs::remove_dir_all(root).expect("cleanup");
}
