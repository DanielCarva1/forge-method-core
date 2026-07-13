//! Pure reconstruction of durable Assurance state from admitted workflow events.
//!
//! Unlike the proposal-only Obligation Engine, this projector accepts no
//! caller-authored claim status, readiness verdict, or evaluator. An accepted
//! human intent opens a new assurance epoch with every universal lens unknown.

use forge_core_contracts::{
    DurableAssuranceCapabilityBinding, DurableAssuranceClaimBinding,
    DurableAssuranceDecisionBinding, DurableAssuranceEpistemicState, DurableAssuranceEpochBinding,
    DurableAssuranceEvidenceBinding, DurableAssuranceLensProjection, DurableAssuranceNextAction,
    DurableAssuranceProjection, DurableAssuranceReadinessState, DurableAssuranceWaiverBinding,
    HumanIntentRevisionAcceptedEvent, NextActionKind, ObligationCriticality, PrincipalId,
    ReadinessTarget, StableId, UniversalAssuranceLens, WorkflowAssuranceClaimRole,
    WorkflowBrokerOriginProfile, WorkflowEvaluatorProvider, WorkflowEvidenceKind,
    WorkflowEvidenceOutcome, WorkflowEvidenceStrength, WorkflowEvidenceSubjectKind,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceEvent, WorkflowGovernanceLedgerRecord,
    WorkflowHumanIntentRevision, WorkflowRepresentativeSliceDefinitionDocument,
    MAX_DURABLE_ASSURANCE_NEXT_ACTIONS, MAX_REPRESENTATIVE_SLICE_ITEMS,
    MAX_REPRESENTATIVE_SLICE_ITEM_BYTES, MAX_REPRESENTATIVE_SLICE_TEXT_BYTES,
    MAX_REPRESENTATIVE_SLICE_TOTAL_BYTES, MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES,
    MAX_WORKFLOW_INTENT_ITEM_BYTES, MAX_WORKFLOW_INTENT_LIST_ITEMS,
    MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES, MAX_WORKFLOW_INTENT_TOTAL_BYTES,
    WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Defensive rejection from intent canonicalization or durable projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssuranceProjectionError {
    pub issues: Vec<AssuranceProjectionIssue>,
}

impl std::fmt::Display for AssuranceProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "durable Assurance projection rejected with {} issue(s)",
            self.issues.len()
        )
    }
}

impl std::error::Error for AssuranceProjectionError {}

/// Stable issue codes suitable for a typed adapter failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AssuranceProjectionIssue {
    EmptyField { field: String },
    FieldTooLarge { field: String, maximum_bytes: usize },
    TooManyItems { field: String, maximum_items: usize },
    AggregateIntentTooLarge { maximum_bytes: usize },
    InvalidDigest { field: String },
    InvalidInitialEpoch { found: u64 },
    InvalidInitialRevision { found: u64 },
    NonMonotonicEpoch { expected: u64, found: u64 },
    NonMonotonicRevision { expected: u64, found: u64 },
    IntentIdentityChanged,
    PreviousIntentDigestMismatch,
    IntentDigestMismatch,
    LedgerHeadBindingMismatch,
    AcceptanceTimeMismatch,
    MissingOriginCompanion,
    OriginBindingMismatch,
    ProjectIdentityChanged,
    CounterOverflow,
    ProjectionEncodingFailed,
    RepresentativeSliceInvalid { field: String },
}

/// Validate and canonically digest one kernel-assigned human intent revision.
///
/// The digest uses RFC 8785-style canonical JSON bytes and a lower-case
/// `sha256:` wire value. Validation is intentionally shared by the kernel and
/// projector so oversized or structurally ambiguous content fails closed.
///
/// # Errors
///
/// Returns an assurance projection error when the intent is malformed,
/// outside the bounded contract, or cannot be canonicalized.
pub fn workflow_human_intent_digest(
    intent: &WorkflowHumanIntentRevision,
) -> Result<String, AssuranceProjectionError> {
    let issues = validate_intent(intent);
    if !issues.is_empty() {
        return Err(AssuranceProjectionError { issues });
    }
    canonical_digest(intent)
}

