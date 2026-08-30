//! End-to-end tests for `forge-core start` (F12 Guided Start).
//!
//! Exercises the binary as a real subprocess across all five [`BootstrapState`]s,
//! mirroring the `project_init_e2e.rs` harness pattern (`assert_cmd::Command` +
//! `FreshParent` with Drop cleanup). The unit tests in `start_cmd.rs` cover the
//! pure classifier; these tests verify the full argv → stdout-envelope → exit-code
//! contract that agents consume.
//!
//! What is locked here:
//! - clean, never-initialized projects bootstrap exactly once;
//! - linked missing/incomplete state fails closed with byte-identical filesystem state;
//! - `start` emits exactly one `CliEnvelope` as JSON on stdout;
//! - state loss is distinct from malformed-link corruption and clean bootstrap;
//! - healthy-state routing is idempotent and nonmutating.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The bootstrap states, as they appear in the `state` field of the `start`
/// payload. Mirrors `BootstrapState::as_str` in `start_cmd.rs`. Kept as plain
/// strings (not imported) so this test stays a black-box against the binary —
/// it catches wire-form regressions the unit tests would not.
///
/// `no_link` is covered by its own dedicated case below (it returns an `ok`
/// envelope with a `project init` `next_step`), so it has no entry in the
/// bootstrap-state constant list.
const STATE_SIDECAR_READY: &str = "sidecar_ready_no_contract";
const STATE_CONTRACT_PRESENT: &str = "contract_present";
const STATE_PREVIEW_RUN: &str = "preview_run";

const PROJECT_LINK_FILE_NAME: &str = ".forge-method.yaml";
const PROJECT_LINK_SCHEMA_VERSION: &str = "forge_project_link_v1";

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

/// A fresh temp parent under the OS temp dir, cleaned up on drop. Mirrors the
/// `project_init_e2e` harness so the two stay consistent.
struct FreshParent {
    path: PathBuf,
}

