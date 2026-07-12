use forge_core_contracts::{
    PrincipalId, RepoPath, StableId, WorkflowCompatibilityField, WorkflowCompatibilityLifecycle,
    WorkflowCompatibilityReason, WorkflowCompatibilityReasonCode,
    WorkflowConsumerDiagnosticsPolicy, WorkflowDomainPackCandidate,
    WorkflowDomainPackDeferralReason, WorkflowGovernanceReleaseManifest,
    WorkflowGovernanceReleaseManifestDocument, WorkflowLegacyCompatibilityAuthority,
    WorkflowMigrationBatch, WorkflowMigrationBatchAuthority, WorkflowMigrationBatchBinding,
    WorkflowMigrationBatchDocument, WorkflowMigrationBatchEvidence,
    WorkflowMigrationEvidenceReference, WorkflowQuarantine, WorkflowQuarantineReasonCode,
    WorkflowQuarantineRiskTier, WorkflowReleaseBatchReference, WorkflowReleaseCompatibilityPolicy,
    WorkflowReleaseCompatibilityProjectionMode, WorkflowReleaseDispositionIntent,
    WorkflowReleaseWorkflowEntry, WorkflowRetirementAdmissionPolicy,
    WorkflowRetirementAuthorization, WorkflowRetirementAuthorizationDocument,
    WorkflowRetirementAuthorizationReference, WorkflowRetirementCompatibilityWindow,
    WorkflowRetirementEvidenceBinding, WorkflowRetirementReviewer,
    WorkflowRetirementSignatureAlgorithm, WorkflowRetirementSignatureEnvelope,
    WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION, WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION,
    WORKFLOW_RETIREMENT_AUTHORIZATION_SCHEMA_VERSION,
};
use schemars::schema_for;

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn retirement_reference() -> WorkflowRetirementAuthorizationReference {
    WorkflowRetirementAuthorizationReference {
        authorization_id: id("retirement.workflow-old.v1"),
        embedded_ref: RepoPath(
            "contracts/workflow-retirement/retirement.workflow-old.v1.yaml".to_owned(),
        ),
        expected_digest: digest('9'),
    }
}

fn compatibility_policy() -> WorkflowReleaseCompatibilityPolicy {
    WorkflowReleaseCompatibilityPolicy {
        policy_version: "0.1.0".to_owned(),
        lifecycle: WorkflowCompatibilityLifecycle::Deprecated {
            announced_at_unix: 1_700_000_000,
            removal_not_before_unix: 1_710_000_000,
        },
        diagnostic_code: id("workflow.compatibility.deprecated"),
        replacement_argv: vec![
            "forge-core".to_owned(),
            "workflow".to_owned(),
            "next".to_owned(),
        ],
        projection_mode: WorkflowReleaseCompatibilityProjectionMode::ReadOnlyExactProjection,
        legacy_authority: WorkflowLegacyCompatibilityAuthority::NonAuthoritative,
        exact_fields: vec![
            WorkflowCompatibilityField::Id,
            WorkflowCompatibilityField::WorkflowRef,
        ],
        consumer_diagnostics: WorkflowConsumerDiagnosticsPolicy::Required,
        minimum_consumer_version: "0.4.0".to_owned(),
        retirement_admission: WorkflowRetirementAdmissionPolicy::VerifiedAuthorizationRequired,
    }
}

