//! Read-only host Adapter for agent-native Assurance Case guidance.
//!
//! `derive` converts a host-proposed Obligation Engine input into a validated
//! Assurance Case. `resume` validates a previously persisted case and projects
//! the same compact guidance. The CLI never writes the case: a host may persist
//! the returned document, while Forge remains read-only in this slice.

use std::fs;
use std::path::PathBuf;

use forge_core_command_surface::COMMAND_ASSURANCE;
use forge_core_contracts::{
    AssuranceCaseDocument, CliEnvelope, DecisionRequest, ExitReason, NextAction, ReadinessTarget,
    ReadinessVerdict, StableId,
};
use forge_core_decisions::{
    assurance_case_token, derive_assurance_case, ObligationEngineInputDocument,
    ObligationEngineIssue,
};
use forge_core_validate::{validate_assurance_case, Diagnostic};
use serde::Serialize;

use crate::cli_error::ExitError;

pub const ASSURANCE_ADAPTER_SCHEMA_VERSION: &str = "0.1";

const ASSURANCE_COMMAND: &str = "assurance";
const DERIVE_COMMAND: &str = "assurance derive";
const RESUME_COMMAND: &str = "assurance resume";

/// Structured payload returned on success and on self-correctable rejection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AssuranceAdapterPayload {
    Guidance {
        response: Box<AssuranceAdapterResponse>,
    },
    InputRejected {
        issues: Vec<ObligationEngineIssue>,
    },
    CaseRejected {
        diagnostics: Vec<Diagnostic>,
    },
}

/// Stable host-facing response. The complete case is included so a host can
/// persist it without reconstructing authority-bearing state from prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssuranceAdapterResponse {
    pub schema_version: String,
    pub source: AssuranceAdapterSource,
    pub resume_token: String,
    pub guidance: AssuranceGuidanceProjection,
    pub assurance_case: AssuranceCaseDocument,
}

/// Whether the projection was freshly derived or loaded from durable state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceAdapterSource {
    Derived,
    Resumed,
}

/// Compact facts a host needs to choose its next interaction without exposing
/// YAML or workflow selection to the human.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssuranceGuidanceProjection {
    pub case_id: StableId,
    pub snapshot_id: StableId,
    pub target: ReadinessTarget,
    pub verdict: ReadinessVerdict,
    pub blocker_refs: Vec<StableId>,
    pub human_attention: HumanAttentionProjection,
    pub next_action: Option<NextAction>,
}

/// Human attention is requested only for due irreducible Decision Requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HumanAttentionProjection {
    pub status: HumanAttentionStatus,
    pub due_decision_refs: Vec<StableId>,
    pub deferred_decision_refs: Vec<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanAttentionStatus {
    NotRequired,
    RequiredNow,
    Deferred,
}

/// Parse and run `forge-core assurance <subcommand>`.
///
/// # Errors
///
/// Returns the typed CLI error produced by envelope emission. Invalid input is
/// returned as a structured rejection rather than panicking or partially
/// deriving a case.
pub fn run_assurance_command(args: &[String]) -> Result<(), ExitError> {
    let subcommand = args.get(1).map_or("--help", String::as_str);
    match subcommand {
        "derive" => run_derive(&args[2..]),
        "resume" => run_resume(&args[2..]),
        "--help" | "-h" | "help" => {
            print_usage();
            Ok(())
        }
        flag if flag.starts_with('-') => run_flag_only(&args[1..]),
        other => emit_error(
            ASSURANCE_COMMAND,
            ExitReason::InvalidDecisionShape,
            format!(
                "unknown assurance subcommand {other:?}; expected {}",
                COMMAND_ASSURANCE.concrete_subcommand_hint()
            ),
            requested_json(&args[2..]),
        ),
    }
}

fn run_flag_only(args: &[String]) -> Result<(), ExitError> {
    let has_input = args.iter().any(|argument| argument == "--input-file");
    let has_case = args.iter().any(|argument| argument == "--case-file");
    match (has_input, has_case) {
        (true, false) => run_derive(args),
        (false, true) => run_resume(args),
        _ => emit_error(
            ASSURANCE_COMMAND,
            ExitReason::InvalidDecisionShape,
            "flag-only Adapter invocation requires exactly one of --input-file or --case-file",
            requested_json(args),
        ),
    }
}

