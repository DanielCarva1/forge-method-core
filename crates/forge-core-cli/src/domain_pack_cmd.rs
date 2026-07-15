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
    CliEnvelope, DomainPackAcquisitionCatalogDocument, DomainPackAcquisitionDerivationInput,
    DomainPackAcquisitionIntentDocument, DomainPackAcquisitionPlanningInput,
    DomainPackActivePointerDocument, DomainPackArtifactBinding, DomainPackCandidateAuthority,
    DomainPackCandidateInput, DomainPackCapabilitySandboxPolicyDocument,
    DomainPackCompatibilityStatus, DomainPackCompositionIssue,
    DomainPackCompositionRequestDocument, DomainPackCompositionStatus, DomainPackContentDocument,
    DomainPackCoreBinding, DomainPackDiscoveryGap, DomainPackDiscoveryMatch,
    DomainPackDiscoveryProjectionDocument, DomainPackDiscoveryRequestDocument, DomainPackExactLock,
    DomainPackExactLockDocument, DomainPackExactLockPayload, DomainPackExpectedLifecycleState,
    DomainPackLifecycleLedgerRecord, DomainPackLifecycleOperation, DomainPackLifecyclePreflight,
    DomainPackLifecyclePreflightDocument, DomainPackLifecyclePreflightStatus,
    DomainPackLifecycleReceiptDocument, DomainPackLifecycleRequest,
    DomainPackLifecycleRequestDocument, DomainPackLockedPackage, DomainPackManifestDocument,
    DomainPackRebasePlanDocument, DomainPackResolutionRequestDocument, DomainPackResolutionStatus,
    DomainPackRuntimeCapabilityRegistryDocument, DomainPackSemanticAssurance,
    DomainPackSourceAssurance, DomainPackSupplyChainAssessment,
    DomainPackSupplyChainRegistryDocument, DomainPackTrustPolicyDocument,
    DurableAssuranceEpochBinding, RepoPath, StableId, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
};
use forge_core_decisions::{
    compose_domain_packs, derive_domain_pack_acquisition_inputs, discover_domain_packs,
    domain_pack_resolution_projection_digest, evaluate_domain_pack_compatibility,
    evaluate_domain_pack_trust, join_reviewed_registry_to_resolution, plan_domain_pack_acquisition,
    resolve_domain_packs, validate_domain_pack_candidate, verify_domain_pack_discovery_projection,
    verify_domain_pack_rebase_plan, DomainPackCandidateMaterial, DomainPackCompatibilityInput,
    DomainPackReviewedResolutionJoinStatus, DomainPackTrustEvaluationInput,
    DomainPackTrustEvaluationStatus, DomainPackTrustSelectedPackage,
    MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES,
};
use forge_core_domain_pack_tcb::{
    authorize_prepared_domain_pack_lifecycle, derive_domain_pack_capability_demands,
    domain_pack_project_snapshot_digest, lock_domain_pack_lifecycle,
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
struct DomainPackDiscoveryExplanationPayload {
    request_id: StableId,
    demand_digest: String,
    requirement_ref: StableId,
    authority: DomainPackCandidateAuthority,
    assurance_binding: DurableAssuranceEpochBinding,
    uncertainties: Vec<String>,
    matches: Vec<DomainPackDiscoveryMatch>,
    gaps: Vec<DomainPackDiscoveryGap>,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackOperatorSourceBinding {
    schema_version: String,
    operator_root: String,
    trust_policy_file: String,
    registry_file: String,
    reviewer_registry_file: String,
    reviewed_registry_file: String,
    capability_registry_file: String,
    sandbox_policy_file: String,
    artifact_root: String,
}

struct LockedOperatorRegistryAnchor {
    operator_root: PathBuf,
    lock: EffectStoreLock,
    previous_digest: Option<String>,
}

const DOMAIN_PACK_REGISTRY_ANCHOR_SCHEMA_VERSION: &str = "forge-domain-pack-registry-anchor-v1";
const DOMAIN_PACK_OPERATOR_SOURCE_SCHEMA_VERSION: &str = "forge-domain-pack-operator-sources-v1";
const DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH: &str = "domain-packs/operator-sources.yaml";
const DOMAIN_PACK_OPERATOR_SOURCE_LOCK_RELATIVE_PATH: &str =
    "locks/domain-packs.operator-sources.lock";
const DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH: &str = "domain-packs/rebase-plan.yaml";
const DOMAIN_PACK_REBASE_PLAN_LOCK_RELATIVE_PATH: &str = "locks/domain-packs.rebase-plan.lock";
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
        "search" => run_search(&args[2..]),
        "explain" => run_explain(&args[2..]),
        "acquire" => run_acquire(&args[2..]),
        "compose" => run_compose(&args[2..]),
        "resolve" => run_resolve(&args[2..]),
        "learning" => crate::domain_pack_learning_cmd::run_domain_pack_learning_command(&args[2..]),
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

fn run_search(args: &[String]) -> Result<(), ExitError> {
    let mut request_file: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--request-file" => set_path(&mut request_file, value),
            _ => false,
        },
        &mut want_json,
    )?;
    let request_file = request_file.ok_or_else(|| ExitError::usage(usage()))?;
    let raw = read_bounded(
        &request_file,
        "domain discovery request",
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )?;
    let request: DomainPackDiscoveryRequestDocument = parse(&raw, &request_file)?;
    let projection = discover_domain_packs(&request).map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown discovery validation error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: discovery request rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack search", projection), want_json)
}

fn run_explain(args: &[String]) -> Result<(), ExitError> {
    let mut projection_file: Option<PathBuf> = None;
    let mut requirement_ref: Option<StableId> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--projection-file" => set_path(&mut projection_file, value),
            "--requirement-ref" => {
                requirement_ref = Some(StableId(value.to_owned()));
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let projection_file = projection_file.ok_or_else(|| ExitError::usage(usage()))?;
    let requirement_ref = requirement_ref.ok_or_else(|| ExitError::usage(usage()))?;
    let raw = read_bounded(
        &projection_file,
        "domain discovery projection",
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )?;
    let document: DomainPackDiscoveryProjectionDocument = parse(&raw, &projection_file)?;
    if !verify_domain_pack_discovery_projection(&document) {
        return Err(ExitError::invalid_value(
            "domain-pack: discovery projection digest or authority is invalid",
        ));
    }
    let projection = document.domain_pack_discovery_projection;
    let matches = projection
        .matches
        .into_iter()
        .filter(|candidate| candidate.requirement_ref == requirement_ref)
        .collect::<Vec<_>>();
    let gaps = projection
        .gaps
        .into_iter()
        .filter(|gap| gap.requirement_ref == requirement_ref)
        .collect::<Vec<_>>();
    if matches.is_empty() && gaps.is_empty() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: requirement '{}' is absent from the discovery projection",
            requirement_ref.0
        )));
    }
    crate::cli_util::emit_envelope(
        CliEnvelope::ok(
            "domain-pack explain",
            DomainPackDiscoveryExplanationPayload {
                request_id: projection.request_id,
                demand_digest: projection.demand_digest,
                requirement_ref,
                authority: DomainPackCandidateAuthority::CandidateOnly,
                assurance_binding: projection.assurance_binding,
                uncertainties: projection.uncertainties,
                matches,
                gaps,
                boundary: "explanation of an integrity-checked candidate projection; no trust, selection, installation, or activation authority",
            },
        ),
        want_json,
    )
}

