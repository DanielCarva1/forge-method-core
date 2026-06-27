//! Autonomy command family — exposes the dual-lane risk router (the flagship
//! "scale-with-the-model" surface) to host agents via the CLI.
//!
//! `forge-core autonomy route` loads an [`AutonomyPolicyContract`] and an
//! optional [`VerificationGoalContract`], then asks the engine which lane the
//! proposed work should run in (fast vs rigorous). The result is emitted as a
//! standard [`CliEnvelope`] so it composes with every other forge command.

use forge_core_contracts::autonomy_policy::AutonomyPolicyContractDocument;
use forge_core_contracts::verification_goal::VerificationGoalContractDocument;
use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_engine::autonomy_router::{route_lane, LaneDecision};

/// Parse and run `forge-core autonomy <subcommand>`.
pub fn run_autonomy_command(args: &[String]) {
    let sub = args.get(1).map(String::as_str).unwrap_or("--help");
    match sub {
        "route" => run_route(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("forge-core autonomy <subcommand> [options]");
            println!(
                "  route --policy-file <path> [--goal-file <path>] [--failure-streak <n>] [--no-json]"
            );
        }
        other => {
            eprintln!(
                "forge-core autonomy: unknown subcommand '{other}'. Try: route"
            );
            std::process::exit(2);
        }
    }
}

/// Handler for `forge-core autonomy route`.
pub fn run_route(args: &[String]) {
    let mut policy_file: Option<std::path::PathBuf> = None;
    let mut goal_file: Option<std::path::PathBuf> = None;
    let mut failure_streak: u8 = 0;
    let mut want_json = true;

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--policy-file" => {
                idx += 1;
                policy_file = Some(std::path::PathBuf::from(require_value(
                    args, idx, "route", "policy-file",
                )));
            }
            "--goal-file" => {
                idx += 1;
                goal_file = Some(std::path::PathBuf::from(require_value(
                    args, idx, "route", "goal-file",
                )));
            }
            "--failure-streak" => {
                idx += 1;
                let raw = require_value(args, idx, "route", "failure-streak");
                match raw.parse::<u8>() {
                    Ok(n) => failure_streak = n,
                    Err(_) => {
                        emit_err(
                            "route",
                            "failure-streak must be an integer 0-255",
                            want_json,
                        );
                    }
                }
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!(
                    "forge-core autonomy route --policy-file <path> [--goal-file <path>] [--failure-streak <n>] [--no-json]"
                );
                return;
            }
            other => {
                eprintln!("forge-core autonomy route: unknown argument '{other}'");
                std::process::exit(3);
            }
        }
        idx += 1;
    }

    let Some(policy_file) = policy_file else {
        emit_err("route", "--policy-file is required", want_json);
    };

    // Load + parse the autonomy policy contract.
    let policy_text = match std::fs::read_to_string(&policy_file) {
        Ok(t) => t,
        Err(e) => emit_err("route", &format!("cannot read policy file: {e}"), want_json),
    };
    let policy_doc: AutonomyPolicyContractDocument = match serde_yaml::from_str(&policy_text) {
        Ok(d) => d,
        Err(e) => emit_err(
            "route",
            &format!("policy file is not a valid autonomy_policy contract: {e}"),
            want_json,
        ),
    };

    // Optional verification goal — the machine-checkable evidence that unlocks
    // the fast lane. Without it the router is fail-closed to Rigorous.
    let goal = match goal_file.as_deref() {
        Some(path) => {
            let goal_text = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => emit_err("route", &format!("cannot read goal file: {e}"), want_json),
            };
            let goal_doc: VerificationGoalContractDocument =
                match serde_yaml::from_str(&goal_text) {
                    Ok(d) => d,
                    Err(e) => emit_err(
                        "route",
                        &format!("goal file is not a valid verification_goal contract: {e}"),
                        want_json,
                    ),
                };
            Some(goal_doc.verification_goal_contract)
        }
        None => None,
    };

    let decision: LaneDecision = route_lane(
        &policy_doc.autonomy_policy_contract,
        goal.as_ref(),
        failure_streak,
    );
    let env = CliEnvelope::ok("autonomy route", decision);
    emit(env, want_json);
}

fn require_value(args: &[String], idx: usize, subcommand: &str, flag: &str) -> String {
    match args.get(idx) {
        Some(v) => v.clone(),
        None => {
            eprintln!("forge-core autonomy {subcommand}: --{flag} requires a value");
            std::process::exit(3);
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn emit_err(command: &str, message: &str, want_json: bool) -> ! {
    let env: CliEnvelope<()> =
        CliEnvelope::err(command, ExitReason::InvalidDecisionShape, message);
    emit(env, want_json);
}

fn emit<T: serde::Serialize>(env: CliEnvelope<T>, want_json: bool) -> ! {
    let code = env.exit_code();
    if want_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&env).expect("serialize envelope")
        );
        std::process::exit(code);
    }
    // Text mode: surface the lane for the accepted case.
    if env.ok {
        let data_value = env
            .data
            .as_ref()
            .and_then(|d| serde_json::to_value(d).ok());
        let lane = data_value
            .as_ref()
            .and_then(|v| v.get("lane"))
            .and_then(|l| l.as_str());
        match lane {
            Some(l) => println!("lane: {l}"),
            None => println!("autonomy route: ok"),
        }
    } else {
        eprintln!(
            "autonomy route failed: {}",
            env.error
                .as_ref()
                .map(|e| e.message.as_str())
                .unwrap_or("unknown")
        );
    }
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_engine::autonomy_router::LaneKind;
    use forge_core_contracts::autonomy_policy::{
        AutonomyMode, AutonomyPolicyContract, EscalationPolicy, PolicyScope, PolicyScopeKind,
    };
    use forge_core_contracts::common::StableId;

    fn manual_policy() -> AutonomyPolicyContract {
        AutonomyPolicyContract {
            id: StableId("p1".into()),
            applies_to: PolicyScope {
                kind: PolicyScopeKind::Run,
                ids: vec![StableId("run-1".into())],
            },
            default_mode: AutonomyMode::Manual,
            tool_classes: vec![],
            escalation: EscalationPolicy {
                on_repeated_failure: 3,
                on_high_risk_path: true,
                on_semantic_uncertainty: true,
                max_retries_before_human: 3,
                cooldown_seconds: 60,
            },
            evidence_basis: None,
        }
    }

    #[test]
    fn route_lane_manual_policy_is_rigorous() {
        let policy = manual_policy();
        let decision = route_lane(&policy, None, 0);
        assert_eq!(decision.lane, LaneKind::Rigorous);
    }

    #[test]
    fn require_value_returns_present_value() {
        let args = vec!["--x".to_string(), "v".to_string()];
        let v = require_value(&args, 1, "route", "x");
        assert_eq!(v, "v");
    }
}
