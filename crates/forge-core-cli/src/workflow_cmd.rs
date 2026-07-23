//! Agent-facing P5c workflow-governance command family.
//!
//! Humans stay in chat: these commands are intended for the host agent. The
//! command never accepts a workflow, phase, bundle, or readiness target.

use crate::cli_error::ExitError;
use crate::cli_util::{command_surface_usage, emit_envelope};
use forge_core_authority::{
    AttestationInput, AttestationPolicy, AttestationVerifier, AuthorizedPrincipalRegistry,
    WorkflowApplicabilityAuthorizationRequest, WorkflowCapabilityAuthorizationRequest,
    WorkflowDecisionAuthorizationRequest, WorkflowEvidenceAuthorizationRequest,
    WorkflowSignalAuthorizationRequest, WorkflowWaiverAuthorizationRequest,
};
use forge_core_command_surface::COMMAND_WORKFLOW;
use forge_core_contracts::{CliEnvelope, ExitReason, PrincipalId, StableId};
use forge_core_kernel::{
    load_admitted_workflow_retirement_checkpoint, WorkflowGovernanceAdapterError,
    WorkflowGovernanceProjectAdapter,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct WorkflowCliArgs {
    subcommand: String,
    root: PathBuf,
    want_json: bool,
    flags: BTreeMap<String, Vec<String>>,
}

/// Dispatch the live governance family.
///
/// # Errors
/// Returns typed usage, environment, governance, integrity, or authorization
/// errors through the canonical CLI envelope path.
///
/// # Panics
/// Panics only if a repository-owned typed workflow response unexpectedly
/// fails JSON serialization, which would violate its derived serde contract.
pub fn run_workflow_command(args: &[String]) -> Result<(), ExitError> {
    if args.get(1).is_some_and(|value| value == "intent") {
        let want_json = wants_json(args);
        return match crate::workflow_intent_cmd::run(&args[2..]) {
            Ok(()) => Ok(()),
            Err(error) if want_json => emit_failure(
                "workflow.intent",
                credential_exit_reason(&error),
                error.message().to_owned(),
                true,
            ),
            Err(error) => Err(error),
        };
    }
    if args.get(1).is_some_and(|value| value == "action") {
        let want_json = wants_json(args);
        return match crate::workflow_action_cmd::run(&args[2..]) {
            Ok(()) => Ok(()),
            Err(error) if want_json => emit_failure(
                "workflow.action",
                credential_exit_reason(&error),
                error.message().to_owned(),
                true,
            ),
            Err(error) => Err(error),
        };
    }
    if args.get(1).is_some_and(|value| value == "broker") {
        let want_json = wants_json(args);
        return match crate::workflow_broker_cmd::run(&args[2..]) {
            Ok(()) => Ok(()),
            Err(error) if want_json => emit_failure(
                "workflow.broker",
                credential_exit_reason(&error),
                error.message().to_owned(),
                true,
            ),
            Err(error) => Err(error),
        };
    }
    if args.get(1).is_some_and(|value| value == "credential") {
        let want_json = wants_json(args);
        return match crate::workflow_credential_cmd::run(&args[2..]) {
            Ok(()) => Ok(()),
            Err(error) if want_json => emit_failure(
                "workflow.credential",
                credential_exit_reason(&error),
                error.message().to_owned(),
                true,
            ),
            Err(error) => Err(error),
        };
    }
    if args
        .get(1)
        .is_some_and(|value| matches!(value.as_str(), "--help" | "-h"))
        || args.len() < 2
    {
        println!("{}", command_surface_usage(&COMMAND_WORKFLOW));
        return Ok(());
    }
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            return emit_failure(
                "workflow",
                ExitReason::InvalidDecisionShape,
                message,
                wants_json(args),
            );
        }
    };
    if parsed.subcommand == "help" {
        println!("{}", command_surface_usage(&COMMAND_WORKFLOW));
        return Ok(());
    }
    let command = format!("workflow.{}", parsed.subcommand.replace('-', "_"));
    if legacy_direct_authorization_is_disabled(&parsed.subcommand) {
        return emit_failure(
            &command,
            ExitReason::RejectedByGate,
            "legacy request-file and attestation-file authorization is disabled; use `workflow action authorize` for an operator_credential_broker packet or `workflow action apply` for an already-signed external broker envelope"
                .to_owned(),
            parsed.want_json,
        );
    }
    if let Err(message) = validate_release_args(&parsed) {
        return emit_failure(
            &command,
            ExitReason::InvalidDecisionShape,
            message,
            parsed.want_json,
        );
    }
    if parsed.subcommand == "retirement-status" {
        return match retirement_status(&parsed.root) {
            Ok(value) => emit_envelope(CliEnvelope::ok(&command, value), parsed.want_json),
            Err(message) => {
                emit_failure(&command, ExitReason::EnvConfig, message, parsed.want_json)
            }
        };
    }
    let adapter = match resolve_adapter(&parsed.root) {
        Ok(adapter) => adapter,
        Err(message) => {
            return emit_failure(&command, ExitReason::EnvConfig, message, parsed.want_json);
        }
    };
    let result = match parsed.subcommand.as_str() {
        "init" => adapter
            .initialize()
            .map(|value| serde_json::to_value(value).expect("serializable initialization")),
        "next" => adapter
            .next()
            .map(|value| serde_json::to_value(value).expect("serializable guidance")),
        "action-packets" => adapter.action_packets().map(|value| {
            serde_json::to_value(value).expect("serializable workflow action packets")
        }),
        "resume" => adapter
            .resume()
            .map(|value| serde_json::to_value(value).expect("serializable guidance")),
        "release-status" => adapter
            .release_status()
            .map(|value| serde_json::to_value(value).expect("serializable release status")),
        "release-rebase-plan" => release_rebase_plan(&adapter, &parsed),
        "release-rebase-apply" => release_rebase_apply(&adapter, &parsed),
        "release-upgrade" => release_upgrade(&adapter, &parsed),
        "shadow" => adapter
            .shadow()
            .map(|value| serde_json::to_value(value).expect("serializable shadow report")),
        "complete" => complete(&adapter, &parsed),
        "applicability-authorize" => authorize_applicability(&adapter, &parsed),
        "capability-authorize" => authorize_capability(&adapter, &parsed),
        "decision-resolve" => authorize_decision(&adapter, &parsed),
        "evidence-authorize" => authorize_evidence(&adapter, &parsed),
        "signal-authorize" => authorize_signal(&adapter, &parsed),
        "waiver-authorize" => authorize_waiver(&adapter, &parsed),
        other => {
            return emit_failure(
                &command,
                ExitReason::InvalidDecisionShape,
                format!(
                    "unknown workflow subcommand '{other}'\n\n{}",
                    command_surface_usage(&COMMAND_WORKFLOW)
                ),
                parsed.want_json,
            );
        }
    };
    match result {
        Ok(value) => emit_envelope(CliEnvelope::ok(&command, value), parsed.want_json),
        Err(error) => emit_failure(
            &command,
            classify_error(&error),
            error.to_string(),
            parsed.want_json,
        ),
    }
}

