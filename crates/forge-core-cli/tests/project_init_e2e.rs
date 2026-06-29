use assert_cmd::Command;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const NOW: i64 = 1_800_000_000;

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

struct FreshParent {
    path: PathBuf,
}

impl FreshParent {
    fn new(label: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let path = repo_root().join("target").join(format!(
            "project-init-e2e-{label}-{}-{n}",
            std::process::id()
        ));
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

fn create_existing_app(parent: &Path, name: &str) -> PathBuf {
    let app = parent.join(name);
    fs::create_dir_all(app.join("src")).expect("create app source directory");
    fs::write(app.join("README.md"), format!("# {name}\n")).expect("write app README");
    fs::write(app.join("src").join("main.rs"), "fn main() {}\n").expect("write app source");
    app
}

fn project_init(app: &Path) -> std::process::Output {
    bin()
        .args(["project", "init", "--root"])
        .arg(app)
        .arg("--json")
        .output()
        .expect("run project init")
}

fn output_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be json: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &std::process::Output, label: &str) -> Value {
    assert!(
        output.status.success(),
        "{label} should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], true, "{label} should report ok: {json:#}");
    json
}

fn assert_failure(output: &std::process::Output, label: &str) -> Value {
    assert!(
        !output.status.success(),
        "{label} should fail closed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], false, "{label} should report not ok: {json:#}");
    json
}

fn error_message(json: &Value) -> String {
    json["error"]["message"]
        .as_str()
        .expect("json error message")
        .to_string()
}

fn assert_message_mentions_all(json: &Value, terms: &[&str]) {
    let message = error_message(json);
    let lower = message.to_ascii_lowercase();
    for term in terms {
        assert!(
            lower.contains(&term.to_ascii_lowercase()),
            "error message should mention '{term}': {message}"
        );
    }
}

fn root_entry_names(root: &Path) -> BTreeSet<String> {
    fs::read_dir(root)
        .expect("read app root")
        .map(|entry| {
            entry
                .expect("read app root entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

fn expected_existing_app_entries() -> BTreeSet<String> {
    BTreeSet::from([
        ".forge-method.yaml".to_string(),
        "README.md".to_string(),
        "src".to_string(),
    ])
}

fn yaml_file_count(dir: &Path) -> usize {
    fs::read_dir(dir)
        .expect("read yaml dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "yaml"))
        .count()
}

fn assert_no_consumer_local_state(app: &Path) {
    assert!(
        !app.join(".forge-method").exists(),
        "consumer app must not contain local .forge-method state"
    );
}

#[test]
fn project_init_creates_project_link_and_sibling_sidecar_only() {
    let parent = FreshParent::new("default-sidecar");
    let app = create_existing_app(&parent.path, "darkest-roguelite");
    let sidecar_root = parent.path.join("forge-darkest-roguelite");
    let state_root = sidecar_root.join(".forge-method");

    let output = project_init(&app);

    let json = assert_success(&output, "project init with default sidecar");
    assert_eq!(json["command"], "project.init");
    assert!(app.join(".forge-method.yaml").is_file());
    assert!(
        state_root.is_dir(),
        "project init should create sibling sidecar state root"
    );
    assert_no_consumer_local_state(&app);
    assert_eq!(
        root_entry_names(&app),
        expected_existing_app_entries(),
        "project init should only add the project link inside the consumer app"
    );

    let resolve = bin()
        .args(["project", "resolve", "--root"])
        .arg(&app)
        .arg("--json")
        .output()
        .expect("run project resolve after init");
    let resolve_json = assert_success(&resolve, "project resolve after init");
    assert_eq!(resolve_json["command"], "project.resolve");
    assert_eq!(resolve_json["data"]["layout"], "sidecar");
    assert_eq!(resolve_json["data"]["state_exists"], true);
    assert!(
        Path::new(resolve_json["data"]["sidecar_root"].as_str().unwrap())
            .ends_with("forge-darkest-roguelite")
    );
    assert!(
        Path::new(resolve_json["data"]["state_root"].as_str().unwrap())
            .ends_with(Path::new("forge-darkest-roguelite").join(".forge-method"))
    );
    assert_eq!(
        Path::new(resolve_json["data"]["state_root"].as_str().unwrap()),
        state_root
    );
}

#[test]
fn claim_status_after_project_init_uses_sidecar_claim_bus() {
    let parent = FreshParent::new("claim-status");
    let app = create_existing_app(&parent.path, "darkest-roguelite");
    let claims_dir = parent
        .path
        .join("forge-darkest-roguelite")
        .join(".forge-method")
        .join("claims-active");

    let init = project_init(&app);
    assert_success(&init, "project init before claim status");

    let empty_status = bin()
        .args(["claim", "status", "--root"])
        .arg(&app)
        .args(["--now-unix", &NOW.to_string(), "--json"])
        .output()
        .expect("run claim status after init");
    let empty_status_json = assert_success(&empty_status, "claim status after init");
    assert_eq!(empty_status_json["command"], "claim.status");
    assert!(
        empty_status_json["data"]["active"]
            .as_array()
            .expect("active claims array")
            .is_empty(),
        "fresh sidecar claim bus should start empty"
    );
    assert_no_consumer_local_state(&app);

    let acquire = bin()
        .args(["claim", "acquire", "--root"])
        .arg(&app)
        .args([
            "--scope",
            "story",
            "--id",
            "project-init-sidecar",
            "--agent",
            "init-agent",
            "--path",
            "src/main.rs",
            "--now-unix",
            &NOW.to_string(),
            "--json",
        ])
        .output()
        .expect("run sidecar claim acquire");
    assert_success(&acquire, "sidecar claim acquire");
    assert_eq!(
        yaml_file_count(&claims_dir),
        1,
        "claim acquire should write to sidecar claims-active"
    );
    assert_no_consumer_local_state(&app);

    let status = bin()
        .args(["claim", "status", "--root"])
        .arg(&app)
        .args(["--now-unix", &(NOW + 1).to_string(), "--json"])
        .output()
        .expect("run claim status against sidecar bus");
    let status_json = assert_success(&status, "claim status against sidecar bus");
    let active = status_json["data"]["active"]
        .as_array()
        .expect("active claims array");
    assert!(
        active.iter().any(|claim| {
            claim["agent_id"] == "init-agent"
                && claim["paths"]
                    .as_array()
                    .is_some_and(|paths| paths.iter().any(|path| path == "src/main.rs"))
        }),
        "claim status should read the sidecar claim bus: {status_json:#}"
    );
    assert_no_consumer_local_state(&app);
}

#[test]
fn project_init_is_idempotent_for_same_root() {
    let parent = FreshParent::new("idempotent");
    let app = create_existing_app(&parent.path, "darkest-roguelite");

    let first = project_init(&app);
    assert_success(&first, "first project init");
    let link_path = app.join(".forge-method.yaml");
    let first_link = fs::read_to_string(&link_path).expect("read first project link");

    let second = project_init(&app);

    let second_json = assert_success(&second, "second project init");
    assert_eq!(second_json["command"], "project.init");
    assert_eq!(second_json["data"]["status"], "already_initialized");
    assert_eq!(
        fs::read_to_string(&link_path).expect("read project link after idempotent init"),
        first_link,
        "idempotent init should not rewrite the project link"
    );
    assert_no_consumer_local_state(&app);
}

#[test]
fn project_init_rejects_preexisting_consumer_local_state_without_creating_link() {
    let parent = FreshParent::new("preexisting-local-state");
    let app = create_existing_app(&parent.path, "darkest-roguelite");
    let local_state = app.join(".forge-method");
    fs::create_dir_all(local_state.join("wal")).expect("seed unsafe local Forge state");
    fs::write(
        local_state.join("wal").join("effects.ndjson"),
        "legacy
",
    )
    .expect("seed local state marker");

    let output = project_init(&app);

    let json = assert_failure(&output, "project init with preexisting local state");
    assert_message_mentions_all(&json, &["local", ".forge-method"]);
    assert!(
        !app.join(".forge-method.yaml").exists(),
        "failed init must not create a project link"
    );
    assert!(
        local_state.join("wal").join("effects.ndjson").is_file(),
        "failed init must preserve the unsafe local state for explicit migration/quarantine"
    );
    assert!(
        !parent.path.join("forge-darkest-roguelite").exists(),
        "failed init must not create a sidecar when local state needs migration first"
    );
}

#[test]
fn project_init_rejects_conflicting_existing_project_link_without_overwrite() {
    let parent = FreshParent::new("conflicting-link");
    let app = create_existing_app(&parent.path, "darkest-roguelite");
    let link_path = app.join(".forge-method.yaml");
    let conflicting_link = "schema_version: forge_project_link_v1\nproject_id: other-app\nsidecar_root: ../forge-other-app\nstate_root: ../forge-other-app/.forge-method\n";
    fs::write(&link_path, conflicting_link).expect("write conflicting project link");

    let output = project_init(&app);

    let json = assert_failure(&output, "project init with conflicting project link");
    assert_message_mentions_all(&json, &[".forge-method.yaml"]);
    assert_eq!(
        fs::read_to_string(&link_path).expect("read conflicting project link"),
        conflicting_link,
        "failed init must not overwrite an existing conflicting project link"
    );
    assert!(
        !parent.path.join("forge-darkest-roguelite").exists(),
        "failed init must not create the default sibling sidecar"
    );
    assert_no_consumer_local_state(&app);
}

#[test]
fn project_init_accepts_custom_external_sidecar_and_state_roots() {
    let parent = FreshParent::new("custom-roots");
    let app = create_existing_app(&parent.path, "darkest-roguelite");
    let sidecar_root = parent.path.join("custom-forge-runtime");
    let state_root = sidecar_root.join(".forge-method");

    let output = bin()
        .args(["project", "init", "--root"])
        .arg(&app)
        .args(["--sidecar-root"])
        .arg(&sidecar_root)
        .args(["--state-root"])
        .arg(&state_root)
        .arg("--json")
        .output()
        .expect("run project init with custom roots");

    let json = assert_success(&output, "project init with custom roots");
    assert_eq!(json["command"], "project.init");
    assert!(app.join(".forge-method.yaml").is_file());
    assert!(
        state_root.is_dir(),
        "custom project init should create the requested state root"
    );
    assert_no_consumer_local_state(&app);

    let resolve = bin()
        .args(["project", "resolve", "--root"])
        .arg(&app)
        .arg("--json")
        .output()
        .expect("run project resolve for custom roots");
    let resolve_json = assert_success(&resolve, "project resolve with custom roots");
    assert_eq!(
        Path::new(resolve_json["data"]["sidecar_root"].as_str().unwrap()),
        sidecar_root
    );
    assert_eq!(
        Path::new(resolve_json["data"]["state_root"].as_str().unwrap()),
        state_root
    );
    assert_eq!(resolve_json["data"]["layout"], "sidecar");
    assert_eq!(resolve_json["data"]["state_exists"], true);
}

#[test]
fn project_init_rejects_custom_state_root_without_dot_forge_method_leaf() {
    let parent = FreshParent::new("custom-state-leaf");
    let app = create_existing_app(&parent.path, "darkest-roguelite");
    let sidecar_root = parent.path.join("custom-forge-runtime");
    let invalid_state_root = sidecar_root.join("state");

    let output = bin()
        .args(["project", "init", "--root"])
        .arg(&app)
        .args(["--sidecar-root"])
        .arg(&sidecar_root)
        .args(["--state-root"])
        .arg(&invalid_state_root)
        .arg("--json")
        .output()
        .expect("run project init with invalid custom state root leaf");

    let json = assert_failure(&output, "project init with non-.forge-method state_root");
    assert_message_mentions_all(&json, &["state_root", ".forge-method"]);
    assert!(
        !app.join(".forge-method.yaml").exists(),
        "failed init must not create a project link"
    );
    assert!(
        !invalid_state_root.exists(),
        "failed init must not create invalid custom state root"
    );
    assert_no_consumer_local_state(&app);
}

#[test]
fn project_init_rejects_consumer_local_state_root_without_creating_local_state() {
    let parent = FreshParent::new("consumer-local-state");
    let app = create_existing_app(&parent.path, "darkest-roguelite");
    let local_state_root = app.join(".forge-method");

    let output = bin()
        .args(["project", "init", "--root"])
        .arg(&app)
        .args(["--sidecar-root"])
        .arg(&app)
        .args(["--state-root"])
        .arg(&local_state_root)
        .arg("--json")
        .output()
        .expect("run project init with consumer-local state_root");

    let json = assert_failure(&output, "project init with consumer-local state_root");
    assert_message_mentions_all(&json, &["state_root"]);
    let message = error_message(&json).to_ascii_lowercase();
    assert!(
        message.contains("consumer") || message.contains("local") || message.contains("sidecar"),
        "error should explain that consumer-local state is unsafe/actionable: {message}"
    );
    assert!(
        !app.join(".forge-method.yaml").exists(),
        "failed init must not create a project link"
    );
    assert_no_consumer_local_state(&app);
}

#[test]
fn project_init_missing_root_fails_clearly() {
    let parent = FreshParent::new("missing-root");
    let missing_app = parent.path.join("missing-app");

    let output = project_init(&missing_app);

    let json = assert_failure(&output, "project init with missing root");
    assert_message_mentions_all(&json, &["root"]);
    let message = error_message(&json).to_ascii_lowercase();
    assert!(
        message.contains("exist") || message.contains("not found"),
        "missing-root error should be clear/actionable: {message}"
    );
    assert!(
        !missing_app.exists(),
        "failed init must not create a missing project root"
    );
}
