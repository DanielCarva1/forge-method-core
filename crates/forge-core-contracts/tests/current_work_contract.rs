use forge_core_contracts::{
    Phase, PrincipalId, RepoPath, StableId, WorkflowCooperativeHostProvenance,
    WorkflowCurrentWorkAuthority, WorkflowCurrentWorkContext, WorkflowCurrentWorkDetail,
    WorkflowCurrentWorkDetailFocus, WorkflowCurrentWorkStatus, WorkflowCurrentWorkSummary,
    WorkflowCurrentWorkValidationError, WorkflowQuickCycleCloseout, WorkflowQuickCycleExpansion,
    WorkflowQuickCycleSnapshot, WorkflowQuickCycleStageCloseouts,
    WorkflowWorkFocusObjectiveBinding, WorkflowWorkFocusRecordedEvent, WorkflowWorkFocusState,
    CURRENT_WORK_CONTEXT_SCHEMA_VERSION, CURRENT_WORK_DETAIL_SCHEMA_VERSION,
    MAX_CURRENT_WORK_DETAIL_BYTES, MAX_CURRENT_WORK_SUMMARY_BYTES, MAX_WORK_FOCUS_TEXT_BYTES,
};

fn sample_event() -> WorkflowWorkFocusRecordedEvent {
    WorkflowWorkFocusRecordedEvent {
        focus_id: StableId("focus.current-work-continuity".into()),
        objective: WorkflowWorkFocusObjectiveBinding {
            objective_id: StableId("objective.workflow.forge-method-core".into()),
            objective_revision: 2,
            objective_digest:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            accepted_objective_record_digest:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            accepted_objective_record_sequence: 147,
            assurance_epoch: 2,
        },
        phase: Phase::Discovery,
        state: WorkflowWorkFocusState::Active,
        title: "Restore current work continuity".into(),
        intended_outcome: "A replacement agent can continue without prior chat history.".into(),
        acceptance_summary: "The exact active focus is recoverable from Forge state.".into(),
        non_goals: vec!["Do not classify every message inside Forge.".into()],
        canonical_refs: vec![RepoPath(
            "contracts/spec/product-journey-guidance-v0.yaml".into(),
        )],
        affected_area_refs: vec![RepoPath("crates/forge-core-contracts".into())],
        external_work_item_ref: Some("github:DanielCarva1/forge-method-core#32".into()),
        selected_practice_ref: Some(StableId("investigation".into())),
        selected_practice_reason: Some("The current state had to be mapped before editing.".into()),
        current_activity: "Define the ledger contract.".into(),
        next_step: "Add the bounded event and compatibility tests.".into(),
        blocker_record_digests: Vec::new(),
        evidence_record_digests: Vec::new(),
        quick_cycle: None,
        previous_work_focus_record_digest: None,
        admission_ledger_head_digest:
            "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".into(),
        admission_state_version: 24,
        recorded_by: PrincipalId("principal.agent.codex.same-owner".into()),
        host_provenance: WorkflowCooperativeHostProvenance {
            host_id: StableId("host.codex".into()),
            host_version: "openai-codex/gpt-5.6-sol".into(),
            session_ref: "session.current".into(),
            interaction_ref: "turn.accept-current-work".into(),
            conversation_digest:
                "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".into(),
            observed_at_unix: 1_786_579_992,
        },
        authority: WorkflowCurrentWorkAuthority::AdvisoryReadOnly,
        recorded_at_unix: 1_786_579_992,
    }
}

fn sample_quick_cycle() -> WorkflowQuickCycleSnapshot {
    WorkflowQuickCycleSnapshot {
        compactness_reason: "The task is bounded, reversible, and has one clear outcome.".into(),
        stage_closeouts: WorkflowQuickCycleStageCloseouts {
            analysis_discovery: Some(WorkflowQuickCycleCloseout {
                summary: "The missing durable closeout was reproduced in isolated recovery.".into(),
                evidence_record_digests: vec![
                    "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                        .into(),
                ],
            }),
            product_planning: None,
            solution_definition: None,
            implementation: None,
            validation_delivery: None,
        },
        expansion_history: vec![WorkflowQuickCycleExpansion {
            phase: Phase::Discovery,
            reason: "The first review found a wire-compatibility question.".into(),
            evidence_record_digests: vec![
                "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
            ],
        }],
    }
}

