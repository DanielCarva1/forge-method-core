//! Pure conservative checks for the versioned solo cooperative evidence route.

use forge_core_contracts::{
    WorkflowCooperativeEvidenceAssuranceEffect, WorkflowCooperativeEvidenceBinding,
    WorkflowCooperativeEvidenceDisposition, WorkflowCooperativeEvidenceOffer,
    WorkflowCooperativeEvidenceRejection, WorkflowCooperativeEvidenceRoute,
    WorkflowCooperativeEvidenceTarget, WorkflowCooperativeMaterialScenarioKind,
    WorkflowEvaluatorProvider, WorkflowEvidenceKind, WorkflowEvidenceStrength,
    WorkflowEvidenceSubjectKind, COOPERATIVE_APPLICABILITY_ATTESTATION_SCHEMA_VERSION,
    COOPERATIVE_APPLICABILITY_OFFER_SCHEMA_VERSION,
    COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION,
    COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION_V1, COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION,
    COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION_V1, MAX_WORKFLOW_COOPERATIVE_EVIDENCE_BASIS_ITEMS,
    SOLO_COOPERATIVE_APPLICABILITY_DESCRIPTOR_VERSION,
    SOLO_COOPERATIVE_APPLICABILITY_POLICY_VERSION, SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION,
    SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION_V1, SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION,
    SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION_V1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CooperativeEvidenceDecision {
    pub disposition: WorkflowCooperativeEvidenceDisposition,
    pub rejection: Option<WorkflowCooperativeEvidenceRejection>,
}

#[must_use]
pub fn evaluate_cooperative_evidence(
    current: &WorkflowCooperativeEvidenceBinding,
    route: &WorkflowCooperativeEvidenceRoute,
    offer: &WorkflowCooperativeEvidenceOffer,
    readback_observed_at_unix: u64,
) -> CooperativeEvidenceDecision {
    let reject = |reason| CooperativeEvidenceDecision {
        disposition: WorkflowCooperativeEvidenceDisposition::Rejected,
        rejection: Some(reason),
    };
    let statement = &offer.attestation;
    let statement_target = statement
        .target
        .unwrap_or(WorkflowCooperativeEvidenceTarget::SourceClaim);
    let legacy_route = offer.schema_version == COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION_V1
        && statement.schema_version == COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION_V1
        && statement.policy_version == SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION_V1
        && statement.claim_descriptor_version == SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION_V1
        && route.policy_version == SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION_V1
        && route.claim_descriptor_version == SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION_V1
        && route.target == WorkflowCooperativeEvidenceTarget::SourceClaim
        && statement_target == WorkflowCooperativeEvidenceTarget::SourceClaim
        && route.assurance_effect
            == WorkflowCooperativeEvidenceAssuranceEffect::CooperativeClaimOnlyDoesNotSatisfySourceClaim;
    let source_route = offer.schema_version == COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION
        && statement.schema_version == COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION
        && statement.policy_version == SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION
        && statement.claim_descriptor_version == SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION
        && route.policy_version == SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION
        && route.claim_descriptor_version == SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION
        && route.target == WorkflowCooperativeEvidenceTarget::SourceClaim
        && statement_target == WorkflowCooperativeEvidenceTarget::SourceClaim
        && route.assurance_effect
            == WorkflowCooperativeEvidenceAssuranceEffect::SoloSourceClaimSatisfiedByAgentInspection;
    let applicability_route =
        offer.schema_version == COOPERATIVE_APPLICABILITY_OFFER_SCHEMA_VERSION
            && statement.schema_version == COOPERATIVE_APPLICABILITY_ATTESTATION_SCHEMA_VERSION
            && statement.policy_version == SOLO_COOPERATIVE_APPLICABILITY_POLICY_VERSION
            && statement.claim_descriptor_version
                == SOLO_COOPERATIVE_APPLICABILITY_DESCRIPTOR_VERSION
            && route.policy_version == SOLO_COOPERATIVE_APPLICABILITY_POLICY_VERSION
            && route.claim_descriptor_version
                == SOLO_COOPERATIVE_APPLICABILITY_DESCRIPTOR_VERSION
            && route.target == WorkflowCooperativeEvidenceTarget::PolicyApplicability
            && statement_target == WorkflowCooperativeEvidenceTarget::PolicyApplicability
            && route.assurance_effect
                == WorkflowCooperativeEvidenceAssuranceEffect::SoloPolicyApplicabilityAssessedByAgentInspection;
    if !legacy_route && !source_route && !applicability_route {
        return reject(WorkflowCooperativeEvidenceRejection::UnsupportedSchema);
    }
    if route.provider != WorkflowEvaluatorProvider::RepositoryInspector
        || route.kind != WorkflowEvidenceKind::ArtifactInspection
        || route.strength != WorkflowEvidenceStrength::InspectedArtifact
        || (source_route && route.source_provider != WorkflowEvaluatorProvider::RepositoryInspector)
    {
        return reject(WorkflowCooperativeEvidenceRejection::PolicyDoesNotPermitCooperation);
    }
    if statement.binding != *current {
        return reject(WorkflowCooperativeEvidenceRejection::BindingStale);
    }
    if statement.policy_ref != route.policy_ref
        || statement.claim_ref != route.claim_ref
        || statement.evaluator_ref != route.evaluator_ref
        || statement.cooperative_claim_ref != route.cooperative_claim_ref
        || statement.cooperative_evaluator_ref != route.cooperative_evaluator_ref
    {
        return reject(WorkflowCooperativeEvidenceRejection::PolicyDoesNotPermitCooperation);
    }
    if statement.producer != route.producer {
        return reject(WorkflowCooperativeEvidenceRejection::WrongProducer);
    }
    if !route
        .allowed_subject_kinds
        .contains(&statement.subject.kind)
        || route.allowed_subject_kinds != [WorkflowEvidenceSubjectKind::ProjectSnapshot]
        || statement.subject.kind != WorkflowEvidenceSubjectKind::ProjectSnapshot
        || statement.subject.subject_ref != route.subject_ref
    {
        return reject(WorkflowCooperativeEvidenceRejection::WrongSubject);
    }
    if !is_sha256_digest(&statement.subject.subject_digest) {
        return reject(WorkflowCooperativeEvidenceRejection::SubjectDigestMismatch);
    }
    let scenario_valid = statement.scenario_digest == route.scenario_digest
        && is_sha256_digest(&statement.scenario_digest)
        && ((legacy_route
            && statement.scenario_kind
                == WorkflowCooperativeMaterialScenarioKind::KernelProjectSnapshotReadback
            && statement.source_assessment.is_none()
            && statement.applicability_assessment.is_none())
            || (source_route
                && statement.scenario_kind
                    == WorkflowCooperativeMaterialScenarioKind::AgentRepositoryInspectionWithContentAddressedBasis
                && statement.applicability_assessment.is_none()
                && statement.source_assessment.as_ref().is_some_and(|assessment| {
                    !assessment.summary.trim().is_empty()
                        && !assessment.basis_paths.is_empty()
                        && assessment.basis_paths.len()
                            <= MAX_WORKFLOW_COOPERATIVE_EVIDENCE_BASIS_ITEMS
                }))
            || (applicability_route
                && statement.scenario_kind
                    == WorkflowCooperativeMaterialScenarioKind::AgentPolicyApplicabilityInspectionWithContentAddressedBasis
                && statement.source_assessment.is_none()
                && statement
                    .applicability_assessment
                    .as_ref()
                    .is_some_and(|assessment| {
                        !assessment.summary.trim().is_empty()
                            && !assessment.basis_paths.is_empty()
                            && assessment.basis_paths.len()
                                <= MAX_WORKFLOW_COOPERATIVE_EVIDENCE_BASIS_ITEMS
                    })));
    if !scenario_valid {
        return reject(WorkflowCooperativeEvidenceRejection::WrongScenario);
    }
    if offer.offer_id.0.trim().is_empty() || readback_observed_at_unix == 0 {
        return reject(WorkflowCooperativeEvidenceRejection::FabricatedOrMalformedReceipt);
    }
    CooperativeEvidenceDecision {
        disposition: WorkflowCooperativeEvidenceDisposition::Admitted,
        rejection: None,
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{
        PrincipalId, StableId, WorkflowCooperativeEvidenceAttestation,
        WorkflowCooperativeSourceAssessmentOffer, WorkflowEvidenceOutcome, WorkflowEvidenceSubject,
    };

    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn binding() -> WorkflowCooperativeEvidenceBinding {
        WorkflowCooperativeEvidenceBinding {
            objective_id: StableId("objective.solo".to_owned()),
            objective_revision: 1,
            objective_digest: DIGEST.to_owned(),
            assurance_epoch: 1,
            accepted_objective_record_digest: DIGEST.to_owned(),
            accepted_objective_record_sequence: 2,
            policy_bundle_digest: DIGEST.to_owned(),
            snapshot_digest: DIGEST.to_owned(),
            ledger_head_digest: DIGEST.to_owned(),
            state_version: 2,
        }
    }

    fn route() -> WorkflowCooperativeEvidenceRoute {
        WorkflowCooperativeEvidenceRoute {
            policy_version: SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION_V1.to_owned(),
            claim_descriptor_version: SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION_V1.to_owned(),
            target: WorkflowCooperativeEvidenceTarget::SourceClaim,
            policy_ref: StableId("policy.solo".to_owned()),
            claim_ref: StableId("claim.solo".to_owned()),
            evaluator_ref: StableId("evaluator.solo".to_owned()),
            source_provider: WorkflowEvaluatorProvider::AuthorizedHuman,
            cooperative_claim_ref: StableId("claim.solo.cooperative".to_owned()),
            cooperative_evaluator_ref: StableId("evaluator.solo.cooperative".to_owned()),
            producer: PrincipalId("agent.solo".to_owned()),
            provider: WorkflowEvaluatorProvider::RepositoryInspector,
            kind: WorkflowEvidenceKind::ArtifactInspection,
            strength: WorkflowEvidenceStrength::InspectedArtifact,
            allowed_subject_kinds: vec![WorkflowEvidenceSubjectKind::ProjectSnapshot],
            subject_ref: "project.current".to_owned(),
            scenario_digest: DIGEST.to_owned(),
            max_age_seconds: 60,
            assurance_effect:
                WorkflowCooperativeEvidenceAssuranceEffect::CooperativeClaimOnlyDoesNotSatisfySourceClaim,
        }
    }

    fn offer() -> WorkflowCooperativeEvidenceOffer {
        WorkflowCooperativeEvidenceOffer {
            schema_version: COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION_V1.to_owned(),
            offer_id: StableId("offer.1".to_owned()),
            attestation: WorkflowCooperativeEvidenceAttestation {
                schema_version: COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION_V1.to_owned(),
                policy_version: SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION_V1.to_owned(),
                claim_descriptor_version: SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION_V1.to_owned(),
                target: None,
                binding: binding(),
                policy_ref: StableId("policy.solo".to_owned()),
                claim_ref: StableId("claim.solo".to_owned()),
                evaluator_ref: StableId("evaluator.solo".to_owned()),
                cooperative_claim_ref: StableId("claim.solo.cooperative".to_owned()),
                cooperative_evaluator_ref: StableId("evaluator.solo.cooperative".to_owned()),
                producer: PrincipalId("agent.solo".to_owned()),
                subject: WorkflowEvidenceSubject {
                    kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
                    subject_ref: "project.current".to_owned(),
                    subject_digest: DIGEST.to_owned(),
                },
                scenario_kind:
                    WorkflowCooperativeMaterialScenarioKind::KernelProjectSnapshotReadback,
                scenario_digest: DIGEST.to_owned(),
                source_assessment: None,
                applicability_assessment: None,
            },
        }
    }

    #[test]
    fn admits_only_current_kernel_verifiable_route() {
        assert_eq!(
            evaluate_cooperative_evidence(&binding(), &route(), &offer(), 11).disposition,
            WorkflowCooperativeEvidenceDisposition::Admitted
        );
        let mut runtime = offer();
        runtime.attestation.subject.kind = WorkflowEvidenceSubjectKind::Runtime;
        assert_eq!(
            evaluate_cooperative_evidence(&binding(), &route(), &runtime, 11).rejection,
            Some(WorkflowCooperativeEvidenceRejection::WrongSubject)
        );
    }

    #[test]
    fn stale_zero_readback_and_wrong_producer_fail_closed() {
        let mut stale = offer();
        stale.attestation.binding.assurance_epoch += 1;
        assert_eq!(
            evaluate_cooperative_evidence(&binding(), &route(), &stale, 11).rejection,
            Some(WorkflowCooperativeEvidenceRejection::BindingStale)
        );
        assert_eq!(
            evaluate_cooperative_evidence(&binding(), &route(), &offer(), 0).rejection,
            Some(WorkflowCooperativeEvidenceRejection::FabricatedOrMalformedReceipt)
        );
        let mut wrong = offer();
        wrong.attestation.producer = PrincipalId("agent.other".to_owned());
        assert_eq!(
            evaluate_cooperative_evidence(&binding(), &route(), &wrong, 11).rejection,
            Some(WorkflowCooperativeEvidenceRejection::WrongProducer)
        );
    }

    #[test]
    fn repository_inspection_source_assessment_is_admitted_without_relabeling_other_providers() {
        let mut source_route = route();
        source_route.policy_version = SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION.to_owned();
        source_route.claim_descriptor_version =
            SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION.to_owned();
        source_route.source_provider = WorkflowEvaluatorProvider::RepositoryInspector;
        source_route.assurance_effect =
            WorkflowCooperativeEvidenceAssuranceEffect::SoloSourceClaimSatisfiedByAgentInspection;

        let mut source_offer = offer();
        source_offer.schema_version = COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION.to_owned();
        source_offer.attestation.schema_version =
            COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION.to_owned();
        source_offer.attestation.policy_version =
            SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION.to_owned();
        source_offer.attestation.claim_descriptor_version =
            SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION.to_owned();
        source_offer.attestation.scenario_kind =
            WorkflowCooperativeMaterialScenarioKind::AgentRepositoryInspectionWithContentAddressedBasis;
        source_offer.attestation.source_assessment =
            Some(WorkflowCooperativeSourceAssessmentOffer {
                outcome: WorkflowEvidenceOutcome::Pass,
                summary: "The inspected behavior matches the claim".to_owned(),
                basis_paths: vec!["README.md".to_owned()],
                limitations: vec!["This is not an independent review".to_owned()],
            });
        for outcome in [
            WorkflowEvidenceOutcome::Pass,
            WorkflowEvidenceOutcome::Fail,
            WorkflowEvidenceOutcome::Inconclusive,
        ] {
            source_offer
                .attestation
                .source_assessment
                .as_mut()
                .expect("source assessment")
                .outcome = outcome;
            assert_eq!(
                evaluate_cooperative_evidence(&binding(), &source_route, &source_offer, 11)
                    .disposition,
                WorkflowCooperativeEvidenceDisposition::Admitted,
                "the decision layer must preserve the agent's honest {outcome:?} assessment"
            );
        }

        source_route.source_provider = WorkflowEvaluatorProvider::AuthorizedHuman;
        assert_eq!(
            evaluate_cooperative_evidence(&binding(), &source_route, &source_offer, 11).rejection,
            Some(WorkflowCooperativeEvidenceRejection::PolicyDoesNotPermitCooperation)
        );
    }

    #[test]
    fn representative_runtime_descriptor_is_default_denied() {
        let mut runtime_route = route();
        runtime_route.provider = WorkflowEvaluatorProvider::RepresentativeRuntime;
        runtime_route.kind = WorkflowEvidenceKind::RepresentativeExecution;
        runtime_route.strength = WorkflowEvidenceStrength::RepresentativeExecution;
        assert_eq!(
            evaluate_cooperative_evidence(&binding(), &runtime_route, &offer(), 11).rejection,
            Some(WorkflowCooperativeEvidenceRejection::PolicyDoesNotPermitCooperation)
        );
    }
}
