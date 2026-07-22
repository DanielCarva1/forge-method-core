#!/usr/bin/env python3
"""Generate the 16 operation-contract fixtures that S0.1 diagnosed as missing.

Authority: the Rust type OperationContractDocument (crates/forge-core-contracts/src/operation.rs)
is the source of truth, NOT the design doc (which has diverged enums).
deny_unknown_fields is on, so every field must match exactly.

The 6 named fixtures carry behavior expected by operation_plan.rs tests.
The 10 generic fixtures cover remaining autonomy modes and only need to
deserialize + pass validate_operation with zero errors.
"""
import os, re, yaml

OUT = "docs/fixtures/operation-contract-v0"
os.makedirs(OUT, exist_ok=True)


class QuotedDumper(yaml.SafeDumper):
    pass


def _str_presenter(dumper, data):
    # force double-quoted style for string VALUES so enum fields render as
    # `mode: "execute"` (matches the repo style AND the parity tests' string replace)
    if "\n" in data:
        return dumper.represent_scalar("tag:yaml.org,2002:str", data, style="|")
    return dumper.represent_scalar("tag:yaml.org,2002:str", data, style='"')


QuotedDumper.add_representer(str, _str_presenter)


def base(contract_id, autonomy_mode, phase, workflow, action):
    """Common fields every operation contract needs."""
    return {
        "schema_version": "0.1",
        "contract_id": contract_id,
        "created_at": "2026-06-25T00:00:00Z",
        "project_ref": {"root": ".", "project_id": "forge-method-rust", "state_version": 1},
        "source": {
            "host": "codex",
            "surface": "cli_json",
            "operation": "guide",
            "human_input_digest": "sha256:fixture",
        },
        "autonomy": {"mode": autonomy_mode, "rationale": "fixture"},
        "recommendation": {
            "next_actor": "human",
            "next_operation": None,
            "host_action": "show_status",
            "phase": phase,
            "workflow": workflow,
            "action": action,
        },
        "authority": {
            "mutation_policy": "forbidden",
            "side_effect_policy": "read_only",
            "authority_sources": [],
            "authority_evidence": [],
            "missing_authority": [],
        },
        "coordination_scope": {
            "target": {"kind": "none", "id": None, "product_area": None, "paths": []},
            "concurrency": {
                "expected_state_version": 1,
                "agent_id": None,
                "caller_role": "driver",
                "fleet_mode": False,
                "registry_ref": None,
            },
            "write_authority": {
                "requires_driver_claim": False,
                "requires_lane_claim": False,
                "claim_contract_ref": None,
            },
            "completion": {"must_check_completion": False, "completion_contract_ref": None},
        },
        "execution_policy": {
            "mode": "observe_only",
            "max_steps": 0,
            "retry_policy": {"max_attempts": 0, "on_failure": "stop"},
            "branch_policy": {"allowed_branches": [], "default_branch": "stop"},
        },
        "stop_policy": {
            "stop_when": ["human_input_required"],
            "on_stop": {"next_actor": "human", "next_operation": None, "host_action": "show_status"},
        },
        "request": None,
        "decision_close": None,
        "runtime_handoff": None,
        "allowed_actions": ["read_contract"],
        "forbidden_actions": ["change_state"],
        "human": {
            "input_requirement": "none",
            "prompt": {"mode": "none", "text": "", "options": []},
            "tone_contract": "curious_direct",
        },
        "loads": {"required": [], "optional": []},
        "gates": {
            "required_before_mutation": [],
            "current_gate_status": "not_applicable",
            "gate_contract_refs": [],
        },
        "stop_conditions": ["human_input_required"],
        "command_refs": [],
        "effect_contract_refs": [],
        "diagnostics": {"warnings": [], "errors": []},
    }


def write(name, operation):
    path = os.path.join(OUT, name + ".yaml")
    text = yaml.dump(
        {"operation_contract": operation},
        Dumper=QuotedDumper,
        sort_keys=False,
        width=100,
        allow_unicode=True,
    )
    # the dumper quotes both keys and values; we only want VALUES quoted.
    # strip quotes from keys:  "key": value  ->  key: value
    text = re.sub(r'^(\s*)"([^"]+)":', r'\1\2:', text, flags=re.MULTILINE)
    with open(path, "w") as f:
        f.write(text)
    return path