fn credential_exit_reason(error: &ExitError) -> ExitReason {
    match error {
        ExitError::Usage { .. } | ExitError::InvalidValue { .. } => {
            ExitReason::InvalidDecisionShape
        }
        ExitError::Conflict { .. } => ExitReason::Conflict,
        ExitError::EnvConfig { .. } => ExitReason::EnvConfig,
        ExitError::Failed { .. } | ExitError::WithCode { .. } => ExitReason::RejectedByGate,
    }
}

const RETIREMENT_EVIDENCE_INDEX: &str =
    "contracts/migration/workflow-retirement-evidence-index-v0.yaml";
const RETIREMENT_TOMBSTONES: &str = "contracts/migration/workflow-retirement-tombstones-v0.yaml";
const RETIREMENT_SCORECARD: &str =
    "contracts/migration/workflow-governance-final-scorecard-v0.yaml";

#[derive(Debug, serde::Serialize)]
struct WorkflowRetirementStatus {
    /// This is an audit projection. The underlying capability remains opaque
    /// and process-owned by the kernel.
    authority: &'static str,
    authorization_projection: &'static str,
    release_id: String,
    verified_retirement_count: usize,
    operational_workflow_count: usize,
    authorization_id: String,
    payload_digest: String,
    retirement_set_digest: String,
    final_scorecard_digest: String,
    evidence_index_ref: &'static str,
    tombstone_catalog_ref: &'static str,
    scorecard_ref: &'static str,
}

