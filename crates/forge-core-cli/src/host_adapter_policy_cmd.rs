//! Host-adapter policy / admission / projection / manifest CLI dispatchers.
//!
//! Each `run_host_adapter_*_command` here parses argv into typed inputs
//! (`HostAdapterDistributionAdmissionInput`, `HostAdapterInvocationRequest`,
//! `HostAdapterProjectionTarget`, `HostAdapterProcessTarget`), calls the
//! corresponding entrypoint re-exported from `forge_core_crypto`, prints
//! the result as JSON or human-readable text, and exits non-zero on
//! verification failure.
//!
//! Extracted from the legacy god-file `main.rs` as part of R11.3.

use crate::cli_error::ExitError;
use crate::cli_util::{
    command_surface_usage, parse_host_adapter_process_target_or_err,
    parse_host_adapter_projection_target_or_err, parse_runtime_kind_or_err,
    parse_update_channel_or_err,
};
use crate::{
    run_host_adapter_distribution_admission, run_host_adapter_distribution_policy,
    run_host_adapter_invocation_admission, run_host_adapter_manifest,
    run_host_adapter_process_security_policy, run_host_adapter_projection,
    HostAdapterDistributionAdmissionStatus, HostAdapterDistributionEvidence,
    HostAdapterInvocationAdmissionStatus, HostAdapterInvocationRequest, HostAdapterProcessTarget,
    HostAdapterProjectionTarget, HostAdapterUpdateChannel,
};
use forge_core_command_surface::{
    CommandSpec, COMMAND_HOST_ADAPTER_ADMIT_DISTRIBUTION, COMMAND_HOST_ADAPTER_ADMIT_INVOCATION,
    COMMAND_HOST_ADAPTER_DISTRIBUTION_POLICY, COMMAND_HOST_ADAPTER_MANIFEST,
    COMMAND_HOST_ADAPTER_PROCESS_POLICY, COMMAND_HOST_ADAPTER_PROJECTION,
};
use forge_core_contracts::runtime::RuntimeKind;

fn host_adapter_command_usage(command: &CommandSpec) -> String {
    command_surface_usage(command)
}

fn next_arg_or_command_usage<'a>(
    args: &'a [String],
    index: usize,
    command: &CommandSpec,
) -> Result<&'a str, ExitError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(host_adapter_command_usage(command)))
}

fn parse_runtime_kind_or_command_usage(
    value: &str,
    command: &CommandSpec,
) -> Result<RuntimeKind, ExitError> {
    parse_runtime_kind_or_err(value)
        .map_err(|_| ExitError::usage(host_adapter_command_usage(command)))
}

fn parse_update_channel_or_command_usage(
    value: &str,
    command: &CommandSpec,
) -> Result<HostAdapterUpdateChannel, ExitError> {
    parse_update_channel_or_err(value)
        .map_err(|_| ExitError::usage(host_adapter_command_usage(command)))
}

fn parse_process_target_or_command_usage(
    value: &str,
    command: &CommandSpec,
) -> Result<HostAdapterProcessTarget, ExitError> {
    parse_host_adapter_process_target_or_err(value)
        .map_err(|_| ExitError::usage(host_adapter_command_usage(command)))
}

fn parse_projection_target_or_command_usage(
    value: &str,
    command: &CommandSpec,
) -> Result<HostAdapterProjectionTarget, ExitError> {
    parse_host_adapter_projection_target_or_err(value)
        .map_err(|_| ExitError::usage(host_adapter_command_usage(command)))
}

/// Runs the `host-adapter-distribution-policy` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present in `args`.
///
/// # Panics
///
/// Panics if the distribution policy cannot be serialized as JSON. The
/// policy type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_distribution_policy_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_HOST_ADAPTER_DISTRIBUTION_POLICY;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_command_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_command_usage(command)));
            }
        }
        index += 1;
    }

    let policy = run_host_adapter_distribution_policy();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&policy)
                .expect("serialize host adapter distribution policy")
        );
    } else {
        println!(
            "forge_core_host_adapter_distribution_policy default={:?} targets={}",
            policy.default_admission,
            policy.supported_runtime_targets.len()
        );
    }
    Ok(())
}