# =============================================================================
# NAMED FIXTURE 1: facilitate-first-product-idea
#   test: facilitation_fixture_waits_for_human
#   expect: AwaitingHuman, [HumanInputRequired], prompt.is_some(), validation_error_count=0
# =============================================================================
def fixture_facilitate():
    o = base("op_fixture_facilitate_first_product_idea", "facilitate", "1-discovery", "guidance-engine", "ask_first_question")
    o["authority"]["mutation_policy"] = "forbidden"  # forbidden -> no authority_evidence needed
    o["authority"]["missing_authority"] = ["human_explicit_direction"]
    o["human"]["input_requirement"] = "required"  # -> AwaitingHuman (HumanInputRequired)
    o["human"]["prompt"] = {"mode": "question", "text": "What should the user feel in the first successful session?", "options": []}
    o["recommendation"]["next_actor"] = "human"
    o["recommendation"]["host_action"] = "request_confirmation"
    o["allowed_actions"] = ["ask_human", "read_contract"]
    o["stop_conditions"] = ["human_answer_required"]
    return o


# =============================================================================
# NAMED FIXTURE 2: mechanical-story-execute
#   test: mechanical_story_execute_fixture_can_call_operation (+ many others)
#   expect: ReadyToCallOperation, [HostCallAllowed], next_operation=RecordArtifact,
#           command_refs not empty, effect_contract_refs not empty, validation_error_count=0
#   contract_id MUST be op_fixture_mechanical_story_execute (evidence record test)
# =============================================================================
def fixture_mechanical():
    o = base("op_fixture_mechanical_story_execute", "execute", "4-build-verify", "quick-dev", "run_story_step")
    o["authority"]["mutation_policy"] = "allowed"
    o["authority"]["side_effect_policy"] = "write_project_files"
    o["authority"]["authority_sources"] = ["operation_contract", "lane_claim"]
    o["authority"]["authority_evidence"] = [
        {"kind": "lane_claim", "ref": "contracts/claims/story-v2-010-active-claim.yaml"}
    ]
    o["coordination_scope"]["target"] = {"kind": "story", "id": "story-v2-010", "product_area": "runtime-core", "paths": ["crates/"]}
    o["coordination_scope"]["write_authority"] = {
        "requires_driver_claim": False,
        "requires_lane_claim": True,
        "claim_contract_ref": "contracts/claims/story-v2-010-active-claim.yaml",
    }
    o["execution_policy"]["mode"] = "autonomous_sequence"
    o["execution_policy"]["max_steps"] = 10
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["next_operation"] = "record_artifact"
    o["recommendation"]["host_action"] = "call_operation"
    o["human"]["input_requirement"] = "none"
    o["command_refs"] = [{"id": "cmd.validate.story_fast", "required": True}]
    o["effect_contract_refs"] = ["contracts/effects/story-artifact-write-effect.yaml"]
    o["allowed_actions"] = ["run_command", "write_project_file", "record_evidence"]
    o["forbidden_actions"] = ["change_route_without_contract"]
    o["stop_conditions"] = ["execution_step_limit_reached", "gate_failed"]
    o["gates"]["current_gate_status"] = "pass"
    return o


# =============================================================================
# NAMED FIXTURE 3: release-gate-required
#   test: release_gate_fixture_requires_gate_before_advance
#   expect: GateRequired, [GateMissingOrPending], next_operation=Gate, validation_error_count=0
# =============================================================================
def fixture_release_gate():
    o = base("op_fixture_release_gate_required", "gate_review", "5-ready-operate", "ready-release", "require_release_gate")
    o["authority"]["mutation_policy"] = "requires_review"
    o["authority"]["authority_sources"] = ["workflow_gate"]
    o["authority"]["authority_evidence"] = [
        {"kind": "gate_result", "ref": "contracts/gates/release-missing-gate.yaml"}
    ]
    o["gates"]["required_before_mutation"] = [
        {"scope": "release", "gate_contract_ref": "contracts/gates/release-missing-gate.yaml", "reason": "release requires gate before advance"}
    ]
    o["gates"]["current_gate_status"] = "pending"  # missing/pending + required -> GateRequired
    o["gates"]["gate_contract_refs"] = ["contracts/gates/release-missing-gate.yaml"]
    o["recommendation"]["next_actor"] = "forge_core"
    o["recommendation"]["next_operation"] = "gate"
    o["recommendation"]["host_action"] = "call_operation"
    o["stop_conditions"] = ["gate_failed", "missing_authority"]
    return o


