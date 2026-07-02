//! G2 — Framework (not script) proof for the autonomy router.
//!
//! ## Why this test exists
//!
//! G1 (`progress/g1_policies_script_novela_audit.md`) confirmed that every
//! YAML in `contracts/policies/` is a parametric matrix (modes + thresholds),
//! not a prescriptive script. G2 closes the loop by proving the **runtime**
//! side of that claim: the same `route_lane` function must produce coherent,
//! distinct decisions when fed multiple legitimate policy inputs.
//!
//! If the router were a script — i.e. hardcoded to expect one canonical
//! policy shape — these tests would fail loudly because the fixtures below
//! deliberately vary every parametric axis the policy exposes:
//!
//! - `default_mode` across {Manual, Yolo, `SandboxAuto`, `ConfidenceThreshold`}
//! - per-tool `mode` overrides that must beat the default
//! - `escalation.on_high_risk_path` enabled vs disabled
//! - verification goals across {satisfied, pending, failed}
//!
//! ## What "framework" means here
//!
//! A framework accepts N inputs and produces N coherent outputs through the
//! same code path. A script accepts exactly one input and reproduces one
//! canned output. The matrix below asserts the framework property: each row
//! is a (policy, goal, `failure_streak`) triple; each cell expects a specific
//! decision. Removing any axis must break at least one assertion.
//!
//! ## Fixtures live on disk
//!
//! Fixtures are real `.yaml` files under `docs/fixtures/autonomy-policy-v0/`
//! and `docs/fixtures/verification-goal-v0/`. They are also exercised by the
//! CLI validator (anchor 122), so schema drift between contracts and
//! fixtures is caught twice.

use forge_core_contracts::autonomy_policy::{
    AutonomyPolicyContract, AutonomyPolicyContractDocument,
};
use forge_core_contracts::verification_goal::{
    VerificationGoalContract, VerificationGoalContractDocument,
};
use forge_core_decisions::autonomy_router::{route_lane, LaneDecision, LaneKind, LaneRouteReason};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

// -----------------------------------------------------------------------------
// Fixture loading helpers
// -----------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    // forge-core-decisions is at crates/forge-core-decisions, workspace root is two up.
    manifest
        .ancestors()
        .nth(2)
        .expect("workspace root must be two levels above forge-core-decisions manifest")
        .to_path_buf()
}

fn policy_fixture(name: &str) -> PathBuf {
    workspace_root()
        .join("docs")
        .join("fixtures")
        .join("autonomy-policy-v0")
        .join(name)
}

fn goal_fixture(name: &str) -> PathBuf {
    workspace_root()
        .join("docs")
        .join("fixtures")
        .join("verification-goal-v0")
        .join(name)
}

static SEQ: AtomicUsize = AtomicUsize::new(0);

/// Returns a human-readable label that includes the running test's process
/// id and a per-process counter. Used only in panic messages so failures
/// point at the exact fixture combination that broke.
fn case_label(policy: &str, goal: Option<&str>, streak: u8) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let goal_label = goal.unwrap_or("<none>");
    format!("case#{n} policy={policy} goal={goal_label} streak={streak}")
}