/// Runs the `host-adapter-admit-distribution` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the distribution admission comes back as
/// `Blocked`.
///
/// # Panics
///
/// Panics if the admission result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_admit_distribution_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_HOST_ADAPTER_ADMIT_DISTRIBUTION;
    let mut target = RuntimeKind::Codex;
    let mut channel = HostAdapterUpdateChannel::Stable;
    let mut artifact_name: Option<String> = None;
    let mut artifact_sha256 = None;
    let mut signature_ref = None;
    let mut provenance_ref = None;
    let mut source_ref = None;
    let mut version = None;
    let mut compatible_core_version = None;
    let mut rollback_ref = None;
    let mut update_summary_ref = None;
    let mut explicit_canary_opt_in = false;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                index += 1;
                target = parse_runtime_kind_or_command_usage(
                    next_arg_or_command_usage(args, index, command)?,
                    command,
                )?;
            }
            "--channel" => {
                index += 1;
                channel = parse_update_channel_or_command_usage(
                    next_arg_or_command_usage(args, index, command)?,
                    command,
                )?;
            }
            "--artifact" => {
                index += 1;
                artifact_name = Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--sha256" => {
                index += 1;
                artifact_sha256 =
                    Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--signature-ref" => {
                index += 1;
                signature_ref = Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--provenance-ref" => {
                index += 1;
                provenance_ref = Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--source-ref" => {
                index += 1;
                source_ref = Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--version" => {
                index += 1;
                version = Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--compatible-core-version" => {
                index += 1;
                compatible_core_version =
                    Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--rollback-ref" => {
                index += 1;
                rollback_ref = Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--update-summary-ref" => {
                index += 1;
                update_summary_ref =
                    Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--explicit-canary-opt-in" => explicit_canary_opt_in = true,
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_command_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_command_usage(command)));
            }
        }
        index += 1;
    }

    let Some(artifact_name) = artifact_name else {
        return Err(ExitError::usage(host_adapter_command_usage(command)));
    };
    let admission = run_host_adapter_distribution_admission(HostAdapterDistributionEvidence {
        target,
        channel,
        artifact_name,
        artifact_sha256,
        signature_ref,
        provenance_ref,
        source_ref,
        version,
        compatible_core_version,
        rollback_ref,
        update_summary_ref,
        explicit_canary_opt_in,
    });
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&admission)
                .expect("serialize host adapter distribution admission")
        );
    } else {
        println!(
            "forge_core_host_adapter_distribution_admission artifact={} status={:?} reasons={:?}",
            admission.artifact_name, admission.status, admission.reasons
        );
    }
    if admission.status == HostAdapterDistributionAdmissionStatus::Blocked {
        return Err(ExitError::failed("distribution admission blocked"));
    }
    Ok(())
}

/// Runs the `host-adapter-process-policy` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid.
///
/// # Panics
///
/// Panics if the process policy cannot be serialized as JSON. The
/// policy type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_process_policy_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_HOST_ADAPTER_PROCESS_POLICY;
    let mut target = HostAdapterProcessTarget::McpStdio;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                index += 1;
                target = parse_process_target_or_command_usage(
                    next_arg_or_command_usage(args, index, command)?,
                    command,
                )?;
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_command_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_command_usage(command)));
            }
        }
        index += 1;
    }

    let policy = run_host_adapter_process_security_policy(target);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&policy).expect("serialize host adapter process policy")
        );
    } else {
        println!(
            "forge_core_host_adapter_process_policy target={:?} default={:?} commands={}",
            policy.target,
            policy.default_admission,
            policy.command_admissions.len()
        );
    }
    Ok(())
}

/// Runs the `host-adapter-admit-invocation` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the invocation admission comes back as
/// `Blocked`.
///
/// # Panics
///
/// Panics if the admission result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
#[allow(clippy::similar_names)]
pub fn run_host_adapter_admit_invocation_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_HOST_ADAPTER_ADMIT_INVOCATION;
    let mut command_name: Option<String> = None;
    let mut target = HostAdapterProcessTarget::McpStdio;
    let mut explicit_invocation = false;
    let mut argv = Vec::new();
    let mut cwd = None;
    let mut env_keys = Vec::new();
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--command" => {
                index += 1;
                command_name = Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--target" => {
                index += 1;
                target = parse_process_target_or_command_usage(
                    next_arg_or_command_usage(args, index, command)?,
                    command,
                )?;
            }
            "--explicit" => explicit_invocation = true,
            "--argv" => {
                index += 1;
                argv.push(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--cwd" => {
                index += 1;
                cwd = Some(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--env-key" => {
                index += 1;
                env_keys.push(next_arg_or_command_usage(args, index, command)?.to_string());
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_command_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_command_usage(command)));
            }
        }
        index += 1;
    }

    let Some(command_name) = command_name else {
        return Err(ExitError::usage(host_adapter_command_usage(command)));
    };
    let admission = run_host_adapter_invocation_admission(HostAdapterInvocationRequest {
        command_name,
        target,
        explicit_invocation,
        argv,
        cwd,
        env_keys,
    });
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&admission)
                .expect("serialize host adapter invocation admission")
        );
    } else {
        println!(
            "forge_core_host_adapter_invocation_admission command={} status={:?} reasons={:?}",
            admission.command_name, admission.status, admission.reasons
        );
    }
    if admission.status == HostAdapterInvocationAdmissionStatus::Blocked {
        return Err(ExitError::failed("invocation admission blocked"));
    }
    Ok(())
}

