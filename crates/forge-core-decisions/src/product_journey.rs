use forge_core_contracts::{Catalog, Phase, StableId};
use forge_core_contracts::{
    ProductJourneyCatalogConsultation, ProductJourneyCatalogHostAction,
    ProductJourneyCatalogRecheckEvent, ProductJourneyDetailArgv, ProductJourneyDocument,
    ProductJourneyGuidance, ProductJourneyGuidanceAuthority, ProductJourneyGuidanceCatalog,
    ProductJourneyGuidanceStage, ProductJourneyRecommendationOwner, ProductStageDescriptor,
    CATALOG_CONSULTATION_SCHEMA_VERSION, PRODUCT_JOURNEY_CONTRACT_REF,
    PRODUCT_JOURNEY_GUIDANCE_SCHEMA_VERSION, PRODUCT_JOURNEY_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

const MAX_STAGE_ID_BYTES: usize = 64;
const MAX_STAGE_NAME_BYTES: usize = 64;
const MAX_STAGE_OBJECTIVE_BYTES: usize = 256;
const MAX_GUIDANCE_JSON_BYTES: usize = 2048;

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
    GuidanceTooLarge,
}

/// Durable semantic coordinates for one host-session catalog consultation.
#[derive(Debug, Clone, Copy)]
pub struct ProductJourneyConsultationContext<'a> {
    pub project_id: &'a StableId,
    pub objective_digest: Option<&'a str>,
    pub work_focus_id: Option<&'a StableId>,
}

/// Load and validate the product journey compiled into the binary.
///
/// # Errors
///
/// Returns a rejection when the embedded YAML cannot be parsed or fails journey validation.
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

/// Validate a product journey against the accepted lifecycle and autonomy funnel.
///
/// # Errors
///
/// Returns a rejection containing every detected schema, phase, density, or stage issue.
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

/// Project the descriptor for one phase from a valid product journey.
///
/// # Errors
///
/// Returns a rejection when the journey is invalid or the requested phase is missing.
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

/// Derive bounded, advisory guidance for one product journey phase.
///
/// # Errors
///
/// Returns a rejection when the journey is invalid or the resulting guidance exceeds its bound.
pub fn derive_product_journey_guidance(
    document: &ProductJourneyDocument,
    catalog: &Catalog,
    phase: Phase,
    project_root: &str,
    consultation_context: ProductJourneyConsultationContext<'_>,
) -> Result<ProductJourneyGuidance, ProductJourneyRejection> {
    let stage = project_product_stage(document, phase)?;
    let guidance = ProductJourneyGuidance {
        schema_version: PRODUCT_JOURNEY_GUIDANCE_SCHEMA_VERSION.to_owned(),
        authority: ProductJourneyGuidanceAuthority::AdvisoryReadOnly,
        phase,
        stage: ProductJourneyGuidanceStage {
            id: stage.stage_id,
            display_name: stage.display_name,
            objective: stage.objective,
            contact_density: stage.contact_density,
        },
        catalog: ProductJourneyGuidanceCatalog {
            eligible_count: super::eligible_count(catalog, phase),
            consultation: ProductJourneyCatalogConsultation {
                schema_version: CATALOG_CONSULTATION_SCHEMA_VERSION.to_owned(),
                key: catalog_consultation_key(phase, consultation_context),
                host_action: ProductJourneyCatalogHostAction::ConsultOnceWhenUnseen,
                recheck_events: vec![
                    ProductJourneyCatalogRecheckEvent::MaterialHumanRedirect,
                    ProductJourneyCatalogRecheckEvent::ValidationRevealsMisunderstanding,
                ],
            },
            status_argv: vec![
                "forge-core".to_owned(),
                "guide".to_owned(),
                "status".to_owned(),
                "--root".to_owned(),
                project_root.to_owned(),
                "--phase".to_owned(),
                phase.to_string(),
                "--json".to_owned(),
            ],
            detail_argv: ProductJourneyDetailArgv {
                argv: vec![
                    "forge-core".to_owned(),
                    "guide".to_owned(),
                    "detail".to_owned(),
                    "--workflow".to_owned(),
                    "__FORGE_CAPABILITY_ID__".to_owned(),
                    "--json".to_owned(),
                ],
                workflow_id_token: "__FORGE_CAPABILITY_ID__".to_owned(),
            },
        },
        recommendation_owner: ProductJourneyRecommendationOwner::HostAgent,
        recommendation_is_authority: false,
    };
    let guidance_bytes =
        serde_json::to_vec(&guidance).map_err(|error| ProductJourneyRejection {
            issues: vec![ProductJourneyIssue {
                code: ProductJourneyIssueCode::GuidanceTooLarge,
                path: "journey_guidance".to_owned(),
                message: format!("serialize journey guidance: {error}"),
            }],
        })?;
    if guidance_bytes.len() > MAX_GUIDANCE_JSON_BYTES {
        return Err(ProductJourneyRejection {
            issues: vec![ProductJourneyIssue {
                code: ProductJourneyIssueCode::GuidanceTooLarge,
                path: "journey_guidance".to_owned(),
                message: format!(
                    "journey guidance is {} bytes; maximum is {MAX_GUIDANCE_JSON_BYTES}",
                    guidance_bytes.len()
                ),
            }],
        });
    }
    Ok(guidance)
}

