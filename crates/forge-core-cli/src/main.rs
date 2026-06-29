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
        "guide" => run_guide_command(&args),
        "claim" => run_claim_command(&args),
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

fn run_guide_command(args: &[String]) {
    // Subcommand: `forge-core guide <subcommand> [...]`.
    let sub = args.get(1).map(String::as_str).unwrap_or("--help");

    match sub {
        "describe" => run_guide_describe(&args[2..]),
        "decide" => run_guide_decide(&args[2..]),
        "status" => run_guide_status(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("forge-core guide <subcommand> [options]");
            println!("  describe [--catalog-dir <path>] [--no-json]");
            println!("  decide --decision-file <path> [--catalog-dir <path>] [--gates-file <path>] [--no-json]");
            println!("  status --phase <phase> [--catalog-dir <path>] [--no-json]");
        }
        other => {
            eprintln!("forge-core guide: unknown subcommand '{other}'. Try: describe | decide");
            std::process::exit(2);
        }
    }
}

fn guide_value(args: &[String], idx: usize) -> Option<&str> {
    args.get(idx)
        .filter(|value| !value.is_empty() && !value.starts_with("--"))
        .map(String::as_str)
}

fn require_guide_value(args: &[String], idx: usize, subcommand: &str, flag: &str) -> String {
    match guide_value(args, idx) {
        Some(value) => value.to_owned(),
        None => {
            eprintln!("guide {subcommand}: --{flag} requires a value");
            std::process::exit(3);
        }
    }
}

fn reject_unknown_guide_arg(subcommand: &str, arg: &str) -> ! {
    eprintln!("guide {subcommand}: unrecognized argument '{arg}'");
    std::process::exit(3);
}

fn run_guide_describe(args: &[String]) {
    use forge_core_cli::guide::{run_describe, DescribePayload};
    use forge_core_contracts::CliEnvelope;

    let mut catalog_dir: Option<std::path::PathBuf> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--catalog-dir" => {
                idx += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "describe",
                    "catalog-dir",
                )));
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core guide describe [--catalog-dir <path>] [--no-json]");
                return;
            }
            other => reject_unknown_guide_arg("describe", other),
        }
        idx += 1;
    }

    let env: CliEnvelope<DescribePayload> = run_describe(catalog_dir.as_deref());
    emit_guide(env, want_json);
}

fn run_guide_decide(args: &[String]) {
    use forge_core_cli::guide::{run_decide, DecideAccepted};
    use forge_core_contracts::CliEnvelope;

    let mut decision_file: Option<std::path::PathBuf> = None;
    let mut catalog_dir: Option<std::path::PathBuf> = None;
    let mut gates_file: Option<std::path::PathBuf> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--decision-file" => {
                idx += 1;
                decision_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "decide",
                    "decision-file",
                )));
            }
            "--catalog-dir" => {
                idx += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "decide",
                    "catalog-dir",
                )));
            }
            "--gates-file" => {
                idx += 1;
                gates_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "decide",
                    "gates-file",
                )));
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core guide decide --decision-file <path> [--catalog-dir <path>] [--gates-file <path>] [--no-json]");
                return;
            }
            other => reject_unknown_guide_arg("decide", other),
        }
        idx += 1;
    }

    let Some(decision_file) = decision_file else {
        eprintln!("guide decide: --decision-file is required");
        std::process::exit(3);
    };

    // Gates are optional (only needed for phase transitions). Loaded from a simple
    // YAML file: [{gate_kind: system-design, status: pass}, ...].
    let gates = load_gates(gates_file.as_deref());

    let env: CliEnvelope<DecideAccepted> =
        run_decide(&decision_file, catalog_dir.as_deref(), &gates);
    emit_guide(env, want_json);
}

fn run_guide_status(args: &[String]) {
    use forge_core_cli::guide::{run_status, StatusPayload};
    use forge_core_contracts::CliEnvelope;

    let mut phase: Option<String> = None;
    let mut catalog_dir: Option<std::path::PathBuf> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--phase" => {
                idx += 1;
                phase = Some(require_guide_value(args, idx, "status", "phase"));
            }
            "--catalog-dir" => {
                idx += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "status",
                    "catalog-dir",
                )));
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!(
                    "forge-core guide status --phase <phase> [--catalog-dir <path>] [--no-json]"
                );
                return;
            }
            other => reject_unknown_guide_arg("status", other),
        }
        idx += 1;
    }

    let Some(phase) = phase else {
        eprintln!("guide status: --phase is required");
        std::process::exit(3);
    };

    let env: CliEnvelope<StatusPayload> = run_status(catalog_dir.as_deref(), &phase);
    emit_guide(env, want_json);
}