# =============================================================================
# NAMED FIXTURE 4: destructive-effect-missing-inverse-blocked
#   test: destructive_missing_inverse_fixture_blocks
#   expect: Blocked, [GateBlocked], NOT ReadyToCallOperation
# =============================================================================
def fixture_destructive():
    o = base("op_fixture_destructive_missing_inverse_blocked", "repair", "4-build-verify", "build-story", "apply_destructive_effect")
    o["authority"]["mutation_policy"] = "allowed"
    o["authority"]["side_effect_policy"] = "write_project_files"
    o["authority"]["authority_sources"] = ["operation_contract"]
    o["authority"]["authority_evidence"] = [
        {"kind": "state_rule", "ref": "contracts/effects/destructive-file-delete-missing-inverse-effect.yaml"}
    ]
    o["effect_contract_refs"] = ["contracts/effects/destructive-file-delete-missing-inverse-effect.yaml"]
    # gate BLOCKED -> GateBlocked status (the destructive op without inverse is blocked by the gate)
    o["gates"]["current_gate_status"] = "blocked"
    o["gates"]["gate_contract_refs"] = ["contracts/gates/route-authority-missing-gate.yaml"]
    o["execution_policy"]["mode"] = "single_step"
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["next_operation"] = "record_artifact"
    o["recommendation"]["host_action"] = "call_operation"
    o["human"]["input_requirement"] = "none"
    o["stop_conditions"] = ["gate_failed", "lane_claim_required"]
    return o


# =============================================================================
# NAMED FIXTURE 5: host-drift-invented-next-step
#   test: host_drift_fixture_does_not_become_ready_to_execute
#   expect: AwaitingHuman, [HumanInputRequired], NOT ReadyToCallOperation
#   narrative: host invented a next step; forge forces human confirmation
# =============================================================================
def fixture_host_drift():
    o = base("op_fixture_host_drift_invented_next_step", "facilitate", "2-specification", "write-spec", "host_invented_step_blocked")
    o["authority"]["mutation_policy"] = "forbidden"
    o["authority"]["missing_authority"] = ["human_explicit_direction"]
    o["human"]["input_requirement"] = "required"
    o["human"]["prompt"] = {"mode": "question", "text": "Host proposed a next step Forge did not authorize. Confirm direction?", "options": []}
    o["recommendation"]["next_actor"] = "human"
    o["recommendation"]["host_action"] = "request_confirmation"
    o["stop_conditions"] = ["human_answer_required", "missing_authority"]
    return o


# =============================================================================
# NAMED FIXTURE 6: read-write-conflict-notify
#   test: read_write_conflict_notification_stages_effect_refs_only
#   expect: ReadyToCallOperation, staging command_refs empty, effect_contract_refs len==2
# =============================================================================
def fixture_read_write_conflict():
    o = base("op_fixture_read_write_conflict_notify", "execute", "4-build-verify", "build-story", "notify_read_write_conflict")
    o["authority"]["mutation_policy"] = "allowed"
    o["authority"]["side_effect_policy"] = "write_project_files"
    o["authority"]["authority_sources"] = ["operation_contract", "claim_contract"]
    o["authority"]["authority_evidence"] = [
        {"kind": "claim_contract", "ref": "contracts/claims/story-v2-010-active-claim.yaml"}
    ]
    o["execution_policy"]["mode"] = "autonomous_sequence"
    o["execution_policy"]["max_steps"] = 5
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["next_operation"] = "record_request"
    o["recommendation"]["host_action"] = "call_operation"
    o["human"]["input_requirement"] = "none"
    o["command_refs"] = []  # staging command_refs must be empty
    o["effect_contract_refs"] = [
        "contracts/effects/story-artifact-write-effect.yaml",
        "contracts/effects/conflict-request-append-effect.yaml",
    ]
    o["gates"]["current_gate_status"] = "pass"
    o["stop_conditions"] = ["execution_step_limit_reached"]
    return o


