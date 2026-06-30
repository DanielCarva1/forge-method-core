//! Host-adapter verification CLI dispatchers.
//!
//! Each `run_host_adapter_verify_*_command` parses argv into a typed
//! `HostAdapter*VerificationInput`, calls the corresponding verification
//! entrypoint re-exported from `forge_core_crypto`, prints the result as
//! JSON or human-readable text, and exits non-zero on verification failure.
//!
//! Extracted from the legacy god-file `main.rs` as part of R11.2
//! (see `docs/dev-docs/forge-method-core-dev-docs-v2/09_system_design_roadmap.md`).

use crate::cli_error::ExitError;
use crate::cli_util::{next_arg_or_err, parse_i64_or_err, usage};
use crate::{
    run_host_adapter_artifact_verification, run_host_adapter_certificate_crl_status_verification,
    run_host_adapter_certificate_ocsp_status_verification,
    run_host_adapter_certificate_revocation_policy_verification,
    run_host_adapter_certificate_transparency_sct_verification,
    run_host_adapter_fulcio_certificate_identity_verification,
    run_host_adapter_provenance_verification, run_host_adapter_rekor_verification,
    run_host_adapter_sigstore_bundle_subject_verification,
    run_host_adapter_sigstore_dsse_in_toto_subject_verification,
    run_host_adapter_sigstore_timestamp_authority_verification,
    run_host_adapter_sigstore_trust_policy_verification,
    run_host_adapter_tuf_trusted_root_freshness_verification, HostAdapterArtifactVerificationInput,
    HostAdapterArtifactVerificationStatus, HostAdapterCertificateCrlStatusVerificationInput,
    HostAdapterCertificateCrlStatusVerificationStatus,
    HostAdapterCertificateOcspStatusVerificationInput,
    HostAdapterCertificateOcspStatusVerificationStatus,
    HostAdapterCertificateRevocationPolicyVerificationInput,
    HostAdapterCertificateRevocationPolicyVerificationStatus,
    HostAdapterCertificateTransparencySctVerificationInput,
    HostAdapterCertificateTransparencySctVerificationStatus,
    HostAdapterFulcioCertificateIdentityVerificationInput,
    HostAdapterFulcioCertificateIdentityVerificationStatus, HostAdapterProvenanceVerificationInput,
    HostAdapterProvenanceVerificationStatus, HostAdapterRekorVerificationInput,
    HostAdapterRekorVerificationStatus, HostAdapterSigstoreBundleSubjectVerificationInput,
    HostAdapterSigstoreBundleSubjectVerificationStatus,
    HostAdapterSigstoreDsseInTotoSubjectVerificationInput,
    HostAdapterSigstoreDsseInTotoSubjectVerificationStatus,
    HostAdapterSigstoreTimestampAuthorityVerificationInput,
    HostAdapterSigstoreTimestampAuthorityVerificationStatus,
    HostAdapterSigstoreTrustPolicyVerificationInput,
    HostAdapterSigstoreTrustPolicyVerificationStatus,
    HostAdapterTufTrustedRootFreshnessVerificationInput,
    HostAdapterTufTrustedRootFreshnessVerificationStatus,
};
use std::path::PathBuf;

