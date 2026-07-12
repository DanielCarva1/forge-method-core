//! Embedded shared contract definitions — zero-config consumer bootstrap.
//!
//! A consumer project that runs `forge-core project init` gets the runtime
//! state tree (claims-active, WAL, handoffs, …) but **not** the `contracts/`
//! tree, which belongs to the Forge core repo. Without that tree, `validate`
//! and `execute-operation` fail to resolve the shared contract definitions
//! (claim-contract-v0, operation-contract-v0, gate-contract-v0, …) and the
//! canonical operation contracts they reference.
//!
//! This module closes that gap by embedding the same `contracts/` tree into
//! the binary, exactly the way [`catalog`] already embeds `contracts/workflows/`
//! for the `guide` command. Callers resolve a contract by its repo-relative
//! path (e.g. `"contracts/claims/claim-contract-v0.yaml"`): the on-disk copy
//! under the project root always wins when present (so a core checkout, or a
//! consumer that pins/patches a contract locally, is unaffected), and the
//! embedded bytes are the fallback.
//!
//! # Why embed (not copy/symlink/indirection)
//!
//! - **Provenance-safe:** `execute-operation` canonicalizes every contract
//!   input path and rejects anything resolving outside the project root
//!   (`ContractPathOutsideRoot`). Bytes served from the binary never pass
//!   through filesystem canonicalization, so the fallback cannot be used to
//!   escape the root.
//! - **Zero-config:** `project init` copies nothing; there is no drift, no
//!   symlink (fragile on Windows and in git), no new config field.
//! - **Deterministic updates:** reinstalling `forge-core` updates the shared
//!   contracts for every consumer.
//!
//! See `contracts/spec/claims-integrity-spine-spec.yaml` and the
//! `contracts/migration/` tree for the contract family this serves.
//!
//! [`catalog`]: crate::catalog

use include_dir::{include_dir, Dir};
use std::path::Path;

/// The full `contracts/` tree, compiled into the binary at build time.
///
/// `include_dir!` stores the bytes compressed; access is lazy, so the runtime
/// cost is bounded by what callers actually request, not by the tree size.
/// The build fails if the source `contracts/` dir is absent.
static EMBEDDED_CONTRACTS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../contracts");

/// Resolve a contract's text by repo-relative path, falling back to the
/// embedded tree.
///
/// `rel_path` is a repo-relative POSIX-style string such as
/// `"contracts/claims/claim-contract-v0.yaml"`. Resolution order:
///
/// 1. `<root>/<rel_path>` on disk — the project's own copy wins when present
///    (core checkout, or a consumer patching a contract locally).
/// 2. The embedded tree compiled into the binary.
///
/// Returns the file contents as a `String`, or `None` if the contract is
/// absent from both sources.
#[must_use]
pub fn read_contract_text(root: &Path, rel_path: &str) -> Option<String> {
    // 1. Disk first: a local copy always overrides the embedded bytes. This
    //    preserves the core-repo behaviour (reads its own contracts/) and lets
    //    a consumer pin/patch a contract by placing a file at the same path.
    let on_disk = root.join(rel_path);
    if let Ok(text) = std::fs::read_to_string(&on_disk) {
        return Some(text);
    }
    // 2. Embedded fallback.
    embedded_text(rel_path).map(std::string::ToString::to_string)
}

/// Look up a contract purely in the embedded tree (no disk access).
///
/// Useful when the caller has already decided the disk copy is absent or
/// untrusted, and wants only the canonical shipped bytes.
#[must_use]
pub fn embedded_text(rel_path: &str) -> Option<&'static str> {
    // The embedded Dir is rooted AT `contracts/`, so its internal paths are
    // relative to that dir ("claims/claim-contract-v0.yaml", not
    // "contracts/claims/..."). Strip a leading "contracts/" (or any leading
    // separators) before the lookup.
    let within = strip_contracts_prefix(rel_path);
    // include_dir stores internal paths with the host's native separator at
    // build time, but the embedded bytes are baked into the binary, so a
    // Windows-built artifact would carry '\' paths and a Linux-built one '/'.
    // Callers may pass either separator (a repo-relative path written on one
    // platform is often resolved on another), so normalise to '/' before the
    // lookup and try both the normalised and the raw form. This keeps
    // embedded_exists / embedded_text portable across build and runtime hosts.
    let normalised = within.replace('\\', "/");
    let file = EMBEDDED_CONTRACTS
        .get_file(normalised.as_str())
        .or_else(|| EMBEDDED_CONTRACTS.get_file(within))?;
    let contents = file.contents();
    std::str::from_utf8(contents).ok()
}

