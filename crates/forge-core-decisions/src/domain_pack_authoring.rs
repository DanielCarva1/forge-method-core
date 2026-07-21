//! Pure C7.1 candidate-only Domain Pack authoring decisions.
//!
//! This module composes the established P6a/P6b/P6c validators into a compact
//! author workflow. It has no filesystem, network, subprocess, trust-anchor,
//! signing, publishing, lifecycle, apply, commit, or activation capability.

use forge_core_contracts::{
    DomainAdapterProtocol, DomainEvaluatorImplementation, DomainPackArtifactBinding,
    DomainPackAuthorArtifactRefs, DomainPackAuthorCheck, DomainPackAuthorCheckKind,
    DomainPackAuthorCheckStatus, DomainPackAuthorCompatibilityReadiness,
    DomainPackAuthorCompatibilityStatus, DomainPackAuthorExactLockComparison,
    DomainPackAuthorIssue, DomainPackAuthorIssueCode, DomainPackAuthorLearningReadiness,
    DomainPackAuthorPackTemplate, DomainPackAuthorPromotionReadiness,
    DomainPackAuthorRawArtifactTemplate, DomainPackAuthorRawContentTemplate,
    DomainPackAuthorRawSidecars, DomainPackAuthorReviewedRegistryReadiness,
    DomainPackAuthorReviewedRegistryReadinessStatus, DomainPackAuthorSkeleton,
    DomainPackAuthorSkeletonDocument, DomainPackAuthorSkeletonRequestDocument,
    DomainPackAuthorSkeletonStatus, DomainPackAuthorTestReport, DomainPackAuthorTestReportDocument,
    DomainPackAuthorTestRequestDocument, DomainPackAuthorTestStatus, DomainPackCandidateAuthority,
    DomainPackCandidateInput, DomainPackCompatibility, DomainPackCompatibilityIssue,
    DomainPackCompatibilityIssueCode, DomainPackCompatibilityStatus, DomainPackCompositionGap,
    DomainPackCompositionGapCode, DomainPackCompositionIssue, DomainPackCompositionIssueCode,
    DomainPackCompositionRequest, DomainPackCompositionRequestDocument,
    DomainPackCompositionStatus, DomainPackContent, DomainPackContentBinding,
    DomainPackContentDocument, DomainPackIdentity, DomainPackManifest, DomainPackManifestDocument,
    DomainPackProvenance, DomainPackReviewedEligibility, DomainPackVersionReference, StableId,
    WorkflowGovernancePolicyOverlay, DOMAIN_PACK_AUTHORING_SCHEMA_VERSION,
    DOMAIN_PACK_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    compose_domain_packs, evaluate_domain_pack_compatibility, evaluate_domain_pack_promotion,
    evaluate_domain_pack_reviewed_registry_evolution, validate_domain_pack_candidate,
    DomainPackCandidateMaterial, DomainPackCompatibilityInput, DomainPackPromotionEvaluationInput,
    DomainPackPromotionReadinessStatus, DomainPackReviewedRegistryEvolutionInput,
    DomainPackReviewedRegistryEvolutionStatus, MAX_DOMAIN_PACK_DIAGNOSTICS,
};

const MAX_AUTHORING_DIAGNOSTICS: usize = MAX_DOMAIN_PACK_DIAGNOSTICS;

#[derive(Serialize)]
struct SkeletonDigestSubject<'a> {
    schema_version: &'a str,
    request_id: &'a StableId,
    authority: DomainPackCandidateAuthority,
    status: DomainPackAuthorSkeletonStatus,
    template: &'a Option<DomainPackAuthorPackTemplate>,
    issues: &'a [DomainPackAuthorIssue],
}

#[derive(Serialize)]
struct ReportDigestSubject<'a> {
    schema_version: &'a str,
    report_id: &'a StableId,
    authority: DomainPackCandidateAuthority,
    candidate: &'a DomainPackVersionReference,
    status: DomainPackAuthorTestStatus,
    structural: &'a DomainPackAuthorCheck,
    artifact_binding: &'a DomainPackAuthorCheck,
    composition: &'a DomainPackAuthorCheck,
    compatibility: &'a Option<DomainPackAuthorCompatibilityReadiness>,
    learning: &'a Option<DomainPackAuthorLearningReadiness>,
    reviewed_registry: &'a Option<DomainPackAuthorReviewedRegistryReadiness>,
    adversarial: &'a DomainPackAuthorCheck,
    gaps: &'a [DomainPackAuthorIssue],
    issues: &'a [DomainPackAuthorIssue],
}

