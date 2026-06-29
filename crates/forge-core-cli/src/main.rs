use forge_core_cli::cli_util::*;
use forge_core_cli::eval_cmd::EvalCompareCommandInput;
use forge_core_cli::graph_cmd::{GraphCommandInput, GraphCommandKind, GraphCommandStatus};
use forge_core_cli::host_adapter_policy_cmd;
use forge_core_cli::host_adapter_verify_cmd;
use forge_core_cli::m1_cmd::{M1CommandInput, M1CommandKind};
use forge_core_cli::telemetry_cmd::{TelemetryExportCommandInput, TelemetryExportFormat};
use forge_core_cli::{
    run_execute_operation, run_host_adapter_artifact_verification,
    run_host_adapter_certificate_crl_status_verification,
    run_host_adapter_certificate_ocsp_status_verification,
    run_host_adapter_certificate_revocation_policy_verification,
    run_host_adapter_certificate_transparency_sct_verification,
    run_host_adapter_distribution_admission, run_host_adapter_distribution_policy,
    run_host_adapter_fulcio_certificate_identity_verification,
    run_host_adapter_invocation_admission, run_host_adapter_manifest,
    run_host_adapter_process_security_policy, run_host_adapter_projection,
    run_host_adapter_provenance_verification, run_host_adapter_rekor_verification,
    run_host_adapter_sigstore_bundle_subject_verification,
    run_host_adapter_sigstore_dsse_in_toto_subject_verification,
    run_host_adapter_sigstore_timestamp_authority_verification,
    run_host_adapter_sigstore_trust_policy_verification,
    run_host_adapter_tuf_trusted_root_freshness_verification, run_query_effect_index,
    run_query_effect_index_context, run_rebuild_effect_index, run_validate, ExecuteOperationInput,
    HostAdapterArtifactVerificationInput, HostAdapterArtifactVerificationStatus,
    HostAdapterCertificateCrlStatusVerificationInput,
    HostAdapterCertificateCrlStatusVerificationStatus,
    HostAdapterCertificateOcspStatusVerificationInput,
    HostAdapterCertificateOcspStatusVerificationStatus,
    HostAdapterCertificateRevocationPolicyVerificationInput,
    HostAdapterCertificateRevocationPolicyVerificationStatus,
    HostAdapterCertificateTransparencySctVerificationInput,
    HostAdapterCertificateTransparencySctVerificationStatus,
    HostAdapterDistributionAdmissionStatus, HostAdapterDistributionEvidence,
    HostAdapterFulcioCertificateIdentityVerificationInput,
    HostAdapterFulcioCertificateIdentityVerificationStatus, HostAdapterInvocationAdmissionStatus,
    HostAdapterInvocationRequest, HostAdapterProcessTarget, HostAdapterProjectionTarget,
    HostAdapterProvenanceVerificationInput, HostAdapterProvenanceVerificationStatus,
    HostAdapterRekorVerificationInput, HostAdapterRekorVerificationStatus,
    HostAdapterSigstoreBundleSubjectVerificationInput,
    HostAdapterSigstoreBundleSubjectVerificationStatus,
    HostAdapterSigstoreDsseInTotoSubjectVerificationInput,
    HostAdapterSigstoreDsseInTotoSubjectVerificationStatus,
    HostAdapterSigstoreTimestampAuthorityVerificationInput,
    HostAdapterSigstoreTimestampAuthorityVerificationStatus,
    HostAdapterSigstoreTrustPolicyVerificationInput,
    HostAdapterSigstoreTrustPolicyVerificationStatus,
    HostAdapterTufTrustedRootFreshnessVerificationInput,
    HostAdapterTufTrustedRootFreshnessVerificationStatus, HostAdapterUpdateChannel,
    PayloadFileSpec, PayloadLoadPolicy, QueryEffectIndexInput, RebuildEffectIndexInput,
};
use forge_core_contracts::runtime::RuntimeKind;
use forge_core_contracts::tool_effect::EffectTargetKind;
use forge_core_eval::{EvalArmLabel, EvalCompareStatus};
use forge_core_store::{EffectMetadataAdapterTrigger, EffectMetadataConsumerUse};
use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("validate");
    match command {
        "guide" => forge_core_cli::guide::run_guide_command(&args),
        "claim" => forge_core_cli::claim::run_claim_command(&args),
        "autonomy" => forge_core_cli::autonomy_cmd::run_autonomy_command(&args),
        "contract" => forge_core_cli::contract_cmd::run_contract_command(&args),
        "isolation" => run_isolation_command(&args),
        "coordination" => run_coordination_command(&args),
        "project" => run_project_command(&args),
        "graph" => forge_core_cli::graph_cmd::run_graph_command(&args),
        "eval" => forge_core_cli::eval_cmd::run_eval_command(&args),
        "telemetry" => forge_core_cli::telemetry_cmd::run_telemetry_command(&args),
        "preview" => forge_core_cli::m1_cmd::run_m1_command(&args, M1CommandKind::Preview),
        "ready" => forge_core_cli::m1_cmd::run_m1_command(&args, M1CommandKind::Ready),
        "explain" => forge_core_cli::m1_cmd::run_m1_command(&args, M1CommandKind::Explain),
        "validate" => forge_core_cli::validate::run_validate_command(&args),
        "execute-operation" => {
            forge_core_cli::execute_operation::run_execute_operation_command(&args)
        }
        "rebuild-effect-index" => {
            forge_core_cli::effect_index::run_rebuild_effect_index_command(&args)
        }
        "query-effect-index" => forge_core_cli::effect_index::run_query_effect_index_command(&args),
        "host-adapter-manifest" => {
            host_adapter_policy_cmd::run_host_adapter_manifest_command(&args)
        }
        "host-adapter-projection" => {
            host_adapter_policy_cmd::run_host_adapter_projection_command(&args)
        }
        "host-adapter-process-policy" => {
            host_adapter_policy_cmd::run_host_adapter_process_policy_command(&args)
        }
        "host-adapter-admit-invocation" => {
            host_adapter_policy_cmd::run_host_adapter_admit_invocation_command(&args)
        }
        "host-adapter-distribution-policy" => {
            host_adapter_policy_cmd::run_host_adapter_distribution_policy_command(&args)
        }
        "host-adapter-admit-distribution" => {
            host_adapter_policy_cmd::run_host_adapter_admit_distribution_command(&args)
        }
        "host-adapter-verify-artifact" => {
            host_adapter_verify_cmd::run_host_adapter_verify_artifact_command(&args)
        }
        "host-adapter-verify-provenance" => {
            host_adapter_verify_cmd::run_host_adapter_verify_provenance_command(&args)
        }
        "host-adapter-verify-rekor-entry" => {
            host_adapter_verify_cmd::run_host_adapter_verify_rekor_entry_command(&args)
        }
        "host-adapter-verify-sigstore-trust-policy" => {
            host_adapter_verify_cmd::run_host_adapter_verify_sigstore_trust_policy_command(&args)
        }
        "host-adapter-verify-fulcio-certificate-identity" => {
            host_adapter_verify_cmd::run_host_adapter_verify_fulcio_certificate_identity_command(
                &args,
            )
        }
        "host-adapter-verify-sigstore-bundle-subject" => {
            host_adapter_verify_cmd::run_host_adapter_verify_sigstore_bundle_subject_command(&args)
        }
        "host-adapter-verify-sigstore-dsse-in-toto-subject" => {
            host_adapter_verify_cmd::run_host_adapter_verify_sigstore_dsse_in_toto_subject_command(
                &args,
            )
        }
        "host-adapter-verify-sigstore-timestamp-authority" => {
            host_adapter_verify_cmd::run_host_adapter_verify_sigstore_timestamp_authority_command(
                &args,
            )
        }
        "host-adapter-verify-certificate-transparency-sct" => {
            host_adapter_verify_cmd::run_host_adapter_verify_certificate_transparency_sct_command(
                &args,
            )
        }
        "host-adapter-verify-certificate-revocation-policy" => {
            host_adapter_verify_cmd::run_host_adapter_verify_certificate_revocation_policy_command(
                &args,
            )
        }
        "host-adapter-verify-tuf-trusted-root-freshness" => {
            host_adapter_verify_cmd::run_host_adapter_verify_tuf_trusted_root_freshness_command(
                &args,
            )
        }
        "host-adapter-verify-certificate-crl-status" => {
            host_adapter_verify_cmd::run_host_adapter_verify_certificate_crl_status_command(&args)
        }
        "host-adapter-verify-certificate-ocsp-status" => {
            host_adapter_verify_cmd::run_host_adapter_verify_certificate_ocsp_status_command(&args);
        }
        "--help" | "-h" => println!("{}", usage()),
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

// ============================================================================
// claim command family — governance surface (slice 4, S4.4). Same envelope
// contract as guide/* (DD17).
// ============================================================================

// ============================================================================
// isolation (layer-1 worktree governance, S4.6)
// ============================================================================

fn run_isolation_command(args: &[String]) {
    let sub = args.get(1).map(String::as_str).unwrap_or("--help");
    match sub {
        "propose" => run_isolation_propose(&args[2..]),
        "status" => run_isolation_status(&args[2..]),
        "merge-plan" => run_isolation_merge_plan(&args[2..]),
        "transition" => run_isolation_transition(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("forge-core isolation <subcommand> [options]");
            println!("  propose [--root <path>] [--allow-bootstrap-core] --agent <id> --branch <name> --worktree-path <path> --base-ref <ref> [--id <isolation-id>] [--merge-policy rebase|merge|squash] [--claim <claim-id>] [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  status [--root <path>] [--allow-bootstrap-core] [--agent <id>] [--isolation-dir <path>] [--no-json]");
            println!("  merge-plan [--root <path>] [--allow-bootstrap-core] --id <isolation-id> [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  transition [--root <path>] [--allow-bootstrap-core] --id <isolation-id> --to proposed|active|merging|merged|abandoned [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  Defaults: without --isolation-dir, resolves --root as a Forge project and uses <state_root>/contracts/isolations; --isolation-dir is an explicit override.");
        }
        other => {
            eprintln!("forge-core isolation: unknown subcommand '{other}'. Try: propose | status | merge-plan | transition");
            std::process::exit(2);
        }
    }
}

fn run_project_command(args: &[String]) {
    let (output, exit) = forge_core_cli::project_cmd::dispatch(args);
    if !output.is_empty() {
        println!("{output}");
    }
    std::process::exit(exit);
}

fn run_coordination_command(args: &[String]) {
    use forge_core_cli::coordination::dispatch;
    let (json, exit) = dispatch(args);
    if !json.is_empty() {
        println!("{json}");
    }
    std::process::exit(exit);
}

#[must_use]
fn resolve_isolation_dir_or_exit(
    command: &str,
    isolation_dir: Option<PathBuf>,
    root: &std::path::Path,
    allow_bootstrap_core: bool,
    want_json: bool,
) -> PathBuf {
    if let Some(isolation_dir) = isolation_dir {
        return isolation_dir;
    }

    match forge_core_cli::project_cmd::resolve_project(root, allow_bootstrap_core) {
        Ok(project) if project.state_exists => {
            let state_root = PathBuf::from(project.state_root);
            if state_root.is_dir() {
                state_root.join("contracts").join("isolations")
            } else {
                let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                    command,
                    forge_core_contracts::ExitReason::EnvConfig,
                    format!(
                        "resolved Forge state_root is not a directory for isolation command: {}; fix {} or recreate the sidecar .forge-method directory",
                        state_root.display(),
                        forge_core_contracts::PROJECT_LINK_FILE_NAME
                    ),
                );
                emit_envelope("isolation", env, want_json);
                unreachable!("emit_envelope exits the process");
            }
        }
        Ok(project) => {
            let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                command,
                forge_core_contracts::ExitReason::EnvConfig,
                format!(
                    "resolved Forge state_root does not exist for isolation command: {}; create the sidecar .forge-method directory or fix {}",
                    project.state_root,
                    forge_core_contracts::PROJECT_LINK_FILE_NAME
                ),
            );
            emit_envelope("isolation", env, want_json);
            unreachable!("emit_envelope exits the process");
        }
        Err(err) => {
            let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                command,
                err.exit_reason(),
                format!("project resolve failed for isolation command: {err}"),
            );
            emit_envelope("isolation", env, want_json);
            unreachable!("emit_envelope exits the process");
        }
    }
}

