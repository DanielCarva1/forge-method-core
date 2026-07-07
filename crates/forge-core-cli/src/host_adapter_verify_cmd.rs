//! Host-adapter verification CLI dispatchers.
//!
//! Each `run_host_adapter_verify_*_command` parses argv into a typed
//! `HostAdapter*VerificationInput`, calls the corresponding verification
//! entrypoint re-exported from `forge_core_crypto`, prints the result as
//! JSON or human-readable text, and exits non-zero on verification failure.
//!
//! Extracted from the legacy god-file `main.rs` as part of R11.2.

use crate::cli_error::ExitError;
use crate::cli_util::command_surface_usage;
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
    HostAdapterTufTrustedRootFreshnessVerificationStatus, OcspNonceHex,
};
use forge_core_command_surface::{
    CommandSpec, COMMAND_HOST_ADAPTER_VERIFY_ARTIFACT,
    COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_CRL_STATUS,
    COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_OCSP_STATUS,
    COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_REVOCATION_POLICY,
    COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_TRANSPARENCY_SCT,
    COMMAND_HOST_ADAPTER_VERIFY_FULCIO_CERTIFICATE_IDENTITY,
    COMMAND_HOST_ADAPTER_VERIFY_PROVENANCE, COMMAND_HOST_ADAPTER_VERIFY_REKOR_ENTRY,
    COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_BUNDLE_SUBJECT,
    COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_DSSE_IN_TOTO_SUBJECT,
    COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TIMESTAMP_AUTHORITY,
    COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TRUST_POLICY,
    COMMAND_HOST_ADAPTER_VERIFY_TUF_TRUSTED_ROOT_FRESHNESS,
};
use std::path::PathBuf;

fn host_adapter_verify_usage(command: &CommandSpec) -> String {
    command_surface_usage(command)
}

fn next_arg_or_verify_usage<'a>(
    args: &'a [String],
    index: usize,
    command: &CommandSpec,
) -> Result<&'a str, ExitError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(host_adapter_verify_usage(command)))
}

fn parse_i64_or_verify_usage(value: &str, command: &CommandSpec) -> Result<i64, ExitError> {
    value
        .parse::<i64>()
        .map_err(|_| ExitError::usage(host_adapter_verify_usage(command)))
}

