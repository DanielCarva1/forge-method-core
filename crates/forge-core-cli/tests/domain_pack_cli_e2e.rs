use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use forge_core_decisions::MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES;
use sha2::{Digest, Sha256};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create fixture destination");
    for entry in fs::read_dir(source).expect("read fixture source") {
        let entry = entry.expect("fixture entry");
        let destination = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).expect("copy fixture");
        }
    }
}

fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, current: &Path, output: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(current).expect("read snapshot tree") {
            let entry = entry.expect("snapshot entry");
            if entry.path().is_dir() {
                walk(root, &entry.path(), output);
            } else {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("relative fixture")
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = fs::read(entry.path()).expect("snapshot bytes");
                output.insert(relative, format!("{:x}", Sha256::digest(bytes)));
            }
        }
    }
    let mut output = BTreeMap::new();
    walk(root, root, &mut output);
    output
}

fn fresh_temp(label: &str) -> PathBuf {
    let temp = std::env::temp_dir().join(format!(
        "forge-domain-pack-cli-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("clean stale temp");
    }
    fs::create_dir_all(&temp).expect("create temp root");
    temp
}

fn write_oversized(path: &Path) {
    let file = fs::File::create(path).expect("create oversized input");
    file.set_len((MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES as u64) + 1)
        .expect("size oversized input");
}

fn assert_too_large(temp: &Path, args: &[&str], label: &str) {
    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(temp)
        .args(args)
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(
        stderr.contains(label) && stderr.contains("exceeds maximum"),
        "unexpected bounded-read error: {stderr}"
    );
}

fn create_dir_link(link: &Path, target: &Path) {
    #[cfg(windows)]
    {
        // PowerShell binds paths as parameters instead of reparsing one
        // `cmd /C` command string, which keeps nested temporary paths intact.
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "New-Item -ItemType Junction -Path $env:FORGE_TEST_LINK -Target $env:FORGE_TEST_TARGET | Out-Null",
            ])
            .env("FORGE_TEST_LINK", link)
            .env("FORGE_TEST_TARGET", target)
            .output()
            .expect("create Windows directory junction");
        assert!(
            output.status.success(),
            "junction creation failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).expect("create Unix directory symlink");

    #[cfg(not(any(windows, unix)))]
    panic!("directory-link escape tests require Windows junctions or Unix symlinks");
}

#[test]
fn agent_cli_validates_and_composes_without_writes() {
    let temp = std::env::temp_dir().join(format!(
        "forge-domain-pack-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );
    let before = snapshot(&temp);

    let validate = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "validate",
            "--manifest-file",
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "--content-file",
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "--artifact-root",
            ".",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let validate_json: serde_json::Value =
        serde_json::from_slice(&validate).expect("validate JSON");
    assert_eq!(validate_json["ok"], true);
    assert_eq!(validate_json["data"]["structurally_valid"], true);
    assert_eq!(validate_json["data"]["authority"], "candidate_only");

    let compose = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "compose",
            "--request-file",
            "docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml",
            "--artifact-root",
            ".",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let compose_json: serde_json::Value = serde_json::from_slice(&compose).expect("compose JSON");
    assert_eq!(compose_json["ok"], true);
    assert_eq!(
        compose_json["data"]["domain_pack_composition_projection"]["status"],
        "composable"
    );
    assert_eq!(
        compose_json["data"]["domain_pack_composition_projection"]["authority"],
        "candidate_only"
    );
    assert_eq!(
        snapshot(&temp),
        before,
        "read-only CLI changed fixture tree"
    );

    fs::remove_dir_all(temp).expect("cleanup CLI fixture");
}

#[test]
fn compose_rejects_artifact_path_escape() {
    let temp = std::env::temp_dir().join(format!(
        "forge-domain-pack-cli-escape-{}",
        std::process::id()
    ));
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("clean stale temp");
    }
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );
    let request_path = fixture_root.join("requests/escape.yaml");
    let request = fs::read_to_string(fixture_root.join("requests/neutral-extension-removed.yaml"))
        .expect("request");
    fs::write(
        &request_path,
        request.replace(
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "../outside.yaml",
        ),
    )
    .expect("escape request");

    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "compose",
            "--request-file",
            "docs/fixtures/domain-pack-v0/requests/escape.yaml",
            "--artifact-root",
            ".",
            "--json",
        ])
        .assert()
        .failure();
    fs::remove_dir_all(temp).expect("cleanup escape fixture");
}

#[test]
fn domain_pack_inputs_are_bounded_before_parse() {
    let temp = fresh_temp("bounded-inputs");
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );

    let oversized_request = temp.join("oversized-request.yaml");
    write_oversized(&oversized_request);
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "compose",
            "--request-file",
            "oversized-request.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "composition request",
    );

    let oversized_manifest = temp.join("oversized-manifest.yaml");
    write_oversized(&oversized_manifest);
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "validate",
            "--manifest-file",
            "oversized-manifest.yaml",
            "--content-file",
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "manifest",
    );

    let oversized_content = temp.join("oversized-content.yaml");
    write_oversized(&oversized_content);
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "validate",
            "--manifest-file",
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "--content-file",
            "oversized-content.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "content",
    );

    write_oversized(&fixture_root.join("artifacts/license-notice.yaml"));
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "validate",
            "--manifest-file",
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "--content-file",
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "license artifact",
    );

    fs::remove_dir_all(temp).expect("cleanup bounded-input fixture");
}

#[test]
fn compose_rejects_canonical_directory_link_escape() {
    let temp = fresh_temp("canonical-escape");
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );
    let outside = temp.join("outside");
    fs::create_dir_all(&outside).expect("create external artifact directory");
    fs::copy(
        fixture_root.join("content/foundation.yaml"),
        outside.join("foundation.yaml"),
    )
    .expect("copy external artifact");
    let link = fixture_root.join("escape-link");
    create_dir_link(&link, &outside);

    let request_path = fixture_root.join("requests/canonical-escape.yaml");
    let request = fs::read_to_string(fixture_root.join("requests/neutral-extension-removed.yaml"))
        .expect("request");
    fs::write(
        &request_path,
        request.replace(
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "escape-link/foundation.yaml",
        ),
    )
    .expect("canonical escape request");

    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "compose",
            "--request-file",
            "docs/fixtures/domain-pack-v0/requests/canonical-escape.yaml",
            "--artifact-root",
            "docs/fixtures/domain-pack-v0",
            "--json",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8_lossy(&stderr).contains("escapes canonical --artifact-root"),
        "unexpected canonical confinement error: {}",
        String::from_utf8_lossy(&stderr)
    );

    fs::remove_dir(&link).expect("remove directory link");
    fs::remove_dir_all(temp).expect("cleanup canonical escape fixture");
}