fn run_acquire(args: &[String]) -> Result<(), ExitError> {
    match args.first().map_or("--help", String::as_str) {
        "plan" => run_acquisition_plan(&args[1..]),
        "prepare" => run_acquisition_prepare(&args[1..]),
        "apply" => run_acquisition_apply(&args[1..]),
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core domain-pack acquire: unknown subcommand '{other}'\n{}",
            usage()
        ))),
    }
}

fn run_acquisition_plan(args: &[String]) -> Result<(), ExitError> {
    let mut intent_file: Option<PathBuf> = None;
    let mut request_file: Option<PathBuf> = None;
    let mut projection_file: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--intent-file" => set_path(&mut intent_file, value),
            "--request-file" => set_path(&mut request_file, value),
            "--projection-file" => set_path(&mut projection_file, value),
            _ => false,
        },
        &mut want_json,
    )?;
    let intent_file = intent_file.ok_or_else(|| ExitError::usage(usage()))?;
    let request_file = request_file.ok_or_else(|| ExitError::usage(usage()))?;
    let projection_file = projection_file.ok_or_else(|| ExitError::usage(usage()))?;
    let intent: DomainPackAcquisitionIntentDocument =
        read_typed(&intent_file, "acquisition intent")?;
    let request: DomainPackDiscoveryRequestDocument =
        read_typed(&request_file, "domain discovery request")?;
    let discovery: DomainPackDiscoveryProjectionDocument =
        read_typed(&projection_file, "domain discovery projection")?;
    let plan = plan_domain_pack_acquisition(&DomainPackAcquisitionPlanningInput {
        intent,
        request,
        discovery,
    })
    .map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown acquisition planning error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: acquisition planning rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack acquire plan", plan), want_json)
}