/// Generates a minimal generic candidate skeleton in memory. The exact output
/// bytes are bound but never written: a CLI adapter is the only appropriate
/// owner for choosing any target directory or filesystem operation.
#[must_use]
pub fn generate_domain_pack_author_skeleton(
    request_document: &DomainPackAuthorSkeletonRequestDocument,
) -> DomainPackAuthorSkeletonDocument {
    let request = &request_document.domain_pack_author_skeleton_request;
    let mut issues = skeleton_request_issues(request_document);
    let mut template = None;

    if issues.is_empty() {
        let content = DomainPackContentDocument {
            schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
            domain_pack_content: DomainPackContent {
                pack: DomainPackVersionReference {
                    publisher: request.publisher.clone(),
                    name: request.name.clone(),
                    version: request.version.clone(),
                },
                namespace: request.namespace.clone(),
                workflow_overlay: WorkflowGovernancePolicyOverlay {
                    id: StableId(format!("{}.authoring-overlay", request.namespace.0)),
                    base_bundle_id: request.core.bundle_id.clone(),
                    policies: Vec::new(),
                },
                provided_domains: Vec::new(),
                provided_capabilities: Vec::new(),
                hazards: Vec::new(),
                lifecycle_models: Vec::new(),
                evaluators: Vec::new(),
                fixtures: Vec::new(),
                adapters: Vec::new(),
            },
        };
        let content_raw = yaml_bytes(&content);
        let content_binding = DomainPackContentBinding {
            content_ref: request.artifact_refs.content_ref.clone(),
            raw_sha256: sha256_digest(&content_raw),
            canonical_sha256: canonical_digest(&content),
        };

        let license_value = serde_json::json!({
            "schema_version": DOMAIN_PACK_AUTHORING_SCHEMA_VERSION,
            "license_spdx_expression": request.provenance.license_spdx_expression.clone(),
            "authors": request.provenance.authors.clone(),
            "notice": "candidate-only authoring template; replace with the complete license text",
        });
        let license_raw = yaml_bytes(&license_value);
        let license_binding = DomainPackArtifactBinding {
            artifact_ref: request.artifact_refs.license_ref.clone(),
            raw_sha256: sha256_digest(&license_raw),
            canonical_sha256: canonical_digest(&license_value),
        };

        let manifest = DomainPackManifestDocument {
            schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
            domain_pack_manifest: DomainPackManifest {
                identity: DomainPackIdentity {
                    publisher: request.publisher.clone(),
                    name: request.name.clone(),
                    namespace: request.namespace.clone(),
                    version: request.version.clone(),
                },
                authority: DomainPackCandidateAuthority::CandidateOnly,
                compatibility: DomainPackCompatibility {
                    pack_schema_requirement: "=0.1.0".to_owned(),
                    forge_core_requirement: format!("={}", request.forge_core_version),
                },
                provenance: DomainPackProvenance {
                    source_kind: request.provenance.source_kind,
                    source_uri: request.provenance.source_uri.clone(),
                    source_revision: request.provenance.source_revision.clone(),
                    source_digest: request.provenance.source_digest.clone(),
                    authors: request.provenance.authors.clone(),
                    license_spdx_expression: request.provenance.license_spdx_expression.clone(),
                    license_text: license_binding.clone(),
                },
                content: content_binding.clone(),
                // Explicit editable placeholders. Their empty values do not
                // create implicit resolution, conflict, or replacement behavior.
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                replacement_slots: Vec::new(),
                replacement_declarations: Vec::new(),
            },
        };
        let manifest_raw = yaml_bytes(&manifest);
        let candidate = DomainPackCandidateInput {
            manifest_binding: DomainPackArtifactBinding {
                artifact_ref: request.artifact_refs.manifest_ref.clone(),
                raw_sha256: sha256_digest(&manifest_raw),
                canonical_sha256: canonical_digest(&manifest),
            },
            manifest,
            content,
        };
        let material = DomainPackCandidateMaterial {
            publisher: &request.publisher.0,
            name: &request.name.0,
            version: &request.version,
            manifest_raw: &manifest_raw,
            content_raw: &content_raw,
            license_raw: &license_raw,
        };
        issues.extend(map_composition_issues(validate_domain_pack_candidate(
            &candidate,
            &material,
            &request.forge_core_version,
        )));
        finish_issues(&mut issues);

        if issues.is_empty() {
            let composition_request = DomainPackCompositionRequestDocument {
                schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
                domain_pack_composition_request: DomainPackCompositionRequest {
                    request_id: request.request_id.clone(),
                    authority: DomainPackCandidateAuthority::CandidateOnly,
                    forge_core_version: request.forge_core_version.clone(),
                    core: request.core.clone(),
                    requirements: request
                        .requirements
                        .domain_pack_project_requirements
                        .clone(),
                    candidates: vec![candidate.clone()],
                },
            };
            template = Some(DomainPackAuthorPackTemplate {
                manifest: DomainPackAuthorRawArtifactTemplate {
                    artifact_ref: request.artifact_refs.manifest_ref.clone(),
                    raw_sha256: candidate.manifest_binding.raw_sha256.clone(),
                    canonical_sha256: candidate.manifest_binding.canonical_sha256.clone(),
                    raw_bytes: manifest_raw,
                },
                content: DomainPackAuthorRawContentTemplate {
                    content_ref: content_binding.content_ref,
                    raw_sha256: content_binding.raw_sha256,
                    canonical_sha256: content_binding.canonical_sha256,
                    raw_bytes: content_raw,
                },
                license: DomainPackAuthorRawArtifactTemplate {
                    artifact_ref: license_binding.artifact_ref,
                    raw_sha256: license_binding.raw_sha256,
                    canonical_sha256: license_binding.canonical_sha256,
                    raw_bytes: license_raw,
                },
                candidate,
                requirements: request.requirements.clone(),
                composition_request,
            });
        }
    }

    finish_issues(&mut issues);
    let status = if template.is_some() {
        DomainPackAuthorSkeletonStatus::Generated
    } else {
        DomainPackAuthorSkeletonStatus::Blocked
    };
    let mut skeleton = DomainPackAuthorSkeleton {
        request_id: request.request_id.clone(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        status,
        template,
        issues,
        skeleton_digest: String::new(),
    };
    skeleton.skeleton_digest = canonical_digest(&SkeletonDigestSubject {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION,
        request_id: &skeleton.request_id,
        authority: skeleton.authority,
        status: skeleton.status,
        template: &skeleton.template,
        issues: &skeleton.issues,
    });
    DomainPackAuthorSkeletonDocument {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION.to_owned(),
        domain_pack_author_skeleton: skeleton,
    }
}

/// Runs a deterministic candidate-only author test over supplied raw sidecars
/// and typed documents. Existing P6 validators/evaluators remain the source of
/// all composition, exact-lock, promotion, and registry-evolution semantics.
#[must_use]
pub fn evaluate_domain_pack_author_test(
    request_document: &DomainPackAuthorTestRequestDocument,
) -> DomainPackAuthorTestReportDocument {
    let request = &request_document.domain_pack_author_test_request;
    let candidate_ref = candidate_version_reference(&request.candidate);
    let mut structural_issues = request_contract_issues(request_document);
    let target_in_request = request
        .composition_request
        .domain_pack_composition_request
        .candidates
        .iter()
        .any(|candidate| candidate == &request.candidate);
    if !target_in_request {
        structural_issues.push(author_issue(
            DomainPackAuthorIssueCode::CoordinateVersionMismatch,
            "composition_request.candidates",
            "the direct author-test candidate is not an exact candidate in the composition request",
        ));
    }
    finish_issues(&mut structural_issues);
    let structural = check(
        DomainPackAuthorCheckKind::Structural,
        structural_issues,
        false,
    );

    let mut ordered_sidecars = request.raw_sidecars.iter().collect::<Vec<_>>();
    ordered_sidecars.sort_by_key(|sidecars| sidecar_order_key(sidecars));
    let materials = ordered_sidecars
        .iter()
        .map(|sidecars| DomainPackCandidateMaterial {
            publisher: &sidecars.pack.publisher.0,
            name: &sidecars.pack.name.0,
            version: &sidecars.pack.version,
            manifest_raw: &sidecars.manifest_raw,
            content_raw: &sidecars.content_raw,
            license_raw: &sidecars.license_raw,
        })
        .collect::<Vec<_>>();
    let fallback_material = DomainPackCandidateMaterial {
        publisher: "",
        name: "",
        version: "",
        manifest_raw: &[],
        content_raw: &[],
        license_raw: &[],
    };
    let target_material = materials.iter().find(|material| {
        material.publisher == candidate_ref.publisher.0
            && material.name == candidate_ref.name.0
            && material.version == candidate_ref.version
    });
    let mut artifact_issues = if target_material.is_none() {
        vec![author_issue(
            DomainPackAuthorIssueCode::MissingMaterial,
            "raw_sidecars",
            "no raw manifest/content/license sidecars were supplied for the direct candidate",
        )]
    } else {
        Vec::new()
    };
    artifact_issues.extend(map_composition_issues(validate_domain_pack_candidate(
        &request.candidate,
        target_material.unwrap_or(&fallback_material),
        &request
            .composition_request
            .domain_pack_composition_request
            .forge_core_version,
    )));
    finish_issues(&mut artifact_issues);
    let artifact_binding = check(
        DomainPackAuthorCheckKind::ArtifactBinding,
        artifact_issues,
        false,
    );

    let composition_projection = compose_domain_packs(&request.composition_request, &materials);
    let composition_data = &composition_projection.domain_pack_composition_projection;
    let mut composition_issues = map_composition_issues(composition_data.issues.clone());
    finish_issues(&mut composition_issues);
    let composition = DomainPackAuthorCheck {
        kind: DomainPackAuthorCheckKind::Composition,
        status: if composition_data.status == DomainPackCompositionStatus::Composable
            && composition_issues.is_empty()
        {
            DomainPackAuthorCheckStatus::Passed
        } else {
            DomainPackAuthorCheckStatus::Failed
        },
        issues: composition_issues,
    };
    let mut gaps = map_composition_gaps(composition_data.gaps.clone());
    finish_issues(&mut gaps);

    let compatibility = request.compatibility.as_ref().map(compatibility_readiness);
    let learning = request.learning.as_ref().map(learning_readiness);
    let reviewed_registry = request
        .reviewed_registry
        .as_ref()
        .map(|evidence| reviewed_registry_readiness(evidence, &candidate_ref));
    let adversarial = adversarial_check(&request.candidate);

    let mut all_issues = Vec::new();
    all_issues.extend(structural.issues.clone());
    all_issues.extend(artifact_binding.issues.clone());
    all_issues.extend(composition.issues.clone());
    all_issues.extend(gaps.clone());
    if let Some(section) = &compatibility {
        all_issues.extend(section.issues.clone());
    }
    if let Some(section) = &learning {
        all_issues.extend(section.issues.clone());
    }
    if let Some(section) = &reviewed_registry {
        all_issues.extend(section.issues.clone());
    }
    all_issues.extend(adversarial.issues.clone());
    finish_issues(&mut all_issues);

    let status = if all_issues.is_empty() {
        DomainPackAuthorTestStatus::CandidateReady
    } else {
        DomainPackAuthorTestStatus::Blocked
    };

    let mut report = DomainPackAuthorTestReport {
        report_id: request.request_id.clone(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        candidate: candidate_ref,
        status,
        structural,
        artifact_binding,
        composition,
        compatibility,
        learning,
        reviewed_registry,
        adversarial,
        gaps,
        issues: all_issues,
        report_digest: String::new(),
    };
    report.report_digest = canonical_digest(&ReportDigestSubject {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION,
        report_id: &report.report_id,
        authority: report.authority,
        candidate: &report.candidate,
        status: report.status,
        structural: &report.structural,
        artifact_binding: &report.artifact_binding,
        composition: &report.composition,
        compatibility: &report.compatibility,
        learning: &report.learning,
        reviewed_registry: &report.reviewed_registry,
        adversarial: &report.adversarial,
        gaps: &report.gaps,
        issues: &report.issues,
    });
    DomainPackAuthorTestReportDocument {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION.to_owned(),
        domain_pack_author_test_report: report,
    }
}

fn skeleton_request_issues(
    document: &DomainPackAuthorSkeletonRequestDocument,
) -> Vec<DomainPackAuthorIssue> {
    let request = &document.domain_pack_author_skeleton_request;
    let mut issues = document.validate_sealed_core_binding();
    if document.schema_version != DOMAIN_PACK_AUTHORING_SCHEMA_VERSION {
        issues.push(author_issue(
            DomainPackAuthorIssueCode::InvalidAuthorContract,
            "schema_version",
            "unsupported Domain Pack authoring schema version",
        ));
    }
    if request.requirements.schema_version != DOMAIN_PACK_SCHEMA_VERSION {
        issues.push(author_issue(
            DomainPackAuthorIssueCode::InvalidAuthorContract,
            "requirements.schema_version",
            "project requirements must use the supported Domain Pack schema version",
        ));
    }
    if request.request_id.0.trim().is_empty()
        || request.publisher.0.trim().is_empty()
        || request.name.0.trim().is_empty()
        || request.namespace.0.trim().is_empty()
        || request.version.trim().is_empty()
        || request.forge_core_version.trim().is_empty()
    {
        issues.push(author_issue(
            DomainPackAuthorIssueCode::InvalidAuthorContract,
            "skeleton_request.identity",
            "request id, publisher, name, namespace, version, and Forge Core version are required",
        ));
    }
    if request.provenance.source_uri.trim().is_empty()
        || request.provenance.source_revision.trim().is_empty()
        || request.provenance.authors.is_empty()
        || request.provenance.license_spdx_expression.trim().is_empty()
        || !valid_sha256_digest(&request.provenance.source_digest)
    {
        issues.push(author_issue(
            DomainPackAuthorIssueCode::InvalidAuthorContract,
            "provenance",
            "source, revision, valid source digest, authors, and SPDX license are required",
        ));
    }
    if !valid_artifact_refs(&request.artifact_refs) {
        issues.push(author_issue(
            DomainPackAuthorIssueCode::InvalidAuthorContract,
            "artifact_refs",
            "manifest, content, and license artifact refs must be safe non-empty relative paths",
        ));
    }
    issues
}

fn request_contract_issues(
    document: &DomainPackAuthorTestRequestDocument,
) -> Vec<DomainPackAuthorIssue> {
    let request = &document.domain_pack_author_test_request;
    let mut issues = Vec::new();
    if document.schema_version != DOMAIN_PACK_AUTHORING_SCHEMA_VERSION {
        issues.push(author_issue(
            DomainPackAuthorIssueCode::InvalidAuthorContract,
            "schema_version",
            "unsupported Domain Pack authoring schema version",
        ));
    }
    if request.request_id.0.trim().is_empty() {
        issues.push(author_issue(
            DomainPackAuthorIssueCode::InvalidAuthorContract,
            "request_id",
            "author-test request id must not be blank",
        ));
    }
    if request.composition_request.schema_version != DOMAIN_PACK_SCHEMA_VERSION {
        issues.push(author_issue(
            DomainPackAuthorIssueCode::InvalidAuthorContract,
            "composition_request.schema_version",
            "composition request must use the supported Domain Pack schema version",
        ));
    }
    issues
}

fn compatibility_readiness(
    comparison: &DomainPackAuthorExactLockComparison,
) -> DomainPackAuthorCompatibilityReadiness {
    let result = evaluate_domain_pack_compatibility(&DomainPackCompatibilityInput {
        report_id: comparison.comparison_id.clone(),
        operation: comparison.operation.clone(),
        sealed_core: comparison.sealed_core.clone(),
        from_lock: comparison.current_lock.clone(),
        to_lock: comparison.proposed_lock.clone(),
    });
    let report = &result.domain_pack_compatibility_report;
    let mut issues = report
        .issues
        .iter()
        .map(map_compatibility_issue)
        .collect::<Vec<_>>();
    finish_issues(&mut issues);
    DomainPackAuthorCompatibilityReadiness {
        status: match report.status {
            DomainPackCompatibilityStatus::Compatible => {
                DomainPackAuthorCompatibilityStatus::Compatible
            }
            DomainPackCompatibilityStatus::Degraded => {
                DomainPackAuthorCompatibilityStatus::Degraded
            }
            DomainPackCompatibilityStatus::Blocked => DomainPackAuthorCompatibilityStatus::Blocked,
        },
        report_digest: report.report_digest.clone(),
        issues,
    }
}

fn learning_readiness(
    evidence: &forge_core_contracts::DomainPackAuthorLearningEvidence,
) -> DomainPackAuthorLearningReadiness {
    let result = evaluate_domain_pack_promotion(&DomainPackPromotionEvaluationInput {
        dossier: &evidence.dossier,
        candidates: &evidence.candidates,
        independent_reviews: &evidence.independent_reviews,
        conflicts: &evidence.conflicts,
    });
    let mut issues = result
        .issues
        .iter()
        .map(|issue| map_learning_issue(issue.code, &issue.path, &issue.message))
        .collect::<Vec<_>>();
    finish_issues(&mut issues);
    DomainPackAuthorLearningReadiness {
        status: match result.status {
            DomainPackPromotionReadinessStatus::ReadyForTrustedReview => {
                DomainPackAuthorPromotionReadiness::ReadyForReview
            }
            DomainPackPromotionReadinessStatus::ReviewRequired => {
                DomainPackAuthorPromotionReadiness::ReviewRequired
            }
            DomainPackPromotionReadinessStatus::Blocked => {
                DomainPackAuthorPromotionReadiness::Blocked
            }
        },
        evaluation_digest: result.evaluation_digest,
        review_request_digest: result.review_request.map(|request| request.request_digest),
        issues,
    }
}

fn reviewed_registry_readiness(
    evidence: &forge_core_contracts::DomainPackAuthorReviewedRegistryEvidence,
    candidate: &DomainPackVersionReference,
) -> DomainPackAuthorReviewedRegistryReadiness {
    let result = evaluate_domain_pack_reviewed_registry_evolution(
        &DomainPackReviewedRegistryEvolutionInput {
            current: evidence.current.as_ref(),
            proposed: &evidence.proposed,
            competing_heads: &evidence.competing_heads,
        },
    );
    let mut issues = result
        .issues
        .iter()
        .map(|issue| map_learning_issue(issue.code, &issue.path, &issue.message))
        .collect::<Vec<_>>();
    for entry in &evidence.proposed.domain_pack_reviewed_registry.entries {
        if &entry.pack == candidate {
            let code = match entry.eligibility {
                DomainPackReviewedEligibility::EligibleReviewed => continue,
                DomainPackReviewedEligibility::IneligibleDeprecated => {
                    DomainPackAuthorIssueCode::DeprecatedRecord
                }
                DomainPackReviewedEligibility::IneligibleRevoked => {
                    DomainPackAuthorIssueCode::RevokedRecord
                }
                DomainPackReviewedEligibility::IneligibleSuperseded => {
                    DomainPackAuthorIssueCode::SupersededRecord
                }
            };
            issues.push(author_issue(
                code,
                "reviewed_registry.proposed.entries",
                "the candidate's reviewed record is ineligible for a new trust boundary",
            ));
        }
    }
    finish_issues(&mut issues);
    DomainPackAuthorReviewedRegistryReadiness {
        status: match result.status {
            DomainPackReviewedRegistryEvolutionStatus::AdmissibleCandidate => {
                DomainPackAuthorReviewedRegistryReadinessStatus::AdmissibleCandidate
            }
            DomainPackReviewedRegistryEvolutionStatus::GenesisCandidate => {
                DomainPackAuthorReviewedRegistryReadinessStatus::GenesisCandidate
            }
            DomainPackReviewedRegistryEvolutionStatus::Replay => {
                DomainPackAuthorReviewedRegistryReadinessStatus::Replay
            }
            DomainPackReviewedRegistryEvolutionStatus::Blocked => {
                DomainPackAuthorReviewedRegistryReadinessStatus::Blocked
            }
        },
        evaluation_digest: result.evaluation_digest,
        issues,
    }
}

fn adversarial_check(candidate: &DomainPackCandidateInput) -> DomainPackAuthorCheck {
    let mut issues = Vec::new();
    let content = &candidate.content.domain_pack_content;
    let value = serde_json::to_value(content).unwrap_or(serde_json::Value::Null);
    scan_prose(&value, "candidate.content", false, &mut issues);
    for capability in &content.provided_capabilities {
        if matches!(
            capability.kind,
            forge_core_contracts::DomainPackCapabilityKind::Tool
                | forge_core_contracts::DomainPackCapabilityKind::Runtime
                | forge_core_contracts::DomainPackCapabilityKind::Credential
                | forge_core_contracts::DomainPackCapabilityKind::ExternalAuthority
        ) {
            issues.push(author_issue(
                DomainPackAuthorIssueCode::ExternalExecutableCapabilityClaim,
                format!("candidate.content.provided_capabilities.{}", capability.id.0),
                "external, executable, runtime, credential, and authority capability claims remain unavailable in the pure author workflow",
            ));
        }
    }
    for adapter in &content.adapters {
        if adapter.protocol != DomainAdapterProtocol::BuiltIn {
            issues.push(author_issue(
                DomainPackAuthorIssueCode::ExternalExecutableCapabilityClaim,
                format!("candidate.content.adapters.{}", adapter.id.0),
                "a non-built-in adapter declaration cannot be exercised or treated as runtime capability",
            ));
        }
    }
    for evaluator in &content.evaluators {
        if let DomainEvaluatorImplementation::Adapter { protocol, .. } = &evaluator.implementation {
            if *protocol != DomainAdapterProtocol::BuiltIn {
                issues.push(author_issue(
                    DomainPackAuthorIssueCode::ExternalExecutableCapabilityClaim,
                    format!("candidate.content.evaluators.{}", evaluator.id.0),
                    "an adapter-backed evaluator remains a declaration and cannot be executed by author testing",
                ));
            }
        }
    }
    finish_issues(&mut issues);
    check(DomainPackAuthorCheckKind::Adversarial, issues, false)
}

fn scan_prose(
    value: &serde_json::Value,
    path: &str,
    prose_context: bool,
    issues: &mut Vec<DomainPackAuthorIssue>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                let child_is_prose = prose_context
                    || matches!(
                        key.as_str(),
                        "description" | "explanation" | "statement" | "rationale" | "steps"
                    );
                scan_prose(child, &child_path, child_is_prose, issues);
            }
        }
        serde_json::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                scan_prose(child, &format!("{path}[{index}]"), prose_context, issues);
            }
        }
        serde_json::Value::String(text) if prose_context && unsafe_prose(text) => {
            issues.push(author_issue(
                DomainPackAuthorIssueCode::UnsafePromptOrToolProse,
                path,
                "untrusted prose contains prompt-injection or tool-execution language and must remain inert data",
            ));
        }
        _ => {}
    }
}