fn release_manifest() -> WorkflowGovernanceReleaseManifestDocument {
    WorkflowGovernanceReleaseManifestDocument {
        schema_version: WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION.to_owned(),
        workflow_governance_release_manifest: WorkflowGovernanceReleaseManifest {
            lineage_id: id("workflow-governance.core"),
            release_id: id("workflow-governance.release.0-2-0"),
            release_version: "0.2.0".to_owned(),
            previous_release_digest: Some(digest('0')),
            legacy_catalog_digest: digest('1'),
            batches: vec![WorkflowReleaseBatchReference {
                batch_id: id("workflow-batch.core-01"),
                batch_version: "0.1.0".to_owned(),
                embedded_ref: RepoPath(
                    "contracts/workflow-governance/batches/core-01-v0.yaml".to_owned(),
                ),
                expected_digest: digest('2'),
                deterministic_order: 10,
            }],
            workflow_entries: vec![
                WorkflowReleaseWorkflowEntry {
                    workflow_id: id("discover-intent"),
                    legacy_workflow_digest: digest('3'),
                    disposition_intent: WorkflowReleaseDispositionIntent::MigrationCandidate {
                        batch_id: id("workflow-batch.core-01"),
                        policy_ref: id("policy.workflow.discover-intent"),
                    },
                },
                WorkflowReleaseWorkflowEntry {
                    workflow_id: id("legacy-advisory"),
                    legacy_workflow_digest: digest('4'),
                    disposition_intent: WorkflowReleaseDispositionIntent::CompatibilityOnly {
                        reason: WorkflowCompatibilityReason {
                            code: WorkflowCompatibilityReasonCode::AwaitingMigration,
                            explanation: "Retain read-only guidance until a reviewed batch exists."
                                .to_owned(),
                        },
                    },
                },
                WorkflowReleaseWorkflowEntry {
                    workflow_id: id("ambiguous-flow"),
                    legacy_workflow_digest: digest('5'),
                    disposition_intent: WorkflowReleaseDispositionIntent::Quarantined {
                        quarantine: WorkflowQuarantine {
                            reason_code: WorkflowQuarantineReasonCode::AmbiguousLegacyAuthority,
                            risk_tier: WorkflowQuarantineRiskTier::Critical,
                            explanation: "Legacy completion semantics are ambiguous.".to_owned(),
                            blocking_refs: vec![id("legacy.done_when")],
                            affected_consumer_refs: vec![id("consumer.forge-core-cli")],
                            review_owner: id("team.workflow-governance"),
                            review_due_release_version: "0.3.0".to_owned(),
                        },
                    },
                },
                WorkflowReleaseWorkflowEntry {
                    workflow_id: id("game-project"),
                    legacy_workflow_digest: digest('6'),
                    disposition_intent: WorkflowReleaseDispositionIntent::DomainPackCandidate {
                        candidate: WorkflowDomainPackCandidate {
                            domain_id: id("domain.game-development"),
                            proposed_pack_id: id("pack.game-development"),
                            deferral_reason:
                                WorkflowDomainPackDeferralReason::DomainSpecificLifecycle,
                            explanation: "Remain compatibility-only until P6 pack composition."
                                .to_owned(),
                        },
                    },
                },
                WorkflowReleaseWorkflowEntry {
                    workflow_id: id("workflow-old"),
                    legacy_workflow_digest: digest('7'),
                    disposition_intent: WorkflowReleaseDispositionIntent::RetirementCandidate {
                        replacement_policy_ref: id("policy.workflow.replacement"),
                        authorization: retirement_reference(),
                    },
                },
            ],
            compatibility_policy: compatibility_policy(),
        },
    }
}

fn migration_batch() -> WorkflowMigrationBatchDocument {
    WorkflowMigrationBatchDocument {
        schema_version: WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION.to_owned(),
        workflow_migration_batch: WorkflowMigrationBatch {
            id: id("workflow-batch.core-01"),
            batch_version: "0.1.0".to_owned(),
            authority: WorkflowMigrationBatchAuthority::CandidateOnly,
            source_catalog_digest: digest('1'),
            previous_batch_digest: None,
            evidence: WorkflowMigrationBatchEvidence {
                representative_fixtures: vec![WorkflowMigrationEvidenceReference {
                    embedded_ref: RepoPath(
                        "contracts/workflow-governance/evidence/core-01/representative.json"
                            .to_owned(),
                    ),
                    expected_digest: digest('5'),
                }],
                adversarial_fixtures: vec![WorkflowMigrationEvidenceReference {
                    embedded_ref: RepoPath(
                        "contracts/workflow-governance/evidence/core-01/adversarial.json"
                            .to_owned(),
                    ),
                    expected_digest: digest('6'),
                }],
                shadow_reports: vec![WorkflowMigrationEvidenceReference {
                    embedded_ref: RepoPath(
                        "contracts/workflow-governance/evidence/core-01/shadow-report.json"
                            .to_owned(),
                    ),
                    expected_digest: digest('7'),
                }],
            },
            workflow_bindings: vec![WorkflowMigrationBatchBinding {
                workflow_id: id("discover-intent"),
                legacy_workflow_digest: digest('3'),
                policy_ref: id("policy.workflow.discover-intent"),
            }],
            policies: Vec::new(),
        },
    }
}

