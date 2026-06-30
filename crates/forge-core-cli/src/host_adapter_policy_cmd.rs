//! Host-adapter policy / admission / projection / manifest CLI dispatchers.
//!
//! Each `run_host_adapter_*_command` here parses argv into typed inputs
//! (`HostAdapterDistributionAdmissionInput`, `HostAdapterInvocationRequest`,
//! `HostAdapterProjectionTarget`, `HostAdapterProcessTarget`), calls the
//! corresponding entrypoint re-exported from `forge_core_crypto`, prints
//! the result as JSON or human-readable text, and exits non-zero on
//! verification failure.
//!
//! Extracted from the legacy god-file `main.rs` as part of R11.3
//! (see `docs/dev-docs/forge-method-core-dev-docs-v2/09_system_design_roadmap.md`).

use crate::cli_error::ExitError;
use crate::cli_util::{
    next_arg_or_err, parse_host_adapter_process_target_or_err,
    parse_host_adapter_projection_target_or_err, parse_runtime_kind_or_err,
    parse_update_channel_or_err, usage,
};
use crate::*;
use forge_core_contracts::runtime::RuntimeKind;

pub fn run_host_adapter_distribution_policy_command(args: &[String]) -> Result<(), ExitError> {
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(usage()));
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

pub fn run_host_adapter_admit_distribution_command(args: &[String]) -> Result<(), ExitError> {
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
                target = parse_runtime_kind_or_err(next_arg_or_err(args, index)?)?;
            }
            "--channel" => {
                index += 1;
                channel = parse_update_channel_or_err(next_arg_or_err(args, index)?)?;
            }
            "--artifact" => {
                index += 1;
                artifact_name = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--sha256" => {
                index += 1;
                artifact_sha256 = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--signature-ref" => {
                index += 1;
                signature_ref = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--provenance-ref" => {
                index += 1;
                provenance_ref = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--source-ref" => {
                index += 1;
                source_ref = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--version" => {
                index += 1;
                version = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--compatible-core-version" => {
                index += 1;
                compatible_core_version = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--rollback-ref" => {
                index += 1;
                rollback_ref = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--update-summary-ref" => {
                index += 1;
                update_summary_ref = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--explicit-canary-opt-in" => explicit_canary_opt_in = true,
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(usage()));
            }
        }
        index += 1;
    }

    let Some(artifact_name) = artifact_name else {
        return Err(ExitError::usage(usage()));
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

pub fn run_host_adapter_process_policy_command(args: &[String]) -> Result<(), ExitError> {
    let mut target = HostAdapterProcessTarget::McpStdio;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                index += 1;
                target = parse_host_adapter_process_target_or_err(next_arg_or_err(args, index)?)?;
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(usage()));
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

pub fn run_host_adapter_admit_invocation_command(args: &[String]) -> Result<(), ExitError> {
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
                command_name = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--target" => {
                index += 1;
                target = parse_host_adapter_process_target_or_err(next_arg_or_err(args, index)?)?;
            }
            "--explicit" => explicit_invocation = true,
            "--argv" => {
                index += 1;
                argv.push(next_arg_or_err(args, index)?.to_string());
            }
            "--cwd" => {
                index += 1;
                cwd = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--env-key" => {
                index += 1;
                env_keys.push(next_arg_or_err(args, index)?.to_string());
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(usage()));
            }
        }
        index += 1;
    }

    let Some(command_name) = command_name else {
        return Err(ExitError::usage(usage()));
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

pub fn run_host_adapter_projection_command(args: &[String]) -> Result<(), ExitError> {
    let mut target = HostAdapterProjectionTarget::McpTools;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                index += 1;
                target =
                    parse_host_adapter_projection_target_or_err(next_arg_or_err(args, index)?)?;
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(usage()));
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

pub fn run_host_adapter_manifest_command(args: &[String]) -> Result<(), ExitError> {
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(usage()));
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
