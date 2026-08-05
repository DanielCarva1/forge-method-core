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
use forge_core_contracts::workflow_governance::WorkflowReadinessProfile;
use forge_core_contracts::{
    CliEnvelope, DomainPackCompositionGap, ExitReason, IsolationStatus, PrincipalId,
    ReadinessTarget, StableId, WorkflowCooperativeEvidenceCurrentStatus,
    WorkflowCooperativeEvidenceNonProof, WorkflowCooperativeEvidenceProof,
    WorkflowEffectiveBundleIdentity,
};
use forge_core_decisions::{AgentAutonomyEvaluationError, WorkflowGovernanceSimulation};
use forge_core_kernel::{
    load_admitted_workflow_retirement_checkpoint, WorkflowActiveCooperativeObjective,
    WorkflowAgentAutonomyGuidance, WorkflowAuthorizationGuidance,
    WorkflowCooperativeEvidenceActionPacket, WorkflowDurableAssuranceBlocker,
    WorkflowDurableAssuranceStatus, WorkflowGovernanceAdapterError,
    WorkflowGovernanceBoundaryRecheck, WorkflowGovernanceGuidance,
    WorkflowGovernanceGuidanceAuthority, WorkflowGovernanceGuidanceStatus,
    WorkflowGovernanceProjectAdapter, WorkflowGovernanceReleaseAudit,
    WorkflowReplacementContinuityStatus, WorkflowReplacementDecisionAudit, WorkflowReplacementGap,
    WorkflowReplacementIsolationAudit, WorkflowReplacementObjectiveRevision,
    WorkflowReplacementPromotionAudit, WorkflowReplacementPromotionStatus,
    WorkflowReplacementRankedAction,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
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
    let normalized_profile_args;
    let args = if args.get(1).is_some_and(|value| value == "profile") {
        let Some(action) = args.get(2) else {
            return emit_failure(
                "workflow.profile",
                ExitReason::InvalidDecisionShape,
                "workflow profile requires one of: status, adopt-solo".to_owned(),
                wants_json(args),
            );
        };
        normalized_profile_args = std::iter::once("workflow".to_owned())
            .chain(std::iter::once(format!("profile-{action}")))
            .chain(args.iter().skip(3).cloned())
            .collect::<Vec<_>>();
        normalized_profile_args.as_slice()
    } else {
        args
    };
    if args.get(1).is_some_and(|value| value == "autonomy") {
        return crate::workflow_autonomy_cmd::run(&args[2..]);
    }
    if args.get(1).is_some_and(|value| value == "evidence") {
        return crate::workflow_evidence_cmd::run(&args[2..]);
    }
    if args.get(1).is_some_and(|value| value == "promotion") {
        return crate::workflow_promotion_cmd::run(&args[2..]);
    }
    if args.get(1).is_some_and(|value| value == "intent") {
        let want_json = wants_json(args);
        let command = if args
            .get(2)
            .is_some_and(|value| value == "accept-cooperative")
        {
            "workflow.intent.accept_cooperative"
        } else {
            "workflow.intent"
        };
        return match crate::workflow_intent_cmd::run(&args[2..]) {
            Ok(()) => Ok(()),
            // An empty message is the shared envelope emitter's signal that the
            // subdispatcher already wrote the canonical failure envelope.
            Err(error) if error.message().is_empty() => Err(error),
            Err(error) if want_json => emit_failure(
                command,
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
    let command = if let Some(action) = parsed.subcommand.strip_prefix("profile-") {
        format!("workflow.profile.{}", action.replace('-', "_"))
    } else {
        format!("workflow.{}", parsed.subcommand.replace('-', "_"))
    };
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
            .initialize_with_readiness_profile(
                requested_readiness_profile(&parsed)
                    .expect("readiness profile was validated before adapter dispatch"),
            )
            .map(|value| serde_json::to_value(value).expect("serializable initialization")),
        "next" => adapter
            .next()
            .map(|value| serde_json::to_value(value).expect("serializable guidance")),
        "action-packets" => adapter.action_packets().map(|value| {
            serde_json::to_value(value).expect("serializable workflow action packets")
        }),
        "resume" => adapter.resume().map(|value| {
            serde_json::to_value(workflow_resume_summary(&value))
                .expect("serializable resume summary")
        }),
        "report" => adapter
            .resume()
            .map(|value| serde_json::to_value(value).expect("serializable workflow report")),
        "release-status" => adapter
            .release_status()
            .map(|value| serde_json::to_value(value).expect("serializable release status")),
        "profile-status" => adapter
            .profile_status()
            .map(|value| serde_json::to_value(value).expect("serializable profile status")),
        "profile-adopt-solo" => profile_adopt_solo(&adapter, &parsed),
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

const WORKFLOW_RESUME_SUMMARY_SCHEMA_VERSION: &str = "workflow_resume_summary_v4";

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowResumeSummary<'a> {
    schema_version: &'static str,
    detail_level: &'static str,
    forge_core_version: &'static str,
    authority: WorkflowGovernanceGuidanceAuthority,
    status: WorkflowGovernanceGuidanceStatus,
    readiness_profile: WorkflowReadinessProfile,
    project_id: &'a StableId,
    current_phase: &'a str,
    target: ReadinessTarget,
    snapshot_digest: &'a str,
    ledger_head_digest: &'a str,
    state_version: u64,
    release: &'a WorkflowGovernanceReleaseAudit,
    bundle_id: &'a StableId,
    bundle_digest: &'a str,
    effective: &'a WorkflowEffectiveBundleIdentity,
    selected_policy_ref: &'a StableId,
    compatibility_workflow_id: &'a StableId,
    applicability: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_objective: Option<&'a WorkflowActiveCooperativeObjective>,
    agent_autonomy: &'a WorkflowAgentAutonomyGuidance,
    current_evaluation: &'a WorkflowGovernanceSimulation,
    boundary_rechecks: &'a [WorkflowGovernanceBoundaryRecheck],
    human_decisions: WorkflowResumeHumanDecisionSummary<'a>,
    blockers: WorkflowResumeBlockerSummary<'a>,
    actions: WorkflowResumeActionSummary<'a>,
    active_isolations: Vec<&'a WorkflowReplacementIsolationAudit>,
    recoverable_promotions: Vec<&'a WorkflowReplacementPromotionAudit>,
    current_cooperative_evidence: Vec<WorkflowResumeEvidenceSummary<'a>>,
    authorization: &'a WorkflowAuthorizationGuidance,
    omitted_history: WorkflowResumeOmittedHistory,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowResumeHumanDecisionSummary<'a> {
    recovered_pending: &'a [WorkflowReplacementDecisionAudit],
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowResumeBlockerSummary<'a> {
    domain_pack_degraded: bool,
    domain_pack_gaps: &'a [DomainPackCompositionGap],
    durable_assurance_status: WorkflowDurableAssuranceStatus,
    durable_assurance_blockers: &'a [WorkflowDurableAssuranceBlocker],
    #[serde(skip_serializing_if = "Option::is_none")]
    continuity_status: Option<WorkflowReplacementContinuityStatus>,
    continuity_blockers: Vec<&'a WorkflowReplacementGap>,
    continuity_warnings: Vec<&'a WorkflowReplacementGap>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowResumeActionSummary<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    recommended: Option<WorkflowResumeActionRecommendation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cooperative_evidence_packet: Option<&'a WorkflowCooperativeEvidenceActionPacket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cooperative_evidence_gap: Option<&'a str>,
    continuity_ranked: &'a [WorkflowReplacementRankedAction],
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowResumeActionRecommendation {
    kind: WorkflowResumeActionRecommendationKind,
    action_ref: &'static str,
    reason: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum WorkflowResumeActionRecommendationKind {
    ExecuteCooperativeEvidencePacket,
    ResolveCooperativeEvidenceGap,
    ExecuteContinuityRankedAction,
}

fn recommended_workflow_resume_action(
    has_cooperative_evidence_packet: bool,
    has_cooperative_evidence_gap: bool,
    continuity_ranked: &[WorkflowReplacementRankedAction],
) -> Option<WorkflowResumeActionRecommendation> {
    if has_cooperative_evidence_packet {
        return Some(WorkflowResumeActionRecommendation {
            kind: WorkflowResumeActionRecommendationKind::ExecuteCooperativeEvidencePacket,
            action_ref: "actions.cooperative_evidence_packet",
            reason: "a concrete Solo Cooperative packet is executable before capability acquisition or human escalation",
        });
    }
    if has_cooperative_evidence_gap {
        return Some(WorkflowResumeActionRecommendation {
            kind: WorkflowResumeActionRecommendationKind::ResolveCooperativeEvidenceGap,
            action_ref: "actions.cooperative_evidence_gap",
            reason:
                "the Solo Cooperative route gap must be resolved before abstract fallback actions",
        });
    }
    (!continuity_ranked.is_empty()).then_some(WorkflowResumeActionRecommendation {
        kind: WorkflowResumeActionRecommendationKind::ExecuteContinuityRankedAction,
        action_ref: "actions.continuity_ranked[0]",
        reason: "no concrete Solo Cooperative packet or route gap is currently published",
    })
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowResumeEvidenceSummary<'a> {
    record_digest: &'a str,
    offer_digest: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    supports_cooperative_claim_ref: Option<&'a StableId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    does_not_satisfy_source_claim_ref: Option<&'a StableId>,
    current_status: WorkflowCooperativeEvidenceCurrentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    valid_through_unix: Option<u64>,
    proves: &'a [WorkflowCooperativeEvidenceProof],
    does_not_prove: &'a [WorkflowCooperativeEvidenceNonProof],
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowResumeOmittedHistory {
    superseded_objective_revisions: usize,
    decision_records: usize,
    governed_evidence_records: usize,
    claim_records: usize,
    inactive_isolations: usize,
    non_recoverable_promotions: usize,
    non_current_cooperative_evidence: usize,
}

fn workflow_resume_summary(guidance: &WorkflowGovernanceGuidance) -> WorkflowResumeSummary<'_> {
    let continuity = guidance.replacement_continuity.as_ref();
    let active_isolations = continuity
        .into_iter()
        .flat_map(|continuity| continuity.isolations.iter())
        .filter(|isolation| {
            matches!(
                isolation.contract.status,
                IsolationStatus::Active | IsolationStatus::Merging
            )
        })
        .collect::<Vec<_>>();
    let recoverable_promotions = continuity
        .into_iter()
        .flat_map(|continuity| continuity.promotions.iter())
        .filter(|promotion| promotion.status == WorkflowReplacementPromotionStatus::Recoverable)
        .collect::<Vec<_>>();
    let current_cooperative_evidence = guidance
        .cooperative_evidence
        .iter()
        .filter(|evidence| {
            matches!(
                evidence.current_status,
                WorkflowCooperativeEvidenceCurrentStatus::Supporting
                    | WorkflowCooperativeEvidenceCurrentStatus::Disproving
            )
        })
        .map(|evidence| WorkflowResumeEvidenceSummary {
            record_digest: &evidence.record_digest,
            offer_digest: &evidence.offer_digest,
            supports_cooperative_claim_ref: evidence.supports_cooperative_claim_ref.as_ref(),
            does_not_satisfy_source_claim_ref: evidence.does_not_satisfy_source_claim_ref.as_ref(),
            current_status: evidence.current_status,
            valid_through_unix: evidence.valid_through_unix,
            proves: &evidence.proves,
            does_not_prove: &evidence.does_not_prove,
        })
        .collect::<Vec<_>>();
    let continuity_blockers = continuity
        .into_iter()
        .flat_map(|continuity| continuity.gaps.iter())
        .filter(|gap| gap.blocking)
        .collect::<Vec<_>>();
    let continuity_warnings = continuity
        .into_iter()
        .flat_map(|continuity| continuity.gaps.iter())
        .filter(|gap| !gap.blocking)
        .collect::<Vec<_>>();
    let omitted_history = WorkflowResumeOmittedHistory {
        superseded_objective_revisions: continuity.map_or(0, |value| {
            value
                .objective_history
                .iter()
                .filter(|revision| {
                    !matches!(
                        revision,
                        WorkflowReplacementObjectiveRevision::CooperativeSameOwner {
                            active: true,
                            ..
                        } | WorkflowReplacementObjectiveRevision::HumanIntent { active: true, .. }
                    )
                })
                .count()
        }),
        decision_records: continuity.map_or(0, |value| value.decision_history.len()),
        governed_evidence_records: continuity.map_or(0, |value| value.governed_evidence.len()),
        claim_records: continuity.map_or(0, |value| value.claims.len()),
        inactive_isolations: continuity.map_or(0, |value| {
            value
                .isolations
                .len()
                .saturating_sub(active_isolations.len())
        }),
        non_recoverable_promotions: continuity.map_or(0, |value| {
            value
                .promotions
                .len()
                .saturating_sub(recoverable_promotions.len())
        }),
        non_current_cooperative_evidence: guidance
            .cooperative_evidence
            .len()
            .saturating_sub(current_cooperative_evidence.len()),
    };

    WorkflowResumeSummary {
        schema_version: WORKFLOW_RESUME_SUMMARY_SCHEMA_VERSION,
        detail_level: "summary",
        forge_core_version: env!("CARGO_PKG_VERSION"),
        authority: guidance.authority,
        status: guidance.status,
        readiness_profile: guidance.readiness_profile,
        project_id: &guidance.project_id,
        current_phase: &guidance.current_phase,
        target: guidance.target,
        snapshot_digest: &guidance.snapshot_digest,
        ledger_head_digest: &guidance.ledger_head_digest,
        state_version: guidance.state_version,
        release: &guidance.release,
        bundle_id: &guidance.bundle_id,
        bundle_digest: &guidance.bundle_digest,
        effective: &guidance.effective,
        selected_policy_ref: &guidance.selected_policy_ref,
        compatibility_workflow_id: &guidance.compatibility_workflow_id,
        applicability: guidance.applicability,
        active_objective: guidance.active_cooperative_objective.as_ref(),
        agent_autonomy: &guidance.agent_autonomy,
        current_evaluation: &guidance.simulation,
        boundary_rechecks: &guidance.boundary_rechecks,
        human_decisions: WorkflowResumeHumanDecisionSummary {
            recovered_pending: continuity
                .map_or(&[], |continuity| &continuity.durable_pending_decisions),
        },
        blockers: WorkflowResumeBlockerSummary {
            domain_pack_degraded: guidance.domain_pack_degraded,
            domain_pack_gaps: &guidance.domain_pack_gaps,
            durable_assurance_status: guidance.durable_assurance.status,
            durable_assurance_blockers: &guidance.durable_assurance.blockers,
            continuity_status: continuity.map(|continuity| continuity.status),
            continuity_blockers,
            continuity_warnings,
        },
        actions: {
            let continuity_ranked: &[WorkflowReplacementRankedAction] =
                continuity.map_or(&[], |continuity| continuity.ranked_next_actions.as_slice());
            WorkflowResumeActionSummary {
                recommended: recommended_workflow_resume_action(
                    guidance.cooperative_evidence_action_packet.is_some(),
                    guidance.cooperative_evidence_action_gap.is_some(),
                    continuity_ranked,
                ),
                cooperative_evidence_packet: guidance.cooperative_evidence_action_packet.as_ref(),
                cooperative_evidence_gap: guidance.cooperative_evidence_action_gap.as_deref(),
                continuity_ranked,
            }
        },
        active_isolations,
        recoverable_promotions,
        current_cooperative_evidence,
        authorization: &guidance.authorization,
        omitted_history,
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

fn profile_adopt_solo(
    adapter: &WorkflowGovernanceProjectAdapter,
    args: &WorkflowCliArgs,
) -> Result<Value, WorkflowGovernanceAdapterError> {
    let expected_head_digest =
        required(args, "expected-head-digest").map_err(invalid_observation)?;
    let expected_snapshot_digest =
        required(args, "expected-snapshot-digest").map_err(invalid_observation)?;
    adapter
        .adopt_legacy_solo_profile(&expected_head_digest, &expected_snapshot_digest)
        .map(|value| serde_json::to_value(value).expect("serializable legacy profile adoption"))
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

pub(crate) fn resolve_adapter(root: &Path) -> Result<WorkflowGovernanceProjectAdapter, String> {
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
            | "--expected-snapshot-digest"
            | "--readiness-profile" => {
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

fn requested_readiness_profile(
    args: &WorkflowCliArgs,
) -> Result<Option<WorkflowReadinessProfile>, String> {
    optional(args, "readiness-profile")
        .map(|value| match value.as_str() {
            "solo_cooperative" => Ok(WorkflowReadinessProfile::SoloCooperative),
            "strict_external" => Ok(WorkflowReadinessProfile::StrictExternal),
            _ => Err(
                "--readiness-profile must be one of: solo_cooperative, strict_external".to_owned(),
            ),
        })
        .transpose()
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
    if args.subcommand != "init" && args.flags.contains_key("readiness-profile") {
        return Err(format!(
            "--readiness-profile is valid only for workflow init, not workflow {}",
            args.subcommand
        ));
    }
    match args.subcommand.as_str() {
        "init" => {
            if let Some(flag) = args
                .flags
                .keys()
                .find(|flag| flag.as_str() != "readiness-profile")
            {
                return Err(format!("--{flag} is not valid for workflow init"));
            }
            requested_readiness_profile(args).map(|_| ())
        }
        "action-packets" | "release-status" | "retirement-status" | "profile-status"
            if !args.flags.is_empty() =>
        {
            Err(format!(
                "workflow {} accepts only --root and the JSON output switch",
                args.subcommand
            ))
        }
        "profile-adopt-solo" => {
            let expected = ["expected-head-digest", "expected-snapshot-digest"];
            if let Some(flag) = args
                .flags
                .keys()
                .find(|flag| !expected.contains(&flag.as_str()))
            {
                return Err(format!(
                    "--{flag} is not valid for workflow profile adopt-solo"
                ));
            }
            for name in expected {
                let digest = required(args, name)?;
                if !is_lowercase_sha256(&digest) {
                    return Err(format!(
                        "--{name} must be a canonical lowercase sha256:<64-hex> digest"
                    ));
                }
            }
            Ok(())
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

pub(crate) fn classify_error(error: &WorkflowGovernanceAdapterError) -> ExitReason {
    match error {
        WorkflowGovernanceAdapterError::AgentAutonomyEvaluation(
            AgentAutonomyEvaluationError::StaleBinding,
        )
        | WorkflowGovernanceAdapterError::Ledger(_)
        | WorkflowGovernanceAdapterError::LedgerIdentityMismatch
        | WorkflowGovernanceAdapterError::ReadinessProfileReconfiguration { .. }
        | WorkflowGovernanceAdapterError::LegacySoloAdoptionCasMismatch
        | WorkflowGovernanceAdapterError::LegacySoloAdoptionRetryConflict
        | WorkflowGovernanceAdapterError::CooperativeObjectiveAlreadyAccepted
        | WorkflowGovernanceAdapterError::CooperativeObjectiveRetryConflict
        | WorkflowGovernanceAdapterError::StaleCooperativeObjectiveManagementPacket
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

pub(crate) fn emit_failure(
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
    fn report_is_a_separate_command_and_resume_rejects_the_retired_full_flag() {
        let resume = parse_args(&argv(&["workflow", "resume"])).expect("resume arguments parse");
        validate_release_args(&resume).expect("resume validates");

        let report = parse_args(&argv(&["workflow", "report"])).expect("report arguments parse");
        validate_release_args(&report).expect("report validates");

        let retired = parse_args(&argv(&["workflow", "resume", "--full"]))
            .expect_err("resume no longer exposes a second detail mode");
        assert_eq!(retired, "unrecognized workflow argument '--full'");
    }

    #[test]
    fn resume_recommends_concrete_cooperative_packet_before_abstract_actions() {
        let abstract_actions = [WorkflowReplacementRankedAction {
            rank: 1,
            kind: forge_core_kernel::WorkflowReplacementRankedActionKind::GovernedNext,
            description: "Acquire the missing capability".to_owned(),
            argv: Vec::new(),
            governed_action: None,
        }];
        let recommended = recommended_workflow_resume_action(true, false, &abstract_actions)
            .expect("concrete packet must outrank the abstract action");
        assert_eq!(
            serde_json::to_value(recommended).expect("recommendation JSON"),
            serde_json::json!({
                "kind": "execute_cooperative_evidence_packet",
                "action_ref": "actions.cooperative_evidence_packet",
                "reason": "a concrete Solo Cooperative packet is executable before capability acquisition or human escalation"
            })
        );
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
    fn readiness_profile_selector_is_closed_and_init_only() {
        for (wire, expected) in [
            (
                "solo_cooperative",
                WorkflowReadinessProfile::SoloCooperative,
            ),
            ("strict_external", WorkflowReadinessProfile::StrictExternal),
        ] {
            let parsed = parse_args(&argv(&["workflow", "init", "--readiness-profile", wire]))
                .expect("known profile parses");
            validate_release_args(&parsed).expect("known init profile validates");
            assert_eq!(
                requested_readiness_profile(&parsed).expect("known profile"),
                Some(expected)
            );
        }

        let unknown = parse_args(&argv(&[
            "workflow",
            "init",
            "--readiness-profile",
            "permissive_magic",
        ]))
        .expect("value shape parses before closed-enum validation");
        assert!(validate_release_args(&unknown).is_err());

        let wrong_command = parse_args(&argv(&[
            "workflow",
            "next",
            "--readiness-profile",
            "strict_external",
        ]))
        .expect("known value-bearing flag parses");
        assert!(validate_release_args(&wrong_command).is_err());

        let duplicate = parse_args(&argv(&[
            "workflow",
            "init",
            "--readiness-profile",
            "solo_cooperative",
            "--readiness-profile",
            "strict_external",
        ]))
        .expect_err("profile selector must be single-valued");
        assert_eq!(duplicate, "--readiness-profile may be supplied only once");
    }

    #[test]
    fn nested_profile_commands_preserve_root_with_spaces_and_exact_cas() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let original = argv(&[
            "workflow",
            "profile",
            "adopt-solo",
            "--root",
            "/tmp/project with spaces",
            "--expected-head-digest",
            &digest,
            "--expected-snapshot-digest",
            &digest,
            "--json",
        ]);
        let normalized = std::iter::once("workflow".to_owned())
            .chain(std::iter::once("profile-adopt-solo".to_owned()))
            .chain(original.iter().skip(3).cloned())
            .collect::<Vec<_>>();
        let parsed = parse_args(&normalized).expect("nested profile argv parses");
        validate_release_args(&parsed).expect("exact profile CAS validates");
        assert_eq!(parsed.root, PathBuf::from("/tmp/project with spaces"));
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
            WorkflowGovernanceAdapterError::ReadinessProfileReconfiguration {
                current: WorkflowReadinessProfile::SoloCooperative,
                requested: WorkflowReadinessProfile::StrictExternal,
            },
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
