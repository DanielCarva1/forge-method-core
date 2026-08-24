//! Public, bounded bridge to the kernel-owned post-BuildVerify episode admission.

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;
use forge_core_contracts::{CliEnvelope, ExitReason, PostBuildVerifyEpisodeDocument};
use forge_core_kernel::{
    PostBuildVerifyEpisodeApplyRequest, MAX_POST_BUILD_VERIFY_EPISODE_APPLY_INPUT_BYTES,
};
use serde::Deserialize;
use std::path::PathBuf;

const COMMAND: &str = "workflow.episode.apply";
const PREPARE_COMMAND: &str = "workflow.episode.prepare";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EpisodeApplyInput {
    document: PostBuildVerifyEpisodeDocument,
    expected_snapshot_digest: String,
    expected_ledger_head_digest: String,
    expected_state_version: u64,
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
    let mut root = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--json" | "--no-json" => {}
            "--root" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--root requires a value".to_owned())?;
                if value.starts_with('-') {
                    return Err(format!("--root requires a value, got flag '{value}'"));
                }
                if root.replace(PathBuf::from(value)).is_some() {
                    return Err("--root may be supplied only once".to_owned());
                }
            }
            other => return Err(format!("unrecognized workflow episode argument '{other}'")),
        }
        index += 1;
    }
    root.ok_or_else(|| "--root is required".to_owned())
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
            "invalid episode apply input: {error}; maximum {} bytes",
            MAX_POST_BUILD_VERIFY_EPISODE_APPLY_INPUT_BYTES
        )
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("episode apply input {} is invalid: {error}", path.display()))
}
