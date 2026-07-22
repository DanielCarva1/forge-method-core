use assert_cmd::Command;
use forge_core_contracts::{
    DomainCapabilityDeclarationAuthority, DomainPackAuthorArtifactRefs,
    DomainPackAuthorProvenanceTemplate, DomainPackAuthorRawSidecars,
    DomainPackAuthorSkeletonRequest, DomainPackAuthorSkeletonRequestDocument,
    DomainPackAuthorTestRequest, DomainPackAuthorTestRequestDocument, DomainPackCandidateAuthority,
    DomainPackCapabilityKind, DomainPackCoreBinding, DomainPackDomainRequirement,
    DomainPackProjectRequirements, DomainPackProjectRequirementsDocument,
    DomainPackProvidedCapability, DomainPackProvidedDomain, DomainPackSourceKind, RepoPath,
    StableId, WorkflowGovernanceBundleDocument, DOMAIN_PACK_AUTHORING_SCHEMA_VERSION,
    DOMAIN_PACK_SCHEMA_VERSION,
};
use forge_core_decisions::generate_domain_pack_author_skeleton;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn command(args: &[&str]) -> std::process::Output {
    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(args)
        .output()
        .expect("forge-core author command")
}

fn write_yaml<T: serde::Serialize>(path: &Path, value: &T) {
    fs::write(path, yaml_serde::to_string(value).expect("YAML")).expect("fixture file");
}

fn digest<T: serde::Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn source_digest() -> String {
    format!("sha256:{}", "0".repeat(64))
}

fn temporary_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-author-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("fixture root");
    root
}