/// Strip a leading `contracts/` prefix (POSIX or Windows separators) from a
/// repo-relative path, yielding the path relative to the embedded `Dir`.
fn strip_contracts_prefix(rel_path: &str) -> &str {
    let trimmed = rel_path.trim_start_matches(['/', '\\']);
    trimmed
        .strip_prefix("contracts/")
        .or_else(|| trimmed.strip_prefix("contracts\\"))
        .unwrap_or(trimmed)
}

/// Whether a contract at `rel_path` exists in the embedded tree.
#[must_use]
pub fn embedded_exists(rel_path: &str) -> bool {
    embedded_text(rel_path).is_some()
}

/// Every `.yaml` file in the embedded tree, as repo-relative paths
/// (e.g. `"contracts/claims/claim-contract-v0.yaml"`).
///
/// Used by callers that need the full set of canonically-known contract
/// paths to seed a reference index without scanning disk.
#[must_use]
pub fn embedded_yaml_paths() -> Vec<String> {
    use include_dir::DirEntry;
    fn walk<'a>(dir: &'a Dir<'a>, out: &mut Vec<String>) {
        for entry in dir.entries() {
            match entry {
                // include_dir paths are already relative to the embedded root;
                // carrying a second recursive prefix duplicates directory
                // components on Windows (for example evidence/evidence/...).
                DirEntry::Dir(d) => walk(d, out),
                DirEntry::File(f) => {
                    if f.path().extension().is_some_and(|ext| ext == "yaml") {
                        let relative = f.path().to_string_lossy().replace('\\', "/");
                        out.push(format!("contracts/{relative}"));
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&EMBEDDED_CONTRACTS, &mut out);
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_claim_contract_definition_exists() {
        // One of the 12 shared definitions; must always be embedded.
        assert!(embedded_exists("contracts/claims/claim-contract-v0.yaml"));
        let text = embedded_text("contracts/claims/claim-contract-v0.yaml")
            .expect("claim-contract-v0.yaml embedded");
        assert!(
            text.contains("schema_version") || text.contains("schema_version"),
            "embedded claim contract looks like YAML: {text:?}"
        );
    }

    #[test]
    fn embedded_operation_contract_definition_exists() {
        assert!(embedded_exists(
            "contracts/operations/operation-contract-v0.yaml"
        ));
    }

    #[test]
    fn embedded_handles_both_separators() {
        // POSIX and Windows path separators both resolve.
        assert!(embedded_exists("contracts/claims/claim-contract-v0.yaml"));
        assert!(embedded_exists("contracts\\claims\\claim-contract-v0.yaml"));
    }

    #[test]
    fn embedded_returns_none_for_absent() {
        assert!(!embedded_exists("contracts/does/not/exist.yaml"));
        assert!(embedded_text("contracts/does/not/exist.yaml").is_none());
    }

    #[test]
    fn embedded_yaml_paths_are_unique_repo_relative_paths() {
        let paths = embedded_yaml_paths();
        assert!(paths.contains(
            &"contracts/workflow-governance/runtime-core-assurance-candidate-v0.yaml".to_owned()
        ));
        assert!(paths.iter().all(|path| {
            !path.contains("/evidence/evidence/")
                && !path.contains("/migration/migration/")
                && !path.contains("/workflow-governance/workflow-governance/")
        }));
        let mut unique = paths.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(paths.len(), unique.len());
    }

    #[test]
    fn read_contract_text_prefers_disk_over_embedded() {
        // A temp dir with an overriding file must shadow the embedded bytes.
        let tmp = std::env::temp_dir().join(format!(
            "forge-embedded-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(tmp.join("contracts/claims")).unwrap();
        let rel = "contracts/claims/claim-contract-v0.yaml";
        std::fs::write(tmp.join(rel), "LOCAL_OVERRIDE: true\n").unwrap();
        let text = read_contract_text(&tmp, rel).expect("resolved");
        assert!(
            text.contains("LOCAL_OVERRIDE"),
            "disk copy must shadow embedded: {text:?}"
        );
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_contract_text_falls_back_to_embedded_when_disk_absent() {
        // No file on disk; the embedded bytes must surface.
        let tmp = std::env::temp_dir().join(format!(
            "forge-embedded-empty-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let rel = "contracts/claims/claim-contract-v0.yaml";
        let text = read_contract_text(&tmp, rel).expect("fell back to embedded");
        assert!(
            !text.contains("LOCAL_OVERRIDE"),
            "must be the embedded copy, not a stray disk file"
        );
        std::fs::remove_dir_all(&tmp).ok();
    }
}