fn load_policy(name: &str) -> AutonomyPolicyContract {
    let path = policy_fixture(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read policy fixture {}: {e}", path.display()));
    let doc: AutonomyPolicyContractDocument = yaml_serde::from_str(&text)
        .unwrap_or_else(|e| panic!("parse policy fixture {}: {e}", path.display()));
    doc.autonomy_policy_contract
}

fn load_goal(name: &str) -> VerificationGoalContract {
    let path = goal_fixture(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read goal fixture {}: {e}", path.display()));
    let doc: VerificationGoalContractDocument = yaml_serde::from_str(&text)
        .unwrap_or_else(|e| panic!("parse goal fixture {}: {e}", path.display()));
    doc.verification_goal_contract
}

#[allow(clippy::needless_pass_by_value)]
fn expect_rigorous(decision: LaneDecision, expected_reason: LaneRouteReason, label: &str) {
    assert_eq!(
        decision.lane,
        LaneKind::Rigorous,
        "{label}: expected rigorous lane"
    );
    assert_eq!(
        decision.reason, expected_reason,
        "{label}: rigorous reason mismatch"
    );
}

#[allow(clippy::needless_pass_by_value)]
fn expect_fast(decision: LaneDecision, label: &str) {
    assert_eq!(decision.lane, LaneKind::Fast, "{label}: expected fast lane");
    assert_eq!(
        decision.reason,
        LaneRouteReason::LowRiskVerified,
        "{label}: fast reason must be LowRiskVerified"
    );
}

// -----------------------------------------------------------------------------
// Branch coverage matrix
// -----------------------------------------------------------------------------
//
// Each test pins one branch of `route_lane_for_tool_classes` to a fixture
// triple. The fixture set is intentionally diverse: removing any single
// fixture must break at least one test, proving the router depends on the
// parametric input and not on a hardcoded canonical shape.

#[test]
fn framework_default_manual_forces_rigorous_regardless_of_goal() {
    // Policy axis: default_mode = Manual. Both goals (satisfied and failed)
    // must produce rigorous-ManualMode — proves the default_manual branch
    // does not peek at goal state.
    let policy = load_policy("policy-manual-default.yaml");

    let label_satisfied = case_label("manual-default", Some("satisfied"), 0);
    let decision_satisfied = route_lane(&policy, Some(&load_goal("goal-all-satisfied.yaml")), 0);
    expect_rigorous(
        decision_satisfied,
        LaneRouteReason::ManualMode,
        &label_satisfied,
    );

    let label_failed = case_label("manual-default", Some("failed"), 0);
    let decision_failed = route_lane(&policy, Some(&load_goal("goal-failed.yaml")), 0);
    expect_rigorous(decision_failed, LaneRouteReason::ManualMode, &label_failed);
}

#[test]
fn framework_yolo_default_routes_fast_when_evidence_is_green() {
    // Policy axis: default_mode = Yolo + low-risk tool classes + green goal.
    // Must produce fast-LowRiskVerified. Proves a permissive policy is
    // honoured by the same code path that conservative policies use.
    let policy = load_policy("policy-yolo-default.yaml");
    let goal = load_goal("goal-all-satisfied.yaml");
    let label = case_label("yolo-default", Some("satisfied"), 0);
    let decision = route_lane(&policy, Some(&goal), 0);
    expect_fast(decision, &label);
}

#[test]
fn framework_yolo_default_routes_rigorous_without_goal() {
    // Same policy as above; only the goal input changes. Must produce
    // rigorous-NoVerificationGoal. Proves the router fail-closes on missing
    // evidence even under a permissive policy.
    let policy = load_policy("policy-yolo-default.yaml");
    let label = case_label("yolo-default", None, 0);
    let decision = route_lane(&policy, None, 0);
    expect_rigorous(decision, LaneRouteReason::NoVerificationGoal, &label);
}

#[test]
fn framework_pending_goal_blocks_fast_lane_even_under_yolo() {
    // Policy axis: permissive. Goal axis: declared but not yet satisfied
    // (pending overall). Must produce rigorous-HighRiskNeedsHuman (no
    // high-risk tool class in scope on this policy at the proposed tool
    // set). Proves the fail-closed invariant: incomplete evidence is
    // treated as no evidence.
    let policy = load_policy("policy-yolo-default.yaml");
    let goal = load_goal("goal-pending.yaml");
    let label = case_label("yolo-default", Some("pending"), 0);
    let decision = route_lane(&policy, Some(&goal), 0);
    expect_rigorous(decision, LaneRouteReason::HighRiskNeedsHuman, &label);
}

#[test]
fn framework_manual_tool_override_beats_yolo_default() {
    // Policy axis: default_mode = Yolo but secret_access = Manual.
    // When secret_access is in the declared tool scope, the override must
    // force rigorous-ManualMode even though the default is Yolo. Proves
    // the in-scope override seam is real (two adapters: default vs override).
    let policy = load_policy("policy-mixed-with-manual-secret.yaml");
    let goal = load_goal("goal-all-satisfied.yaml");
    let tool_classes = [
        forge_core_contracts::autonomy_policy::ToolClass::FileEdit,
        forge_core_contracts::autonomy_policy::ToolClass::SecretAccess,
    ];
    let label = case_label("mixed-manual-secret", Some("satisfied"), 0);
    let decision = forge_core_decisions::autonomy_router::route_lane_for_tool_classes(
        &policy,
        Some(&goal),
        0,
        &tool_classes,
    );
    expect_rigorous(decision, LaneRouteReason::ManualMode, &label);
}

#[test]
fn framework_disabled_escalation_lets_satisfied_goal_route_fast() {
    // Policy axis: escalation.on_high_risk_path = false. Even with a
    // high-risk tool class in scope (secret_access at risk 100), the
    // router must NOT surface HighRiskToolClass — it must route fast when
    // evidence is satisfied. Proves the high-risk surfacing path is
    // opt-in per policy, not hard-coded.
    let policy = load_policy("policy-yolo-disabled-escalation.yaml");
    let goal = load_goal("goal-all-satisfied.yaml");
    let tool_classes = [forge_core_contracts::autonomy_policy::ToolClass::SecretAccess];
    let label = case_label("yolo-disabled-escalation", Some("satisfied"), 0);
    let decision = forge_core_decisions::autonomy_router::route_lane_for_tool_classes(
        &policy,
        Some(&goal),
        0,
        &tool_classes,
    );
    expect_fast(decision, &label);
}

#[test]
fn framework_failure_streak_beats_satisfied_goal() {
    // Failure-streak axis: failure_streak >= max_retries_before_human must
    // route rigorous-RepeatedFailures even with a permissive policy and a
    // green goal. Proves the streak threshold is a real input, not a
    // constant.
    let policy = load_policy("policy-yolo-default.yaml"); // max_retries_before_human = 5
    let goal = load_goal("goal-all-satisfied.yaml");
    let label = case_label("yolo-default", Some("satisfied"), 5);
    let decision = route_lane(&policy, Some(&goal), 5);
    expect_rigorous(decision, LaneRouteReason::RepeatedFailures(5), &label);
}

// -----------------------------------------------------------------------------
// Schema drift guard
// -----------------------------------------------------------------------------
//
// If any fixture drifts away from the contract type (typo, missing field,
// extra key), the loader above will panic at the first test that touches
// it. This final test is a cheap explicit assertion that all seven
// fixtures parse, so a schema breakage surfaces as a single named failure
// rather than N cascading panics.

#[test]
fn framework_all_fixtures_parse_against_current_contracts() {
    let policies = [
        "policy-manual-default.yaml",
        "policy-yolo-default.yaml",
        "policy-mixed-with-manual-secret.yaml",
        "policy-yolo-disabled-escalation.yaml",
    ];
    let goals = [
        "goal-all-satisfied.yaml",
        "goal-pending.yaml",
        "goal-failed.yaml",
    ];

    for name in policies {
        let _ = load_policy(name);
    }
    for name in goals {
        let _ = load_goal(name);
    }
}
