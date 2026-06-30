use forge_core_cli::cli_util::*;
use forge_core_cli::host_adapter_policy_cmd;
use forge_core_cli::host_adapter_verify_cmd;
use forge_core_cli::m1_cmd::M1CommandKind;
use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("validate");
    match command {
        "guide" => match forge_core_cli::guide::run_guide_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "claim" => match forge_core_cli::claim::run_claim_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "autonomy" => match forge_core_cli::autonomy_cmd::run_autonomy_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "contract" => match forge_core_cli::contract_cmd::run_contract_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "isolation" => match forge_core_cli::isolation::run_isolation_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "coordination" => match forge_core_cli::coordination::run_coordination_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "project" => match forge_core_cli::project_cmd::run_project_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "graph" => match forge_core_cli::graph_cmd::run_graph_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "eval" => match forge_core_cli::eval_cmd::run_eval_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "telemetry" => match forge_core_cli::telemetry_cmd::run_telemetry_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "preview" => match forge_core_cli::m1_cmd::run_m1_command(&args, M1CommandKind::Preview) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "ready" => match forge_core_cli::m1_cmd::run_m1_command(&args, M1CommandKind::Ready) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "explain" => match forge_core_cli::m1_cmd::run_m1_command(&args, M1CommandKind::Explain) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "validate" => match forge_core_cli::validate::run_validate_command(&args) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
        },
        "execute-operation" => {
            match forge_core_cli::execute_operation::run_execute_operation_command(&args)
            {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "rebuild-effect-index" => {
            match forge_core_cli::effect_index::run_rebuild_effect_index_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "query-effect-index" => {
            match forge_core_cli::effect_index::run_query_effect_index_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-manifest" => {
            match host_adapter_policy_cmd::run_host_adapter_manifest_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-projection" => {
            match host_adapter_policy_cmd::run_host_adapter_projection_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-process-policy" => {
            match host_adapter_policy_cmd::run_host_adapter_process_policy_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-admit-invocation" => {
            match host_adapter_policy_cmd::run_host_adapter_admit_invocation_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-distribution-policy" => {
            match host_adapter_policy_cmd::run_host_adapter_distribution_policy_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-admit-distribution" => {
            match host_adapter_policy_cmd::run_host_adapter_admit_distribution_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-artifact" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_artifact_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-provenance" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_provenance_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-rekor-entry" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_rekor_entry_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-sigstore-trust-policy" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_sigstore_trust_policy_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-fulcio-certificate-identity" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_fulcio_certificate_identity_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-sigstore-bundle-subject" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_sigstore_bundle_subject_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-sigstore-dsse-in-toto-subject" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_sigstore_dsse_in_toto_subject_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-sigstore-timestamp-authority" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_sigstore_timestamp_authority_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-certificate-transparency-sct" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_certificate_transparency_sct_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-certificate-revocation-policy" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_certificate_revocation_policy_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-tuf-trusted-root-freshness" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_tuf_trusted_root_freshness_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-certificate-crl-status" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_certificate_crl_status_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "host-adapter-verify-certificate-ocsp-status" => {
            match host_adapter_verify_cmd::run_host_adapter_verify_certificate_ocsp_status_command(&args) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            }
        }
        "--help" | "-h" => println!("{}", usage()),
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}
