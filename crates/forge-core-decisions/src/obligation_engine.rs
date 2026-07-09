//! Read-only derivation of an agent-native Assurance Case.
//!
//! The host agent proposes intent, observations, uncertainty signals, and
//! capabilities. This Module applies deterministic cross-domain policy and
//! returns obligations, claims, gaps, decisions, and ranked next actions. It
//! performs no IO and carries no mutation authority.

use forge_core_contracts::{
    AssuranceCase, AssuranceCaseDocument, AssuranceClaim, AssuranceClaimStatus, AssuranceWaiver,
    CapabilityGap, CapabilityGapKind, DecisionAlternative, DecisionRequest, HumanDecisionReason,
    IntentProposal, NextAction, NextActionKind, Obligation, ObligationCriticality,
    ObligationStatus, ProjectSnapshot, ReadinessAssessment, ReadinessTarget, ReadinessVerdict,
    StableId, ASSURANCE_CASE_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub const OBLIGATION_ENGINE_INPUT_SCHEMA_VERSION: &str = "0.1";

/// Versioned input proposed by a host agent for deterministic derivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationEngineInputDocument {
    pub schema_version: String,
    pub obligation_engine_input: ObligationEngineInput,
}

/// Complete read-only input to the first Obligation Engine vertical slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationEngineInput {
    pub case_id: StableId,
    pub intent: IntentProposal,
    pub project_snapshot: ProjectSnapshot,
    pub target: ReadinessTarget,
    pub risk_level: RiskLevel,
    pub lens_observations: Vec<LensObservation>,
    pub epistemic_signals: Vec<EpistemicRiskSignal>,
    pub capability_observations: Vec<CapabilityObservation>,
    pub decision_needs: Vec<DecisionNeed>,
}

/// Ordinal risk used only for proportional assurance policy in this slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    const fn requires_early_independent_challenge(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

/// Cross-domain assurance lenses from the accepted architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UniversalAssuranceLens {
    IntendedOutcome,
    CriticalJourneys,
    SystemIntegrity,
    QualityAttributes,
    Operability,
    LifecycleCoverage,
    RiskAndFailure,
    EvidenceRepresentativeness,
}

impl UniversalAssuranceLens {
    pub const ALL: [Self; 8] = [
        Self::IntendedOutcome,
        Self::CriticalJourneys,
        Self::SystemIntegrity,
        Self::QualityAttributes,
        Self::Operability,
        Self::LifecycleCoverage,
        Self::RiskAndFailure,
        Self::EvidenceRepresentativeness,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::IntendedOutcome => "intended_outcome",
            Self::CriticalJourneys => "critical_journeys",
            Self::SystemIntegrity => "system_integrity",
            Self::QualityAttributes => "quality_attributes",
            Self::Operability => "operability",
            Self::LifecycleCoverage => "lifecycle_coverage",
            Self::RiskAndFailure => "risk_and_failure",
            Self::EvidenceRepresentativeness => "evidence_representativeness",
        }
    }

    const fn statement(self) -> &'static str {
        match self {
            Self::IntendedOutcome => "The integrated result solves the intended human problem.",
            Self::CriticalJourneys => "Representative critical user journeys work end to end.",
            Self::SystemIntegrity => "State, data, effects, authority, and recovery remain coherent.",
            Self::QualityAttributes => "Applicable quality attributes meet explicit expectations.",
            Self::Operability => "The result can be delivered, observed, diagnosed, updated, and recovered as applicable.",
            Self::LifecycleCoverage => "Consequential before, during, and after-use lifecycle concerns are covered.",
            Self::RiskAndFailure => "Unacceptable outcomes have credible prevention, detection, or recovery evidence.",
            Self::EvidenceRepresentativeness => "Readiness evidence exercises the integrated result in a representative environment.",
        }
    }

    const fn action_kind(self) -> NextActionKind {
        match self {
            Self::IntendedOutcome | Self::LifecycleCoverage => NextActionKind::Research,
            Self::RiskAndFailure => NextActionKind::Challenge,
            Self::CriticalJourneys
            | Self::SystemIntegrity
            | Self::QualityAttributes
            | Self::Operability
            | Self::EvidenceRepresentativeness => NextActionKind::Evaluate,
        }
    }
}