fn run_acquisition_prepare(args: &[String]) -> Result<(), ExitError> {
    let mut intent_file: Option<PathBuf> = None;
    let mut request_file: Option<PathBuf> = None;
    let mut projection_file: Option<PathBuf> = None;
    let mut catalog_file: Option<PathBuf> = None;
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--intent-file" => set_path(&mut intent_file, value),
            "--request-file" => set_path(&mut request_file, value),
            "--projection-file" => set_path(&mut projection_file, value),
            "--catalog-file" => set_path(&mut catalog_file, value),
            _ => false,
        },
        &mut want_json,
    )?;
    let planning_input = DomainPackAcquisitionPlanningInput {
        intent: read_typed(&required(intent_file)?, "acquisition intent")?,
        request: read_typed(&required(request_file)?, "domain discovery request")?,
        discovery: read_typed(&required(projection_file)?, "domain discovery projection")?,
    };
    let catalog: DomainPackAcquisitionCatalogDocument =
        read_typed(&required(catalog_file)?, "acquisition catalog")?;
    if catalog.schema_version != forge_core_contracts::DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION {
        return Err(ExitError::invalid_value(
            "domain-pack: acquisition catalog has unsupported schema",
        ));
    }
    let plan = plan_domain_pack_acquisition(&planning_input).map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown acquisition planning error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: acquisition preparation rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    let input = DomainPackAcquisitionDerivationInput {
        planning_input,
        plan,
        forge_core_version: catalog.forge_core_version,
        core: catalog.core,
        registry: catalog.registry,
        candidates: catalog.candidates,
    };
    derive_domain_pack_acquisition_inputs(&input).map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown acquisition derivation error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: acquisition catalog material rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack acquire prepare", input),
        want_json,
    )
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn run_acquisition_apply(args: &[String]) -> Result<(), ExitError> {
    let mut derivation_input_file: Option<PathBuf> = None;
    let mut approved_candidate: Option<String> = None;
    let mut trust_policy_file: Option<PathBuf> = None;
    let mut registry_file: Option<PathBuf> = None;
    let mut reviewer_registry_file: Option<PathBuf> = None;
    let mut reviewed_registry_file: Option<PathBuf> = None;
    let mut capability_registry_file: Option<PathBuf> = None;
    let mut sandbox_policy_file: Option<PathBuf> = None;
    let mut principal_id: Option<StableId> = None;
    let mut project_root: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut state_root = PathBuf::from(".forge-method");
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--derivation-input-file" => set_path(&mut derivation_input_file, value),
            "--operator-approve-candidate" => {
                approved_candidate = Some(value.to_owned());
                true
            }
            "--trust-policy-file" => set_path(&mut trust_policy_file, value),
            "--registry-file" => set_path(&mut registry_file, value),
            "--reviewer-registry-file" => set_path(&mut reviewer_registry_file, value),
            "--reviewed-registry-file" => set_path(&mut reviewed_registry_file, value),
            "--capability-registry-file" => set_path(&mut capability_registry_file, value),
            "--sandbox-policy-file" => set_path(&mut sandbox_policy_file, value),
            "--principal-id" => {
                principal_id = Some(StableId(value.to_owned()));
                true
            }
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
    let derivation_input_file = required(derivation_input_file)?;
    let approved_candidate = approved_candidate.ok_or_else(|| ExitError::usage(usage()))?;
    let trust_policy_file = required(trust_policy_file)?;
    let registry_file = required(registry_file)?;
    let reviewer_registry_file = required(reviewer_registry_file)?;
    let reviewed_registry_file = required(reviewed_registry_file)?;
    let capability_registry_file = required(capability_registry_file)?;
    let sandbox_policy_file = required(sandbox_policy_file)?;
    let principal_id = principal_id.ok_or_else(|| ExitError::usage(usage()))?;
    let project_root = required(project_root)?;

    let input: DomainPackAcquisitionDerivationInput =
        read_typed(&derivation_input_file, "acquisition derivation input")?;
    let plan = &input.plan.domain_pack_acquisition_plan;
    if approved_candidate != plan.selected.candidate_id.0 {
        return Err(ExitError::invalid_value(
            "domain-pack: explicit operator approval does not name the exact planned candidate id",
        ));
    }
    let derived = derive_domain_pack_acquisition_inputs(&input).map_err(|rejection| {
        let first = rejection.issues.first().map_or_else(
            || "unknown acquisition derivation error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        ExitError::invalid_value(format!(
            "domain-pack: acquisition derivation rejected with {} issue(s); first: {first}",
            rejection.issues.len()
        ))
    })?;
    let derived = derived.domain_pack_acquisition_derived_inputs;

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
    let reviewer_registry_file = trusted_external_file(
        &reviewer_registry_file,
        "signed reviewer registry",
        &controlled_roots,
    )?;
    let reviewed_registry_file = trusted_external_file(
        &reviewed_registry_file,
        "dual-signed reviewed registry",
        &controlled_roots,
    )?;
    let capability_registry_file = trusted_external_file(
        &capability_registry_file,
        "runtime capability registry",
        &controlled_roots,
    )?;
    let sandbox_policy_file = trusted_external_file(
        &sandbox_policy_file,
        "capability sandbox policy",
        &controlled_roots,
    )?;
    let trust_policy: DomainPackTrustPolicyDocument =
        read_typed(&trust_policy_file, "operator trust policy")?;
    let registry: DomainPackSupplyChainRegistryDocument =
        read_typed(&registry_file, "signed supply-chain registry")?;
    if registry != input.registry {
        return Err(ExitError::invalid_value(
            "domain-pack: operator registry differs from the exact acquisition derivation input",
        ));
    }
    let capability_registry: DomainPackRuntimeCapabilityRegistryDocument =
        read_typed(&capability_registry_file, "runtime capability registry")?;
    let sandbox_policy: DomainPackCapabilitySandboxPolicyDocument =
        read_typed(&sandbox_policy_file, "capability sandbox policy")?;
    let project_snapshot_digest = domain_pack_project_snapshot_digest(&controlled_roots.project)
        .map_err(map_lifecycle_error)?;
    if project_snapshot_digest != plan.expected_project_snapshot_digest {
        return Err(ExitError::conflict(
            "domain-pack: project snapshot changed after candidate acquisition planning",
        ));
    }

    let operator_anchor = lock_operator_registry_anchor(&registry_file)?;
    for (path, label) in [
        (&reviewer_registry_file, "signed reviewer registry"),
        (&reviewed_registry_file, "dual-signed reviewed registry"),
        (&capability_registry_file, "runtime capability registry"),
        (&sandbox_policy_file, "capability sandbox policy"),
    ] {
        require_direct_operator_file(path, &operator_anchor.operator_root, label)?;
    }
    let now_unix = trusted_now_unix()?;
    let verified_snapshot =
        verify_domain_pack_supply_chain_snapshot(&trust_policy, &registry, now_unix).map_err(
            |error| {
                ExitError::with_code(
                    2,
                    format!("domain-pack: supply-chain verification rejected: {error}"),
                )
            },
        )?;
    let mut registry_anchor = load_operator_registry_anchor(&operator_anchor)?;
    let anchored_snapshot = advance_operator_registry_anchor(
        &operator_anchor,
        &mut registry_anchor,
        verified_snapshot,
    )?;
    let reviewed_guard = crate::domain_pack_learning_cmd::lock_reviewed_snapshot_for_lifecycle(
        &operator_anchor.operator_root,
        &reviewer_registry_file,
        &reviewed_registry_file,
        now_unix,
    )?;
    persist_domain_pack_operator_sources(
        &state_root,
        &DomainPackOperatorSourceBinding {
            schema_version: DOMAIN_PACK_OPERATOR_SOURCE_SCHEMA_VERSION.to_owned(),
            operator_root: normalized_path(&operator_anchor.operator_root),
            trust_policy_file: normalized_path(&trust_policy_file),
            registry_file: normalized_path(&registry_file),
            reviewer_registry_file: normalized_path(&reviewer_registry_file),
            reviewed_registry_file: normalized_path(&reviewed_registry_file),
            capability_registry_file: normalized_path(&capability_registry_file),
            sandbox_policy_file: normalized_path(&sandbox_policy_file),
            artifact_root: normalized_path(&controlled_roots.artifacts),
        },
    )?;

    let resolution_request = derived.resolution_request;
    let composition_request = derived.composition_request;
    let mut resolution = resolve_domain_packs(&resolution_request, &registry);
    if resolution.domain_pack_resolution_projection.status != DomainPackResolutionStatus::Resolved
        || !resolution
            .domain_pack_resolution_projection
            .issues
            .is_empty()
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: exact acquisition resolution is no longer clean",
        ));
    }
    let reviewed_join =
        join_reviewed_registry_to_resolution(&resolution, reviewed_guard.snapshot().registry());
    if reviewed_join.reviewed_registry_digest != reviewed_guard.snapshot().registry_digest() {
        return Err(ExitError::with_code(
            2,
            "domain-pack: reviewed-registry join differs from the anchored reviewed snapshot",
        ));
    }
    let mut assessments = Vec::new();
    for selected in &mut resolution.domain_pack_resolution_projection.selected {
        let record = anchored_snapshot
            .verified_snapshot()
            .entries()
            .iter()
            .map(forge_core_authority::VerifiedDomainPackSupplyChainEntry::record)
            .find(|record| {
                record.record_digest == selected.registry_record_digest
                    && record.identity == selected.identity
                    && record.package_digest == selected.package.package_digest
                    && record.manifest_digest == selected.package.manifest.canonical_sha256
                    && record.content_digest == selected.package.content.canonical_sha256
                    && record.license_digest == selected.package.license.canonical_sha256
                    && record.fixture_digests
                        == selected
                            .package
                            .fixtures
                            .iter()
                            .map(|fixture| fixture.canonical_sha256.clone())
                            .collect::<Vec<_>>()
            })
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: selected package is absent from the verified supply-chain snapshot",
                )
            })?;
        let join = reviewed_join
            .joins
            .iter()
            .find(|join| {
                join.publisher == selected.identity.publisher.0
                    && join.name == selected.identity.name.0
                    && join.version == selected.identity.version
                    && join.package_digest == selected.package.package_digest
                    && join.registry_record_digest == selected.registry_record_digest
            })
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: selected package is absent from the reviewed-registry join",
                )
            })?;
        if join.status != DomainPackReviewedResolutionJoinStatus::EligibleReviewed {
            return Err(ExitError::with_code(
                2,
                "domain-pack: selected package is not currently eligible and reviewed",
            ));
        }
        let entry_digest = join.reviewed_entry_digest.as_ref().ok_or_else(|| {
            ExitError::with_code(2, "domain-pack: eligible reviewed join lacks entry digest")
        })?;
        let entry = reviewed_guard
            .snapshot()
            .registry()
            .domain_pack_reviewed_registry
            .entries
            .iter()
            .find(|entry| &entry.entry_digest == entry_digest)
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: reviewed entry is absent from anchored registry",
                )
            })?;
        selected.source_assurance = DomainPackSourceAssurance::SupplyChainVerified;
        selected.semantic_assurance = DomainPackSemanticAssurance::Reviewed;
        selected.reviewed_entry_digest = Some(entry.entry_digest.clone());
        selected.promotion_authorization_digest = Some(entry.authorization_digest.clone());
        assessments.push(DomainPackSupplyChainAssessment {
            package_digest: record.package_digest.clone(),
            registry_record_digest: record.record_digest.clone(),
            publisher_signature_verified: true,
            registry_signature_threshold_verified: true,
            namespace_grant_verified: true,
            revoked: false,
        });
    }
    let promoted = &mut resolution.domain_pack_resolution_projection;
    promoted.resolution_digest = domain_pack_resolution_projection_digest(
        &resolution_request,
        anchored_snapshot.verified_snapshot().snapshot_digest(),
        promoted,
    );

    let owned_materials = load_composition_materials(&composition_request, &artifact_root)?;
    let materials = material_views(&composition_request, &owned_materials);
    let composition = compose_domain_packs(&composition_request, &materials);
    if !composition
        .domain_pack_composition_projection
        .issues
        .is_empty()
        || !composition
            .domain_pack_composition_projection
            .gaps
            .is_empty()
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: exact acquisition composition is not clean and composable",
        ));
    }
    let selected_for_trust = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .zip(&assessments)
        .map(|(package, assessment)| {
            Ok(DomainPackTrustSelectedPackage {
                package: package.clone(),
                structurally_valid: true,
                supply_chain: assessment.clone(),
                capability_demands: derive_domain_pack_capability_demands(
                    package,
                    &composition_request.domain_pack_composition_request,
                )
                .map_err(map_lifecycle_error)?,
            })
        })
        .collect::<Result<Vec<_>, ExitError>>()?;
    let trust_input = DomainPackTrustEvaluationInput {
        project_id: resolution_request
            .domain_pack_resolution_request
            .project_id
            .clone(),
        selected: selected_for_trust,
        trust_policy: trust_policy.domain_pack_trust_policy.clone(),
        capability_registry: capability_registry
            .domain_pack_runtime_capability_registry
            .clone(),
        sandbox_policy: sandbox_policy.domain_pack_capability_sandbox_policy.clone(),
    };
    let trust = evaluate_domain_pack_trust(&trust_input);
    if trust.status != DomainPackTrustEvaluationStatus::Approved || !trust.issues.is_empty() {
        return Err(ExitError::with_code(
            2,
            format!(
                "domain-pack: trust/capability evaluation blocked: {:?}",
                trust.issues
            ),
        ));
    }

    let lock_payload = DomainPackExactLockPayload {
        project_id: resolution_request
            .domain_pack_resolution_request
            .project_id
            .clone(),
        core: resolution_request
            .domain_pack_resolution_request
            .core
            .clone(),
        requirements_digest: canonical_digest(
            &resolution_request
                .domain_pack_resolution_request
                .requirements,
        )?,
        roots: resolution_request
            .domain_pack_resolution_request
            .roots
            .clone(),
        registry_snapshot_digest: anchored_snapshot
            .verified_snapshot()
            .snapshot_digest()
            .to_owned(),
        reviewer_registry_digest: reviewed_guard
            .snapshot()
            .reviewer_registry_digest()
            .to_owned(),
        reviewed_registry_digest: reviewed_guard.snapshot().registry_digest().to_owned(),
        trust_policy_digest: anchored_snapshot
            .verified_snapshot()
            .trust_policy_digest()
            .to_owned(),
        capability_registry_digest: canonical_digest(&capability_registry)?,
        sandbox_policy_digest: canonical_digest(&sandbox_policy)?,
        resolution_digest: resolution
            .domain_pack_resolution_projection
            .resolution_digest
            .clone(),
        composition_digest: composition
            .domain_pack_composition_projection
            .composition_digest
            .clone(),
        packages: resolution
            .domain_pack_resolution_projection
            .selected
            .iter()
            .map(locked_package_from_resolution)
            .collect(),
        verified_capability_bindings: trust.verified_capability_bindings,
        unresolved_composition_gaps: composition.domain_pack_composition_projection.gaps.clone(),
        unresolved_capability_gaps: trust.capability_gaps.clone(),
    };
    let proposed_lock = DomainPackExactLockDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_exact_lock: DomainPackExactLock {
            lock_digest: canonical_digest(&lock_payload)?,
            payload: lock_payload,
        },
    };
    let mut lifecycle = lock_domain_pack_lifecycle(&state_root).map_err(map_lifecycle_error)?;
    if lifecycle.projection().active_pointer.is_some() {
        return Err(ExitError::conflict(
            "domain-pack: acquire apply is clean-install-only; use intent-specific lifecycle upgrade, rollback, or remove for initialized state",
        ));
    }
    let expected_state = DomainPackExpectedLifecycleState::Uninitialized {
        project_snapshot_digest: project_snapshot_digest.clone(),
    };
    let operation = DomainPackLifecycleOperation::Install {
        root: resolution_request.domain_pack_resolution_request.roots[0]
            .pack
            .clone(),
    };
    let lifecycle_request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: StableId(format!("{}.lifecycle", plan.acquisition_id.0)),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: resolution_request
                .domain_pack_resolution_request
                .project_id
                .clone(),
            principal_id,
            operation: operation.clone(),
            expected_state: expected_state.clone(),
            resolution_request_digest: canonical_digest(&resolution_request)?,
            project_snapshot_digest: project_snapshot_digest.clone(),
        },
    };
    let compatibility = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: StableId(format!("{}.compatibility", plan.acquisition_id.0)),
        operation,
        sealed_core: resolution_request
            .domain_pack_resolution_request
            .core
            .clone(),
        from_lock: None,
        to_lock: proposed_lock.clone(),
    });
    if compatibility.domain_pack_compatibility_report.status
        != DomainPackCompatibilityStatus::Compatible
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: acquisition compatibility evaluation blocked",
        ));
    }
    let mut staged_artifacts = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .flat_map(|package| {
            let content = DomainPackArtifactBinding {
                artifact_ref: package.package.content.content_ref.clone(),
                raw_sha256: package.package.content.raw_sha256.clone(),
                canonical_sha256: package.package.content.canonical_sha256.clone(),
            };
            std::iter::once(package.package.manifest.clone())
                .chain(std::iter::once(content))
                .chain(std::iter::once(package.package.license.clone()))
                .chain(package.package.fixtures.iter().cloned())
        })
        .collect::<Vec<_>>();
    staged_artifacts.sort_by(|left, right| left.artifact_ref.0.cmp(&right.artifact_ref.0));
    staged_artifacts.dedup();
    let mut preflight = DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: StableId(format!("{}.preflight", plan.acquisition_id.0)),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request_digest: canonical_digest(&lifecycle_request)?,
            request: lifecycle_request,
            observed_state: expected_state,
            resolution,
            proposed_lock,
            composition,
            supply_chain_assessments: assessments,
            trust_decisions: trust.trust_decisions,
            capability_gaps: trust.capability_gaps,
            compatibility_report: compatibility,
            staged_artifacts,
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: vec![],
            preflight_digest: String::new(),
        },
    };
    preflight.domain_pack_lifecycle_preflight.preflight_digest = canonical_digest(&preflight)?;
    let owned_artifacts = load_immutable_artifacts(&preflight, &artifact_root)?;
    let artifacts = immutable_artifact_views(&owned_artifacts);
    let project_snapshot =
        verify_domain_pack_project_snapshot(&controlled_roots.project, &project_snapshot_digest)
            .map_err(map_lifecycle_error)?;
    let prepared = lifecycle
        .prepare_candidate(preflight)
        .map_err(map_lifecycle_error)?;
    let context = DomainPackLifecycleAuthorizationContext {
        anchored_snapshot: &anchored_snapshot,
        anchored_reviewed_snapshot: reviewed_guard.snapshot(),
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
    let receipt = lifecycle
        .commit(prepared, authority)
        .map_err(map_lifecycle_error)?;
    drop(reviewed_guard);
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack acquire apply", receipt),
        want_json,
    )
}

