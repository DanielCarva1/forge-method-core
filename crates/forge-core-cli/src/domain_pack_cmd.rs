//! Agent surface for governed Domain Pack validation, resolution, and state.

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_authority::{
    verify_domain_pack_supply_chain_snapshot, AnchoredDomainPackSupplyChainSnapshot,
    DomainPackRegistryAnchor, DomainPackRegistryAnchorAdvance,
    VerifiedDomainPackSupplyChainSnapshot,
};
use forge_core_command_surface::COMMAND_DOMAIN_PACK;
use forge_core_contracts::{
    CliEnvelope, DomainPackActivePointerDocument, DomainPackArtifactBinding,
    DomainPackCandidateAuthority, DomainPackCandidateInput, DomainPackCompositionIssue,
    DomainPackCompositionRequestDocument, DomainPackContentDocument, DomainPackExactLockDocument,
    DomainPackLifecycleLedgerRecord, DomainPackLifecyclePreflightDocument,
    DomainPackManifestDocument, DomainPackResolutionRequestDocument,
    DomainPackSupplyChainRegistryDocument, DomainPackTrustPolicyDocument, RepoPath, StableId,
};
use forge_core_decisions::{
    compose_domain_packs, resolve_domain_packs, validate_domain_pack_candidate,
    DomainPackCandidateMaterial, DomainPackTrustEvaluationInput,
    MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES,
};
use forge_core_domain_pack_tcb::{
    authorize_prepared_domain_pack_lifecycle, lock_domain_pack_lifecycle,
    verify_domain_pack_project_snapshot, DomainPackImmutableArtifact,
    DomainPackLifecycleAuthorizationContext, DomainPackLifecycleStoreError,
    DOMAIN_PACK_MAX_DOCUMENT_BYTES,
};
use forge_core_store::{
    acquire_effect_store_lock,
    crash_replace::{recover_file_crash_safe_under_lock, replace_file_crash_safe_under_lock},
    EffectStoreLock,
};
use sha2::{Digest, Sha256};

use crate::cli_error::ExitError;

#[derive(Debug, serde::Serialize)]
struct DomainPackValidationPayload {
    authority: DomainPackCandidateAuthority,
    structurally_valid: bool,
    publisher: String,
    name: String,
    version: String,
    issues: Vec<DomainPackCompositionIssue>,
    boundary: &'static str,
}

#[derive(Debug, serde::Serialize)]
struct DomainPackLifecycleStatusPayload {
    state_root: String,
    active: bool,
    active_pointer: Option<DomainPackActivePointerDocument>,
    active_lock: Option<DomainPackExactLockDocument>,
    ledger_records: Vec<DomainPackLifecycleLedgerRecord>,
    recovery_report: forge_core_contracts::DomainPackRecoveryReportDocument,
    recovery_checked: bool,
    boundary: &'static str,
}

#[derive(Debug, serde::Serialize)]
struct DomainPackLifecyclePreflightPayload {
    ready: bool,
    preflight_digest: String,
    supply_chain: forge_core_authority::VerifiedDomainPackSupplyChainSnapshotAudit,
    boundary: &'static str,
}

#[derive(Debug, serde::Serialize)]
struct DomainPackTrustProvisionPayload {
    operator_root: String,
    registry_id: StableId,
    audience: StableId,
    generation: u64,
    snapshot_digest: String,
    trust_policy_digest: String,
    anchor_previously_present: bool,
    boundary: &'static str,
}

struct OwnedMaterial {
    manifest: Vec<u8>,
    content: Vec<u8>,
    license: Vec<u8>,
}

struct OwnedImmutableArtifact {
    binding: DomainPackArtifactBinding,
    raw_bytes: Vec<u8>,
}