fn retirement_authorization() -> WorkflowRetirementAuthorizationDocument {
    WorkflowRetirementAuthorizationDocument {
        schema_version: WORKFLOW_RETIREMENT_AUTHORIZATION_SCHEMA_VERSION.to_owned(),
        workflow_retirement_authorization: WorkflowRetirementAuthorization {
            id: id("retirement.workflow-old.v1"),
            workflow_id: id("workflow-old"),
            legacy_workflow_digest: digest('7'),
            replacement_policy_ref: id("policy.workflow.replacement"),
            replacement_policy_digest: digest('8'),
            governance_release_digest: digest('a'),
            evidence: WorkflowRetirementEvidenceBinding {
                executable_coverage_digest: digest('b'),
                shadow_evidence_digest: digest('c'),
                deletion_test_digest: digest('d'),
                consumer_compatibility_digest: digest('e'),
            },
            compatibility_window: WorkflowRetirementCompatibilityWindow {
                announced_at_unix: 1_700_000_000,
                retirement_not_before_unix: 1_710_000_000,
                minimum_consumer_version: "0.4.0".to_owned(),
                diagnostics_evidence_digest: digest('f'),
            },
            reviewer: WorkflowRetirementReviewer {
                principal_id: PrincipalId("principal.release-reviewer".to_owned()),
                credential_id: id("credential.release-reviewer"),
                authority_scope: id("workflow.retirement.authorize"),
                registry_digest: digest('1'),
                public_key_fingerprint: digest('2'),
            },
            signature: WorkflowRetirementSignatureEnvelope {
                algorithm: WorkflowRetirementSignatureAlgorithm::Ed25519,
                audience: "forge-core:workflow-release".to_owned(),
                nonce: "retirement-workflow-old-0001".to_owned(),
                intent_digest: digest('3'),
                attestation_digest: digest('4'),
                signature: "ab".repeat(64),
                signed_at_unix: 1_710_000_001,
                expires_at_unix: 1_720_000_000,
            },
        },
    }
}

#[test]
fn release_manifest_round_trips_with_explicit_non_authoritative_intents() {
    let document = release_manifest();
    let json = serde_json::to_string_pretty(&document).expect("serialize release manifest");
    let decoded: WorkflowGovernanceReleaseManifestDocument =
        serde_json::from_str(&json).expect("deserialize release manifest");
    assert_eq!(decoded, document);
    let yaml = yaml_serde::to_string(&document).expect("serialize release manifest YAML");
    let decoded_yaml: WorkflowGovernanceReleaseManifestDocument =
        yaml_serde::from_str(&yaml).expect("deserialize release manifest YAML");
    assert_eq!(decoded_yaml, document);
    assert_eq!(
        decoded
            .workflow_governance_release_manifest
            .workflow_entries
            .len(),
        5
    );

    let mut unknown = serde_json::to_value(&document).expect("manifest JSON");
    unknown["workflow_governance_release_manifest"]["caller_executable"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowGovernanceReleaseManifestDocument>(unknown).is_err());
}

#[test]
fn authored_disposition_cannot_claim_executable_or_retired() {
    for forbidden in ["executable", "retired"] {
        let value = serde_json::json!({
            "workflow_id": "workflow.attack",
            "legacy_workflow_digest": digest('a'),
            "disposition_intent": {"kind": forbidden}
        });
        assert!(serde_json::from_value::<WorkflowReleaseWorkflowEntry>(value).is_err());
    }
}

#[test]
fn quarantine_requires_closed_risk_and_review_impact_shape() {
    let manifest = serde_json::to_value(release_manifest()).expect("manifest JSON");
    let quarantine = &manifest["workflow_governance_release_manifest"]["workflow_entries"][2]
        ["disposition_intent"]["quarantine"];

    let mut invented_risk = quarantine.clone();
    invented_risk["risk_tier"] = serde_json::json!("unbounded");
    assert!(serde_json::from_value::<WorkflowQuarantine>(invented_risk).is_err());

    for required in [
        "blocking_refs",
        "risk_tier",
        "affected_consumer_refs",
        "review_due_release_version",
    ] {
        let mut missing = quarantine.clone();
        missing
            .as_object_mut()
            .expect("quarantine object")
            .remove(required);
        assert!(serde_json::from_value::<WorkflowQuarantine>(missing).is_err());
    }

    let mut extended = quarantine.clone();
    extended["caller_safe"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowQuarantine>(extended).is_err());
}