/// Whether a universal lens applies to the current project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LensApplicability {
    Applicable,
    NotApplicable,
    Unknown,
}

/// Host observation for one assurance lens. The engine owns policy severity and
/// readiness timing; the host cannot downgrade those fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LensObservation {
    pub lens: UniversalAssuranceLens,
    pub applicability: LensApplicability,
    pub status: AssuranceClaimStatus,
    pub evidence_refs: Vec<String>,
    pub waiver: Option<AssuranceWaiver>,
    pub rationale: Option<String>,
}

/// Signals that make apparent progress epistemically unsafe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicRiskSignal {
    NovelDomain,
    MethodGap,
    SingleView,
    ArtifactOnlyProgress,
    LateIntegration,
    UnverifiedCapability,
    WeakProvenance,
    LongFeedbackGap,
}

/// Availability state for a capability relevant to assurance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailability {
    Available,
    Provisional,
    Missing,
}

/// Evidence-backed capability observation. Missing and provisional capabilities
/// become explicit gaps rather than being hidden by host confidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityObservation {
    pub id: StableId,
    pub kind: CapabilityGapKind,
    pub availability: CapabilityAvailability,
    pub description: String,
    pub affected_lenses: Vec<UniversalAssuranceLens>,
    pub resolution_options: Vec<String>,
}

/// Irreducible human choice proposed by the host agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionNeed {
    pub id: StableId,
    pub question: String,
    pub reason: HumanDecisionReason,
    pub alternatives: Vec<DecisionAlternative>,
    pub recommended_alternative_ref: StableId,
    pub affected_lenses: Vec<UniversalAssuranceLens>,
    pub blocking: bool,
    pub blocks_before: ReadinessTarget,
}

/// Accumulated input issues. Invalid host proposals never produce a partial
/// Assurance Case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationEngineRejection {
    pub issues: Vec<ObligationEngineIssue>,
}

impl std::fmt::Display for ObligationEngineRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Obligation Engine input rejected with {} issue(s)",
            self.issues.len()
        )
    }
}

impl std::error::Error for ObligationEngineRejection {}

/// Typed reasons a host proposal is rejected before derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationEngineIssue {
    UnsupportedSchemaVersion {
        found: String,
    },
    EmptyCaseId,
    EmptyDesiredOutcome,
    DuplicateLensObservation {
        lens: UniversalAssuranceLens,
    },
    IntendedOutcomeNotApplicable,
    NotApplicableWithoutRationale {
        lens: UniversalAssuranceLens,
    },
    EvidenceMissing {
        lens: UniversalAssuranceLens,
    },
    WaiverInconsistent {
        lens: UniversalAssuranceLens,
    },
    DuplicateEpistemicSignal {
        signal: EpistemicRiskSignal,
    },
    EpistemicSignalConflictsWithObservation {
        signal: EpistemicRiskSignal,
        lens: UniversalAssuranceLens,
    },
    EmptyCapabilityId,
    DuplicateCapabilityId {
        id: StableId,
    },
    CapabilityResolutionMissing {
        id: StableId,
    },
    CapabilityAffectedLensMissing {
        id: StableId,
    },
    EmptyDecisionId,
    DuplicateDecisionId {
        id: StableId,
    },
    DecisionAlternativesInvalid {
        id: StableId,
    },
    DecisionAffectedLensMissing {
        id: StableId,
    },
}

#[derive(Debug)]
struct ActionDraft {
    priority: u8,
    stable_key: String,
    action: NextAction,
}