fn run_derive(args: &[String]) -> Result<(), ExitError> {
    let want_json = requested_json(args);
    let options = match parse_file_options(args, "input-file") {
        Ok(ParseOutcome::Help) => {
            println!("{}", usage_line("derive"));
            return Ok(());
        }
        Ok(ParseOutcome::Run(options)) => options,
        Err(message) => {
            return emit_error(
                DERIVE_COMMAND,
                ExitReason::InvalidDecisionShape,
                message_with_usage("derive", &message),
                want_json,
            );
        }
    };

    let input_path = options.resolved_path();
    let text = match fs::read_to_string(&input_path) {
        Ok(text) => text,
        Err(error) => {
            return emit_error(
                DERIVE_COMMAND,
                ExitReason::EnvConfig,
                format!("cannot read input file {}: {error}", input_path.display()),
                options.want_json,
            );
        }
    };
    let input: ObligationEngineInputDocument = match yaml_serde::from_str(&text) {
        Ok(input) => input,
        Err(error) => {
            return emit_error(
                DERIVE_COMMAND,
                ExitReason::InvalidDecisionShape,
                format!("input file is not a valid Obligation Engine input: {error}"),
                options.want_json,
            );
        }
    };
    let case = match derive_assurance_case(&input) {
        Ok(case) => case,
        Err(rejection) => {
            let message = rejection
                .issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            let envelope = CliEnvelope::reject(
                DERIVE_COMMAND,
                ExitReason::InvalidDecisionShape,
                message,
                AssuranceAdapterPayload::InputRejected {
                    issues: rejection.issues,
                },
            );
            return crate::cli_util::emit_envelope(envelope, options.want_json);
        }
    };

    let report = validate_assurance_case(&case);
    if report.has_errors() {
        let diagnostics = report.diagnostics().to_vec();
        let envelope = CliEnvelope::reject(
            DERIVE_COMMAND,
            ExitReason::EnvConfig,
            "Obligation Engine emitted an invalid Assurance Case",
            AssuranceAdapterPayload::CaseRejected { diagnostics },
        );
        return crate::cli_util::emit_envelope(envelope, options.want_json);
    }
    emit_guidance(
        DERIVE_COMMAND,
        case,
        AssuranceAdapterSource::Derived,
        options.want_json,
    )
}

fn run_resume(args: &[String]) -> Result<(), ExitError> {
    let want_json = requested_json(args);
    let options = match parse_file_options(args, "case-file") {
        Ok(ParseOutcome::Help) => {
            println!("{}", usage_line("resume"));
            return Ok(());
        }
        Ok(ParseOutcome::Run(options)) => options,
        Err(message) => {
            return emit_error(
                RESUME_COMMAND,
                ExitReason::InvalidDecisionShape,
                message_with_usage("resume", &message),
                want_json,
            );
        }
    };

    let case_path = options.resolved_path();
    let text = match fs::read_to_string(&case_path) {
        Ok(text) => text,
        Err(error) => {
            return emit_error(
                RESUME_COMMAND,
                ExitReason::EnvConfig,
                format!("cannot read case file {}: {error}", case_path.display()),
                options.want_json,
            );
        }
    };
    let case: AssuranceCaseDocument = match yaml_serde::from_str(&text) {
        Ok(case) => case,
        Err(error) => {
            return emit_error(
                RESUME_COMMAND,
                ExitReason::InvalidDecisionShape,
                format!("case file is not a valid Assurance Case: {error}"),
                options.want_json,
            );
        }
    };
    let report = validate_assurance_case(&case);
    if report.has_errors() {
        let diagnostics = report.diagnostics().to_vec();
        let message = diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "{:?} at {}: {}",
                    diagnostic.code, diagnostic.path, diagnostic.message
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let envelope = CliEnvelope::reject(
            RESUME_COMMAND,
            ExitReason::InvalidDecisionShape,
            message,
            AssuranceAdapterPayload::CaseRejected { diagnostics },
        );
        return crate::cli_util::emit_envelope(envelope, options.want_json);
    }
    emit_guidance(
        RESUME_COMMAND,
        case,
        AssuranceAdapterSource::Resumed,
        options.want_json,
    )
}

fn emit_guidance(
    command: &str,
    case: AssuranceCaseDocument,
    source: AssuranceAdapterSource,
    want_json: bool,
) -> Result<(), ExitError> {
    let response = match build_response(case, source) {
        Ok(response) => response,
        Err(message) => {
            return emit_error(command, ExitReason::EnvConfig, message, want_json);
        }
    };
    let text_line = text_projection(command, &response);
    let envelope = CliEnvelope::ok(
        command,
        AssuranceAdapterPayload::Guidance {
            response: Box::new(response),
        },
    );
    crate::cli_util::emit_envelope_with(envelope, want_json, Some(&text_line))
}