# =============================================================================
# GENERIC FIXTURES (7-16): cover remaining autonomy modes + scenarios
#   requirement: deserialize + validate_operation with zero errors
# =============================================================================
def fixture_observe():
    o = base("op_fixture_observe_project_status", "observe", "5-ready-operate", "sprint-status", "show_status")
    o["recommendation"]["host_action"] = "show_status"
    o["stop_conditions"] = ["human_answer_required"]
    return o


def fixture_research():
    o = base("op_fixture_research_market_scan", "research", "1-discovery", "market-scan", "gather_evidence")
    o["authority"]["mutation_policy"] = "forbidden"
    o["authority"]["missing_authority"] = ["human_explicit_direction"]
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["host_action"] = "call_operation"
    o["human"]["input_requirement"] = "none"
    o["stop_conditions"] = ["missing_authority"]
    return o


def fixture_plan():
    o = base("op_fixture_plan_sprint_slice", "plan", "3-plan", "plan-sprint", "sequence_stories")
    o["authority"]["mutation_policy"] = "requires_review"
    o["authority"]["authority_sources"] = ["workflow_gate"]
    o["authority"]["authority_evidence"] = [
        {"kind": "gate_result", "ref": "contracts/gates/integration-missing-gate.yaml"}
    ]
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["host_action"] = "call_operation"
    o["human"]["input_requirement"] = "none"
    o["stop_conditions"] = ["human_answer_required"]
    return o


def fixture_diagnose():
    o = base("op_fixture_diagnose_host_health", "diagnose", "5-ready-operate", "session-prep", "inspect_health")
    o["authority"]["mutation_policy"] = "forbidden"
    o["authority"]["missing_authority"] = ["system_policy"]
    o["recommendation"]["host_action"] = "show_status"
    o["stop_conditions"] = ["host_install_unhealthy"]
    return o


def fixture_repair_drift():
    o = base("op_fixture_repair_story_drift", "repair", "4-build-verify", "build-story", "repair_artifact_drift")
    o["authority"]["mutation_policy"] = "allowed"
    o["authority"]["side_effect_policy"] = "write_project_files"
    o["authority"]["authority_sources"] = ["operation_contract"]
    o["authority"]["authority_evidence"] = [
        {"kind": "state_rule", "ref": "contracts/effects/story-artifact-write-effect.yaml"}
    ]
    o["effect_contract_refs"] = ["contracts/effects/story-artifact-write-effect.yaml"]
    o["execution_policy"]["mode"] = "bounded_loop"
    o["execution_policy"]["max_steps"] = 3
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["next_operation"] = "record_artifact"
    o["recommendation"]["host_action"] = "call_operation"
    o["human"]["input_requirement"] = "none"
    o["gates"]["current_gate_status"] = "pass"
    o["stop_conditions"] = ["execution_step_limit_reached", "gate_failed"]
    return o


def fixture_gate_review():
    o = base("op_fixture_gate_review_story_ready", "gate_review", "4-build-verify", "readiness-check", "review_story_lane")
    o["authority"]["mutation_policy"] = "requires_review"
    o["authority"]["authority_sources"] = ["workflow_gate"]
    o["authority"]["authority_evidence"] = [
        {"kind": "gate_result", "ref": "contracts/gates/story-ready-lane-gate.yaml"}
    ]
    o["gates"]["required_before_mutation"] = [
        {"scope": "lane", "gate_contract_ref": "contracts/gates/story-ready-lane-gate.yaml"}
    ]
    o["gates"]["current_gate_status"] = "pass"
    o["gates"]["gate_contract_refs"] = ["contracts/gates/story-ready-lane-gate.yaml"]
    o["recommendation"]["next_actor"] = "forge_core"
    o["recommendation"]["next_operation"] = "gate"
    o["recommendation"]["host_action"] = "call_operation"
    o["stop_conditions"] = ["gate_failed"]
    return o