fn catalog_consultation_key(
    phase: Phase,
    context: ProductJourneyConsultationContext<'_>,
) -> String {
    let mut hasher = Sha256::new();
    let phase = phase.to_string();
    for component in [
        "forge.product-journey.catalog-consultation/1",
        context.project_id.0.as_str(),
        phase.as_str(),
        context.objective_digest.unwrap_or("<no-objective>"),
        context
            .work_focus_id
            .map_or("<no-work-focus>", |focus_id| focus_id.0.as_str()),
    ] {
        let byte_len = u64::try_from(component.len()).unwrap_or(u64::MAX);
        hasher.update(byte_len.to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
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
    let expected_lifecycle_stage = stage.phase.is_product_lifecycle_stage();
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

    #[test]
    fn guidance_is_compact_and_actionable_for_every_phase() {
        let journey = load_accepted_product_journey().expect("accepted product journey");
        let catalog = crate::load_embedded_catalog();
        assert!(catalog.is_clean());
        let project_id = StableId("project.product".to_owned());

        for phase in Phase::ALL {
            let guidance = derive_product_journey_guidance(
                journey,
                &catalog.catalog,
                phase,
                r"D:\product",
                ProductJourneyConsultationContext {
                    project_id: &project_id,
                    objective_digest: None,
                    work_focus_id: None,
                },
            )
            .expect("journey guidance");
            assert_eq!(guidance.phase, phase);
            assert_eq!(
                guidance.stage.contact_density,
                project_product_stage(journey, phase)
                    .unwrap()
                    .contact_density
            );
            assert_eq!(
                guidance.catalog.eligible_count,
                crate::eligible_count(&catalog.catalog, phase)
            );
            assert_eq!(guidance.catalog.status_argv[0], "forge-core");
            assert_eq!(
                guidance.catalog.detail_argv.workflow_id_token,
                "__FORGE_CAPABILITY_ID__"
            );
            assert!(
                serde_json::to_vec(&guidance).expect("guidance JSON").len() <= 2048,
                "{phase} guidance exceeded the compactness budget"
            );
        }
    }

    #[test]
    fn catalog_consultation_key_changes_only_with_semantic_journey_context() {
        let journey = load_accepted_product_journey().expect("accepted product journey");
        let catalog = crate::load_embedded_catalog();
        let project_id = StableId("project.product".to_owned());
        let other_project_id = StableId("project.other".to_owned());
        let focus_id = StableId("focus.first".to_owned());
        let other_focus_id = StableId("focus.second".to_owned());
        let objective = format!("sha256:{}", "a".repeat(64));
        let other_objective = format!("sha256:{}", "b".repeat(64));
        let derive = |phase, project_id, objective_digest, work_focus_id, root| {
            derive_product_journey_guidance(
                journey,
                &catalog.catalog,
                phase,
                root,
                ProductJourneyConsultationContext {
                    project_id,
                    objective_digest,
                    work_focus_id,
                },
            )
            .expect("journey guidance")
            .catalog
            .consultation
            .key
        };

        let baseline = derive(
            Phase::Discovery,
            &project_id,
            Some(&objective),
            Some(&focus_id),
            r"D:\product",
        );
        assert_eq!(
            baseline,
            derive(
                Phase::Discovery,
                &project_id,
                Some(&objective),
                Some(&focus_id),
                r"D:\different-checkout",
            ),
            "checkout or routine readback location is not a new journey event"
        );
        assert_ne!(
            baseline,
            derive(
                Phase::Discovery,
                &other_project_id,
                Some(&objective),
                Some(&focus_id),
                r"D:\product",
            )
        );
        assert_ne!(
            baseline,
            derive(
                Phase::Discovery,
                &project_id,
                Some(&other_objective),
                Some(&focus_id),
                r"D:\product",
            )
        );
        assert_ne!(
            baseline,
            derive(
                Phase::Discovery,
                &project_id,
                Some(&objective),
                Some(&other_focus_id),
                r"D:\product",
            )
        );
        assert_ne!(
            baseline,
            derive(
                Phase::Plan,
                &project_id,
                Some(&objective),
                Some(&focus_id),
                r"D:\product",
            )
        );
    }

    #[test]
    fn guidance_rejects_a_project_root_that_breaks_the_compactness_budget() {
        let journey = load_accepted_product_journey().expect("accepted product journey");
        let catalog = crate::load_embedded_catalog();
        let oversized_root = format!("D:\\{}", "nested\\".repeat(300));
        let project_id = StableId("project.product".to_owned());

        let rejection = derive_product_journey_guidance(
            journey,
            &catalog.catalog,
            Phase::Discovery,
            &oversized_root,
            ProductJourneyConsultationContext {
                project_id: &project_id,
                objective_digest: None,
                work_focus_id: None,
            },
        )
        .expect_err("oversized guidance must fail closed");

        assert_eq!(
            rejection.issues[0].code,
            ProductJourneyIssueCode::GuidanceTooLarge
        );
    }
}