/// Derive one internally coherent Assurance Case without IO or mutation.
///
/// # Errors
///
/// Returns every detected [`ObligationEngineIssue`] when the host proposal is
/// structurally or epistemically inconsistent.
pub fn derive_assurance_case(
    document: &ObligationEngineInputDocument,
) -> Result<AssuranceCaseDocument, ObligationEngineRejection> {
    let issues = validate_input(document);
    if !issues.is_empty() {
        return Err(ObligationEngineRejection { issues });
    }

    let input = &document.obligation_engine_input;
    let observations = input
        .lens_observations
        .iter()
        .map(|observation| (observation.lens, observation))
        .collect::<BTreeMap<_, _>>();
    let signals = input
        .epistemic_signals
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    let mut obligations = Vec::new();
    let mut claims = Vec::new();
    let mut capability_gaps = Vec::new();
    let mut decision_requests = Vec::new();
    let mut actions = Vec::new();

    for lens in UniversalAssuranceLens::ALL {
        let observation = observations.get(&lens).copied();
        if observation.is_some_and(|value| value.applicability == LensApplicability::NotApplicable)
        {
            continue;
        }

        let status = observation.map_or(AssuranceClaimStatus::Unknown, |value| value.status);
        let evidence_refs = observation.map_or_else(Vec::new, |value| value.evidence_refs.clone());
        let waiver = observation.and_then(|value| value.waiver.clone());
        let (criticality, required_before) = lens_policy(lens, input.risk_level);
        let claim_id = claim_id_for_lens(lens);
        let obligation_id = obligation_id_for_lens(lens);
        let obligation_status = obligation_status_for_claim(status);

        claims.push(AssuranceClaim {
            id: claim_id.clone(),
            statement: lens.statement().to_owned(),
            status,
            evidence_refs,
            waiver,
        });
        obligations.push(Obligation {
            id: obligation_id.clone(),
            description: format!("Satisfy the {} assurance lens.", lens.id()),
            criticality,
            status: obligation_status,
            required_before,
            claim_refs: vec![claim_id.clone()],
        });
        if !status.satisfies_obligation() {
            actions.push(action_for_lens(
                lens,
                &claim_id,
                criticality,
                required_before,
                input.target,
            ));
        }
    }

    apply_epistemic_signals(
        input,
        &signals,
        &mut obligations,
        &mut claims,
        &mut capability_gaps,
        &mut actions,
    );
    apply_capability_observations(input, &observations, &mut capability_gaps, &mut actions);
    apply_decision_needs(input, &observations, &mut decision_requests, &mut actions);

    let blocker_refs = readiness_blockers(
        input.target,
        &obligations,
        &capability_gaps,
        &decision_requests,
    );
    let verdict = if blocker_refs.is_empty() {
        ReadinessVerdict::Ready
    } else {
        ReadinessVerdict::Blocked
    };

    if verdict == ReadinessVerdict::Ready {
        let mut addresses_claim_refs = claims
            .iter()
            .filter(|claim| claim.status.satisfies_obligation())
            .map(|claim| claim.id.clone())
            .collect::<Vec<_>>();
        if addresses_claim_refs.is_empty() {
            addresses_claim_refs.extend(claims.first().map(|claim| claim.id.clone()));
        }
        actions.push(ActionDraft {
            priority: 0,
            stable_key: "proceed".to_owned(),
            action: NextAction {
                id: StableId(format!("action.proceed.{}", target_label(input.target))),
                kind: NextActionKind::Proceed,
                description: format!("Proceed with the {:?} readiness target.", input.target),
                addresses_claim_refs,
                rationale:
                    "No due required obligation, decision, or capability gap blocks the target."
                        .to_owned(),
                rank: 0,
            },
        });
    }

    actions.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.stable_key.cmp(&right.stable_key))
    });
    let next_actions = actions
        .into_iter()
        .enumerate()
        .map(|(index, mut draft)| {
            draft.action.rank = u32::try_from(index + 1).unwrap_or(u32::MAX);
            draft.action
        })
        .collect();

    Ok(AssuranceCaseDocument {
        schema_version: ASSURANCE_CASE_SCHEMA_VERSION.to_owned(),
        assurance_case: AssuranceCase {
            id: input.case_id.clone(),
            intent: input.intent.clone(),
            project_snapshot: input.project_snapshot.clone(),
            obligations,
            claims,
            decision_requests,
            capability_gaps,
            next_actions,
            readiness: ReadinessAssessment {
                target: input.target,
                verdict,
                blocker_refs,
                rationale: readiness_rationale(input.target, verdict),
            },
        },
    })
}