struct CanonicalLifecycleRoots {
    project: PathBuf,
    artifacts: PathBuf,
    state: PathBuf,
    project_lexical: PathBuf,
    artifacts_lexical: PathBuf,
    state_lexical: PathBuf,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackRegistryAnchorHead {
    schema_version: String,
    registry_id: StableId,
    audience: StableId,
    generation: u64,
    snapshot_digest: String,
    trust_policy_digest: String,
}

struct LockedOperatorRegistryAnchor {
    operator_root: PathBuf,
    lock: EffectStoreLock,
    previous_digest: Option<String>,
}

const DOMAIN_PACK_REGISTRY_ANCHOR_SCHEMA_VERSION: &str = "forge-domain-pack-registry-anchor-v1";
const DOMAIN_PACK_REGISTRY_ANCHOR_RELATIVE_PATH: &str = ".forge-domain-pack-registry-anchor.yaml";
const DOMAIN_PACK_REGISTRY_ANCHOR_LOCK_RELATIVE_PATH: &str =
    ".forge-domain-pack-registry-anchor.lock";
const DOMAIN_PACK_TRUST_ON_FIRST_USE_ACKNOWLEDGEMENT: &str = "I_UNDERSTAND_TRUST_ON_FIRST_USE";

/// Dispatch the governed `forge-core domain-pack` command family.
///
/// # Errors
///
/// Returns a typed CLI error when arguments, bounded inputs, trust roots,
/// registry continuity, lifecycle authorization, or durable state fail closed.
pub fn run_domain_pack_command(args: &[String]) -> Result<(), ExitError> {
    match args.get(1).map_or("--help", String::as_str) {
        "validate" => run_validate(&args[2..]),
        "compose" => run_compose(&args[2..]),
        "resolve" => run_resolve(&args[2..]),
        "trust-provision" => run_trust_provision(&args[2..]),
        "status" => run_lifecycle_state(&args[2..], false),
        "recover" => run_lifecycle_state(&args[2..], true),
        "preflight" => run_lifecycle_authorized(&args[2..], false),
        "apply" => run_lifecycle_authorized(&args[2..], true),
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core domain-pack: unknown subcommand '{other}'\n{}",
            usage()
        ))),
    }
}

fn run_trust_provision(args: &[String]) -> Result<(), ExitError> {
    let mut operator_root: Option<PathBuf> = None;
    let mut trust_policy_file: Option<PathBuf> = None;
    let mut registry_file: Option<PathBuf> = None;
    let mut project_root: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut state_root = PathBuf::from(".forge-method");
    let mut acknowledgement: Option<String> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--operator-root" => set_path(&mut operator_root, value),
            "--trust-policy-file" => set_path(&mut trust_policy_file, value),
            "--registry-file" => set_path(&mut registry_file, value),
            "--project-root" => set_path(&mut project_root, value),
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            "--state-root" => {
                state_root = PathBuf::from(value);
                true
            }
            "--operator-acknowledge-trust-on-first-use" => {
                acknowledgement = Some(value.to_owned());
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let operator_root = required(operator_root)?;
    let trust_policy_file = required(trust_policy_file)?;
    let registry_file = required(registry_file)?;
    let project_root = required(project_root)?;
    if acknowledgement.as_deref() != Some(DOMAIN_PACK_TRUST_ON_FIRST_USE_ACKNOWLEDGEMENT) {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: trust provisioning requires explicit operator acknowledgement via --operator-acknowledge-trust-on-first-use {DOMAIN_PACK_TRUST_ON_FIRST_USE_ACKNOWLEDGEMENT}"
        )));
    }

    let controlled_roots = canonical_lifecycle_roots(&project_root, &artifact_root, &state_root)?;
    let operator_root = canonical_external_operator_root(&operator_root, &controlled_roots)?;
    let trust_policy_file = trusted_external_file(
        &trust_policy_file,
        "operator trust policy",
        &controlled_roots,
    )?;
    let registry_file = trusted_external_file(
        &registry_file,
        "signed supply-chain registry",
        &controlled_roots,
    )?;
    require_direct_operator_file(&trust_policy_file, &operator_root, "operator trust policy")?;
    require_direct_operator_file(
        &registry_file,
        &operator_root,
        "signed supply-chain registry",
    )?;

    let locked = lock_operator_registry_anchor(&registry_file)?;
    let now_unix = trusted_now_unix()?;
    let trust_policy: DomainPackTrustPolicyDocument =
        read_typed(&trust_policy_file, "operator trust policy")?;
    let registry: DomainPackSupplyChainRegistryDocument =
        read_typed(&registry_file, "signed supply-chain registry")?;
    let verified = verify_domain_pack_supply_chain_snapshot(&trust_policy, &registry, now_unix)
        .map_err(|error| {
            ExitError::with_code(
                2,
                format!("domain-pack: supply-chain verification rejected: {error}"),
            )
        })?;
    let anchor_previously_present = locked.previous_digest.is_some();
    let mut anchor = if anchor_previously_present {
        load_operator_registry_anchor(&locked)?
    } else {
        DomainPackRegistryAnchor::new_trust_on_first_use(
            registry
                .domain_pack_supply_chain_registry
                .registry_id
                .clone(),
            trust_policy.domain_pack_trust_policy.audience.clone(),
        )
        .map_err(|error| {
            ExitError::with_code(
                2,
                format!("domain-pack: operator registry anchor rejected: {error}"),
            )
        })?
    };
    let anchored = advance_operator_registry_anchor(&locked, &mut anchor, verified)?;
    let verified = anchored.verified_snapshot();
    let payload = DomainPackTrustProvisionPayload {
        operator_root: operator_root.to_string_lossy().replace('\\', "/"),
        registry_id: verified.registry_id().clone(),
        audience: verified.audience().clone(),
        generation: verified.generation(),
        snapshot_digest: verified.snapshot_digest().to_owned(),
        trust_policy_digest: verified.trust_policy_digest().to_owned(),
        anchor_previously_present,
        boundary: "explicit operator-approved trust-on-first-use or monotonic anchor advance; no project lifecycle activation authority",
    };
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack trust-provision", payload),
        want_json,
    )
}

