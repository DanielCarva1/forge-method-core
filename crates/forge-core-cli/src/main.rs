use forge_core_cli::{
    run_execute_operation, run_host_adapter_artifact_verification,
    run_host_adapter_certificate_crl_status_verification,
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
use forge_core_store::{EffectMetadataAdapterTrigger, EffectMetadataConsumerUse};
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("validate");
    match command {
        "validate" => run_validate_command(&args),
        "execute-operation" => run_execute_operation_command(&args),
        "rebuild-effect-index" => run_rebuild_effect_index_command(&args),
        "query-effect-index" => run_query_effect_index_command(&args),
        "host-adapter-manifest" => run_host_adapter_manifest_command(&args),
        "host-adapter-projection" => run_host_adapter_projection_command(&args),
        "host-adapter-process-policy" => run_host_adapter_process_policy_command(&args),
        "host-adapter-admit-invocation" => run_host_adapter_admit_invocation_command(&args),
        "host-adapter-distribution-policy" => run_host_adapter_distribution_policy_command(&args),
        "host-adapter-admit-distribution" => run_host_adapter_admit_distribution_command(&args),
        "host-adapter-verify-artifact" => run_host_adapter_verify_artifact_command(&args),
        "host-adapter-verify-provenance" => run_host_adapter_verify_provenance_command(&args),
        "host-adapter-verify-rekor-entry" => run_host_adapter_verify_rekor_entry_command(&args),
        "host-adapter-verify-sigstore-trust-policy" => {
            run_host_adapter_verify_sigstore_trust_policy_command(&args)
        }
        "host-adapter-verify-fulcio-certificate-identity" => {
            run_host_adapter_verify_fulcio_certificate_identity_command(&args)
        }
        "host-adapter-verify-sigstore-bundle-subject" => {
            run_host_adapter_verify_sigstore_bundle_subject_command(&args)
        }
        "host-adapter-verify-sigstore-dsse-in-toto-subject" => {
            run_host_adapter_verify_sigstore_dsse_in_toto_subject_command(&args)
        }
        "host-adapter-verify-sigstore-timestamp-authority" => {
            run_host_adapter_verify_sigstore_timestamp_authority_command(&args)
        }
        "host-adapter-verify-certificate-transparency-sct" => {
            run_host_adapter_verify_certificate_transparency_sct_command(&args)
        }
        "host-adapter-verify-certificate-revocation-policy" => {
            run_host_adapter_verify_certificate_revocation_policy_command(&args)
        }
        "host-adapter-verify-tuf-trusted-root-freshness" => {
            run_host_adapter_verify_tuf_trusted_root_freshness_command(&args)
        }
        "host-adapter-verify-certificate-crl-status" => {
            run_host_adapter_verify_certificate_crl_status_command(&args)
        }
        "--help" | "-h" => println!("{}", usage()),
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

fn run_host_adapter_distribution_policy_command(args: &[String]) {
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
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
}

fn run_host_adapter_admit_distribution_command(args: &[String]) {
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
                target = parse_runtime_kind(next_arg(args, index));
            }
            "--channel" => {
                index += 1;
                channel = parse_update_channel(next_arg(args, index));
            }
            "--artifact" => {
                index += 1;
                artifact_name = Some(next_arg(args, index).to_string());
            }
            "--sha256" => {
                index += 1;
                artifact_sha256 = Some(next_arg(args, index).to_string());
            }
            "--signature-ref" => {
                index += 1;
                signature_ref = Some(next_arg(args, index).to_string());
            }
            "--provenance-ref" => {
                index += 1;
                provenance_ref = Some(next_arg(args, index).to_string());
            }
            "--source-ref" => {
                index += 1;
                source_ref = Some(next_arg(args, index).to_string());
            }
            "--version" => {
                index += 1;
                version = Some(next_arg(args, index).to_string());
            }
            "--compatible-core-version" => {
                index += 1;
                compatible_core_version = Some(next_arg(args, index).to_string());
            }
            "--rollback-ref" => {
                index += 1;
                rollback_ref = Some(next_arg(args, index).to_string());
            }
            "--update-summary-ref" => {
                index += 1;
                update_summary_ref = Some(next_arg(args, index).to_string());
            }
            "--explicit-canary-opt-in" => explicit_canary_opt_in = true,
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let Some(artifact_name) = artifact_name else {
        eprintln!("{}", usage());
        std::process::exit(2);
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
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_artifact_command(args: &[String]) {
    let mut artifact_path: Option<PathBuf> = None;
    let mut expected_sha256: Option<String> = None;
    let mut signature_ref = None;
    let mut provenance_ref = None;
    let mut source_ref = None;
    let mut version = None;
    let mut compatible_core_version = None;
    let mut rollback_ref = None;
    let mut update_summary_ref = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--artifact-path" => {
                index += 1;
                artifact_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--sha256" => {
                index += 1;
                expected_sha256 = Some(next_arg(args, index).to_string());
            }
            "--signature-ref" => {
                index += 1;
                signature_ref = Some(next_arg(args, index).to_string());
            }
            "--provenance-ref" => {
                index += 1;
                provenance_ref = Some(next_arg(args, index).to_string());
            }
            "--source-ref" => {
                index += 1;
                source_ref = Some(next_arg(args, index).to_string());
            }
            "--version" => {
                index += 1;
                version = Some(next_arg(args, index).to_string());
            }
            "--compatible-core-version" => {
                index += 1;
                compatible_core_version = Some(next_arg(args, index).to_string());
            }
            "--rollback-ref" => {
                index += 1;
                rollback_ref = Some(next_arg(args, index).to_string());
            }
            "--update-summary-ref" => {
                index += 1;
                update_summary_ref = Some(next_arg(args, index).to_string());
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (Some(artifact_path), Some(expected_sha256)) = (artifact_path, expected_sha256) else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };
    let verification =
        run_host_adapter_artifact_verification(HostAdapterArtifactVerificationInput {
            artifact_path,
            expected_sha256,
            signature_ref,
            provenance_ref,
            source_ref,
            version,
            compatible_core_version,
            rollback_ref,
            update_summary_ref,
        });
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize host adapter artifact verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_artifact_verification artifact={} status={:?} reasons={:?}",
            verification.artifact_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterArtifactVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_provenance_command(args: &[String]) {
    let mut artifact_path: Option<PathBuf> = None;
    let mut provenance_path: Option<PathBuf> = None;
    let mut signature_path: Option<PathBuf> = None;
    let mut public_key_path: Option<PathBuf> = None;
    let mut transparency_log_path: Option<PathBuf> = None;
    let mut expected_sha256: Option<String> = None;
    let mut expected_builder_id: Option<String> = None;
    let mut expected_source_uri: Option<String> = None;
    let mut expected_source_ref: Option<String> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--artifact-path" => {
                index += 1;
                artifact_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--provenance-path" => {
                index += 1;
                provenance_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--signature-path" => {
                index += 1;
                signature_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--public-key-path" => {
                index += 1;
                public_key_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--transparency-log-path" => {
                index += 1;
                transparency_log_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--sha256" => {
                index += 1;
                expected_sha256 = Some(next_arg(args, index).to_string());
            }
            "--expected-builder-id" => {
                index += 1;
                expected_builder_id = Some(next_arg(args, index).to_string());
            }
            "--expected-source-uri" => {
                index += 1;
                expected_source_uri = Some(next_arg(args, index).to_string());
            }
            "--expected-source-ref" => {
                index += 1;
                expected_source_ref = Some(next_arg(args, index).to_string());
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (
        Some(artifact_path),
        Some(provenance_path),
        Some(signature_path),
        Some(public_key_path),
        Some(transparency_log_path),
        Some(expected_sha256),
        Some(expected_builder_id),
        Some(expected_source_uri),
        Some(expected_source_ref),
    ) = (
        artifact_path,
        provenance_path,
        signature_path,
        public_key_path,
        transparency_log_path,
        expected_sha256,
        expected_builder_id,
        expected_source_uri,
        expected_source_ref,
    )
    else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification =
        run_host_adapter_provenance_verification(HostAdapterProvenanceVerificationInput {
            artifact_path,
            provenance_path,
            signature_path,
            public_key_path,
            transparency_log_path,
            expected_sha256,
            expected_builder_id,
            expected_source_uri,
            expected_source_ref,
        });
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize host adapter provenance verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_provenance_verification artifact={} status={:?} reasons={:?}",
            verification.artifact_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterProvenanceVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_rekor_entry_command(args: &[String]) {
    let mut log_entry_path: Option<PathBuf> = None;
    let mut public_key_path: Option<PathBuf> = None;
    let mut expected_log_id: Option<String> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--log-entry-path" => {
                index += 1;
                log_entry_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--public-key-path" => {
                index += 1;
                public_key_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--expected-log-id" => {
                index += 1;
                expected_log_id = Some(next_arg(args, index).to_string());
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (Some(log_entry_path), Some(public_key_path), Some(expected_log_id)) =
        (log_entry_path, public_key_path, expected_log_id)
    else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_rekor_verification(HostAdapterRekorVerificationInput {
        log_entry_path,
        public_key_path,
        expected_log_id,
    });
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize host adapter Rekor verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_rekor_verification log_entry={} status={:?} reasons={:?}",
            verification.log_entry_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterRekorVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_sigstore_trust_policy_command(args: &[String]) {
    let mut policy_path: Option<PathBuf> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--policy-path" => {
                index += 1;
                policy_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let Some(policy_path) = policy_path else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_sigstore_trust_policy_verification(
        HostAdapterSigstoreTrustPolicyVerificationInput { policy_path },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize Sigstore trust policy verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_sigstore_trust_policy_verification policy={} status={:?} reasons={:?}",
            verification.policy_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterSigstoreTrustPolicyVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_fulcio_certificate_identity_command(args: &[String]) {
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut issuer_certificate_paths = Vec::new();
    let mut verification_time_unix: Option<i64> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_paths.push(PathBuf::from(next_arg(args, index)));
            }
            "--verification-time-unix" => {
                index += 1;
                verification_time_unix = Some(parse_i64(next_arg(args, index)));
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(certificate_path), Some(verification_time_unix)) =
        (trust_policy_path, certificate_path, verification_time_unix)
    else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_fulcio_certificate_identity_verification(
        HostAdapterFulcioCertificateIdentityVerificationInput {
            trust_policy_path,
            certificate_path,
            issuer_certificate_paths,
            verification_time_unix,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize Fulcio certificate identity verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_fulcio_certificate_identity_verification certificate={} status={:?} reasons={:?}",
            verification.certificate_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterFulcioCertificateIdentityVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_sigstore_bundle_subject_command(args: &[String]) {
    let mut bundle_path: Option<PathBuf> = None;
    let mut artifact_path: Option<PathBuf> = None;
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut issuer_certificate_paths = Vec::new();
    let mut rekor_log_entry_path: Option<PathBuf> = None;
    let mut rekor_public_key_path: Option<PathBuf> = None;
    let mut expected_rekor_log_id: Option<String> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--bundle-path" => {
                index += 1;
                bundle_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--artifact-path" => {
                index += 1;
                artifact_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_paths.push(PathBuf::from(next_arg(args, index)));
            }
            "--rekor-log-entry-path" => {
                index += 1;
                rekor_log_entry_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--rekor-public-key-path" => {
                index += 1;
                rekor_public_key_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--expected-rekor-log-id" => {
                index += 1;
                expected_rekor_log_id = Some(next_arg(args, index).to_string());
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (
        Some(bundle_path),
        Some(artifact_path),
        Some(trust_policy_path),
        Some(certificate_path),
        Some(rekor_log_entry_path),
        Some(rekor_public_key_path),
        Some(expected_rekor_log_id),
    ) = (
        bundle_path,
        artifact_path,
        trust_policy_path,
        certificate_path,
        rekor_log_entry_path,
        rekor_public_key_path,
        expected_rekor_log_id,
    )
    else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_sigstore_bundle_subject_verification(
        HostAdapterSigstoreBundleSubjectVerificationInput {
            bundle_path,
            artifact_path,
            trust_policy_path,
            certificate_path,
            issuer_certificate_paths,
            rekor_log_entry_path,
            rekor_public_key_path,
            expected_rekor_log_id,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize Sigstore bundle subject verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_sigstore_bundle_subject_verification bundle={} status={:?} reasons={:?}",
            verification.bundle_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterSigstoreBundleSubjectVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_sigstore_dsse_in_toto_subject_command(args: &[String]) {
    let mut bundle_path: Option<PathBuf> = None;
    let mut artifact_path: Option<PathBuf> = None;
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut issuer_certificate_paths = Vec::new();
    let mut rekor_log_entry_path: Option<PathBuf> = None;
    let mut rekor_public_key_path: Option<PathBuf> = None;
    let mut expected_rekor_log_id: Option<String> = None;
    let mut expected_predicate_type: Option<String> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--bundle-path" => {
                index += 1;
                bundle_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--artifact-path" => {
                index += 1;
                artifact_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_paths.push(PathBuf::from(next_arg(args, index)));
            }
            "--rekor-log-entry-path" => {
                index += 1;
                rekor_log_entry_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--rekor-public-key-path" => {
                index += 1;
                rekor_public_key_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--expected-rekor-log-id" => {
                index += 1;
                expected_rekor_log_id = Some(next_arg(args, index).to_string());
            }
            "--expected-predicate-type" => {
                index += 1;
                expected_predicate_type = Some(next_arg(args, index).to_string());
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (
        Some(bundle_path),
        Some(artifact_path),
        Some(trust_policy_path),
        Some(certificate_path),
        Some(rekor_log_entry_path),
        Some(rekor_public_key_path),
        Some(expected_rekor_log_id),
    ) = (
        bundle_path,
        artifact_path,
        trust_policy_path,
        certificate_path,
        rekor_log_entry_path,
        rekor_public_key_path,
        expected_rekor_log_id,
    )
    else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_sigstore_dsse_in_toto_subject_verification(
        HostAdapterSigstoreDsseInTotoSubjectVerificationInput {
            bundle_path,
            artifact_path,
            trust_policy_path,
            certificate_path,
            issuer_certificate_paths,
            rekor_log_entry_path,
            rekor_public_key_path,
            expected_rekor_log_id,
            expected_predicate_type,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize Sigstore DSSE in-toto subject verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_sigstore_dsse_in_toto_subject_verification bundle={} status={:?} reasons={:?}",
            verification.bundle_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterSigstoreDsseInTotoSubjectVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_sigstore_timestamp_authority_command(args: &[String]) {
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut rekor_log_entry_path: Option<PathBuf> = None;
    let mut rekor_public_key_path: Option<PathBuf> = None;
    let mut expected_rekor_log_id: Option<String> = None;
    let mut rfc3161_timestamp_token_path: Option<PathBuf> = None;
    let mut rfc3161_timestamped_signature_path: Option<PathBuf> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--rekor-log-entry-path" => {
                index += 1;
                rekor_log_entry_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--rekor-public-key-path" => {
                index += 1;
                rekor_public_key_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--expected-rekor-log-id" => {
                index += 1;
                expected_rekor_log_id = Some(next_arg(args, index).to_string());
            }
            "--rfc3161-timestamp-token-path" => {
                index += 1;
                rfc3161_timestamp_token_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--rfc3161-timestamped-signature-path" => {
                index += 1;
                rfc3161_timestamped_signature_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(certificate_path)) = (trust_policy_path, certificate_path)
    else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_sigstore_timestamp_authority_verification(
        HostAdapterSigstoreTimestampAuthorityVerificationInput {
            trust_policy_path,
            certificate_path,
            rekor_log_entry_path,
            rekor_public_key_path,
            expected_rekor_log_id,
            rfc3161_timestamp_token_path,
            rfc3161_timestamped_signature_path,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize Sigstore timestamp authority verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_sigstore_timestamp_authority_verification certificate={} status={:?} reasons={:?}",
            verification.certificate_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterSigstoreTimestampAuthorityVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_certificate_transparency_sct_command(args: &[String]) {
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut sct_paths: Vec<PathBuf> = Vec::new();
    let mut verification_time_unix_ms: Option<u64> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--sct-path" => {
                index += 1;
                sct_paths.push(PathBuf::from(next_arg(args, index)));
            }
            "--verification-time-unix-ms" => {
                index += 1;
                verification_time_unix_ms = next_arg(args, index).parse::<u64>().ok();
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(certificate_path), Some(verification_time_unix_ms)) = (
        trust_policy_path,
        certificate_path,
        verification_time_unix_ms,
    ) else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_certificate_transparency_sct_verification(
        HostAdapterCertificateTransparencySctVerificationInput {
            trust_policy_path,
            certificate_path,
            sct_paths,
            verification_time_unix_ms,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize Certificate Transparency SCT verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_certificate_transparency_sct_verification certificate={} status={:?} reasons={:?}",
            verification.certificate_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterCertificateTransparencySctVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_certificate_revocation_policy_command(args: &[String]) {
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut trusted_signing_time_unix: Option<i64> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--trusted-signing-time-unix" => {
                index += 1;
                trusted_signing_time_unix = Some(parse_i64(next_arg(args, index)));
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(certificate_path), Some(trusted_signing_time_unix)) = (
        trust_policy_path,
        certificate_path,
        trusted_signing_time_unix,
    ) else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_certificate_revocation_policy_verification(
        HostAdapterCertificateRevocationPolicyVerificationInput {
            trust_policy_path,
            certificate_path,
            trusted_signing_time_unix,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize certificate revocation policy verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_certificate_revocation_policy_verification certificate={} status={:?} reasons={:?}",
            verification.certificate_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterCertificateRevocationPolicyVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_tuf_trusted_root_freshness_command(args: &[String]) {
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut root_metadata_path: Option<PathBuf> = None;
    let mut timestamp_metadata_path: Option<PathBuf> = None;
    let mut snapshot_metadata_path: Option<PathBuf> = None;
    let mut targets_metadata_path: Option<PathBuf> = None;
    let mut update_start_time_unix: Option<i64> = None;
    let mut min_root_version: Option<i64> = None;
    let mut min_timestamp_version: Option<i64> = None;
    let mut min_snapshot_version: Option<i64> = None;
    let mut min_targets_version: Option<i64> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--root-metadata-path" => {
                index += 1;
                root_metadata_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--timestamp-metadata-path" => {
                index += 1;
                timestamp_metadata_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--snapshot-metadata-path" => {
                index += 1;
                snapshot_metadata_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--targets-metadata-path" => {
                index += 1;
                targets_metadata_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--update-start-time-unix" => {
                index += 1;
                update_start_time_unix = Some(parse_i64(next_arg(args, index)));
            }
            "--min-root-version" => {
                index += 1;
                min_root_version = Some(parse_i64(next_arg(args, index)));
            }
            "--min-timestamp-version" => {
                index += 1;
                min_timestamp_version = Some(parse_i64(next_arg(args, index)));
            }
            "--min-snapshot-version" => {
                index += 1;
                min_snapshot_version = Some(parse_i64(next_arg(args, index)));
            }
            "--min-targets-version" => {
                index += 1;
                min_targets_version = Some(parse_i64(next_arg(args, index)));
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(root_metadata_path), Some(update_start_time_unix)) = (
        trust_policy_path,
        root_metadata_path,
        update_start_time_unix,
    ) else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_tuf_trusted_root_freshness_verification(
        HostAdapterTufTrustedRootFreshnessVerificationInput {
            trust_policy_path,
            root_metadata_path,
            timestamp_metadata_path,
            snapshot_metadata_path,
            targets_metadata_path,
            update_start_time_unix,
            min_root_version,
            min_timestamp_version,
            min_snapshot_version,
            min_targets_version,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize TUF trusted-root freshness verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_tuf_trusted_root_freshness_verification root={} status={:?} reasons={:?}",
            verification.root_metadata_path, verification.status, verification.reasons
        );
    }
    if verification.status == HostAdapterTufTrustedRootFreshnessVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_verify_certificate_crl_status_command(args: &[String]) {
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut issuer_certificate_path: Option<PathBuf> = None;
    let mut crl_path: Option<PathBuf> = None;
    let mut verification_time_unix: Option<i64> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--crl-path" => {
                index += 1;
                crl_path = Some(PathBuf::from(next_arg(args, index)));
            }
            "--verification-time-unix" => {
                index += 1;
                verification_time_unix = Some(parse_i64(next_arg(args, index)));
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let (
        Some(trust_policy_path),
        Some(certificate_path),
        Some(issuer_certificate_path),
        Some(crl_path),
        Some(verification_time_unix),
    ) = (
        trust_policy_path,
        certificate_path,
        issuer_certificate_path,
        crl_path,
        verification_time_unix,
    )
    else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };

    let verification = run_host_adapter_certificate_crl_status_verification(
        HostAdapterCertificateCrlStatusVerificationInput {
            trust_policy_path,
            certificate_path,
            issuer_certificate_path,
            crl_path,
            verification_time_unix,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize certificate CRL status verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_certificate_crl_status_verification certificate={} status={:?} revocation_status={:?} reasons={:?}",
            verification.certificate_path,
            verification.status,
            verification.revocation_status,
            verification.reasons
        );
    }
    if verification.status == HostAdapterCertificateCrlStatusVerificationStatus::Failed {
        std::process::exit(1);
    }
}

fn run_host_adapter_process_policy_command(args: &[String]) {
    let mut target = HostAdapterProcessTarget::McpStdio;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                index += 1;
                target = parse_host_adapter_process_target(next_arg(args, index));
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
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
}

fn run_host_adapter_admit_invocation_command(args: &[String]) {
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
                command_name = Some(next_arg(args, index).to_string());
            }
            "--target" => {
                index += 1;
                target = parse_host_adapter_process_target(next_arg(args, index));
            }
            "--explicit" => explicit_invocation = true,
            "--argv" => {
                index += 1;
                argv.push(next_arg(args, index).to_string());
            }
            "--cwd" => {
                index += 1;
                cwd = Some(next_arg(args, index).to_string());
            }
            "--env-key" => {
                index += 1;
                env_keys.push(next_arg(args, index).to_string());
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let Some(command_name) = command_name else {
        eprintln!("{}", usage());
        std::process::exit(2);
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
        std::process::exit(1);
    }
}

fn run_host_adapter_projection_command(args: &[String]) {
    let mut target = HostAdapterProjectionTarget::McpTools;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                index += 1;
                target = parse_host_adapter_projection_target(next_arg(args, index));
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
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
}

fn run_host_adapter_manifest_command(args: &[String]) {
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
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
}

fn run_validate_command(args: &[String]) {
    let mut root = PathBuf::from(".");
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("{}", usage());
                    std::process::exit(2);
                };
                root = PathBuf::from(value);
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let summary = run_validate(&root);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary).expect("serialize validation summary")
        );
    } else {
        println!("{}", summary.human_summary());
        for diagnostic in &summary.diagnostics {
            eprintln!(
                "{} {} {}: {}",
                diagnostic.severity, diagnostic.code, diagnostic.path, diagnostic.message
            );
        }
    }

    if !summary.passed() {
        std::process::exit(1);
    }
}

fn run_execute_operation_command(args: &[String]) {
    let mut root = PathBuf::from(".");
    let mut operation_path: Option<PathBuf> = None;
    let mut command_paths = Vec::new();
    let mut effect_paths = Vec::new();
    let mut payloads = Vec::new();
    let mut payload_policy = PayloadLoadPolicy::default();
    let mut recorded_at = "unknown".to_string();
    let mut tx_id_prefix = "cli-execute-operation".to_string();
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = next_path(args, index);
            }
            "--operation" => {
                index += 1;
                operation_path = Some(next_path(args, index));
            }
            "--command" => {
                index += 1;
                command_paths.push(next_path(args, index));
            }
            "--effect" => {
                index += 1;
                effect_paths.push(next_path(args, index));
            }
            "--payload" => {
                index += 1;
                payloads.push(parse_payload_arg(next_arg(args, index)));
            }
            "--max-payload-bytes" => {
                index += 1;
                payload_policy.max_payload_bytes = parse_u64(next_arg(args, index));
            }
            "--allow-payload-outside-root" => {
                payload_policy.allow_outside_root = true;
            }
            "--recorded-at" => {
                index += 1;
                recorded_at = next_arg(args, index).to_string();
            }
            "--tx-id-prefix" => {
                index += 1;
                tx_id_prefix = next_arg(args, index).to_string();
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let Some(operation_path) = operation_path else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };
    let input = ExecuteOperationInput {
        root,
        operation_path,
        command_paths,
        effect_paths,
        payloads,
        payload_policy,
        recorded_at,
        tx_id_prefix,
    };
    let execution = match run_execute_operation(input) {
        Ok(execution) => execution,
        Err(error) => {
            eprintln!("execute-operation failed: {error}");
            std::process::exit(1);
        }
    };
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&execution).expect("serialize execution")
        );
    } else {
        println!(
            "forge_core_operation_execution status={:?} reasons={:?}",
            execution.status, execution.reasons
        );
    }
    if execution.status != forge_core_runtime::RuntimeOperationExecutionStatus::Completed {
        std::process::exit(1);
    }
}

fn run_rebuild_effect_index_command(args: &[String]) {
    let mut input = RebuildEffectIndexInput::default();
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                input.root = next_path(args, index);
            }
            "--wal" => {
                index += 1;
                input.wal_relative_path = next_arg(args, index).to_string();
            }
            "--index" => {
                index += 1;
                input.index_relative_path = next_arg(args, index).to_string();
            }
            "--lock" => {
                index += 1;
                input.lock_relative_path = next_arg(args, index).to_string();
            }
            "--recorded-at" => {
                index += 1;
                input.recorded_at = Some(next_arg(args, index).to_string());
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let result = run_rebuild_effect_index(input);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).expect("serialize rebuild result")
        );
    } else {
        println!(
            "forge_core_effect_index_rebuild status={:?} rebuilt={} appended={} reasons={:?}",
            result.status, result.rebuilt_records, result.appended_records, result.reasons
        );
    }
    if result.status == forge_core_store::EffectTargetMetadataIndexRebuildStatus::Failed {
        std::process::exit(1);
    }
}

fn run_query_effect_index_command(args: &[String]) {
    let mut input = QueryEffectIndexInput::default();
    let mut json = false;
    let mut context = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                input.root = next_path(args, index);
            }
            "--index" => {
                index += 1;
                input.index_relative_path = next_arg(args, index).to_string();
            }
            "--logical-ref" => {
                index += 1;
                input.logical_ref = Some(next_arg(args, index).to_string());
            }
            "--effect-id" => {
                index += 1;
                input.effect_id = Some(next_arg(args, index).to_string());
            }
            "--operation-id" => {
                index += 1;
                input.operation_id = Some(next_arg(args, index).to_string());
            }
            "--target-kind" => {
                index += 1;
                input.target_kind = Some(parse_target_kind(next_arg(args, index)));
            }
            "--consumer-use" => {
                index += 1;
                input.consumer_use = parse_metadata_consumer_use(next_arg(args, index));
            }
            "--context" => context = true,
            "--max-context-groups" => {
                index += 1;
                input.context_options.max_groups = parse_usize(next_arg(args, index));
            }
            "--adapter-kind" => {
                index += 1;
                input.context_options.adapter_kind =
                    Some(parse_runtime_kind(next_arg(args, index)));
            }
            "--adapter-trigger" => {
                index += 1;
                input.context_options.adapter_trigger =
                    parse_metadata_adapter_trigger(next_arg(args, index));
            }
            "--latest" => input.latest_per_target = true,
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    if context {
        let result = run_query_effect_index_context(input);
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("serialize context result")
            );
        } else {
            println!(
                "forge_core_effect_index_context status={:?} total_groups={} returned_groups={} omitted_groups={} reasons={:?}",
                result.status, result.total_groups, result.returned_groups, result.omitted_groups, result.reasons
            );
        }
        if result.source_status == forge_core_store::EffectTargetMetadataIndexQueryStatus::Failed {
            std::process::exit(1);
        }
        return;
    }

    let result = run_query_effect_index(input);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).expect("serialize query result")
        );
    } else {
        println!(
            "forge_core_effect_index_query status={:?} scanned={} matched={} returned={} reasons={:?}",
            result.status,
            result.scanned_records,
            result.matched_records,
            result.returned_records,
            result.reasons
        );
    }
    if result.status == forge_core_store::EffectTargetMetadataIndexQueryStatus::Failed {
        std::process::exit(1);
    }
}

fn next_arg(args: &[String], index: usize) -> &str {
    args.get(index).map(String::as_str).unwrap_or_else(|| {
        eprintln!("{}", usage());
        std::process::exit(2);
    })
}

fn next_path(args: &[String], index: usize) -> PathBuf {
    PathBuf::from(next_arg(args, index))
}

fn parse_payload_arg(value: &str) -> PayloadFileSpec {
    let Some((target_ref, path)) = value.split_once('=') else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };
    PayloadFileSpec {
        target_ref: target_ref.to_string(),
        path: PathBuf::from(path),
    }
}

fn parse_u64(value: &str) -> u64 {
    value.parse::<u64>().unwrap_or_else(|_| {
        eprintln!("{}", usage());
        std::process::exit(2);
    })
}

fn parse_i64(value: &str) -> i64 {
    value.parse::<i64>().unwrap_or_else(|_| {
        eprintln!("{}", usage());
        std::process::exit(2);
    })
}

fn parse_usize(value: &str) -> usize {
    value.parse::<usize>().unwrap_or_else(|_| {
        eprintln!("{}", usage());
        std::process::exit(2);
    })
}

fn parse_target_kind(value: &str) -> EffectTargetKind {
    match value {
        "file_path" => EffectTargetKind::FilePath,
        "glob" => EffectTargetKind::Glob,
        "state_key" => EffectTargetKind::StateKey,
        "artifact_id" => EffectTargetKind::ArtifactId,
        "evidence_id" => EffectTargetKind::EvidenceId,
        "ledger_stream" => EffectTargetKind::LedgerStream,
        "request_stream" => EffectTargetKind::RequestStream,
        "completion_id" => EffectTargetKind::CompletionId,
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

fn parse_runtime_kind(value: &str) -> RuntimeKind {
    match value {
        "codex" => RuntimeKind::Codex,
        "cursor" => RuntimeKind::Cursor,
        "claude" => RuntimeKind::Claude,
        "opencode" => RuntimeKind::Opencode,
        "vscode" => RuntimeKind::Vscode,
        "pidev" => RuntimeKind::Pidev,
        "forge_standalone" => RuntimeKind::ForgeStandalone,
        "custom" => RuntimeKind::Custom,
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

fn parse_metadata_consumer_use(value: &str) -> EffectMetadataConsumerUse {
    match value {
        "discovery" => EffectMetadataConsumerUse::Discovery,
        "diagnostics" => EffectMetadataConsumerUse::Diagnostics,
        "handoff_context" => EffectMetadataConsumerUse::HandoffContext,
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

fn parse_metadata_adapter_trigger(value: &str) -> EffectMetadataAdapterTrigger {
    match value {
        "evidence_discovery" => EffectMetadataAdapterTrigger::EvidenceDiscovery,
        "diagnostics" => EffectMetadataAdapterTrigger::Diagnostics,
        "handoff_preparation" => EffectMetadataAdapterTrigger::HandoffPreparation,
        "manual_inspection" => EffectMetadataAdapterTrigger::ManualInspection,
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

fn parse_host_adapter_projection_target(value: &str) -> HostAdapterProjectionTarget {
    match value {
        "mcp_tools" => HostAdapterProjectionTarget::McpTools,
        "borrowed_shell" => HostAdapterProjectionTarget::BorrowedShell,
        "app_ui" => HostAdapterProjectionTarget::AppUi,
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

fn parse_host_adapter_process_target(value: &str) -> HostAdapterProcessTarget {
    match value {
        "mcp_stdio" => HostAdapterProcessTarget::McpStdio,
        "borrowed_shell" => HostAdapterProcessTarget::BorrowedShell,
        "app_bridge" => HostAdapterProcessTarget::AppBridge,
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

fn parse_update_channel(value: &str) -> HostAdapterUpdateChannel {
    match value {
        "stable" => HostAdapterUpdateChannel::Stable,
        "canary" => HostAdapterUpdateChannel::Canary,
        "dev" => HostAdapterUpdateChannel::Dev,
        _ => {
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

fn usage() -> &'static str {
    concat!(
        "usage: forge-core validate [--root <path>] [--json]\n",
        "       forge-core execute-operation --root <path> --operation <path> [--command <path>] [--effect <path>] [--payload <target_ref>=<path>] [--max-payload-bytes <bytes>] [--allow-payload-outside-root] [--recorded-at <value>] [--tx-id-prefix <value>] [--json]\n",
        "       forge-core rebuild-effect-index [--root <path>] [--wal <path>] [--index <path>] [--lock <path>] [--recorded-at <value>] [--json]\n",
        "       forge-core query-effect-index [--root <path>] [--index <path>] [--logical-ref <ref>] [--effect-id <id>] [--operation-id <id>] [--target-kind <kind>] [--consumer-use <discovery|diagnostics|handoff_context>] [--context] [--max-context-groups <n>] [--adapter-kind <codex|cursor|claude|opencode|vscode|pidev|forge_standalone|custom>] [--adapter-trigger <evidence_discovery|diagnostics|handoff_preparation|manual_inspection>] [--latest] [--json]\n",
        "       forge-core host-adapter-manifest [--json]\n",
        "       forge-core host-adapter-projection [--target <mcp_tools|borrowed_shell|app_ui>] [--json]\n",
        "       forge-core host-adapter-process-policy [--target <mcp_stdio|borrowed_shell|app_bridge>] [--json]\n",
        "       forge-core host-adapter-admit-invocation --command <name> [--target <mcp_stdio|borrowed_shell|app_bridge>] [--explicit] [--argv <arg>] [--cwd <path>] [--env-key <key>] [--json]\n",
        "       forge-core host-adapter-distribution-policy [--json]\n",
        "       forge-core host-adapter-admit-distribution --artifact <name> [--target <codex|cursor|claude|opencode|vscode|pidev|forge_standalone|custom>] [--channel <stable|canary|dev>] [--sha256 <digest>] [--signature-ref <ref>] [--provenance-ref <ref>] [--source-ref <ref>] [--version <value>] [--compatible-core-version <value>] [--rollback-ref <ref>] [--update-summary-ref <ref>] [--explicit-canary-opt-in] [--json]\n",
        "       forge-core host-adapter-verify-artifact --artifact-path <path> --sha256 <digest> [--signature-ref <ref>] [--provenance-ref <ref>] [--source-ref <ref>] [--version <value>] [--compatible-core-version <value>] [--rollback-ref <ref>] [--update-summary-ref <ref>] [--json]\n",
        "       forge-core host-adapter-verify-provenance --artifact-path <path> --provenance-path <path> --signature-path <path> --public-key-path <path> --transparency-log-path <path> --sha256 <digest> --expected-builder-id <id> --expected-source-uri <uri> --expected-source-ref <ref> [--json]\n",
        "       forge-core host-adapter-verify-rekor-entry --log-entry-path <path> --public-key-path <path> --expected-log-id <id> [--json]\n",
        "       forge-core host-adapter-verify-sigstore-trust-policy --policy-path <path> [--json]\n",
        "       forge-core host-adapter-verify-fulcio-certificate-identity --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --verification-time-unix <seconds> [--json]\n",
        "       forge-core host-adapter-verify-sigstore-bundle-subject --bundle-path <path> --artifact-path <path> --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --rekor-log-entry-path <path> --rekor-public-key-path <path> --expected-rekor-log-id <id> [--json]\n",
        "       forge-core host-adapter-verify-sigstore-dsse-in-toto-subject --bundle-path <path> --artifact-path <path> --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --rekor-log-entry-path <path> --rekor-public-key-path <path> --expected-rekor-log-id <id> [--expected-predicate-type <type>] [--json]\n",
        "       forge-core host-adapter-verify-sigstore-timestamp-authority --trust-policy-path <path> --certificate-path <path> [--rekor-log-entry-path <path>] [--rekor-public-key-path <path>] [--expected-rekor-log-id <id>] [--rfc3161-timestamp-token-path <path>] [--rfc3161-timestamped-signature-path <path>] [--json]\n",
        "       forge-core host-adapter-verify-certificate-transparency-sct --trust-policy-path <path> --certificate-path <path> --sct-path <path> [--sct-path <path>] --verification-time-unix-ms <milliseconds> [--json]\n",
        "       forge-core host-adapter-verify-certificate-revocation-policy --trust-policy-path <path> --certificate-path <path> --trusted-signing-time-unix <seconds> [--json]\n",
        "       forge-core host-adapter-verify-tuf-trusted-root-freshness --trust-policy-path <path> --root-metadata-path <path> [--timestamp-metadata-path <path>] [--snapshot-metadata-path <path>] [--targets-metadata-path <path>] --update-start-time-unix <seconds> [--min-root-version <n>] [--min-timestamp-version <n>] [--min-snapshot-version <n>] [--min-targets-version <n>] [--json]",
        "\n       forge-core host-adapter-verify-certificate-crl-status --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> --crl-path <path> --verification-time-unix <seconds> [--json]",
    )
}