fn locked_package_from_resolution(
    selected: &forge_core_contracts::DomainPackResolvedPackage,
) -> DomainPackLockedPackage {
    DomainPackLockedPackage {
        identity: selected.identity.clone(),
        package_digest: selected.package.package_digest.clone(),
        manifest_binding: selected.package.manifest.clone(),
        content_binding: selected.package.content.clone(),
        license_binding: selected.package.license.clone(),
        fixture_bindings: selected.package.fixtures.clone(),
        namespace_grant_id: selected.namespace_grant_id.clone(),
        registry_record_digest: selected.registry_record_digest.clone(),
        source_assurance: selected.source_assurance,
        semantic_assurance: selected.semantic_assurance,
        reviewed_entry_digest: selected.reviewed_entry_digest.clone(),
        promotion_authorization_digest: selected.promotion_authorization_digest.clone(),
        dependencies: selected.dependencies.clone(),
        deterministic_order: selected.deterministic_order,
    }
}

/// Commit one exact target-Core lifecycle generation from operator roots that
/// were bound during the original high-level acquisition. Workflow authority
/// is advanced separately by the joined ledger event after this returns.
#[allow(clippy::similar_names)] // Reviewer identity and reviewed-pack authority are distinct registries.
pub(crate) fn apply_domain_pack_core_rebase(
    project_root: &Path,
    state_root: &Path,
    plan: &DomainPackRebasePlanDocument,
    target_core: &DomainPackCoreBinding,
    principal_id: StableId,
) -> Result<DomainPackLifecycleReceiptDocument, ExitError> {
    if !verify_domain_pack_rebase_plan(plan) {
        return Err(ExitError::conflict(
            "domain-pack: rebase plan failed integrity verification",
        ));
    }
    let plan = &plan.domain_pack_rebase_plan;
    if &plan.target_core != target_core {
        return Err(ExitError::invalid_value(
            "domain-pack: rebase plan is not an apply-authorized target-Core plan",
        ));
    }
    let sources = load_domain_pack_operator_sources(state_root)?;
    let artifact_root = PathBuf::from(&sources.artifact_root);
    let controlled_roots = canonical_lifecycle_roots(project_root, &artifact_root, state_root)?;
    let trust_policy_file = trusted_external_file(
        Path::new(&sources.trust_policy_file),
        "operator trust policy",
        &controlled_roots,
    )?;
    let registry_file = trusted_external_file(
        Path::new(&sources.registry_file),
        "signed supply-chain registry",
        &controlled_roots,
    )?;
    let reviewer_registry_file = trusted_external_file(
        Path::new(&sources.reviewer_registry_file),
        "signed reviewer registry",
        &controlled_roots,
    )?;
    let reviewed_registry_file = trusted_external_file(
        Path::new(&sources.reviewed_registry_file),
        "dual-signed reviewed registry",
        &controlled_roots,
    )?;
    let capability_registry_file = trusted_external_file(
        Path::new(&sources.capability_registry_file),
        "runtime capability registry",
        &controlled_roots,
    )?;
    let sandbox_policy_file = trusted_external_file(
        Path::new(&sources.sandbox_policy_file),
        "capability sandbox policy",
        &controlled_roots,
    )?;
    let operator_root = std::fs::canonicalize(&sources.operator_root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve persisted operator root: {error}"
        ))
    })?;
    if normalized_path(&operator_root) != sources.operator_root {
        return Err(ExitError::invalid_value(
            "domain-pack: persisted operator root is no longer canonical",
        ));
    }
    for (path, label) in [
        (&trust_policy_file, "operator trust policy"),
        (&registry_file, "signed supply-chain registry"),
        (&reviewer_registry_file, "signed reviewer registry"),
        (&reviewed_registry_file, "dual-signed reviewed registry"),
        (&capability_registry_file, "runtime capability registry"),
        (&sandbox_policy_file, "capability sandbox policy"),
    ] {
        require_direct_operator_file(path, &operator_root, label)?;
    }
    let project_snapshot_digest = domain_pack_project_snapshot_digest(&controlled_roots.project)
        .map_err(map_lifecycle_error)?;
    if project_snapshot_digest != plan.exact_cas.expected_project_snapshot_digest {
        return Err(ExitError::conflict(
            "domain-pack: project snapshot changed after rebase planning",
        ));
    }

    let trust_policy: DomainPackTrustPolicyDocument =
        read_typed(&trust_policy_file, "operator trust policy")?;
    let registry: DomainPackSupplyChainRegistryDocument =
        read_typed(&registry_file, "signed supply-chain registry")?;
    let capability_registry: DomainPackRuntimeCapabilityRegistryDocument =
        read_typed(&capability_registry_file, "runtime capability registry")?;
    let sandbox_policy: DomainPackCapabilitySandboxPolicyDocument =
        read_typed(&sandbox_policy_file, "capability sandbox policy")?;
    let operator_anchor = lock_operator_registry_anchor(&registry_file)?;
    let now_unix = trusted_now_unix()?;
    let verified_snapshot =
        verify_domain_pack_supply_chain_snapshot(&trust_policy, &registry, now_unix).map_err(
            |error| {
                ExitError::with_code(
                    2,
                    format!("domain-pack: supply-chain verification rejected: {error}"),
                )
            },
        )?;
    if verified_snapshot.snapshot_digest() != plan.exact_cas.expected_supply_chain_registry_digest {
        return Err(ExitError::conflict(
            "domain-pack: supply-chain registry changed after rebase planning",
        ));
    }
    let mut registry_anchor = load_operator_registry_anchor(&operator_anchor)?;
    let anchored_snapshot = advance_operator_registry_anchor(
        &operator_anchor,
        &mut registry_anchor,
        verified_snapshot,
    )?;
    let reviewed_guard = crate::domain_pack_learning_cmd::lock_reviewed_snapshot_for_lifecycle(
        &operator_root,
        &reviewer_registry_file,
        &reviewed_registry_file,
        now_unix,
    )?;
    if reviewed_guard.snapshot().reviewer_registry_digest()
        != plan.exact_cas.expected_reviewer_registry_digest
        || reviewed_guard.snapshot().registry_digest()
            != plan.exact_cas.expected_reviewed_registry_digest
    {
        return Err(ExitError::conflict(
            "domain-pack: reviewer or reviewed registry changed after rebase planning",
        ));
    }

    persist_domain_pack_rebase_plan(
        state_root,
        &DomainPackRebasePlanDocument {
            schema_version: forge_core_contracts::DOMAIN_PACK_REBASE_SCHEMA_VERSION.to_owned(),
            domain_pack_rebase_plan: plan.clone(),
        },
    )?;
    let mut lifecycle = lock_domain_pack_lifecycle(state_root).map_err(map_lifecycle_error)?;
    let source = lifecycle
        .active_rebase_source()
        .map_err(map_lifecycle_error)?;
    let source_pointer = &source.pointer.domain_pack_active_pointer;
    if source_pointer.generation != plan.exact_cas.expected_generation
        || source_pointer.pointer_digest != plan.exact_cas.expected_lifecycle_pointer_digest
        || source_pointer.lifecycle_head_digest != plan.exact_cas.expected_lifecycle_head_digest
        || source_pointer.active_lock_digest != plan.exact_cas.expected_active_lock_digest
        || source.exact_lock.domain_pack_exact_lock.payload.core != plan.source_core
    {
        return Err(ExitError::conflict(
            "domain-pack: active lifecycle generation changed after rebase planning",
        ));
    }
    let source_lock = &source.exact_lock.domain_pack_exact_lock.payload;
    if source_lock.registry_snapshot_digest != plan.exact_cas.expected_supply_chain_registry_digest
        || source_lock.reviewer_registry_digest != plan.exact_cas.expected_reviewer_registry_digest
        || source_lock.reviewed_registry_digest != plan.exact_cas.expected_reviewed_registry_digest
        || canonical_digest(&trust_policy)? != source_lock.trust_policy_digest
        || canonical_digest(&capability_registry)? != source_lock.capability_registry_digest
        || canonical_digest(&sandbox_policy)? != source_lock.sandbox_policy_digest
    {
        return Err(ExitError::conflict(
            "domain-pack: fresh operator policy no longer matches the active exact lock",
        ));
    }

    let mut resolution_request = source.resolution_request.clone();
    resolution_request.domain_pack_resolution_request.request_id = StableId(format!(
        "domain-pack.rebase.resolution.{}",
        source_pointer.generation + 1
    ));
    resolution_request.domain_pack_resolution_request.core = target_core.clone();
    resolution_request
        .domain_pack_resolution_request
        .roots
        .clone_from(&source_lock.roots);
    resolution_request
        .domain_pack_resolution_request
        .current_lock = None;
    resolution_request
        .domain_pack_resolution_request
        .registry_snapshot_digest
        .clone_from(&source_lock.registry_snapshot_digest);
    let mut resolution = resolve_domain_packs(&resolution_request, &registry);
    if resolution.domain_pack_resolution_projection.status != DomainPackResolutionStatus::Resolved
        || !resolution
            .domain_pack_resolution_projection
            .issues
            .is_empty()
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: target-Core re-resolution is blocked",
        ));
    }
    let reviewed_join =
        join_reviewed_registry_to_resolution(&resolution, reviewed_guard.snapshot().registry());
    let mut assessments = Vec::new();
    for selected in &mut resolution.domain_pack_resolution_projection.selected {
        let record = anchored_snapshot
            .verified_snapshot()
            .entries()
            .iter()
            .map(forge_core_authority::VerifiedDomainPackSupplyChainEntry::record)
            .find(|record| {
                record.record_digest == selected.registry_record_digest
                    && record.identity == selected.identity
                    && record.package_digest == selected.package.package_digest
                    && record.manifest_digest == selected.package.manifest.canonical_sha256
                    && record.content_digest == selected.package.content.canonical_sha256
                    && record.license_digest == selected.package.license.canonical_sha256
                    && record.fixture_digests
                        == selected
                            .package
                            .fixtures
                            .iter()
                            .map(|fixture| fixture.canonical_sha256.clone())
                            .collect::<Vec<_>>()
            })
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: target-Core package is absent from verified supply chain",
                )
            })?;
        let join = reviewed_join
            .joins
            .iter()
            .find(|join| join.registry_record_digest == selected.registry_record_digest)
            .ok_or_else(|| {
                ExitError::with_code(
                    2,
                    "domain-pack: target-Core package is absent from reviewed join",
                )
            })?;
        if join.status != DomainPackReviewedResolutionJoinStatus::EligibleReviewed {
            return Err(ExitError::with_code(
                2,
                "domain-pack: target-Core package is not eligible-reviewed",
            ));
        }
        let entry = reviewed_guard
            .snapshot()
            .registry()
            .domain_pack_reviewed_registry
            .entries
            .iter()
            .find(|entry| join.reviewed_entry_digest.as_ref() == Some(&entry.entry_digest))
            .ok_or_else(|| {
                ExitError::with_code(2, "domain-pack: reviewed entry is absent from anchor")
            })?;
        selected.source_assurance = DomainPackSourceAssurance::SupplyChainVerified;
        selected.semantic_assurance = DomainPackSemanticAssurance::Reviewed;
        selected.reviewed_entry_digest = Some(entry.entry_digest.clone());
        selected.promotion_authorization_digest = Some(entry.authorization_digest.clone());
        assessments.push(DomainPackSupplyChainAssessment {
            package_digest: record.package_digest.clone(),
            registry_record_digest: record.record_digest.clone(),
            publisher_signature_verified: true,
            registry_signature_threshold_verified: true,
            namespace_grant_verified: true,
            revoked: false,
        });
    }
    let promoted = &mut resolution.domain_pack_resolution_projection;
    promoted.resolution_digest = domain_pack_resolution_projection_digest(
        &resolution_request,
        anchored_snapshot.verified_snapshot().snapshot_digest(),
        promoted,
    );

    let mut composition_request = source.composition_request.clone();
    composition_request
        .domain_pack_composition_request
        .request_id = StableId(format!(
        "domain-pack.rebase.composition.{}",
        source_pointer.generation + 1
    ));
    composition_request.domain_pack_composition_request.core = target_core.clone();
    let owned_materials = load_composition_materials(&composition_request, &artifact_root)?;
    let materials = material_views(&composition_request, &owned_materials);
    let composition = compose_domain_packs(&composition_request, &materials);
    let source_was_degraded_empty =
        source_lock.packages.is_empty() && !source_lock.unresolved_composition_gaps.is_empty();
    let composition_admissible = composition
        .domain_pack_composition_projection
        .issues
        .is_empty()
        && if source_was_degraded_empty {
            composition.domain_pack_composition_projection.status
                == DomainPackCompositionStatus::Blocked
                && composition.domain_pack_composition_projection.gaps
                    == source_lock.unresolved_composition_gaps
        } else {
            composition.domain_pack_composition_projection.status
                == DomainPackCompositionStatus::Composable
                && composition
                    .domain_pack_composition_projection
                    .gaps
                    .is_empty()
        };
    if !composition_admissible {
        let projection = &composition.domain_pack_composition_projection;
        return Err(ExitError::with_code(
            2,
            format!(
                "domain-pack: target-Core recomposition is incompatible: status={:?}, issues={}, gaps={}, first_issue={:?}",
                projection.status,
                projection.issues.len(),
                projection.gaps.len(),
                projection.issues.first(),
            ),
        ));
    }

    let trust_selected = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .zip(&assessments)
        .map(|(package, assessment)| {
            Ok(DomainPackTrustSelectedPackage {
                package: package.clone(),
                structurally_valid: true,
                supply_chain: assessment.clone(),
                capability_demands: derive_domain_pack_capability_demands(
                    package,
                    &composition_request.domain_pack_composition_request,
                )
                .map_err(map_lifecycle_error)?,
            })
        })
        .collect::<Result<Vec<_>, ExitError>>()?;
    let trust_input = DomainPackTrustEvaluationInput {
        project_id: source_lock.project_id.clone(),
        selected: trust_selected,
        trust_policy: trust_policy.domain_pack_trust_policy.clone(),
        capability_registry: capability_registry
            .domain_pack_runtime_capability_registry
            .clone(),
        sandbox_policy: sandbox_policy.domain_pack_capability_sandbox_policy.clone(),
    };
    let trust = evaluate_domain_pack_trust(&trust_input);
    if trust.status != DomainPackTrustEvaluationStatus::Approved || !trust.issues.is_empty() {
        return Err(ExitError::with_code(
            2,
            "domain-pack: target-Core trust/capability evaluation blocked",
        ));
    }
    let target_packages = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .map(locked_package_from_resolution)
        .collect::<Vec<_>>();
    if target_packages != source_lock.packages
        || trust.capability_gaps != source_lock.unresolved_capability_gaps
    {
        return Err(ExitError::with_code(
            2,
            "domain-pack: target-Core rebase would silently change package or capability state",
        ));
    }
    let target_payload = DomainPackExactLockPayload {
        project_id: source_lock.project_id.clone(),
        core: target_core.clone(),
        requirements_digest: source_lock.requirements_digest.clone(),
        roots: source_lock.roots.clone(),
        registry_snapshot_digest: source_lock.registry_snapshot_digest.clone(),
        reviewer_registry_digest: source_lock.reviewer_registry_digest.clone(),
        reviewed_registry_digest: source_lock.reviewed_registry_digest.clone(),
        trust_policy_digest: source_lock.trust_policy_digest.clone(),
        capability_registry_digest: source_lock.capability_registry_digest.clone(),
        sandbox_policy_digest: source_lock.sandbox_policy_digest.clone(),
        resolution_digest: resolution
            .domain_pack_resolution_projection
            .resolution_digest
            .clone(),
        composition_digest: composition
            .domain_pack_composition_projection
            .composition_digest
            .clone(),
        packages: target_packages,
        verified_capability_bindings: trust.verified_capability_bindings,
        unresolved_composition_gaps: composition.domain_pack_composition_projection.gaps.clone(),
        unresolved_capability_gaps: trust.capability_gaps.clone(),
    };
    let target_lock = DomainPackExactLockDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_exact_lock: DomainPackExactLock {
            lock_digest: canonical_digest(&target_payload)?,
            payload: target_payload,
        },
    };
    let operation = DomainPackLifecycleOperation::RebaseCore {
        target_release_id: plan.target_release.release_id.clone(),
        expected_from_core_digest: plan.source_core.bundle_digest.clone(),
        target_core_digest: target_core.bundle_digest.clone(),
    };
    let expected_state = DomainPackExpectedLifecycleState::Initialized {
        generation: source_pointer.generation,
        active_lock_digest: source_pointer.active_lock_digest.clone(),
        lifecycle_head_digest: source_pointer.lifecycle_head_digest.clone(),
        project_snapshot_digest: project_snapshot_digest.clone(),
    };
    let lifecycle_request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: StableId(format!(
                "domain-pack.rebase.lifecycle.{}",
                source_pointer.generation + 1
            )),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: source_lock.project_id.clone(),
            principal_id,
            operation: operation.clone(),
            expected_state: expected_state.clone(),
            resolution_request_digest: canonical_digest(&resolution_request)?,
            project_snapshot_digest: project_snapshot_digest.clone(),
        },
    };
    let compatibility = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: StableId(format!(
            "domain-pack.rebase.compatibility.{}",
            source_pointer.generation + 1
        )),
        operation,
        sealed_core: target_core.clone(),
        from_lock: Some(source.exact_lock.clone()),
        to_lock: target_lock.clone(),
    });
    if !matches!(
        compatibility.domain_pack_compatibility_report.status,
        DomainPackCompatibilityStatus::Compatible | DomainPackCompatibilityStatus::Degraded
    ) {
        return Err(ExitError::with_code(
            2,
            "domain-pack: target-Core compatibility report is blocked",
        ));
    }
    let mut staged_artifacts = resolution
        .domain_pack_resolution_projection
        .selected
        .iter()
        .flat_map(|package| {
            let content = DomainPackArtifactBinding {
                artifact_ref: package.package.content.content_ref.clone(),
                raw_sha256: package.package.content.raw_sha256.clone(),
                canonical_sha256: package.package.content.canonical_sha256.clone(),
            };
            std::iter::once(package.package.manifest.clone())
                .chain(std::iter::once(content))
                .chain(std::iter::once(package.package.license.clone()))
                .chain(package.package.fixtures.iter().cloned())
        })
        .collect::<Vec<_>>();
    staged_artifacts.sort_by(|left, right| left.artifact_ref.0.cmp(&right.artifact_ref.0));
    staged_artifacts.dedup();
    let mut preflight = DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: StableId(format!(
                "domain-pack.rebase.preflight.{}",
                source_pointer.generation + 1
            )),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request_digest: canonical_digest(&lifecycle_request)?,
            request: lifecycle_request,
            observed_state: expected_state,
            resolution,
            proposed_lock: target_lock,
            composition,
            supply_chain_assessments: assessments,
            trust_decisions: trust.trust_decisions,
            capability_gaps: trust.capability_gaps,
            compatibility_report: compatibility,
            staged_artifacts,
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: Vec::new(),
            preflight_digest: String::new(),
        },
    };
    preflight.domain_pack_lifecycle_preflight.preflight_digest = canonical_digest(&preflight)?;
    let owned_artifacts = load_immutable_artifacts(&preflight, &artifact_root)?;
    let artifacts = immutable_artifact_views(&owned_artifacts);
    let project_snapshot =
        verify_domain_pack_project_snapshot(&controlled_roots.project, &project_snapshot_digest)
            .map_err(map_lifecycle_error)?;
    let prepared = lifecycle
        .prepare_candidate(preflight)
        .map_err(map_lifecycle_error)?;
    let context = DomainPackLifecycleAuthorizationContext {
        anchored_snapshot: &anchored_snapshot,
        anchored_reviewed_snapshot: reviewed_guard.snapshot(),
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
    let receipt = lifecycle
        .commit(prepared, authority)
        .map_err(map_lifecycle_error)?;
    drop(reviewed_guard);
    Ok(receipt)
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

#[allow(clippy::similar_names)] // Reviewer and reviewed registries are separate required roots.
fn run_lifecycle_authorized(args: &[String], apply: bool) -> Result<(), ExitError> {
    let mut preflight_file: Option<PathBuf> = None;
    let mut trust_policy_file: Option<PathBuf> = None;
    let mut registry_file: Option<PathBuf> = None;
    let mut reviewer_registry_file: Option<PathBuf> = None;
    let mut reviewed_registry_file: Option<PathBuf> = None;
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
            "--reviewer-registry-file" => set_path(&mut reviewer_registry_file, value),
            "--reviewed-registry-file" => set_path(&mut reviewed_registry_file, value),
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
    let reviewer_registry_file = required(reviewer_registry_file)?;
    let reviewed_registry_file = required(reviewed_registry_file)?;
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
    let reviewer_registry_file = trusted_external_file(
        &reviewer_registry_file,
        "signed reviewer registry",
        &controlled_roots,
    )?;
    let reviewed_registry_file = trusted_external_file(
        &reviewed_registry_file,
        "dual-signed reviewed registry",
        &controlled_roots,
    )?;

    let preflight: DomainPackLifecyclePreflightDocument =
        read_typed(&preflight_file, "lifecycle preflight")?;
    let owned_artifacts = load_immutable_artifacts(&preflight, &artifact_root)?;
    let artifacts = immutable_artifact_views(&owned_artifacts);
    let operator_anchor = lock_operator_registry_anchor(&registry_file)?;
    require_direct_operator_file(
        &reviewer_registry_file,
        &operator_anchor.operator_root,
        "signed reviewer registry",
    )?;
    require_direct_operator_file(
        &reviewed_registry_file,
        &operator_anchor.operator_root,
        "dual-signed reviewed registry",
    )?;
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
    // Deterministic cross-anchor order: supply-chain lock first, then the
    // combined reviewer/reviewed learning lock. The guard is explicitly kept
    // through authorization and commit below.
    let reviewed_guard = crate::domain_pack_learning_cmd::lock_reviewed_snapshot_for_lifecycle(
        &operator_anchor.operator_root,
        &reviewer_registry_file,
        &reviewed_registry_file,
        now_unix,
    )?;
    let owned = load_composition_materials(&composition_request, &artifact_root)?;
    let materials = material_views(&composition_request, &owned);
    let mut lifecycle = lock_domain_pack_lifecycle(&state_root).map_err(map_lifecycle_error)?;
    let prepared = lifecycle
        .prepare_candidate(preflight.clone())
        .map_err(map_lifecycle_error)?;
    let context = DomainPackLifecycleAuthorizationContext {
        anchored_snapshot: &anchored_snapshot,
        anchored_reviewed_snapshot: reviewed_guard.snapshot(),
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
        drop(reviewed_guard);
        crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack apply", receipt), want_json)
    } else {
        let payload = DomainPackLifecyclePreflightPayload {
            ready: true,
            preflight_digest: preflight.domain_pack_lifecycle_preflight.preflight_digest,
            supply_chain: anchored_snapshot.verified_snapshot().audit(),
            boundary: "fresh verification completed under lifecycle lock; this preflight did not activate the candidate generation",
        };
        drop(reviewed_guard);
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

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn persist_domain_pack_operator_sources(
    state_root: &Path,
    binding: &DomainPackOperatorSourceBinding,
) -> Result<(), ExitError> {
    let lock =
        acquire_effect_store_lock(state_root, DOMAIN_PACK_OPERATOR_SOURCE_LOCK_RELATIVE_PATH)
            .map_err(|error| {
                ExitError::failed(format!(
                    "domain-pack: cannot lock operator-source binding: {error}"
                ))
            })?;
    let recovery = recover_file_crash_safe_under_lock(
        state_root,
        &lock,
        DOMAIN_PACK_OPERATOR_SOURCE_LOCK_RELATIVE_PATH,
        DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH,
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )
    .map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot recover operator-source binding: {error}"
        ))
    })?;
    let raw = yaml_serde::to_string(binding)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot serialize operator-source binding: {error}"
            ))
        })?
        .into_bytes();
    replace_file_crash_safe_under_lock(
        state_root,
        &lock,
        DOMAIN_PACK_OPERATOR_SOURCE_LOCK_RELATIVE_PATH,
        DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH,
        recovery.target_digest.as_deref(),
        &raw,
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )
    .map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot persist operator-source binding: {error}"
        ))
    })?;
    Ok(())
}