fn run_resolve(args: &[String]) -> Result<(), ExitError> {
    let mut request_file: Option<PathBuf> = None;
    let mut registry_file: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--request-file" => {
                request_file = Some(PathBuf::from(value));
                true
            }
            "--registry-file" => {
                registry_file = Some(PathBuf::from(value));
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let request_file = request_file.ok_or_else(|| ExitError::usage(usage()))?;
    let registry_file = registry_file.ok_or_else(|| ExitError::usage(usage()))?;
    let request_raw = read_bounded(
        &request_file,
        "resolution request",
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )?;
    let registry_raw = read_bounded(
        &registry_file,
        "supply-chain registry",
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )?;
    let request: DomainPackResolutionRequestDocument = parse(&request_raw, &request_file)?;
    let registry: DomainPackSupplyChainRegistryDocument = parse(&registry_raw, &registry_file)?;
    let projection = resolve_domain_packs(&request, &registry);
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack resolve", projection),
        want_json,
    )
}

fn run_lifecycle_state(args: &[String], recover: bool) -> Result<(), ExitError> {
    let mut state_root = PathBuf::from(".forge-method");
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--state-root" => {
                state_root = PathBuf::from(value);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let locked = lock_domain_pack_lifecycle(&state_root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect lifecycle state '{}': {error}",
            state_root.display()
        ))
    })?;
    let projection = locked.projection();
    let recovery_report = locked.recovery_report();
    let payload = DomainPackLifecycleStatusPayload {
        state_root: std::fs::canonicalize(&state_root)
            .unwrap_or(state_root)
            .to_string_lossy()
            .replace('\\', "/"),
        active: projection.active_pointer.is_some(),
        active_pointer: projection.active_pointer.clone(),
        active_lock: projection.active_lock.clone(),
        ledger_records: projection.ledger_records.clone(),
        recovery_report,
        recovery_checked: true,
        boundary: if recover {
            "crash-recovery checked under the retained lifecycle lock; no policy or activation authority"
        } else {
            "integrity-checked state projection; interrupted pointer replacement is recovered before read"
        },
    };
    let command = if recover {
        "domain-pack recover"
    } else {
        "domain-pack status"
    };
    crate::cli_util::emit_envelope(CliEnvelope::ok(command, payload), want_json)
}