const fn target_label(target: ReadinessTarget) -> &'static str {
    match target {
        ReadinessTarget::Explore => "explore",
        ReadinessTarget::Execute => "execute",
        ReadinessTarget::Release => "release",
    }
}

fn validate_input(document: &ObligationEngineInputDocument) -> Vec<ObligationEngineIssue> {
    let mut issues = Vec::new();
    if document.schema_version != OBLIGATION_ENGINE_INPUT_SCHEMA_VERSION {
        issues.push(ObligationEngineIssue::UnsupportedSchemaVersion {
            found: document.schema_version.clone(),
        });
    }
    let input = &document.obligation_engine_input;
    if input.case_id.0.trim().is_empty() {
        issues.push(ObligationEngineIssue::EmptyCaseId);
    }
    if input.intent.desired_outcome.trim().is_empty() {
        issues.push(ObligationEngineIssue::EmptyDesiredOutcome);
    }

    let mut lenses = BTreeSet::new();
    for observation in &input.lens_observations {
        if !lenses.insert(observation.lens) {
            issues.push(ObligationEngineIssue::DuplicateLensObservation {
                lens: observation.lens,
            });
        }
        if observation.lens == UniversalAssuranceLens::IntendedOutcome
            && observation.applicability == LensApplicability::NotApplicable
        {
            issues.push(ObligationEngineIssue::IntendedOutcomeNotApplicable);
        }
        if observation.applicability == LensApplicability::NotApplicable
            && observation.rationale.as_deref().is_none_or(str::is_empty)
        {
            issues.push(ObligationEngineIssue::NotApplicableWithoutRationale {
                lens: observation.lens,
            });
        }
        if matches!(
            observation.status,
            AssuranceClaimStatus::Supported
                | AssuranceClaimStatus::Verified
                | AssuranceClaimStatus::Disproven
        ) && observation.evidence_refs.is_empty()
        {
            issues.push(ObligationEngineIssue::EvidenceMissing {
                lens: observation.lens,
            });
        }
        match (observation.status, &observation.waiver) {
            (AssuranceClaimStatus::Waived, Some(waiver))
                if !waiver.authorized_by.0.trim().is_empty()
                    && !waiver.reason.trim().is_empty()
                    && !waiver.consequences.is_empty() => {}
            (AssuranceClaimStatus::Waived, _) | (_, Some(_)) => {
                issues.push(ObligationEngineIssue::WaiverInconsistent {
                    lens: observation.lens,
                });
            }
            (_, None) => {}
        }
    }

    let mut signals = BTreeSet::new();
    for signal in &input.epistemic_signals {
        if !signals.insert(*signal) {
            issues.push(ObligationEngineIssue::DuplicateEpistemicSignal { signal: *signal });
        }
    }
    for signal in [
        EpistemicRiskSignal::ArtifactOnlyProgress,
        EpistemicRiskSignal::LateIntegration,
        EpistemicRiskSignal::LongFeedbackGap,
    ] {
        if signals.contains(&signal)
            && input.lens_observations.iter().any(|observation| {
                observation.lens == UniversalAssuranceLens::EvidenceRepresentativeness
                    && observation.status.satisfies_obligation()
            })
        {
            issues.push(
                ObligationEngineIssue::EpistemicSignalConflictsWithObservation {
                    signal,
                    lens: UniversalAssuranceLens::EvidenceRepresentativeness,
                },
            );
        }
    }

    let mut capability_ids = HashSet::new();
    for capability in &input.capability_observations {
        if capability.id.0.trim().is_empty() {
            issues.push(ObligationEngineIssue::EmptyCapabilityId);
        } else if !capability_ids.insert(capability.id.0.as_str()) {
            issues.push(ObligationEngineIssue::DuplicateCapabilityId {
                id: capability.id.clone(),
            });
        }
        if capability.availability != CapabilityAvailability::Available
            && capability.resolution_options.is_empty()
        {
            issues.push(ObligationEngineIssue::CapabilityResolutionMissing {
                id: capability.id.clone(),
            });
        }
        if capability.availability != CapabilityAvailability::Available
            && (capability.affected_lenses.is_empty()
                || capability
                    .affected_lenses
                    .iter()
                    .all(|lens| lens_is_not_applicable(*lens, &input.lens_observations)))
        {
            issues.push(ObligationEngineIssue::CapabilityAffectedLensMissing {
                id: capability.id.clone(),
            });
        }
    }

    let mut decision_ids = HashSet::new();
    for decision in &input.decision_needs {
        if decision.id.0.trim().is_empty() {
            issues.push(ObligationEngineIssue::EmptyDecisionId);
        } else if !decision_ids.insert(decision.id.0.as_str()) {
            issues.push(ObligationEngineIssue::DuplicateDecisionId {
                id: decision.id.clone(),
            });
        }
        let alternatives = decision
            .alternatives
            .iter()
            .map(|alternative| alternative.id.0.as_str())
            .collect::<HashSet<_>>();
        if alternatives.len() < 2
            || !alternatives.contains(decision.recommended_alternative_ref.0.as_str())
        {
            issues.push(ObligationEngineIssue::DecisionAlternativesInvalid {
                id: decision.id.clone(),
            });
        }
        if decision.affected_lenses.is_empty()
            || decision
                .affected_lenses
                .iter()
                .all(|lens| lens_is_not_applicable(*lens, &input.lens_observations))
        {
            issues.push(ObligationEngineIssue::DecisionAffectedLensMissing {
                id: decision.id.clone(),
            });
        }
    }
    issues
}

