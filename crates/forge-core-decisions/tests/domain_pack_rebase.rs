use forge_core_contracts::{
    DomainPackCandidateAuthority, DomainPackCompositionGap, DomainPackCompositionGapCode,
    DomainPackCompositionRequestDocument, DomainPackLifecycleOperation,
    DomainPackRebaseApplyStatus, DomainPackRebaseCheckStatus, DomainPackRebasePlanInput,
    DomainPackReceiptMigrationPolicy, StableId, WorkflowDomainPackGenerationIdentity,
    WorkflowEffectiveBundleIdentity, WorkflowGovernanceReleaseIdentity, WorkflowReceiptCarryover,
    WorkflowRuntimeBundleIdentity,
};
use forge_core_decisions::{
    plan_domain_pack_rebase, verify_domain_pack_rebase_plan, DomainPackRebasePlanError,
};

const A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const D: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const RAW_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const RAW_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn input() -> DomainPackRebasePlanInput {
    let path = repo_root().join("docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml");
    let request: DomainPackCompositionRequestDocument =
        yaml_serde::from_str(&std::fs::read_to_string(path).expect("composition fixture"))
            .expect("typed composition request");
    let source_core = request.domain_pack_composition_request.core;
    let mut target_core = source_core.clone();
    target_core.bundle_id = StableId("bundle.workflow.target".to_owned());
    D.clone_into(&mut target_core.bundle_digest);
    C.clone_into(&mut target_core.policy_set_digest);
    let core_runtime = WorkflowRuntimeBundleIdentity {
        bundle_id: source_core.bundle_id.clone(),
        bundle_digest: A.to_owned(),
        policy_set_digest: source_core.policy_set_digest.clone(),
    };
    let effective_runtime = WorkflowRuntimeBundleIdentity {
        bundle_id: StableId("bundle.workflow.effective".to_owned()),
        bundle_digest: B.to_owned(),
        policy_set_digest: B.to_owned(),
    };
    DomainPackRebasePlanInput {
        project_id: StableId("project.rebase".to_owned()),
        source_release: WorkflowGovernanceReleaseIdentity {
            lineage_id: StableId("workflow.lineage".to_owned()),
            release_id: StableId("workflow.release.source".to_owned()),
            release_version: "1.0.0".to_owned(),
            release_digest: A.to_owned(),
        },
        target_release: WorkflowGovernanceReleaseIdentity {
            lineage_id: StableId("workflow.lineage".to_owned()),
            release_id: StableId("workflow.release.target".to_owned()),
            release_version: "1.1.0".to_owned(),
            release_digest: B.to_owned(),
        },
        source_core: source_core.clone(),
        target_core,
        target_workflow_receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
        effective_identity: WorkflowEffectiveBundleIdentity {
            core_runtime_bundle: core_runtime,
            effective_runtime_bundle: effective_runtime,
            domain_pack_generation: Some(WorkflowDomainPackGenerationIdentity {
                generation: 7,
                active_lock_digest: C.to_owned(),
                composition_digest: D.to_owned(),
                base_core_bundle_digest: source_core.bundle_digest,
                supply_chain_registry_digest: A.to_owned(),
                reviewer_registry_digest: RAW_B.to_owned(),
                reviewed_registry_digest: RAW_C.to_owned(),
            }),
            receipt_context_digest: D.to_owned(),
        },
        lifecycle_operation: DomainPackLifecycleOperation::Rollback {
            target_receipt_digest: A.to_owned(),
            target_lock_digest: B.to_owned(),
        },
        generation: 7,
        lifecycle_pointer_digest: A.to_owned(),
        lifecycle_head_digest: B.to_owned(),
        active_lock_digest: C.to_owned(),
        composition_digest: D.to_owned(),
        supply_chain_registry_digest: A.to_owned(),
        reviewer_registry_digest: RAW_B.to_owned(),
        reviewed_registry_digest: RAW_C.to_owned(),
        active_package_count: 2,
        active_composition_gaps: vec![],
        workflow_ledger_head_digest: C.to_owned(),
        project_snapshot_digest: D.to_owned(),
    }
}