fn run_isolation_propose(args: &[String]) {
    use forge_core_cli::claim::slug_for_file;
    use forge_core_cli::isolation::{parse_merge_policy, run_propose};
    use forge_core_contracts::isolation::MergePolicy;
    use forge_core_contracts::StableId;

    let mut isolation_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut agent = String::new();
    let mut branch = String::new();
    let mut worktree_path = String::new();
    let mut base_ref = String::from("main");
    let mut merge_policy = MergePolicy::Rebase;
    let mut claim_id: Option<String> = None;
    let mut isolation_id: Option<String> = None;

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value(args, idx, "root"));
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--agent" => {
                idx += 1;
                agent = require_value(args, idx, "agent");
            }
            "--branch" => {
                idx += 1;
                branch = require_value(args, idx, "branch");
            }
            "--worktree-path" => {
                idx += 1;
                worktree_path = require_value(args, idx, "worktree-path");
            }
            "--base-ref" => {
                idx += 1;
                base_ref = require_value(args, idx, "base-ref");
            }
            "--id" => {
                idx += 1;
                isolation_id = Some(require_value(args, idx, "id"));
            }
            "--merge-policy" => {
                idx += 1;
                merge_policy = match parse_merge_policy(&require_value(args, idx, "merge-policy")) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("isolation propose: {e}");
                        std::process::exit(3);
                    }
                };
            }
            "--claim" => {
                idx += 1;
                claim_id = Some(require_value(args, idx, "claim"));
            }
            "--isolation-dir" => {
                idx += 1;
                isolation_dir = Some(PathBuf::from(require_value(args, idx, "isolation-dir")));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict(
                    &require_value(args, idx, "now-unix"),
                    "now-unix",
                ));
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core isolation propose [--root <path>] [--allow-bootstrap-core] --agent <id> --branch <name> --worktree-path <path> --base-ref <ref> [--id <id>] [--merge-policy rebase|merge|squash] [--claim <claim-id>] [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }
    if agent.is_empty() || branch.is_empty() || worktree_path.is_empty() {
        eprintln!("isolation propose: --agent, --branch, --worktree-path are all required");
        std::process::exit(3);
    }
    let now = resolve_now_unix(now_unix);
    let id = isolation_id.unwrap_or_else(|| format!("iso-{}-{}", slug_for_file(&branch), now));
    let isolation_dir = resolve_isolation_dir_or_exit(
        "isolation.propose",
        isolation_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    let env = run_propose(
        &isolation_dir,
        &StableId(agent),
        &branch,
        &worktree_path,
        &base_ref,
        merge_policy,
        claim_id.map(StableId),
        &id,
        now,
    );
    emit_envelope("isolation", env, want_json);
}