def fixture_execute_trivial():
    o = base("op_fixture_execute_trivial_write", "execute", "4-build-verify", "build-story", "write_artifact")
    o["authority"]["mutation_policy"] = "allowed"
    o["authority"]["side_effect_policy"] = "write_project_files"
    o["authority"]["authority_sources"] = ["operation_contract", "lane_claim"]
    o["authority"]["authority_evidence"] = [
        {"kind": "lane_claim", "ref": "contracts/claims/story-v2-010-active-claim.yaml"}
    ]
    o["coordination_scope"]["target"] = {"kind": "artifact", "id": "artifact-current", "product_area": "runtime-core", "paths": [".forge-method/artifacts/"]}
    o["coordination_scope"]["write_authority"]["requires_lane_claim"] = True
    o["coordination_scope"]["write_authority"]["claim_contract_ref"] = "contracts/claims/story-v2-010-active-claim.yaml"
    o["execution_policy"]["mode"] = "single_step"
    o["execution_policy"]["max_steps"] = 1
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["next_operation"] = "record_artifact"
    o["recommendation"]["host_action"] = "call_operation"
    o["human"]["input_requirement"] = "none"
    o["effect_contract_refs"] = ["contracts/effects/story-artifact-write-effect.yaml"]
    o["gates"]["current_gate_status"] = "pass"
    o["stop_conditions"] = ["execution_step_limit_reached"]
    return o


def fixture_checkpoint():
    o = base("op_fixture_checkpoint_spec_review", "facilitate", "2-specification", "write-spec", "checkpoint_review")
    o["authority"]["mutation_policy"] = "forbidden"
    o["authority"]["missing_authority"] = ["human_explicit_direction"]
    o["human"]["input_requirement"] = "checkpoint"
    o["human"]["prompt"] = {"mode": "decision", "text": "Spec checkpoint: approve before planning?", "options": ["approve", "revise"]}
    o["recommendation"]["next_actor"] = "human"
    o["recommendation"]["host_action"] = "request_confirmation"
    o["stop_conditions"] = ["human_answer_required"]
    return o


def fixture_correct_course():
    # named in design doc: correct-course-frustrated-user
    o = base("op_fixture_correct_course_frustrated_user", "facilitate", "6-evolve", "guidance-engine", "address_user_frustration")
    o["authority"]["mutation_policy"] = "forbidden"
    o["authority"]["missing_authority"] = ["human_explicit_direction"]
    o["human"]["input_requirement"] = "required"
    o["human"]["prompt"] = {"mode": "question", "text": "Course correction: what is frustrating the user right now?", "options": []}
    o["recommendation"]["next_actor"] = "human"
    o["recommendation"]["host_action"] = "request_confirmation"
    o["stop_conditions"] = ["human_answer_required"]
    return o


def fixture_multi_agent_lane():
    # named in design doc: multi-agent-lane-claim
    o = base("op_fixture_multi_agent_lane_claim", "execute", "4-build-verify", "build-story", "claim_lane_for_worker")
    o["authority"]["mutation_policy"] = "allowed"
    o["authority"]["side_effect_policy"] = "write_project_files"
    o["authority"]["authority_sources"] = ["operation_contract", "lane_claim", "claim_contract"]
    o["authority"]["authority_evidence"] = [
        {"kind": "lane_claim", "ref": "contracts/claims/story-v2-010-active-claim.yaml"}
    ]
    o["coordination_scope"]["concurrency"]["fleet_mode"] = True
    o["coordination_scope"]["concurrency"]["agent_id"] = "cursor-worker-1"
    o["coordination_scope"]["concurrency"]["caller_role"] = "worker"
    o["coordination_scope"]["concurrency"]["registry_ref"] = "contracts/runtimes/registry-cursor-browser-agent.yaml"
    o["coordination_scope"]["target"] = {"kind": "lane", "id": "story-v2-010", "product_area": "runtime-core", "paths": ["crates/"]}
    o["coordination_scope"]["write_authority"]["requires_lane_claim"] = True
    o["coordination_scope"]["write_authority"]["claim_contract_ref"] = "contracts/claims/story-v2-010-active-claim.yaml"
    o["execution_policy"]["mode"] = "autonomous_sequence"
    o["execution_policy"]["max_steps"] = 8
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["next_operation"] = "claim_lane"
    o["recommendation"]["host_action"] = "call_operation"
    o["human"]["input_requirement"] = "none"
    o["effect_contract_refs"] = ["contracts/effects/lane-claim-effect.yaml"]
    o["gates"]["current_gate_status"] = "pass"
    o["stop_conditions"] = ["execution_step_limit_reached", "lane_claim_required"]
    return o


