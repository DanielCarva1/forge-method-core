use forge_core_contracts::{
    Phase, PrincipalId, RepoPath, StableId, WorkflowCollaborationLane, WorkflowCollaborationPlan,
    WorkflowCooperativeHostProvenance, WorkflowCurrentWorkAuthority,
    WorkflowCurrentWorkCollaborationLaneSummary, WorkflowCurrentWorkCollaborationSummary,
    WorkflowCurrentWorkContext, WorkflowCurrentWorkDetail, WorkflowCurrentWorkDetailFocus,
    WorkflowCurrentWorkQuickCycleState, WorkflowCurrentWorkQuickCycleSummary,
    WorkflowCurrentWorkStatus, WorkflowCurrentWorkSummary, WorkflowCurrentWorkValidationError,
    WorkflowQuickCycleCloseout, WorkflowQuickCycleExpansion, WorkflowQuickCycleSnapshot,
    WorkflowQuickCycleStageCloseouts, WorkflowWorkFocusAcceptInput,
    WorkflowWorkFocusObjectiveBinding, WorkflowWorkFocusRecordedEvent, WorkflowWorkFocusState,
    WorkflowWorkFocusUpdateInput, CURRENT_WORK_CONTEXT_SCHEMA_VERSION,
    CURRENT_WORK_DETAIL_SCHEMA_VERSION, MAX_COLLABORATION_DEPENDENCIES_PER_LANE,
    MAX_COLLABORATION_ISOLATION_ID_BYTES, MAX_COLLABORATION_LANES, MAX_COLLABORATION_LANE_ID_BYTES,
    MAX_COLLABORATION_OUTCOME_BYTES, MAX_CURRENT_WORK_DETAIL_BYTES, MAX_CURRENT_WORK_SUMMARY_BYTES,
    MAX_WORK_FOCUS_ACCEPT_INPUT_BYTES, MAX_WORK_FOCUS_TEXT_BYTES,
    MAX_WORK_FOCUS_UPDATE_INPUT_BYTES, QUICK_CYCLE_WORK_FOCUS_ACCEPT_INPUT_SCHEMA_VERSION,
    QUICK_CYCLE_WORK_FOCUS_UPDATE_INPUT_SCHEMA_VERSION, WORK_FOCUS_ACCEPT_INPUT_SCHEMA_VERSION,
    WORK_FOCUS_UPDATE_INPUT_SCHEMA_VERSION,
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
        collaboration: None,
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

fn sample_collaboration_plan() -> WorkflowCollaborationPlan {
    WorkflowCollaborationPlan {
        lanes: vec![
            WorkflowCollaborationLane {
                lane_id: StableId("lane.contract".into()),
                outcome: "Define the bounded collaboration contract.".into(),
                depends_on: Vec::new(),
                isolation_id: Some(StableId("isolation.contract".into())),
            },
            WorkflowCollaborationLane {
                lane_id: StableId("lane.persistence".into()),
                outcome: "Persist the accepted plan atomically.".into(),
                depends_on: vec![StableId("lane.contract".into())],
                isolation_id: None,
            },
        ],
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
fn collaboration_plan_is_bounded_closed_and_does_not_change_the_event_wire() {
    let plan = sample_collaboration_plan();
    assert_eq!(MAX_COLLABORATION_LANES, 8);
    assert_eq!(MAX_COLLABORATION_DEPENDENCIES_PER_LANE, 7);
    assert_eq!(MAX_COLLABORATION_LANE_ID_BYTES, 128);
    assert_eq!(MAX_COLLABORATION_OUTCOME_BYTES, 256);
    assert_eq!(MAX_COLLABORATION_ISOLATION_ID_BYTES, 128);
    assert_eq!(plan.validate(), Ok(()));

    let encoded = serde_json::to_value(&plan).expect("collaboration plan serializes");
    let decoded: WorkflowCollaborationPlan =
        serde_json::from_value(encoded.clone()).expect("collaboration plan deserializes");
    assert_eq!(decoded, plan);

    let mut unknown = encoded;
    unknown["lanes"][0]
        .as_object_mut()
        .expect("lane object")
        .insert(
            "agent_id".into(),
            serde_json::json!("agent.duplicate-owner"),
        );
    assert!(serde_json::from_value::<WorkflowCollaborationPlan>(unknown)
        .expect_err("unknown lane fields fail closed")
        .to_string()
        .contains("unknown field"));

    let old_event = serde_json::to_value(sample_event()).expect("old event serializes");
    assert!(old_event.get("collaboration").is_none());

    let mut event = sample_event();
    event.collaboration = Some(plan);
    let encoded_event = serde_json::to_value(&event).expect("collaboration event serializes");
    let decoded_event: WorkflowWorkFocusRecordedEvent =
        serde_json::from_value(encoded_event).expect("collaboration event deserializes");
    assert_eq!(decoded_event, event);
}

#[test]
fn collaboration_plan_rejects_invalid_bounds_and_dependency_graphs() {
    let mut empty = sample_collaboration_plan();
    empty.lanes.clear();
    assert_eq!(
        empty.validate(),
        Err(WorkflowCurrentWorkValidationError::ListBound)
    );

    let mut too_many_lanes = sample_collaboration_plan();
    too_many_lanes.lanes = (0..=MAX_COLLABORATION_LANES)
        .map(|index| WorkflowCollaborationLane {
            lane_id: StableId(format!("lane.{index}")),
            outcome: format!("Outcome {index}"),
            depends_on: Vec::new(),
            isolation_id: None,
        })
        .collect();
    assert_eq!(
        too_many_lanes.validate(),
        Err(WorkflowCurrentWorkValidationError::ListBound)
    );

    let mut bad_outcome = sample_collaboration_plan();
    bad_outcome.lanes[0].outcome = "o".repeat(MAX_COLLABORATION_OUTCOME_BYTES + 1);
    assert_eq!(
        bad_outcome.validate(),
        Err(WorkflowCurrentWorkValidationError::FieldBound)
    );

    let mut bad_lane_id = sample_collaboration_plan();
    bad_lane_id.lanes[0].lane_id = StableId("l".repeat(MAX_COLLABORATION_LANE_ID_BYTES + 1));
    assert_eq!(
        bad_lane_id.validate(),
        Err(WorkflowCurrentWorkValidationError::FieldBound)
    );

    let mut bad_isolation_id = sample_collaboration_plan();
    bad_isolation_id.lanes[0].isolation_id = Some(StableId(
        "i".repeat(MAX_COLLABORATION_ISOLATION_ID_BYTES + 1),
    ));
    assert_eq!(
        bad_isolation_id.validate(),
        Err(WorkflowCurrentWorkValidationError::FieldBound)
    );

    let mut too_many_dependencies = sample_collaboration_plan();
    too_many_dependencies.lanes[1].depends_on =
        vec![StableId("lane.contract".into()); MAX_COLLABORATION_DEPENDENCIES_PER_LANE + 1];
    assert_eq!(
        too_many_dependencies.validate(),
        Err(WorkflowCurrentWorkValidationError::ListBound)
    );

    let mut duplicate_lane = sample_collaboration_plan();
    duplicate_lane.lanes[1].lane_id = duplicate_lane.lanes[0].lane_id.clone();
    assert_eq!(
        duplicate_lane.validate(),
        Err(WorkflowCurrentWorkValidationError::DuplicateLaneId)
    );

    let mut duplicate_dependency = sample_collaboration_plan();
    duplicate_dependency.lanes[1]
        .depends_on
        .push(StableId("lane.contract".into()));
    assert_eq!(
        duplicate_dependency.validate(),
        Err(WorkflowCurrentWorkValidationError::DuplicateLaneDependency)
    );

    let mut unknown_dependency = sample_collaboration_plan();
    unknown_dependency.lanes[1].depends_on = vec![StableId("lane.missing".into())];
    assert_eq!(
        unknown_dependency.validate(),
        Err(WorkflowCurrentWorkValidationError::UnknownLaneDependency)
    );

    let mut self_dependency = sample_collaboration_plan();
    self_dependency.lanes[0].depends_on = vec![StableId("lane.contract".into())];
    assert_eq!(
        self_dependency.validate(),
        Err(WorkflowCurrentWorkValidationError::SelfLaneDependency)
    );

    let mut cycle = sample_collaboration_plan();
    cycle.lanes[0].depends_on = vec![StableId("lane.persistence".into())];
    assert_eq!(
        cycle.validate(),
        Err(WorkflowCurrentWorkValidationError::CyclicLaneDependency)
    );
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
fn collaboration_v3_inputs_are_closed_bounded_and_keep_v1_v2_readable() {
    assert_eq!(
        WORK_FOCUS_ACCEPT_INPUT_SCHEMA_VERSION,
        "work_focus_accept_input_v3"
    );
    assert_eq!(
        WORK_FOCUS_UPDATE_INPUT_SCHEMA_VERSION,
        "work_focus_update_input_v3"
    );
    assert_eq!(
        QUICK_CYCLE_WORK_FOCUS_ACCEPT_INPUT_SCHEMA_VERSION,
        "work_focus_accept_input_v2"
    );
    assert_eq!(
        QUICK_CYCLE_WORK_FOCUS_UPDATE_INPUT_SCHEMA_VERSION,
        "work_focus_update_input_v2"
    );
    let focus = serde_json::json!({
        "focus_id": "focus.quick-cycle-contract",
        "title": "Persist Quick Cycle",
        "intended_outcome": "One atomic Work Focus write",
        "acceptance_summary": "The public input carries bounded continuity",
        "current_activity": "Test the contract",
        "next_step": "Persist the event"
    });
    let host_provenance = serde_json::json!({
        "host_id": "host.contract-test",
        "host_version": "test",
        "session_ref": "session.contract-test",
        "interaction_ref": "turn.contract-test",
        "conversation_digest": format!("sha256:{}", "a".repeat(64)),
        "observed_at_unix": 1
    });
    let continuity = serde_json::json!({
        "blocker_record_digests": [],
        "evidence_record_digests": [format!("sha256:{}", "1".repeat(64))],
        "quick_cycle": sample_quick_cycle()
    });
    let v2_accept = serde_json::json!({
        "schema_version": "work_focus_accept_input_v2",
        "expected_snapshot_digest": format!("sha256:{}", "b".repeat(64)),
        "expected_ledger_head_digest": format!("sha256:{}", "c".repeat(64)),
        "expected_state_version": 24,
        "expected_work_focus": { "status": "absent" },
        "focus": focus,
        "continuity": continuity,
        "recorded_by": "principal.agent.contract-test",
        "host_provenance": host_provenance
    });
    assert!(
        serde_json::to_vec(&v2_accept).unwrap().len() <= MAX_WORK_FOCUS_ACCEPT_INPUT_BYTES as usize
    );
    serde_json::from_value::<WorkflowWorkFocusAcceptInput>(v2_accept.clone())
        .expect("v2 accept input is typed");

    let mut v3_accept = v2_accept.clone();
    v3_accept["schema_version"] = serde_json::json!("work_focus_accept_input_v3");
    v3_accept["continuity"]["collaboration"] =
        serde_json::to_value(sample_collaboration_plan()).expect("collaboration serializes");
    assert!(
        serde_json::to_vec(&v3_accept).unwrap().len() <= MAX_WORK_FOCUS_ACCEPT_INPUT_BYTES as usize
    );
    let typed_v3 = serde_json::from_value::<WorkflowWorkFocusAcceptInput>(v3_accept.clone())
        .expect("v3 acceptance input is typed");
    assert_eq!(
        typed_v3
            .continuity
            .expect("v3 continuity")
            .collaboration
            .expect("v3 collaboration"),
        sample_collaboration_plan()
    );

    let v2_complete = serde_json::json!({
        "schema_version": "work_focus_update_input_v2",
        "expected_snapshot_digest": format!("sha256:{}", "b".repeat(64)),
        "expected_ledger_head_digest": format!("sha256:{}", "c".repeat(64)),
        "expected_state_version": 24,
        "expected_work_focus": {
            "status": "current",
            "record_digest": format!("sha256:{}", "d".repeat(64))
        },
        "change": {
            "kind": "complete",
            "completion_summary": "All five stages closed",
            "next_step": "Deliver",
            "continuity": continuity
        },
        "recorded_by": "principal.agent.contract-test",
        "host_provenance": host_provenance
    });
    assert!(
        serde_json::to_vec(&v2_complete).unwrap().len()
            <= MAX_WORK_FOCUS_UPDATE_INPUT_BYTES as usize
    );
    serde_json::from_value::<WorkflowWorkFocusUpdateInput>(v2_complete)
        .expect("v2 completion input is typed");

    let v3_checkpoint = serde_json::json!({
        "schema_version": "work_focus_update_input_v3",
        "expected_snapshot_digest": format!("sha256:{}", "b".repeat(64)),
        "expected_ledger_head_digest": format!("sha256:{}", "c".repeat(64)),
        "expected_state_version": 24,
        "expected_work_focus": {
            "status": "current",
            "record_digest": format!("sha256:{}", "d".repeat(64))
        },
        "change": {
            "kind": "checkpoint_collaboration",
            "current_activity": "Persist the complete plan",
            "next_step": "Run the ready lane",
            "continuity": {
                "collaboration": sample_collaboration_plan()
            }
        },
        "recorded_by": "principal.agent.contract-test",
        "host_provenance": host_provenance
    });
    assert!(
        serde_json::to_vec(&v3_checkpoint).unwrap().len()
            <= MAX_WORK_FOCUS_UPDATE_INPUT_BYTES as usize
    );
    serde_json::from_value::<WorkflowWorkFocusUpdateInput>(v3_checkpoint)
        .expect("v3 collaboration checkpoint is typed");

    let mut unknown = v3_accept;
    unknown["continuity"]["second_authority"] = serde_json::json!(true);
    assert!(
        serde_json::from_value::<WorkflowWorkFocusAcceptInput>(unknown)
            .expect_err("unknown continuity fields fail closed")
            .to_string()
            .contains("unknown field")
    );

    let v1_accept = serde_json::json!({
        "schema_version": "work_focus_accept_input_v1",
        "expected_snapshot_digest": format!("sha256:{}", "b".repeat(64)),
        "expected_ledger_head_digest": format!("sha256:{}", "c".repeat(64)),
        "expected_state_version": 24,
        "expected_work_focus": { "status": "absent" },
        "focus": focus,
        "recorded_by": "principal.agent.contract-test",
        "host_provenance": host_provenance
    });
    serde_json::from_value::<WorkflowWorkFocusAcceptInput>(v1_accept)
        .expect("v1 input remains readable");
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
    assert_eq!(
        CURRENT_WORK_CONTEXT_SCHEMA_VERSION,
        "current_work_context_v3"
    );
    assert_eq!(CURRENT_WORK_DETAIL_SCHEMA_VERSION, "current_work_detail_v2");
    let mut event = sample_event();
    event.quick_cycle = Some(sample_quick_cycle());
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
        quick_cycle: Some(WorkflowCurrentWorkQuickCycleSummary {
            state: WorkflowCurrentWorkQuickCycleState::ActiveExpanded,
            stage_closeout_count: 1,
            expansion_count: 1,
        }),
        collaboration: Some(WorkflowCurrentWorkCollaborationSummary {
            lane_count: 2,
            ready_lane_count: 1,
            active_lane_count: 0,
            blocked_lane_count: 1,
            integrated_lane_count: 0,
            next_ready_lane: Some(WorkflowCurrentWorkCollaborationLaneSummary {
                lane_id: StableId("lane.contract".into()),
                outcome: "Define the bounded contract".into(),
                isolation_id: None,
            }),
        }),
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
        quick_cycle: event.quick_cycle,
    };
    let detail = WorkflowCurrentWorkDetail {
        schema_version: CURRENT_WORK_DETAIL_SCHEMA_VERSION.into(),
        authority: WorkflowCurrentWorkAuthority::AdvisoryReadOnly,
        status: WorkflowCurrentWorkStatus::Current,
        focus: detail_focus,
        open_decision_refs: summary.open_decision_refs,
        blocker_refs: summary.blocker_refs,
        evidence_refs: summary.evidence_refs,
        predecessor_detail_argv: None,
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
            quick_cycle: None,
            collaboration: None,
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