fn build_response(
    case: AssuranceCaseDocument,
    source: AssuranceAdapterSource,
) -> Result<AssuranceAdapterResponse, String> {
    let resume_token = assurance_case_token(&case).map_err(|error| error.to_string())?;
    let assurance_case = &case.assurance_case;
    let target = assurance_case.readiness.target;
    let human_attention = project_human_attention(&assurance_case.decision_requests, target);
    let guidance = AssuranceGuidanceProjection {
        case_id: assurance_case.id.clone(),
        snapshot_id: assurance_case.project_snapshot.id.clone(),
        target,
        verdict: assurance_case.readiness.verdict,
        blocker_refs: assurance_case.readiness.blocker_refs.clone(),
        human_attention,
        next_action: assurance_case.next_actions.first().cloned(),
    };
    Ok(AssuranceAdapterResponse {
        schema_version: ASSURANCE_ADAPTER_SCHEMA_VERSION.to_owned(),
        source,
        resume_token,
        guidance,
        assurance_case: case,
    })
}

fn project_human_attention(
    decisions: &[DecisionRequest],
    target: ReadinessTarget,
) -> HumanAttentionProjection {
    let mut due_decision_refs = Vec::new();
    let mut deferred_decision_refs = Vec::new();
    for decision in decisions {
        if decision.blocking && decision.blocks_before.rank() <= target.rank() {
            due_decision_refs.push(decision.id.clone());
        } else {
            deferred_decision_refs.push(decision.id.clone());
        }
    }
    let status = if !due_decision_refs.is_empty() {
        HumanAttentionStatus::RequiredNow
    } else if deferred_decision_refs.is_empty() {
        HumanAttentionStatus::NotRequired
    } else {
        HumanAttentionStatus::Deferred
    };
    HumanAttentionProjection {
        status,
        due_decision_refs,
        deferred_decision_refs,
    }
}

fn text_projection(command: &str, response: &AssuranceAdapterResponse) -> String {
    let next_action = response
        .guidance
        .next_action
        .as_ref()
        .map_or("none", |action| action.id.0.as_str());
    format!(
        "{command}: verdict={} target={} human_attention={} next_action={} resume_token={}",
        verdict_label(response.guidance.verdict),
        target_label(response.guidance.target),
        human_attention_label(response.guidance.human_attention.status),
        next_action,
        response.resume_token
    )
}

const fn target_label(target: ReadinessTarget) -> &'static str {
    match target {
        ReadinessTarget::Explore => "explore",
        ReadinessTarget::Execute => "execute",
        ReadinessTarget::Release => "release",
    }
}

const fn verdict_label(verdict: ReadinessVerdict) -> &'static str {
    match verdict {
        ReadinessVerdict::Blocked => "blocked",
        ReadinessVerdict::Ready => "ready",
    }
}