/// Runs the `host-adapter-verify-artifact` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the artifact verification reports a
/// non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_artifact_command(args: &[String]) -> Result<(), ExitError> {
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
                artifact_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--sha256" => {
                index += 1;
                expected_sha256 = Some(next_arg_or_err(args, index)?.to_string());
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

    let (Some(artifact_path), Some(expected_sha256)) = (artifact_path, expected_sha256) else {
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-provenance` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the provenance verification reports a
/// non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_provenance_command(args: &[String]) -> Result<(), ExitError> {
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
                artifact_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--provenance-path" => {
                index += 1;
                provenance_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--signature-path" => {
                index += 1;
                signature_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--public-key-path" => {
                index += 1;
                public_key_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--transparency-log-path" => {
                index += 1;
                transparency_log_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--sha256" => {
                index += 1;
                expected_sha256 = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--expected-builder-id" => {
                index += 1;
                expected_builder_id = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--expected-source-uri" => {
                index += 1;
                expected_source_uri = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--expected-source-ref" => {
                index += 1;
                expected_source_ref = Some(next_arg_or_err(args, index)?.to_string());
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
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-rekor-entry` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the Rekor entry verification reports a
/// non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_rekor_entry_command(args: &[String]) -> Result<(), ExitError> {
    let mut log_entry_path: Option<PathBuf> = None;
    let mut public_key_path: Option<PathBuf> = None;
    let mut expected_log_id: Option<String> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--log-entry-path" => {
                index += 1;
                log_entry_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--public-key-path" => {
                index += 1;
                public_key_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--expected-log-id" => {
                index += 1;
                expected_log_id = Some(next_arg_or_err(args, index)?.to_string());
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

    let (Some(log_entry_path), Some(public_key_path), Some(expected_log_id)) =
        (log_entry_path, public_key_path, expected_log_id)
    else {
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-sigstore-trust-policy` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the Sigstore trust policy verification reports
/// a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_sigstore_trust_policy_command(
    args: &[String],
) -> Result<(), ExitError> {
    let mut policy_path: Option<PathBuf> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--policy-path" => {
                index += 1;
                policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
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

    let Some(policy_path) = policy_path else {
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-fulcio-certificate-identity` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the Fulcio certificate identity verification
/// reports a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_fulcio_certificate_identity_command(
    args: &[String],
) -> Result<(), ExitError> {
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_paths.push(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--verification-time-unix" => {
                index += 1;
                verification_time_unix = Some(parse_i64_or_err(next_arg_or_err(args, index)?)?);
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

    let (Some(trust_policy_path), Some(certificate_path), Some(verification_time_unix)) =
        (trust_policy_path, certificate_path, verification_time_unix)
    else {
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-sigstore-bundle-subject` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the Sigstore bundle subject verification
/// reports a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_sigstore_bundle_subject_command(
    args: &[String],
) -> Result<(), ExitError> {
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
                bundle_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--artifact-path" => {
                index += 1;
                artifact_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_paths.push(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--rekor-log-entry-path" => {
                index += 1;
                rekor_log_entry_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--rekor-public-key-path" => {
                index += 1;
                rekor_public_key_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--expected-rekor-log-id" => {
                index += 1;
                expected_rekor_log_id = Some(next_arg_or_err(args, index)?.to_string());
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
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-sigstore-dsse-in-toto-subject` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the Sigstore DSSE in-toto subject verification
/// reports a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_sigstore_dsse_in_toto_subject_command(
    args: &[String],
) -> Result<(), ExitError> {
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
                bundle_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--artifact-path" => {
                index += 1;
                artifact_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_paths.push(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--rekor-log-entry-path" => {
                index += 1;
                rekor_log_entry_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--rekor-public-key-path" => {
                index += 1;
                rekor_public_key_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--expected-rekor-log-id" => {
                index += 1;
                expected_rekor_log_id = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--expected-predicate-type" => {
                index += 1;
                expected_predicate_type = Some(next_arg_or_err(args, index)?.to_string());
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
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-sigstore-timestamp-authority` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the Sigstore timestamp authority verification
/// reports a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_sigstore_timestamp_authority_command(
    args: &[String],
) -> Result<(), ExitError> {
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--rekor-log-entry-path" => {
                index += 1;
                rekor_log_entry_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--rekor-public-key-path" => {
                index += 1;
                rekor_public_key_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--expected-rekor-log-id" => {
                index += 1;
                expected_rekor_log_id = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--rfc3161-timestamp-token-path" => {
                index += 1;
                rfc3161_timestamp_token_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--rfc3161-timestamped-signature-path" => {
                index += 1;
                rfc3161_timestamped_signature_path =
                    Some(PathBuf::from(next_arg_or_err(args, index)?));
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

    let (Some(trust_policy_path), Some(certificate_path)) = (trust_policy_path, certificate_path)
    else {
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-certificate-transparency-sct` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the certificate transparency SCT verification
/// reports a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_certificate_transparency_sct_command(
    args: &[String],
) -> Result<(), ExitError> {
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--sct-path" => {
                index += 1;
                sct_paths.push(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--verification-time-unix-ms" => {
                index += 1;
                verification_time_unix_ms = next_arg_or_err(args, index)?.parse::<u64>().ok();
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

    let (Some(trust_policy_path), Some(certificate_path), Some(verification_time_unix_ms)) = (
        trust_policy_path,
        certificate_path,
        verification_time_unix_ms,
    ) else {
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-certificate-revocation-policy` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the certificate revocation policy verification
/// reports a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_certificate_revocation_policy_command(
    args: &[String],
) -> Result<(), ExitError> {
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut trusted_signing_time_unix: Option<i64> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--trusted-signing-time-unix" => {
                index += 1;
                trusted_signing_time_unix = Some(parse_i64_or_err(next_arg_or_err(args, index)?)?);
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

    let (Some(trust_policy_path), Some(certificate_path), Some(trusted_signing_time_unix)) = (
        trust_policy_path,
        certificate_path,
        trusted_signing_time_unix,
    ) else {
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-tuf-trusted-root-freshness` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the TUF trusted root freshness verification
/// reports a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_tuf_trusted_root_freshness_command(
    args: &[String],
) -> Result<(), ExitError> {
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--root-metadata-path" => {
                index += 1;
                root_metadata_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--timestamp-metadata-path" => {
                index += 1;
                timestamp_metadata_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--snapshot-metadata-path" => {
                index += 1;
                snapshot_metadata_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--targets-metadata-path" => {
                index += 1;
                targets_metadata_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--update-start-time-unix" => {
                index += 1;
                update_start_time_unix = Some(parse_i64_or_err(next_arg_or_err(args, index)?)?);
            }
            "--min-root-version" => {
                index += 1;
                min_root_version = Some(parse_i64_or_err(next_arg_or_err(args, index)?)?);
            }
            "--min-timestamp-version" => {
                index += 1;
                min_timestamp_version = Some(parse_i64_or_err(next_arg_or_err(args, index)?)?);
            }
            "--min-snapshot-version" => {
                index += 1;
                min_snapshot_version = Some(parse_i64_or_err(next_arg_or_err(args, index)?)?);
            }
            "--min-targets-version" => {
                index += 1;
                min_targets_version = Some(parse_i64_or_err(next_arg_or_err(args, index)?)?);
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

    let (Some(trust_policy_path), Some(root_metadata_path), Some(update_start_time_unix)) = (
        trust_policy_path,
        root_metadata_path,
        update_start_time_unix,
    ) else {
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-certificate-crl-status` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the certificate CRL status verification
/// reports a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_certificate_crl_status_command(
    args: &[String],
) -> Result<(), ExitError> {
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--crl-path" => {
                index += 1;
                crl_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--verification-time-unix" => {
                index += 1;
                verification_time_unix = Some(parse_i64_or_err(next_arg_or_err(args, index)?)?);
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
        return Err(ExitError::usage(usage()));
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
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}

/// Runs the `host-adapter-verify-certificate-ocsp-status` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when required arguments are missing or invalid,
/// and `ExitError::failed` when the certificate OCSP status verification
/// reports a non-passed status.
///
/// # Panics
///
/// Panics if the verification result cannot be serialized as JSON. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_host_adapter_verify_certificate_ocsp_status_command(
    args: &[String],
) -> Result<(), ExitError> {
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut issuer_certificate_path: Option<PathBuf> = None;
    let mut ocsp_response_path: Option<PathBuf> = None;
    let mut verification_time_unix: Option<i64> = None;
    let mut expected_nonce_hex: Option<String> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--ocsp-response-path" => {
                index += 1;
                ocsp_response_path = Some(PathBuf::from(next_arg_or_err(args, index)?));
            }
            "--verification-time-unix" => {
                index += 1;
                verification_time_unix = Some(parse_i64_or_err(next_arg_or_err(args, index)?)?);
            }
            "--expected-nonce-hex" => {
                index += 1;
                expected_nonce_hex = Some(next_arg_or_err(args, index)?.to_string());
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

    let (
        Some(trust_policy_path),
        Some(certificate_path),
        Some(issuer_certificate_path),
        Some(ocsp_response_path),
        Some(verification_time_unix),
    ) = (
        trust_policy_path,
        certificate_path,
        issuer_certificate_path,
        ocsp_response_path,
        verification_time_unix,
    )
    else {
        return Err(ExitError::usage(usage()));
    };

    let verification = run_host_adapter_certificate_ocsp_status_verification(
        HostAdapterCertificateOcspStatusVerificationInput {
            trust_policy_path,
            certificate_path,
            issuer_certificate_path,
            ocsp_response_path,
            verification_time_unix,
            expected_nonce_hex,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verification)
                .expect("serialize certificate OCSP status verification")
        );
    } else {
        println!(
            "forge_core_host_adapter_certificate_ocsp_status_verification certificate={} status={:?} revocation_status={:?} reasons={:?}",
            verification.certificate_path,
            verification.status,
            verification.revocation_status,
            verification.reasons
        );
    }
    if verification.status == HostAdapterCertificateOcspStatusVerificationStatus::Failed {
        return Err(ExitError::failed("verification failed"));
    }
    Ok(())
}