impl FreshParent {
    fn new(label: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        // Use the OS temp dir, NOT repo_root()/target/. The repo-identity
        // validation (incident closure) rejects a consumer root nested inside a
        // foreign git repo, and target/ is inside the forge core repo — so the
        // test's bootstrap sidecar would be rejected. std::env::temp_dir()
        // returns a Windows path (D:\Temp\...) on this host, which avoids the
        // WSL→Windows /tmp mangling the old DD46 comment warned about. On
        // macOS, use the physical /private/var path so canonical project roots
        // and no-follow storage checks observe the same fixture identity.
        let temporary_directory = std::env::temp_dir();
        #[cfg(target_os = "macos")]
        let temporary_directory = fs::canonicalize(temporary_directory)
            .expect("canonicalize start test temporary directory");
        let path =
            temporary_directory.join(format!("start-e2e-{label}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create fresh parent");
        Self { path }
    }
}

impl Drop for FreshParent {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Run `forge-core start --root <app> --json`; return `(exit_ok, envelope_json)`.
fn run_start(app: &Path) -> (bool, Value) {
    let output = bin()
        .args(["start", "--root"])
        .arg(app)
        .arg("--json")
        .output()
        .expect("run forge-core start");
    let exit_ok = output.status.success();
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be a CliEnvelope JSON: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (exit_ok, json)
}

fn run_start_text(app: &Path) -> std::process::Output {
    bin()
        .args(["start", "--root"])
        .arg(app)
        .arg("--text")
        .output()
        .expect("run forge-core start in text mode")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_reinitialize(
    subcommand: &str,
    app: &Path,
    destination: &Path,
    diagnosis: &Path,
    plan_file: &Path,
    plan_digest: Option<&str>,
    confirmation: Option<&str>,
) -> std::process::Output {
    let mut command = bin();
    command
        .args(["project", "reinitialize", subcommand, "--root"])
        .arg(app)
        .arg("--destination")
        .arg(destination)
        .args(["--abandoned-authority-id", "abandoned-authority"])
        .args(["--new-project-id", "successor-project"])
        .args(["--new-authority-id", "successor-authority"])
        .arg("--state-loss-diagnosis")
        .arg(diagnosis)
        .arg("--plan-file")
        .arg(plan_file);
    if let Some(plan_digest) = plan_digest {
        command.args(["--plan-digest", plan_digest]);
    }
    if let Some(confirmation) = confirmation {
        command.args(["--confirm", confirmation]);
    }
    command
        .arg("--json")
        .output()
        .expect("run forge-core project reinitialize")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn output_envelope(output: &std::process::Output, label: &str) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{label} must return one JSON envelope: {error}; stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// Write a Project Link pointing at a sidecar/state root relative to `app`.
fn write_link(app: &Path, sidecar_rel: &str, state_rel: &str) {
    fs::write(
        app.join(PROJECT_LINK_FILE_NAME),
        format!(
            "schema_version: {PROJECT_LINK_SCHEMA_VERSION}\n\
             project_id: app\n\
             sidecar_root: {sidecar_rel}\n\
             state_root: {state_rel}\n",
        ),
    )
    .expect("write project link");
}

/// Create the minimum authoritative Forge state shape used by `start`.
fn make_state_tree(state: &Path) {
    for d in [
        "",
        "artifacts",
        "claims-active",
        "evidence",
        "handoffs/expired-claims",
        "index",
        "locks",
        "traces",
        "wal",
    ] {
        fs::create_dir_all(state.join(d)).expect("create state dir");
    }
    for f in [
        "ledger.ndjson",
        "wal/replay.fmr1",
        "replay-wal.manifest.json",
    ] {
        fs::write(state.join(f), b"").expect("create authority marker");
    }
}

fn tree_snapshot(root: &Path) -> Vec<(String, String, Vec<u8>)> {
    fn visit(base: &Path, path: &Path, entries: &mut Vec<(String, String, Vec<u8>)>) {
        let mut children = fs::read_dir(path)
            .expect("read snapshot directory")
            .map(|entry| entry.expect("read snapshot entry").path())
            .collect::<Vec<_>>();
        children.sort();
        for child in children {
            let relative = child
                .strip_prefix(base)
                .expect("snapshot path below base")
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = fs::symlink_metadata(&child).expect("snapshot metadata");
            if metadata.file_type().is_symlink() {
                let target = fs::read_link(&child)
                    .expect("snapshot symlink target")
                    .to_string_lossy()
                    .into_owned()
                    .into_bytes();
                entries.push((relative, "symlink".to_string(), target));
            } else if metadata.is_dir() {
                entries.push((relative, "dir".to_string(), Vec::new()));
                visit(base, &child, entries);
            } else {
                entries.push((
                    relative,
                    "file".to_string(),
                    fs::read(&child).expect("snapshot file bytes"),
                ));
            }
        }
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries
}

fn run_workflow_json(app: &Path, subcommand: &str) -> Value {
    let output = bin()
        .args(["workflow", subcommand, "--root"])
        .arg(app)
        .arg("--json")
        .output()
        .unwrap_or_else(|error| panic!("run workflow {subcommand}: {error}"));
    assert!(
        output.status.success(),
        "workflow {subcommand} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "parse workflow {subcommand} envelope: {error}; stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn resolved_state_root(app: &Path) -> PathBuf {
    let output = bin()
        .args(["project", "resolve", "--root"])
        .arg(app)
        .arg("--json")
        .output()
        .expect("resolve project after workflow handoff");
    assert!(
        output.status.success(),
        "project resolve failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("parse project resolve envelope");
    PathBuf::from(
        envelope["data"]["state_root"]
            .as_str()
            .expect("project resolve exposes the actual state root"),
    )
}

fn crash_replace_residue_paths(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("walk actual Forge state root") {
            let entry = entry.expect("Forge state entry");
            let file_type = entry.file_type().expect("Forge state entry type");
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.contains(".forge-retained-delete-")
                || name.contains(".forge-crash-absence-claim-")
                || name.ends_with(".forge-next")
                || name.ends_with(".forge-previous")
                || name.ends_with(".forge-transaction")
            {
                found.push(entry.path());
            }
        }
    }
    found
}

fn assert_agent_native_init_handoff(env: &Value, app: &Path, state: &str) {
    let root = app.display().to_string();
    assert_eq!(
        env["data"]["next_step"]["argv"],
        serde_json::json!(["forge-core", "workflow", "init", "--root", root]),
        "{state} should expose typed workflow init argv"
    );
    assert!(
        env["data"]["next_step"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("forge-core workflow init --root ")),
        "{state} should hand off to workflow init"
    );

    let references = env["data"]["next_step"]["references"]
        .as_array()
        .unwrap_or_else(|| panic!("{state} references should be an array"));
    assert!(
        references
            .first()
            .and_then(Value::as_str)
            .is_some_and(|reference| {
                reference.contains("next: forge-core workflow resume --root")
                    && reference.contains(&app.display().to_string())
            }),
        "{state} should make workflow resume for the same root the first reference"
    );
}

fn assert_agent_native_resume_handoff(env: &Value, app: &Path, state: &str) {
    let root = app.display().to_string();
    assert_eq!(
        env["data"]["next_step"]["argv"],
        serde_json::json!(["forge-core", "workflow", "resume", "--root", root, "--json"]),
        "{state} should route existing workflow authority directly to resume"
    );
    assert!(
        env["data"]["next_step"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("forge-core workflow resume --root ")),
        "{state} should not repeat workflow init"
    );
}

#[test]
fn state_one_no_link_bootstraps_the_project_in_one_command() {
    // Scenario A: empty repo, no Project Link. `start` now bootstraps the
    // project (creates the Project Link + sidecar) in a single command, then
    // reports the post-init state. The agent does not need a separate
    // `project init` step.
    let parent = FreshParent::new("no-link");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok, "no_link bootstrap must exit zero");
    assert_eq!(env["ok"], true, "bootstrap envelope ok must be true");
    assert_eq!(env["exit_reason"], "ok", "bootstrap must report ok");
    assert_eq!(
        env["data"]["state"], "sidecar_ready_no_contract",
        "start should bootstrap and advance to sidecar_ready_no_contract"
    );
    assert!(
        env["data"].get("state_loss").is_none(),
        "clean bootstrap must not carry state-loss status"
    );
    assert_eq!(
        env["data"]["actions_performed"],
        serde_json::json!(["initialized"]),
        "start should report it initialized the project"
    );
    // Bootstrap actually created the Project Link.
    assert!(
        app.join(PROJECT_LINK_FILE_NAME).is_file(),
        "start should write a Project Link on no_link"
    );
}

#[test]
#[allow(clippy::too_many_lines)] // One start-to-resume journey keeps its ordered user-visible assertions together.
fn fresh_start_handoff_initializes_and_resumes_solo_profile() {
    let skill_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skill/start-forge/SKILL.md");
    let skill = fs::read_to_string(&skill_path).expect("read canonical Start Forge skill");
    for journey in [
        "greenfield",
        "brownfield_unmanaged",
        "brownfield_managed",
        "state_loss_or_integrity_failure",
        "runtime_or_bridge_unavailable",
        "human_decision_required",
        "autonomous_action_available",
    ] {
        assert!(
            skill.contains(&format!("| `{journey}` |")),
            "canonical skill must cover guided activation journey {journey}"
        );
    }
    for behavior in [
        "keep all explanatory prose consistently in that language",
        "Technical detail is welcome, but it must never be the whole explanation",
        "Orientation is a checkpoint, not a stopping point",
        "Ask exactly one concise question",
        "The `start` response is routing evidence, not project orientation",
        "This handoff is part of activation, not the governed project action",
        "<!-- uncertainty-driven-research:start -->",
        "Do not wait for the human to tell you to research.",
        "If it is consequential, research autonomously.",
        "compare competing hypotheses, contrary evidence, source freshness,",
        "Explain the result and its product impact in the human's language",
        "Continue with the next safe action when the result supports one.",
    ] {
        assert!(
            skill.contains(behavior),
            "canonical skill must preserve guided behavior: {behavior}"
        );
    }
    let parent = FreshParent::new("solo-workflow-handoff");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).expect("create app");
    fs::write(app.join("README.md"), "fresh project\n").expect("project evidence");

    let (start_ok, start) = run_start(&app);
    assert!(start_ok);
    assert_agent_native_init_handoff(&start, &app, STATE_SIDECAR_READY);

    let init_output = bin()
        .args(["workflow", "init", "--root"])
        .arg(&app)
        .arg("--json")
        .output()
        .expect("run workflow init handoff");
    assert!(init_output.status.success());
    let initialized: Value =
        serde_json::from_slice(&init_output.stdout).expect("parse initialization envelope");
    assert_eq!(initialized["data"]["readiness_profile"], "solo_cooperative");

    let (restart_ok, restart) = run_start(&app);
    assert!(restart_ok);
    assert_agent_native_resume_handoff(&restart, &app, STATE_SIDECAR_READY);
    let resume_output = bin()
        .args(["workflow", "resume", "--root"])
        .arg(&app)
        .arg("--json")
        .output()
        .expect("run direct resume handoff");
    assert!(resume_output.status.success());
    let resumed: Value =
        serde_json::from_slice(&resume_output.stdout).expect("parse resume envelope");
    assert_eq!(
        resumed["data"]["schema_version"],
        "workflow_resume_summary_v10"
    );
    assert_eq!(resumed["data"]["readiness_profile"], "solo_cooperative");
    assert_eq!(
        resumed["data"]["current_work"]["schema_version"],
        "current_work_context_v3"
    );
    assert_eq!(resumed["data"]["current_work"]["status"], "absent");
    assert!(resumed["data"]["current_work"]["focus"].is_null());

    // Exercise the public nested command and its canonical failure envelope,
    // not only the in-process adapter. With no accepted focus, detail must be
    // read-only and fail honestly instead of inventing work.
    let detail_output = bin()
        .args(["workflow", "current-work", "detail", "--root"])
        .arg(&app)
        .arg("--expected-head-digest")
        .arg(
            resumed["data"]["ledger_head_digest"]
                .as_str()
                .expect("resume ledger head"),
        )
        .arg("--json")
        .output()
        .expect("run Current Work detail through the public binary");
    assert!(!detail_output.status.success());
    let detail: Value =
        serde_json::from_slice(&detail_output.stdout).expect("parse Current Work detail envelope");
    assert_eq!(detail["command"], "workflow.current_work_detail");
    assert_eq!(detail["ok"], false);
    assert_eq!(detail["exit_reason"], "rejected_by_gate");
    assert!(detail["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("no accepted Work Focus")));
    assert_eq!(
        resumed["data"]["journey_guidance"]["schema_version"],
        "product_journey_guidance_v2"
    );
    assert_eq!(
        resumed["data"]["journey_guidance"]["stage"]["id"],
        "analysis-discovery"
    );
    assert_eq!(
        resumed["data"]["journey_guidance"]["catalog"]["status_argv"][0],
        "forge-core"
    );
    let consultation = &resumed["data"]["journey_guidance"]["catalog"]["consultation"];
    assert_eq!(consultation["schema_version"], "catalog_consultation_v1");
    assert_eq!(consultation["host_action"], "consult_once_when_unseen");
    assert!(consultation["key"]
        .as_str()
        .is_some_and(|key| key.starts_with("sha256:") && key.len() == 71));
    assert_eq!(
        consultation["recheck_events"],
        serde_json::json!([
            "material_human_redirect",
            "validation_reveals_misunderstanding"
        ])
    );

    // Dogfood the progressive handoff exactly as a replacement agent does:
    // resume -> compact status -> one selected capability detail. A malformed
    // catalog in the caller's CWD must not replace the embedded catalog named
    // by the resume guidance.
    let decoy = parent.path.join("unrelated-cwd");
    fs::create_dir_all(decoy.join("contracts/workflows")).expect("create decoy catalog");
    fs::write(
        decoy.join("contracts/workflows/not-a-workflow.yaml"),
        "not: a valid workflow\n",
    )
    .expect("write decoy catalog");
    let status_argv = resumed["data"]["journey_guidance"]["catalog"]["status_argv"]
        .as_array()
        .expect("published status argv")
        .iter()
        .map(|value| value.as_str().expect("string argv").to_owned())
        .collect::<Vec<_>>();
    let status_output = bin()
        .current_dir(&decoy)
        .args(&status_argv[1..])
        .output()
        .expect("execute published catalog status argv");
    assert!(status_output.status.success());
    let status: Value = serde_json::from_slice(&status_output.stdout).expect("parse status");
    let selected_id = status["data"]["eligible_workflows"][0]["id"]
        .as_str()
        .expect("eligible capability id");

    let detail_contract = &resumed["data"]["journey_guidance"]["catalog"]["detail_argv"];
    let token = detail_contract["workflow_id_token"]
        .as_str()
        .expect("workflow token");
    let detail_argv = detail_contract["argv"]
        .as_array()
        .expect("published detail argv")
        .iter()
        .map(|value| {
            let value = value.as_str().expect("string argv");
            if value == token {
                selected_id.to_owned()
            } else {
                value.to_owned()
            }
        })
        .collect::<Vec<_>>();
    let detail_output = bin()
        .current_dir(&decoy)
        .args(&detail_argv[1..])
        .output()
        .expect("execute published catalog detail argv");
    assert!(detail_output.status.success());
    let detail: Value = serde_json::from_slice(&detail_output.stdout).expect("parse detail");
    assert_eq!(detail["data"]["id"], selected_id);

    let next_output = bin()
        .args(["workflow", "next", "--root"])
        .arg(&app)
        .arg("--json")
        .output()
        .expect("run workflow next handoff");
    assert!(next_output.status.success());
    let next: Value = serde_json::from_slice(&next_output.stdout).expect("parse next envelope");
    assert_eq!(next["data"]["readiness_profile"], "solo_cooperative");
    assert_eq!(
        next["data"]["durable_assurance"]["status"],
        "missing_objective"
    );
    assert_eq!(
        next["data"]["authorization"]["action_packets"][0]["required_authority"]
            ["approval_boundary"],
        "cooperative_same_owner"
    );
    assert!(next["data"]["authorization"]["setup_gaps"]
        .as_array()
        .is_some_and(Vec::is_empty));

    for _ in 0..3 {
        let envelope = run_workflow_json(&app, "resume");
        assert_eq!(
            envelope["data"]["readiness_profile"], "solo_cooperative",
            "repeated workflow resume processes must retain the solo profile"
        );
    }

    let state_root = resolved_state_root(&app);
    assert!(
        crash_replace_residue_paths(&state_root).is_empty(),
        "real init/next/resume processes must not create crash-replace residue"
    );
    for relative in [
        "domain-packs/active.lock.yaml",
        "domain-packs/rebase-plan.yaml",
    ] {
        assert!(
            !state_root.join(relative).exists(),
            "read-only solo workflow must leave {relative} absent in the actual state root"
        );
    }
}

#[test]
fn state_one_no_link_bootstraps_project_with_space_in_path() {
    // Space-in-path must not break bootstrap. The link is created at the raw
    // path (no shell quoting); agents read `actions_performed` and `state`.
    let parent = FreshParent::new("no-link path");
    let app = parent.path.join("app with spaces");
    fs::create_dir_all(&app).unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok, "no_link with a space path should still exit zero");
    assert_eq!(
        env["data"]["state"], "sidecar_ready_no_contract",
        "no_link with space path should bootstrap and advance"
    );
    assert_eq!(
        env["data"]["actions_performed"],
        serde_json::json!(["initialized"]),
        "no_link with space path should report it initialized"
    );
    assert!(
        app.join(PROJECT_LINK_FILE_NAME).is_file(),
        "start should write a Project Link even with a space in the path"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn state_two_link_without_sidecar_fails_closed_without_mutation() {
    let parent = FreshParent::new("no-sidecar");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    let operator_root = parent.path.join("operator-anchors");
    fs::create_dir_all(&operator_root).unwrap();
    fs::write(operator_root.join("anchor.json"), b"{\"generation\":7}\n").unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok, "linked missing state must fail closed");
    assert_eq!(env["ok"], false);
    assert_eq!(env["exit_reason"], "env_config");
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert_eq!(env["data"]["project"]["project_id"], "app");
    assert_eq!(
        env["data"]["state_loss"]["kind"],
        "linked_state_unavailable"
    );
    assert_eq!(env["data"]["state_loss"]["cause"], "missing_sidecar");
    assert_eq!(env["data"]["state_loss"]["project_id"], "app");
    assert_eq!(
        env["data"]["state_loss"]["project_link_schema_version"],
        PROJECT_LINK_SCHEMA_VERSION
    );
    let link_digest = env["data"]["state_loss"]["project_link_sha256"]
        .as_str()
        .expect("valid Project Link has an exact byte digest");
    assert_eq!(link_digest.len(), 64);
    assert!(link_digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(
        env["data"]["state_loss"]["workflow_release_status"],
        "unavailable_untrusted_state"
    );
    assert!(env["data"]["state_loss"]["workflow_release_id"].is_null());
    assert_eq!(
        env["data"]["state_loss"]["schema_version"],
        "forge_bootstrap_state_loss_v1"
    );
    let diagnosis_digest = env["data"]["state_loss"]["diagnosis_digest"]
        .as_str()
        .expect("state-loss diagnosis has a deterministic digest");
    assert_eq!(diagnosis_digest.len(), 64);
    let choices = &env["data"]["state_loss"]["choices"];
    assert_eq!(choices["inspect"]["availability"], "available_read_only");
    assert_eq!(choices["inspect"]["mutates_authority"], false);
    assert_eq!(
        choices["inspect"]["argv"],
        serde_json::json!([
            "forge-core",
            "project",
            "resolve",
            "--root",
            app.display().to_string(),
            "--json"
        ])
    );
    assert_eq!(
        choices["restore_verified_backup"]["authority_effect"],
        "restores_prior_authority"
    );
    assert_eq!(
        choices["restore_verified_backup"]["availability"],
        "deferred_pending_verified_restore"
    );
    assert_eq!(
        choices["restore_verified_backup"]["automatic_allowed"],
        false
    );
    assert!(
        choices["restore_verified_backup"].get("argv").is_none(),
        "deferred restore must not publish executable argv"
    );
    assert_eq!(
        choices["reinitialize_as_new"]["authority_effect"],
        "abandons_prior_authority_and_creates_new"
    );
    assert_eq!(
        choices["reinitialize_as_new"]["availability"],
        "deferred_pending_reinitialize_plan"
    );
    assert_eq!(choices["reinitialize_as_new"]["automatic_allowed"], false);
    assert_eq!(
        choices["reinitialize_as_new"]["operator_confirmation_required"],
        true
    );
    assert!(choices["reinitialize_as_new"]["requirements"]
        .as_array()
        .is_some_and(|requirements| requirements
            .iter()
            .any(|requirement| requirement == "new_project_identity_distinct_from_prior")));
    assert!(
        choices["reinitialize_as_new"].get("argv").is_none(),
        "deferred reinitialize-as-new must not publish executable argv"
    );
    let state_loss_keys = env["data"]["state_loss"]
        .as_object()
        .expect("state_loss is an object")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        state_loss_keys
            .iter()
            .all(|key| !key.contains("path") && !key.contains("root") && !key.contains("secret")),
        "typed state-loss identity must not expose secret paths: {state_loss_keys:?}"
    );
    assert!(
        env["data"].get("actions_performed").is_none(),
        "state-loss rejection must report no mutation actions"
    );
    assert!(
        env["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("possible durable-state loss")
                && message.contains("Automatic recreation is forbidden")),
        "state loss must have a distinct actionable diagnostic"
    );
    assert_eq!(
        tree_snapshot(&parent.path),
        before,
        "link, project, operator roots, and sidecar namespace must remain byte-identical"
    );
}

#[test]
fn state_loss_inspection_argv_uses_canonical_root_for_relative_input() {
    let parent = FreshParent::new("relative-root");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let output = bin()
        .current_dir(&parent.path)
        .args(["start", "--root", "app", "--json"])
        .output()
        .expect("run start with relative root");
    let env: Value = serde_json::from_slice(&output.stdout).expect("parse start envelope");

    assert!(!output.status.success());
    assert_eq!(
        env["data"]["state_loss"]["choices"]["inspect"]["argv"],
        serde_json::json!([
            "forge-core",
            "project",
            "resolve",
            "--root",
            app.canonicalize().unwrap().display().to_string(),
            "--json"
        ])
    );
}
#[test]
fn healthy_handoff_argv_uses_canonical_root_for_relative_input() {
    let parent = FreshParent::new("relative-healthy-root");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let output = bin()
        .current_dir(&parent.path)
        .args(["start", "--root", "app", "--json"])
        .output()
        .expect("run healthy start with relative root");
    let env: Value = serde_json::from_slice(&output.stdout).expect("parse start envelope");

    assert!(output.status.success());
    assert_eq!(
        env["data"]["next_step"]["argv"],
        serde_json::json!([
            "forge-core",
            "workflow",
            "init",
            "--root",
            app.canonicalize().unwrap().display().to_string()
        ])
    );
}
#[test]
fn repeated_state_loss_diagnosis_is_stable_and_never_becomes_authorization() {
    let parent = FreshParent::new("repeated-state-loss");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);

    let (first_ok, first) = run_start(&app);
    let (second_ok, second) = run_start(&app);

    assert!(!first_ok && !second_ok);
    assert_eq!(
        first["data"]["state_loss"], second["data"]["state_loss"],
        "replacement processes must receive the same pending choices for the same observation"
    );
    assert_eq!(tree_snapshot(&parent.path), before);
}

#[test]
fn stale_state_loss_diagnosis_disappears_when_linked_authority_reappears() {
    let parent = FreshParent::new("stale-state-loss");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let (lost_ok, lost) = run_start(&app);
    assert!(!lost_ok);
    assert!(lost["data"]["state_loss"]["diagnosis_digest"].is_string());

    make_state_tree(&state);
    let restored_before = tree_snapshot(&parent.path);
    let (healthy_ok, healthy) = run_start(&app);

    assert!(healthy_ok);
    assert_eq!(healthy["data"]["state"], STATE_SIDECAR_READY);
    assert!(
        healthy["data"].get("state_loss").is_none(),
        "a stale state-loss diagnosis must not survive a fresh healthy observation"
    );
    assert_eq!(tree_snapshot(&parent.path), restored_before);
}

#[test]
fn reinitialize_like_start_flags_are_rejected_without_mutation() {
    let parent = FreshParent::new("reinitialize-flags");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);

    let output = bin()
        .args(["start", "--root"])
        .arg(&app)
        .args([
            "--json",
            "--reinitialize-as-new",
            "--new-project-id",
            "replacement",
        ])
        .output()
        .expect("run start with forbidden reinitialize-like flags");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("unrecognized argument '--reinitialize-as-new'"));
    assert_eq!(tree_snapshot(&parent.path), before);
}

#[test]
fn state_two_human_output_names_state_loss_and_forbidden_recreation() {
    let parent = FreshParent::new("no-sidecar-text");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let output = run_start_text(&app);

    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("possible durable-state loss"));
    assert!(stderr.contains("Automatic recreation is forbidden"));
}

#[test]
fn linked_empty_and_partial_state_fail_closed_without_normalization() {
    for label in ["empty", "partial"] {
        let parent = FreshParent::new(label);
        let app = parent.path.join("app");
        let state = parent.path.join("forge-app").join(".forge-method");
        fs::create_dir_all(&app).unwrap();
        if label == "empty" {
            fs::create_dir_all(&state).unwrap();
        } else {
            make_state_tree(&state);
            fs::remove_dir(state.join("evidence")).unwrap();
        }
        write_link(&app, "../forge-app", "../forge-app/.forge-method");
        let before = tree_snapshot(&parent.path);

        let (exit_ok, env) = run_start(&app);

        assert!(!exit_ok, "{label} linked state must fail closed");
        assert_eq!(env["data"]["state"], "link_present_no_sidecar");
        assert_eq!(env["data"]["state_loss"]["cause"], "incomplete_state");
        assert_eq!(tree_snapshot(&parent.path), before);
    }
}

#[cfg(unix)]
#[test]
fn linked_sidecar_symlink_substitution_fails_closed_without_mutation() {
    use std::os::unix::fs::symlink;

    let parent = FreshParent::new("sidecar-symlink");
    let app = parent.path.join("app");
    let foreign_sidecar = parent.path.join("foreign-sidecar");
    make_state_tree(&foreign_sidecar.join(".forge-method"));
    fs::create_dir_all(&app).unwrap();
    symlink(&foreign_sidecar, parent.path.join("forge-app")).unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok);
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert!(env["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("symbolic link")));
    assert_eq!(env["data"]["state_loss"]["cause"], "symlink_substitution");
    assert_eq!(tree_snapshot(&parent.path), before);
}
#[cfg(unix)]
#[test]
fn linked_ancestor_symlink_substitution_fails_closed_without_mutation() {
    use std::os::unix::fs::symlink;

    let parent = FreshParent::new("ancestor-symlink");
    let app = parent.path.join("app");
    let real_sidecar = parent.path.join("real").join("forge-app");
    make_state_tree(&real_sidecar.join(".forge-method"));
    fs::create_dir_all(&app).unwrap();
    symlink(parent.path.join("real"), parent.path.join("alias")).unwrap();
    write_link(
        &app,
        "../alias/forge-app",
        "../alias/forge-app/.forge-method",
    );
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok);
    assert_eq!(env["data"]["state_loss"]["cause"], "symlink_substitution");
    assert_eq!(tree_snapshot(&parent.path), before);
}

#[cfg(unix)]
#[test]
fn linked_ledger_symlink_substitution_fails_closed_without_mutation() {
    use std::os::unix::fs::symlink;

    let parent = FreshParent::new("ledger-symlink");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    make_state_tree(&state);
    fs::create_dir_all(&app).unwrap();
    fs::remove_file(state.join("ledger.ndjson")).unwrap();
    fs::write(parent.path.join("foreign-ledger.ndjson"), b"").unwrap();
    symlink(
        parent.path.join("foreign-ledger.ndjson"),
        state.join("ledger.ndjson"),
    )
    .unwrap();
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok);
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert!(env["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("symbolic link")));
    assert_eq!(env["data"]["state_loss"]["cause"], "symlink_substitution");
    assert_eq!(tree_snapshot(&parent.path), before);
}
#[cfg(unix)]
#[test]
fn linked_permission_denial_fails_closed_without_normalization() {
    use std::os::unix::fs::PermissionsExt;

    let parent = FreshParent::new("permission-denied");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    let ledger = state.join("ledger.ndjson");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    let before = tree_snapshot(&parent.path);
    fs::set_permissions(&ledger, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::File::open(&ledger).is_ok() {
        // Elevated principals can bypass mode bits, so no denial exists to test.
        fs::set_permissions(&ledger, fs::Permissions::from_mode(0o644)).unwrap();
        return;
    }

    let (exit_ok, env) = run_start(&app);

    fs::set_permissions(&ledger, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!exit_ok);
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert_eq!(env["data"]["state_loss"]["cause"], "permission_denied");
    assert_eq!(tree_snapshot(&parent.path), before);
}

#[test]
fn malformed_link_corruption_is_distinct_from_state_loss() {
    let parent = FreshParent::new("malformed-link");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    fs::write(app.join(PROJECT_LINK_FILE_NAME), "schema_version: [\n").unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(!exit_ok);
    assert_eq!(env["ok"], false);
    assert!(
        env.get("data").is_none(),
        "corruption has no state-loss data"
    );
    assert!(!env["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("possible durable-state loss"));
}
#[test]
fn human_output_distinguishes_corruption_and_clean_bootstrap() {
    let clean_parent = FreshParent::new("clean-text");
    let clean_app = clean_parent.path.join("app");
    fs::create_dir_all(&clean_app).unwrap();
    let clean = run_start_text(&clean_app);
    assert!(clean.status.success());
    assert_eq!(String::from_utf8_lossy(&clean.stdout).trim(), "start: ok");
    assert!(clean.stderr.is_empty());

    let corrupt_parent = FreshParent::new("corrupt-text");
    let corrupt_app = corrupt_parent.path.join("app");
    fs::create_dir_all(&corrupt_app).unwrap();
    fs::write(
        corrupt_app.join(PROJECT_LINK_FILE_NAME),
        "schema_version: [\n",
    )
    .unwrap();
    let corrupt = run_start_text(&corrupt_app);
    assert!(!corrupt.status.success());
    assert!(corrupt.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&corrupt.stderr);
    assert!(stderr.contains("failed"));
    assert!(!stderr.contains("possible durable-state loss"));
}

#[test]
fn state_two_cross_project_link_fails_closed_not_silent_overwrite() {
    let parent = FreshParent::new("no-sidecar-cross-project");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();
    fs::write(
        app.join(PROJECT_LINK_FILE_NAME),
        "schema_version: forge_project_link_v1\n\
         project_id: other-project\n\
         sidecar_root: ../forge-other-project\n\
         state_root: ../forge-other-project/.forge-method\n",
    )
    .unwrap();
    let before = tree_snapshot(&parent.path);

    let (exit_ok, env) = run_start(&app);

    assert!(
        !exit_ok,
        "cross-project link plus missing state must fail closed"
    );
    assert_eq!(env["ok"], false);
    assert_eq!(env["exit_reason"], "env_config");
    assert_eq!(env["data"]["state"], "link_present_no_sidecar");
    assert_eq!(env["data"]["state_loss"]["project_id"], "other-project");
    assert_eq!(tree_snapshot(&parent.path), before);
}

#[test]
fn state_three_sidecar_ready_points_at_starter_fixtures() {
    // Scenario C: healthy state tree, no operation contract, no preview.
    let parent = FreshParent::new("ready-no-contract");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok);
    assert_eq!(env["data"]["state"], STATE_SIDECAR_READY);
    assert_agent_native_init_handoff(&env, &app, "state 3");
    let refs = env["data"]["next_step"]["references"]
        .as_array()
        .expect("state 3 references is an array");
    let refs_joined = refs
        .iter()
        .map(|v| v.as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        refs_joined.contains("observe-project-status.yaml"),
        "state 3 should name the observe starter fixture"
    );
    assert!(
        refs_joined.contains("execute-trivial-write.yaml"),
        "state 3 should name the execute starter fixture"
    );
    assert!(
        refs_joined.contains("preview --operation"),
        "state 3 should point at the validation command"
    );
}

#[test]
fn state_four_contract_present_hands_off_to_workflow() {
    // Scenario D: state tree + an operation-contract-looking file.
    let parent = FreshParent::new("with-contract");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    fs::write(app.join("my-operation.yaml"), "operation_contract: {}\n").unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok);
    assert_eq!(env["data"]["state"], STATE_CONTRACT_PRESENT);
    assert_agent_native_init_handoff(&env, &app, "state 4");
    let refs = env["data"]["next_step"]["references"]
        .as_array()
        .expect("state 4 references is an array");
    let refs_joined = refs
        .iter()
        .map(|v| v.as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        refs_joined.contains("compatibility: forge-core preview --operation"),
        "state 4 should retain legacy operation validation only as compatibility context"
    );
}

#[test]
fn state_five_preview_run_keeps_workflow_authority() {
    // Scenario E: state tree + non-empty traces dir => preview has run.
    let parent = FreshParent::new("preview-run");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");
    // Simulate a trace having been written.
    fs::write(state.join("traces").join("m1.jsonl"), "{}\n").unwrap();

    let (exit_ok, env) = run_start(&app);

    assert!(exit_ok);
    assert_eq!(env["data"]["state"], STATE_PREVIEW_RUN);
    assert_agent_native_init_handoff(&env, &app, "state 5");
    let refs = env["data"]["next_step"]["references"]
        .as_array()
        .expect("state 5 references is an array");
    assert!(
        refs.iter().filter_map(Value::as_str).any(|reference| {
            reference.contains("preview trace") && reference.contains("not workflow authority")
        }),
        "state 5 should retain preview evidence only as compatibility material"
    );
}

#[test]
fn clean_bootstrap_second_start_is_idempotent_and_nonmutating() {
    let parent = FreshParent::new("clean-bootstrap-twice");
    let app = parent.path.join("app");
    fs::create_dir_all(&app).unwrap();

    let (first_ok, first) = run_start(&app);
    let after_bootstrap = tree_snapshot(&parent.path);
    let (second_ok, second) = run_start(&app);

    assert!(first_ok && second_ok);
    assert_eq!(first["data"]["state"], second["data"]["state"]);
    assert_eq!(
        first["data"]["actions_performed"],
        serde_json::json!(["initialized"])
    );
    assert!(second["data"].get("actions_performed").is_none());
    assert!(second["data"].get("state_loss").is_none());
    assert_eq!(tree_snapshot(&parent.path), after_bootstrap);
}

#[test]
fn start_is_idempotent_running_twice_keeps_same_state() {
    // Read-only invariant: running start twice on the same repo must not
    // advance or regress the state, and must not create any files.
    let parent = FreshParent::new("idempotent");
    let app = parent.path.join("app");
    let state = parent.path.join("forge-app").join(".forge-method");
    fs::create_dir_all(&app).unwrap();
    make_state_tree(&state);
    write_link(&app, "../forge-app", "../forge-app/.forge-method");

    let (_, first) = run_start(&app);
    let (_, second) = run_start(&app);
    assert_eq!(
        first["data"]["state"], second["data"]["state"],
        "idempotent: state must not change across two runs"
    );
    // Nothing was created in the app dir beyond what the test wrote.
    let app_entries = fs::read_dir(&app).unwrap().count();
    assert_eq!(
        app_entries, 1,
        "idempotent: app dir should still contain only the Project Link"
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[allow(clippy::too_many_lines)]
fn reinitialize_resumes_reserved_wal_in_a_fresh_process() {
    use std::os::unix::fs::MetadataExt as _;

    let parent = FreshParent::new("reinitialize-reserved-retry");
    let app = parent.path.join("app");
    let destination = parent.path.join("successor-sidecar");
    let diagnosis = parent.path.join("state-loss.yaml");
    let plan_file = parent.path.join("reinitialize-plan.json");
    fs::create_dir_all(&app).expect("create app root");
    write_link(
        &app,
        "../missing-sidecar",
        "../missing-sidecar/.forge-method",
    );
    let predecessor_link =
        fs::read(app.join(PROJECT_LINK_FILE_NAME)).expect("read predecessor link");

    let (start_ok, start) = run_start(&app);
    assert!(
        !start_ok,
        "missing sidecar must produce a state-loss diagnosis"
    );
    assert_eq!(start["data"]["state"], "link_present_no_sidecar");
    let mut diagnosis_bytes = yaml_serde::to_string(&start["data"]["state_loss"])
        .expect("serialize public state-loss diagnosis")
        .into_bytes();
    if !diagnosis_bytes.ends_with(b"\n") {
        diagnosis_bytes.push(b'\n');
    }
    fs::write(&diagnosis, &diagnosis_bytes).expect("write exact state-loss diagnosis");

    let planned = run_reinitialize(
        "plan",
        &app,
        &destination,
        &diagnosis,
        &plan_file,
        None,
        None,
    );
    assert!(
        planned.status.success(),
        "public reinitialize plan failed: stdout={} stderr={}",
        String::from_utf8_lossy(&planned.stdout),
        String::from_utf8_lossy(&planned.stderr)
    );
    let planned = output_envelope(&planned, "reinitialize plan");
    assert_eq!(planned["data"]["selected_host"], Value::Null);
    let operation_id = planned["data"]["operation_id"]
        .as_str()
        .expect("plan operation id");
    let plan_digest = planned["data"]["plan_digest"]
        .as_str()
        .expect("plan digest");
    let confirmation = planned["data"]["confirmation_token"]
        .as_str()
        .expect("plan confirmation token");
    let sealed_plan: Value =
        serde_json::from_slice(&fs::read(&plan_file).expect("read sealed reinitialize plan"))
            .expect("parse sealed reinitialize plan");
    assert_eq!(sealed_plan["operation_id"], operation_id);
    assert_eq!(sealed_plan["plan_digest"], plan_digest);
    assert_eq!(sealed_plan["confirmation_token"], confirmation);
    assert_eq!(sealed_plan["selected_host"], Value::Null);

    let mut changed_diagnosis = diagnosis_bytes.clone();
    changed_diagnosis.extend_from_slice(b"# changed after planning\n");
    fs::write(&diagnosis, changed_diagnosis).expect("change diagnosis after planning");
    let interrupted = run_reinitialize(
        "apply",
        &app,
        &destination,
        &diagnosis,
        &plan_file,
        Some(plan_digest),
        Some(confirmation),
    );
    assert!(
        !interrupted.status.success(),
        "changed diagnosis must interrupt apply"
    );
    let interrupted = output_envelope(&interrupted, "interrupted reinitialize apply");
    assert_eq!(interrupted["ok"], false);
    assert!(
        interrupted["error"]["message"].as_str().is_some_and(
            |message| message.contains("state-loss diagnosis bytes changed after planning")
        ),
        "unexpected interrupted reinitialize envelope: {interrupted:#}"
    );
    assert_eq!(
        fs::read(app.join(PROJECT_LINK_FILE_NAME)).expect("read predecessor after interruption"),
        predecessor_link
    );

    let durable_root = destination
        .join(".forge-method")
        .join("locks/project-reinitialize")
        .join(operation_id);
    let reserved_path = durable_root.join("wal-reserved.json");
    assert!(durable_root.join("plan.json").is_file());
    assert!(reserved_path.is_file());
    for absent in [
        "wal-applyprepared.json",
        "wal-linkinstalled.json",
        "wal-receiptpublished.json",
        "receipt.json",
    ] {
        assert!(
            !durable_root.join(absent).exists(),
            "{absent} advanced before retry"
        );
    }
    let reserved_bytes = fs::read(&reserved_path).expect("read durable Reserved WAL");
    let reserved: Value =
        serde_json::from_slice(&reserved_bytes).expect("parse durable Reserved WAL");
    assert_eq!(reserved["phase"], "reserved");
    assert_eq!(reserved["operation_id"], operation_id);
    assert_eq!(reserved["plan_digest"], plan_digest);
    assert_eq!(reserved["predecessor_identity"], "abandoned-authority");
    assert_eq!(reserved["successor_project_id"], "successor-project");
    assert_eq!(reserved["successor_identity"], "successor-authority");
    assert_eq!(
        reserved["project_link"],
        sealed_plan["expected_project_link"]
    );
    assert_eq!(
        reserved["project_link_anchor"]["content_digest"],
        sealed_plan["expected_project_link"]["sha256"]
    );
    assert_eq!(
        reserved["project_link_anchor"]["byte_length"],
        sealed_plan["expected_project_link"]["byte_length"]
    );
    assert_eq!(
        reserved["destination_reservation"]["path"],
        destination.display().to_string()
    );
    assert_eq!(
        reserved["destination_reservation"]["anchor_nonce"],
        operation_id
    );
    assert!(reserved["destination_reservation"]["sha256"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:") && digest.len() == 71));
    let reservation_marker: Value = serde_json::from_slice(
        &fs::read(destination.join(".forge-reinitialize-reservation.json"))
            .expect("read destination reservation marker"),
    )
    .expect("parse destination reservation marker");
    assert_eq!(reservation_marker["operation_id"], operation_id);
    assert_eq!(reservation_marker["plan_digest"], plan_digest);
    assert_eq!(
        reservation_marker["destination"],
        destination.display().to_string()
    );
    let anchor_relative_path = reserved["project_link_anchor"]["anchor_relative_path"]
        .as_str()
        .expect("private predecessor anchor path");
    assert!(destination
        .join(".forge-method")
        .join(anchor_relative_path)
        .is_file());

    fs::write(&diagnosis, &diagnosis_bytes).expect("restore exact diagnosis bytes");
    let completed = run_reinitialize(
        "apply",
        &app,
        &destination,
        &diagnosis,
        &plan_file,
        Some(plan_digest),
        Some(confirmation),
    );
    assert!(
        completed.status.success(),
        "fresh-process Reserved retry failed: stdout={} stderr={}",
        String::from_utf8_lossy(&completed.stdout),
        String::from_utf8_lossy(&completed.stderr)
    );
    let completed = output_envelope(&completed, "completed reinitialize apply");
    assert_eq!(completed["data"]["operation_id"], operation_id);
    assert_eq!(completed["data"]["plan_digest"], plan_digest);
    assert_eq!(completed["data"]["selected_host"], Value::Null);
    for present in [
        "wal-applyprepared.json",
        "wal-linkinstalled.json",
        "wal-receiptpublished.json",
        "receipt.json",
    ] {
        assert!(
            durable_root.join(present).is_file(),
            "{present} missing after retry"
        );
    }
    assert_eq!(
        fs::read(&reserved_path).expect("reread durable Reserved WAL"),
        reserved_bytes
    );
    let receipt_path = durable_root.join("receipt.json");
    let receipt_bytes = fs::read(&receipt_path).expect("read immutable reinitialize receipt");
    let receipt: Value =
        serde_json::from_slice(&receipt_bytes).expect("parse immutable reinitialize receipt");
    assert_eq!(receipt, completed["data"]);
    let successor_path = app.join(PROJECT_LINK_FILE_NAME);
    let successor_link = fs::read(&successor_path).expect("read successor link");
    let successor_metadata = fs::metadata(&successor_path).expect("inspect successor link");
    let successor_identity = (successor_metadata.dev(), successor_metadata.ino());
    assert_ne!(successor_link, predecessor_link);
    let completed_snapshot = tree_snapshot(&parent.path);

    let replayed = run_reinitialize(
        "apply",
        &app,
        &destination,
        &diagnosis,
        &plan_file,
        Some(plan_digest),
        Some(confirmation),
    );
    assert!(
        replayed.status.success(),
        "exact receipt replay must succeed"
    );
    let replayed = output_envelope(&replayed, "replayed reinitialize apply");
    assert_eq!(replayed["data"], completed["data"]);
    assert_eq!(
        fs::read(&successor_path).expect("read successor after receipt replay"),
        successor_link
    );
    assert_eq!(
        fs::read(&receipt_path).expect("reread immutable reinitialize receipt"),
        receipt_bytes
    );
    assert_eq!(
        fs::read(&reserved_path).expect("reread Reserved WAL after receipt replay"),
        reserved_bytes
    );
    let replayed_metadata = fs::metadata(&successor_path).expect("reinspect successor link");
    assert_eq!(
        (replayed_metadata.dev(), replayed_metadata.ino()),
        successor_identity,
        "exact receipt replay must not reinstall the successor Project Link"
    );
    assert_eq!(tree_snapshot(&parent.path), completed_snapshot);

    let resolved = bin()
        .args(["project", "resolve", "--root"])
        .arg(&app)
        .output()
        .expect("resolve reinitialized project");
    assert!(
        resolved.status.success(),
        "reinitialized project must resolve"
    );
    let resolved = output_envelope(&resolved, "project resolve after reinitialize");
    assert_eq!(resolved["data"]["project_id"], "successor-project");
    assert_eq!(resolved["data"]["state_exists"], true);
    assert_eq!(
        Path::new(
            resolved["data"]["state_root"]
                .as_str()
                .expect("resolved state root")
        ),
        destination.join(".forge-method")
    );
}
