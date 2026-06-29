//! Autonomy command family - exposes the dual-lane risk router (the flagship
//! "scale-with-the-model" surface) to host agents via the CLI.
//!
//! `forge-core autonomy route` loads an [`AutonomyPolicyContract`] and an
//! optional [`VerificationGoalContract`], then asks the engine which lane the
//! proposed work should run in (fast vs rigorous). The result is emitted as a
//! standard [`CliEnvelope`] so it composes with every other forge command.

use std::path::{Path, PathBuf};

use forge_core_contracts::autonomy_policy::{
    AutonomyPolicyContract, AutonomyPolicyContractDocument, ToolClass,
};
use forge_core_contracts::verification_goal::{
    VerificationGoalContract, VerificationGoalContractDocument,
};
use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_engine::autonomy_router::{route_lane, route_lane_for_tool_classes, LaneDecision};

const AUTONOMY_COMMAND: &str = "autonomy";
const ROUTE_COMMAND: &str = "autonomy route";

/// Parse and run `forge-core autonomy <subcommand>`.
pub fn run_autonomy_command(args: &[String]) {
    let sub = args.get(1).map_or("--help", String::as_str);
    match sub {
        "route" => run_route(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("forge-core autonomy <subcommand> [options]");
            println!(
                "  route --policy-file <path> [--goal-file <path>] [--tool-class <snake_case>]... [--failure-streak <n>] [--no-json]"
            );
        }
        other => {
            let error = unknown_subcommand_error(other, &args[2..]);
            emit_err(error.command(), &error.message(), error.want_json());
        }
    }
}