/// Runs the `host-adapter-projection` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid.
///
/// # Panics
///
/// Panics if the projection cannot be serialized as JSON. The
/// projection type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_projection_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_HOST_ADAPTER_PROJECTION;
    let mut target = HostAdapterProjectionTarget::McpTools;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                index += 1;
                target = parse_projection_target_or_command_usage(
                    next_arg_or_command_usage(args, index, command)?,
                    command,
                )?;
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_command_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_command_usage(command)));
            }
        }
        index += 1;
    }

    let projection = run_host_adapter_projection(target);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&projection).expect("serialize host adapter projection")
        );
    } else {
        println!(
            "forge_core_host_adapter_projection target={:?} commands={} authoritative={}",
            projection.target,
            projection.commands.len(),
            projection.projection_authoritative
        );
    }
    Ok(())
}

/// Runs the `host-adapter-manifest` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present in `args`.
///
/// # Panics
///
/// Panics if the manifest cannot be serialized as JSON. The
/// manifest type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_manifest_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_HOST_ADAPTER_MANIFEST;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_command_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_command_usage(command)));
            }
        }
        index += 1;
    }

    let manifest = run_host_adapter_manifest();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&manifest).expect("serialize host adapter manifest")
        );
    } else {
        println!(
            "forge_core_host_adapter_manifest schema_version={} commands={}",
            manifest.schema_version,
            manifest.commands.len()
        );
        for command in &manifest.commands {
            println!(
                "command={} mutation={:?} authority={:?}",
                command.name, command.mutation_class, command.authority_class
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(slice: &[&str]) -> Vec<String> {
        slice.iter().map(std::string::ToString::to_string).collect()
    }

    #[test]
    fn host_adapter_policy_usage_projects_command_surface_lines() {
        for command in [
            &COMMAND_HOST_ADAPTER_DISTRIBUTION_POLICY,
            &COMMAND_HOST_ADAPTER_ADMIT_DISTRIBUTION,
            &COMMAND_HOST_ADAPTER_PROCESS_POLICY,
            &COMMAND_HOST_ADAPTER_ADMIT_INVOCATION,
            &COMMAND_HOST_ADAPTER_PROJECTION,
            &COMMAND_HOST_ADAPTER_MANIFEST,
        ] {
            let usage = host_adapter_command_usage(command);
            for line in command.usage_lines {
                let canonical = line.trim_start();
                assert!(
                    usage.contains(canonical),
                    "{} usage should include projected Command Surface line {canonical:?}: {usage}",
                    command.name
                );
            }
            assert!(
                usage.contains("[--json|--no-json]"),
                "{} usage should preserve the shared JSON/text contract: {usage}",
                command.name
            );
        }
    }

    #[test]
    fn explicit_no_json_is_accepted_by_host_adapter_policy_commands() {
        assert!(
            run_host_adapter_distribution_policy_command(&args(&[
                "host-adapter-distribution-policy",
                "--no-json",
            ]))
            .is_ok(),
            "distribution-policy should accept explicit --no-json"
        );
        assert!(
            run_host_adapter_process_policy_command(&args(&[
                "host-adapter-process-policy",
                "--no-json",
            ]))
            .is_ok(),
            "process-policy should accept explicit --no-json"
        );
        assert!(
            run_host_adapter_projection_command(&args(&["host-adapter-projection", "--no-json"]))
                .is_ok(),
            "projection should accept explicit --no-json"
        );
        assert!(
            run_host_adapter_manifest_command(&args(&["host-adapter-manifest", "--no-json"]))
                .is_ok(),
            "manifest should accept explicit --no-json"
        );
    }

    #[test]
    fn required_host_adapter_inputs_report_command_specific_usage() {
        let distribution_error = run_host_adapter_admit_distribution_command(&args(&[
            "host-adapter-admit-distribution",
            "--no-json",
        ]))
        .expect_err("missing artifact should be usage");
        assert_eq!(
            distribution_error.message(),
            host_adapter_command_usage(&COMMAND_HOST_ADAPTER_ADMIT_DISTRIBUTION)
        );

        let invocation_error = run_host_adapter_admit_invocation_command(&args(&[
            "host-adapter-admit-invocation",
            "--no-json",
        ]))
        .expect_err("missing command should be usage");
        assert_eq!(
            invocation_error.message(),
            host_adapter_command_usage(&COMMAND_HOST_ADAPTER_ADMIT_INVOCATION)
        );
    }

    #[test]
    fn invalid_host_adapter_flags_report_command_specific_usage() {
        let error = run_host_adapter_manifest_command(&args(&[
            "host-adapter-manifest",
            "--definitely-not-a-real-flag",
        ]))
        .expect_err("unknown flag should be usage");
        assert_eq!(
            error.message(),
            host_adapter_command_usage(&COMMAND_HOST_ADAPTER_MANIFEST)
        );
    }
}
