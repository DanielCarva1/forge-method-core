use crate::{Diagnostic, DiagnosticCode, ValidationReport};
use forge_core_contracts::{
    AssuranceCaseDocument, AssuranceClaimStatus, ObligationCriticality, ObligationStatus,
    ReadinessVerdict, ASSURANCE_CASE_SCHEMA_VERSION,
};
use std::collections::{HashMap, HashSet};

/// Validate the internal epistemic and readiness invariants of an Assurance Case.
///
/// This validator deliberately evaluates only the document's internal claims and
/// references. Whether an evidence ref is truthful or representative belongs to
/// the Evidence Module and its evaluator Adapters.
#[must_use]
pub fn validate_assurance_case(document: &AssuranceCaseDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    if document.schema_version != ASSURANCE_CASE_SCHEMA_VERSION {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceUnsupportedSchemaVersion,
            "schema_version",
            format!(
                "unsupported Assurance Case schema version '{}'; expected {ASSURANCE_CASE_SCHEMA_VERSION}",
                document.schema_version
            ),
        ));
    }

    let case = &document.assurance_case;
    if case.obligations.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceRequiredCollectionEmpty,
            format!("assurance_case.{}.obligations", case.id.0),
            "an Assurance Case must declare at least one obligation",
        ));
    }
    if case.claims.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceRequiredCollectionEmpty,
            format!("assurance_case.{}.claims", case.id.0),
            "an Assurance Case must declare at least one claim",
        ));
    }
    if case.next_actions.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceRequiredCollectionEmpty,
            format!("assurance_case.{}.next_actions", case.id.0),
            "an Assurance Case must declare at least one next action",
        ));
    }

    let mut entity_ids = HashSet::new();
    let mut blocker_ids = HashSet::new();
    register_assurance_entity(
        &mut report,
        &mut entity_ids,
        &case.id.0,
        "assurance_case.id",
    );
    register_assurance_entity(
        &mut report,
        &mut entity_ids,
        &case.intent.id.0,
        "assurance_case.intent.id",
    );
    register_assurance_entity(
        &mut report,
        &mut entity_ids,
        &case.project_snapshot.id.0,
        "assurance_case.project_snapshot.id",
    );

    let mut claim_statuses = HashMap::new();
    for claim in &case.claims {
        let path = format!("assurance_case.{}.claims.{}", case.id.0, claim.id.0);
        register_assurance_entity(&mut report, &mut entity_ids, &claim.id.0, &path);
        blocker_ids.insert(claim.id.0.as_str());
        claim_statuses.insert(claim.id.0.as_str(), claim.status);

        if matches!(
            claim.status,
            AssuranceClaimStatus::Supported
                | AssuranceClaimStatus::Verified
                | AssuranceClaimStatus::Disproven
        ) && claim.evidence_refs.is_empty()
        {
            report.push(Diagnostic::error(
                DiagnosticCode::AssuranceClaimEvidenceMissing,
                format!("{path}.evidence_refs"),
                "supported, verified, and disproven claims require evidence",
            ));
        }

        match (claim.status, &claim.waiver) {
            (AssuranceClaimStatus::Waived, Some(waiver)) => {
                if waiver.authorized_by.0.trim().is_empty()
                    || waiver.reason.trim().is_empty()
                    || waiver.consequences.is_empty()
                {
                    report.push(Diagnostic::error(
                        DiagnosticCode::AssuranceWaiverInconsistent,
                        format!("{path}.waiver"),
                        "a waiver requires authority, reason, and explicit consequences",
                    ));
                }
            }
            (AssuranceClaimStatus::Waived, None) => report.push(Diagnostic::error(
                DiagnosticCode::AssuranceWaiverInconsistent,
                format!("{path}.waiver"),
                "a waived claim requires an explicit waiver",
            )),
            (_, Some(_)) => report.push(Diagnostic::error(
                DiagnosticCode::AssuranceWaiverInconsistent,
                format!("{path}.waiver"),
                "only a claim with status waived may carry waiver authority",
            )),
            (_, None) => {}
        }
    }

    for obligation in &case.obligations {
        let path = format!(
            "assurance_case.{}.obligations.{}",
            case.id.0, obligation.id.0
        );
        register_assurance_entity(&mut report, &mut entity_ids, &obligation.id.0, &path);
        blocker_ids.insert(obligation.id.0.as_str());

        if obligation.claim_refs.is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::AssuranceDanglingClaimRef,
                format!("{path}.claim_refs"),
                "an obligation must reference at least one Assurance Claim",
            ));
        }

        let mut all_claims_satisfy = !obligation.claim_refs.is_empty();
        for claim_ref in &obligation.claim_refs {
            if let Some(status) = claim_statuses.get(claim_ref.0.as_str()) {
                all_claims_satisfy &= status.satisfies_obligation();
            } else {
                all_claims_satisfy = false;
                report.push(Diagnostic::error(
                    DiagnosticCode::AssuranceDanglingClaimRef,
                    format!("{path}.claim_refs"),
                    format!("claim ref '{}' does not resolve", claim_ref.0),
                ));
            }
        }
        if obligation.status == ObligationStatus::Satisfied && !all_claims_satisfy {
            report.push(Diagnostic::error(
                DiagnosticCode::AssuranceSatisfiedObligationUnsupported,
                format!("{path}.status"),
                "a satisfied obligation requires every referenced claim to be verified or waived",
            ));
        }
    }

    for request in &case.decision_requests {
        let path = format!(
            "assurance_case.{}.decision_requests.{}",
            case.id.0, request.id.0
        );
        register_assurance_entity(&mut report, &mut entity_ids, &request.id.0, &path);
        blocker_ids.insert(request.id.0.as_str());

        let mut alternative_ids = HashSet::new();
        for alternative in &request.alternatives {
            register_assurance_entity(
                &mut report,
                &mut entity_ids,
                &alternative.id.0,
                &format!("{path}.alternatives.{}", alternative.id.0),
            );
            alternative_ids.insert(alternative.id.0.as_str());
        }
        if request.alternatives.len() < 2
            || !alternative_ids.contains(request.recommended_alternative_ref.0.as_str())
        {
            report.push(Diagnostic::error(
                DiagnosticCode::AssuranceDecisionRecommendationInvalid,
                format!("{path}.recommended_alternative_ref"),
                "a Decision Request needs at least two alternatives and a recommendation that resolves",
            ));
        }
    }

    for gap in &case.capability_gaps {
        let path = format!("assurance_case.{}.capability_gaps.{}", case.id.0, gap.id.0);
        register_assurance_entity(&mut report, &mut entity_ids, &gap.id.0, &path);
        blocker_ids.insert(gap.id.0.as_str());
        validate_assurance_claim_refs(
            &mut report,
            &claim_statuses,
            &gap.affected_claim_refs,
            &format!("{path}.affected_claim_refs"),
        );
        if gap.resolution_options.is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::AssuranceCapabilityGapResolutionEmpty,
                format!("{path}.resolution_options"),
                "a Capability Gap must offer at least one resolution option",
            ));
        }
    }

    let mut action_ranks = HashSet::new();
    for action in &case.next_actions {
        let path = format!("assurance_case.{}.next_actions.{}", case.id.0, action.id.0);
        register_assurance_entity(&mut report, &mut entity_ids, &action.id.0, &path);
        validate_assurance_claim_refs(
            &mut report,
            &claim_statuses,
            &action.addresses_claim_refs,
            &format!("{path}.addresses_claim_refs"),
        );
        if action.rank == 0 || !action_ranks.insert(action.rank) {
            report.push(Diagnostic::error(
                DiagnosticCode::AssuranceNextActionRankInvalid,
                format!("{path}.rank"),
                "next-action ranks must be unique and greater than zero",
            ));
        }
    }
    if !action_ranks.is_empty()
        && (1..=u32::try_from(action_ranks.len()).unwrap_or(u32::MAX))
            .any(|rank| !action_ranks.contains(&rank))
    {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceNextActionRankInvalid,
            format!("assurance_case.{}.next_actions", case.id.0),
            "next-action ranks must be contiguous from one",
        ));
    }

    validate_assurance_readiness(&mut report, document, &blocker_ids, &claim_statuses);
    report
}