fn run_lifecycle_authorized(args: &[String], apply: bool) -> Result<(), ExitError> {
    let mut preflight_file: Option<PathBuf> = None;
    let mut trust_policy_file: Option<PathBuf> = None;
    let mut registry_file: Option<PathBuf> = None;
    let mut resolution_request_file: Option<PathBuf> = None;
    let mut composition_request_file: Option<PathBuf> = None;
    let mut trust_input_file: Option<PathBuf> = None;
    let mut project_root: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut state_root = PathBuf::from(".forge-method");
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--preflight-file" => set_path(&mut preflight_file, value),
            "--trust-policy-file" => set_path(&mut trust_policy_file, value),
            "--registry-file" => set_path(&mut registry_file, value),
            "--resolution-request-file" => set_path(&mut resolution_request_file, value),
            "--composition-request-file" => set_path(&mut composition_request_file, value),
            "--trust-input-file" => set_path(&mut trust_input_file, value),
            "--project-root" => set_path(&mut project_root, value),
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            "--state-root" => {
                state_root = PathBuf::from(value);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let preflight_file = required(preflight_file)?;
    let trust_policy_file = required(trust_policy_file)?;
    let registry_file = required(registry_file)?;
    let resolution_request_file = required(resolution_request_file)?;
    let composition_request_file = required(composition_request_file)?;
    let trust_input_file = required(trust_input_file)?;
    let project_root = required(project_root)?;
    let now_unix = trusted_now_unix()?;

    let controlled_roots = canonical_lifecycle_roots(&project_root, &artifact_root, &state_root)?;
    let trust_policy_file = trusted_external_file(
        &trust_policy_file,
        "operator trust policy",
        &controlled_roots,
    )?;
    let registry_file = trusted_external_file(
        &registry_file,
        "signed supply-chain registry",
        &controlled_roots,
    )?;

    let preflight: DomainPackLifecyclePreflightDocument =
        read_typed(&preflight_file, "lifecycle preflight")?;
    let owned_artifacts = load_immutable_artifacts(&preflight, &artifact_root)?;
    let artifacts = immutable_artifact_views(&owned_artifacts);
    let operator_anchor = lock_operator_registry_anchor(&registry_file)?;
    let trust_policy: DomainPackTrustPolicyDocument =
        read_typed(&trust_policy_file, "operator trust policy")?;
    let registry: DomainPackSupplyChainRegistryDocument =
        read_typed(&registry_file, "signed supply-chain registry")?;
    let mut registry_anchor = load_operator_registry_anchor(&operator_anchor)?;
    let resolution_request: DomainPackResolutionRequestDocument =
        read_typed(&resolution_request_file, "resolution request")?;
    let composition_request: DomainPackCompositionRequestDocument =
        read_typed(&composition_request_file, "composition request")?;
    let trust_input: DomainPackTrustEvaluationInput =
        read_typed(&trust_input_file, "trust evaluation input")?;

    let expected_project_snapshot_digest = &preflight
        .domain_pack_lifecycle_preflight
        .request
        .domain_pack_lifecycle_request
        .project_snapshot_digest;
    let project_snapshot = verify_domain_pack_project_snapshot(
        &controlled_roots.project,
        expected_project_snapshot_digest,
    )
    .map_err(map_lifecycle_error)?;

    let verified_snapshot =
        verify_domain_pack_supply_chain_snapshot(&trust_policy, &registry, now_unix).map_err(
            |error| {
                ExitError::with_code(
                    2,
                    format!("domain-pack: supply-chain verification rejected: {error}"),
                )
            },
        )?;
    let anchored_snapshot = advance_operator_registry_anchor(
        &operator_anchor,
        &mut registry_anchor,
        verified_snapshot,
    )?;
    let owned = load_composition_materials(&composition_request, &artifact_root)?;
    let materials = material_views(&composition_request, &owned);
    let mut lifecycle = lock_domain_pack_lifecycle(&state_root).map_err(map_lifecycle_error)?;
    let prepared = lifecycle
        .prepare_candidate(preflight.clone())
        .map_err(map_lifecycle_error)?;
    let context = DomainPackLifecycleAuthorizationContext {
        anchored_snapshot: &anchored_snapshot,
        project_snapshot: &project_snapshot,
        trust_policy_document: &trust_policy,
        registry_document: &registry,
        resolution_request: &resolution_request,
        composition_request: &composition_request,
        materials: &materials,
        artifacts: &artifacts,
        trust_input: &trust_input,
    };
    let authority = authorize_prepared_domain_pack_lifecycle(&prepared, &context)
        .map_err(map_lifecycle_error)?;
    if apply {
        let receipt = lifecycle
            .commit(prepared, authority)
            .map_err(map_lifecycle_error)?;
        crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack apply", receipt), want_json)
    } else {
        let payload = DomainPackLifecyclePreflightPayload {
            ready: true,
            preflight_digest: preflight.domain_pack_lifecycle_preflight.preflight_digest,
            supply_chain: anchored_snapshot.verified_snapshot().audit(),
            boundary: "fresh verification completed under lifecycle lock; this preflight did not activate the candidate generation",
        };
        crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack preflight", payload), want_json)
    }
}

fn set_path(target: &mut Option<PathBuf>, value: &str) -> bool {
    *target = Some(PathBuf::from(value));
    true
}

fn required(path: Option<PathBuf>) -> Result<PathBuf, ExitError> {
    path.ok_or_else(|| ExitError::usage(usage()))
}

fn map_lifecycle_error(error: DomainPackLifecycleStoreError) -> ExitError {
    match error {
        DomainPackLifecycleStoreError::StaleExpectedState { .. } => {
            ExitError::conflict(error.to_string())
        }
        DomainPackLifecycleStoreError::PreflightBlocked { .. } => {
            ExitError::with_code(2, error.to_string())
        }
        DomainPackLifecycleStoreError::InvalidArgument { .. }
        | DomainPackLifecycleStoreError::InvalidDigest { .. }
        | DomainPackLifecycleStoreError::InvalidDocument { .. } => {
            ExitError::invalid_value(error.to_string())
        }
        _ => ExitError::failed(error.to_string()),
    }
}

fn run_validate(args: &[String]) -> Result<(), ExitError> {
    let mut manifest_file: Option<PathBuf> = None;
    let mut content_file: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut forge_core_version = env!("CARGO_PKG_VERSION").to_owned();
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--manifest-file" => {
                manifest_file = Some(PathBuf::from(value));
                true
            }
            "--content-file" => {
                content_file = Some(PathBuf::from(value));
                true
            }
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            "--forge-core-version" => {
                value.clone_into(&mut forge_core_version);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let manifest_file = manifest_file.ok_or_else(|| ExitError::usage(usage()))?;
    let content_file = content_file.ok_or_else(|| ExitError::usage(usage()))?;
    let manifest_raw = read(&manifest_file, "manifest")?;
    let content_raw = read(&content_file, "content")?;
    let manifest: DomainPackManifestDocument = parse(&manifest_raw, &manifest_file)?;
    let content: DomainPackContentDocument = parse(&content_raw, &content_file)?;
    let license_path = safe_join(
        &artifact_root,
        &manifest
            .domain_pack_manifest
            .provenance
            .license_text
            .artifact_ref
            .0,
    )?;
    let license_raw = read(&license_path, "license artifact")?;
    let identity = manifest.domain_pack_manifest.identity.clone();
    let candidate = DomainPackCandidateInput {
        // Standalone validation receives the authored manifest as an explicit
        // CLI input rather than through a composition request. Bind those exact
        // bytes and their closed typed semantics before handing the candidate
        // to the pure validator.
        manifest_binding: DomainPackArtifactBinding {
            artifact_ref: RepoPath("manifest-input.yaml".to_owned()),
            raw_sha256: sha256_bytes(&manifest_raw),
            canonical_sha256: canonical_digest(&manifest)?,
        },
        manifest,
        content,
    };
    let material = DomainPackCandidateMaterial {
        publisher: &identity.publisher.0,
        name: &identity.name.0,
        version: &identity.version,
        manifest_raw: &manifest_raw,
        content_raw: &content_raw,
        license_raw: &license_raw,
    };
    let issues = validate_domain_pack_candidate(&candidate, &material, &forge_core_version);
    let payload = DomainPackValidationPayload {
        authority: DomainPackCandidateAuthority::CandidateOnly,
        structurally_valid: issues.is_empty(),
        publisher: identity.publisher.0,
        name: identity.name.0,
        version: identity.version,
        issues,
        boundary: "candidate_only; no install, trust, activation, execution, or mutation authority",
    };
    crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack validate", payload), want_json)
}

fn run_compose(args: &[String]) -> Result<(), ExitError> {
    let mut request_file: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--request-file" => {
                request_file = Some(PathBuf::from(value));
                true
            }
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let request_file = request_file.ok_or_else(|| ExitError::usage(usage()))?;
    let request_raw = read(&request_file, "composition request")?;
    let request: DomainPackCompositionRequestDocument = parse(&request_raw, &request_file)?;

    let owned = load_composition_materials(&request, &artifact_root)?;
    let materials = material_views(&request, &owned);
    let projection = compose_domain_packs(&request, &materials);
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack compose", projection),
        want_json,
    )
}