const fn human_attention_label(status: HumanAttentionStatus) -> &'static str {
    match status {
        HumanAttentionStatus::NotRequired => "not_required",
        HumanAttentionStatus::RequiredNow => "required_now",
        HumanAttentionStatus::Deferred => "deferred",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileOptions {
    path: PathBuf,
    root: PathBuf,
    want_json: bool,
}

impl FileOptions {
    fn resolved_path(&self) -> PathBuf {
        if self.path.is_absolute() {
            self.path.clone()
        } else {
            self.root.join(&self.path)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParseOutcome {
    Help,
    Run(FileOptions),
}

fn parse_file_options(args: &[String], path_flag: &'static str) -> Result<ParseOutcome, String> {
    let mut path = None;
    let mut root = None;
    let mut want_json = true;
    let mut index = 0;
    let path_argument = format!("--{path_flag}");
    while index < args.len() {
        match args[index].as_str() {
            "--help" | "-h" => return Ok(ParseOutcome::Help),
            "--json" => {
                want_json = true;
                index += 1;
            }
            "--no-json" => {
                want_json = false;
                index += 1;
            }
            flag if flag == path_argument.as_str() => {
                if path.is_some() {
                    return Err(format!("--{path_flag} may be provided only once"));
                }
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| format!("--{path_flag} requires a value"))?;
                path = Some(PathBuf::from(value));
                index += 2;
            }
            "--root" => {
                if root.is_some() {
                    return Err("--root may be provided only once".to_owned());
                }
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| "--root requires a value".to_owned())?;
                root = Some(PathBuf::from(value));
                index += 2;
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let path = path.ok_or_else(|| format!("--{path_flag} is required"))?;
    Ok(ParseOutcome::Run(FileOptions {
        path,
        root: root.unwrap_or_else(|| PathBuf::from(".")),
        want_json,
    }))
}

fn requested_json(args: &[String]) -> bool {
    args.iter()
        .fold(true, |want_json, argument| match argument.as_str() {
            "--json" => true,
            "--no-json" => false,
            _ => want_json,
        })
}

fn usage_line(subcommand: &str) -> &'static str {
    COMMAND_ASSURANCE
        .usage_line_for_subcommand(subcommand)
        .unwrap_or("forge-core assurance <derive|resume> [options]")
}

fn message_with_usage(subcommand: &str, message: &str) -> String {
    format!("{message}\n\nusage:\n  {}", usage_line(subcommand))
}

fn print_usage() {
    println!("forge-core assurance <subcommand> [options]");
    for line in COMMAND_ASSURANCE.local_usage_lines() {
        println!("  {line}");
    }
}

fn emit_error(
    command: &str,
    reason: ExitReason,
    message: impl Into<String>,
    want_json: bool,
) -> Result<(), ExitError> {
    let envelope: CliEnvelope<AssuranceAdapterPayload> = CliEnvelope::err(command, reason, message);
    crate::cli_util::emit_envelope(envelope, want_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{DecisionAlternative, HumanDecisionReason, ReadinessTarget};
    use std::path::Path;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root")
            .to_path_buf()
    }

    fn derived_case(fixture: &str) -> AssuranceCaseDocument {
        let text = fs::read_to_string(
            repo_root()
                .join("docs/fixtures/obligation-engine-v0")
                .join(fixture),
        )
        .expect("read fixture");
        let input: ObligationEngineInputDocument =
            yaml_serde::from_str(&text).expect("parse fixture");
        derive_assurance_case(&input).expect("derive case")
    }

    #[test]
    fn derive_and_resume_projection_keep_case_and_token_stable() {
        let case = derived_case("artifact-only-release.yaml");
        let derived = build_response(case.clone(), AssuranceAdapterSource::Derived)
            .expect("derived response");
        let resumed =
            build_response(case, AssuranceAdapterSource::Resumed).expect("resumed response");

        assert_eq!(derived.resume_token, resumed.resume_token);
        assert_eq!(derived.guidance, resumed.guidance);
        assert_eq!(derived.assurance_case, resumed.assurance_case);
        assert_ne!(derived.source, resumed.source);
    }

    #[test]
    fn blocked_technical_gap_does_not_ask_human() {
        let case = derived_case("novel-domain-execute.yaml");
        let response = build_response(case, AssuranceAdapterSource::Derived).expect("response");

        assert_eq!(response.guidance.verdict, ReadinessVerdict::Blocked);
        assert_eq!(
            response.guidance.human_attention.status,
            HumanAttentionStatus::NotRequired
        );
    }

    #[test]
    fn only_due_blocking_decision_requires_human_attention_now() {
        let mut case = derived_case("verified-release.yaml");
        case.assurance_case.decision_requests.push(DecisionRequest {
            id: StableId("decision.release_direction".to_owned()),
            question: "Choose the release direction.".to_owned(),
            reason: HumanDecisionReason::ProductDirection,
            alternatives: vec![
                DecisionAlternative {
                    id: StableId("alternative.now".to_owned()),
                    description: "release now".to_owned(),
                    consequences: vec!["narrower audience".to_owned()],
                },
                DecisionAlternative {
                    id: StableId("alternative.later".to_owned()),
                    description: "release later".to_owned(),
                    consequences: vec!["longer feedback gap".to_owned()],
                },
            ],
            recommended_alternative_ref: StableId("alternative.now".to_owned()),
            blocking: true,
            blocks_before: ReadinessTarget::Release,
        });
        case.assurance_case.readiness.verdict = ReadinessVerdict::Blocked;
        case.assurance_case.readiness.blocker_refs =
            vec![StableId("decision.release_direction".to_owned())];
        let response = build_response(case, AssuranceAdapterSource::Derived).expect("response");

        assert_eq!(
            response.guidance.human_attention.status,
            HumanAttentionStatus::RequiredNow
        );
        assert_eq!(
            response.guidance.human_attention.due_decision_refs,
            vec![StableId("decision.release_direction".to_owned())]
        );
    }

    #[test]
    fn parser_rejects_duplicate_path_and_obeys_last_json_flag() {
        let args = vec![
            "--input-file".to_owned(),
            "a.yaml".to_owned(),
            "--json".to_owned(),
            "--no-json".to_owned(),
        ];
        let ParseOutcome::Run(options) = parse_file_options(&args, "input-file").expect("parse")
        else {
            panic!("expected run options");
        };
        assert!(!options.want_json);

        let duplicate = vec![
            "--input-file".to_owned(),
            "a.yaml".to_owned(),
            "--input-file".to_owned(),
            "b.yaml".to_owned(),
        ];
        assert!(parse_file_options(&duplicate, "input-file").is_err());
    }
}