fn lens_is_not_applicable(lens: UniversalAssuranceLens, observations: &[LensObservation]) -> bool {
    observations.iter().any(|observation| {
        observation.lens == lens && observation.applicability == LensApplicability::NotApplicable
    })
}

fn lens_policy(
    lens: UniversalAssuranceLens,
    risk: RiskLevel,
) -> (ObligationCriticality, ReadinessTarget) {
    match lens {
        UniversalAssuranceLens::IntendedOutcome => {
            (ObligationCriticality::Critical, ReadinessTarget::Execute)
        }
        UniversalAssuranceLens::LifecycleCoverage => {
            (ObligationCriticality::Required, ReadinessTarget::Execute)
        }
        UniversalAssuranceLens::RiskAndFailure if risk.requires_early_independent_challenge() => {
            (ObligationCriticality::Critical, ReadinessTarget::Execute)
        }
        UniversalAssuranceLens::CriticalJourneys
        | UniversalAssuranceLens::SystemIntegrity
        | UniversalAssuranceLens::RiskAndFailure
        | UniversalAssuranceLens::EvidenceRepresentativeness => {
            (ObligationCriticality::Critical, ReadinessTarget::Release)
        }
        UniversalAssuranceLens::QualityAttributes | UniversalAssuranceLens::Operability => {
            (ObligationCriticality::Required, ReadinessTarget::Release)
        }
    }
}