fn load_domain_pack_operator_sources(
    state_root: &Path,
) -> Result<DomainPackOperatorSourceBinding, ExitError> {
    let path = state_root.join(DOMAIN_PACK_OPERATOR_SOURCE_RELATIVE_PATH);
    let binding: DomainPackOperatorSourceBinding = read_typed(&path, "operator-source binding")?;
    if binding.schema_version != DOMAIN_PACK_OPERATOR_SOURCE_SCHEMA_VERSION {
        return Err(ExitError::invalid_value(
            "domain-pack: operator-source binding has unsupported schema",
        ));
    }
    Ok(binding)
}

fn persist_domain_pack_rebase_plan(
    state_root: &Path,
    plan: &DomainPackRebasePlanDocument,
) -> Result<(), ExitError> {
    let lock = acquire_effect_store_lock(state_root, DOMAIN_PACK_REBASE_PLAN_LOCK_RELATIVE_PATH)
        .map_err(|error| {
            ExitError::failed(format!("domain-pack: cannot lock rebase plan: {error}"))
        })?;
    let recovery = recover_file_crash_safe_under_lock(
        state_root,
        &lock,
        DOMAIN_PACK_REBASE_PLAN_LOCK_RELATIVE_PATH,
        DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH,
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )
    .map_err(|error| {
        ExitError::failed(format!("domain-pack: cannot recover rebase plan: {error}"))
    })?;
    let raw = yaml_serde::to_string(plan)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot serialize rebase plan: {error}"
            ))
        })?
        .into_bytes();
    replace_file_crash_safe_under_lock(
        state_root,
        &lock,
        DOMAIN_PACK_REBASE_PLAN_LOCK_RELATIVE_PATH,
        DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH,
        recovery.target_digest.as_deref(),
        &raw,
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
    )
    .map_err(|error| {
        ExitError::failed(format!("domain-pack: cannot persist rebase plan: {error}"))
    })?;
    Ok(())
}

