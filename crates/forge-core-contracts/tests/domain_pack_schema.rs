use forge_core_contracts::{
    DomainPackCandidateAuthority, DomainPackCompositionProjectionDocument,
    DomainPackCompositionRequestDocument, DomainPackCompositionStatus, DomainPackContentDocument,
    DomainPackManifestDocument, DomainPackSourceKind, DOMAIN_PACK_SCHEMA_VERSION,
};
use schemars::schema_for;

const MANIFEST_YAML: &str = r#"
schema_version: "0.1"
domain_pack_manifest:
  identity:
    publisher: "forge.test"
    name: "neutral-domain"
    namespace: "neutral"
    version: "1.2.3"
  authority: "candidate_only"
  compatibility:
    pack_schema_requirement: ">=0.1,<0.2"
    forge_core_requirement: ">=0.5,<0.6"
  provenance:
    source_kind: "repository"
    source_uri: "https://example.invalid/neutral-domain"
    source_revision: "refs/tags/v1.2.3"
    source_digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    authors: ["principal.test.author"]
    license_spdx_expression: "Apache-2.0"
    license_text:
      artifact_ref: "LICENSE"
      raw_sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
      canonical_sha256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  content:
    content_ref: "contracts/domain-packs/neutral/content.yaml"
    raw_sha256: "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    canonical_sha256: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
  dependencies:
    - pack:
        publisher: "forge.test"
        name: "base-domain"
      version_requirement: "^1.0"
      required_content_digest: null
  conflicts: []
  replacement_slots:
    - id: "neutral.slot.evaluator"
      contribution_kind: "evaluator"
      target_ref: "neutral.evaluator.baseline"
      target_digest: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
      allowed_replacers:
        - publisher: "forge.test"
          name: "neutral-successor"
      replacement_version_requirement: ">=2.0,<3.0"
  replacement_declarations: []
"#;

const CONTENT_YAML: &str = r#"
schema_version: "0.1"
domain_pack_content:
  pack:
    publisher: "forge.test"
    name: "neutral-domain"
    version: "1.2.3"
  namespace: "neutral"
  workflow_overlay:
    id: "neutral.overlay"
    base_bundle_id: "bundle.workflow-governance.core"
    policies: []
  provided_domains:
    - id: "neutral.domain"
      description: "A neutral schema fixture."
      policy_refs: []
      hazard_refs: ["neutral.hazard.loss"]
      lifecycle_model_refs: ["neutral.lifecycle.delivery"]
  provided_capabilities:
    - id: "neutral.capability.review"
      kind: "human_review"
      description: "A declared review requirement, not runtime availability."
      evidence_rule_refs: []
      authority: "declaration_only"
  hazards:
    - id: "neutral.hazard.loss"
      category: "quality"
      severity: "high"
      description: "Representative loss may go unnoticed."
      trigger_refs: []
      mitigation_obligation_refs: []
      evidence_claim_refs: []
  lifecycle_models:
    - id: "neutral.lifecycle.delivery"
      description: "Minimal two-state domain lifecycle."
      initial_state_ref: "neutral.state.proposed"
      terminal_state_refs: ["neutral.state.accepted"]
      states:
        - id: "neutral.state.proposed"
          description: "Proposed"
          entry_obligation_refs: []
          exit_claim_refs: []
        - id: "neutral.state.accepted"
          description: "Accepted"
          entry_obligation_refs: []
          exit_claim_refs: []
      transitions:
        - id: "neutral.transition.accept"
          from_state_ref: "neutral.state.proposed"
          to_state_ref: "neutral.state.accepted"
          guard_claim_refs: []
          required_capability_refs: ["neutral.capability.review"]
  evaluators:
    - id: "neutral.evaluator.baseline"
      implementation:
        kind: "built_in"
        provider: "repository_inspector"
      accepted_evidence_kinds: ["artifact_inspection"]
      minimum_strength: "inspected_artifact"
      authority: "declaration_only"
  fixtures:
    - id: "neutral.fixture.representative"
      kind: "representative"
      artifact:
        artifact_ref: "docs/fixtures/domain-pack-v0/representative.yaml"
        raw_sha256: "1111111111111111111111111111111111111111111111111111111111111111"
        canonical_sha256: "2222222222222222222222222222222222222222222222222222222222222222"
      subject_refs: ["neutral.domain"]
  adapters:
    - id: "neutral.adapter.review"
      protocol: "runtime_handshake"
      surface: "surface.neutral.review"
      required_capability_refs: ["neutral.capability.review"]
      authority: "declaration_only"
"#;

#[test]
fn manifest_and_content_are_closed_candidate_only_documents() {
    let manifest: DomainPackManifestDocument =
        yaml_serde::from_str(MANIFEST_YAML).expect("closed manifest");
    let content: DomainPackContentDocument =
        yaml_serde::from_str(CONTENT_YAML).expect("closed content");

    assert_eq!(manifest.schema_version, DOMAIN_PACK_SCHEMA_VERSION);
    assert_eq!(content.schema_version, DOMAIN_PACK_SCHEMA_VERSION);
    assert_eq!(
        manifest.domain_pack_manifest.authority,
        DomainPackCandidateAuthority::CandidateOnly
    );
    assert_eq!(
        manifest.domain_pack_manifest.provenance.source_kind,
        DomainPackSourceKind::Repository
    );
    assert_eq!(content.domain_pack_content.hazards.len(), 1);
    assert_eq!(content.domain_pack_content.lifecycle_models.len(), 1);
    assert_eq!(content.domain_pack_content.adapters.len(), 1);
}