/// Handler for `forge-core autonomy route`.
pub fn run_route(args: &[String]) {
    let options = match parse_route_args(args) {
        Ok(RouteParseOutcome::Help) => {
            println!(
                "forge-core autonomy route --policy-file <path> [--goal-file <path>] [--tool-class <snake_case>]... [--failure-streak <n>] [--no-json]"
            );
            return;
        }
        Ok(RouteParseOutcome::Run(options)) => options,
        Err(error) => emit_err(ROUTE_COMMAND, &error.message(), error.want_json()),
    };

    let contracts = match load_route_contracts(&options) {
        Ok(contracts) => contracts,
        Err(error) => emit_err(ROUTE_COMMAND, &error.message(), options.want_json),
    };

    let decision: LaneDecision = if options.tool_classes.is_empty() {
        route_lane(
            &contracts.policy,
            contracts.goal.as_ref(),
            options.failure_streak,
        )
    } else {
        route_lane_for_tool_classes(
            &contracts.policy,
            contracts.goal.as_ref(),
            options.failure_streak,
            &options.tool_classes,
        )
    };
    let env = CliEnvelope::ok(ROUTE_COMMAND, decision);
    emit(env, options.want_json);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteOptions {
    policy_file: PathBuf,
    goal_file: Option<PathBuf>,
    tool_classes: Vec<ToolClass>,
    failure_streak: u8,
    want_json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RouteParseOutcome {
    Help,
    Run(RouteOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RouteParseError {
    MissingValue {
        flag: &'static str,
        want_json: bool,
    },
    FlagAsValue {
        flag: &'static str,
        value: String,
        want_json: bool,
    },
    InvalidU8 {
        flag: &'static str,
        value: String,
        want_json: bool,
    },
    InvalidToolClass {
        value: String,
        want_json: bool,
    },
    MissingPolicyFile {
        want_json: bool,
    },
    UnknownArgument {
        argument: String,
        want_json: bool,
    },
}

impl RouteParseError {
    #[must_use]
    fn want_json(&self) -> bool {
        match self {
            Self::MissingValue { want_json, .. }
            | Self::FlagAsValue { want_json, .. }
            | Self::InvalidU8 { want_json, .. }
            | Self::InvalidToolClass { want_json, .. }
            | Self::MissingPolicyFile { want_json }
            | Self::UnknownArgument { want_json, .. } => *want_json,
        }
    }

    #[must_use]
    fn message(&self) -> String {
        match self {
            Self::MissingValue { flag, .. } => format!("--{flag} requires a value"),
            Self::FlagAsValue { flag, value, .. } => {
                format!("--{flag} requires a value, got another flag '{value}'")
            }
            Self::InvalidU8 { flag, value, .. } => {
                format!("--{flag} must be an integer 0-255, got '{value}'")
            }
            Self::InvalidToolClass { value, .. } => {
                format!("--tool-class must be one of file_edit, terminal_command, network_egress, package_install, secret_access, mcp_tool_call, code_exec, git_mutation; got '{value}'")
            }
            Self::MissingPolicyFile { .. } => "--policy-file is required".to_string(),
            Self::UnknownArgument { argument, .. } => format!("unknown argument '{argument}'"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RouteLoadError {
    PolicyRead { path: String, source: String },
    PolicyYaml { source: String },
    GoalRead { path: String, source: String },
    GoalYaml { source: String },
}

impl RouteLoadError {
    #[must_use]
    fn message(&self) -> String {
        match self {
            Self::PolicyRead { path, source } => {
                format!("cannot read policy file '{path}': {source}")
            }
            Self::PolicyYaml { source } => {
                format!("policy file is not a valid autonomy_policy contract: {source}")
            }
            Self::GoalRead { path, source } => {
                format!("cannot read goal file '{path}': {source}")
            }
            Self::GoalYaml { source } => {
                format!("goal file is not a valid verification_goal contract: {source}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteContracts {
    policy: AutonomyPolicyContract,
    goal: Option<VerificationGoalContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AutonomySubcommandError {
    UnknownSubcommand { subcommand: String, want_json: bool },
}

impl AutonomySubcommandError {
    #[must_use]
    fn command(&self) -> &'static str {
        match self {
            Self::UnknownSubcommand { .. } => AUTONOMY_COMMAND,
        }
    }

    #[must_use]
    fn want_json(&self) -> bool {
        match self {
            Self::UnknownSubcommand { want_json, .. } => *want_json,
        }
    }

    #[must_use]
    fn message(&self) -> String {
        match self {
            Self::UnknownSubcommand { subcommand, .. } => {
                format!("unknown subcommand '{subcommand}'. Try: route")
            }
        }
    }
}

#[must_use]
fn unknown_subcommand_error(
    subcommand: &str,
    remaining_args: &[String],
) -> AutonomySubcommandError {
    AutonomySubcommandError::UnknownSubcommand {
        subcommand: subcommand.to_string(),
        want_json: json_output_unless_text_selected(remaining_args),
    }
}

#[must_use]
fn json_output_unless_text_selected(args: &[String]) -> bool {
    !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"))
}

fn parse_route_args(args: &[String]) -> Result<RouteParseOutcome, RouteParseError> {
    let mut policy_file: Option<PathBuf> = None;
    let mut goal_file: Option<PathBuf> = None;
    let mut tool_classes = Vec::new();
    let mut failure_streak: u8 = 0;
    let mut want_json = true;

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--policy-file" => {
                idx += 1;
                policy_file = Some(PathBuf::from(require_value(
                    args,
                    idx,
                    "policy-file",
                    want_json,
                )?));
            }
            "--goal-file" => {
                idx += 1;
                goal_file = Some(PathBuf::from(require_value(
                    args,
                    idx,
                    "goal-file",
                    want_json,
                )?));
            }
            "--failure-streak" => {
                idx += 1;
                let raw = require_value(args, idx, "failure-streak", want_json)?;
                match raw.parse::<u8>() {
                    Ok(n) => failure_streak = n,
                    Err(_) => {
                        return Err(RouteParseError::InvalidU8 {
                            flag: "failure-streak",
                            value: raw,
                            want_json,
                        });
                    }
                }
            }
            "--tool-class" => {
                idx += 1;
                let raw = require_value(args, idx, "tool-class", want_json)?;
                let tool_class =
                    parse_tool_class(&raw).ok_or(RouteParseError::InvalidToolClass {
                        value: raw,
                        want_json,
                    })?;
                tool_classes.push(tool_class);
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => return Ok(RouteParseOutcome::Help),
            other => {
                return Err(RouteParseError::UnknownArgument {
                    argument: other.to_string(),
                    want_json,
                });
            }
        }
        idx += 1;
    }

    let Some(policy_file) = policy_file else {
        return Err(RouteParseError::MissingPolicyFile { want_json });
    };

    Ok(RouteParseOutcome::Run(RouteOptions {
        policy_file,
        goal_file,
        tool_classes,
        failure_streak,
        want_json,
    }))
}

#[must_use]
fn parse_tool_class(raw: &str) -> Option<ToolClass> {
    match raw {
        "file_edit" => Some(ToolClass::FileEdit),
        "terminal_command" => Some(ToolClass::TerminalCommand),
        "network_egress" => Some(ToolClass::NetworkEgress),
        "package_install" => Some(ToolClass::PackageInstall),
        "secret_access" => Some(ToolClass::SecretAccess),
        "mcp_tool_call" => Some(ToolClass::McpToolCall),
        "code_exec" => Some(ToolClass::CodeExec),
        "git_mutation" => Some(ToolClass::GitMutation),
        _ => None,
    }
}

fn load_route_contracts(options: &RouteOptions) -> Result<RouteContracts, RouteLoadError> {
    // Load + parse the autonomy policy contract.
    let policy_text = std::fs::read_to_string(&options.policy_file).map_err(|source| {
        RouteLoadError::PolicyRead {
            path: display_path(&options.policy_file),
            source: source.to_string(),
        }
    })?;
    let policy_doc: AutonomyPolicyContractDocument =
        serde_yaml::from_str(&policy_text).map_err(|source| RouteLoadError::PolicyYaml {
            source: source.to_string(),
        })?;

    // Optional verification goal - the machine-checkable evidence that unlocks
    // the fast lane. Without it the router is fail-closed to Rigorous.
    let goal = match options.goal_file.as_deref() {
        Some(path) => {
            let goal_text =
                std::fs::read_to_string(path).map_err(|source| RouteLoadError::GoalRead {
                    path: display_path(path),
                    source: source.to_string(),
                })?;
            let goal_doc: VerificationGoalContractDocument = serde_yaml::from_str(&goal_text)
                .map_err(|source| RouteLoadError::GoalYaml {
                    source: source.to_string(),
                })?;
            Some(goal_doc.verification_goal_contract)
        }
        None => None,
    };

    Ok(RouteContracts {
        policy: policy_doc.autonomy_policy_contract,
        goal,
    })
}

#[must_use]
fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn require_value(
    args: &[String],
    idx: usize,
    flag: &'static str,
    want_json: bool,
) -> Result<String, RouteParseError> {
    match args.get(idx) {
        Some(value) if value.starts_with('-') => Err(RouteParseError::FlagAsValue {
            flag,
            value: value.clone(),
            want_json,
        }),
        Some(value) => Ok(value.clone()),
        None => Err(RouteParseError::MissingValue { flag, want_json }),
    }
}

#[must_use]
fn error_envelope(command: &str, message: &str) -> CliEnvelope<()> {
    CliEnvelope::err(command, ExitReason::InvalidDecisionShape, message)
}

fn emit_err(command: &str, message: &str, want_json: bool) -> ! {
    let env = error_envelope(command, message);
    emit(env, want_json);
}

#[allow(clippy::needless_pass_by_value)]
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
    let command = env.command.0.as_str();
    if env.ok {
        let data_value = env.data.as_ref().and_then(|d| serde_json::to_value(d).ok());
        let lane = data_value
            .as_ref()
            .and_then(|v| v.get("lane"))
            .and_then(|l| l.as_str());
        match lane {
            Some(l) => println!("lane: {l}"),
            None => println!("{command}: ok"),
        }
    } else {
        eprintln!(
            "{command} failed: {}",
            env.error
                .as_ref()
                .map_or("unknown", |error| error.message.as_str())
        );
    }
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::autonomy_policy::{
        AutonomyMode, AutonomyPolicyContract, EscalationPolicy, PolicyScope, PolicyScopeKind,
    };
    use forge_core_contracts::common::StableId;
    use forge_core_engine::autonomy_router::LaneKind;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

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
        let args = args(&["--x", "v"]);
        let v = require_value(&args, 1, "x", true).expect("value");
        assert_eq!(v, "v");
    }

    #[test]
    fn require_value_rejects_missing_value() {
        let args = args(&["--policy-file"]);
        let error = require_value(&args, 1, "policy-file", true).expect_err("missing value");

        assert_eq!(
            error,
            RouteParseError::MissingValue {
                flag: "policy-file",
                want_json: true,
            }
        );
        assert_eq!(error.message(), "--policy-file requires a value");
    }

    #[test]
    fn require_value_rejects_flag_as_value() {
        let args = args(&["--policy-file", "--goal-file", "goal.yaml"]);
        let error = require_value(&args, 1, "policy-file", true).expect_err("flag as value");

        assert_eq!(
            error,
            RouteParseError::FlagAsValue {
                flag: "policy-file",
                value: "--goal-file".to_string(),
                want_json: true,
            }
        );
        assert!(
            error.message().contains("got another flag '--goal-file'"),
            "message: {}",
            error.message()
        );
    }

    #[test]
    fn parse_route_args_rejects_unknown_argument_with_json_by_default() {
        let error = parse_route_args(&args(&["--unknown"])).expect_err("unknown argument");

        assert_eq!(
            error,
            RouteParseError::UnknownArgument {
                argument: "--unknown".to_string(),
                want_json: true,
            }
        );
        let envelope = error_envelope(ROUTE_COMMAND, &error.message());
        assert_eq!(envelope.command.0, ROUTE_COMMAND);
        assert!(!envelope.ok);
        assert_eq!(
            envelope.exit_reason.0,
            ExitReason::InvalidDecisionShape.as_str()
        );
    }

    #[test]
    fn parse_route_args_keeps_text_preference_for_later_errors() {
        let error =
            parse_route_args(&args(&["--no-json", "--unknown"])).expect_err("unknown argument");

        assert_eq!(
            error,
            RouteParseError::UnknownArgument {
                argument: "--unknown".to_string(),
                want_json: false,
            }
        );
    }

    #[test]
    fn unknown_subcommand_uses_autonomy_envelope_by_default() {
        let error = unknown_subcommand_error("frobnicate", &[]);
        let envelope = error_envelope(error.command(), &error.message());

        assert!(error.want_json());
        assert_eq!(envelope.command.0, AUTONOMY_COMMAND);
        assert!(!envelope.ok);
        assert!(envelope
            .error
            .as_ref()
            .expect("error")
            .message
            .contains("unknown subcommand 'frobnicate'"));
    }

    #[test]
    fn unknown_subcommand_respects_explicit_text_mode() {
        let error = unknown_subcommand_error("frobnicate", &args(&["--text"]));

        assert!(!error.want_json());
    }

    #[test]
    fn parse_route_args_rejects_bad_u8() {
        let error = parse_route_args(&args(&[
            "--policy-file",
            "policy.yaml",
            "--failure-streak",
            "999",
        ]))
        .expect_err("bad u8");

        assert_eq!(
            error,
            RouteParseError::InvalidU8 {
                flag: "failure-streak",
                value: "999".to_string(),
                want_json: true,
            }
        );
        assert_eq!(
            error.message(),
            "--failure-streak must be an integer 0-255, got '999'"
        );
    }

    #[test]
    fn parse_route_args_rejects_negative_number_as_flag_value() {
        let error = parse_route_args(&args(&[
            "--policy-file",
            "policy.yaml",
            "--failure-streak",
            "-1",
        ]))
        .expect_err("flag value");

        assert_eq!(
            error,
            RouteParseError::FlagAsValue {
                flag: "failure-streak",
                value: "-1".to_string(),
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_route_args_accepts_complete_route() {
        let outcome = parse_route_args(&args(&[
            "--policy-file",
            "policy.yaml",
            "--goal-file",
            "goal.yaml",
            "--tool-class",
            "file_edit",
            "--tool-class",
            "code_exec",
            "--failure-streak",
            "7",
            "--text",
        ]))
        .expect("parse route");

        assert_eq!(
            outcome,
            RouteParseOutcome::Run(RouteOptions {
                policy_file: PathBuf::from("policy.yaml"),
                goal_file: Some(PathBuf::from("goal.yaml")),
                tool_classes: vec![ToolClass::FileEdit, ToolClass::CodeExec],
                failure_streak: 7,
                want_json: false,
            })
        );
    }

    #[test]
    fn parse_route_args_rejects_unknown_tool_class() {
        let error = parse_route_args(&args(&[
            "--policy-file",
            "policy.yaml",
            "--tool-class",
            "browser_cookie_dump",
        ]))
        .expect_err("unknown tool class");

        assert_eq!(
            error,
            RouteParseError::InvalidToolClass {
                value: "browser_cookie_dump".to_string(),
                want_json: true,
            }
        );
        assert!(error.message().contains("file_edit"));
        assert!(error.message().contains("browser_cookie_dump"));
    }

    #[test]
    fn parse_tool_class_accepts_every_public_snake_case_value() {
        let cases = [
            ("file_edit", ToolClass::FileEdit),
            ("terminal_command", ToolClass::TerminalCommand),
            ("network_egress", ToolClass::NetworkEgress),
            ("package_install", ToolClass::PackageInstall),
            ("secret_access", ToolClass::SecretAccess),
            ("mcp_tool_call", ToolClass::McpToolCall),
            ("code_exec", ToolClass::CodeExec),
            ("git_mutation", ToolClass::GitMutation),
        ];

        for (raw, expected) in cases {
            assert_eq!(parse_tool_class(raw), Some(expected));
        }
    }

    #[test]
    fn parse_route_args_requires_policy_file() {
        let error = parse_route_args(&args(&["--text"])).expect_err("missing policy");

        assert_eq!(
            error,
            RouteParseError::MissingPolicyFile { want_json: false }
        );
    }

    #[test]
    fn load_route_contracts_reports_unreadable_policy_file() {
        let missing_path = unique_temp_path("missing-policy");
        let options = RouteOptions {
            policy_file: missing_path.clone(),
            goal_file: None,
            tool_classes: vec![],
            failure_streak: 0,
            want_json: true,
        };

        let error = load_route_contracts(&options).expect_err("unreadable policy");

        match error {
            RouteLoadError::PolicyRead { path, source } => {
                assert_eq!(path, display_path(&missing_path));
                assert!(!source.is_empty());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn load_route_contracts_reports_invalid_policy_yaml() {
        let policy_path = unique_temp_path("invalid-policy");
        std::fs::write(&policy_path, "schema_version: [").expect("write invalid policy yaml");
        let options = RouteOptions {
            policy_file: policy_path.clone(),
            goal_file: None,
            tool_classes: vec![],
            failure_streak: 0,
            want_json: true,
        };

        let error = load_route_contracts(&options).expect_err("invalid policy yaml");
        std::fs::remove_file(&policy_path).expect("remove invalid policy yaml");

        match error {
            RouteLoadError::PolicyYaml { source } => assert!(!source.is_empty()),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn load_route_contracts_accepts_valid_policy_without_goal() {
        let policy_path = unique_temp_path("valid-policy");
        std::fs::write(&policy_path, valid_policy_yaml()).expect("write valid policy yaml");
        let options = RouteOptions {
            policy_file: policy_path.clone(),
            goal_file: None,
            tool_classes: vec![],
            failure_streak: 0,
            want_json: true,
        };

        let contracts = load_route_contracts(&options).expect("valid policy");
        std::fs::remove_file(&policy_path).expect("remove valid policy yaml");

        assert_eq!(contracts.policy.default_mode, AutonomyMode::Manual);
        assert!(contracts.goal.is_none());
    }

    fn unique_temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("forge-autonomy-cmd-{label}-{nanos}.yaml"))
    }

    fn valid_policy_yaml() -> &'static str {
        r#"
schema_version: "0.1"
autonomy_policy_contract:
  id: "autonomy.test"
  applies_to:
    kind: "run"
    ids: ["run-1"]
  default_mode: "manual"
  tool_classes: []
  escalation:
    on_repeated_failure: 3
    on_high_risk_path: true
    on_semantic_uncertainty: true
    max_retries_before_human: 3
    cooldown_seconds: 60
  evidence_basis: null
"#
    }
}