#[test]
fn compatibility_lifecycle_is_closed_and_retired_remains_authorization_bound() {
    let policy = WorkflowReleaseCompatibilityPolicy {
        lifecycle: WorkflowCompatibilityLifecycle::Retired {
            authorization_ref: retirement_reference(),
        },
        ..release_manifest()
            .workflow_governance_release_manifest
            .compatibility_policy
    };
    let value = serde_json::to_value(&policy).expect("compatibility policy JSON");
    let decoded: WorkflowReleaseCompatibilityPolicy =
        serde_json::from_value(value.clone()).expect("retired lifecycle round trip");
    assert_eq!(decoded, policy);

    let mut invented = value.clone();
    invented["lifecycle"] = serde_json::json!({"status": "silently_removed"});
    assert!(serde_json::from_value::<WorkflowReleaseCompatibilityPolicy>(invented).is_err());

    let mut extended = value;
    extended["lifecycle"]["trust_me"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowReleaseCompatibilityPolicy>(extended).is_err());

    let serialized = serde_json::to_value(&policy).expect("compatibility policy JSON");
    for required in ["policy_version", "diagnostic_code", "replacement_argv"] {
        let mut missing = serialized.clone();
        missing
            .as_object_mut()
            .expect("compatibility policy object")
            .remove(required);
        assert!(serde_json::from_value::<WorkflowReleaseCompatibilityPolicy>(missing).is_err());
    }
}

#[test]
fn migration_batch_is_closed_and_candidate_only() {
    let document = migration_batch();
    let json = serde_json::to_value(&document).expect("batch JSON");
    let decoded: WorkflowMigrationBatchDocument =
        serde_json::from_value(json.clone()).expect("batch round trip");
    assert_eq!(decoded, document);
    assert_eq!(
        decoded.workflow_migration_batch.authority,
        WorkflowMigrationBatchAuthority::CandidateOnly
    );

    let mut escalated = json.clone();
    escalated["workflow_migration_batch"]["authority"] = serde_json::json!("executable");
    assert!(serde_json::from_value::<WorkflowMigrationBatchDocument>(escalated).is_err());

    let mut extended_evidence = json.clone();
    extended_evidence["workflow_migration_batch"]["evidence"]["trusted"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowMigrationBatchDocument>(extended_evidence).is_err());

    let mut missing_evidence_class = json;
    missing_evidence_class["workflow_migration_batch"]["evidence"]
        .as_object_mut()
        .expect("evidence object")
        .remove("adversarial_fixtures");
    assert!(
        serde_json::from_value::<WorkflowMigrationBatchDocument>(missing_evidence_class).is_err()
    );
}

#[test]
fn retirement_authorization_round_trips_and_rejects_signature_extension() {
    let document = retirement_authorization();
    let json = serde_json::to_value(&document).expect("retirement JSON");
    let decoded: WorkflowRetirementAuthorizationDocument =
        serde_json::from_value(json.clone()).expect("retirement round trip");
    assert_eq!(decoded, document);

    let mut extended = json;
    extended["workflow_retirement_authorization"]["signature"]["trust_me"] =
        serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowRetirementAuthorizationDocument>(extended).is_err());
}

#[test]
fn generated_schemas_are_closed_at_each_document_root() {
    let release_schema = schema_for!(WorkflowGovernanceReleaseManifestDocument);
    let release_schema_value = serde_json::to_value(&release_schema).expect("release schema JSON");
    let quarantine_required = release_schema_value["$defs"]["WorkflowQuarantine"]["required"]
        .as_array()
        .expect("WorkflowQuarantine required fields");
    assert!(quarantine_required.contains(&serde_json::json!("blocking_refs")));

    for schema in [
        release_schema,
        schema_for!(WorkflowMigrationBatchDocument),
        schema_for!(WorkflowRetirementAuthorizationDocument),
    ] {
        let value = serde_json::to_value(schema).expect("schema JSON");
        assert_eq!(value["additionalProperties"], serde_json::json!(false));
    }
}
