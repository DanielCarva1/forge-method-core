//! Closed CLI input for same-owner evidence admission.

use std::path::PathBuf;

use forge_core_contracts::{
    CliEnvelope, ExitReason, WorkflowCooperativeEvidenceDisposition, WorkflowGovernanceEvent,
    MAX_WORKFLOW_COOPERATIVE_EVIDENCE_INPUT_BYTES,
};

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let json_count = args.iter().filter(|arg| arg.as_str() == "--json").count();
    let no_json_count = args
        .iter()
        .filter(|arg| arg.as_str() == "--no-json")
        .count();
    let want_json = no_json_count == 0;
    if json_count > 1 || no_json_count > 1 || (json_count > 0 && no_json_count > 0) {
        return crate::workflow_cmd::emit_failure(
            "workflow.evidence.admit_cooperative",
            ExitReason::InvalidDecisionShape,
            "output mode flags must be unique and cannot conflict".to_owned(),
            want_json,
        );
    }
    if args
        .first()
        .is_none_or(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
    {
        println!("forge-core workflow evidence admit-cooperative --root <path> --input-file <offer.json> [--json|--no-json]");
        return Ok(());
    }
    if args.first().is_none_or(|arg| arg != "admit-cooperative") {
        return crate::workflow_cmd::emit_failure(
            "workflow.evidence",
            ExitReason::InvalidDecisionShape,
            "unknown workflow evidence subcommand".to_owned(),
            want_json,
        );
    }
    let mut root = None;
    let mut input = None;
    let mut root_seen = false;
    let mut input_seen = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                if root_seen {
                    return crate::workflow_cmd::emit_failure(
                        "workflow.evidence.admit_cooperative",
                        ExitReason::InvalidDecisionShape,
                        "--root must be supplied exactly once".to_owned(),
                        want_json,
                    );
                }
                root_seen = true;
                index += 1;
                let Some(value) = args.get(index).filter(|value| !value.starts_with("--")) else {
                    return crate::workflow_cmd::emit_failure(
                        "workflow.evidence.admit_cooperative",
                        ExitReason::InvalidDecisionShape,
                        "--root requires one path value".to_owned(),
                        want_json,
                    );
                };
                root = Some(PathBuf::from(value));
            }
            "--input-file" => {
                if input_seen {
                    return crate::workflow_cmd::emit_failure(
                        "workflow.evidence.admit_cooperative",
                        ExitReason::InvalidDecisionShape,
                        "--input-file must be supplied exactly once".to_owned(),
                        want_json,
                    );
                }
                input_seen = true;
                index += 1;
                let Some(value) = args.get(index).filter(|value| !value.starts_with("--")) else {
                    return crate::workflow_cmd::emit_failure(
                        "workflow.evidence.admit_cooperative",
                        ExitReason::InvalidDecisionShape,
                        "--input-file requires one path value".to_owned(),
                        want_json,
                    );
                };
                input = Some(PathBuf::from(value));
            }
            "--json" | "--no-json" => {}
            other => {
                return crate::workflow_cmd::emit_failure(
                    "workflow.evidence.admit_cooperative",
                    ExitReason::InvalidDecisionShape,
                    format!("unknown argument {other}"),
                    want_json,
                )
            }
        }
        index += 1;
    }
    let Some(root) = root else {
        return crate::workflow_cmd::emit_failure(
            "workflow.evidence.admit_cooperative",
            ExitReason::InvalidDecisionShape,
            "--root is required".to_owned(),
            want_json,
        );
    };
    let Some(input) = input else {
        return crate::workflow_cmd::emit_failure(
            "workflow.evidence.admit_cooperative",
            ExitReason::InvalidDecisionShape,
            "--input-file is required".to_owned(),
            want_json,
        );
    };
    let raw = match crate::io_util::read_regular_file_no_follow_bounded(
        &input,
        (MAX_WORKFLOW_COOPERATIVE_EVIDENCE_INPUT_BYTES + 1) as u64,
    ) {
        Ok(raw) => raw,
        Err(error) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.evidence.admit_cooperative",
                ExitReason::InvalidDecisionShape,
                format!("invalid cooperative evidence input: {error}"),
                want_json,
            )
        }
    };
    let adapter = match crate::workflow_cmd::resolve_adapter(&root) {
        Ok(adapter) => adapter,
        Err(error) => {
            return crate::workflow_cmd::emit_failure(
                "workflow.evidence.admit_cooperative",
                ExitReason::EnvConfig,
                error.clone(),
                want_json,
            )
        }
    };
    match adapter.record_cooperative_evidence(&raw) {
        Ok(record) => {
            let rejected = matches!(
                &record.event,
                WorkflowGovernanceEvent::CooperativeEvidenceObserved(event)
                    if event.disposition == WorkflowCooperativeEvidenceDisposition::Rejected
            );
            if rejected {
                emit_envelope(
                    CliEnvelope::reject(
                        "workflow.evidence.admit_cooperative",
                        ExitReason::RejectedByGate,
                        "cooperative evidence was rejected and durably audited",
                        record,
                    ),
                    want_json,
                )
            } else {
                emit_envelope(
                    CliEnvelope::ok("workflow.evidence.admit_cooperative", record),
                    want_json,
                )
            }
        }
        Err(error) => crate::workflow_cmd::emit_failure(
            "workflow.evidence.admit_cooperative",
            ExitReason::RejectedByGate,
            error.to_string(),
            want_json,
        ),
    }
}
