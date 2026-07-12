use forge_core_contracts::{
    RepoPath, StableId, WorkflowCompatibilityField, WorkflowCompatibilityLifecycle,
    WorkflowCompatibilityReason, WorkflowCompatibilityReasonCode,
    WorkflowConsumerDiagnosticsPolicy, WorkflowDomainPackCandidate,
    WorkflowDomainPackDeferralReason, WorkflowGovernanceBundleDocument, WorkflowGovernancePolicy,
    WorkflowGovernanceReleaseManifest, WorkflowGovernanceReleaseManifestDocument,
    WorkflowLegacyCompatibilityAuthority, WorkflowMigrationBatch, WorkflowMigrationBatchAuthority,
    WorkflowMigrationBatchBinding, WorkflowMigrationBatchDocument, WorkflowMigrationBatchEvidence,
    WorkflowMigrationDisposition, WorkflowMigrationEvidenceReference,
    WorkflowMigrationPlanDocument, WorkflowReleaseBatchReference,
    WorkflowReleaseCompatibilityPolicy, WorkflowReleaseCompatibilityProjectionMode,
    WorkflowReleaseDispositionIntent, WorkflowReleaseWorkflowEntry,
    WorkflowRetirementAdmissionPolicy, WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION,
    WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION,
};
use forge_core_decisions::{
    evaluate_workflow_migration, load_catalog, load_workflow_documents,
    workflow_migration_batch_digest, workflow_release_legacy_digest, LoadedWorkflowDocument,
    WorkflowMigrationAudit,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::{Path, PathBuf};

const MANIFEST_PATH: &str = "contracts/migration/workflow-governance-release-foundation-v0.yaml";
const BATCH_PATH: &str = "contracts/migration/workflow-governance-batch-golden-path-v0.yaml";
const REPRESENTATIVE_EVIDENCE_PATH: &str =
    "contracts/evidence/workflow-release-representative-fixtures-v0.yaml";
const ADVERSARIAL_EVIDENCE_PATH: &str =
    "contracts/evidence/workflow-release-adversarial-fixtures-v0.yaml";
const SHADOW_EVIDENCE_PATH: &str = "contracts/evidence/workflow-release-shadow-report-v0.yaml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

#[derive(Serialize)]
struct PublicationEvidenceDocument<'a> {
    schema_version: &'static str,
    workflow_release_publication_evidence: PublicationEvidence<'a>,
}

#[derive(Serialize)]
struct PublicationEvidence<'a> {
    id: &'static str,
    evidence_class: PublicationEvidenceClass,
    publication_scope: &'static str,
    integrity_claim: &'static str,
    behavioral_sufficiency: BehavioralSufficiency,
    limitation: &'static str,
    references: Vec<PublicationEvidenceReference<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum PublicationEvidenceClass {
    RepresentativeFixtures,
    AdversarialFixtures,
    ShadowReport,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum BehavioralSufficiency {
    NotAsserted,
}

#[derive(Serialize)]
struct PublicationEvidenceReference<'a> {
    repo_path: &'a str,
    anchor: &'a str,
    relevance: &'a str,
}

struct GeneratedArtifact {
    relative_path: &'static str,
    bytes: Vec<u8>,
}

struct EvidenceArtifacts {
    representative: Vec<u8>,
    adversarial: Vec<u8>,
    shadow: Vec<u8>,
}

struct FoundationInputs {
    workflows: Vec<LoadedWorkflowDocument>,
    audit: WorkflowMigrationAudit,
    policies: Vec<WorkflowGovernancePolicy>,
}

#[derive(Default)]
struct DispositionCounts {
    migration: usize,
    domain: usize,
    compatibility: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mode = parse_mode()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let artifacts = generate(&root)?;
    match mode {
        Mode::Write => write_artifacts(&root, &artifacts)?,
        Mode::Check => check_artifacts(&root, &artifacts)?,
    }
    Ok(())
}