fn load_composition_materials(
    request: &DomainPackCompositionRequestDocument,
    artifact_root: &Path,
) -> Result<Vec<OwnedMaterial>, ExitError> {
    request
        .domain_pack_composition_request
        .candidates
        .iter()
        .map(|candidate| {
            let manifest = &candidate.manifest.domain_pack_manifest;
            let manifest_path =
                safe_join(artifact_root, &candidate.manifest_binding.artifact_ref.0)?;
            let content_path = safe_join(artifact_root, &manifest.content.content_ref.0)?;
            let license_path = safe_join(
                artifact_root,
                &manifest.provenance.license_text.artifact_ref.0,
            )?;
            Ok(OwnedMaterial {
                manifest: read(&manifest_path, "pack manifest")?,
                content: read(&content_path, "pack content")?,
                license: read(&license_path, "license artifact")?,
            })
        })
        .collect::<Result<Vec<_>, ExitError>>()
}

fn material_views<'a>(
    request: &'a DomainPackCompositionRequestDocument,
    owned: &'a [OwnedMaterial],
) -> Vec<DomainPackCandidateMaterial<'a>> {
    request
        .domain_pack_composition_request
        .candidates
        .iter()
        .zip(owned)
        .map(|(candidate, owned)| {
            let identity = &candidate.manifest.domain_pack_manifest.identity;
            DomainPackCandidateMaterial {
                publisher: &identity.publisher.0,
                name: &identity.name.0,
                version: &identity.version,
                manifest_raw: &owned.manifest,
                content_raw: &owned.content,
                license_raw: &owned.license,
            }
        })
        .collect()
}

fn load_immutable_artifacts(
    preflight: &DomainPackLifecyclePreflightDocument,
    artifact_root: &Path,
) -> Result<Vec<OwnedImmutableArtifact>, ExitError> {
    preflight
        .domain_pack_lifecycle_preflight
        .staged_artifacts
        .iter()
        .map(|binding| {
            let path = safe_join(artifact_root, &binding.artifact_ref.0)?;
            let raw_bytes = read_bounded(
                &path,
                "staged immutable artifact",
                DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            )?;
            let actual = sha256_bytes(&raw_bytes);
            if actual != binding.raw_sha256 {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: staged immutable artifact '{}' differs from raw_sha256 binding: expected '{}', found '{actual}'",
                    binding.artifact_ref.0, binding.raw_sha256
                )));
            }
            Ok(OwnedImmutableArtifact {
                binding: binding.clone(),
                raw_bytes,
            })
        })
        .collect()
}