/// Parse the gates-file into ProvidedGateResult rows. Empty/absent = no gates provided.
fn load_gates(path: Option<&std::path::Path>) -> Vec<forge_core_engine::ProvidedGateResult> {
    use forge_core_contracts::gate::GateStatus;
    use forge_core_engine::GateKind;
    let Some(path) = path else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    #[derive(serde::Deserialize)]
    struct GateRow {
        gate_kind: String,
        status: String,
    }
    let rows: Vec<GateRow> = serde_yaml::from_str(&text).unwrap_or_default();
    rows.into_iter()
        .filter_map(|r| {
            let gk = match r.gate_kind.as_str() {
                "system-design" => Some(GateKind::SystemDesign),
                "grill" | "grill-gate" => Some(GateKind::Grill),
                _ => None,
            }?;
            let status = match r.status.as_str() {
                "pass" => GateStatus::Pass,
                "fail" => GateStatus::Fail,
                "concerns" => GateStatus::Concerns,
                "missing" => GateStatus::Missing,
                _ => GateStatus::NotApplicable,
            };
            Some(forge_core_engine::ProvidedGateResult {
                gate_kind: gk,
                status,
            })
        })
        .collect()
}

/// Emit a guide envelope to stdout (JSON) or stderr (text) and exit with the envelope's code.
fn emit_guide<T: serde::Serialize>(env: forge_core_contracts::CliEnvelope<T>, want_json: bool) {
    let code = env.exit_code();
    if want_json {
        println!("{}", serde_json::to_string_pretty(&env).unwrap());
    } else if !env.ok {
        eprintln!(
            "guide failed: {}",
            env.error
                .as_ref()
                .map(|e| e.message.as_str())
                .unwrap_or("unknown")
        );
    }
    std::process::exit(code);
}

// ============================================================================
// claim command family — governance surface (slice 4, S4.4). Same envelope
// contract as guide/* (DD17).
// ============================================================================

fn run_claim_command(args: &[String]) {
    let sub = args.get(1).map(String::as_str).unwrap_or("--help");
    match sub {
        "acquire" => run_claim_acquire(&args[2..]),
        "heartbeat" => run_claim_heartbeat(&args[2..]),
        "release" => run_claim_release(&args[2..]),
        "handoff" => run_claim_handoff(&args[2..]),
        "status" => run_claim_status(&args[2..]),
        "reconcile" => run_claim_reconcile(&args[2..]),
        "check-write" => run_claim_check_write(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("forge-core claim <subcommand> [options]");
            println!("  acquire [--root <path>] [--allow-bootstrap-core] --scope <kind> --id <scope-id> --agent <id> [--path <repo-path>...] [--role worker] [--ttl 600] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  heartbeat [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  release [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  handoff [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> --summary <text> [--evidence <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  status [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  reconcile [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--now-unix <epoch>] [--loop] [--interval-ms 30000] [--max-ticks <n>] [--no-json]");
            println!("  check-write [--root <path>] [--allow-bootstrap-core] --agent <id> --target <path> [--target <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  Defaults: without --claims-dir, resolves --root as a Forge project and uses <state_root>/claims-active; --claims-dir is an explicit override.");
        }
        other => {
            eprintln!("forge-core claim: unknown subcommand '{other}'. Try: acquire | heartbeat | release | handoff | status | reconcile | check-write");
            std::process::exit(2);
        }
    }
}

/// Resolve --now-unix to epoch seconds, defaulting to real system time (DD23).
fn resolve_now_unix(flag: Option<i64>) -> i64 {
    flag.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(0))
            .unwrap_or(0)
    })
}