/// Reconstruct the latest durable Assurance epoch from governance history.
///
/// `None` means no human intent has yet been admitted. Every successful
/// projection contains each universal lens exactly once in canonical order,
/// with unknown status, no evidence, no evaluator, and unknown readiness.
///
/// # Errors
///
/// Returns an assurance projection error when intent history is malformed,
/// non-monotonic, or lacks the exact adjacent broker-origin binding.
pub fn project_durable_assurance(
    records: &[WorkflowGovernanceLedgerRecord],
) -> Result<Option<DurableAssuranceProjection>, AssuranceProjectionError> {
    let mut previous: Option<(
        &HumanIntentRevisionAcceptedEvent,
        &WorkflowGovernanceLedgerRecord,
    )> = None;
    let mut issues = Vec::new();

    for (index, record) in records.iter().enumerate() {
        let WorkflowGovernanceEvent::HumanIntentRevisionAccepted(event) = &record.event else {
            continue;
        };

        issues.extend(validate_intent(&event.intent));
        for (field, digest) in [
            ("intent_digest", event.intent_digest.as_str()),
            ("snapshot_digest", event.snapshot_digest.as_str()),
            (
                "acceptance_action_packet_digest",
                event.acceptance_action_packet_digest.as_str(),
            ),
            ("record_digest", record.record_digest.as_str()),
        ] {
            if !is_sha256_digest(digest) {
                issues.push(AssuranceProjectionIssue::InvalidDigest {
                    field: field.to_owned(),
                });
            }
        }
        if event.accepted_by.0.trim().is_empty() {
            issues.push(AssuranceProjectionIssue::EmptyField {
                field: "accepted_by".to_owned(),
            });
        }
        if record.previous_record_digest.as_deref() != Some(event.ledger_head_digest.as_str()) {
            issues.push(AssuranceProjectionIssue::LedgerHeadBindingMismatch);
        }
        if event.accepted_at_unix != record.recorded_at_unix {
            issues.push(AssuranceProjectionIssue::AcceptanceTimeMismatch);
        }
        match records.get(index + 1) {
            Some(companion_record) => match &companion_record.event {
                WorkflowGovernanceEvent::BrokerOriginApplied(companion) => {
                    if companion.action_record_digest != record.record_digest
                        || companion.action_packet_digest != event.acceptance_action_packet_digest
                        || companion.origin_principal_id != event.accepted_by
                        || companion.issuer_profile != WorkflowBrokerOriginProfile::Human
                        || companion.issued_at_unix != event.accepted_at_unix
                        || companion_record.project_id != record.project_id
                        || companion_record.previous_record_digest.as_deref()
                            != Some(record.record_digest.as_str())
                    {
                        issues.push(AssuranceProjectionIssue::OriginBindingMismatch);
                    }
                }
                _ => issues.push(AssuranceProjectionIssue::MissingOriginCompanion),
            },
            None => issues.push(AssuranceProjectionIssue::MissingOriginCompanion),
        }
        match workflow_human_intent_digest(&event.intent) {
            Ok(digest) if digest == event.intent_digest => {}
            Ok(_) => issues.push(AssuranceProjectionIssue::IntentDigestMismatch),
            Err(_) => {}
        }

        match previous {
            None => {
                if event.assurance_epoch != 1 {
                    issues.push(AssuranceProjectionIssue::InvalidInitialEpoch {
                        found: event.assurance_epoch,
                    });
                }
                if event.intent.revision != 1 {
                    issues.push(AssuranceProjectionIssue::InvalidInitialRevision {
                        found: event.intent.revision,
                    });
                }
                if event.previous_intent_digest.is_some() {
                    issues.push(AssuranceProjectionIssue::PreviousIntentDigestMismatch);
                }
            }
            Some((prior_event, prior_record)) => {
                if prior_record.project_id != record.project_id {
                    issues.push(AssuranceProjectionIssue::ProjectIdentityChanged);
                }
                match prior_event.assurance_epoch.checked_add(1) {
                    Some(expected) if event.assurance_epoch == expected => {}
                    Some(expected) => issues.push(AssuranceProjectionIssue::NonMonotonicEpoch {
                        expected,
                        found: event.assurance_epoch,
                    }),
                    None => issues.push(AssuranceProjectionIssue::CounterOverflow),
                }
                match prior_event.intent.revision.checked_add(1) {
                    Some(expected) if event.intent.revision == expected => {}
                    Some(expected) => {
                        issues.push(AssuranceProjectionIssue::NonMonotonicRevision {
                            expected,
                            found: event.intent.revision,
                        });
                    }
                    None => issues.push(AssuranceProjectionIssue::CounterOverflow),
                }
                if event.intent.intent_id != prior_event.intent.intent_id {
                    issues.push(AssuranceProjectionIssue::IntentIdentityChanged);
                }
                if event.previous_intent_digest.as_deref()
                    != Some(prior_event.intent_digest.as_str())
                {
                    issues.push(AssuranceProjectionIssue::PreviousIntentDigestMismatch);
                }
            }
        }
        previous = Some((event, record));
    }

    if !issues.is_empty() {
        return Err(AssuranceProjectionError { issues });
    }

    let Some((event, record)) = previous else {
        return Ok(None);
    };
    let lenses = UniversalAssuranceLens::ALL
        .into_iter()
        .map(|lens| DurableAssuranceLensProjection {
            lens,
            claim_status: DurableAssuranceEpistemicState::Unknown,
            required_before: ReadinessTarget::Release,
            due: true,
            claims: Vec::new(),
            evidence: Vec::new(),
            capabilities: Vec::new(),
            decisions: Vec::new(),
            waivers: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut projection = DurableAssuranceProjection {
        binding: DurableAssuranceEpochBinding {
            project_id: record.project_id.clone(),
            assurance_epoch: event.assurance_epoch,
            intent_id: event.intent.intent_id.clone(),
            intent_revision: event.intent.revision,
            intent_digest: event.intent_digest.clone(),
            accepted_record_digest: record.record_digest.clone(),
            accepted_sequence: record.sequence,
            accepted_state_version: record.state_version,
            snapshot_digest: event.snapshot_digest.clone(),
            ledger_head_before_acceptance: event.ledger_head_digest.clone(),
        },
        intent: event.intent.clone(),
        lenses,
        readiness: DurableAssuranceReadinessState::Unknown,
        blocker_lenses: UniversalAssuranceLens::ALL.to_vec(),
        next_actions: Vec::new(),
        projection_digest: String::new(),
    };
    projection.projection_digest = canonical_digest(&DurableAssuranceProjectionDigestSubject {
        binding: &projection.binding,
        intent: &projection.intent,
        lenses: &projection.lenses,
        readiness: projection.readiness,
        blocker_lenses: &projection.blocker_lenses,
        next_actions: &projection.next_actions,
    })?;
    Ok(Some(projection))
}

#[derive(Serialize)]
struct DurableAssuranceProjectionDigestSubject<'a> {
    binding: &'a DurableAssuranceEpochBinding,
    intent: &'a WorkflowHumanIntentRevision,
    lenses: &'a [DurableAssuranceLensProjection],
    readiness: DurableAssuranceReadinessState,
    blocker_lenses: &'a [UniversalAssuranceLens],
    next_actions: &'a [DurableAssuranceNextAction],
}

/// Evidence fact already authenticated and freshness-checked by the kernel.
/// It contains no caller-selected claim status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedAssuranceEvidenceFact {
    pub assurance_epoch: u64,
    pub sequence: u64,
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub evidence_ref: String,
    pub evidence_record_digest: String,
    pub origin_record_digest: String,
    pub provider: WorkflowEvaluatorProvider,
    pub kind: WorkflowEvidenceKind,
    pub strength: WorkflowEvidenceStrength,
    pub outcome: WorkflowEvidenceOutcome,
    pub subject_kind: WorkflowEvidenceSubjectKind,
    pub subject_ref: String,
    pub subject_digest: String,
    pub scenario_digest: String,
    pub origin_principal: PrincipalId,
    pub separation_domain: StableId,
    pub broker_profile: WorkflowBrokerOriginProfile,
    pub representative_slice: Option<WorkflowRepresentativeSliceDefinitionDocument>,
    /// Exact content digest of the independently reviewed definition. For the
    /// definition receipt this equals `subject_digest`; for runtime receipts it
    /// is derived by the kernel from the accepted definition, never from caller
    /// input. Ordinary lens evidence carries `None`.
    pub representative_slice_definition_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedAssuranceCapabilityFact {
    pub assurance_epoch: u64,
    pub sequence: u64,
    pub policy_ref: StableId,
    pub capability_ref: StableId,
    pub available: bool,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedAssuranceDecisionFact {
    pub assurance_epoch: u64,
    pub sequence: u64,
    pub policy_ref: StableId,
    pub decision_ref: StableId,
    pub resolved: bool,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedAssuranceWaiverFact {
    pub assurance_epoch: u64,
    pub sequence: u64,
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub receipt_digest: String,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedAssuranceActionPacketFact {
    pub policy_ref: StableId,
    pub subject_ref: StableId,
    pub packet_digest: String,
}

/// Complete fact set supplied only after the kernel has verified ledger,
/// registry, companion, revocation, subject, snapshot, and time bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedAssuranceFacts {
    pub target: ReadinessTarget,
    pub evidence: Vec<GovernedAssuranceEvidenceFact>,
    pub capabilities: Vec<GovernedAssuranceCapabilityFact>,
    pub decisions: Vec<GovernedAssuranceDecisionFact>,
    pub waivers: Vec<GovernedAssuranceWaiverFact>,
    pub action_packets: Vec<GovernedAssuranceActionPacketFact>,
}

/// Derive governed lens state from admitted policy plus verified receipt facts.
///
/// The function accepts no claim status or readiness verdict. Facts from an
/// older intent epoch or from before the current intent receipt are ignored.
///
/// # Errors
/// Returns [`AssuranceProjectionError`] when admitted assurance metadata,
/// representative-slice evidence, or the derived projection is inconsistent.
pub fn project_governed_durable_assurance(
    mut projection: DurableAssuranceProjection,
    bundle: &WorkflowGovernanceBundleDocument,
    facts: &GovernedAssuranceFacts,
) -> Result<DurableAssuranceProjection, AssuranceProjectionError> {
    let current_evidence = facts
        .evidence
        .iter()
        .filter(|fact| fact_is_current(fact.assurance_epoch, fact.sequence, &projection))
        .collect::<Vec<_>>();
    // The latest reviewer observation owns definition authority regardless of
    // outcome. A newer failure or malformed replacement invalidates an older
    // passing definition instead of silently falling back to stale authority.
    let latest_definition_observation = current_evidence
        .iter()
        .copied()
        .filter_map(|fact| {
            let (_policy, claim, evaluator) = find_claim(bundle, fact)?;
            if claim.assurance_role
                != Some(WorkflowAssuranceClaimRole::RepresentativeSliceDefinition)
                || evaluator.provider != WorkflowEvaluatorProvider::IndependentReviewer
                || fact.provider != evaluator.provider
                || fact.kind != WorkflowEvidenceKind::IndependentReview
                || fact.broker_profile != WorkflowBrokerOriginProfile::Reviewer
                || fact.strength < evaluator.minimum_strength
                || !evaluator.accepted_evidence_kinds.contains(&fact.kind)
                || !origin_matches_provider(fact, evaluator.provider)
            {
                return None;
            }
            Some(fact)
        })
        .max_by_key(|fact| fact.sequence);
    let definition = latest_definition_observation.and_then(|fact| {
        if fact.outcome != WorkflowEvidenceOutcome::Pass
            || fact.subject_kind != WorkflowEvidenceSubjectKind::Artifact
            || fact.representative_slice_definition_digest.as_deref()
                != Some(fact.subject_digest.as_str())
        {
            return None;
        }
        let manifest = fact.representative_slice.as_ref()?;
        validate_representative_slice_definition(manifest, &projection.binding.intent_digest)
            .is_ok()
            .then_some((fact, manifest))
    });

    let mut lenses = Vec::with_capacity(UniversalAssuranceLens::ALL.len());
    for lens in UniversalAssuranceLens::ALL {
        let mut claim_bindings = Vec::new();
        let mut evidence_bindings = Vec::new();
        let mut capability_bindings = Vec::new();
        let mut decision_bindings = Vec::new();
        let mut waiver_bindings = Vec::new();

        for policy in &bundle.workflow_governance_bundle.policies {
            for claim in policy
                .claims
                .iter()
                .filter(|claim| claim_contributes_to_lens(claim, lens))
            {
                let Some(evaluator) = policy
                    .evaluators
                    .iter()
                    .find(|candidate| candidate.id == claim.evaluator_ref)
                else {
                    continue;
                };
                let required_before =
                    claim_required_before(policy, &claim.id).unwrap_or(ReadinessTarget::Release);
                let observations = current_evidence
                    .iter()
                    .copied()
                    .filter(|fact| {
                        fact.policy_ref == policy.id
                            && fact.claim_ref == claim.id
                            && fact.evaluator_ref == evaluator.id
                            && fact.provider == evaluator.provider
                            && evaluator.accepted_evidence_kinds.contains(&fact.kind)
                            && origin_matches_provider(fact, evaluator.provider)
                    })
                    .collect::<Vec<_>>();
                let waivers = facts
                    .waivers
                    .iter()
                    .filter(|fact| {
                        fact_is_current(fact.assurance_epoch, fact.sequence, &projection)
                            && fact.policy_ref == policy.id
                            && fact.claim_ref == claim.id
                    })
                    .collect::<Vec<_>>();
                let state = claim_epistemic_state(
                    claim.assurance_role,
                    evaluator,
                    &observations,
                    &waivers,
                    definition,
                );
                claim_bindings.push(DurableAssuranceClaimBinding {
                    policy_ref: policy.id.clone(),
                    claim_ref: claim.id.clone(),
                    evaluator_ref: evaluator.id.clone(),
                    evaluator_provider: evaluator.provider,
                    required_before,
                    state,
                });
                evidence_bindings.extend(observations.into_iter().map(evidence_binding));
                waiver_bindings.extend(waivers.into_iter().map(|fact| {
                    DurableAssuranceWaiverBinding {
                        policy_ref: fact.policy_ref.clone(),
                        claim_ref: fact.claim_ref.clone(),
                        receipt_digest: fact.receipt_digest.clone(),
                        expires_at_unix: fact.expires_at_unix,
                    }
                }));

                for requirement in policy
                    .capability_requirements
                    .iter()
                    .filter(|requirement| requirement.affected_claim_refs.contains(&claim.id))
                {
                    let latest = facts
                        .capabilities
                        .iter()
                        .filter(|fact| {
                            fact_is_current(fact.assurance_epoch, fact.sequence, &projection)
                                && fact.policy_ref == policy.id
                                && fact.capability_ref == requirement.id
                        })
                        .max_by_key(|fact| fact.sequence);
                    capability_bindings.push(DurableAssuranceCapabilityBinding {
                        policy_ref: policy.id.clone(),
                        capability_ref: requirement.id.clone(),
                        available: latest.is_some_and(|fact| fact.available),
                        receipt_digest: latest.map(|fact| fact.receipt_digest.clone()),
                    });
                }
                for rule in policy
                    .decision_rules
                    .iter()
                    .filter(|rule| rule.claim_ref.as_ref() == Some(&claim.id))
                {
                    let latest = facts
                        .decisions
                        .iter()
                        .filter(|fact| {
                            fact_is_current(fact.assurance_epoch, fact.sequence, &projection)
                                && fact.policy_ref == policy.id
                                && fact.decision_ref == rule.id
                        })
                        .max_by_key(|fact| fact.sequence);
                    decision_bindings.push(DurableAssuranceDecisionBinding {
                        policy_ref: policy.id.clone(),
                        decision_ref: rule.id.clone(),
                        resolved: latest.is_some_and(|fact| fact.resolved),
                        receipt_digest: latest.map(|fact| fact.receipt_digest.clone()),
                    });
                }
            }
        }

        claim_bindings.sort_by(|left, right| {
            left.policy_ref
                .cmp(&right.policy_ref)
                .then_with(|| left.claim_ref.cmp(&right.claim_ref))
        });
        evidence_bindings.sort_by(|left, right| {
            left.policy_ref
                .cmp(&right.policy_ref)
                .then_with(|| left.claim_ref.cmp(&right.claim_ref))
                .then_with(|| {
                    left.evidence_record_digest
                        .cmp(&right.evidence_record_digest)
                })
        });
        capability_bindings.sort_by(|left, right| {
            left.policy_ref
                .cmp(&right.policy_ref)
                .then_with(|| left.capability_ref.cmp(&right.capability_ref))
        });
        capability_bindings.dedup();
        decision_bindings.sort_by(|left, right| {
            left.policy_ref
                .cmp(&right.policy_ref)
                .then_with(|| left.decision_ref.cmp(&right.decision_ref))
        });
        decision_bindings.dedup();
        waiver_bindings.sort_by(|left, right| left.receipt_digest.cmp(&right.receipt_digest));
        waiver_bindings.dedup();

        let required_before = claim_bindings
            .iter()
            .map(|binding| binding.required_before)
            .min_by_key(|target| target.rank())
            .unwrap_or(ReadinessTarget::Release);
        let due = claim_bindings
            .iter()
            .any(|binding| facts.target.rank() >= binding.required_before.rank());
        // Claims for a later boundary remain visible in the projection, but
        // cannot block an earlier target. The definition prerequisite is
        // derived onto the two execution lenses without mapping/satisfying a
        // policy lens by itself.
        let claim_status = aggregate_lens_state_for_target(&claim_bindings, facts.target);
        lenses.push(DurableAssuranceLensProjection {
            lens,
            claim_status,
            required_before,
            due,
            claims: claim_bindings,
            evidence: evidence_bindings,
            capabilities: capability_bindings,
            decisions: decision_bindings,
            waivers: waiver_bindings,
        });
    }

    let blocker_lenses = lenses
        .iter()
        .filter(|lens| {
            lens.due
                && !matches!(
                    lens.claim_status,
                    DurableAssuranceEpistemicState::Verified
                        | DurableAssuranceEpistemicState::Waived
                )
        })
        .map(|lens| lens.lens)
        .collect::<Vec<_>>();
    let next_actions = bounded_next_actions(&lenses, facts);
    projection.lenses = lenses;
    projection.blocker_lenses = blocker_lenses;
    projection.readiness = if projection.blocker_lenses.is_empty() {
        DurableAssuranceReadinessState::Ready
    } else {
        DurableAssuranceReadinessState::Blocked
    };
    projection.next_actions = next_actions;
    projection.projection_digest = canonical_digest(&DurableAssuranceProjectionDigestSubject {
        binding: &projection.binding,
        intent: &projection.intent,
        lenses: &projection.lenses,
        readiness: projection.readiness,
        blocker_lenses: &projection.blocker_lenses,
        next_actions: &projection.next_actions,
    })?;
    Ok(projection)
}

fn fact_is_current(epoch: u64, sequence: u64, projection: &DurableAssuranceProjection) -> bool {
    epoch == projection.binding.assurance_epoch && sequence > projection.binding.accepted_sequence
}

fn find_claim<'a>(
    bundle: &'a WorkflowGovernanceBundleDocument,
    fact: &GovernedAssuranceEvidenceFact,
) -> Option<(
    &'a forge_core_contracts::WorkflowGovernancePolicy,
    &'a forge_core_contracts::WorkflowClaimPolicy,
    &'a forge_core_contracts::WorkflowEvaluatorBinding,
)> {
    let policy = bundle
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id == fact.policy_ref)?;
    let claim = policy
        .claims
        .iter()
        .find(|claim| claim.id == fact.claim_ref && claim.evaluator_ref == fact.evaluator_ref)?;
    let evaluator = policy
        .evaluators
        .iter()
        .find(|evaluator| evaluator.id == fact.evaluator_ref)?;
    Some((policy, claim, evaluator))
}

fn claim_required_before(
    policy: &forge_core_contracts::WorkflowGovernancePolicy,
    claim_ref: &StableId,
) -> Option<ReadinessTarget> {
    policy
        .obligations
        .iter()
        .filter(|obligation| {
            obligation.criticality != ObligationCriticality::Advisory
                && obligation.claim_refs.contains(claim_ref)
        })
        .map(|obligation| obligation.required_before)
        .min_by_key(|target| target.rank())
}

fn claim_contributes_to_lens(
    claim: &forge_core_contracts::WorkflowClaimPolicy,
    lens: UniversalAssuranceLens,
) -> bool {
    claim.assurance_lenses.contains(&lens)
        || (claim.assurance_role == Some(WorkflowAssuranceClaimRole::RepresentativeSliceDefinition)
            && matches!(
                lens,
                UniversalAssuranceLens::CriticalJourneys
                    | UniversalAssuranceLens::EvidenceRepresentativeness
            ))
}

fn origin_matches_provider(
    fact: &GovernedAssuranceEvidenceFact,
    provider: WorkflowEvaluatorProvider,
) -> bool {
    !fact.origin_record_digest.trim().is_empty()
        && !fact.origin_principal.0.trim().is_empty()
        && !fact.separation_domain.0.trim().is_empty()
        && match provider {
            WorkflowEvaluatorProvider::AuthorizedHuman => {
                fact.broker_profile == WorkflowBrokerOriginProfile::Human
            }
            WorkflowEvaluatorProvider::IndependentReviewer => {
                fact.broker_profile == WorkflowBrokerOriginProfile::Reviewer
            }
            WorkflowEvaluatorProvider::RepositoryInspector
            | WorkflowEvaluatorProvider::DeterministicTool
            | WorkflowEvaluatorProvider::RepresentativeRuntime
            | WorkflowEvaluatorProvider::ExternalAuthority
            | WorkflowEvaluatorProvider::ResearchSource => {
                fact.broker_profile == WorkflowBrokerOriginProfile::Runtime
            }
        }
}

fn claim_epistemic_state(
    role: Option<WorkflowAssuranceClaimRole>,
    evaluator: &forge_core_contracts::WorkflowEvaluatorBinding,
    observations: &[&GovernedAssuranceEvidenceFact],
    waivers: &[&GovernedAssuranceWaiverFact],
    definition: Option<(
        &GovernedAssuranceEvidenceFact,
        &WorkflowRepresentativeSliceDefinitionDocument,
    )>,
) -> DurableAssuranceEpistemicState {
    if observations
        .iter()
        .any(|fact| fact.outcome == WorkflowEvidenceOutcome::Fail)
    {
        return DurableAssuranceEpistemicState::Disproven;
    }
    if !waivers.is_empty() {
        return DurableAssuranceEpistemicState::Waived;
    }
    let passing = observations
        .iter()
        .copied()
        .filter(|fact| fact.outcome == WorkflowEvidenceOutcome::Pass)
        .collect::<Vec<_>>();
    let qualifying = passing
        .iter()
        .copied()
        .filter(|fact| {
            fact.provider != WorkflowEvaluatorProvider::ResearchSource
                && fact.kind != WorkflowEvidenceKind::Research
                && fact.strength >= evaluator.minimum_strength
                && representative_evidence_matches(role, fact, definition)
        })
        .collect::<Vec<_>>();
    let principals = qualifying
        .iter()
        .map(|fact| fact.origin_principal.0.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let separation_domains = qualifying
        .iter()
        .map(|fact| fact.separation_domain.0.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let reviewer_separation = evaluator.provider != WorkflowEvaluatorProvider::IndependentReviewer
        || separation_domains.len() >= evaluator.minimum_distinct_principals.max(1);
    let representative_complete = role
        != Some(WorkflowAssuranceClaimRole::RepresentativeSliceExecution)
        || representative_execution_complete(&qualifying, definition);
    if qualifying.len() >= evaluator.minimum_passing_observations
        && principals.len() >= evaluator.minimum_distinct_principals
        && reviewer_separation
        && representative_complete
    {
        DurableAssuranceEpistemicState::Verified
    } else if passing.is_empty() {
        DurableAssuranceEpistemicState::Unknown
    } else {
        DurableAssuranceEpistemicState::Supported
    }
}

fn representative_evidence_matches(
    role: Option<WorkflowAssuranceClaimRole>,
    fact: &GovernedAssuranceEvidenceFact,
    definition: Option<(
        &GovernedAssuranceEvidenceFact,
        &WorkflowRepresentativeSliceDefinitionDocument,
    )>,
) -> bool {
    if role == Some(WorkflowAssuranceClaimRole::RepresentativeSliceDefinition) {
        return definition.is_some_and(|(definition_fact, _)| {
            definition_fact.evidence_record_digest == fact.evidence_record_digest
        });
    }
    if role != Some(WorkflowAssuranceClaimRole::RepresentativeSliceExecution) {
        return true;
    }
    fact.provider == WorkflowEvaluatorProvider::RepresentativeRuntime
        && fact.kind == WorkflowEvidenceKind::RepresentativeExecution
        && fact.subject_kind == WorkflowEvidenceSubjectKind::Runtime
        && definition.is_some_and(|(definition_fact, manifest)| {
            fact.sequence > definition_fact.sequence
                && fact.representative_slice_definition_digest.as_deref()
                    == Some(definition_fact.subject_digest.as_str())
                && fact.separation_domain != definition_fact.separation_domain
                && manifest
                    .representative_slice
                    .representative_environment
                    .runtime_subject_ref
                    == fact.subject_ref
                && manifest
                    .representative_slice
                    .representative_environment
                    .runtime_subject_digest
                    == fact.subject_digest
                && manifest
                    .representative_slice
                    .scenarios
                    .iter()
                    .any(|scenario| scenario.declared_scenario_digest == fact.scenario_digest)
        })
}

fn representative_execution_complete(
    qualifying: &[&GovernedAssuranceEvidenceFact],
    definition: Option<(
        &GovernedAssuranceEvidenceFact,
        &WorkflowRepresentativeSliceDefinitionDocument,
    )>,
) -> bool {
    definition.is_some_and(|(definition_fact, manifest)| {
        manifest
            .representative_slice
            .scenarios
            .iter()
            .all(|scenario| {
                qualifying.iter().any(|fact| {
                    fact.sequence > definition_fact.sequence
                        && fact.representative_slice_definition_digest.as_deref()
                            == Some(definition_fact.subject_digest.as_str())
                        && fact.scenario_digest == scenario.declared_scenario_digest
                        && fact.separation_domain != definition_fact.separation_domain
                        && fact.subject_kind == WorkflowEvidenceSubjectKind::Runtime
                        && fact.subject_ref
                            == manifest
                                .representative_slice
                                .representative_environment
                                .runtime_subject_ref
                        && fact.subject_digest
                            == manifest
                                .representative_slice
                                .representative_environment
                                .runtime_subject_digest
                })
            })
    })
}

fn aggregate_lens_state(claims: &[DurableAssuranceClaimBinding]) -> DurableAssuranceEpistemicState {
    if claims.is_empty() {
        return DurableAssuranceEpistemicState::Unknown;
    }
    if claims
        .iter()
        .any(|claim| claim.state == DurableAssuranceEpistemicState::Disproven)
    {
        return DurableAssuranceEpistemicState::Disproven;
    }
    if claims
        .iter()
        .all(|claim| claim.state == DurableAssuranceEpistemicState::Verified)
    {
        return DurableAssuranceEpistemicState::Verified;
    }
    if claims.iter().all(|claim| {
        matches!(
            claim.state,
            DurableAssuranceEpistemicState::Verified | DurableAssuranceEpistemicState::Waived
        )
    }) && claims
        .iter()
        .any(|claim| claim.state == DurableAssuranceEpistemicState::Waived)
    {
        return DurableAssuranceEpistemicState::Waived;
    }
    if claims.iter().any(|claim| {
        matches!(
            claim.state,
            DurableAssuranceEpistemicState::Supported | DurableAssuranceEpistemicState::Verified
        )
    }) {
        DurableAssuranceEpistemicState::Supported
    } else {
        DurableAssuranceEpistemicState::Unknown
    }
}

fn aggregate_lens_state_for_target(
    claims: &[DurableAssuranceClaimBinding],
    target: ReadinessTarget,
) -> DurableAssuranceEpistemicState {
    let due = claims
        .iter()
        .filter(|claim| target.rank() >= claim.required_before.rank())
        .cloned()
        .collect::<Vec<_>>();
    aggregate_lens_state(&due)
}

fn evidence_binding(fact: &GovernedAssuranceEvidenceFact) -> DurableAssuranceEvidenceBinding {
    DurableAssuranceEvidenceBinding {
        policy_ref: fact.policy_ref.clone(),
        claim_ref: fact.claim_ref.clone(),
        evaluator_ref: fact.evaluator_ref.clone(),
        evidence_ref: fact.evidence_ref.clone(),
        evidence_record_digest: fact.evidence_record_digest.clone(),
        origin_record_digest: fact.origin_record_digest.clone(),
        provider: fact.provider,
        kind: fact.kind,
        strength: fact.strength,
        outcome: fact.outcome,
        subject_kind: fact.subject_kind,
        subject_ref: fact.subject_ref.clone(),
        subject_digest: fact.subject_digest.clone(),
        scenario_digest: fact.scenario_digest.clone(),
        origin_principal: fact.origin_principal.clone(),
        separation_domain: fact.separation_domain.clone(),
        broker_profile: fact.broker_profile,
    }
}

fn bounded_next_actions(
    lenses: &[DurableAssuranceLensProjection],
    facts: &GovernedAssuranceFacts,
) -> Vec<DurableAssuranceNextAction> {
    let mut actions = lenses
        .iter()
        .filter(|lens| {
            lens.due
                && !matches!(
                    lens.claim_status,
                    DurableAssuranceEpistemicState::Verified
                        | DurableAssuranceEpistemicState::Waived
                )
        })
        .filter_map(|lens| {
            let default_claim = lens.claims.iter().find(|claim| {
                facts.target.rank() >= claim.required_before.rank()
                    && !matches!(
                        claim.state,
                        DurableAssuranceEpistemicState::Verified
                            | DurableAssuranceEpistemicState::Waived
                    )
            })?;
            let disproven_claim = lens.claims.iter().find(|claim| {
                facts.target.rank() >= claim.required_before.rank()
                    && claim.state == DurableAssuranceEpistemicState::Disproven
            });
            let research_claim = lens.claims.iter().find(|claim| {
                facts.target.rank() >= claim.required_before.rank()
                    && claim.evaluator_provider == WorkflowEvaluatorProvider::ResearchSource
                    && !matches!(
                        claim.state,
                        DurableAssuranceEpistemicState::Verified
                            | DurableAssuranceEpistemicState::Waived
                    )
            });
            let missing_capability = lens.capabilities.iter().find(|item| !item.available);
            let unresolved_decision = lens.decisions.iter().find(|item| !item.resolved);
            let kind = if lens.claim_status == DurableAssuranceEpistemicState::Disproven {
                NextActionKind::Challenge
            } else if missing_capability.is_some() {
                NextActionKind::AcquireCapability
            } else if unresolved_decision.is_some() {
                NextActionKind::AskHuman
            } else if research_claim.is_some() {
                NextActionKind::Research
            } else {
                NextActionKind::Evaluate
            };
            let claim = disproven_claim.or(research_claim).unwrap_or(default_claim);
            let subject_ref = missing_capability
                .map(|item| item.capability_ref.clone())
                .or_else(|| unresolved_decision.map(|item| item.decision_ref.clone()))
                .unwrap_or_else(|| claim.claim_ref.clone());
            let policy_ref = missing_capability
                .map(|item| item.policy_ref.clone())
                .or_else(|| unresolved_decision.map(|item| item.policy_ref.clone()))
                .unwrap_or_else(|| claim.policy_ref.clone());
            let packet = facts.action_packets.iter().find(|packet| {
                packet.policy_ref == policy_ref && packet.subject_ref == subject_ref
            });
            Some(DurableAssuranceNextAction {
                lens: lens.lens,
                kind,
                policy_ref,
                subject_ref: Some(subject_ref),
                action_packet_digest: packet.map(|packet| packet.packet_digest.clone()),
                rank: 0,
            })
        })
        .collect::<Vec<_>>();
    actions.sort_by(|left, right| {
        durable_action_priority(left.kind)
            .cmp(&durable_action_priority(right.kind))
            .then_with(|| left.lens.cmp(&right.lens))
            .then_with(|| left.policy_ref.cmp(&right.policy_ref))
    });
    actions.truncate(MAX_DURABLE_ASSURANCE_NEXT_ACTIONS);
    for (index, action) in actions.iter_mut().enumerate() {
        action.rank = u32::try_from(index + 1).unwrap_or(u32::MAX);
    }
    actions
}

const fn durable_action_priority(kind: NextActionKind) -> u8 {
    match kind {
        NextActionKind::Challenge => 0,
        NextActionKind::AcquireCapability => 1,
        NextActionKind::AskHuman => 2,
        NextActionKind::Research => 3,
        NextActionKind::Evaluate => 4,
        NextActionKind::Experiment
        | NextActionKind::Implement
        | NextActionKind::DeclareGap
        | NextActionKind::Proceed => 5,
    }
}

/// Validate bounded representative-slice content and its current intent bind.
///
/// # Errors
/// Returns [`AssuranceProjectionError`] when the document violates its schema,
/// size bounds, content-addressed references, or current-intent binding.
pub fn validate_representative_slice_definition(
    document: &WorkflowRepresentativeSliceDefinitionDocument,
    current_intent_digest: &str,
) -> Result<(), AssuranceProjectionError> {
    let mut issues = Vec::new();
    if document.schema_version != WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION {
        issues.push(AssuranceProjectionIssue::RepresentativeSliceInvalid {
            field: "schema_version".to_owned(),
        });
    }
    let slice = &document.representative_slice;
    if slice.intent_digest != current_intent_digest || !is_sha256_digest(&slice.intent_digest) {
        issues.push(AssuranceProjectionIssue::RepresentativeSliceInvalid {
            field: "intent_digest".to_owned(),
        });
    }
    for (field, value) in [
        ("critical_journey", slice.critical_journey.as_str()),
        ("falsifier", slice.falsifier.as_str()),
        (
            "representative_environment.expectation",
            slice.representative_environment.expectation.as_str(),
        ),
    ] {
        if value.trim().is_empty() || value.len() > MAX_REPRESENTATIVE_SLICE_TEXT_BYTES {
            issues.push(AssuranceProjectionIssue::RepresentativeSliceInvalid {
                field: field.to_owned(),
            });
        }
    }
    if slice
        .representative_environment
        .runtime_subject_ref
        .trim()
        .is_empty()
        || !is_sha256_digest(&slice.representative_environment.runtime_subject_digest)
    {
        issues.push(AssuranceProjectionIssue::RepresentativeSliceInvalid {
            field: "representative_environment".to_owned(),
        });
    }
    if slice.scenarios.is_empty()
        || slice.scenarios.len() > MAX_REPRESENTATIVE_SLICE_ITEMS
        || slice.material_failure_modes.is_empty()
        || slice.material_failure_modes.len() > MAX_REPRESENTATIVE_SLICE_ITEMS
    {
        issues.push(AssuranceProjectionIssue::RepresentativeSliceInvalid {
            field: "bounded_items".to_owned(),
        });
    }
    let failure_ids = slice
        .material_failure_modes
        .iter()
        .map(|failure| failure.id.0.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if failure_ids.len() != slice.material_failure_modes.len()
        || slice.material_failure_modes.iter().any(|failure| {
            failure.id.0.trim().is_empty()
                || failure.description.trim().is_empty()
                || failure.description.len() > MAX_REPRESENTATIVE_SLICE_ITEM_BYTES
        })
    {
        issues.push(AssuranceProjectionIssue::RepresentativeSliceInvalid {
            field: "material_failure_modes".to_owned(),
        });
    }
    let mut covered = std::collections::BTreeSet::new();
    let mut scenario_refs = std::collections::BTreeSet::new();
    for scenario in &slice.scenarios {
        let path_valid = !scenario.scenario_ref.trim().is_empty()
            && scenario.scenario_ref.len() <= MAX_REPRESENTATIVE_SLICE_ITEM_BYTES
            && !scenario.scenario_ref.contains('\\')
            && !scenario.scenario_ref.starts_with('/')
            && !scenario.scenario_ref.contains(':')
            && !scenario
                .scenario_ref
                .split('/')
                .any(|segment| segment == ".." || segment.is_empty());
        let distinct_failure_refs = scenario
            .failure_mode_refs
            .iter()
            .map(|reference| reference.0.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if !path_valid
            || !scenario_refs.insert(scenario.scenario_ref.as_str())
            || !is_sha256_digest(&scenario.declared_scenario_digest)
            || scenario.failure_mode_refs.is_empty()
            || scenario.failure_mode_refs.len() > MAX_REPRESENTATIVE_SLICE_ITEMS
            || distinct_failure_refs.len() != scenario.failure_mode_refs.len()
            || scenario
                .failure_mode_refs
                .iter()
                .any(|reference| !failure_ids.contains(reference.0.as_str()))
        {
            issues.push(AssuranceProjectionIssue::RepresentativeSliceInvalid {
                field: "scenarios".to_owned(),
            });
        }
        covered.extend(
            scenario
                .failure_mode_refs
                .iter()
                .map(|reference| reference.0.as_str()),
        );
    }
    if covered != failure_ids {
        issues.push(AssuranceProjectionIssue::RepresentativeSliceInvalid {
            field: "failure_mode_coverage".to_owned(),
        });
    }
    let total_bytes = slice.critical_journey.len()
        + slice.falsifier.len()
        + slice.representative_environment.runtime_subject_ref.len()
        + slice
            .representative_environment
            .runtime_subject_digest
            .len()
        + slice.representative_environment.expectation.len()
        + slice
            .scenarios
            .iter()
            .map(|scenario| scenario.scenario_ref.len() + scenario.declared_scenario_digest.len())
            .sum::<usize>()
        + slice
            .material_failure_modes
            .iter()
            .map(|failure| failure.id.0.len() + failure.description.len())
            .sum::<usize>();
    if total_bytes > MAX_REPRESENTATIVE_SLICE_TOTAL_BYTES {
        issues.push(AssuranceProjectionIssue::RepresentativeSliceInvalid {
            field: "total_bytes".to_owned(),
        });
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(AssuranceProjectionError { issues })
    }
}

fn validate_intent(intent: &WorkflowHumanIntentRevision) -> Vec<AssuranceProjectionIssue> {
    let mut issues = Vec::new();
    if intent.intent_id.0.trim().is_empty() {
        issues.push(AssuranceProjectionIssue::EmptyField {
            field: "intent_id".to_owned(),
        });
    }
    validate_required_text(
        &mut issues,
        "desired_outcome",
        &intent.desired_outcome,
        MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES,
    );
    validate_required_text(
        &mut issues,
        "source_conversation_ref",
        &intent.source_conversation_ref,
        MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES,
    );
    if !is_sha256_digest(&intent.source_conversation_digest) {
        issues.push(AssuranceProjectionIssue::InvalidDigest {
            field: "source_conversation_digest".to_owned(),
        });
    }
    for (field, values) in [
        ("constraints", intent.constraints.as_slice()),
        ("preferences", intent.preferences.as_slice()),
        (
            "unacceptable_outcomes",
            intent.unacceptable_outcomes.as_slice(),
        ),
        ("uncertainties", intent.uncertainties.as_slice()),
    ] {
        if values.len() > MAX_WORKFLOW_INTENT_LIST_ITEMS {
            issues.push(AssuranceProjectionIssue::TooManyItems {
                field: field.to_owned(),
                maximum_items: MAX_WORKFLOW_INTENT_LIST_ITEMS,
            });
        }
        for (index, value) in values.iter().enumerate() {
            validate_required_text(
                &mut issues,
                &format!("{field}[{index}]"),
                value,
                MAX_WORKFLOW_INTENT_ITEM_BYTES,
            );
        }
    }
    let aggregate_bytes = intent.desired_outcome.len()
        + intent.constraints.iter().map(String::len).sum::<usize>()
        + intent.preferences.iter().map(String::len).sum::<usize>()
        + intent
            .unacceptable_outcomes
            .iter()
            .map(String::len)
            .sum::<usize>()
        + intent.uncertainties.iter().map(String::len).sum::<usize>()
        + intent.source_conversation_ref.len()
        + intent.source_conversation_digest.len();
    if aggregate_bytes > MAX_WORKFLOW_INTENT_TOTAL_BYTES {
        issues.push(AssuranceProjectionIssue::AggregateIntentTooLarge {
            maximum_bytes: MAX_WORKFLOW_INTENT_TOTAL_BYTES,
        });
    }
    issues
}

fn validate_required_text(
    issues: &mut Vec<AssuranceProjectionIssue>,
    field: &str,
    value: &str,
    maximum_bytes: usize,
) {
    if value.trim().is_empty() {
        issues.push(AssuranceProjectionIssue::EmptyField {
            field: field.to_owned(),
        });
    }
    if value.len() > maximum_bytes {
        issues.push(AssuranceProjectionIssue::FieldTooLarge {
            field: field.to_owned(),
            maximum_bytes,
        });
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, AssuranceProjectionError> {
    serde_json_canonicalizer::to_vec(value)
        .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)))
        .map_err(|_| AssuranceProjectionError {
            issues: vec![AssuranceProjectionIssue::ProjectionEncodingFailed],
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::workflow_governance::BrokerOriginAppliedEvent;
    use forge_core_contracts::{PrincipalId, StableId};

    fn hash(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn intent(revision: u64, outcome: &str) -> WorkflowHumanIntentRevision {
        WorkflowHumanIntentRevision {
            intent_id: StableId("intent.product".to_owned()),
            revision,
            desired_outcome: outcome.to_owned(),
            constraints: vec!["Remain portable".to_owned()],
            preferences: Vec::new(),
            unacceptable_outcomes: vec!["Silent data loss".to_owned()],
            uncertainties: vec!["Deployment topology".to_owned()],
            source_conversation_ref: format!("conversation:turn:{revision}"),
            source_conversation_digest: hash('c'),
        }
    }

    fn record(
        sequence: u64,
        state_version: u64,
        prior_head: &str,
        assurance_epoch: u64,
        intent: WorkflowHumanIntentRevision,
        previous_intent_digest: Option<String>,
    ) -> WorkflowGovernanceLedgerRecord {
        let intent_digest = workflow_human_intent_digest(&intent).expect("valid intent");
        WorkflowGovernanceLedgerRecord {
            record_id: StableId(format!("record.intent.{sequence}")),
            sequence,
            project_id: StableId("project.example".to_owned()),
            bundle_id: StableId("bundle.example".to_owned()),
            bundle_digest: hash('b'),
            state_version,
            previous_record_digest: Some(prior_head.to_owned()),
            record_digest: hash(char::from_digit((sequence % 10) as u32, 10).unwrap_or('a')),
            recorded_at_unix: 100 + sequence,
            event: WorkflowGovernanceEvent::HumanIntentRevisionAccepted(
                HumanIntentRevisionAcceptedEvent {
                    assurance_epoch,
                    intent,
                    intent_digest,
                    previous_intent_digest,
                    snapshot_digest: hash('d'),
                    ledger_head_digest: prior_head.to_owned(),
                    acceptance_action_packet_digest: hash('a'),
                    accepted_by: PrincipalId("principal.human".to_owned()),
                    accepted_at_unix: 100 + sequence,
                },
            ),
        }
    }

    fn origin_companion(
        action_record: &WorkflowGovernanceLedgerRecord,
        sequence: u64,
    ) -> WorkflowGovernanceLedgerRecord {
        let WorkflowGovernanceEvent::HumanIntentRevisionAccepted(action) = &action_record.event
        else {
            panic!("action record must contain accepted intent");
        };
        WorkflowGovernanceLedgerRecord {
            record_id: StableId(format!("record.origin.{sequence}")),
            sequence,
            project_id: action_record.project_id.clone(),
            bundle_id: action_record.bundle_id.clone(),
            bundle_digest: action_record.bundle_digest.clone(),
            state_version: action_record.state_version,
            previous_record_digest: Some(action_record.record_digest.clone()),
            record_digest: hash(char::from_digit((sequence % 10) as u32, 10).unwrap_or('a')),
            recorded_at_unix: action_record.recorded_at_unix + 1,
            event: WorkflowGovernanceEvent::BrokerOriginApplied(BrokerOriginAppliedEvent {
                action_packet_digest: action.acceptance_action_packet_digest.clone(),
                broker_event_digest: hash('e'),
                action_record_digest: action_record.record_digest.clone(),
                origin_principal_id: action.accepted_by.clone(),
                separation_domain: StableId("human-presence".to_owned()),
                nonce_fingerprint: hash('a'),
                issuer_id: StableId("broker.human".to_owned()),
                issuer_profile: WorkflowBrokerOriginProfile::Human,
                public_key_fingerprint: hash('b'),
                signature_fingerprint: hash('c'),
                enrollment_ceremony_digest: hash('d'),
                broker_registry_digest: hash('e'),
                issued_at_unix: action.accepted_at_unix,
                expires_at_unix: action.accepted_at_unix + 300,
            }),
        }
    }

    #[test]
    fn no_accepted_intent_has_no_durable_assurance_epoch() {
        assert_eq!(project_durable_assurance(&[]).expect("project"), None);
    }

    #[test]
    fn accepted_intent_creates_exactly_every_unknown_lens() {
        let record = record(1, 3, &hash('0'), 1, intent(1, "Ship safely"), None);
        let companion = origin_companion(&record, 2);

        let projection = project_durable_assurance(&[record, companion])
            .expect("valid projection")
            .expect("accepted intent");

        assert_eq!(projection.binding.assurance_epoch, 1);
        assert_eq!(projection.binding.intent_revision, 1);
        assert_eq!(projection.lenses.len(), UniversalAssuranceLens::ALL.len());
        assert_eq!(
            projection
                .lenses
                .iter()
                .map(|entry| entry.lens)
                .collect::<Vec<_>>(),
            UniversalAssuranceLens::ALL
        );
        assert!(projection.lenses.iter().all(|entry| {
            entry.claim_status == DurableAssuranceEpistemicState::Unknown
                && entry.evidence.is_empty()
                && entry.claims.is_empty()
        }));
        assert_eq!(
            projection.readiness,
            DurableAssuranceReadinessState::Unknown
        );
        assert_eq!(projection.blocker_lenses, UniversalAssuranceLens::ALL);
        assert!(is_sha256_digest(&projection.projection_digest));
    }

    #[test]
    fn latest_revision_advances_epoch_and_rebuilds_unknown_state() {
        let first = record(1, 3, &hash('0'), 1, intent(1, "First"), None);
        let first_companion = origin_companion(&first, 2);
        let first_intent_digest = match &first.event {
            WorkflowGovernanceEvent::HumanIntentRevisionAccepted(event) => {
                event.intent_digest.clone()
            }
            _ => unreachable!(),
        };
        let second = record(
            3,
            5,
            &hash('2'),
            2,
            intent(2, "Revised"),
            Some(first_intent_digest),
        );
        let second_companion = origin_companion(&second, 4);

        let projection =
            project_durable_assurance(&[first, first_companion, second, second_companion])
                .expect("valid revisions")
                .expect("accepted intent");

        assert_eq!(projection.binding.assurance_epoch, 2);
        assert_eq!(projection.binding.intent_revision, 2);
        assert_eq!(projection.intent.desired_outcome, "Revised");
        assert!(projection
            .lenses
            .iter()
            .all(|lens| lens.claim_status == DurableAssuranceEpistemicState::Unknown));
    }

    #[test]
    fn forged_digest_or_non_monotonic_revision_fails_closed() {
        let first = record(1, 3, &hash('0'), 1, intent(1, "First"), None);
        let first_companion = origin_companion(&first, 2);
        let mut second = record(2, 4, &hash('1'), 3, intent(3, "Skip"), Some(hash('f')));
        let WorkflowGovernanceEvent::HumanIntentRevisionAccepted(event) = &mut second.event else {
            unreachable!();
        };
        event.intent_digest = hash('f');
        let second_companion = origin_companion(&second, 4);

        let error = project_durable_assurance(&[first, first_companion, second, second_companion])
            .expect_err("must fail closed");

        assert!(error
            .issues
            .contains(&AssuranceProjectionIssue::IntentDigestMismatch));
        assert!(error.issues.iter().any(|issue| matches!(
            issue,
            AssuranceProjectionIssue::NonMonotonicEpoch { .. }
                | AssuranceProjectionIssue::NonMonotonicRevision { .. }
        )));
        assert!(error
            .issues
            .contains(&AssuranceProjectionIssue::PreviousIntentDigestMismatch));
    }

    #[test]
    fn oversized_or_blank_human_content_is_rejected_before_digest() {
        let mut invalid = intent(1, " ");
        invalid.constraints = vec!["x".repeat(MAX_WORKFLOW_INTENT_ITEM_BYTES + 1)];

        let error = workflow_human_intent_digest(&invalid).expect_err("must reject");

        assert!(error.issues.iter().any(|issue| matches!(
            issue,
            AssuranceProjectionIssue::EmptyField { field } if field == "desired_outcome"
        )));
        assert!(error.issues.iter().any(|issue| matches!(
            issue,
            AssuranceProjectionIssue::FieldTooLarge { field, .. } if field == "constraints[0]"
        )));
    }

    #[test]
    fn orphan_or_mismatched_origin_companion_fails_closed() {
        let record = record(1, 3, &hash('0'), 1, intent(1, "Ship safely"), None);

        let orphan_error =
            project_durable_assurance(std::slice::from_ref(&record)).expect_err("orphan rejected");
        assert!(orphan_error
            .issues
            .contains(&AssuranceProjectionIssue::MissingOriginCompanion));

        let mut companion = origin_companion(&record, 2);
        let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = &mut companion.event else {
            unreachable!();
        };
        origin.origin_principal_id = PrincipalId("principal.impostor".to_owned());

        let mismatch_error = project_durable_assurance(&[record.clone(), companion])
            .expect_err("mismatched origin rejected");
        assert!(mismatch_error
            .issues
            .contains(&AssuranceProjectionIssue::OriginBindingMismatch));

        let mut companion = origin_companion(&record, 2);
        let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = &mut companion.event else {
            unreachable!();
        };
        origin.issued_at_unix = origin.issued_at_unix.saturating_add(1);

        let clock_mismatch_error = project_durable_assurance(&[record, companion])
            .expect_err("mismatched signed-origin clock rejected");
        assert!(clock_mismatch_error
            .issues
            .contains(&AssuranceProjectionIssue::OriginBindingMismatch));
    }

    fn evidence_fact(
        provider: WorkflowEvaluatorProvider,
        kind: WorkflowEvidenceKind,
        outcome: WorkflowEvidenceOutcome,
    ) -> GovernedAssuranceEvidenceFact {
        GovernedAssuranceEvidenceFact {
            assurance_epoch: 1,
            sequence: 3,
            policy_ref: StableId("policy.assurance".to_owned()),
            claim_ref: StableId("claim.assurance".to_owned()),
            evaluator_ref: StableId("evaluator.assurance".to_owned()),
            evidence_ref: "evidence.assurance".to_owned(),
            evidence_record_digest: hash('1'),
            origin_record_digest: hash('2'),
            provider,
            kind,
            strength: WorkflowEvidenceStrength::IndependentConfirmation,
            outcome,
            subject_kind: WorkflowEvidenceSubjectKind::ExternalSystem,
            subject_ref: "external:source".to_owned(),
            subject_digest: hash('3'),
            scenario_digest: hash('4'),
            origin_principal: PrincipalId("principal.researcher".to_owned()),
            separation_domain: StableId("domain.research".to_owned()),
            broker_profile: WorkflowBrokerOriginProfile::Runtime,
            representative_slice: None,
            representative_slice_definition_digest: None,
        }
    }

    fn evaluator(
        provider: WorkflowEvaluatorProvider,
    ) -> forge_core_contracts::WorkflowEvaluatorBinding {
        forge_core_contracts::WorkflowEvaluatorBinding {
            id: StableId("evaluator.assurance".to_owned()),
            provider,
            accepted_evidence_kinds: vec![WorkflowEvidenceKind::Research],
            minimum_strength: WorkflowEvidenceStrength::IndependentConfirmation,
            minimum_passing_observations: 1,
            minimum_distinct_principals: 1,
            max_age_seconds: 300,
            freshness: forge_core_contracts::WorkflowFreshnessRequirement::CurrentOnly,
            disproof_policy: forge_core_contracts::WorkflowDisproofPolicy::RejectAnyDisproof,
        }
    }

    #[test]
    fn governed_research_can_support_but_never_verify() {
        let evaluator = evaluator(WorkflowEvaluatorProvider::ResearchSource);
        let fact = evidence_fact(
            WorkflowEvaluatorProvider::ResearchSource,
            WorkflowEvidenceKind::Research,
            WorkflowEvidenceOutcome::Pass,
        );

        assert_eq!(
            claim_epistemic_state(
                Some(WorkflowAssuranceClaimRole::LensEvidence),
                &evaluator,
                &[&fact],
                &[],
                None,
            ),
            DurableAssuranceEpistemicState::Supported
        );
    }

    #[test]
    fn governed_disproof_dominates_an_existing_waiver() {
        let evaluator = evaluator(WorkflowEvaluatorProvider::ResearchSource);
        let fact = evidence_fact(
            WorkflowEvaluatorProvider::ResearchSource,
            WorkflowEvidenceKind::Research,
            WorkflowEvidenceOutcome::Fail,
        );
        let waiver = GovernedAssuranceWaiverFact {
            assurance_epoch: 1,
            sequence: 4,
            policy_ref: StableId("policy.assurance".to_owned()),
            claim_ref: StableId("claim.assurance".to_owned()),
            receipt_digest: hash('5'),
            expires_at_unix: 500,
        };

        assert_eq!(
            claim_epistemic_state(
                Some(WorkflowAssuranceClaimRole::LensEvidence),
                &evaluator,
                &[&fact],
                &[&waiver],
                None,
            ),
            DurableAssuranceEpistemicState::Disproven
        );
    }

    #[test]
    fn later_boundary_claims_remain_visible_without_blocking_execute() {
        let binding = |claim: &str,
                       required_before: ReadinessTarget,
                       state: DurableAssuranceEpistemicState| {
            DurableAssuranceClaimBinding {
                policy_ref: StableId("policy.assurance".to_owned()),
                claim_ref: StableId(claim.to_owned()),
                evaluator_ref: StableId(format!("evaluator.{claim}")),
                evaluator_provider: WorkflowEvaluatorProvider::RepresentativeRuntime,
                required_before,
                state,
            }
        };
        let claims = vec![
            binding(
                "claim.execute",
                ReadinessTarget::Execute,
                DurableAssuranceEpistemicState::Verified,
            ),
            binding(
                "claim.release",
                ReadinessTarget::Release,
                DurableAssuranceEpistemicState::Unknown,
            ),
        ];

        assert_eq!(
            aggregate_lens_state_for_target(&claims, ReadinessTarget::Execute),
            DurableAssuranceEpistemicState::Verified
        );
        assert_eq!(
            aggregate_lens_state_for_target(&claims, ReadinessTarget::Release),
            DurableAssuranceEpistemicState::Supported
        );
    }

    #[test]
    fn representative_slice_is_typed_bounded_and_intent_bound() {
        let manifest = WorkflowRepresentativeSliceDefinitionDocument {
            schema_version: WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION.to_owned(),
            representative_slice: forge_core_contracts::WorkflowRepresentativeSliceDefinition {
                intent_digest: hash('a'),
                critical_journey: "First use succeeds end to end".to_owned(),
                falsifier: "The first-use result is unusable".to_owned(),
                representative_environment:
                    forge_core_contracts::WorkflowRepresentativeEnvironment {
                        runtime_subject_ref: "runtime.reference".to_owned(),
                        runtime_subject_digest: hash('b'),
                        expectation: "Reference runtime with production-equivalent effects"
                            .to_owned(),
                    },
                scenarios: vec![
                    forge_core_contracts::WorkflowRepresentativeScenarioReference {
                        scenario_ref: "assurance/first-use.yaml".to_owned(),
                        declared_scenario_digest: hash('c'),
                        failure_mode_refs: vec![StableId("failure.first-use".to_owned())],
                    },
                ],
                material_failure_modes: vec![
                    forge_core_contracts::WorkflowRepresentativeFailureMode {
                        id: StableId("failure.first-use".to_owned()),
                        description: "The integrated result cannot complete first use".to_owned(),
                    },
                ],
            },
        };

        validate_representative_slice_definition(&manifest, &hash('a')).expect("valid manifest");
        assert!(validate_representative_slice_definition(&manifest, &hash('f')).is_err());
    }

    #[test]
    fn representative_execution_requires_every_scenario_and_separate_runtime_domain() {
        let first_digest = hash('c');
        let second_digest = hash('d');
        let manifest = WorkflowRepresentativeSliceDefinitionDocument {
            schema_version: WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION.to_owned(),
            representative_slice: forge_core_contracts::WorkflowRepresentativeSliceDefinition {
                intent_digest: hash('a'),
                critical_journey: "First use succeeds end to end".to_owned(),
                falsifier: "Either scenario fails".to_owned(),
                representative_environment:
                    forge_core_contracts::WorkflowRepresentativeEnvironment {
                        runtime_subject_ref: "runtime.reference".to_owned(),
                        runtime_subject_digest: hash('b'),
                        expectation: "Production-equivalent runtime".to_owned(),
                    },
                scenarios: vec![
                    forge_core_contracts::WorkflowRepresentativeScenarioReference {
                        scenario_ref: "assurance/first.yaml".to_owned(),
                        declared_scenario_digest: first_digest.clone(),
                        failure_mode_refs: vec![StableId("failure.first".to_owned())],
                    },
                    forge_core_contracts::WorkflowRepresentativeScenarioReference {
                        scenario_ref: "assurance/second.yaml".to_owned(),
                        declared_scenario_digest: second_digest.clone(),
                        failure_mode_refs: vec![StableId("failure.second".to_owned())],
                    },
                ],
                material_failure_modes: vec![
                    forge_core_contracts::WorkflowRepresentativeFailureMode {
                        id: StableId("failure.first".to_owned()),
                        description: "First scenario fails".to_owned(),
                    },
                    forge_core_contracts::WorkflowRepresentativeFailureMode {
                        id: StableId("failure.second".to_owned()),
                        description: "Second scenario fails".to_owned(),
                    },
                ],
            },
        };
        let mut definition_fact = evidence_fact(
            WorkflowEvaluatorProvider::IndependentReviewer,
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceOutcome::Pass,
        );
        definition_fact.broker_profile = WorkflowBrokerOriginProfile::Reviewer;
        definition_fact.separation_domain = StableId("domain.reviewer".to_owned());
        definition_fact.subject_kind = WorkflowEvidenceSubjectKind::Artifact;
        definition_fact.representative_slice = Some(manifest.clone());
        definition_fact.representative_slice_definition_digest =
            Some(definition_fact.subject_digest.clone());
        let runtime_evaluator = forge_core_contracts::WorkflowEvaluatorBinding {
            id: StableId("evaluator.assurance".to_owned()),
            provider: WorkflowEvaluatorProvider::RepresentativeRuntime,
            accepted_evidence_kinds: vec![WorkflowEvidenceKind::RepresentativeExecution],
            minimum_strength: WorkflowEvidenceStrength::RepresentativeExecution,
            minimum_passing_observations: 1,
            minimum_distinct_principals: 0,
            max_age_seconds: 300,
            freshness: forge_core_contracts::WorkflowFreshnessRequirement::CurrentOnly,
            disproof_policy: forge_core_contracts::WorkflowDisproofPolicy::RejectAnyDisproof,
        };
        let runtime_fact = |digest: String, domain: &str| {
            let mut fact = evidence_fact(
                WorkflowEvaluatorProvider::RepresentativeRuntime,
                WorkflowEvidenceKind::RepresentativeExecution,
                WorkflowEvidenceOutcome::Pass,
            );
            fact.strength = WorkflowEvidenceStrength::RepresentativeExecution;
            fact.subject_kind = WorkflowEvidenceSubjectKind::Runtime;
            fact.subject_ref = "runtime.reference".to_owned();
            fact.subject_digest = hash('b');
            fact.scenario_digest = digest;
            fact.separation_domain = StableId(domain.to_owned());
            fact.sequence = definition_fact.sequence + 1;
            fact.representative_slice_definition_digest =
                Some(definition_fact.subject_digest.clone());
            fact
        };
        let first = runtime_fact(first_digest, "domain.runtime");
        let second_same_domain = runtime_fact(second_digest.clone(), "domain.reviewer");
        let second = runtime_fact(second_digest, "domain.runtime");
        let definition = Some((&definition_fact, &manifest));

        let mut before_definition = first.clone();
        before_definition.sequence = definition_fact.sequence;
        let mut wrong_definition = first.clone();
        wrong_definition.representative_slice_definition_digest = Some(hash('e'));

        for observations in [
            vec![&first],
            vec![&first, &second_same_domain],
            vec![&before_definition, &second],
            vec![&wrong_definition, &second],
        ] {
            assert_eq!(
                claim_epistemic_state(
                    Some(WorkflowAssuranceClaimRole::RepresentativeSliceExecution),
                    &runtime_evaluator,
                    &observations,
                    &[],
                    definition,
                ),
                DurableAssuranceEpistemicState::Supported
            );
        }
        assert_eq!(
            claim_epistemic_state(
                Some(WorkflowAssuranceClaimRole::RepresentativeSliceExecution),
                &runtime_evaluator,
                &[&first, &second],
                &[],
                definition,
            ),
            DurableAssuranceEpistemicState::Verified
        );
    }
}
