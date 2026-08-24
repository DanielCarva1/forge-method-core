//! Public, bounded bridge to the kernel-owned post-BuildVerify episode admission.

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;
use forge_core_contracts::{CliEnvelope, ExitReason, PostBuildVerifyEpisodeDocument};
use forge_core_kernel::{
    PostBuildVerifyEpisodeApplyRequest, MAX_POST_BUILD_VERIFY_EPISODE_APPLY_INPUT_BYTES,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const COMMAND: &str = "workflow.episode.apply";
const PREPARE_COMMAND: &str = "workflow.episode.prepare";
const FINALIZE_COMMAND: &str = "workflow.episode.finalize";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EpisodeApplyInput {
    document: PostBuildVerifyEpisodeDocument,
    expected_snapshot_digest: String,
    expected_ledger_head_digest: String,
    expected_state_version: u64,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct EpisodeFinalizeResult {
    status: &'static str,
    apply_input: EpisodeApplyInput,
}

pub(crate) fn run(args: &[String]) -> Result<(), ExitError> {
    let want_json = !args.iter().any(|argument| argument == "--no-json");
    if args.first().map(String::as_str) == Some("prepare") {
        let root = match parse_prepare_args(args) {
            Ok(value) => value,
            Err(message) => {
                return crate::workflow_cmd::emit_failure(
                    PREPARE_COMMAND,
                    ExitReason::InvalidDecisionShape,
                    message,
                    want_json,
                );
            }
        };
        let adapter = match crate::workflow_cmd::resolve_adapter(&root) {
            Ok(adapter) => adapter,
            Err(message) => {
                return crate::workflow_cmd::emit_failure(
                    PREPARE_COMMAND,
                    ExitReason::EnvConfig,
                    message,
                    want_json,
                );
            }
        };
        return match adapter.prepare_post_build_verify_episode() {
            Ok(packet) => emit_envelope(CliEnvelope::ok(PREPARE_COMMAND, packet), want_json),
            Err(error) => crate::workflow_cmd::emit_failure(
                PREPARE_COMMAND,
                crate::workflow_cmd::classify_error(&error),
                error.to_string(),
                want_json,
            ),
        };
    }
    if args.first().map(String::as_str) == Some("finalize") {
        let input_file = match parse_finalize_args(args) {
            Ok(value) => value,
            Err(message) => {
                return crate::workflow_cmd::emit_failure(
                    FINALIZE_COMMAND,
                    ExitReason::InvalidDecisionShape,
                    message,
                    want_json,
                );
            }
        };
        let mut input = match read_input(&input_file) {
            Ok(value) => value,
            Err(message) => {
                return crate::workflow_cmd::emit_failure(
                    FINALIZE_COMMAND,
                    ExitReason::InvalidDecisionShape,
                    message,
                    want_json,
                );
            }
        };
        input.document.post_build_verify_episode.episode_digest =
            match input.document.episode_digest() {
                Ok(digest) => digest,
                Err(error) => {
                    return crate::workflow_cmd::emit_failure(
                        FINALIZE_COMMAND,
                        ExitReason::InvalidDecisionShape,
                        format!("episode digest could not be calculated: {error}"),
                        want_json,
                    );
                }
            };
        let issues = input.document.validate();
        if !issues.is_empty() {
            let message = issues
                .iter()
                .take(8)
                .map(|issue| format!("{}: {}", issue.path, issue.message))
                .collect::<Vec<_>>()
                .join("; ");
            return crate::workflow_cmd::emit_failure(
                FINALIZE_COMMAND,
                ExitReason::InvalidDecisionShape,
                message,
                want_json,
            );
        }
        return emit_envelope(
            CliEnvelope::ok(
                FINALIZE_COMMAND,
                EpisodeFinalizeResult {
                    status: "valid_candidate_only",
                    apply_input: input,
                },
            ),
            want_json,
        );
    }
    let (root, input_file) = match parse_args(args) {
        Ok(value) => value,
        Err(message) => {
            return crate::workflow_cmd::emit_failure(
                COMMAND,
                ExitReason::InvalidDecisionShape,
                message,
                want_json,
            );
        }
    };

    let input = match read_input(&input_file) {
        Ok(value) => value,
        Err(message) => {
            return crate::workflow_cmd::emit_failure(
                COMMAND,
                ExitReason::InvalidDecisionShape,
                message,
                want_json,
            );
        }
    };
    let adapter = match crate::workflow_cmd::resolve_adapter(&root) {
        Ok(adapter) => adapter,
        Err(message) => {
            return crate::workflow_cmd::emit_failure(
                COMMAND,
                ExitReason::EnvConfig,
                message,
                want_json,
            );
        }
    };
    match adapter.apply_post_build_verify_episode(PostBuildVerifyEpisodeApplyRequest {
        document: &input.document,
        expected_snapshot_digest: &input.expected_snapshot_digest,
        expected_ledger_head_digest: &input.expected_ledger_head_digest,
        expected_state_version: input.expected_state_version,
    }) {
        Ok(receipt) => emit_envelope(CliEnvelope::ok(COMMAND, receipt), want_json),
        Err(error) => crate::workflow_cmd::emit_failure(
            COMMAND,
            crate::workflow_cmd::classify_error(&error),
            error.to_string(),
            want_json,
        ),
    }
}

fn parse_prepare_args(args: &[String]) -> Result<PathBuf, String> {
    parse_single_path_arg(args, "--root")
}

fn parse_finalize_args(args: &[String]) -> Result<PathBuf, String> {
    parse_single_path_arg(args, "--input-file")
}

fn parse_single_path_arg(args: &[String], expected_flag: &str) -> Result<PathBuf, String> {
    let mut path = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--json" | "--no-json" => {}
            flag if flag == expected_flag => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{expected_flag} requires a value"))?;
                if value.starts_with('-') {
                    return Err(format!(
                        "{expected_flag} requires a value, got flag '{value}'"
                    ));
                }
                if path.replace(PathBuf::from(value)).is_some() {
                    return Err(format!("{expected_flag} may be supplied only once"));
                }
            }
            other => return Err(format!("unrecognized workflow episode argument '{other}'")),
        }
        index += 1;
    }
    path.ok_or_else(|| format!("{expected_flag} is required"))
}