fn immutable_artifact_views(
    owned: &[OwnedImmutableArtifact],
) -> Vec<DomainPackImmutableArtifact<'_>> {
    owned
        .iter()
        .map(|artifact| DomainPackImmutableArtifact {
            binding: &artifact.binding,
            raw_bytes: &artifact.raw_bytes,
        })
        .collect()
}

fn canonical_lifecycle_roots(
    project_root: &Path,
    artifact_root: &Path,
    state_root: &Path,
) -> Result<CanonicalLifecycleRoots, ExitError> {
    Ok(CanonicalLifecycleRoots {
        project: canonicalize_existing_root(project_root, "--project-root")?,
        artifacts: canonicalize_existing_root(artifact_root, "--artifact-root")?,
        state: canonicalize_allow_missing(state_root, "--state-root")?,
        project_lexical: absolute_lexical(project_root, "--project-root")?,
        artifacts_lexical: absolute_lexical(artifact_root, "--artifact-root")?,
        state_lexical: absolute_lexical(state_root, "--state-root")?,
    })
}

fn absolute_lexical(path: &Path, label: &str) -> Result<PathBuf, ExitError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                ExitError::failed(format!(
                    "domain-pack: cannot resolve current directory for {label}: {error}"
                ))
            })?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(ExitError::invalid_value(format!(
                        "domain-pack: {label} '{}' escapes its filesystem root",
                        path.display()
                    )));
                }
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

fn canonicalize_existing_root(path: &Path, label: &str) -> Result<PathBuf, ExitError> {
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve {label} '{}': {error}",
            path.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must resolve to a directory",
            path.display()
        )));
    }
    Ok(canonical)
}

fn canonicalize_allow_missing(path: &Path, label: &str) -> Result<PathBuf, ExitError> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        if !canonical.is_dir() {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: {label} '{}' must resolve to a directory",
                path.display()
            )));
        }
        return Ok(canonical);
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                ExitError::failed(format!(
                    "domain-pack: cannot resolve current directory for {label}: {error}"
                ))
            })?
            .join(path)
    };
    let mut cursor = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        match std::fs::canonicalize(cursor) {
            Ok(mut canonical) => {
                for component in missing.iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = cursor.file_name().ok_or_else(|| {
                    ExitError::failed(format!(
                        "domain-pack: cannot resolve missing {label} '{}'",
                        path.display()
                    ))
                })?;
                missing.push(name.to_os_string());
                cursor = cursor.parent().ok_or_else(|| {
                    ExitError::failed(format!(
                        "domain-pack: cannot resolve missing {label} '{}'",
                        path.display()
                    ))
                })?;
            }
            Err(error) => {
                return Err(ExitError::failed(format!(
                    "domain-pack: cannot resolve {label} '{}': {error}",
                    path.display()
                )));
            }
        }
    }
}

fn trusted_external_file(
    path: &Path,
    label: &str,
    roots: &CanonicalLifecycleRoots,
) -> Result<PathBuf, ExitError> {
    let link_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect {label} '{}': {error}",
            path.display()
        ))
    })?;
    if link_metadata.file_type().is_symlink() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must not be a symbolic link or junction",
            path.display()
        )));
    }
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve {label} '{}': {error}",
            path.display()
        ))
    })?;
    let metadata = std::fs::metadata(&canonical).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect resolved {label} '{}': {error}",
            canonical.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must be a regular file",
            path.display()
        )));
    }
    let lexical = absolute_lexical(path, label)?;
    for (root, lexical_root, root_label) in [
        (&roots.project, &roots.project_lexical, "--project-root"),
        (
            &roots.artifacts,
            &roots.artifacts_lexical,
            "--artifact-root",
        ),
        (&roots.state, &roots.state_lexical, "--state-root"),
    ] {
        if canonical.starts_with(root) || lexical.starts_with(lexical_root) {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: {label} '{}' must be operator-controlled and external to {root_label}",
                path.display()
            )));
        }
    }
    Ok(canonical)
}

fn canonical_external_operator_root(
    path: &Path,
    roots: &CanonicalLifecycleRoots,
) -> Result<PathBuf, ExitError> {
    let link_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect --operator-root '{}': {error}",
            path.display()
        ))
    })?;
    if link_metadata.file_type().is_symlink() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: --operator-root '{}' must not be a symbolic link or junction",
            path.display()
        )));
    }
    let canonical = canonicalize_existing_root(path, "--operator-root")?;
    let lexical = absolute_lexical(path, "--operator-root")?;
    for (root, lexical_root, root_label) in [
        (&roots.project, &roots.project_lexical, "--project-root"),
        (
            &roots.artifacts,
            &roots.artifacts_lexical,
            "--artifact-root",
        ),
        (&roots.state, &roots.state_lexical, "--state-root"),
    ] {
        if canonical.starts_with(root)
            || root.starts_with(&canonical)
            || lexical.starts_with(lexical_root)
            || lexical_root.starts_with(&lexical)
        {
            return Err(ExitError::invalid_value(format!(
                "domain-pack: --operator-root '{}' must be operator-controlled and disjoint from {root_label}",
                path.display()
            )));
        }
    }
    Ok(canonical)
}

