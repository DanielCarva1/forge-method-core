use forge_core_cli::cli_error::ExitError;
use forge_core_cli::cli_util::usage;
use forge_core_cli::host_adapter_policy_cmd;
use forge_core_cli::host_adapter_verify_cmd;
use forge_core_cli::m1_cmd::M1CommandKind;
use forge_core_cli::tracing_init;
use std::env;

fn main() {
    // Initialize structured tracing BEFORE any dispatcher runs. The subscriber
    // is idempotent and writes to stderr; stdout (the JSON contract channel)
    // is untouched. Format is auto-selected from FORGE_LOG_FORMAT or stderr
    // TTY detection.
    tracing_init::init_subscriber();

    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map_or("validate", String::as_str);

    // Root session span. When FORGE_AGENT_ID is set, every nested span carries
    // `agent_id`, which lets a trace store correlate multi-agent runs (host +
    // sub-agents, N parallel workers, etc.) without parsing argv. When unset,
    // the field stays Empty so a human-driven run is distinguishable from a
    // future agent that has not yet identified itself.
    let agent_id = tracing_init::current_agent_id();
    let session_span = tracing::info_span!(
        "forge_session",
        agent_id = tracing::field::Empty,
        command = %command,
    );
    if let Some(id) = &agent_id {
        session_span.record("agent_id", id.as_str());
    }

    // Dispatchers are sync; run them inside the session span so every nested
    // `#[instrument]` inherits `agent_id` + `command` automatically.
    let result: Result<(), ExitError> = session_span.in_scope(|| dispatch(command, &args));

    // The single std::process::exit call in the entire forge-core-cli crate.
    // Every dispatcher returns Result<(), ExitError>; this block converts the
    // typed error back into the shell exit code and stderr text.
    match result {
        Ok(()) => {}
        Err(error) => {
            // The dispatcher already wrote any stdout/stderr it needed (JSON
            // envelope or text-mode failure line). The ExitError's own message
            // is non-empty only for direct usage / parse failures, where the
            // dispatcher did NOT print anything itself.
            let message = error.message();
            if !message.is_empty() {
                eprintln!("{message}");
            }
            std::process::exit(error.exit_code());
        }
    }
}

/// Pure dispatcher. Kept separate from `main` so it can be unit-tested and so
/// the span-wrapping in `main` is the only orchestration concern there.
fn dispatch(command: &str, args: &[String]) -> Result<(), ExitError> {
    match command {
        "guide" => forge_core_cli::guide::run_guide_command(args),
        "claim" => forge_core_cli::claim::run_claim_command(args),
        "autonomy" => forge_core_cli::autonomy_cmd::run_autonomy_command(args),
        "contract" => forge_core_cli::contract_cmd::run_contract_command(args),
        "isolation" => forge_core_cli::isolation::run_isolation_command(args),
        "coordination" => forge_core_cli::coordination::run_coordination_command(args),
        "project" => forge_core_cli::project_cmd::run_project_command(args),
        "graph" => forge_core_cli::graph_cmd::run_graph_command(args),
        "eval" => forge_core_cli::eval_cmd::run_eval_command(args),
        "telemetry" => forge_core_cli::telemetry_cmd::run_telemetry_command(args),
        "preview" => forge_core_cli::m1_cmd::run_m1_command(args, M1CommandKind::Preview),
        "ready" => forge_core_cli::m1_cmd::run_m1_command(args, M1CommandKind::Ready),
        "explain" => forge_core_cli::m1_cmd::run_m1_command(args, M1CommandKind::Explain),
        "validate" => forge_core_cli::validate::run_validate_command(args),
        "preflight" => forge_core_cli::preflight_cmd::run_preflight_command(args),
        "execute-operation" => {
            forge_core_cli::execute_operation::run_execute_operation_command(args)
        }
        "rebuild-effect-index" => {
            forge_core_cli::effect_index::run_rebuild_effect_index_command(args)
        }
        "query-effect-index" => forge_core_cli::effect_index::run_query_effect_index_command(args),
        "host-adapter-manifest" => host_adapter_policy_cmd::run_host_adapter_manifest_command(args),
        "host-adapter-projection" => {
            host_adapter_policy_cmd::run_host_adapter_projection_command(args)
        }
        "host-adapter-process-policy" => {
            host_adapter_policy_cmd::run_host_adapter_process_policy_command(args)
        }
        "host-adapter-admit-invocation" => {
            host_adapter_policy_cmd::run_host_adapter_admit_invocation_command(args)
        }
        "host-adapter-distribution-policy" => {
            host_adapter_policy_cmd::run_host_adapter_distribution_policy_command(args)
        }
        "host-adapter-admit-distribution" => {
            host_adapter_policy_cmd::run_host_adapter_admit_distribution_command(args)
        }
        "host-adapter-verify-artifact" => {
            host_adapter_verify_cmd::run_host_adapter_verify_artifact_command(args)
        }
        "host-adapter-verify-provenance" => {
            host_adapter_verify_cmd::run_host_adapter_verify_provenance_command(args)
        }
        "host-adapter-verify-rekor-entry" => {
            host_adapter_verify_cmd::run_host_adapter_verify_rekor_entry_command(args)
        }
        "host-adapter-verify-sigstore-trust-policy" => {
            host_adapter_verify_cmd::run_host_adapter_verify_sigstore_trust_policy_command(args)
        }
        "host-adapter-verify-fulcio-certificate-identity" => {
            host_adapter_verify_cmd::run_host_adapter_verify_fulcio_certificate_identity_command(
                args,
            )
        }
        "host-adapter-verify-sigstore-bundle-subject" => {
            host_adapter_verify_cmd::run_host_adapter_verify_sigstore_bundle_subject_command(args)
        }
        "host-adapter-verify-sigstore-dsse-in-toto-subject" => {
            host_adapter_verify_cmd::run_host_adapter_verify_sigstore_dsse_in_toto_subject_command(
                args,
            )
        }
        "host-adapter-verify-sigstore-timestamp-authority" => {
            host_adapter_verify_cmd::run_host_adapter_verify_sigstore_timestamp_authority_command(
                args,
            )
        }
        "host-adapter-verify-certificate-transparency-sct" => {
            host_adapter_verify_cmd::run_host_adapter_verify_certificate_transparency_sct_command(
                args,
            )
        }
        "host-adapter-verify-certificate-revocation-policy" => {
            host_adapter_verify_cmd::run_host_adapter_verify_certificate_revocation_policy_command(
                args,
            )
        }
        "host-adapter-verify-tuf-trusted-root-freshness" => {
            host_adapter_verify_cmd::run_host_adapter_verify_tuf_trusted_root_freshness_command(
                args,
            )
        }
        "host-adapter-verify-certificate-crl-status" => {
            host_adapter_verify_cmd::run_host_adapter_verify_certificate_crl_status_command(args)
        }
        "host-adapter-verify-certificate-ocsp-status" => {
            host_adapter_verify_cmd::run_host_adapter_verify_certificate_ocsp_status_command(args)
        }
        "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(ExitError::usage(usage())),
    }
}