fn parse_mode() -> Result<Mode, Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [flag] if flag == "--write" => Ok(Mode::Write),
        [flag] if flag == "--check" => Ok(Mode::Check),
        _ => Err(error(
            "usage: cargo run -p forge-core-decisions --example generate_workflow_release_foundation -- (--write|--check)",
        )),
    }
}

fn generate(root: &Path) -> Result<Vec<GeneratedArtifact>, Box<dyn Error>> {
    let evidence = generate_evidence_artifacts(root)?;
    let foundation = load_foundation_inputs(root)?;
    let batch_id = id("workflow-batch.golden-path-v0");
    let batch = build_batch(&foundation, &evidence, &batch_id)?;
    let batch_digest = workflow_migration_batch_digest(&batch).map_err(error)?;
    let batch_bytes = yaml_bytes(&batch)?;
    let workflow_entries = build_manifest_entries(&foundation, &batch_id)?;
    let manifest = build_manifest(
        workflow_entries,
        foundation.audit.manifest.catalog_digest,
        batch_id,
        batch_digest,
    );
    Ok(assemble_artifacts(
        evidence,
        batch_bytes,
        yaml_bytes(&manifest)?,
    ))
}

fn generate_evidence_artifacts(root: &Path) -> Result<EvidenceArtifacts, Box<dyn Error>> {
    Ok(EvidenceArtifacts {
        representative: yaml_bytes(&representative_evidence(root)?)?,
        adversarial: yaml_bytes(&adversarial_evidence(root)?)?,
        shadow: yaml_bytes(&shadow_evidence(root)?)?,
    })
}

fn representative_evidence(root: &Path) -> Result<PublicationEvidenceDocument<'_>, Box<dyn Error>> {
    publication_evidence(
        root,
        "workflow-release-evidence.representative-fixtures.v0",
        PublicationEvidenceClass::RepresentativeFixtures,
        "Indexes representative P5c policy, trusted-kernel, and real CLI integration coverage used for release publication traceability.",
        &[
            (
                "contracts/workflow-governance/golden-path-v0.yaml",
                "workflow_governance_bundle",
                "The admitted fifteen-policy P5c golden-path composition used verbatim by the candidate batch.",
            ),
            (
                "crates/forge-core-kernel/tests/workflow_governance_golden_path.rs",
                "all_admitted_required_policies_use_signed_authority_and_reach_terminal_resume",
                "Representative trusted-kernel execution across every admitted required policy.",
            ),
            (
                "crates/forge-core-cli/tests/workflow_governance_integration_e2e.rs",
                "signed_cli_flow_completes_first_policy_and_resumes_capability_gap",
                "Real binary integration coverage for signed progression and resumable capability gaps.",
            ),
        ],
    )
}

fn adversarial_evidence(root: &Path) -> Result<PublicationEvidenceDocument<'_>, Box<dyn Error>> {
    publication_evidence(
        root,
        "workflow-release-evidence.adversarial-fixtures.v0",
        PublicationEvidenceClass::AdversarialFixtures,
        "Indexes concrete P5c fail-closed cases for contradictory evidence, stale evidence, replay, drift, and waiver boundaries.",
        &[
            (
                "crates/forge-core-decisions/tests/workflow_governance.rs",
                "contradictory_evidence_and_invented_completion_fail_closed",
                "Pure decision-layer adversarial evidence against invented or contested completion.",
            ),
            (
                "crates/forge-core-kernel/tests/workflow_governance_golden_path.rs",
                "expired_evidence_is_recomputed_as_stale_and_blocks_completion; signed_snapshot_and_head_bound_authorizations_reject_replay_and_drift",
                "Trusted-kernel adversarial coverage for stale evidence, replay, and snapshot drift.",
            ),
            (
                "contracts/spec/workflow-governance-golden-path-v0.yaml",
                "acceptance_surfaces",
                "Repository-owned P5c specification defining the boundaries exercised by the adversarial suites.",
            ),
        ],
    )
}