#[test]
fn work_focus_event_round_trips_without_gaining_authority() {
    let event = sample_event();
    let encoded = serde_json::to_value(&event).expect("event serializes");

    assert_eq!(encoded["authority"], "advisory_read_only");
    assert!(encoded.get("mutation_authority").is_none());

    let decoded: WorkflowWorkFocusRecordedEvent =
        serde_json::from_value(encoded).expect("event deserializes");
    assert_eq!(decoded, event);
}

#[test]
fn quick_cycle_snapshot_is_optional_closed_and_round_trips() {
    let old_event = serde_json::to_value(sample_event()).expect("old event serializes");
    assert!(old_event.get("quick_cycle").is_none());

    let mut event = sample_event();
    event.evidence_record_digests =
        vec!["sha256:1111111111111111111111111111111111111111111111111111111111111111".into()];
    event.quick_cycle = Some(sample_quick_cycle());
    let encoded = serde_json::to_value(&event).expect("Quick Cycle event serializes");
    assert_eq!(
        encoded["quick_cycle"]["stage_closeouts"]["analysis_discovery"]["summary"],
        "The missing durable closeout was reproduced in isolated recovery."
    );
    assert_eq!(
        encoded["quick_cycle"]["expansion_history"][0]["phase"],
        "1-discovery"
    );

    let decoded: WorkflowWorkFocusRecordedEvent =
        serde_json::from_value(encoded.clone()).expect("Quick Cycle event deserializes");
    assert_eq!(decoded, event);

    let mut unknown = encoded;
    unknown["quick_cycle"]
        .as_object_mut()
        .expect("Quick Cycle object")
        .insert("second_authority".into(), serde_json::json!(true));
    assert!(
        serde_json::from_value::<WorkflowWorkFocusRecordedEvent>(unknown)
            .expect_err("unknown Quick Cycle fields fail closed")
            .to_string()
            .contains("unknown field")
    );
}

