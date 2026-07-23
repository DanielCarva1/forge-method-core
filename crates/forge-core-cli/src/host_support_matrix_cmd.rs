//! Read-only candidate host-support matrix command.
//!
//! The command validates and renders a closed matrix document. It never selects
//! a host or treats a candidate record as support, release, install, mutation,
//! signing, trust, or private-key authority.

use std::path::{Path, PathBuf};

use crate::cli_error::ExitError;
use crate::cli_util::{command_surface_usage, emit_envelope, emit_envelope_with};
use crate::io_util::read_regular_file_no_follow_bounded;
use forge_core_command_surface::{CommandSpec, COMMAND_HOST_SUPPORT_MATRIX};
use forge_core_contracts::{CliEnvelope, ExitReason, HostSupportMatrixDocument};

const COMMAND: &str = "host-support-matrix.show";
const MAX_MATRIX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostSupportMatrixArgs {
    matrix_file: PathBuf,
    json: bool,
}

fn usage(command: &CommandSpec) -> String {
    command_surface_usage(command)
}

/// Run `host-support-matrix show`.
///
/// The matrix file is a candidate-only input. Successful parsing only reports
/// its closed shape; `selected_host` remains none by contract validation.
///
/// # Errors
///
/// Returns an error when argv is malformed, the matrix cannot be read or
/// validated, or the result envelope cannot be emitted.
pub fn run_host_support_matrix_command(args: &[String]) -> Result<(), ExitError> {
    if args
        .iter()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{}", usage(&COMMAND_HOST_SUPPORT_MATRIX));
        return Ok(());
    }
    let parsed = parse_args(args)?;
    match load_matrix(&parsed.matrix_file) {
        Ok(matrix) => {
            let summary = format!(
                "forge_core_host_support_matrix matrix_id={} selected_host=none records={}",
                matrix.host_support_matrix.matrix_id,
                matrix.host_support_matrix.records.len()
            );
            emit_envelope_with(
                CliEnvelope::ok(COMMAND, matrix),
                parsed.json,
                Some(&summary),
            )
        }
        Err(message) => emit_envelope(
            CliEnvelope::<HostSupportMatrixDocument>::err(COMMAND, ExitReason::EnvConfig, message),
            parsed.json,
        ),
    }
}

fn parse_args(args: &[String]) -> Result<HostSupportMatrixArgs, ExitError> {
    let command = &COMMAND_HOST_SUPPORT_MATRIX;
    let mut matrix_file = None::<PathBuf>;
    let mut json = false;
    let mut index = 1;

    if args.get(index).map(String::as_str) == Some("show") {
        index += 1;
    } else if args.get(index).is_some() && !matches!(args[index].as_str(), "--help" | "-h") {
        return Err(ExitError::usage(usage(command)));
    }

    while index < args.len() {
        match args[index].as_str() {
            "--matrix-file" => {
                index += 1;
                let value = args
                    .get(index)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| ExitError::usage(usage(command)))?;
                if matrix_file.replace(PathBuf::from(value)).is_some() {
                    return Err(ExitError::usage(usage(command)));
                }
            }
            "--json" => json = true,
            "--no-json" => json = false,
            _ => return Err(ExitError::usage(usage(command))),
        }
        index += 1;
    }

    Ok(HostSupportMatrixArgs {
        matrix_file: matrix_file.ok_or_else(|| ExitError::usage(usage(command)))?,
        json,
    })
}

fn load_matrix(path: &Path) -> Result<HostSupportMatrixDocument, String> {
    let bytes = read_regular_file_no_follow_bounded(path, MAX_MATRIX_BYTES).map_err(|error| {
        format!(
            "cannot read host support matrix {}: {error}",
            path.display()
        )
    })?;
    decode_matrix(path, &bytes)
}

fn decode_matrix(path: &Path, bytes: &[u8]) -> Result<HostSupportMatrixDocument, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        format!(
            "host support matrix {} is not UTF-8: {error}",
            path.display()
        )
    })?;
    let matrix: HostSupportMatrixDocument = yaml_serde::from_str(text)
        .map_err(|error| format!("invalid host support matrix {}: {error}", path.display()))?;
    matrix
        .validate()
        .map_err(|error| format!("invalid host support matrix {}: {error}", path.display()))?;
    Ok(matrix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn rejects_missing_matrix_file() {
        assert!(matches!(
            parse_args(&args(&["host-support-matrix", "show"])),
            Err(ExitError::Usage { .. })
        ));
    }

    #[test]
    fn rejects_duplicate_matrix_file() {
        assert!(matches!(
            parse_args(&args(&[
                "host-support-matrix",
                "show",
                "--matrix-file",
                "a.yaml",
                "--matrix-file",
                "b.yaml"
            ])),
            Err(ExitError::Usage { .. })
        ));
    }

    #[test]
    fn decodes_empty_candidate_matrix_without_selecting_a_host() {
        let matrix = decode_matrix(
            Path::new("matrix.yaml"),
            br#"schema_version: "0.1"
host_support_matrix:
  matrix_id: "candidate-host-support-v0"
  authority: "candidate_only"
  selected_host: null
  serialization_boundary:
    grants_support_authority: false
    grants_release_authority: false
    grants_install_authority: false
    grants_mutation_authority: false
    grants_signing_authority: false
    grants_trust_authority: false
    grants_private_key_authority: false
    grants_host_selection_authority: false
  records: []
"#,
        )
        .expect("candidate-only matrix");
        assert!(matrix.host_support_matrix.selected_host.is_none());
        assert!(matrix.host_support_matrix.records.is_empty());
    }
}