fn parse_args(args: &[String]) -> Result<(PathBuf, PathBuf), String> {
    if args.first().map(String::as_str) != Some("apply") {
        return Err("workflow episode requires `apply`".to_owned());
    }
    let mut root = None;
    let mut input_file = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--json" | "--no-json" => {}
            "--root" | "--input-file" => {
                let flag = args[index].as_str();
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                if value.starts_with('-') {
                    return Err(format!("{flag} requires a value, got flag '{value}'"));
                }
                let slot = if flag == "--root" {
                    &mut root
                } else {
                    &mut input_file
                };
                if slot.replace(PathBuf::from(value)).is_some() {
                    return Err(format!("{flag} may be supplied only once"));
                }
            }
            other => return Err(format!("unrecognized workflow episode argument '{other}'")),
        }
        index += 1;
    }
    Ok((
        root.ok_or_else(|| "--root is required".to_owned())?,
        input_file.ok_or_else(|| "--input-file is required".to_owned())?,
    ))
}

fn read_input(path: &PathBuf) -> Result<EpisodeApplyInput, String> {
    let bytes = crate::io_util::read_regular_file_no_follow_bounded(
        path,
        MAX_POST_BUILD_VERIFY_EPISODE_APPLY_INPUT_BYTES as u64,
    )
    .map_err(|error| {
        format!(
            "invalid episode input: {error}; maximum {MAX_POST_BUILD_VERIFY_EPISODE_APPLY_INPUT_BYTES} bytes"
        )
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("episode input {} is invalid: {error}", path.display()))
}