fn require_direct_operator_file(
    file: &Path,
    operator_root: &Path,
    label: &str,
) -> Result<(), ExitError> {
    if file.parent() != Some(operator_root) {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must be a direct child of --operator-root '{}'",
            file.display(),
            operator_root.display()
        )));
    }
    Ok(())
}

fn lock_operator_registry_anchor(
    registry_file: &Path,
) -> Result<LockedOperatorRegistryAnchor, ExitError> {
    let operator_root = registry_file
        .parent()
        .ok_or_else(|| {
            ExitError::invalid_value(format!(
                "domain-pack: signed supply-chain registry '{}' has no operator root",
                registry_file.display()
            ))
        })?
        .to_path_buf();
    let lock = acquire_effect_store_lock(
        &operator_root,
        DOMAIN_PACK_REGISTRY_ANCHOR_LOCK_RELATIVE_PATH,
    )
    .map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot lock operator registry anchor below '{}': {error}",
            operator_root.display()
        ))
    })?;
    let recovery = recover_file_crash_safe_under_lock(
        &operator_root,
        &lock,
        DOMAIN_PACK_REGISTRY_ANCHOR_LOCK_RELATIVE_PATH,
        DOMAIN_PACK_REGISTRY_ANCHOR_RELATIVE_PATH,
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )
    .map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot recover operator registry anchor below '{}': {error}",
            operator_root.display()
        ))
    })?;
    Ok(LockedOperatorRegistryAnchor {
        operator_root,
        lock,
        previous_digest: recovery.target_digest,
    })
}

fn load_operator_registry_anchor(
    locked: &LockedOperatorRegistryAnchor,
) -> Result<DomainPackRegistryAnchor, ExitError> {
    let anchor_path = locked
        .operator_root
        .join(DOMAIN_PACK_REGISTRY_ANCHOR_RELATIVE_PATH);
    if locked.previous_digest.is_none() {
        return Err(ExitError::with_code(
            2,
            "domain-pack: operator registry anchor is not provisioned; obtain explicit operator approval and run 'forge-core domain-pack trust-provision' first",
        ));
    }
    let head: DomainPackRegistryAnchorHead = read_typed(&anchor_path, "operator registry anchor")?;
    if head.schema_version != DOMAIN_PACK_REGISTRY_ANCHOR_SCHEMA_VERSION {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: unsupported operator registry anchor schema '{}'",
            head.schema_version
        )));
    }
    DomainPackRegistryAnchor::from_operator_protected_head(
        head.registry_id,
        head.audience,
        head.generation,
        head.snapshot_digest,
        head.trust_policy_digest,
    )
    .map_err(|error| {
        ExitError::with_code(
            2,
            format!("domain-pack: operator registry anchor rejected: {error}"),
        )
    })
}

fn trusted_now_unix() -> Result<u64, ExitError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ExitError::failed("domain-pack: system clock is before Unix epoch"))
        .map(|duration| duration.as_secs())
}

fn advance_operator_registry_anchor(
    locked: &LockedOperatorRegistryAnchor,
    anchor: &mut DomainPackRegistryAnchor,
    verified: VerifiedDomainPackSupplyChainSnapshot,
) -> Result<AnchoredDomainPackSupplyChainSnapshot, ExitError> {
    let expected = anchor.version();
    let capability = match anchor
        .compare_and_advance(&expected, verified)
        .map_err(|error| {
            ExitError::with_code(
                2,
                format!("domain-pack: monotonic registry anchor rejected: {error}"),
            )
        })? {
        DomainPackRegistryAnchorAdvance::Advanced(capability)
        | DomainPackRegistryAnchorAdvance::Replay { capability, .. } => capability,
    };
    let verified = capability.verified_snapshot();
    let head = DomainPackRegistryAnchorHead {
        schema_version: DOMAIN_PACK_REGISTRY_ANCHOR_SCHEMA_VERSION.to_owned(),
        registry_id: verified.registry_id().clone(),
        audience: verified.audience().clone(),
        generation: verified.generation(),
        snapshot_digest: verified.snapshot_digest().to_owned(),
        trust_policy_digest: verified.trust_policy_digest().to_owned(),
    };
    let raw = yaml_serde::to_string(&head)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot serialize operator registry anchor: {error}"
            ))
        })?
        .into_bytes();
    replace_file_crash_safe_under_lock(
        &locked.operator_root,
        &locked.lock,
        DOMAIN_PACK_REGISTRY_ANCHOR_LOCK_RELATIVE_PATH,
        DOMAIN_PACK_REGISTRY_ANCHOR_RELATIVE_PATH,
        locked.previous_digest.as_deref(),
        &raw,
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )
    .map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot persist operator registry anchor below '{}': {error}",
            locked.operator_root.display()
        ))
    })?;
    Ok(capability)
}

