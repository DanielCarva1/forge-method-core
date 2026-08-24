use assert_cmd::Command;
use forge_core_contracts::PostBuildVerifyEpisodeDocument;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn workflow_help_exposes_guarded_episode_apply() {
    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(["workflow", "--help"])
        .output()
        .expect("workflow help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(
        stdout.contains("forge-core workflow episode prepare --root <path>"),
        "help must expose read-only episode preparation:\n{stdout}"
    );
    assert!(
        stdout.contains(
            "forge-core workflow episode apply --root <path> --input-file <episode-apply.json>"
        ),
        "help must expose the guarded public episode seam:\n{stdout}"
    );
}

#[test]
fn episode_prepare_is_bounded_and_does_not_write_project_state() {
    let root = temporary_root("forge-episode-prepare");
    let project = root.join("app");
    let state = root.join("forge-app").join(".forge-method");
    fs::create_dir_all(&project).expect("temporary project");
    fs::create_dir_all(&state).expect("temporary state");
    fs::write(project.join("README.md"), "episode preparation consumer\n")
        .expect("project artifact");
    fs::write(
        project.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("project link");
    fs::write(state.join("state.yaml"), "current_phase: 4-build-verify\n")
        .expect("compatibility state");
    let init = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(["workflow", "init", "--root"])
        .arg(&project)
        .args(["--readiness-profile", "solo_cooperative", "--json"])
        .output()
        .expect("workflow init");
    assert!(init.status.success(), "init failed: {:?}", init.stderr);
    let before = state_tree(&state);

    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(["workflow", "episode", "prepare", "--root"])
        .arg(&project)
        .arg("--json")
        .output()
        .expect("episode prepare");
    let after = state_tree(&state);
    let _ = fs::remove_dir_all(&root);

    assert!(
        output.status.success(),
        "prepare failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(before, after, "read-only preparation changed Forge state");
    assert!(
        output.stdout.len() < 32 * 1024,
        "packet is unexpectedly large"
    );
    let mut envelope: Value =
        serde_json::from_slice(&output.stdout).expect("JSON prepare envelope");
    assert_eq!(envelope["command"], "workflow.episode.prepare");
    assert_eq!(envelope["data"]["authority"], "candidate_preparation_only");
    assert_eq!(envelope["data"]["current_phase"], "1-discovery");
    assert_eq!(envelope["data"]["applicable_now"], false);
    assert_eq!(envelope["data"]["apply_argv"][0], "forge-core");
    assert_eq!(envelope["data"]["apply_argv"][1], "workflow");
    assert_eq!(envelope["data"]["apply_argv"][2], "episode");
    assert_eq!(
        envelope["data"]["apply_input_template"]["expected_snapshot_digest"],
        envelope["data"]["binding"]["snapshot_digest"]
    );
    assert!(envelope["data"]["required_replacements"]
        .as_array()
        .is_some_and(|items| items
            .iter()
            .any(|item| item == "${DEPLOYMENT_OBSERVATIONS_JSON}")));
    let template = &mut envelope["data"]["apply_input_template"];
    let release_digest = template["document"]["post_build_verify_episode"]["release_subject"]
        ["release_digest"]
        .as_str()
        .expect("release digest")
        .to_owned();
    let episode = &mut template["document"]["post_build_verify_episode"];
    episode["episode_id"] = serde_json::json!("episode.prepared");
    episode["deployment_observations"] = serde_json::json!([{
        "observation_id": "observation.prepared",
        "release_digest": release_digest,
        "deployment": {"subject_ref": "deployment/prepared", "subject_digest": digest('2')},
        "outcome": "healthy",
        "observed_at_unix": 1
    }]);
    episode["operational_evidence"] = serde_json::json!([{
        "evidence_id": "evidence.prepared",
        "release_digest": release_digest,
        "evidence": {"subject_ref": "evidence/prepared", "subject_digest": digest('3')},
        "kind": "verification",
        "outcome": "supports_readiness",
        "observed_at_unix": 1
    }]);
    episode["evolution"] = serde_json::json!({
        "evolution_episode_id": "evolution.prepared",
        "generation": 1,
        "release_digest": release_digest,
        "status": "dormant",
        "trigger": "planned_follow_up",
        "proposed_entry_phase": "1-discovery",
        "continuity_subject": {"subject_ref": "continuity/prepared", "subject_digest": digest('4')}
    });
    episode["continuity"] = serde_json::json!({
        "context_recovery_subject": {"subject_ref": "recovery/prepared", "subject_digest": digest('5')},
        "next_action_ref": "action.prepared"
    });
    episode["episode_digest"] = serde_json::json!(digest('0'));
    let mut document: PostBuildVerifyEpisodeDocument =
        serde_json::from_value(template["document"].clone()).expect("typed prepared document");
    document.post_build_verify_episode.episode_digest =
        document.episode_digest().expect("canonical episode digest");
    assert!(
        document.validate().is_empty(),
        "filled preparation template must satisfy the existing episode contract"
    );
}

#[test]
fn episode_apply_rejects_oversized_input_before_project_access() {
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "forge-episode-limit-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temporary root");
    let input = root.join("oversized.json");
    fs::write(&input, vec![b' '; 256 * 1024 + 1]).expect("oversized input");

    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(["workflow", "episode", "apply", "--root"])
        .arg(root.join("missing-project"))
        .arg("--input-file")
        .arg(&input)
        .arg("--json")
        .output()
        .expect("episode apply");

    let _ = fs::remove_dir_all(&root);
    assert!(!output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("JSON failure envelope");
    assert_eq!(envelope["command"], "workflow.episode.apply");
    assert_eq!(envelope["exit_reason"], "invalid_decision_shape");
    assert!(
        envelope["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("maximum 262144 bytes")),
        "unexpected envelope: {envelope}"
    );
}

fn temporary_root(prefix: &str) -> std::path::PathBuf {
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::SeqCst)
    ))
}

fn state_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn visit(root: &Path, current: &Path, files: &mut Vec<(String, Vec<u8>)>) {
        let mut entries = fs::read_dir(current)
            .expect("read state directory")
            .map(|entry| entry.expect("state entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let kind = entry.file_type().expect("state entry type");
            if kind.is_dir() {
                visit(root, &path, files);
            } else if kind.is_file() {
                files.push((
                    path.strip_prefix(root)
                        .expect("state-relative path")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    fs::read(path).expect("state file"),
                ));
            }
        }
    }

    let mut files = Vec::new();
    visit(root, root, &mut files);
    files
}

fn digest(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}