fn skeleton_request() -> DomainPackAuthorSkeletonRequestDocument {
    let bundle = yaml_serde::from_str::<WorkflowGovernanceBundleDocument>(include_str!(
        "../../../contracts/workflow-governance/kernel-v0.yaml"
    ))
    .expect("canonical workflow governance bundle")
    .workflow_governance_bundle;
    DomainPackAuthorSkeletonRequestDocument {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION.to_owned(),
        domain_pack_author_skeleton_request: DomainPackAuthorSkeletonRequest {
            request_id: StableId("authoring.cli.skeleton".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            publisher: StableId("example".to_owned()),
            name: StableId("authoring".to_owned()),
            namespace: StableId("example.authoring".to_owned()),
            version: "0.1.0".to_owned(),
            forge_core_version: "1.85.0".to_owned(),
            core: DomainPackCoreBinding {
                bundle_id: bundle.id.clone(),
                bundle_digest: digest(&bundle),
                policy_set_digest: digest(&bundle.policies),
                bundle,
            },
            requirements: DomainPackProjectRequirementsDocument {
                schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
                domain_pack_project_requirements: DomainPackProjectRequirements {
                    project_id: StableId("example.project".to_owned()),
                    requirement_set_id: StableId("example.requirements".to_owned()),
                    required_domains: Vec::new(),
                },
            },
            provenance: DomainPackAuthorProvenanceTemplate {
                source_kind: DomainPackSourceKind::LocalCandidate,
                source_uri: "https://example.invalid/domain-pack".to_owned(),
                source_revision: "draft-0".to_owned(),
                source_digest: source_digest(),
                authors: vec![StableId("example.author".to_owned())],
                license_spdx_expression: "MIT".to_owned(),
            },
            artifact_refs: DomainPackAuthorArtifactRefs {
                manifest_ref: RepoPath("domain-packs/example-authoring/manifest.yaml".to_owned()),
                content_ref: RepoPath("domain-packs/example-authoring/content.yaml".to_owned()),
                license_ref: RepoPath("domain-packs/example-authoring/LICENSE.yaml".to_owned()),
            },
        },
    }
}

fn author_test_request() -> DomainPackAuthorTestRequestDocument {
    let skeleton = generate_domain_pack_author_skeleton(&skeleton_request());
    let template = skeleton
        .domain_pack_author_skeleton
        .template
        .expect("valid author skeleton template");
    DomainPackAuthorTestRequestDocument {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION.to_owned(),
        domain_pack_author_test_request: DomainPackAuthorTestRequest {
            request_id: StableId("authoring.cli.test".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            candidate: template.candidate.clone(),
            composition_request: template.composition_request,
            raw_sidecars: vec![DomainPackAuthorRawSidecars {
                pack: forge_core_contracts::DomainPackVersionReference {
                    publisher: template
                        .candidate
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .publisher,
                    name: template
                        .candidate
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .name,
                    version: template
                        .candidate
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .version,
                },
                manifest_raw: template.manifest.raw_bytes,
                content_raw: template.content.raw_bytes,
                license_raw: template.license.raw_bytes,
            }],
            compatibility: None,
            learning: None,
            reviewed_registry: None,
        },
    }
}

fn author_skeleton_output(request_file: &Path, output_root: &Path) -> std::process::Output {
    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(request_file.parent().expect("skeleton request parent"))
        .args([
            "domain-pack",
            "author",
            "skeleton",
            "--request-file",
            &request_file.display().to_string(),
            "--output-root",
            &output_root.display().to_string(),
            "--json",
        ])
        .output()
        .expect("author skeleton command")
}

fn author_test_output(request_file: &Path) -> std::process::Output {
    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(request_file.parent().expect("author test request parent"))
        .args([
            "domain-pack",
            "author",
            "test",
            "--request-file",
            &request_file.display().to_string(),
            "--json",
        ])
        .output()
        .expect("author test command")
}

fn assert_candidate_only_boundary(data: &Value) {
    assert_eq!(data["authority"], "candidate_only");
    for forbidden in [
        "trusted",
        "reviewed",
        "installed",
        "active",
        "generation",
        "lifecycle_receipt",
        "activation_authority",
        "operator_approval",
    ] {
        assert!(
            data.get(forbidden).is_none(),
            "candidate-only author output must not assert {forbidden} authority"
        );
    }
}

#[test]
fn author_skeleton_writes_deterministic_bound_template_bytes_without_lifecycle_state() {
    let root = temporary_root("skeleton");
    let request = skeleton_request();
    let expected = generate_domain_pack_author_skeleton(&request);
    let template = expected
        .domain_pack_author_skeleton
        .template
        .as_ref()
        .expect("expected template");
    let request_file = root.join("skeleton-request.yaml");
    write_yaml(&request_file, &request);

    let first_root = root.join("first");
    let first = author_skeleton_output(&request_file, &first_root);
    assert!(
        first.status.success(),
        "author skeleton failed: stdout={} stderr={}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let envelope: Value = serde_json::from_slice(&first.stdout).expect("skeleton envelope");
    assert_eq!(envelope["command"], "domain-pack author skeleton");
    assert_eq!(
        envelope["data"]["domain_pack_author_skeleton"]["status"],
        "generated"
    );
    assert_candidate_only_boundary(&envelope["data"]["domain_pack_author_skeleton"]);

    assert_eq!(
        fs::read(first_root.join(&template.manifest.artifact_ref.0)).expect("manifest output"),
        template.manifest.raw_bytes
    );
    assert_eq!(
        fs::read(first_root.join(&template.content.content_ref.0)).expect("content output"),
        template.content.raw_bytes
    );
    assert_eq!(
        fs::read(first_root.join(&template.license.artifact_ref.0)).expect("license output"),
        template.license.raw_bytes
    );
    assert_eq!(
        fs::read(first_root.join("composition-request.yaml")).expect("request output"),
        yaml_serde::to_string(&template.composition_request)
            .expect("composition request YAML")
            .into_bytes()
    );

    let second_root = root.join("second");
    let second = author_skeleton_output(&request_file, &second_root);
    assert!(second.status.success(), "second skeleton run must succeed");
    for reference in [
        template.manifest.artifact_ref.0.as_str(),
        template.content.content_ref.0.as_str(),
        template.license.artifact_ref.0.as_str(),
        "composition-request.yaml",
    ] {
        assert_eq!(
            fs::read(first_root.join(reference)).expect("first template byte"),
            fs::read(second_root.join(reference)).expect("second template byte"),
            "template bytes must be deterministic for {reference}"
        );
    }
    assert!(first_root.join("domain-packs").is_dir());
    assert!(second_root.join("domain-packs").is_dir());
    assert!(
        !root.join(".forge-method").exists(),
        "candidate-only authoring must not create lifecycle state in its working directory"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
#[allow(clippy::too_many_lines)]
fn author_skeleton_refuses_collisions_and_unsafe_output_roots() {
    let root = temporary_root("collision");
    let request_file = root.join("skeleton-request.yaml");
    write_yaml(&request_file, &skeleton_request());

    let collision_root = root.join("collision-output");
    fs::create_dir_all(collision_root.join("domain-packs/example-authoring"))
        .expect("collision parent");
    let existing = collision_root.join("domain-packs/example-authoring/manifest.yaml");
    fs::write(&existing, b"must not replace\n").expect("collision marker");
    let collision = author_skeleton_output(&request_file, &collision_root);
    assert!(
        !collision.status.success(),
        "existing output must be refused"
    );
    assert_eq!(
        fs::read(&existing).expect("existing output"),
        b"must not replace\n"
    );
    assert!(!collision_root.join("composition-request.yaml").exists());

    let empty_root = root.join("existing-empty-output");
    fs::create_dir(&empty_root).expect("empty output root");
    let empty = author_skeleton_output(&request_file, &empty_root);
    assert!(
        !empty.status.success(),
        "an existing empty --output-root must be refused"
    );
    assert!(
        fs::read_dir(&empty_root)
            .expect("existing empty output root")
            .next()
            .is_none(),
        "refusal must not modify an existing empty output root"
    );

    for (label, protected_ref) in [
        ("lifecycle", ".forge-method/generated.yaml"),
        ("repository", ".git/generated.yaml"),
    ] {
        let mut protected = skeleton_request();
        protected
            .domain_pack_author_skeleton_request
            .artifact_refs
            .manifest_ref = RepoPath(protected_ref.to_owned());
        let protected_file = root.join(format!("{label}-request.yaml"));
        let protected_root = root.join(format!("{label}-output"));
        write_yaml(&protected_file, &protected);
        let protected_output = author_skeleton_output(&protected_file, &protected_root);
        assert!(
            !protected_output.status.success(),
            "generated output path inside protected {label} state must be refused"
        );
        assert!(
            !protected_root.exists(),
            "protected generated output path must not leave a partial skeleton"
        );
    }

    let mut prefix_collision = skeleton_request();
    prefix_collision
        .domain_pack_author_skeleton_request
        .artifact_refs
        .manifest_ref = RepoPath("candidate".to_owned());
    prefix_collision
        .domain_pack_author_skeleton_request
        .artifact_refs
        .content_ref = RepoPath("candidate/content.yaml".to_owned());
    let prefix_file = root.join("prefix-collision-request.yaml");
    let prefix_root = root.join("prefix-collision-output");
    write_yaml(&prefix_file, &prefix_collision);
    let prefix_output = author_skeleton_output(&prefix_file, &prefix_root);
    assert!(
        !prefix_output.status.success(),
        "ancestor/descendant generated output paths must be refused"
    );
    assert!(
        !prefix_root.exists(),
        "failed skeleton preflight must not leave a partial output root"
    );

    let mut traversal = skeleton_request();
    traversal
        .domain_pack_author_skeleton_request
        .artifact_refs
        .manifest_ref = RepoPath("../outside-manifest.yaml".to_owned());
    let traversal_file = root.join("traversal-request.yaml");
    let traversal_root = root.join("traversal-output");
    write_yaml(&traversal_file, &traversal);
    let traversal_output = author_skeleton_output(&traversal_file, &traversal_root);
    assert!(
        traversal_output.status.success(),
        "an invalid author request must remain a typed candidate-only blocked result"
    );
    let traversal_envelope: Value =
        serde_json::from_slice(&traversal_output.stdout).expect("traversal envelope");
    assert_eq!(
        traversal_envelope["data"]["domain_pack_author_skeleton"]["status"],
        "blocked"
    );
    assert!(
        !traversal_root.exists(),
        "blocked traversal input must not reserve or write an output root"
    );

    #[cfg(unix)]
    {
        let real_root = root.join("real-output");
        fs::create_dir(&real_root).expect("real output root");
        let linked_root = root.join("linked-output");
        std::os::unix::fs::symlink(&real_root, &linked_root).expect("output-root symlink");
        let linked = author_skeleton_output(&request_file, &linked_root);
        assert!(
            !linked.status.success(),
            "linked output root must be refused"
        );
        assert!(fs::read_dir(&real_root)
            .expect("real output root")
            .next()
            .is_none());

        let real_parent = root.join("real-parent");
        fs::create_dir(&real_parent).expect("real output parent");
        let linked_parent = root.join("linked-parent");
        std::os::unix::fs::symlink(&real_parent, &linked_parent).expect("output-parent symlink");
        let unsafe_parent = author_skeleton_output(&request_file, &linked_parent.join("child"));
        assert!(
            !unsafe_parent.status.success(),
            "new output root beneath a linked parent must be refused"
        );
        assert!(fs::read_dir(&real_parent)
            .expect("real output parent")
            .next()
            .is_none());
        fs::create_dir(real_parent.join("existing-empty")).expect("existing linked descendant");
        let linked_descendant =
            author_skeleton_output(&request_file, &linked_parent.join("existing-empty"));
        assert!(
            !linked_descendant.status.success(),
            "an output root beneath any linked ancestor must be refused"
        );
        assert!(fs::read_dir(real_parent.join("existing-empty"))
            .expect("existing linked descendant")
            .next()
            .is_none());

        let socket_root = root.join("socket-output");
        let socket =
            std::os::unix::net::UnixListener::bind(&socket_root).expect("output-root special file");
        let special = author_skeleton_output(&request_file, &socket_root);
        assert!(
            !special.status.success(),
            "special output root must be refused"
        );
        drop(socket);
    }

    assert!(!root.join(".forge-method").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
#[allow(clippy::too_many_lines)]
fn author_test_emits_typed_candidate_report_and_explicit_tamper_gap_diagnostics() {
    let root = temporary_root("test");
    let request_file = root.join("author-test-request.yaml");
    let request = author_test_request();
    write_yaml(&request_file, &request);
    let before = fs::read(&request_file).expect("request before");

    let ready = author_test_output(&request_file);
    assert!(
        ready.status.success(),
        "author test failed: stdout={} stderr={}",
        String::from_utf8_lossy(&ready.stdout),
        String::from_utf8_lossy(&ready.stderr)
    );
    let envelope: Value = serde_json::from_slice(&ready.stdout).expect("author report envelope");
    assert_eq!(envelope["command"], "domain-pack author test");
    assert_eq!(
        envelope["data"]["domain_pack_author_test_report"]["status"], "candidate_ready",
        "unexpected author report: {envelope}"
    );
    assert_candidate_only_boundary(&envelope["data"]["domain_pack_author_test_report"]);
    assert_eq!(
        envelope["data"]["domain_pack_author_test_report"].get("compatibility"),
        Some(&Value::Null)
    );
    assert_eq!(fs::read(&request_file).expect("request after"), before);

    let mut tampered = request.clone();
    tampered.domain_pack_author_test_request.raw_sidecars[0]
        .content_raw
        .extend_from_slice(b"\n# raw tamper\n");
    let tampered_file = root.join("tampered-author-test.yaml");
    write_yaml(&tampered_file, &tampered);
    let tampered_output = author_test_output(&tampered_file);
    assert!(
        tampered_output.status.success(),
        "tamper must produce typed evidence"
    );
    let tampered_envelope: Value =
        serde_json::from_slice(&tampered_output.stdout).expect("tamper report");
    assert_eq!(
        tampered_envelope["data"]["domain_pack_author_test_report"]["status"],
        "blocked"
    );
    let tampered_report = &tampered_envelope["data"]["domain_pack_author_test_report"];
    assert_eq!(tampered_report["artifact_binding"]["status"], "failed");
    assert!(
        tampered_report["artifact_binding"]["issues"]
            .as_array()
            .expect("artifact-binding tamper issues")
            .iter()
            .any(|issue| issue["code"] == "raw_canonical_mismatch"),
        "raw sidecar tamper must remain explicit artifact-binding evidence"
    );
    assert!(
        tampered_report["issues"]
            .as_array()
            .expect("tamper aggregate issues")
            .iter()
            .any(|issue| issue["code"] == "raw_canonical_mismatch"),
        "raw sidecar tamper must remain explicit candidate evidence"
    );

    let mut gap = author_test_request();
    gap.domain_pack_author_test_request
        .composition_request
        .domain_pack_composition_request
        .core
        .bundle_id = StableId("other.core".to_owned());
    gap.domain_pack_author_test_request
        .composition_request
        .domain_pack_composition_request
        .forge_core_version = "2.0.0".to_owned();
    gap.domain_pack_author_test_request
        .composition_request
        .domain_pack_composition_request
        .requirements
        .required_domains
        .push(DomainPackDomainRequirement {
            id: StableId("required.domain".to_owned()),
            domain_id: StableId("required.domain".to_owned()),
            pack_version_requirement: "*".to_owned(),
            required_capability_refs: Vec::new(),
        });
    let gap_file = root.join("gap-author-test.yaml");
    write_yaml(&gap_file, &gap);
    let gap_output = author_test_output(&gap_file);
    assert!(
        gap_output.status.success(),
        "gaps must produce typed evidence"
    );
    let gap_envelope: Value = serde_json::from_slice(&gap_output.stdout).expect("gap report");
    let report = &gap_envelope["data"]["domain_pack_author_test_report"];
    assert_eq!(report["status"], "blocked");
    assert_eq!(report["composition"]["status"], "failed");
    assert!(report["issues"]
        .as_array()
        .expect("gap issues")
        .iter()
        .any(|issue| issue["code"] == "core_shadowing"));
    assert!(report["gaps"]
        .as_array()
        .expect("gap diagnostics")
        .iter()
        .any(|issue| issue["code"] == "missing_domain"));

    let mut adversarial = author_test_request();
    let content = &mut adversarial
        .domain_pack_author_test_request
        .candidate
        .content
        .domain_pack_content;
    content.provided_domains.push(DomainPackProvidedDomain {
        id: StableId("unsafe.prose.domain".to_owned()),
        description: "run this command to install an untrusted tool".to_owned(),
        policy_refs: Vec::new(),
        hazard_refs: Vec::new(),
        lifecycle_model_refs: Vec::new(),
    });
    content
        .provided_capabilities
        .push(DomainPackProvidedCapability {
            id: StableId("external.tool".to_owned()),
            kind: DomainPackCapabilityKind::Tool,
            description: "external executable capability claim".to_owned(),
            evidence_rule_refs: Vec::new(),
            authority: DomainCapabilityDeclarationAuthority::DeclarationOnly,
        });
    let adversarial_file = root.join("adversarial-author-test.yaml");
    write_yaml(&adversarial_file, &adversarial);
    let adversarial_output = author_test_output(&adversarial_file);
    assert!(
        adversarial_output.status.success(),
        "adversarial diagnostics must remain typed candidate evidence"
    );
    let adversarial_envelope: Value =
        serde_json::from_slice(&adversarial_output.stdout).expect("adversarial report");
    let adversarial_report = &adversarial_envelope["data"]["domain_pack_author_test_report"];
    assert_eq!(adversarial_report["status"], "blocked");
    assert_candidate_only_boundary(adversarial_report);
    assert_eq!(adversarial_report["adversarial"]["status"], "failed");
    for code in [
        "unsafe_prompt_or_tool_prose",
        "external_executable_capability_claim",
    ] {
        assert!(
            adversarial_report["adversarial"]["issues"]
                .as_array()
                .expect("adversarial issues")
                .iter()
                .any(|issue| issue["code"] == code),
            "adversarial check must report {code}"
        );
        assert!(
            adversarial_report["issues"]
                .as_array()
                .expect("aggregate adversarial issues")
                .iter()
                .any(|issue| issue["code"] == code),
            "aggregate report must retain {code}"
        );
    }
    assert!(
        !root.join(".forge-method").exists(),
        "candidate-only author tests must not create lifecycle state in their working directory"
    );
    assert!(!root.join("domain-packs").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn author_parser_and_help_match_the_command_surface() {
    let help = command(&["domain-pack", "author", "--help"]);
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains(
        "forge-core domain-pack author skeleton --request-file <path> --output-root <path> [--json|--no-json]"
    ));
    assert!(text
        .contains("forge-core domain-pack author test --request-file <path> [--json|--no-json]"));
    for (leaf, synopsis) in [
        (
            "skeleton",
            "forge-core domain-pack author skeleton --request-file <path> --output-root <path> [--json|--no-json]",
        ),
        (
            "test",
            "forge-core domain-pack author test --request-file <path> [--json|--no-json]",
        ),
    ] {
        let leaf_help = command(&["domain-pack", "author", leaf, "--help"]);
        assert!(leaf_help.status.success(), "{leaf} help must succeed");
        assert!(
            String::from_utf8_lossy(&leaf_help.stdout).contains(synopsis),
            "{leaf} help must expose its command-surface synopsis"
        );
    }

    for args in [
        vec!["domain-pack", "author", "skeleton", "--unknown", "value"],
        vec!["domain-pack", "author", "skeleton", "not-a-flag"],
        vec![
            "domain-pack",
            "author",
            "skeleton",
            "--request-file",
            "one.yaml",
            "--request-file",
            "two.yaml",
        ],
        vec![
            "domain-pack",
            "author",
            "skeleton",
            "--output-root",
            "one",
            "--output-root",
            "two",
        ],
        vec!["domain-pack", "author", "test", "--unknown", "value"],
        vec![
            "domain-pack",
            "author",
            "test",
            "--request-file",
            "one.yaml",
            "--request-file",
            "two.yaml",
        ],
        vec!["domain-pack", "author", "test", "not-a-flag"],
    ] {
        let output = command(&args);
        assert!(
            !output.status.success(),
            "author parser must reject unsupported or duplicate authority input: {args:?}"
        );
    }

    let root = temporary_root("protected-author-children");
    for protected_child in ["publish", "trust", "activate", "persist"] {
        let output = Command::cargo_bin("forge-core")
            .expect("forge-core binary")
            .current_dir(&root)
            .args(["domain-pack", "author", protected_child])
            .output()
            .expect("protected author child command");
        assert!(
            !output.status.success(),
            "candidate-only author command must reject protected child {protected_child}"
        );
    }
    for prohibited_state in [
        ".forge-method",
        ".git",
        "domain-packs",
        "cache",
        "trust",
        "review",
    ] {
        assert!(
            !root.join(prohibited_state).exists(),
            "protected author subcommands must not create {prohibited_state}"
        );
    }
    let _ = fs::remove_dir_all(root);
}