fn shadow_evidence(root: &Path) -> Result<PublicationEvidenceDocument<'_>, Box<dyn Error>> {
    publication_evidence(
        root,
        "workflow-release-evidence.shadow-report.v0",
        PublicationEvidenceClass::ShadowReport,
        "Indexes real P5c recovery/resume results and the P5a exact-projection audit consumed by the P5d.1 release evaluator.",
        &[
            (
                "crates/forge-core-cli/tests/workflow_governance_integration_e2e.rs",
                "fresh_agent_resumes_same_automatically_selected_governance_state",
                "Real binary result proving fresh-agent recovery observes the same governed state.",
            ),
            (
                "docs/fixtures/workflow-governance-golden-path-v0/ledger-all-events.yaml",
                "workflow_governance_ledger",
                "Durable P5c event fixture spanning the admitted governance event families.",
            ),
            (
                "crates/forge-core-cli/tests/workflow_migration_cli_e2e.rs",
                "agent_receives_complete_p5a_manifest_from_one_read_only_command",
                "The exact 110-workflow shadow-parity result that P5d.1 reuses as its migration baseline.",
            ),
        ],
    )
}

fn load_foundation_inputs(root: &Path) -> Result<FoundationInputs, Box<dyn Error>> {
    let (workflows, catalog) = load_clean_catalog(root)?;
    let plan: WorkflowMigrationPlanDocument =
        read_yaml(&root.join("contracts/policies/workflow-migration-foundation-v0.yaml"))?;
    let audit = evaluate_workflow_migration(&plan, &workflows, &catalog);
    validate_audit(&audit)?;
    let policies = load_golden_policies(root, &plan)?;
    Ok(FoundationInputs {
        workflows,
        audit,
        policies,
    })
}

fn load_clean_catalog(
    root: &Path,
) -> Result<(Vec<LoadedWorkflowDocument>, forge_core_contracts::Catalog), Box<dyn Error>> {
    let catalog_dir = root.join("contracts/workflows");
    let workflows = load_workflow_documents(&catalog_dir);
    if !workflows.is_clean() || workflows.workflows.len() != 110 {
        return Err(error(format!(
            "expected a clean 110-workflow inventory, found {} workflow(s) and {} error(s)",
            workflows.workflows.len(),
            workflows.errors.len()
        )));
    }
    let catalog = load_catalog(&catalog_dir);
    if !catalog.is_clean() || catalog.catalog.entries.len() != 110 {
        return Err(error(format!(
            "expected a clean 110-entry catalog, found {} entries and {} error(s)",
            catalog.catalog.entries.len(),
            catalog.errors.len()
        )));
    }
    Ok((workflows.workflows, catalog.catalog))
}

fn validate_audit(audit: &WorkflowMigrationAudit) -> Result<(), Box<dyn Error>> {
    if !audit.issues.is_empty()
        || audit.catalog_count != 110
        || audit.classified_count != 110
        || audit.unresolved_count != 0
        || audit.shadow_parity.drift_count != 0
    {
        return Err(error(format!(
            "P5a foundation is not publication-ready: catalog={}, classified={}, unresolved={}, drift={}, issues={}",
            audit.catalog_count,
            audit.classified_count,
            audit.unresolved_count,
            audit.shadow_parity.drift_count,
            audit.issues.len()
        )));
    }
    Ok(())
}

fn load_golden_policies(
    root: &Path,
    plan: &WorkflowMigrationPlanDocument,
) -> Result<Vec<WorkflowGovernancePolicy>, Box<dyn Error>> {
    let golden_bundle: WorkflowGovernanceBundleDocument =
        read_yaml(&root.join("contracts/workflow-governance/golden-path-v0.yaml"))?;
    let policies = golden_bundle.workflow_governance_bundle.policies;
    if policies.len() != 15 {
        return Err(error(format!(
            "expected fifteen golden-path policies, found {}",
            policies.len()
        )));
    }
    let golden_plan_ids = plan
        .workflow_migration_plan
        .golden_path_selections
        .iter()
        .map(|selection| selection.workflow_id.0.as_str())
        .collect::<BTreeSet<_>>();
    let golden_policy_ids = policies
        .iter()
        .map(|policy| policy.compatibility_workflow_id.0.as_str())
        .collect::<BTreeSet<_>>();
    if golden_plan_ids != golden_policy_ids {
        return Err(error(
            "P5a golden selections and P5c golden bundle compatibility ids differ",
        ));
    }
    Ok(policies)
}