fn parse_u64_or_verify_usage(value: &str, command: &CommandSpec) -> Result<u64, ExitError> {
    value
        .parse::<u64>()
        .map_err(|_| ExitError::usage(host_adapter_verify_usage(command)))
}

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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_ARTIFACT;
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
                artifact_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--sha256" => {
                index += 1;
                expected_sha256 = Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--signature-ref" => {
                index += 1;
                signature_ref = Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--provenance-ref" => {
                index += 1;
                provenance_ref = Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--source-ref" => {
                index += 1;
                source_ref = Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--version" => {
                index += 1;
                version = Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--compatible-core-version" => {
                index += 1;
                compatible_core_version =
                    Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--rollback-ref" => {
                index += 1;
                rollback_ref = Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--update-summary-ref" => {
                index += 1;
                update_summary_ref =
                    Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
            }
        }
        index += 1;
    }

    let (Some(artifact_path), Some(expected_sha256)) = (artifact_path, expected_sha256) else {
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_PROVENANCE;
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
                artifact_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--provenance-path" => {
                index += 1;
                provenance_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--signature-path" => {
                index += 1;
                signature_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--public-key-path" => {
                index += 1;
                public_key_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--transparency-log-path" => {
                index += 1;
                transparency_log_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--sha256" => {
                index += 1;
                expected_sha256 = Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--expected-builder-id" => {
                index += 1;
                expected_builder_id =
                    Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--expected-source-uri" => {
                index += 1;
                expected_source_uri =
                    Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--expected-source-ref" => {
                index += 1;
                expected_source_ref =
                    Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_REKOR_ENTRY;
    let mut log_entry_path: Option<PathBuf> = None;
    let mut public_key_path: Option<PathBuf> = None;
    let mut expected_log_id: Option<String> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--log-entry-path" => {
                index += 1;
                log_entry_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--public-key-path" => {
                index += 1;
                public_key_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--expected-log-id" => {
                index += 1;
                expected_log_id = Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
            }
        }
        index += 1;
    }

    let (Some(log_entry_path), Some(public_key_path), Some(expected_log_id)) =
        (log_entry_path, public_key_path, expected_log_id)
    else {
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TRUST_POLICY;
    let mut policy_path: Option<PathBuf> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--policy-path" => {
                index += 1;
                policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
            }
        }
        index += 1;
    }

    let Some(policy_path) = policy_path else {
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_FULCIO_CERTIFICATE_IDENTITY;
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_paths.push(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--verification-time-unix" => {
                index += 1;
                verification_time_unix = Some(parse_i64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(certificate_path), Some(verification_time_unix)) =
        (trust_policy_path, certificate_path, verification_time_unix)
    else {
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_BUNDLE_SUBJECT;
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
                bundle_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--artifact-path" => {
                index += 1;
                artifact_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_paths.push(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--rekor-log-entry-path" => {
                index += 1;
                rekor_log_entry_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--rekor-public-key-path" => {
                index += 1;
                rekor_public_key_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--expected-rekor-log-id" => {
                index += 1;
                expected_rekor_log_id =
                    Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_DSSE_IN_TOTO_SUBJECT;
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
                bundle_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--artifact-path" => {
                index += 1;
                artifact_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_paths.push(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--rekor-log-entry-path" => {
                index += 1;
                rekor_log_entry_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--rekor-public-key-path" => {
                index += 1;
                rekor_public_key_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--expected-rekor-log-id" => {
                index += 1;
                expected_rekor_log_id =
                    Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--expected-predicate-type" => {
                index += 1;
                expected_predicate_type =
                    Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TIMESTAMP_AUTHORITY;
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--rekor-log-entry-path" => {
                index += 1;
                rekor_log_entry_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--rekor-public-key-path" => {
                index += 1;
                rekor_public_key_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--expected-rekor-log-id" => {
                index += 1;
                expected_rekor_log_id =
                    Some(next_arg_or_verify_usage(args, index, command)?.to_string());
            }
            "--rfc3161-timestamp-token-path" => {
                index += 1;
                rfc3161_timestamp_token_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--rfc3161-timestamped-signature-path" => {
                index += 1;
                rfc3161_timestamped_signature_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(certificate_path)) = (trust_policy_path, certificate_path)
    else {
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_TRANSPARENCY_SCT;
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--sct-path" => {
                index += 1;
                sct_paths.push(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--verification-time-unix-ms" => {
                index += 1;
                verification_time_unix_ms = Some(parse_u64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(certificate_path), Some(verification_time_unix_ms)) = (
        trust_policy_path,
        certificate_path,
        verification_time_unix_ms,
    ) else {
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_REVOCATION_POLICY;
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut trusted_signing_time_unix: Option<i64> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--trusted-signing-time-unix" => {
                index += 1;
                trusted_signing_time_unix = Some(parse_i64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(certificate_path), Some(trusted_signing_time_unix)) = (
        trust_policy_path,
        certificate_path,
        trusted_signing_time_unix,
    ) else {
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_TUF_TRUSTED_ROOT_FRESHNESS;
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--root-metadata-path" => {
                index += 1;
                root_metadata_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--timestamp-metadata-path" => {
                index += 1;
                timestamp_metadata_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--snapshot-metadata-path" => {
                index += 1;
                snapshot_metadata_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--targets-metadata-path" => {
                index += 1;
                targets_metadata_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--update-start-time-unix" => {
                index += 1;
                update_start_time_unix = Some(parse_i64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--min-root-version" => {
                index += 1;
                min_root_version = Some(parse_i64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--min-timestamp-version" => {
                index += 1;
                min_timestamp_version = Some(parse_i64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--min-snapshot-version" => {
                index += 1;
                min_snapshot_version = Some(parse_i64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--min-targets-version" => {
                index += 1;
                min_targets_version = Some(parse_i64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
            }
        }
        index += 1;
    }

    let (Some(trust_policy_path), Some(root_metadata_path), Some(update_start_time_unix)) = (
        trust_policy_path,
        root_metadata_path,
        update_start_time_unix,
    ) else {
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_CRL_STATUS;
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
                trust_policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--crl-path" => {
                index += 1;
                crl_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--verification-time-unix" => {
                index += 1;
                verification_time_unix = Some(parse_i64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
    let command = &COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_OCSP_STATUS;
    let mut trust_policy_path: Option<PathBuf> = None;
    let mut certificate_path: Option<PathBuf> = None;
    let mut issuer_certificate_path: Option<PathBuf> = None;
    let mut ocsp_response_path: Option<PathBuf> = None;
    let mut verification_time_unix: Option<i64> = None;
    let mut expected_nonce_hex: Option<OcspNonceHex> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--trust-policy-path" => {
                index += 1;
                trust_policy_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--certificate-path" => {
                index += 1;
                certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--issuer-certificate-path" => {
                index += 1;
                issuer_certificate_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--ocsp-response-path" => {
                index += 1;
                ocsp_response_path = Some(PathBuf::from(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--verification-time-unix" => {
                index += 1;
                verification_time_unix = Some(parse_i64_or_verify_usage(
                    next_arg_or_verify_usage(args, index, command)?,
                    command,
                )?);
            }
            "--expected-nonce-hex" => {
                index += 1;
                expected_nonce_hex = Some(OcspNonceHex::new(next_arg_or_verify_usage(
                    args, index, command,
                )?));
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", host_adapter_verify_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(host_adapter_verify_usage(command)));
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
        return Err(ExitError::usage(host_adapter_verify_usage(command)));
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

#[cfg(test)]
mod tests {
    use super::*;

    type VerifyRunner = fn(&[String]) -> Result<(), ExitError>;

    fn args(slice: &[&str]) -> Vec<String> {
        slice.iter().map(std::string::ToString::to_string).collect()
    }

    fn verify_command_cases() -> [(&'static CommandSpec, VerifyRunner, &'static str); 13] {
        [
            (
                &COMMAND_HOST_ADAPTER_VERIFY_ARTIFACT,
                run_host_adapter_verify_artifact_command,
                "host-adapter-verify-artifact",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_PROVENANCE,
                run_host_adapter_verify_provenance_command,
                "host-adapter-verify-provenance",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_REKOR_ENTRY,
                run_host_adapter_verify_rekor_entry_command,
                "host-adapter-verify-rekor-entry",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TRUST_POLICY,
                run_host_adapter_verify_sigstore_trust_policy_command,
                "host-adapter-verify-sigstore-trust-policy",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_FULCIO_CERTIFICATE_IDENTITY,
                run_host_adapter_verify_fulcio_certificate_identity_command,
                "host-adapter-verify-fulcio-certificate-identity",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_BUNDLE_SUBJECT,
                run_host_adapter_verify_sigstore_bundle_subject_command,
                "host-adapter-verify-sigstore-bundle-subject",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_DSSE_IN_TOTO_SUBJECT,
                run_host_adapter_verify_sigstore_dsse_in_toto_subject_command,
                "host-adapter-verify-sigstore-dsse-in-toto-subject",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TIMESTAMP_AUTHORITY,
                run_host_adapter_verify_sigstore_timestamp_authority_command,
                "host-adapter-verify-sigstore-timestamp-authority",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_TRANSPARENCY_SCT,
                run_host_adapter_verify_certificate_transparency_sct_command,
                "host-adapter-verify-certificate-transparency-sct",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_REVOCATION_POLICY,
                run_host_adapter_verify_certificate_revocation_policy_command,
                "host-adapter-verify-certificate-revocation-policy",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_TUF_TRUSTED_ROOT_FRESHNESS,
                run_host_adapter_verify_tuf_trusted_root_freshness_command,
                "host-adapter-verify-tuf-trusted-root-freshness",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_CRL_STATUS,
                run_host_adapter_verify_certificate_crl_status_command,
                "host-adapter-verify-certificate-crl-status",
            ),
            (
                &COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_OCSP_STATUS,
                run_host_adapter_verify_certificate_ocsp_status_command,
                "host-adapter-verify-certificate-ocsp-status",
            ),
        ]
    }

    #[test]
    fn host_adapter_verify_usage_projects_command_surface_lines() {
        for (command, _, _) in verify_command_cases() {
            let usage = host_adapter_verify_usage(command);
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
    fn missing_inputs_report_command_specific_verify_usage() {
        for (command, runner, argv0) in verify_command_cases() {
            let error = runner(&args(&[argv0, "--no-json"]))
                .expect_err("missing required inputs should be usage");
            assert_eq!(
                error.message(),
                host_adapter_verify_usage(command),
                "{} should report its own Command Surface usage",
                command.name
            );
        }
    }

    #[test]
    fn invalid_numeric_input_reports_command_specific_verify_usage() {
        let error = run_host_adapter_verify_certificate_transparency_sct_command(&args(&[
            "host-adapter-verify-certificate-transparency-sct",
            "--trust-policy-path",
            "policy.yaml",
            "--certificate-path",
            "cert.pem",
            "--sct-path",
            "sct.bin",
            "--verification-time-unix-ms",
            "not-a-number",
            "--no-json",
        ]))
        .expect_err("invalid timestamp should be usage");
        assert_eq!(
            error.message(),
            host_adapter_verify_usage(&COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_TRANSPARENCY_SCT)
        );
    }

    #[test]
    fn explicit_no_json_reaches_artifact_verification_instead_of_usage() {
        let path = std::env::temp_dir().join(format!(
            "forge-core-host-adapter-verify-artifact-{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, b"artifact").expect("write temp artifact");
        let path_string = path.to_string_lossy().to_string();
        let result = run_host_adapter_verify_artifact_command(&args(&[
            "host-adapter-verify-artifact",
            "--artifact-path",
            &path_string,
            "--sha256",
            "not-a-real-sha256",
            "--no-json",
        ]));
        let _ = std::fs::remove_file(&path);
        assert!(
            !matches!(result, Err(ExitError::Usage { .. })),
            "explicit --no-json should parse and reach verification, got {result:?}"
        );
    }
}