pub(crate) fn load_persisted_domain_pack_rebase_plan(
    state_root: &Path,
    expected_digest: &str,
) -> Result<DomainPackRebasePlanDocument, ExitError> {
    let path = state_root.join(DOMAIN_PACK_REBASE_PLAN_RELATIVE_PATH);
    let plan: DomainPackRebasePlanDocument = read_typed(&path, "persisted rebase plan")?;
    if !verify_domain_pack_rebase_plan(&plan)
        || plan.domain_pack_rebase_plan.plan_digest != expected_digest
    {
        return Err(ExitError::conflict(
            "domain-pack: persisted rebase plan failed integrity or expected CAS",
        ));
    }
    Ok(plan)
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
    let (capability, changed) =
        match anchor
            .compare_and_advance(&expected, verified)
            .map_err(|error| {
                ExitError::with_code(
                    2,
                    format!("domain-pack: monotonic registry anchor rejected: {error}"),
                )
            })? {
            DomainPackRegistryAnchorAdvance::Advanced(capability) => (capability, true),
            DomainPackRegistryAnchorAdvance::Replay { capability, .. } => (capability, false),
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
    if changed {
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
    }
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
    let path_metadata = std::fs::metadata(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect {label} '{}': {error}",
            path.display()
        ))
    })?;
    if !path_metadata.is_file() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: {label} '{}' must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(path)
    };
    #[cfg(not(unix))]
    let file = std::fs::OpenOptions::new().read(true).open(path);
    let file = file.map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot open bounded regular {label} '{}': {error}",
            path.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect {label} '{}': {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(ExitError::invalid_value(format!(
            "domain-pack: opened {label} '{}' is not a regular file",
            path.display()
        )));
    }
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
    reject_yaml_alias_syntax(raw, path)?;
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