const fn obligation_status_for_claim(status: AssuranceClaimStatus) -> ObligationStatus {
    match status {
        AssuranceClaimStatus::Verified | AssuranceClaimStatus::Waived => {
            ObligationStatus::Satisfied
        }
        AssuranceClaimStatus::Disproven => ObligationStatus::Blocked,
        AssuranceClaimStatus::Unknown
        | AssuranceClaimStatus::Hypothesized
        | AssuranceClaimStatus::Supported => ObligationStatus::Pending,
    }
}

fn claim_id_for_lens(lens: UniversalAssuranceLens) -> StableId {
    StableId(format!("claim.assurance.{}", lens.id()))
}

fn obligation_id_for_lens(lens: UniversalAssuranceLens) -> StableId {
    StableId(format!("obligation.assurance.{}", lens.id()))
}

fn action_for_lens(
    lens: UniversalAssuranceLens,
    claim_id: &StableId,
    criticality: ObligationCriticality,
    required_before: ReadinessTarget,
    target: ReadinessTarget,
) -> ActionDraft {
    ActionDraft {
        priority: action_priority(criticality, required_before, target),
        stable_key: lens.id().to_owned(),
        action: NextAction {
            id: StableId(format!("action.assurance.{}", lens.id())),
            kind: lens.action_kind(),
            description: format!("Acquire representative evidence for {}.", lens.id()),
            addresses_claim_refs: vec![claim_id.clone()],
            rationale: format!(
                "The {} Assurance Claim is not yet verified or waived.",
                lens.id()
            ),
            rank: 0,
        },
    }
}

const fn action_priority(
    criticality: ObligationCriticality,
    required_before: ReadinessTarget,
    target: ReadinessTarget,
) -> u8 {
    if required_before.rank() <= target.rank() {
        match criticality {
            ObligationCriticality::Critical => 1,
            ObligationCriticality::Required => 2,
            ObligationCriticality::Advisory => 3,
        }
    } else {
        20 + required_before.rank()
    }
}