fn build_batch(
    foundation: &FoundationInputs,
    evidence: &EvidenceArtifacts,
    batch_id: &StableId,
) -> Result<WorkflowMigrationBatchDocument, Box<dyn Error>> {
    let mut bindings = build_bindings(&foundation.workflows, &foundation.policies)?;
    bindings.sort_by(|left, right| left.workflow_id.0.cmp(&right.workflow_id.0));
    let mut policies = foundation.policies.clone();
    policies.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    Ok(WorkflowMigrationBatchDocument {
        schema_version: WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION.to_owned(),
        workflow_migration_batch: WorkflowMigrationBatch {
            id: batch_id.clone(),
            batch_version: "0.1.0".to_owned(),
            authority: WorkflowMigrationBatchAuthority::CandidateOnly,
            source_catalog_digest: foundation.audit.manifest.catalog_digest.clone(),
            previous_batch_digest: None,
            evidence: WorkflowMigrationBatchEvidence {
                representative_fixtures: vec![content_reference(
                    REPRESENTATIVE_EVIDENCE_PATH,
                    &evidence.representative,
                )],
                adversarial_fixtures: vec![content_reference(
                    ADVERSARIAL_EVIDENCE_PATH,
                    &evidence.adversarial,
                )],
                shadow_reports: vec![content_reference(SHADOW_EVIDENCE_PATH, &evidence.shadow)],
            },
            workflow_bindings: bindings,
            policies,
        },
    })
}

fn build_bindings(
    workflows: &[LoadedWorkflowDocument],
    policies: &[WorkflowGovernancePolicy],
) -> Result<Vec<WorkflowMigrationBatchBinding>, Box<dyn Error>> {
    let workflow_by_id = workflows
        .iter()
        .map(|workflow| (workflow.document.workflow.id.0.as_str(), workflow))
        .collect::<BTreeMap<_, _>>();
    policies
        .iter()
        .map(|policy| {
            let workflow = workflow_by_id
                .get(policy.compatibility_workflow_id.0.as_str())
                .ok_or_else(|| {
                    error(format!(
                        "golden policy {} references missing workflow {}",
                        policy.id.0, policy.compatibility_workflow_id.0
                    ))
                })?;
            Ok(WorkflowMigrationBatchBinding {
                workflow_id: policy.compatibility_workflow_id.clone(),
                legacy_workflow_digest: workflow_release_legacy_digest(workflow).map_err(error)?,
                policy_ref: policy.id.clone(),
            })
        })
        .collect()
}