fn reject_yaml_alias_syntax(raw: &[u8], path: &Path) -> Result<(), ExitError> {
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut comment = false;
    let mut index = 0usize;
    while index < raw.len() {
        let byte = raw[index];
        if comment {
            if byte == b'\n' {
                comment = false;
            }
            index += 1;
            continue;
        }
        if double_quoted {
            if byte == b'\\' {
                index = index.saturating_add(2);
                continue;
            }
            if byte == b'"' {
                double_quoted = false;
            }
            index += 1;
            continue;
        }
        if single_quoted {
            if byte == b'\'' {
                if raw.get(index + 1) == Some(&b'\'') {
                    index = index.saturating_add(2);
                    continue;
                }
                single_quoted = false;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' => double_quoted = true,
            b'\'' => single_quoted = true,
            b'#' if index == 0
                || raw
                    .get(index.wrapping_sub(1))
                    .is_some_and(u8::is_ascii_whitespace) =>
            {
                comment = true;
            }
            b'&' | b'*'
                if (index == 0
                    || raw.get(index.wrapping_sub(1)).is_some_and(|previous| {
                        previous.is_ascii_whitespace()
                            || matches!(previous, b'-' | b'?' | b':' | b',' | b'[' | b'{')
                    }))
                    && raw.get(index + 1).is_some_and(|next| {
                        !next.is_ascii_whitespace()
                            && !matches!(next, b',' | b'[' | b']' | b'{' | b'}')
                    }) =>
            {
                return Err(ExitError::invalid_value(format!(
                    "domain-pack: '{}' uses YAML anchor/alias syntax; bounded authority documents must be alias-free",
                    path.display()
                )));
            }
            _ => {}
        }
        index += 1;
    }
    Ok(())
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
        "\n  validate, search, explain, acquire plan, compose, and resolve are read-only candidate projections; trust-provision requires explicit operator approval and mutates only the external monotonic trust anchor; status/recover integrity-check lifecycle state and may complete an interrupted pointer replacement",
    );
    output
}