fn register_assurance_entity(
    report: &mut ValidationReport,
    ids: &mut HashSet<String>,
    id: &str,
    path: &str,
) {
    if id.trim().is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceEmptyEntityId,
            path,
            "Assurance Case entity id must not be empty",
        ));
    } else if !ids.insert(id.to_owned()) {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceDuplicateEntityId,
            path,
            format!("duplicate Assurance Case entity id '{id}'"),
        ));
    }
}

fn validate_assurance_claim_refs(
    report: &mut ValidationReport,
    claims: &HashMap<&str, AssuranceClaimStatus>,
    refs: &[forge_core_contracts::StableId],
    path: &str,
) {
    if refs.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceDanglingClaimRef,
            path,
            "at least one Assurance Claim ref is required",
        ));
    }
    for claim_ref in refs {
        if !claims.contains_key(claim_ref.0.as_str()) {
            report.push(Diagnostic::error(
                DiagnosticCode::AssuranceDanglingClaimRef,
                path,
                format!("claim ref '{}' does not resolve", claim_ref.0),
            ));
        }
    }
}

fn validate_assurance_readiness(
    report: &mut ValidationReport,
    document: &AssuranceCaseDocument,
    blocker_ids: &HashSet<&str>,
    claim_statuses: &HashMap<&str, AssuranceClaimStatus>,
) {
    let case = &document.assurance_case;
    let readiness = &case.readiness;
    let blocker_refs = readiness
        .blocker_refs
        .iter()
        .map(|id| id.0.as_str())
        .collect::<HashSet<_>>();

    match readiness.verdict {
        ReadinessVerdict::Blocked if blocker_refs.is_empty() => report.push(Diagnostic::error(
            DiagnosticCode::AssuranceReadinessInconsistent,
            format!("assurance_case.{}.readiness.blocker_refs", case.id.0),
            "a blocked readiness verdict requires at least one blocker ref",
        )),
        ReadinessVerdict::Ready if !blocker_refs.is_empty() => report.push(Diagnostic::error(
            DiagnosticCode::AssuranceReadinessInconsistent,
            format!("assurance_case.{}.readiness.blocker_refs", case.id.0),
            "a ready verdict cannot carry blocker refs",
        )),
        ReadinessVerdict::Blocked | ReadinessVerdict::Ready => {}
    }

    for blocker_ref in &readiness.blocker_refs {
        if !blocker_ids.contains(blocker_ref.0.as_str()) {
            report.push(Diagnostic::error(
                DiagnosticCode::AssuranceReadinessInconsistent,
                format!("assurance_case.{}.readiness.blocker_refs", case.id.0),
                format!("blocker ref '{}' does not resolve", blocker_ref.0),
            ));
        }
    }

    for obligation in &case.obligations {
        let due = obligation.criticality != ObligationCriticality::Advisory
            && obligation.required_before.rank() <= readiness.target.rank();
        if due && obligation.status != ObligationStatus::Satisfied {
            validate_due_assurance_blocker(
                report,
                readiness.verdict,
                &blocker_refs,
                &obligation.id.0,
                &format!(
                    "assurance_case.{}.obligations.{}.status",
                    case.id.0, obligation.id.0
                ),
            );
        }
    }

    for request in &case.decision_requests {
        if request.blocking && request.blocks_before.rank() <= readiness.target.rank() {
            validate_due_assurance_blocker(
                report,
                readiness.verdict,
                &blocker_refs,
                &request.id.0,
                &format!(
                    "assurance_case.{}.decision_requests.{}.blocking",
                    case.id.0, request.id.0
                ),
            );
        }
    }

    for gap in &case.capability_gaps {
        if gap.blocking && gap.blocks_before.rank() <= readiness.target.rank() {
            validate_due_assurance_blocker(
                report,
                readiness.verdict,
                &blocker_refs,
                &gap.id.0,
                &format!(
                    "assurance_case.{}.capability_gaps.{}.blocking",
                    case.id.0, gap.id.0
                ),
            );
        }
    }

    if readiness.verdict == ReadinessVerdict::Ready
        && case.claims.iter().any(|claim| {
            blocker_refs.contains(claim.id.0.as_str())
                || claim_statuses
                    .get(claim.id.0.as_str())
                    .is_some_and(|status| !status.satisfies_obligation())
                    && case.obligations.iter().any(|obligation| {
                        obligation.criticality == ObligationCriticality::Critical
                            && obligation.required_before.rank() <= readiness.target.rank()
                            && obligation
                                .claim_refs
                                .iter()
                                .any(|claim_ref| claim_ref == &claim.id)
                    })
        })
    {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceReadinessInconsistent,
            format!("assurance_case.{}.readiness.verdict", case.id.0),
            "ready verdict cannot depend on an unresolved critical claim",
        ));
    }
}

fn validate_due_assurance_blocker(
    report: &mut ValidationReport,
    verdict: ReadinessVerdict,
    blocker_refs: &HashSet<&str>,
    entity_id: &str,
    path: &str,
) {
    if verdict == ReadinessVerdict::Ready || !blocker_refs.contains(entity_id) {
        report.push(Diagnostic::error(
            DiagnosticCode::AssuranceReadinessInconsistent,
            path,
            format!(
                "due blocking entity '{entity_id}' must block readiness and appear in blocker_refs"
            ),
        ));
    }
}