#[test]
fn unknown_fields_and_authority_shortcuts_are_rejected() {
    let manifest: DomainPackManifestDocument =
        yaml_serde::from_str(MANIFEST_YAML).expect("manifest fixture");
    let mut value = serde_json::to_value(&manifest).expect("manifest json");
    value["domain_pack_manifest"]["installed"] = serde_json::json!(true);
    assert!(serde_json::from_value::<DomainPackManifestDocument>(value).is_err());

    let content: DomainPackContentDocument =
        yaml_serde::from_str(CONTENT_YAML).expect("content fixture");
    let mut value = serde_json::to_value(&content).expect("content json");
    value["domain_pack_content"]["adapters"][0]["may_mutate"] = serde_json::json!(true);
    assert!(serde_json::from_value::<DomainPackContentDocument>(value).is_err());

    let bad_authority = MANIFEST_YAML.replace("candidate_only", "active");
    assert!(yaml_serde::from_str::<DomainPackManifestDocument>(&bad_authority).is_err());

    let bad_evaluator = CONTENT_YAML.replace("kind: \"built_in\"", "kind: \"agent_script\"");
    assert!(yaml_serde::from_str::<DomainPackContentDocument>(&bad_evaluator).is_err());
}

#[test]
fn request_and_projection_round_trip_without_gaining_authority() {
    let manifest: DomainPackManifestDocument = yaml_serde::from_str(MANIFEST_YAML).unwrap();
    let content: DomainPackContentDocument = yaml_serde::from_str(CONTENT_YAML).unwrap();
    let request = serde_json::json!({
        "schema_version": "0.1",
        "domain_pack_composition_request": {
            "request_id": "composition.test",
            "authority": "candidate_only",
            "forge_core_version": "0.5.0",
            "core": {
                "bundle_id": "bundle.workflow-governance.core",
                "bundle_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "policy_set_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "bundle": { "id": "bundle.workflow-governance.core", "policies": [] }
            },
            "requirements": {
                "project_id": "project.test",
                "requirement_set_id": "requirements.test",
                "required_domains": [{
                    "id": "requirement.neutral",
                    "domain_id": "neutral.domain",
                    "pack_version_requirement": "^1.0",
                    "required_capability_refs": ["neutral.capability.review"]
                }]
            },
            "candidates": [{
                "manifest_binding": {
                    "artifact_ref": "packs/test/manifest.yaml",
                    "raw_sha256": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "canonical_sha256": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                },
                "manifest": manifest,
                "content": content
            }]
        }
    });
    let request: DomainPackCompositionRequestDocument =
        serde_json::from_value(request).expect("closed composition request");
    assert_eq!(
        request.domain_pack_composition_request.authority,
        DomainPackCandidateAuthority::CandidateOnly
    );

    let projection = serde_json::json!({
        "schema_version": "0.1",
        "domain_pack_composition_projection": {
            "request_id": "composition.test",
            "authority": "candidate_only",
            "status": "blocked",
            "core_bundle_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "ordered_packs": [],
            "contribution_index": [],
            "provided_domain_refs": [],
            "declared_capability_refs": [],
            "composed_bundle": null,
            "gaps": [{
                "code": "missing_domain",
                "requirement_ref": "requirement.neutral",
                "subject_ref": "neutral.domain",
                "message": "Required domain has no selected pack.",
                "authority": "candidate_only"
            }],
            "issues": [],
            "composition_digest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        }
    });
    let projection: DomainPackCompositionProjectionDocument =
        serde_json::from_value(projection).expect("closed candidate projection");
    assert_eq!(
        projection.domain_pack_composition_projection.status,
        DomainPackCompositionStatus::Blocked
    );
    assert_eq!(projection.domain_pack_composition_projection.gaps.len(), 1);
}

#[test]
fn generated_schemas_expose_closed_enums_and_no_activation_state() {
    let manifest_schema =
        serde_json::to_string(&schema_for!(DomainPackManifestDocument)).expect("manifest schema");
    assert!(manifest_schema.contains("candidate_only"));
    assert!(manifest_schema.contains("replacement_slots"));
    assert!(manifest_schema.contains("replacement_declarations"));
    assert!(!manifest_schema.contains("installed"));
    assert!(!manifest_schema.contains("activated"));

    let content_schema =
        serde_json::to_string(&schema_for!(DomainPackContentDocument)).expect("content schema");
    assert!(content_schema.contains("workflow_overlay"));
    assert!(content_schema.contains("lifecycle_models"));
    assert!(content_schema.contains("provided_capabilities"));
    assert!(content_schema.contains("declaration_only"));

    let projection_schema =
        serde_json::to_value(schema_for!(DomainPackCompositionProjectionDocument))
            .expect("projection schema");
    let projection_schema = projection_schema.to_string();
    assert!(projection_schema.contains("missing_domain"));
    assert!(projection_schema.contains("replacement_not_bilateral"));
}
