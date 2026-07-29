//! Pure conservative checks for the versioned solo cooperative evidence route.

use forge_core_contracts::{
    WorkflowCooperativeEvidenceAssuranceEffect, WorkflowCooperativeEvidenceBinding,
    WorkflowCooperativeEvidenceDisposition, WorkflowCooperativeEvidenceOffer,
    WorkflowCooperativeEvidenceRejection, WorkflowCooperativeEvidenceRoute,
    WorkflowCooperativeMaterialScenarioKind, WorkflowEvaluatorProvider, WorkflowEvidenceKind,
    WorkflowEvidenceStrength, WorkflowEvidenceSubjectKind,
    COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION, COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION,
    SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION, SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION,
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
    if offer.schema_version != COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION
        || statement.schema_version != COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION
        || statement.policy_version != SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION
        || statement.claim_descriptor_version != SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION
        || route.policy_version != SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION
        || route.claim_descriptor_version != SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION
    {
        return reject(WorkflowCooperativeEvidenceRejection::UnsupportedSchema);
    }
    if route.provider != WorkflowEvaluatorProvider::RepositoryInspector
        || route.kind != WorkflowEvidenceKind::ArtifactInspection
        || route.strength != WorkflowEvidenceStrength::InspectedArtifact
        || route.assurance_effect
            != WorkflowCooperativeEvidenceAssuranceEffect::CooperativeClaimOnlyDoesNotSatisfySourceClaim
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
    if statement.scenario_digest != route.scenario_digest
        || !is_sha256_digest(&statement.scenario_digest)
        || statement.scenario_kind
            != WorkflowCooperativeMaterialScenarioKind::KernelProjectSnapshotReadback
    {
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
        PrincipalId, StableId, WorkflowCooperativeEvidenceAttestation, WorkflowEvidenceSubject,
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
            policy_version: SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION.to_owned(),
            claim_descriptor_version: SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION.to_owned(),
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
            schema_version: COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION.to_owned(),
            offer_id: StableId("offer.1".to_owned()),
            attestation: WorkflowCooperativeEvidenceAttestation {
                schema_version: COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION.to_owned(),
                policy_version: SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION.to_owned(),
                claim_descriptor_version: SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION.to_owned(),
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