#[must_use]
fn resolve_claims_dir_or_exit(
    command: &str,
    claims_dir: Option<PathBuf>,
    root: &std::path::Path,
    allow_bootstrap_core: bool,
    want_json: bool,
) -> PathBuf {
    if let Some(claims_dir) = claims_dir {
        return claims_dir;
    }

    match forge_core_cli::project_cmd::resolve_project(root, allow_bootstrap_core) {
        Ok(project) if project.state_exists => {
            PathBuf::from(project.state_root).join("claims-active")
        }
        Ok(project) => {
            let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                command,
                forge_core_contracts::ExitReason::EnvConfig,
                format!(
                    "resolved Forge state_root does not exist for claim command: {}; create the sidecar .forge-method directory or fix {}",
                    project.state_root,
                    forge_core_contracts::PROJECT_LINK_FILE_NAME
                ),
            );
            emit_envelope("claim", env, want_json);
            unreachable!("emit_envelope exits the process");
        }
        Err(err) => {
            let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                command,
                err.exit_reason(),
                format!("project resolve failed for claim command: {err}"),
            );
            emit_envelope("claim", env, want_json);
            unreachable!("emit_envelope exits the process");
        }
    }
}

fn run_claim_acquire(args: &[String]) {
    use forge_core_cli::claim::{parse_role, parse_scope_kind, run_acquire};
    use forge_core_contracts::{RepoPath, ScopeId, StableId};
    use forge_core_engine::AcquireRequest;

    let mut scope_kind: Option<String> = None;
    let mut scope_id: Option<String> = None;
    let mut agent_id: Option<String> = None;
    let mut role = "worker".to_string();
    let mut ttl: u64 = 600;
    let mut heartbeat_interval: u64 = 120;
    let mut paths: Vec<String> = Vec::new();
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value(args, idx, "root"));
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--scope" => {
                idx += 1;
                scope_kind = Some(require_value(args, idx, "scope"));
            }
            "--id" => {
                idx += 1;
                scope_id = Some(require_value(args, idx, "id"));
            }
            "--agent" => {
                idx += 1;
                agent_id = Some(require_value(args, idx, "agent"));
            }
            "--role" => {
                idx += 1;
                role = require_value(args, idx, "role");
            }
            "--ttl" => {
                idx += 1;
                ttl = parse_strict(&require_value(args, idx, "ttl"), "ttl");
            }
            "--heartbeat-interval" => {
                idx += 1;
                heartbeat_interval = parse_strict(
                    &require_value(args, idx, "heartbeat-interval"),
                    "heartbeat-interval",
                );
            }
            "--path" => {
                idx += 1;
                paths.push(require_value(args, idx, "path"));
            }
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value(args, idx, "claims-dir")));
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
                println!("forge-core claim acquire [--root <path>] [--allow-bootstrap-core] --scope <kind> --id <scope-id> --agent <id> [--path <repo-path>...] [--role worker] [--ttl 600] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }

    let (Some(scope_kind_str), Some(scope_id), Some(agent_id)) = (scope_kind, scope_id, agent_id)
    else {
        eprintln!("claim acquire: --scope, --id, --agent are all required");
        std::process::exit(3);
    };
    let Some(sk) = parse_scope_kind(&scope_kind_str) else {
        eprintln!("claim acquire: unknown --scope '{scope_kind_str}'");
        std::process::exit(3);
    };
    let Some(role_kind) = parse_role(&role) else {
        eprintln!("claim acquire: unknown --role '{role}'");
        std::process::exit(3);
    };

    let req = AcquireRequest {
        scope_kind: sk,
        scope_id: ScopeId(scope_id),
        agent_id: StableId(agent_id),
        role: role_kind,
        ttl_seconds: ttl,
        heartbeat_interval_seconds: heartbeat_interval,
        paths: paths.iter().map(|p| RepoPath(p.clone())).collect(),
        product_area: None,
        expected_state_version: None,
    };
    let claims_dir = resolve_claims_dir_or_exit(
        "claim.acquire",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    let env = run_acquire(&claims_dir, &req, resolve_now_unix(now_unix));
    emit_envelope("claim", env, want_json);
}

fn run_claim_heartbeat(args: &[String]) {
    use forge_core_cli::claim::run_heartbeat;
    run_claim_single_target(args, "heartbeat", |claims_dir, claim_id, agent_id, now| {
        run_heartbeat(claims_dir, claim_id, agent_id, now)
    });
}

fn run_claim_release(args: &[String]) {
    use forge_core_cli::claim::run_release;
    run_claim_single_target(args, "release", |claims_dir, claim_id, agent_id, now| {
        run_release(claims_dir, claim_id, agent_id, now)
    });
}

/// Shared arg parsing for heartbeat/release (both take --id + --agent + optional dirs/time).
fn run_claim_single_target(
    args: &[String],
    sub: &str,
    op: impl Fn(
        &std::path::Path,
        &forge_core_contracts::StableId,
        &forge_core_contracts::StableId,
        i64,
    ) -> forge_core_contracts::CliEnvelope<forge_core_cli::claim::ClaimResult>,
) {
    use forge_core_contracts::StableId;
    let mut claim_id: Option<String> = None;
    let mut agent_id: Option<String> = None;
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
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
                claim_id = Some(require_value(args, idx, "id"));
            }
            "--agent" => {
                idx += 1;
                agent_id = Some(require_value(args, idx, "agent"));
            }
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value(args, idx, "claims-dir")));
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
                println!("forge-core claim {sub} [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }
    let (Some(claim_id), Some(agent_id)) = (claim_id, agent_id) else {
        eprintln!("claim {sub}: --id and --agent are required");
        std::process::exit(3);
    };
    let claims_dir = resolve_claims_dir_or_exit(
        &format!("claim.{sub}"),
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    let env = op(
        &claims_dir,
        &StableId(claim_id),
        &StableId(agent_id),
        resolve_now_unix(now_unix),
    );
    emit_envelope("claim", env, want_json);
}

fn run_claim_handoff(args: &[String]) {
    use forge_core_cli::claim::run_handoff;
    use forge_core_contracts::StableId;

    let mut claim_id: Option<String> = None;
    let mut agent_id: Option<String> = None;
    let mut summary: Option<String> = None;
    let mut evidence_refs: Vec<String> = Vec::new();
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
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
                claim_id = Some(require_value(args, idx, "id"));
            }
            "--agent" => {
                idx += 1;
                agent_id = Some(require_value(args, idx, "agent"));
            }
            "--summary" => {
                idx += 1;
                summary = Some(require_value(args, idx, "summary"));
            }
            "--evidence" => {
                idx += 1;
                evidence_refs.push(require_value(args, idx, "evidence"));
            }
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value(args, idx, "claims-dir")));
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
                println!("forge-core claim handoff [--root <path>] [--allow-bootstrap-core] --id <claim-id> --agent <id> --summary <text> [--evidence <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Records official context for an expired handoff-required claim, writes <state_root>/handoffs/expired-claims, marks the old claim handoff_recorded, and reopens the scope.");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }
    let (Some(claim_id), Some(agent_id), Some(summary)) = (claim_id, agent_id, summary) else {
        eprintln!("claim handoff: --id, --agent, and --summary are required");
        std::process::exit(3);
    };
    let claims_dir = resolve_claims_dir_or_exit(
        "claim.handoff",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    let env = run_handoff(
        &claims_dir,
        &StableId(claim_id),
        &StableId(agent_id),
        &summary,
        &evidence_refs,
        resolve_now_unix(now_unix),
    );
    emit_envelope("claim", env, want_json);
}