fn build_manifest_entries(
    foundation: &FoundationInputs,
    batch_id: &StableId,
) -> Result<Vec<WorkflowReleaseWorkflowEntry>, Box<dyn Error>> {
    let p5a_disposition = foundation
        .audit
        .manifest
        .entries
        .iter()
        .map(|entry| (entry.workflow_id.as_str(), entry.disposition))
        .collect::<BTreeMap<_, _>>();
    let policy_by_workflow = foundation
        .policies
        .iter()
        .map(|policy| {
            (
                policy.compatibility_workflow_id.0.clone(),
                policy.id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut counts = DispositionCounts::default();
    let mut entries = foundation
        .workflows
        .iter()
        .map(|workflow| {
            let workflow_id = &workflow.document.workflow.id;
            let disposition_intent = if let Some(policy_ref) = policy_by_workflow.get(&workflow_id.0)
            {
                counts.migration += 1;
                WorkflowReleaseDispositionIntent::MigrationCandidate {
                    batch_id: batch_id.clone(),
                    policy_ref: policy_ref.clone(),
                }
            } else if p5a_disposition.get(workflow_id.0.as_str())
                == Some(&WorkflowMigrationDisposition::DomainPackCandidate)
            {
                counts.domain += 1;
                WorkflowReleaseDispositionIntent::DomainPackCandidate {
                    candidate: WorkflowDomainPackCandidate {
                        domain_id: id("domain.game-development"),
                        proposed_pack_id: id("pack.game-development"),
                        deferral_reason: WorkflowDomainPackDeferralReason::DomainSpecificLifecycle,
                        explanation: "Deferred to the governed P6 game-development domain pack."
                            .to_owned(),
                    },
                }
            } else {
                counts.compatibility += 1;
                WorkflowReleaseDispositionIntent::CompatibilityOnly {
                    reason: WorkflowCompatibilityReason {
                        code: WorkflowCompatibilityReasonCode::AwaitingMigration,
                        explanation: "Retained as an exact, read-only, non-authoritative legacy compatibility projection."
                            .to_owned(),
                    },
                }
            };
            Ok(WorkflowReleaseWorkflowEntry {
                workflow_id: workflow_id.clone(),
                legacy_workflow_digest: workflow_release_legacy_digest(workflow)
                    .map_err(error)?,
                disposition_intent,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    entries.sort_by(|left, right| left.workflow_id.0.cmp(&right.workflow_id.0));
    if (counts.migration, counts.domain, counts.compatibility) != (15, 18, 77) {
        return Err(error(format!(
            "unexpected disposition counts: migration={}, domain={}, compatibility={}",
            counts.migration, counts.domain, counts.compatibility
        )));
    }
    Ok(entries)
}

fn build_manifest(
    workflow_entries: Vec<WorkflowReleaseWorkflowEntry>,
    catalog_digest: String,
    batch_id: StableId,
    batch_digest: String,
) -> WorkflowGovernanceReleaseManifestDocument {
    WorkflowGovernanceReleaseManifestDocument {
        schema_version: WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION.to_owned(),
        workflow_governance_release_manifest: WorkflowGovernanceReleaseManifest {
            lineage_id: id("workflow-governance.core"),
            release_id: id("workflow-governance.release.foundation-v0"),
            release_version: "0.1.0".to_owned(),
            previous_release_digest: None,
            legacy_catalog_digest: catalog_digest,
            batches: vec![WorkflowReleaseBatchReference {
                batch_id,
                batch_version: "0.1.0".to_owned(),
                embedded_ref: RepoPath(BATCH_PATH.to_owned()),
                expected_digest: batch_digest,
                deterministic_order: 0,
            }],
            workflow_entries,
            compatibility_policy: compatibility_policy(),
        },
    }
}

fn compatibility_policy() -> WorkflowReleaseCompatibilityPolicy {
    WorkflowReleaseCompatibilityPolicy {
        policy_version: "0.1.0".to_owned(),
        lifecycle: WorkflowCompatibilityLifecycle::Supported,
        diagnostic_code: id("workflow.compatibility.foundation-retained"),
        replacement_argv: [
            "forge-core",
            "guide",
            "rollout-audit",
            "--manifest-file",
            MANIFEST_PATH,
            "--batch-file",
            BATCH_PATH,
            "--json",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        projection_mode: WorkflowReleaseCompatibilityProjectionMode::ReadOnlyExactProjection,
        legacy_authority: WorkflowLegacyCompatibilityAuthority::NonAuthoritative,
        exact_fields: vec![
            WorkflowCompatibilityField::Id,
            WorkflowCompatibilityField::Phases,
            WorkflowCompatibilityField::WorkflowRef,
            WorkflowCompatibilityField::Triggers,
            WorkflowCompatibilityField::Prerequisites,
            WorkflowCompatibilityField::Outputs,
        ],
        consumer_diagnostics: WorkflowConsumerDiagnosticsPolicy::Required,
        minimum_consumer_version: "0.4.0".to_owned(),
        retirement_admission: WorkflowRetirementAdmissionPolicy::VerifiedAuthorizationRequired,
    }
}

fn assemble_artifacts(
    evidence: EvidenceArtifacts,
    batch: Vec<u8>,
    manifest: Vec<u8>,
) -> Vec<GeneratedArtifact> {
    vec![
        GeneratedArtifact {
            relative_path: REPRESENTATIVE_EVIDENCE_PATH,
            bytes: evidence.representative,
        },
        GeneratedArtifact {
            relative_path: ADVERSARIAL_EVIDENCE_PATH,
            bytes: evidence.adversarial,
        },
        GeneratedArtifact {
            relative_path: SHADOW_EVIDENCE_PATH,
            bytes: evidence.shadow,
        },
        GeneratedArtifact {
            relative_path: BATCH_PATH,
            bytes: batch,
        },
        GeneratedArtifact {
            relative_path: MANIFEST_PATH,
            bytes: manifest,
        },
    ]
}

fn publication_evidence<'a>(
    root: &Path,
    id: &'static str,
    evidence_class: PublicationEvidenceClass,
    integrity_claim: &'static str,
    references: &'a [(&'a str, &'a str, &'a str)],
) -> Result<PublicationEvidenceDocument<'a>, Box<dyn Error>> {
    for (repo_path, _, _) in references {
        if !root.join(repo_path).is_file() {
            return Err(error(format!(
                "publication evidence source does not exist: {repo_path}"
            )));
        }
    }
    Ok(PublicationEvidenceDocument {
        schema_version: "0.1",
        workflow_release_publication_evidence: PublicationEvidence {
            id,
            evidence_class,
            publication_scope: "p5d_1_release_publication",
            integrity_claim,
            behavioral_sufficiency: BehavioralSufficiency::NotAsserted,
            limitation: "This artifact is a publication evidence reference. P5d.1 validates its embedded path and exact content digest only; it does not prove that the referenced behavior is sufficient, current, or passing.",
            references: references
                .iter()
                .map(|(repo_path, anchor, relevance)| PublicationEvidenceReference {
                    repo_path,
                    anchor,
                    relevance,
                })
                .collect(),
        },
    })
}

fn content_reference(path: &str, bytes: &[u8]) -> WorkflowMigrationEvidenceReference {
    WorkflowMigrationEvidenceReference {
        embedded_ref: RepoPath(path.to_owned()),
        expected_digest: sha256(bytes),
    }
}

fn read_yaml<T>(path: &Path) -> Result<T, Box<dyn Error>>
where
    T: serde::de::DeserializeOwned,
{
    let text = std::fs::read_to_string(path)?;
    yaml_serde::from_str(&text).map_err(|cause| {
        error(format!(
            "cannot parse typed YAML {}: {cause}",
            path.display()
        ))
    })
}

fn yaml_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut text = yaml_serde::to_string(value)?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text.into_bytes())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn write_artifacts(root: &Path, artifacts: &[GeneratedArtifact]) -> Result<(), Box<dyn Error>> {
    for artifact in artifacts {
        let path = root.join(artifact.relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &artifact.bytes)?;
        println!("wrote {}", artifact.relative_path);
    }
    Ok(())
}

fn check_artifacts(root: &Path, artifacts: &[GeneratedArtifact]) -> Result<(), Box<dyn Error>> {
    let drift = artifacts
        .iter()
        .filter_map(|artifact| {
            let path = root.join(artifact.relative_path);
            match std::fs::read(&path) {
                Ok(found) if found == artifact.bytes => None,
                Ok(_) => Some(format!("{} has byte drift", artifact.relative_path)),
                Err(cause) => Some(format!(
                    "{} is unavailable: {cause}",
                    artifact.relative_path
                )),
            }
        })
        .collect::<Vec<_>>();
    if drift.is_empty() {
        println!("workflow release foundation artifacts are byte-exact");
        Ok(())
    } else {
        Err(error(format!(
            "workflow release foundation drift:\n{}\nrun the generator with --write",
            drift.join("\n")
        )))
    }
}

fn error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message.into()))
}
