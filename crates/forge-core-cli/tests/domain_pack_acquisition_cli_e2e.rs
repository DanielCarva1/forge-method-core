use assert_cmd::Command;
use forge_core_contracts::{
    DomainPackAcquisitionIntent, DomainPackAcquisitionIntentDocument,
    DomainPackAcquisitionOperation, DomainPackCandidateAuthority,
    DomainPackDiscoveryProjectionDocument, StableId, DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION,
};
use serde_json::Value;
use std::fs;
use std::path::Path;

fn command(args: &[&str]) -> std::process::Output {
    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(args)
        .output()
        .expect("forge-core command")
}

fn write_yaml<T: serde::Serialize>(path: &Path, value: &T) {
    fs::write(path, yaml_serde::to_string(value).expect("YAML")).expect("fixture file");
}

#[test]
#[allow(clippy::too_many_lines)] // One subprocess journey proves replay, selection, and no mutation.
fn acquisition_plan_selects_only_exact_current_candidate_and_requires_trust() {
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-acquisition-plan-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("fixture root");
    let request = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/domain-pack-discovery/neutral-reviewed-match.yaml");
    let search = command(&[
        "domain-pack",
        "search",
        "--request-file",
        &request.display().to_string(),
        "--json",
    ]);
    assert!(
        search.status.success(),
        "search failed: {}",
        String::from_utf8_lossy(&search.stderr)
    );
    let search: Value = serde_json::from_slice(&search.stdout).expect("search envelope");
    let discovery: DomainPackDiscoveryProjectionDocument =
        serde_json::from_value(search["data"].clone()).expect("typed discovery projection");
    let projection = &discovery.domain_pack_discovery_projection;
    let selected = &projection.matches[0];
    let intent = DomainPackAcquisitionIntentDocument {
        schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_acquisition_intent: DomainPackAcquisitionIntent {
            acquisition_id: StableId("acquisition.cli.neutral".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            assurance_binding: projection.assurance_binding.clone(),
            discovery_projection_digest: projection.projection_digest.clone(),
            demand_digest: projection.demand_digest.clone(),
            candidate_id: selected.candidate_id.clone(),
            requirement_ref: selected.requirement_ref.clone(),
            operation: DomainPackAcquisitionOperation::Install,
            expected_project_snapshot_digest: projection.assurance_binding.snapshot_digest.clone(),
        },
    };
    let intent_path = root.join("intent.yaml");
    let projection_path = root.join("projection.yaml");
    write_yaml(&intent_path, &intent);
    write_yaml(&projection_path, &discovery);

    let plan = command(&[
        "domain-pack",
        "acquire",
        "plan",
        "--intent-file",
        &intent_path.display().to_string(),
        "--request-file",
        &request.display().to_string(),
        "--projection-file",
        &projection_path.display().to_string(),
        "--json",
    ]);
    assert!(
        plan.status.success(),
        "plan failed: stdout={} stderr={}",
        String::from_utf8_lossy(&plan.stdout),
        String::from_utf8_lossy(&plan.stderr)
    );
    let plan: Value = serde_json::from_slice(&plan.stdout).expect("plan envelope");
    assert_eq!(plan["command"], "domain-pack acquire plan");
    assert_eq!(
        plan["data"]["domain_pack_acquisition_plan"]["status"],
        "trust_ceremony_required"
    );
    assert_eq!(
        plan["data"]["domain_pack_acquisition_plan"]["selected"]["candidate_id"],
        selected.candidate_id.0
    );
    assert_eq!(
        plan["data"]["domain_pack_acquisition_plan"]["requirements"]["project_id"],
        projection.assurance_binding.project_id.0
    );
    assert!(
        plan["data"]["domain_pack_acquisition_plan"]["discovery_request_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    assert_eq!(
        plan["data"]["domain_pack_acquisition_plan"]["required_ceremonies"]
            .as_array()
            .expect("ceremonies")
            .len(),
        5
    );
    assert!(
        !root.join(".forge-method/domain-packs/active.yaml").exists(),
        "read-only planning must not create lifecycle state"
    );

    let mut stale_intent = intent.clone();
    stale_intent.domain_pack_acquisition_intent.candidate_id =
        StableId("candidate.absent".to_owned());
    write_yaml(&intent_path, &stale_intent);
    let rejected = command(&[
        "domain-pack",
        "acquire",
        "plan",
        "--intent-file",
        &intent_path.display().to_string(),
        "--request-file",
        &request.display().to_string(),
        "--projection-file",
        &projection_path.display().to_string(),
        "--json",
    ]);
    assert!(!rejected.status.success());

    let mut stale_request: forge_core_contracts::DomainPackDiscoveryRequestDocument =
        yaml_serde::from_slice(&fs::read(&request).expect("request corpus"))
            .expect("typed request corpus");
    stale_request
        .domain_pack_discovery_request
        .uncertainties
        .push("changed after projection".to_owned());
    let stale_request_path = root.join("stale-request.yaml");
    write_yaml(&stale_request_path, &stale_request);
    write_yaml(&intent_path, &intent);
    let replay_rejected = command(&[
        "domain-pack",
        "acquire",
        "plan",
        "--intent-file",
        &intent_path.display().to_string(),
        "--request-file",
        &stale_request_path.display().to_string(),
        "--projection-file",
        &projection_path.display().to_string(),
        "--json",
    ]);
    assert!(!replay_rejected.status.success());
}