fn run_claim_status(args: &[String]) {
    use forge_core_cli::claim::run_status;
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value(args, idx, "root"));
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value(args, idx, "claims-dir")));
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
                println!("forge-core claim status [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }
    let claims_dir = resolve_claims_dir_or_exit(
        "claim.status",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    let env = run_status(&claims_dir, resolve_now_unix(now_unix));
    emit_envelope("claim", env, want_json);
}

fn run_claim_reconcile(args: &[String]) {
    use forge_core_cli::claim::run_reconcile_once;

    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut run_loop = false;
    let mut interval_ms: u64 = 30_000;
    let mut max_ticks: Option<u64> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value(args, idx, "root"));
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value(args, idx, "claims-dir")));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict(
                    &require_value(args, idx, "now-unix"),
                    "now-unix",
                ));
            }
            "--loop" => run_loop = true,
            "--interval-ms" => {
                idx += 1;
                interval_ms = parse_strict(&require_value(args, idx, "interval-ms"), "interval-ms");
            }
            "--max-ticks" => {
                idx += 1;
                max_ticks = Some(parse_strict(
                    &require_value(args, idx, "max-ticks"),
                    "max-ticks",
                ));
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core claim reconcile [--root <path>] [--allow-bootstrap-core] [--claims-dir <path>] [--now-unix <epoch>] [--loop] [--interval-ms 30000] [--max-ticks <n>] [--no-json]");
                println!("  One-shot mode is deterministic and materializes stale/expired claim statuses once.");
                println!("  --loop runs a foreground Tokio interval reconciler; missed ticks use Skip and no filesystem watcher/notify is used.");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }
    if interval_ms == 0 {
        eprintln!("claim reconcile: --interval-ms must be greater than zero");
        std::process::exit(3);
    }

    let claims_dir = resolve_claims_dir_or_exit(
        "claim.reconcile",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    if !run_loop {
        let env = run_reconcile_once(&claims_dir, resolve_now_unix(now_unix));
        emit_envelope("claim", env, want_json);
    }

    run_claim_reconcile_loop_or_exit(ClaimReconcileLoopConfig {
        claims_dir,
        now_unix,
        interval_ms,
        max_ticks,
        want_json,
    });
}