fn apply_epistemic_signals(
    input: &ObligationEngineInput,
    signals: &BTreeSet<EpistemicRiskSignal>,
    obligations: &mut Vec<Obligation>,
    claims: &mut Vec<AssuranceClaim>,
    gaps: &mut Vec<CapabilityGap>,
    actions: &mut Vec<ActionDraft>,
) {
    let domain_gap = signals.contains(&EpistemicRiskSignal::NovelDomain)
        || signals.contains(&EpistemicRiskSignal::MethodGap)
        || signals.contains(&EpistemicRiskSignal::WeakProvenance);
    if domain_gap {
        add_special_requirement(
            "domain_method_is_credible",
            "The selected domain method has credible lifecycle, failure-mode, and quality coverage.",
            ObligationCriticality::Critical,
            ReadinessTarget::Execute,
            NextActionKind::Research,
            input.target,
            obligations,
            claims,
            actions,
        );
        let has_domain_capability_observation =
            input.capability_observations.iter().any(|capability| {
                matches!(
                    capability.kind,
                    CapabilityGapKind::DomainPack | CapabilityGapKind::Knowledge
                )
            });
        if !has_domain_capability_observation {
            let claim_ref = StableId("claim.assurance.domain_method_is_credible".to_owned());
            gaps.push(CapabilityGap {
                id: StableId("gap.assurance.domain_competence".to_owned()),
                kind: CapabilityGapKind::DomainPack,
                description: "No verified domain method or Domain Pack is available.".to_owned(),
                affected_claim_refs: vec![claim_ref.clone()],
                resolution_options: vec![
                    "Research multiple credible domain methods with provenance.".to_owned(),
                    "Acquire a reviewed Domain Pack or specialist evaluator.".to_owned(),
                ],
                blocking: true,
                blocks_before: ReadinessTarget::Execute,
            });
            actions.push(ActionDraft {
                priority: action_priority(
                    ObligationCriticality::Critical,
                    ReadinessTarget::Execute,
                    input.target,
                ),
                stable_key: "resolve_domain_competence".to_owned(),
                action: NextAction {
                    id: StableId("action.assurance.resolve_domain_competence".to_owned()),
                    kind: NextActionKind::AcquireCapability,
                    description: "Acquire a credible domain method, pack, or evaluator.".to_owned(),
                    addresses_claim_refs: vec![claim_ref],
                    rationale:
                        "Shared human and agent ignorance must be managed before durable execution."
                            .to_owned(),
                    rank: 0,
                },
            });
        }
    }

    if signals.contains(&EpistemicRiskSignal::SingleView) {
        add_special_requirement(
            "assurance_case_survives_independent_challenge",
            "An independent challenge finds no consequential unsupported claim.",
            ObligationCriticality::Critical,
            ReadinessTarget::Release,
            NextActionKind::Challenge,
            input.target,
            obligations,
            claims,
            actions,
        );
    }

    if signals.contains(&EpistemicRiskSignal::UnverifiedCapability) {
        add_special_requirement(
            "critical_capabilities_are_verified",
            "Capabilities used by the plan have representative verification evidence.",
            ObligationCriticality::Critical,
            ReadinessTarget::Release,
            NextActionKind::Evaluate,
            input.target,
            obligations,
            claims,
            actions,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn add_special_requirement(
    suffix: &str,
    statement: &str,
    criticality: ObligationCriticality,
    required_before: ReadinessTarget,
    action_kind: NextActionKind,
    target: ReadinessTarget,
    obligations: &mut Vec<Obligation>,
    claims: &mut Vec<AssuranceClaim>,
    actions: &mut Vec<ActionDraft>,
) {
    let claim_id = StableId(format!("claim.assurance.{suffix}"));
    claims.push(AssuranceClaim {
        id: claim_id.clone(),
        statement: statement.to_owned(),
        status: AssuranceClaimStatus::Unknown,
        evidence_refs: Vec::new(),
        waiver: None,
    });
    obligations.push(Obligation {
        id: StableId(format!("obligation.assurance.{suffix}")),
        description: format!("Satisfy the {suffix} assurance requirement."),
        criticality,
        status: ObligationStatus::Pending,
        required_before,
        claim_refs: vec![claim_id.clone()],
    });
    actions.push(ActionDraft {
        priority: action_priority(criticality, required_before, target),
        stable_key: suffix.to_owned(),
        action: NextAction {
            id: StableId(format!("action.assurance.{suffix}")),
            kind: action_kind,
            description: format!("Resolve the {suffix} Assurance Claim."),
            addresses_claim_refs: vec![claim_id],
            rationale: "An epistemic-risk signal introduced this requirement.".to_owned(),
            rank: 0,
        },
    });
}

fn apply_capability_observations(
    input: &ObligationEngineInput,
    observations: &BTreeMap<UniversalAssuranceLens, &LensObservation>,
    gaps: &mut Vec<CapabilityGap>,
    actions: &mut Vec<ActionDraft>,
) {
    for capability in &input.capability_observations {
        if capability.availability == CapabilityAvailability::Available {
            continue;
        }
        let affected_claim_refs = capability
            .affected_lenses
            .iter()
            .filter(|lens| {
                !observations
                    .get(lens)
                    .is_some_and(|value| value.applicability == LensApplicability::NotApplicable)
            })
            .map(|lens| claim_id_for_lens(*lens))
            .collect::<Vec<_>>();
        let blocks_before = match capability.availability {
            CapabilityAvailability::Provisional => ReadinessTarget::Release,
            CapabilityAvailability::Missing => match capability.kind {
                CapabilityGapKind::Agent | CapabilityGapKind::Evaluator => ReadinessTarget::Release,
                CapabilityGapKind::Tool
                | CapabilityGapKind::Environment
                | CapabilityGapKind::Knowledge
                | CapabilityGapKind::Authority
                | CapabilityGapKind::DomainPack => ReadinessTarget::Execute,
            },
            CapabilityAvailability::Available => continue,
        };
        let gap_id = StableId(format!("gap.assurance.{}", capability.id.0));
        gaps.push(CapabilityGap {
            id: gap_id,
            kind: capability.kind,
            description: capability.description.clone(),
            affected_claim_refs: affected_claim_refs.clone(),
            resolution_options: capability.resolution_options.clone(),
            blocking: true,
            blocks_before,
        });
        actions.push(ActionDraft {
            priority: action_priority(ObligationCriticality::Critical, blocks_before, input.target),
            stable_key: format!("capability.{}", capability.id.0),
            action: NextAction {
                id: StableId(format!("action.assurance.resolve.{}", capability.id.0)),
                kind: NextActionKind::AcquireCapability,
                description: format!("Resolve capability gap {}.", capability.id.0),
                addresses_claim_refs: affected_claim_refs,
                rationale: "The current capability is missing or only provisional.".to_owned(),
                rank: 0,
            },
        });
    }
}

fn apply_decision_needs(
    input: &ObligationEngineInput,
    observations: &BTreeMap<UniversalAssuranceLens, &LensObservation>,
    requests: &mut Vec<DecisionRequest>,
    actions: &mut Vec<ActionDraft>,
) {
    for decision in &input.decision_needs {
        let affected_claim_refs = decision
            .affected_lenses
            .iter()
            .filter(|lens| {
                !observations
                    .get(lens)
                    .is_some_and(|value| value.applicability == LensApplicability::NotApplicable)
            })
            .map(|lens| claim_id_for_lens(*lens))
            .collect::<Vec<_>>();
        requests.push(DecisionRequest {
            id: decision.id.clone(),
            question: decision.question.clone(),
            reason: decision.reason,
            alternatives: decision.alternatives.clone(),
            recommended_alternative_ref: decision.recommended_alternative_ref.clone(),
            blocking: decision.blocking,
            blocks_before: decision.blocks_before,
        });
        actions.push(ActionDraft {
            priority: action_priority(
                ObligationCriticality::Required,
                decision.blocks_before,
                input.target,
            ),
            stable_key: format!("decision.{}", decision.id.0),
            action: NextAction {
                id: StableId(format!("action.assurance.decide.{}", decision.id.0)),
                kind: NextActionKind::AskHuman,
                description: decision.question.clone(),
                addresses_claim_refs: affected_claim_refs,
                rationale: "Project evidence cannot resolve this human value or authority choice."
                    .to_owned(),
                rank: 0,
            },
        });
    }
}

fn readiness_blockers(
    target: ReadinessTarget,
    obligations: &[Obligation],
    gaps: &[CapabilityGap],
    requests: &[DecisionRequest],
) -> Vec<StableId> {
    let mut blockers = obligations
        .iter()
        .filter(|obligation| {
            obligation.criticality != ObligationCriticality::Advisory
                && obligation.required_before.rank() <= target.rank()
                && obligation.status != ObligationStatus::Satisfied
        })
        .map(|obligation| obligation.id.clone())
        .chain(
            gaps.iter()
                .filter(|gap| gap.blocking && gap.blocks_before.rank() <= target.rank())
                .map(|gap| gap.id.clone()),
        )
        .chain(
            requests
                .iter()
                .filter(|request| request.blocking && request.blocks_before.rank() <= target.rank())
                .map(|request| request.id.clone()),
        )
        .collect::<Vec<_>>();
    blockers.sort_by(|left, right| left.0.cmp(&right.0));
    blockers.dedup();
    blockers
}

fn readiness_rationale(target: ReadinessTarget, verdict: ReadinessVerdict) -> String {
    match verdict {
        ReadinessVerdict::Ready => format!(
            "No due required obligation, Decision Request, or Capability Gap blocks {target:?}."
        ),
        ReadinessVerdict::Blocked => {
            format!("One or more due obligations, decisions, or capabilities block {target:?}.")
        }
    }
}
