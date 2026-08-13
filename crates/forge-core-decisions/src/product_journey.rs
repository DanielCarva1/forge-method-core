use forge_core_contracts::Phase;
use forge_core_contracts::{
    ProductJourneyDocument, ProductStageDescriptor, PRODUCT_JOURNEY_CONTRACT_REF,
    PRODUCT_JOURNEY_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const MAX_STAGE_ID_BYTES: usize = 64;
const MAX_STAGE_NAME_BYTES: usize = 64;
const MAX_STAGE_OBJECTIVE_BYTES: usize = 256;

static ACCEPTED_JOURNEY: OnceLock<Result<ProductJourneyDocument, ProductJourneyRejection>> =
    OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductJourneyRejection {
    pub issues: Vec<ProductJourneyIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductJourneyIssue {
    pub code: ProductJourneyIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductJourneyIssueCode {
    ParseFailed,
    UnsupportedSchemaVersion,
    MissingPhase,
    DuplicatePhase,
    ContactDensityMismatch,
    InvalidStageDescriptor,
    InvalidTransitionStage,
}

pub fn load_accepted_product_journey(
) -> Result<&'static ProductJourneyDocument, ProductJourneyRejection> {
    match ACCEPTED_JOURNEY.get_or_init(|| {
        let text = include_str!("../../../contracts/guidance/product-journey-v0.yaml");
        let document = yaml_serde::from_str::<ProductJourneyDocument>(text).map_err(|error| {
            ProductJourneyRejection {
                issues: vec![ProductJourneyIssue {
                    code: ProductJourneyIssueCode::ParseFailed,
                    path: PRODUCT_JOURNEY_CONTRACT_REF.to_owned(),
                    message: error.to_string(),
                }],
            }
        })?;
        validate_product_journey(&document)?;
        Ok(document)
    }) {
        Ok(document) => Ok(document),
        Err(rejection) => Err(rejection.clone()),
    }
}

pub fn validate_product_journey(
    document: &ProductJourneyDocument,
) -> Result<(), ProductJourneyRejection> {
    let mut issues = Vec::new();
    if document.schema_version != PRODUCT_JOURNEY_SCHEMA_VERSION {
        push_issue(
            &mut issues,
            ProductJourneyIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            format!(
                "unsupported product journey schema {}; expected {}",
                document.schema_version, PRODUCT_JOURNEY_SCHEMA_VERSION
            ),
        );
    }

    let funnel = super::load_accepted_funnel_autonomy_policy().map_err(|rejection| {
        ProductJourneyRejection {
            issues: rejection
                .issues
                .into_iter()
                .map(|issue| ProductJourneyIssue {
                    code: ProductJourneyIssueCode::ContactDensityMismatch,
                    path: issue.path,
                    message: issue.message,
                })
                .collect(),
        }
    })?;

    for phase in Phase::ALL {
        let matches = document
            .product_journey
            .stages
            .iter()
            .filter(|stage| stage.phase == phase)
            .collect::<Vec<_>>();
        if matches.is_empty() {
            push_issue(
                &mut issues,
                ProductJourneyIssueCode::MissingPhase,
                "product_journey.stages",
                format!("missing stage for phase {phase}"),
            );
            continue;
        }
        if matches.len() > 1 {
            push_issue(
                &mut issues,
                ProductJourneyIssueCode::DuplicatePhase,
                "product_journey.stages",
                format!("phase {phase} appears {} times", matches.len()),
            );
        }

        let stage = matches[0];
        let expected_density = funnel
            .funnel_autonomy_policy
            .phase_profiles
            .iter()
            .find(|profile| profile.phase == phase)
            .map(|profile| profile.contact_density);
        if expected_density != Some(stage.contact_density) {
            push_issue(
                &mut issues,
                ProductJourneyIssueCode::ContactDensityMismatch,
                "product_journey.stages.contact_density",
                format!("stage {phase} does not match the accepted funnel profile"),
            );
        }
        validate_stage(stage, &mut issues);
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(ProductJourneyRejection { issues })
    }
}

pub fn project_product_stage(
    document: &ProductJourneyDocument,
    phase: Phase,
) -> Result<ProductStageDescriptor, ProductJourneyRejection> {
    validate_product_journey(document)?;
    document
        .product_journey
        .stages
        .iter()
        .find(|stage| stage.phase == phase)
        .cloned()
        .ok_or_else(|| ProductJourneyRejection {
            issues: vec![ProductJourneyIssue {
                code: ProductJourneyIssueCode::MissingPhase,
                path: "product_journey.stages".to_owned(),
                message: format!("missing stage for phase {phase}"),
            }],
        })
}

fn validate_stage(stage: &ProductStageDescriptor, issues: &mut Vec<ProductJourneyIssue>) {
    if !bounded(&stage.stage_id.0, MAX_STAGE_ID_BYTES)
        || !bounded(&stage.display_name, MAX_STAGE_NAME_BYTES)
        || !bounded(&stage.objective, MAX_STAGE_OBJECTIVE_BYTES)
    {
        push_issue(
            issues,
            ProductJourneyIssueCode::InvalidStageDescriptor,
            "product_journey.stages",
            format!("stage {} contains blank or oversized text", stage.phase),
        );
    }
    let expected_lifecycle_stage = !matches!(stage.phase, Phase::Route | Phase::Evolve);
    if stage.transition_only != (stage.phase == Phase::Evolve)
        || stage.lifecycle_stage != expected_lifecycle_stage
    {
        push_issue(
            issues,
            ProductJourneyIssueCode::InvalidTransitionStage,
            "product_journey.stages.transition_only",
            "only Evolve is transition-only; Route and Evolve are not lifecycle stages",
        );
    }
}

fn bounded(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max
}

fn push_issue(
    issues: &mut Vec<ProductJourneyIssue>,
    code: ProductJourneyIssueCode,
    path: &str,
    message: impl Into<String>,
) {
    issues.push(ProductJourneyIssue {
        code,
        path: path.to_owned(),
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::FunnelContactDensity;

    #[test]
    fn accepted_journey_covers_every_phase_with_the_funnel_contact_density() {
        let journey = load_accepted_product_journey().expect("accepted product journey");
        validate_product_journey(journey).expect("valid product journey");

        for phase in Phase::ALL {
            let stage = project_product_stage(journey, phase).expect("stage for every phase");
            assert_eq!(stage.phase, phase);
            assert!(!stage.stage_id.0.is_empty());
            assert!(!stage.display_name.is_empty());
            assert!(!stage.objective.is_empty());
        }

        assert_eq!(
            project_product_stage(journey, Phase::Discovery)
                .expect("discovery stage")
                .contact_density,
            FunnelContactDensity::High
        );
        assert_eq!(
            project_product_stage(journey, Phase::BuildVerify)
                .expect("implementation stage")
                .contact_density,
            FunnelContactDensity::Low
        );
    }

    #[test]
    fn validation_accumulates_duplicate_density_and_transition_errors() {
        let mut journey = load_accepted_product_journey()
            .expect("accepted product journey")
            .clone();
        let discovery = journey
            .product_journey
            .stages
            .iter_mut()
            .find(|stage| stage.phase == Phase::Discovery)
            .expect("discovery stage");
        discovery.contact_density = FunnelContactDensity::Low;
        discovery.transition_only = true;
        let duplicate = discovery.clone();
        journey.product_journey.stages.push(duplicate);

        let rejection = validate_product_journey(&journey).expect_err("invalid journey");
        assert!(rejection
            .issues
            .iter()
            .any(|issue| issue.code == ProductJourneyIssueCode::DuplicatePhase));
        assert!(rejection
            .issues
            .iter()
            .any(|issue| issue.code == ProductJourneyIssueCode::ContactDensityMismatch));
        assert!(rejection
            .issues
            .iter()
            .any(|issue| issue.code == ProductJourneyIssueCode::InvalidTransitionStage));
    }
}