#[derive(Debug, Clone)]
struct ClaimReconcileLoopConfig {
    claims_dir: PathBuf,
    now_unix: Option<i64>,
    interval_ms: u64,
    max_ticks: Option<u64>,
    want_json: bool,
}

fn run_claim_reconcile_loop_or_exit(config: ClaimReconcileLoopConfig) -> ! {
    use forge_core_cli::claim::run_reconcile_once;
    use std::time::Duration;
    use tokio::time::{interval_at, Instant, MissedTickBehavior};

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("claim reconcile: cannot build Tokio runtime: {error}");
            std::process::exit(5);
        }
    };
    let ClaimReconcileLoopConfig {
        claims_dir,
        now_unix,
        interval_ms,
        max_ticks,
        want_json,
    } = config;
    let exit_code = runtime.block_on(async move {
        let period = Duration::from_millis(interval_ms);
        let mut ticker = interval_at(Instant::now(), period);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut ticks = 0_u64;
        loop {
            ticker.tick().await;
            ticks = ticks.saturating_add(1);
            let env = run_reconcile_once(&claims_dir, resolve_now_unix(now_unix));
            let code = env.exit_code();
            if want_json {
                println!("{}", serde_json::to_string(&env).unwrap());
            } else if let Some(data) = env.data.as_ref() {
                eprintln!(
                    "claim.reconcile tick={ticks} scanned={} changed={}",
                    data.scanned, data.changed
                );
            } else if let Some(error) = env.error.as_ref() {
                eprintln!("claim.reconcile tick={ticks} failed: {}", error.message);
            }
            if code != 0 {
                return code;
            }
            if max_ticks.is_some_and(|limit| ticks >= limit) {
                return 0;
            }
        }
    });
    std::process::exit(exit_code);
}

fn run_claim_check_write(args: &[String]) {
    use forge_core_cli::claim::run_check_write;
    use forge_core_contracts::StableId;
    let mut claims_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut agent_id = String::new();
    let mut targets: Vec<String> = Vec::new();
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
                agent_id = require_value(args, idx, "agent");
            }
            "--target" => {
                idx += 1;
                targets.push(require_value(args, idx, "target"));
            }
            "--claims-dir" => {
                idx += 1;
                claims_dir = Some(PathBuf::from(require_value(args, idx, "claims-dir")));
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
                println!("forge-core claim check-write [--root <path>] [--allow-bootstrap-core] --agent <id> --target <path> [--target <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --claims-dir, resolves --root and uses <state_root>/claims-active; --claims-dir preserves the explicit override.");
                return;
            }
            _ => {}
        }
        idx += 1;
    }
    if agent_id.is_empty() {
        eprintln!("claim check-write: --agent <id> is required");
        std::process::exit(3);
    }
    if targets.is_empty() {
        eprintln!("claim check-write: at least one --target <path> is required");
        std::process::exit(3);
    }
    let claims_dir = resolve_claims_dir_or_exit(
        "check-write",
        claims_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    );
    let env = run_check_write(
        &claims_dir,
        &StableId(agent_id),
        &targets,
        resolve_now_unix(now_unix),
    );
    emit_envelope("claim", env, want_json);
}

/// Emit a CliEnvelope to stdout (JSON) / stderr (text) and exit with its code.
/// Used by both guide/* and claim/* (DD17: shared envelope contract).
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