fn run_isolation_status(args: &[String]) {
    use forge_core_cli::isolation::run_status;
    use forge_core_contracts::StableId;
    let mut isolation_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut want_json = true;
    let mut agent: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value(args, idx, "root"));
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--agent" => {
                idx += 1;
                agent = Some(require_value(args, idx, "agent"));
            }
            "--isolation-dir" => {
                idx += 1;
                isolation_dir = Some(PathBuf::from(require_value(args, idx, "isolation-dir")));
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core isolation status [--root <path>] [--allow-bootstrap-core] [--agent <id>] [--isolation-dir <path>] [--no-json]");
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }
    let isolation_dir = resolve_isolation_dir_or_exit(
        "isolation.status",
        isolation_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    let env = run_status(
        &isolation_dir,
        agent.as_ref().map(|a| StableId(a.clone())).as_ref(),
    );
    emit_envelope("isolation", env, want_json);
}

fn run_isolation_merge_plan(args: &[String]) {
    use forge_core_cli::isolation::run_merge_plan;
    use forge_core_contracts::StableId;
    let mut isolation_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut id = String::new();
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value(args, idx, "root"));
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--id" => {
                idx += 1;
                id = require_value(args, idx, "id");
            }
            "--isolation-dir" => {
                idx += 1;
                isolation_dir = Some(PathBuf::from(require_value(args, idx, "isolation-dir")));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict(
                    &require_value(args, idx, "now-unix"),
                    "now-unix",
                ));
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core isolation merge-plan [--root <path>] [--allow-bootstrap-core] --id <isolation-id> [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }
    if id.is_empty() {
        eprintln!("isolation merge-plan: --id <isolation-id> is required");
        std::process::exit(3);
    }
    let isolation_dir = resolve_isolation_dir_or_exit(
        "isolation.merge-plan",
        isolation_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    let env = run_merge_plan(&isolation_dir, &StableId(id), resolve_now_unix(now_unix));
    emit_envelope("isolation", env, want_json);
}

