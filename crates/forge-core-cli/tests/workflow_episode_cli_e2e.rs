use assert_cmd::Command;
use serde_json::Value;
use std::fs;
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
        stdout.contains(
            "forge-core workflow episode apply --root <path> --input-file <episode-apply.json>"
        ),
        "help must expose the guarded public episode seam:\n{stdout}"
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