fn parse_flags(
    args: &[String],
    mut set_value: impl FnMut(&str, &str) -> bool,
    want_json: &mut bool,
) -> Result<(), ExitError> {
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => *want_json = true,
            "--no-json" | "--text" => *want_json = false,
            "--help" | "-h" => return Err(ExitError::usage(usage())),
            flag if flag.starts_with("--") => {
                index += 1;
                let value = args
                    .get(index)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| ExitError::usage(usage()))?;
                if !set_value(flag, value) {
                    return Err(ExitError::usage(usage()));
                }
            }
            _ => return Err(ExitError::usage(usage())),
        }
        index += 1;
    }
    Ok(())
}

fn read(path: &Path, label: &str) -> Result<Vec<u8>, ExitError> {
    read_bounded(path, label, MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES as u64)
}

fn read_bounded(path: &Path, label: &str, maximum: u64) -> Result<Vec<u8>, ExitError> {
    let file = std::fs::File::open(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot read {label} '{}': {error}",
            path.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect {label} '{}': {error}",
            path.display()
        ))
    })?;
    if metadata.len() > maximum {
        return Err(document_too_large(path, label, metadata.len(), maximum));
    }

    // Metadata is only an early rejection. The capped reader also closes the
    // race where a regular file grows between metadata and the read, and it
    // bounds streams whose metadata does not expose a useful byte length.
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(usize::MAX)
            .min(usize::try_from(maximum).unwrap_or(usize::MAX)),
    );
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot read {label} '{}': {error}",
                path.display()
            ))
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(document_too_large(path, label, bytes.len() as u64, maximum));
    }
    Ok(bytes)
}

fn document_too_large(path: &Path, label: &str, observed: u64, maximum: u64) -> ExitError {
    ExitError::failed(format!(
        "domain-pack: {label} '{}' is {observed} bytes and exceeds maximum {maximum} bytes",
        path.display()
    ))
}

fn canonical_digest<T: serde::Serialize>(value: &T) -> Result<String, ExitError> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot canonicalize manifest input: {error}"
        ))
    })?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn parse<T: serde::de::DeserializeOwned>(raw: &[u8], path: &Path) -> Result<T, ExitError> {
    let text = std::str::from_utf8(raw).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: '{}' is not UTF-8: {error}",
            path.display()
        ))
    })?;
    yaml_serde::from_str(text).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: '{}' is not a closed typed document: {error}",
            path.display()
        ))
    })
}

fn read_typed<T: serde::de::DeserializeOwned>(path: &Path, label: &str) -> Result<T, ExitError> {
    let raw = read_bounded(path, label, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
    parse(&raw, path)
}

fn safe_join(root: &Path, reference: &str) -> Result<PathBuf, ExitError> {
    let reference = Path::new(reference);
    if reference.is_absolute()
        || reference.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ExitError::failed(format!(
            "domain-pack: artifact ref must remain relative to --artifact-root: {}",
            reference.display()
        )));
    }
    let canonical_root = std::fs::canonicalize(root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve --artifact-root '{}': {error}",
            root.display()
        ))
    })?;
    let candidate = root.join(reference);
    let candidate_metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect artifact '{}' below --artifact-root '{}': {error}",
            candidate.display(),
            root.display()
        ))
    })?;
    if candidate_metadata.file_type().is_symlink() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: artifact ref must name a regular file, not a symbolic link or junction: {}",
            reference.display()
        )));
    }
    let canonical_candidate = std::fs::canonicalize(&candidate).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve artifact '{}' below --artifact-root '{}': {error}",
            candidate.display(),
            root.display()
        ))
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(ExitError::failed(format!(
            "domain-pack: artifact ref escapes canonical --artifact-root: {}",
            reference.display()
        )));
    }
    if !candidate_metadata.is_file() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: artifact ref must name a regular file: {}",
            reference.display()
        )));
    }
    Ok(canonical_candidate)
}

fn usage() -> String {
    let mut output = String::from("usage:");
    for line in COMMAND_DOMAIN_PACK.usage_lines {
        output.push('\n');
        output.push_str("  ");
        output.push_str(line.trim_start());
    }
    output.push_str(
        "\n  validate, compose, and resolve are read-only candidate projections; trust-provision requires explicit operator approval and mutates only the external monotonic trust anchor; status/recover integrity-check lifecycle state and may complete an interrupted pointer replacement",
    );
    output
}