fn run_isolation_transition(args: &[String]) {
    use forge_core_cli::isolation::{parse_status, run_transition};
    use forge_core_contracts::StableId;
    let mut isolation_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut id = String::new();
    let mut to_raw = String::new();
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value(args, idx, "root"));
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--id" => {
                idx += 1;
                id = require_value(args, idx, "id");
            }
            "--to" => {
                idx += 1;
                to_raw = require_value(args, idx, "to");
            }
            "--isolation-dir" => {
                idx += 1;
                isolation_dir = Some(PathBuf::from(require_value(args, idx, "isolation-dir")));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict(
                    &require_value(args, idx, "now-unix"),
                    "now-unix",
                ));
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core isolation transition [--root <path>] [--allow-bootstrap-core] --id <isolation-id> --to proposed|active|merging|merged|abandoned [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }
    if id.is_empty() || to_raw.is_empty() {
        eprintln!("isolation transition: --id and --to are both required");
        std::process::exit(3);
    }
    let to = match parse_status(&to_raw) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("isolation transition: {e}");
            std::process::exit(3);
        }
    };
    let isolation_dir = resolve_isolation_dir_or_exit(
        "isolation.transition",
        isolation_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    let env = run_transition(
        &isolation_dir,
        &StableId(id),
        to,
        resolve_now_unix(now_unix),
    );
    emit_envelope("isolation", env, want_json);
}

#[cfg(test)]
mod tests {
    use super::guide_value;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn guide_value_requires_present_non_flag_value() {
        let parsed = args(&["--catalog-dir", "contracts/workflows"]);
        assert_eq!(guide_value(&parsed, 1), Some("contracts/workflows"));

        let missing = args(&["--catalog-dir"]);
        assert_eq!(guide_value(&missing, 1), None);

        let next_flag = args(&["--catalog-dir", "--no-json"]);
        assert_eq!(guide_value(&next_flag, 1), None);
    }
}