# =============================================================================
# EVAL-SUITE FIXTURES (7): referenced by contracts/evals/minimal-coordination-eval-suite.yaml
#   requirement: deserialize + validate_operation zero errors + cross-refs resolve
# =============================================================================
def fixture_already_done():
    # story already completed; re-execution is not needed -> read-only observe
    o = base("op_fixture_already_done_story", "observe", "4-build-verify", "quick-dev", "story_already_done")
    o["authority"]["mutation_policy"] = "forbidden"
    o["authority"]["missing_authority"] = ["completion_contract"]
    o["coordination_scope"]["completion"]["must_check_completion"] = True
    o["coordination_scope"]["completion"]["completion_contract_ref"] = "contracts/completion/story-done-completion.yaml"
    o["recommendation"]["host_action"] = "show_status"
    o["stop_conditions"] = ["completion_state_required"]
    return o


def fixture_destructive_with_inverse():
    # destructive effect that HAS an inverse -> allowed (not blocked like missing-inverse)
    o = base("op_fixture_destructive_with_inverse", "execute", "4-build-verify", "build-story", "apply_reversible_delete")
    o["authority"]["mutation_policy"] = "allowed"
    o["authority"]["side_effect_policy"] = "write_project_files"
    o["authority"]["authority_sources"] = ["operation_contract"]
    o["authority"]["authority_evidence"] = [
        {"kind": "state_rule", "ref": "contracts/effects/file-delete-restore-inverse-effect.yaml"}
    ]
    o["effect_contract_refs"] = ["contracts/effects/file-delete-restore-inverse-effect.yaml"]
    o["execution_policy"]["mode"] = "single_step"
    o["execution_policy"]["max_steps"] = 1
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["next_operation"] = "record_artifact"
    o["recommendation"]["host_action"] = "call_operation"
    o["human"]["input_requirement"] = "none"
    o["gates"]["current_gate_status"] = "pass"
    o["stop_conditions"] = ["execution_step_limit_reached", "gate_failed"]
    return o


def fixture_expired_claim_handoff():
    # claim expired -> route to handoff request, do not silently release
    o = base("op_fixture_expired_claim_handoff", "repair", "4-build-verify", "build-story", "expired_claim_handoff")
    o["authority"]["mutation_policy"] = "requires_review"
    o["authority"]["authority_sources"] = ["claim_contract"]
    o["authority"]["authority_evidence"] = [
        {"kind": "claim_contract", "ref": "contracts/claims/story-current-expired-claim.yaml"}
    ]
    o["request"] = {"ref": "contracts/requests/claim-expiry-handoff-request.yaml"}
    o["recommendation"]["next_actor"] = "forge_core"
    o["recommendation"]["next_operation"] = "record_request"
    o["recommendation"]["host_action"] = "call_operation"
    o["stop_conditions"] = ["lane_claim_required", "missing_authority"]
    return o


def fixture_integration_gate_required():
    # cross-lane integration requires integration gate before advance
    o = base("op_fixture_integration_gate_required", "gate_review", "5-ready-operate", "readiness-check", "require_integration_gate")
    o["authority"]["mutation_policy"] = "requires_review"
    o["authority"]["authority_sources"] = ["workflow_gate"]
    o["authority"]["authority_evidence"] = [
        {"kind": "gate_result", "ref": "contracts/gates/integration-missing-gate.yaml"}
    ]
    o["gates"]["required_before_mutation"] = [
        {"scope": "integration", "gate_contract_ref": "contracts/gates/integration-missing-gate.yaml", "reason": "integration requires gate"}
    ]
    o["gates"]["current_gate_status"] = "missing"
    o["gates"]["gate_contract_refs"] = ["contracts/gates/integration-missing-gate.yaml"]
    o["recommendation"]["next_actor"] = "forge_core"
    o["recommendation"]["next_operation"] = "gate"
    o["recommendation"]["host_action"] = "call_operation"
    o["stop_conditions"] = ["gate_failed", "missing_authority"]
    return o