/// Read-only audit projection of the kernel-admitted retirement checkpoint.
/// Caller/project files are never consulted and cannot select authority.
fn retirement_status(_root: &Path) -> Result<Value, String> {
    let checkpoint = load_admitted_workflow_retirement_checkpoint()
        .map_err(|error| format!("verified retirement checkpoint is unavailable: {error}"))?;
    let audit = checkpoint.audit();
    let score = &checkpoint.scorecard().workflow_final_scorecard;
    serde_json::to_value(WorkflowRetirementStatus {
        authority: "verified_retirement_checkpoint",
        authorization_projection: "non_authoritative_audit_of_opaque_capability",
        release_id: audit.release_id.clone(),
        verified_retirement_count: score.legacy_authority_counts.retired,
        operational_workflow_count: score.legacy_authority_counts.retained,
        authorization_id: audit.authorization_id.clone(),
        payload_digest: audit.payload_digest.clone(),
        retirement_set_digest: audit.retirement_set_digest.clone(),
        final_scorecard_digest: audit.final_scorecard_digest.clone(),
        evidence_index_ref: RETIREMENT_EVIDENCE_INDEX,
        tombstone_catalog_ref: RETIREMENT_TOMBSTONES,
        scorecard_ref: RETIREMENT_SCORECARD,
    })
    .map_err(|error| format!("serialize retirement status: {error}"))
}