#[test]
fn work_focus_event_rejects_unknown_fields() {
    let mut encoded = serde_json::to_value(sample_event()).expect("event serializes");
    encoded
        .as_object_mut()
        .expect("event object")
        .insert("mutation_authority".into(), serde_json::json!(true));

    let error = serde_json::from_value::<WorkflowWorkFocusRecordedEvent>(encoded)
        .expect_err("unknown authority-like fields must fail closed");

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn current_work_readback_is_bounded_advisory_and_closed() {
    let event = sample_event();
    let summary = WorkflowCurrentWorkSummary {
        focus_id: event.focus_id.clone(),
        record_digest: "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
            .into(),
        objective: event.objective.clone(),
        phase: event.phase,
        title: event.title.clone(),
        intended_outcome: event.intended_outcome.clone(),
        external_work_item_ref: event.external_work_item_ref.clone(),
        selected_practice_ref: event.selected_practice_ref.clone(),
        current_activity: event.current_activity.clone(),
        next_step: event.next_step.clone(),
        open_decision_count: 1,
        blocker_count: 0,
        evidence_count: 2,
        open_decision_refs: vec![
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into(),
        ],
        blocker_refs: Vec::new(),
        evidence_refs: vec![
            "sha256:9999999999999999999999999999999999999999999999999999999999999999".into(),
        ],
        detail_argv: vec![
            "forge-core".into(),
            "workflow".into(),
            "current-work".into(),
            "detail".into(),
            "--json".into(),
        ],
    };
    let context = WorkflowCurrentWorkContext {
        schema_version: CURRENT_WORK_CONTEXT_SCHEMA_VERSION.into(),
        authority: WorkflowCurrentWorkAuthority::AdvisoryReadOnly,
        status: WorkflowCurrentWorkStatus::Current,
        focus: Some(summary.clone()),
    };
    let summary_bytes = serde_json::to_vec(&context).expect("summary serializes");
    context.validate().expect("summary contract validates");
    assert!(summary_bytes.len() <= MAX_CURRENT_WORK_SUMMARY_BYTES);
    let decoded: WorkflowCurrentWorkContext =
        serde_json::from_slice(&summary_bytes).expect("summary round trips");
    assert_eq!(decoded, context);

    let detail_focus = WorkflowCurrentWorkDetailFocus {
        focus_id: event.focus_id,
        record_digest: summary.record_digest,
        objective: event.objective,
        phase: event.phase,
        state: event.state,
        title: event.title,
        intended_outcome: event.intended_outcome,
        acceptance_summary: event.acceptance_summary,
        non_goals: event.non_goals,
        canonical_refs: event.canonical_refs,
        affected_area_refs: event.affected_area_refs,
        external_work_item_ref: event.external_work_item_ref,
        selected_practice_ref: event.selected_practice_ref,
        selected_practice_reason: event.selected_practice_reason,
        current_activity: event.current_activity,
        next_step: event.next_step,
        previous_work_focus_record_digest: event.previous_work_focus_record_digest,
        admission_ledger_head_digest: event.admission_ledger_head_digest,
        admission_state_version: event.admission_state_version,
        recorded_at_unix: event.recorded_at_unix,
    };
    let detail = WorkflowCurrentWorkDetail {
        schema_version: CURRENT_WORK_DETAIL_SCHEMA_VERSION.into(),
        authority: WorkflowCurrentWorkAuthority::AdvisoryReadOnly,
        status: WorkflowCurrentWorkStatus::Current,
        focus: detail_focus,
        open_decision_refs: summary.open_decision_refs,
        blocker_refs: summary.blocker_refs,
        evidence_refs: summary.evidence_refs,
    };
    let mut detail_value = serde_json::to_value(&detail).expect("detail serializes");
    detail.validate().expect("detail contract validates");
    let mut contradictory_detail = detail.clone();
    contradictory_detail.status = WorkflowCurrentWorkStatus::Completed;
    assert_eq!(
        contradictory_detail.validate(),
        Err(WorkflowCurrentWorkValidationError::StatusFocusMismatch)
    );
    let mut zero_coordinates = detail.clone();
    zero_coordinates.focus.admission_state_version = 0;
    zero_coordinates
        .validate()
        .expect("the first valid ledger state version is zero");
    zero_coordinates.focus.recorded_at_unix = 0;
    assert_eq!(
        zero_coordinates.validate(),
        Err(WorkflowCurrentWorkValidationError::FieldBound)
    );
    assert!(
        serde_json::to_vec(&detail_value)
            .expect("detail bytes")
            .len()
            <= MAX_CURRENT_WORK_DETAIL_BYTES
    );
    detail_value
        .as_object_mut()
        .expect("detail object")
        .insert("completion_authority".into(), serde_json::json!(true));
    assert!(
        serde_json::from_value::<WorkflowCurrentWorkDetail>(detail_value)
            .expect_err("unknown authority-like detail fields must fail closed")
            .to_string()
            .contains("unknown field")
    );
}

#[test]
fn current_work_readback_rejects_invalid_state_and_bounds() {
    let absent_with_focus = WorkflowCurrentWorkContext {
        schema_version: CURRENT_WORK_CONTEXT_SCHEMA_VERSION.into(),
        authority: WorkflowCurrentWorkAuthority::AdvisoryReadOnly,
        status: WorkflowCurrentWorkStatus::Absent,
        focus: Some(WorkflowCurrentWorkSummary {
            focus_id: StableId("focus.invalid".into()),
            record_digest:
                "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into(),
            objective: sample_event().objective,
            phase: Phase::Discovery,
            title: "x".repeat(MAX_WORK_FOCUS_TEXT_BYTES + 1),
            intended_outcome: "bounded readback".into(),
            external_work_item_ref: None,
            selected_practice_ref: None,
            current_activity: "validate".into(),
            next_step: "reject".into(),
            open_decision_count: 0,
            blocker_count: 0,
            evidence_count: 0,
            open_decision_refs: Vec::new(),
            blocker_refs: Vec::new(),
            evidence_refs: Vec::new(),
            detail_argv: vec!["forge-core".into()],
        }),
    };
    assert_eq!(
        absent_with_focus.validate(),
        Err(WorkflowCurrentWorkValidationError::StatusFocusMismatch)
    );

    let mut oversized = absent_with_focus;
    oversized.status = WorkflowCurrentWorkStatus::Current;
    assert_eq!(
        oversized.validate(),
        Err(WorkflowCurrentWorkValidationError::FieldBound)
    );

    let absent_without_focus = WorkflowCurrentWorkContext {
        schema_version: CURRENT_WORK_CONTEXT_SCHEMA_VERSION.into(),
        authority: WorkflowCurrentWorkAuthority::AdvisoryReadOnly,
        status: WorkflowCurrentWorkStatus::Absent,
        focus: None,
    };
    absent_without_focus
        .validate()
        .expect("absence is represented without contradictory focus data");
}