fn unsafe_prose(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    [
        "ignore previous",
        "system prompt",
        "developer message",
        "prompt injection",
        "tool call",
        "run this command",
        "execute this command",
        "shell command",
        "subprocess",
        "curl ",
        "wget ",
        "rm -rf",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn map_composition_issues(issues: Vec<DomainPackCompositionIssue>) -> Vec<DomainPackAuthorIssue> {
    issues
        .into_iter()
        .map(|issue| map_composition_issue(&issue))
        .collect()
}

fn map_composition_issue(issue: &DomainPackCompositionIssue) -> DomainPackAuthorIssue {
    let code = match issue.code {
        DomainPackCompositionIssueCode::ContentBindingMismatch => {
            if issue.path.contains("fixture") {
                DomainPackAuthorIssueCode::InvalidFixtureVariant
            } else if issue.path.contains("material") || issue.path.contains("identity") {
                DomainPackAuthorIssueCode::CoordinateVersionMismatch
            } else {
                DomainPackAuthorIssueCode::RawCanonicalMismatch
            }
        }
        DomainPackCompositionIssueCode::InvalidEvaluatorDeclaration => {
            DomainPackAuthorIssueCode::InvalidEvaluatorVariant
        }
        DomainPackCompositionIssueCode::InvalidAdapterDeclaration => {
            DomainPackAuthorIssueCode::InvalidAdapterVariant
        }
        DomainPackCompositionIssueCode::InvalidLifecycleModel => {
            DomainPackAuthorIssueCode::InvalidLifecycleVariant
        }
        DomainPackCompositionIssueCode::InvalidCapabilityDeclaration => {
            DomainPackAuthorIssueCode::InvalidCapabilityVariant
        }
        DomainPackCompositionIssueCode::DanglingReference => {
            DomainPackAuthorIssueCode::DanglingReference
        }
        DomainPackCompositionIssueCode::DependencyCycle => {
            DomainPackAuthorIssueCode::DependencyCycle
        }
        DomainPackCompositionIssueCode::DeclaredConflict
        | DomainPackCompositionIssueCode::IncompatibleDependency
        | DomainPackCompositionIssueCode::ReplacementNotBilateral
        | DomainPackCompositionIssueCode::ReplacementTargetMismatch => {
            DomainPackAuthorIssueCode::DependencyConflict
        }
        DomainPackCompositionIssueCode::MissingDependency => {
            DomainPackAuthorIssueCode::MissingMaterial
        }
        DomainPackCompositionIssueCode::CoreShadow => DomainPackAuthorIssueCode::CoreShadowing,
        DomainPackCompositionIssueCode::PackShadow => DomainPackAuthorIssueCode::PackShadowing,
        DomainPackCompositionIssueCode::DuplicateNamespace
        | DomainPackCompositionIssueCode::DuplicatePack
        | DomainPackCompositionIssueCode::DuplicateContribution => {
            DomainPackAuthorIssueCode::NamespaceCollision
        }
        DomainPackCompositionIssueCode::IncompatibleForgeCore => {
            DomainPackAuthorIssueCode::IncompatibleCore
        }
        DomainPackCompositionIssueCode::InvalidComposedBundle if issue.path.contains("fixture") => {
            DomainPackAuthorIssueCode::InvalidFixtureVariant
        }
        DomainPackCompositionIssueCode::ResourceLimitExceeded => {
            DomainPackAuthorIssueCode::ResourceLimitExceeded
        }
        _ => DomainPackAuthorIssueCode::InvalidAuthorContract,
    };
    author_issue(code, &issue.path, &issue.message)
}

fn map_composition_gaps(gaps: Vec<DomainPackCompositionGap>) -> Vec<DomainPackAuthorIssue> {
    gaps.into_iter()
        .map(|gap| {
            let code = match gap.code {
                DomainPackCompositionGapCode::MissingDomain => {
                    DomainPackAuthorIssueCode::MissingDomain
                }
                DomainPackCompositionGapCode::MissingCapability
                | DomainPackCompositionGapCode::MissingEvaluator
                | DomainPackCompositionGapCode::MissingAdapter => {
                    DomainPackAuthorIssueCode::MissingCapability
                }
                DomainPackCompositionGapCode::MissingDependency => {
                    DomainPackAuthorIssueCode::MissingMaterial
                }
            };
            author_issue(
                code,
                format!(
                    "composition.gaps.{}.{}",
                    gap.requirement_ref.0, gap.subject_ref.0
                ),
                &gap.message,
            )
        })
        .collect()
}

fn map_compatibility_issue(issue: &DomainPackCompatibilityIssue) -> DomainPackAuthorIssue {
    let code = match issue.code {
        DomainPackCompatibilityIssueCode::CoreChanged => {
            DomainPackAuthorIssueCode::IncompatibleCore
        }
        DomainPackCompatibilityIssueCode::RequirementsChangedWithoutIntent => {
            DomainPackAuthorIssueCode::IncompatibleProjectRequirements
        }
        DomainPackCompatibilityIssueCode::MissingRequiredDomain => {
            DomainPackAuthorIssueCode::MissingDomain
        }
        DomainPackCompatibilityIssueCode::MissingRequiredCapability
        | DomainPackCompatibilityIssueCode::ExecutableCapabilityDenied => {
            DomainPackAuthorIssueCode::MissingCapability
        }
        DomainPackCompatibilityIssueCode::RevokedTarget => DomainPackAuthorIssueCode::RevokedRecord,
        DomainPackCompatibilityIssueCode::NamespaceChanged => {
            DomainPackAuthorIssueCode::NamespaceCollision
        }
        DomainPackCompatibilityIssueCode::InvalidLockDigest => {
            DomainPackAuthorIssueCode::RawCanonicalMismatch
        }
        _ => DomainPackAuthorIssueCode::CompatibilityBlocked,
    };
    author_issue(code, &issue.path, &issue.message)
}

fn map_learning_issue(
    code: crate::DomainPackLearningIssueCode,
    path: &str,
    message: &str,
) -> DomainPackAuthorIssue {
    use crate::DomainPackLearningIssueCode;
    let author_code = match code {
        DomainPackLearningIssueCode::NoOpComparison
        | DomainPackLearningIssueCode::NoOpRegistrySuccessor => {
            DomainPackAuthorIssueCode::NoOpComparison
        }
        DomainPackLearningIssueCode::RegressionDetected => {
            DomainPackAuthorIssueCode::RegressingComparison
        }
        DomainPackLearningIssueCode::NonIndependentJudge => {
            DomainPackAuthorIssueCode::NonIndependentReview
        }
        DomainPackLearningIssueCode::MissingIndependentReview => {
            DomainPackAuthorIssueCode::MissingIndependentReview
        }
        DomainPackLearningIssueCode::ReviewRejected => DomainPackAuthorIssueCode::RejectedReview,
        DomainPackLearningIssueCode::UnresolvedConflict
        | DomainPackLearningIssueCode::SemanticConflict => {
            DomainPackAuthorIssueCode::UnresolvedConflict
        }
        DomainPackLearningIssueCode::MissingCandidate => DomainPackAuthorIssueCode::MissingMaterial,
        DomainPackLearningIssueCode::ResourceLimitExceeded => {
            DomainPackAuthorIssueCode::ResourceLimitExceeded
        }
        DomainPackLearningIssueCode::RegistryChainMismatch
        | DomainPackLearningIssueCode::RegistryEquivocation
        | DomainPackLearningIssueCode::RegistryEntryRemoved
        | DomainPackLearningIssueCode::RegistryEntryRewritten
        | DomainPackLearningIssueCode::InvalidRegistryStage
        | DomainPackLearningIssueCode::TerminalReactivation
        | DomainPackLearningIssueCode::SupersessionTargetNotReviewed => {
            DomainPackAuthorIssueCode::ReviewedRegistryBlocked
        }
        _ => DomainPackAuthorIssueCode::LearningBlocked,
    };
    author_issue(author_code, path, message)
}

fn check(
    kind: DomainPackAuthorCheckKind,
    mut issues: Vec<DomainPackAuthorIssue>,
    not_supplied: bool,
) -> DomainPackAuthorCheck {
    finish_issues(&mut issues);
    DomainPackAuthorCheck {
        kind,
        status: if not_supplied {
            DomainPackAuthorCheckStatus::NotSupplied
        } else if issues.is_empty() {
            DomainPackAuthorCheckStatus::Passed
        } else {
            DomainPackAuthorCheckStatus::Failed
        },
        issues,
    }
}

fn candidate_version_reference(candidate: &DomainPackCandidateInput) -> DomainPackVersionReference {
    let identity = &candidate.manifest.domain_pack_manifest.identity;
    DomainPackVersionReference {
        publisher: identity.publisher.clone(),
        name: identity.name.clone(),
        version: identity.version.clone(),
    }
}

fn sidecar_order_key(
    sidecars: &DomainPackAuthorRawSidecars,
) -> (String, String, String, String, String, String) {
    (
        sidecars.pack.publisher.0.clone(),
        sidecars.pack.name.0.clone(),
        sidecars.pack.version.clone(),
        sha256_digest(&sidecars.manifest_raw),
        sha256_digest(&sidecars.content_raw),
        sha256_digest(&sidecars.license_raw),
    )
}

fn valid_artifact_refs(refs: &DomainPackAuthorArtifactRefs) -> bool {
    [&refs.manifest_ref, &refs.content_ref, &refs.license_ref]
        .iter()
        .all(|value| valid_repo_ref(&value.0))
}

fn valid_repo_ref(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with(['/', '\\'])
        && !value.contains('\\')
        && !value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && !value.contains(':')
}

fn valid_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn author_issue(
    code: DomainPackAuthorIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) -> DomainPackAuthorIssue {
    DomainPackAuthorIssue {
        code,
        path: path.into(),
        message: message.into(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
    }
}

fn finish_issues(issues: &mut Vec<DomainPackAuthorIssue>) {
    issues.sort_by(|left, right| {
        (left.code, &left.path, &left.message).cmp(&(right.code, &right.path, &right.message))
    });
    issues.dedup_by(|left, right| {
        left.code == right.code && left.path == right.path && left.message == right.message
    });
    if issues.len() > MAX_AUTHORING_DIAGNOSTICS {
        issues.truncate(MAX_AUTHORING_DIAGNOSTICS.saturating_sub(1));
        issues.push(author_issue(
            DomainPackAuthorIssueCode::ResourceLimitExceeded,
            "authoring.diagnostics",
            "authoring diagnostic count exceeded the bounded limit",
        ));
    }
}

fn yaml_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    yaml_serde::to_string(value)
        .expect("closed authoring templates must serialize to YAML")
        .into_bytes()
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    serde_json_canonicalizer::to_vec(value).map_or_else(
        |_| sha256_digest(b"forge-domain-pack-authoring-canonical-encoding-failed"),
        |bytes| sha256_digest(&bytes),
    )
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