def fixture_runtime_handoff_missing_double_gate():
    # handoff that requires both a gate and a handoff contract before proceeding
    o = base("op_fixture_runtime_handoff_missing_double_gate", "repair", "4-build-verify", "build-story", "handoff_needs_double_gate")
    o["authority"]["mutation_policy"] = "requires_review"
    o["authority"]["authority_sources"] = ["workflow_gate"]
    o["authority"]["authority_evidence"] = [
        {"kind": "gate_result", "ref": "contracts/gates/route-authority-missing-gate.yaml"}
    ]
    o["runtime_handoff"] = {"ref": "contracts/runtimes/cursor-browser-validation-runtime.yaml"}
    o["gates"]["required_before_mutation"] = [
        {"scope": "lane", "gate_contract_ref": "contracts/gates/route-authority-missing-gate.yaml", "reason": "handoff requires gate"}
    ]
    o["gates"]["current_gate_status"] = "pending"
    o["recommendation"]["next_actor"] = "forge_core"
    o["recommendation"]["next_operation"] = "gate"
    o["recommendation"]["host_action"] = "call_operation"
    o["stop_conditions"] = ["gate_failed", "lane_claim_required"]
    return o


def fixture_runtime_handoff_suggestible():
    # host suggested a handoff; forge forces human confirmation (not auto-accept)
    o = base("op_fixture_runtime_handoff_suggestible", "facilitate", "4-build-verify", "build-story", "handoff_host_suggestion_blocked")
    o["authority"]["mutation_policy"] = "forbidden"
    o["authority"]["missing_authority"] = ["human_explicit_direction"]
    o["human"]["input_requirement"] = "required"
    o["human"]["prompt"] = {"mode": "question", "text": "Host suggested a runtime handoff. Confirm before proceeding?", "options": []}
    o["recommendation"]["next_actor"] = "human"
    o["recommendation"]["host_action"] = "request_confirmation"
    o["stop_conditions"] = ["human_answer_required", "missing_authority"]
    return o


def fixture_worker_record_request():
    # worker (not driver) must route state change via record_request, not apply_transition
    o = base("op_fixture_worker_record_request", "execute", "4-build-verify", "build-story", "worker_routes_via_request")
    o["authority"]["mutation_policy"] = "requires_review"
    o["authority"]["authority_sources"] = ["claim_contract"]
    o["authority"]["authority_evidence"] = [
        {"kind": "claim_contract", "ref": "contracts/claims/story-v2-010-active-claim.yaml"}
    ]
    o["coordination_scope"]["concurrency"]["fleet_mode"] = True
    o["coordination_scope"]["concurrency"]["agent_id"] = "cursor-worker-1"
    o["coordination_scope"]["concurrency"]["caller_role"] = "worker"
    o["coordination_scope"]["concurrency"]["registry_ref"] = "contracts/runtimes/registry-cursor-browser-agent.yaml"
    o["request"] = {"ref": "contracts/requests/worker-state-transition-request.yaml"}
    o["recommendation"]["next_actor"] = "host_agent"
    o["recommendation"]["next_operation"] = "record_request"
    o["recommendation"]["host_action"] = "call_operation"
    o["stop_conditions"] = ["missing_authority", "lane_claim_required"]
    return o


ALL = {
    "facilitate-first-product-idea": fixture_facilitate,
    "mechanical-story-execute": fixture_mechanical,
    "release-gate-required": fixture_release_gate,
    "destructive-effect-missing-inverse-blocked": fixture_destructive,
    "host-drift-invented-next-step": fixture_host_drift,
    "read-write-conflict-notify": fixture_read_write_conflict,
    "observe-project-status": fixture_observe,
    "research-market-scan": fixture_research,
    "plan-sprint-slice": fixture_plan,
    "diagnose-host-health": fixture_diagnose,
    "repair-story-drift": fixture_repair_drift,
    "gate-review-story-ready": fixture_gate_review,
    "execute-trivial-write": fixture_execute_trivial,
    "checkpoint-spec-review": fixture_checkpoint,
    "correct-course-frustrated-user": fixture_correct_course,
    "multi-agent-lane-claim": fixture_multi_agent_lane,
    # eval-suite referenced fixtures
    "already-done-story": fixture_already_done,
    "destructive-effect-with-inverse": fixture_destructive_with_inverse,
    "expired-claim-handoff": fixture_expired_claim_handoff,
    "integration-gate-required": fixture_integration_gate_required,
    "runtime-handoff-missing-double-gate": fixture_runtime_handoff_missing_double_gate,
    "runtime-handoff-suggestible": fixture_runtime_handoff_suggestible,
    "worker-record-request": fixture_worker_record_request,
}

written = []
for name, fn in ALL.items():
    written.append(write(name, fn()))

print(f"wrote {len(written)} fixtures to {OUT}/")
for p in sorted(written):
    print(f"  {p}")
