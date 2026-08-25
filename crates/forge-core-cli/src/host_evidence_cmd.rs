//! Read-only verification command for bounded P7F real-host evidence bundles.

use std::path::PathBuf;

use crate::cli_error::ExitError;
use crate::cli_util::{command_surface_usage, emit_envelope, emit_envelope_with};
use forge_core_command_surface::{CommandSpec, COMMAND_HOST_EVIDENCE};
use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_validate::real_host_evidence::{
    verify_real_host_evidence, RealHostEvidenceSummary, DISCLAIMER,
};

const COMMAND: &str = "host-evidence.verify";

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostEvidenceArgs {
    bundle_file: PathBuf,
    json: bool,
}

fn usage(command: &CommandSpec) -> String {
    command_surface_usage(command)
}

/// Run `host-evidence verify` against one bounded YAML or JSON bundle.
///
/// # Errors
///
/// Returns an error when argv is malformed or the result envelope cannot be emitted.
pub fn run_host_evidence_command(args: &[String]) -> Result<(), ExitError> {
    if args
        .iter()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{}", usage(&COMMAND_HOST_EVIDENCE));
        return Ok(());
    }
    let parsed = parse_args(args)?;
    match verify_real_host_evidence(&parsed.bundle_file) {
        Ok(summary) => {
            let rendered = format!(
                "structurally/content-integrity valid: {} ({} artifacts, {} governed-write claims)\n{}",
                summary.bundle_path.display(),
                summary.artifact_count,
                summary.governed_write_count,
                DISCLAIMER,
            );
            emit_envelope_with(
                CliEnvelope::ok(COMMAND, summary),
                parsed.json,
                Some(&rendered),
            )
        }
        Err(error) => emit_envelope(
            CliEnvelope::<RealHostEvidenceSummary>::err(
                COMMAND,
                ExitReason::EnvConfig,
                format!("real-host evidence verification failed: {error}\n{DISCLAIMER}"),
            ),
            parsed.json,
        ),
    }
}

fn parse_args(args: &[String]) -> Result<HostEvidenceArgs, ExitError> {
    let mut bundle_file = None;
    let mut json = false;
    let mut index = 1;
    if args.get(index).map(String::as_str) == Some("verify") {
        index += 1;
    } else {
        return Err(ExitError::usage(usage(&COMMAND_HOST_EVIDENCE)));
    }
    while index < args.len() {
        match args[index].as_str() {
            "--bundle-file" => {
                index += 1;
                let value = args
                    .get(index)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| ExitError::usage(usage(&COMMAND_HOST_EVIDENCE)))?;
                if bundle_file.replace(PathBuf::from(value)).is_some() {
                    return Err(ExitError::usage(usage(&COMMAND_HOST_EVIDENCE)));
                }
            }
            "--json" => json = true,
            "--no-json" => json = false,
            _ => return Err(ExitError::usage(usage(&COMMAND_HOST_EVIDENCE))),
        }
        index += 1;
    }
    Ok(HostEvidenceArgs {
        bundle_file: bundle_file.ok_or_else(|| ExitError::usage(usage(&COMMAND_HOST_EVIDENCE)))?,
        json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn parses_public_verify_command() {
        let parsed = parse_args(&args(&[
            "host-evidence",
            "verify",
            "--bundle-file",
            "bundle.yaml",
            "--json",
        ]))
        .expect("valid arguments");
        assert_eq!(parsed.bundle_file, PathBuf::from("bundle.yaml"));
        assert!(parsed.json);
    }

    #[test]
    fn rejects_missing_or_duplicate_bundle_file() {
        assert!(parse_args(&args(&["host-evidence", "verify"])).is_err());
        assert!(parse_args(&args(&[
            "host-evidence",
            "verify",
            "--bundle-file",
            "a.yaml",
            "--bundle-file",
            "b.yaml",
        ]))
        .is_err());
    }
}