fn release_upgrade(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, WorkflowGovernanceAdapterError> {
    let target_release_id =
        StableId(required(args, "target-release-id").map_err(invalid_observation)?);
    let expected_current_release_digest =
        required(args, "expected-current-release-digest").map_err(invalid_observation)?;
    let expected_head_digest =
        required(args, "expected-head-digest").map_err(invalid_observation)?;
    let expected_snapshot_digest =
        required(args, "expected-snapshot-digest").map_err(invalid_observation)?;
    adapter
        .release_upgrade(
            &target_release_id,
            &expected_current_release_digest,
            &expected_head_digest,
            &expected_snapshot_digest,
        )
        .map(|value| serde_json::to_value(value).expect("serializable release upgrade receipt"))
}
fn release_rebase_plan(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, WorkflowGovernanceAdapterError> {
    let target_release_id =
        StableId(required(args, "target-release-id").map_err(invalid_observation)?);
    let expected_plan_digest =
        required(args, "expected-rebase-plan-digest").map_err(invalid_observation)?;
    adapter
        .release_rebase_plan(&target_release_id, &expected_plan_digest)
        .map(|value| serde_json::to_value(value).expect("serializable Domain Pack rebase plan"))
}

fn release_rebase_apply(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, WorkflowGovernanceAdapterError> {
    let target_release_id =
        StableId(required(args, "target-release-id").map_err(invalid_observation)?);
    let expected_plan_digest =
        required(args, "expected-rebase-plan-digest").map_err(invalid_observation)?;
    let plan = match adapter.release_rebase_plan(&target_release_id, &expected_plan_digest) {
        Ok(plan) => {
            crate::domain_pack_cmd::apply_domain_pack_core_rebase(
                &adapter.binding().project_root,
                &adapter.binding().state_root,
                &plan,
                &plan.domain_pack_rebase_plan.target_core,
                StableId("principal.domain-pack-rebase-operator".to_owned()),
            )
            .map_err(|error| {
                WorkflowGovernanceAdapterError::DomainPackRebaseLifecycle(error.to_string())
            })?;
            #[cfg(feature = "expensive-p6d-e2e")]
            if matches!(
                std::env::var("FORGE_TEST_CRASH_AFTER_REBASE_LIFECYCLE").as_deref(),
                Ok("1")
            ) {
                eprintln!("injected crash after lifecycle commit");
                // This must bypass unwinding: the E2E proves that a replacement
                // process recovers the durable lifecycle-first boundary.
                std::process::exit(86);
            }
            plan
        }
        Err(fresh_error) => {
            let persisted = crate::domain_pack_cmd::load_persisted_domain_pack_rebase_plan(
                &adapter.binding().state_root,
                &expected_plan_digest,
            )
            .map_err(|_| fresh_error)?;
            if persisted.domain_pack_rebase_plan.target_release.release_id != target_release_id {
                return Err(WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch);
            }
            persisted
        }
    };
    adapter
        .complete_release_rebase(&plan)
        .map(|value| serde_json::to_value(value).expect("serializable joined rebase receipt"))
}

fn complete(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, forge_core_kernel::WorkflowGovernanceAdapterError> {
    let expected = required(args, "if-snapshot").map_err(invalid_observation)?;
    let principal = PrincipalId(
        optional(args, "principal").unwrap_or_else(|| "principal.replacement-agent".to_owned()),
    );
    let prepared = adapter.prepare_completion_for_snapshot(&expected)?;
    adapter
        .consume_completion(prepared, principal)
        .map(|receipt| serde_json::to_value(receipt).expect("serializable completion receipt"))
}

fn authorize_applicability(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, forge_core_kernel::WorkflowGovernanceAdapterError> {
    let (registry, attestation) =
        authorization_material(adapter, args).map_err(invalid_observation)?;
    let request: WorkflowApplicabilityAuthorizationRequest =
        load_json(&required_path(args, "request-file").map_err(invalid_observation)?)
            .map_err(invalid_observation)?;
    let authorization = registry
        .authorize_workflow_applicability(
            &AttestationVerifier::new(AttestationPolicy::Default),
            request,
            &attestation,
        )
        .map_err(|error| invalid_observation(error.to_string()))?;
    adapter
        .record_authorized_applicability(authorization)
        .map(|record| serde_json::to_value(record).expect("serializable receipt"))
}

fn authorize_capability(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, forge_core_kernel::WorkflowGovernanceAdapterError> {
    let (registry, attestation) =
        authorization_material(adapter, args).map_err(invalid_observation)?;
    let request: WorkflowCapabilityAuthorizationRequest =
        load_json(&required_path(args, "request-file").map_err(invalid_observation)?)
            .map_err(invalid_observation)?;
    let authorization = registry
        .authorize_workflow_capability(
            &AttestationVerifier::new(AttestationPolicy::Default),
            request,
            &attestation,
        )
        .map_err(|error| invalid_observation(error.to_string()))?;
    adapter
        .record_authorized_capability(authorization)
        .map(|record| serde_json::to_value(record).expect("serializable receipt"))
}

fn authorize_decision(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, forge_core_kernel::WorkflowGovernanceAdapterError> {
    let (registry, attestation) =
        authorization_material(adapter, args).map_err(invalid_observation)?;
    let request: WorkflowDecisionAuthorizationRequest =
        load_json(&required_path(args, "request-file").map_err(invalid_observation)?)
            .map_err(invalid_observation)?;
    let authorization = registry
        .authorize_workflow_decision(
            &AttestationVerifier::new(AttestationPolicy::Default),
            request,
            &attestation,
        )
        .map_err(|error| invalid_observation(error.to_string()))?;
    adapter
        .record_authorized_decision(authorization)
        .map(|record| serde_json::to_value(record).expect("serializable receipt"))
}

fn authorize_evidence(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, forge_core_kernel::WorkflowGovernanceAdapterError> {
    let (registry, attestation) =
        authorization_material(adapter, args).map_err(invalid_observation)?;
    let request: WorkflowEvidenceAuthorizationRequest =
        load_json(&required_path(args, "request-file").map_err(invalid_observation)?)
            .map_err(invalid_observation)?;
    let authorization = registry
        .authorize_workflow_evidence(
            &AttestationVerifier::new(AttestationPolicy::Default),
            request,
            &attestation,
        )
        .map_err(|error| invalid_observation(error.to_string()))?;
    adapter
        .record_authorized_evidence(authorization)
        .map(|record| serde_json::to_value(record).expect("serializable receipt"))
}

fn authorize_waiver(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, forge_core_kernel::WorkflowGovernanceAdapterError> {
    let (registry, attestation) =
        authorization_material(adapter, args).map_err(invalid_observation)?;
    let request: WorkflowWaiverAuthorizationRequest =
        load_json(&required_path(args, "request-file").map_err(invalid_observation)?)
            .map_err(invalid_observation)?;
    let authorization = registry
        .authorize_workflow_waiver(
            &AttestationVerifier::new(AttestationPolicy::Default),
            request,
            &attestation,
        )
        .map_err(|error| invalid_observation(error.to_string()))?;
    adapter
        .record_authorized_waiver(authorization)
        .map(|record| serde_json::to_value(record).expect("serializable receipt"))
}

fn authorize_signal(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, forge_core_kernel::WorkflowGovernanceAdapterError> {
    let (registry, attestation) =
        authorization_material(adapter, args).map_err(invalid_observation)?;
    let request: WorkflowSignalAuthorizationRequest =
        load_json(&required_path(args, "request-file").map_err(invalid_observation)?)
            .map_err(invalid_observation)?;
    let authorization = registry
        .authorize_workflow_signal(
            &AttestationVerifier::new(AttestationPolicy::Default),
            request,
            &attestation,
        )
        .map_err(|error| invalid_observation(error.to_string()))?;
    adapter
        .record_authorized_signal(authorization)
        .map(|record| serde_json::to_value(record).expect("serializable receipt"))
}

fn authorization_material(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<(AuthorizedPrincipalRegistry, AttestationInput), String> {
    let registry_path = adapter.trusted_principal_registry_path();
    let registry_raw = fs::read_to_string(&registry_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "workflow authority is not provisioned at {}; use `forge-core workflow credential provision --root {} --credential-id <id> --principal-id <id> --agent-id <id> --profile <human|agent|runtime> --json` before recording an authority-bearing observation",
                registry_path.display(),
                adapter.binding().project_root.display()
            )
        } else {
            format!(
                "read principal registry {}: {error}",
                registry_path.display()
            )
        }
    })?;
    let registry = AuthorizedPrincipalRegistry::from_yaml_str(&registry_raw)
        .map_err(|error| format!("invalid principal registry: {error}"))?;
    let attestation = load_json(&required_path(args, "attestation-file")?)?;
    Ok((registry, attestation))
}

fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let raw =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_str(&raw).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn resolve_adapter(root: &Path) -> Result<WorkflowGovernanceProjectAdapter, String> {
    let project = crate::project_cmd::resolve_project(root)
        .map_err(|error| format!("project resolve failed: {error}"))?;
    if !project.state_exists {
        return Err(format!(
            "resolved state root {} does not exist; run project init first",
            project.state_root
        ));
    }
    WorkflowGovernanceProjectAdapter::new(
        StableId(project.project_id),
        PathBuf::from(project.project_root),
        PathBuf::from(project.state_root),
    )
    .map_err(|error| error.to_string())
}

fn parse_args(args: &[String]) -> Result<WorkflowCliArgs, String> {
    let subcommand = args
        .get(1)
        .ok_or_else(|| "workflow subcommand is required".to_owned())?
        .clone();
    if matches!(subcommand.as_str(), "--help" | "-h") {
        return Ok(WorkflowCliArgs {
            subcommand: "help".to_owned(),
            root: PathBuf::from("."),
            want_json: true,
            flags: BTreeMap::new(),
        });
    }
    let mut root = PathBuf::from(".");
    let mut want_json = true;
    let mut flags = BTreeMap::<String, Vec<String>>::new();
    let mut index = 2usize;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--json" => want_json = true,
            "--no-json" => want_json = false,
            "--policy" | "--phase" | "--bundle" | "--bundle-file" | "--bundle-path"
            | "--registry" | "--registry-file" | "--registry-path" | "--manifest"
            | "--manifest-file" | "--manifest-path" | "--batch" | "--batch-file"
            | "--batch-path" | "--release" | "--release-file" | "--release-path" | "--target" => {
                return Err(format!(
                    "{flag} is forbidden: the trusted Adapter derives workflow, phase, admitted release registry, bundle, and readiness target"
                ));
            }
            "--root"
            | "--principal"
            | "--if-snapshot"
            | "--request-file"
            | "--attestation-file"
            | "--target-release-id"
            | "--expected-current-release-digest"
            | "--expected-head-digest"
            | "--expected-rebase-plan-digest"
            | "--expected-snapshot-digest" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                if value.starts_with('-') {
                    return Err(format!("{flag} requires a value, got flag '{value}'"));
                }
                if flag == "--root" {
                    root = PathBuf::from(value);
                } else {
                    flags
                        .entry(flag.trim_start_matches('-').to_owned())
                        .or_default()
                        .push(value.clone());
                }
            }
            "--help" | "-h" => {
                return Ok(WorkflowCliArgs {
                    subcommand: "help".to_owned(),
                    root,
                    want_json,
                    flags,
                });
            }
            other => return Err(format!("unrecognized workflow argument '{other}'")),
        }
        index += 1;
    }
    for (flag, values) in &flags {
        if values.len() > 1 {
            return Err(format!("--{flag} may be supplied only once"));
        }
    }
    Ok(WorkflowCliArgs {
        subcommand,
        root,
        want_json,
        flags,
    })
}

fn required(args: &WorkflowCliArgs, name: &str) -> Result<String, String> {
    optional(args, name).ok_or_else(|| format!("--{name} is required"))
}

fn required_path(args: &WorkflowCliArgs, name: &str) -> Result<PathBuf, String> {
    required(args, name).map(PathBuf::from)
}

fn optional(args: &WorkflowCliArgs, name: &str) -> Option<String> {
    args.flags
        .get(name)
        .and_then(|values| values.first())
        .cloned()
}

fn invalid_observation(message: String) -> forge_core_kernel::WorkflowGovernanceAdapterError {
    forge_core_kernel::WorkflowGovernanceAdapterError::InvalidObservation(message)
}

fn validate_release_args(args: &WorkflowCliArgs) -> Result<(), String> {
    if let Some(flag) = ["request-file", "attestation-file"]
        .iter()
        .find(|flag| args.flags.contains_key(**flag))
    {
        return Err(format!(
            "--{flag} is not valid for workflow {}; direct request/attestation authorization is retired; use `workflow action authorize` or `workflow action apply`",
            args.subcommand
        ));
    }
    match args.subcommand.as_str() {
        "action-packets" | "release-status" | "retirement-status" if !args.flags.is_empty() => {
            Err(format!(
                "workflow {} accepts only --root and the JSON output switch",
                args.subcommand
            ))
        }
        "release-upgrade" => {
            let expected = [
                "target-release-id",
                "expected-current-release-digest",
                "expected-head-digest",
                "expected-snapshot-digest",
            ];
            if let Some(flag) = args
                .flags
                .keys()
                .find(|flag| !expected.contains(&flag.as_str()))
            {
                return Err(format!(
                    "--{flag} is not valid for workflow release-upgrade"
                ));
            }
            let target = required(args, "target-release-id")?;
            if target.trim().is_empty() {
                return Err("--target-release-id must not be blank".to_owned());
            }
            for name in &expected[1..] {
                let digest = required(args, name)?;
                if !is_lowercase_sha256(&digest) {
                    return Err(format!(
                        "--{name} must be a canonical lowercase sha256:<64-hex> digest"
                    ));
                }
            }
            Ok(())
        }
        "release-rebase-plan" | "release-rebase-apply" => {
            let expected = ["target-release-id", "expected-rebase-plan-digest"];
            if let Some(flag) = args
                .flags
                .keys()
                .find(|flag| !expected.contains(&flag.as_str()))
            {
                return Err(format!(
                    "--{flag} is not valid for workflow {}",
                    args.subcommand
                ));
            }
            let target = required(args, "target-release-id")?;
            if target.trim().is_empty() {
                return Err("--target-release-id must not be blank".to_owned());
            }
            let digest = required(args, "expected-rebase-plan-digest")?;
            if !is_lowercase_sha256(&digest) {
                return Err("--expected-rebase-plan-digest must be a canonical lowercase sha256:<64-hex> digest".to_owned());
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn classify_error(error: &WorkflowGovernanceAdapterError) -> ExitReason {
    match error {
        WorkflowGovernanceAdapterError::Ledger(_)
        | WorkflowGovernanceAdapterError::LedgerIdentityMismatch
        | WorkflowGovernanceAdapterError::ReleaseCasMismatch
        | WorkflowGovernanceAdapterError::ReleaseChainInvalid
        | WorkflowGovernanceAdapterError::ReleaseCommitIndeterminate
        | WorkflowGovernanceAdapterError::DomainPackRebaseCasMismatch
        | WorkflowGovernanceAdapterError::CompletionDrift => ExitReason::Conflict,
        WorkflowGovernanceAdapterError::InvalidProjectId
        | WorkflowGovernanceAdapterError::Path { .. }
        | WorkflowGovernanceAdapterError::InvalidStateRoot { .. }
        | WorkflowGovernanceAdapterError::ProjectBinding { .. }
        | WorkflowGovernanceAdapterError::TrustedRegistry { .. }
        | WorkflowGovernanceAdapterError::SnapshotCapacity { .. }
        | WorkflowGovernanceAdapterError::SnapshotPathEscape { .. }
        | WorkflowGovernanceAdapterError::LedgerUninitialized
        | WorkflowGovernanceAdapterError::Clock
        | WorkflowGovernanceAdapterError::ClockOverflow => ExitReason::EnvConfig,
        _ => ExitReason::RejectedByGate,
    }
}

fn legacy_direct_authorization_is_disabled(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "applicability-authorize"
            | "capability-authorize"
            | "decision-resolve"
            | "evidence-authorize"
            | "signal-authorize"
            | "waiver-authorize"
    )
}

fn emit_failure(
    command: &str,
    reason: ExitReason,
    message: String,
    want_json: bool,
) -> Result<(), ExitError> {
    emit_envelope(
        CliEnvelope::<Value>::err(command, reason, message),
        want_json,
    )
}

fn wants_json(args: &[String]) -> bool {
    !args.iter().any(|arg| arg == "--no-json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parser_forbids_caller_selected_authority() {
        for flag in [
            "--policy",
            "--phase",
            "--bundle-file",
            "--registry-path",
            "--manifest-file",
            "--batch-path",
            "--release-file",
            "--target",
        ] {
            let args = argv(&["workflow", "next", flag, "attacker"]);
            let error = parse_args(&args).expect_err("forbidden authority flag");
            assert!(error.contains("forbidden"), "{error}");
        }
    }

    #[test]
    fn legacy_direct_authorization_subcommands_are_all_hard_gated() {
        for subcommand in [
            "applicability-authorize",
            "capability-authorize",
            "decision-resolve",
            "evidence-authorize",
            "signal-authorize",
            "waiver-authorize",
        ] {
            assert!(legacy_direct_authorization_is_disabled(subcommand));
        }
        for permitted in ["action", "intent", "complete", "release-status"] {
            assert!(!legacy_direct_authorization_is_disabled(permitted));
        }
    }

    #[test]
    fn parser_rejects_conflicting_applicability() {
        let args = argv(&[
            "workflow",
            "assess-applicability",
            "--applicable",
            "--not-applicable",
        ]);
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn trusted_clock_override_is_not_accepted() {
        let args = argv(&["workflow", "next", "--now-unix", "9999999999"]);
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn live_commands_reject_retired_authority_files() {
        for flag in ["--request-file", "--attestation-file"] {
            let parsed = parse_args(&argv(&["workflow", "next", flag, "missing-authority.json"]))
                .expect(
                    "generic parser recognizes retired authority flags for hard-gated commands",
                );
            let error = validate_release_args(&parsed).expect_err("live command must reject flag");
            assert!(error.contains("is not valid for workflow next"), "{error}");
            assert!(error.contains("authorization is retired"), "{error}");
        }
    }

    #[test]
    fn release_upgrade_requires_lowercase_sha256_cas_inputs() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let args = argv(&[
            "workflow",
            "release-upgrade",
            "--target-release-id",
            "release.next",
            "--expected-current-release-digest",
            &digest,
            "--expected-head-digest",
            &digest,
            "--expected-snapshot-digest",
            &digest,
        ]);
        let parsed = parse_args(&args).expect("valid release arguments");
        validate_release_args(&parsed).expect("valid release arguments");

        let uppercase = digest.to_uppercase();
        let invalid = argv(&[
            "workflow",
            "release-upgrade",
            "--target-release-id",
            "release.next",
            "--expected-current-release-digest",
            &uppercase,
            "--expected-head-digest",
            &digest,
            "--expected-snapshot-digest",
            &digest,
        ]);
        let parsed = parse_args(&invalid).expect("shape is validated after parsing");
        assert!(validate_release_args(&parsed).is_err());
    }

    #[test]
    fn release_rebase_commands_accept_only_exact_plan_cas() {
        let digest = format!("sha256:{}", "b".repeat(64));
        for subcommand in ["release-rebase-plan", "release-rebase-apply"] {
            let parsed = parse_args(&argv(&[
                "workflow",
                subcommand,
                "--target-release-id",
                "release.next",
                "--expected-rebase-plan-digest",
                &digest,
            ]))
            .expect("exact rebase arguments");
            validate_release_args(&parsed).expect("valid exact rebase arguments");

            let with_authority = parse_args(&argv(&[
                "workflow",
                subcommand,
                "--target-release-id",
                "release.next",
                "--expected-rebase-plan-digest",
                &digest,
                "--expected-head-digest",
                &digest,
            ]))
            .expect("known but forbidden rebase flag");
            assert!(validate_release_args(&with_authority).is_err());
        }
    }

    #[test]
    fn release_failures_have_typed_exit_reasons() {
        for error in [
            WorkflowGovernanceAdapterError::ReleaseCasMismatch,
            WorkflowGovernanceAdapterError::LedgerIdentityMismatch,
            WorkflowGovernanceAdapterError::ReleaseCommitIndeterminate,
        ] {
            assert_eq!(classify_error(&error), ExitReason::Conflict);
        }
        for error in [
            WorkflowGovernanceAdapterError::UnknownRelease("unknown".to_owned()),
            WorkflowGovernanceAdapterError::ReleaseNotAdjacent,
            WorkflowGovernanceAdapterError::ReleasePolicyDrift,
        ] {
            assert_eq!(classify_error(&error), ExitReason::RejectedByGate);
        }
        assert_eq!(
            classify_error(&WorkflowGovernanceAdapterError::InvalidStateRoot {
                path: PathBuf::from("missing"),
            }),
            ExitReason::EnvConfig
        );
    }

    #[test]
    fn retirement_status_projects_verified_opaque_authority_without_runtime_state() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf();
        let value = retirement_status(&root).expect("verified audit projection");
        assert_eq!(value["authority"], "verified_retirement_checkpoint");
        assert_eq!(
            value["authorization_projection"],
            "non_authoritative_audit_of_opaque_capability"
        );
        assert_eq!(value["verified_retirement_count"], 42);
        assert_eq!(value["operational_workflow_count"], 68);
        assert!(value["payload_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
    }

    #[test]
    fn retirement_status_rejects_authority_selection_flags() {
        let parsed = parse_args(&argv(&[
            "workflow",
            "retirement-status",
            "--target-release-id",
            "attacker-selected",
        ]))
        .expect("generic parser accepts known flag before subcommand policy validation");
        assert!(validate_release_args(&parsed).is_err());
    }
}