#[test]
fn active_and_rolled_back_generation_gets_deterministic_apply_plan() {
    let candidate = input();
    let first = plan_domain_pack_rebase(&candidate).expect("plan");
    let second = plan_domain_pack_rebase(&candidate).expect("same plan");
    assert_eq!(first, second);
    let plan = first.domain_pack_rebase_plan;
    assert!(plan.mutation_allowed);
    assert_eq!(
        plan.apply_status,
        DomainPackRebaseApplyStatus::ReadyForTcbRevalidation
    );
    assert_eq!(plan.active_generation.generation, 7);
    assert!(matches!(
        plan.active_generation.lifecycle_operation,
        DomainPackLifecycleOperation::Rollback { .. }
    ));
    assert_eq!(
        plan.compatibility.target_core_pack_compatibility,
        DomainPackRebaseCheckStatus::RequiresTargetRevalidation
    );
    assert_eq!(
        plan.compatibility.domain_pack_receipt_carryover,
        DomainPackReceiptMigrationPolicy::InvalidateAll
    );
    assert!(plan.actionable_gaps.is_empty());
    assert!(plan.plan_digest.starts_with("sha256:"));
}

#[test]
fn degraded_empty_remove_is_explicit_and_keeps_requirement_gaps() {
    let mut candidate = input();
    candidate.lifecycle_operation = DomainPackLifecycleOperation::Remove {
        pack: forge_core_contracts::DomainPackCoordinate {
            publisher: StableId("sample".to_owned()),
            name: StableId("foundation".to_owned()),
        },
    };
    candidate.active_package_count = 0;
    candidate.active_composition_gaps = vec![DomainPackCompositionGap {
        code: DomainPackCompositionGapCode::MissingDomain,
        requirement_ref: StableId("requirement.game".to_owned()),
        subject_ref: StableId("domain.game".to_owned()),
        message: "required domain remains missing".to_owned(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
    }];
    let plan = plan_domain_pack_rebase(&candidate)
        .expect("degraded plan")
        .domain_pack_rebase_plan;
    assert!(plan.active_generation.degraded_empty);
    assert_eq!(plan.active_generation.active_composition_gaps.len(), 1);
}

#[test]
fn every_exact_head_changes_the_plan_digest_and_generation_mismatch_rejects() {
    let candidate = input();
    let original = plan_domain_pack_rebase(&candidate)
        .expect("plan")
        .domain_pack_rebase_plan
        .plan_digest;

    for mutate in [
        |value: &mut DomainPackRebasePlanInput| value.source_release.release_digest = D.to_owned(),
        |value: &mut DomainPackRebasePlanInput| value.workflow_ledger_head_digest = D.to_owned(),
        |value: &mut DomainPackRebasePlanInput| value.project_snapshot_digest = A.to_owned(),
        |value: &mut DomainPackRebasePlanInput| value.lifecycle_pointer_digest = B.to_owned(),
        |value: &mut DomainPackRebasePlanInput| value.lifecycle_head_digest = C.to_owned(),
    ] {
        let mut changed = candidate.clone();
        mutate(&mut changed);
        let digest = plan_domain_pack_rebase(&changed)
            .expect("changed plan")
            .domain_pack_rebase_plan
            .plan_digest;
        assert_ne!(digest, original);
    }

    let mut stale_generation = candidate;
    stale_generation.generation += 1;
    assert_eq!(
        plan_domain_pack_rebase(&stale_generation),
        Err(DomainPackRebasePlanError::ActiveGenerationBindingMismatch(
            "generation"
        ))
    );
}

#[test]
fn persisted_plan_integrity_rejects_tampering() {
    let mut plan = plan_domain_pack_rebase(&input()).expect("plan");
    assert!(verify_domain_pack_rebase_plan(&plan));

    plan.domain_pack_rebase_plan.target_core.bundle_digest = A.to_owned();
    assert!(!verify_domain_pack_rebase_plan(&plan));
}
