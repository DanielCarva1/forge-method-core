#![allow(clippy::missing_errors_doc)]

//! Fail-closed validation and loading for the canonical Markdown retirement authority.

use crate::{Diagnostic, DiagnosticCode, ValidationReport};
use forge_core_contracts::{
    authorize_markdown_load, is_markdown_repo_path, is_typed_authority_reference,
    validate_markdown_allowlist_entry, validate_markdown_policy, MarkdownDebtDisposition,
    MarkdownLoadAudience, MarkdownLoadError, MarkdownRetirementDocument,
};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

pub const MARKDOWN_RETIREMENT_AUTHORITY_PATH: &str =
    "contracts/migration/markdown-debt-inventory.yaml";

const HISTORICAL_DEBT_PATHS: [&str; 12] = [
    "CONTEXT.md",
    "docs/02-runtime-architecture.md",
    "docs/10-agent-facing-contract-migration.md",
    "docs/11-rust-only-core-system-design.md",
    "docs/12-borrowed-shell-runtime-requirements.md",
    "docs/13-operation-contract-v0.md",
    "docs/adr/0016-agent-facing-typed-contracts.md",
    "docs/adr/0017-rust-only-forge-core.md",
    "docs/adr/0018-cli-json-and-mcp-surfaces.md",
    "docs/adr/0019-funnel-autonomy.md",
    "docs/adr/0020-guide-as-default-protocol-orchestrator.md",
    "docs/adr/0021-no-generic-advance-operation.md",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownFileLoadError {
    AuthorityRead(String),
    AuthorityParse(String),
    Authorization(MarkdownLoadError),
    ContentRead(String),
}

/// Canonical Forge-owned Markdown load boundary.
///
/// The typed authority is loaded first, the exact normalized repository path is
/// authorized for the requested audience, and bytes are read only after that
/// decision succeeds. Runtime loads remain denied by every valid policy entry.
pub fn load_authorized_markdown(
    root: &Path,
    path: &str,
    audience: MarkdownLoadAudience,
) -> Result<Vec<u8>, MarkdownFileLoadError> {
    let authority_bytes = read_file_beneath_no_follow(root, MARKDOWN_RETIREMENT_AUTHORITY_PATH)
        .map_err(|error| {
            MarkdownFileLoadError::AuthorityRead(format!(
                "{}: {error}",
                root.join(MARKDOWN_RETIREMENT_AUTHORITY_PATH).display()
            ))
        })?;
    let authority_text = String::from_utf8(authority_bytes)
        .map_err(|error| MarkdownFileLoadError::AuthorityParse(error.to_string()))?;
    let document = yaml_serde::from_str::<MarkdownRetirementDocument>(&authority_text)
        .map_err(|error| MarkdownFileLoadError::AuthorityParse(error.to_string()))?;
    authorize_markdown_load(&document, path, audience)
        .map_err(MarkdownFileLoadError::Authorization)?;
    read_file_beneath_no_follow(root, path).map_err(|error| {
        MarkdownFileLoadError::ContentRead(format!("{}: {error}", root.join(path).display()))
    })
}

#[must_use]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn read_file_beneath_no_follow(root: &Path, path: &str) -> io::Result<Vec<u8>> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};

    const O_RDONLY: i32 = 0;
    const O_NONBLOCK: i32 = 0o4000;
    const O_DIRECTORY: i32 = 0o200_000;
    const O_NOFOLLOW: i32 = 0o400_000;
    const O_CLOEXEC: i32 = 0o2_000_000;

    unsafe extern "C" {
        fn openat(dirfd: i32, pathname: *const std::ffi::c_char, flags: i32, mode: u32) -> i32;
    }

    if path.is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path.ends_with('/')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must use its canonical repository-relative spelling",
        ));
    }

    let mut directory = fs::File::open(root)?;
    if !directory.metadata()?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "repository root is not a directory",
        ));
    }
    let components = path.split('/').collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let component = CString::new(*component).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "path component contains NUL")
        })?;
        let is_final = index + 1 == components.len();
        let flags = if is_final {
            O_RDONLY | O_NONBLOCK | O_NOFOLLOW | O_CLOEXEC
        } else {
            O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC
        };
        // SAFETY: the retained directory descriptor and NUL-terminated direct
        // child name are valid, and a successful descriptor is owned once.
        let fd = unsafe { openat(directory.as_raw_fd(), component.as_ptr(), flags, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `openat` returned a new owned descriptor.
        let opened = unsafe { fs::File::from_raw_fd(fd) };
        if is_final {
            if !opened.metadata()?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "authorized Markdown path is not a regular file",
                ));
            }
            let mut bytes = Vec::new();
            let mut opened = opened;
            opened.read_to_end(&mut bytes)?;
            return Ok(bytes);
        }
        directory = opened;
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "authorized Markdown path is empty",
    ))
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn read_file_beneath_no_follow(_root: &Path, _path: &str) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "exact beneath-root no-follow Markdown loading is unsupported on this platform",
    ))
}

#[must_use]
pub fn validate_markdown_retirement(
    root: &Path,
    document: &MarkdownRetirementDocument,
) -> ValidationReport {
    let mut report = ValidationReport::new();
    if validate_markdown_policy(document).is_err() {
        report.push(Diagnostic::error(
            DiagnosticCode::MarkdownPolicyInvalid,
            MARKDOWN_RETIREMENT_AUTHORITY_PATH,
            "Markdown authority must be exhaustive, structurally valid, non-authoritative, and deny unknown/runtime/agent loads by default",
        ));
    }

    if document.scan_roots != vec![".".to_owned()] {
        report.push(Diagnostic::error(
            DiagnosticCode::MarkdownPolicyInvalid,
            "scan_roots",
            "Markdown scan scope must be exactly the full repository root",
        ));
    }

    let mut allowlisted = BTreeSet::new();
    for entry in &document.allowlist {
        if !is_markdown_repo_path(&entry.path) {
            report.push(Diagnostic::error(
                DiagnosticCode::MarkdownPathInvalid,
                &entry.path,
                "allowlist entries must be normalized repository-relative .md paths",
            ));
        }
        if !allowlisted.insert(entry.path.clone()) {
            report.push(Diagnostic::error(
                DiagnosticCode::MarkdownAllowlistDuplicate,
                &entry.path,
                "Markdown path appears more than once in the allowlist",
            ));
        }
        if validate_markdown_allowlist_entry(entry).is_err() {
            report.push(Diagnostic::error(
                DiagnosticCode::MarkdownEntryInvalid,
                &entry.path,
                "Markdown entries must remain non-authoritative and have role-consistent provenance, loading, reason, and typed references",
            ));
        }
        for authority_ref in &entry.typed_authority_refs {
            if !is_typed_authority_reference(authority_ref) || !root.join(authority_ref).is_file() {
                report.push(Diagnostic::error(
                    DiagnosticCode::MarkdownEntryInvalid,
                    &entry.path,
                    format!(
                        "typed authority reference is not an allowed typed source: {authority_ref}"
                    ),
                ));
            }
        }
    }

    match collect_markdown_paths(root, &document.scan_roots) {
        Ok(discovered) => {
            for path in discovered.difference(&allowlisted) {
                report.push(Diagnostic::error(
                    DiagnosticCode::MarkdownNotAllowlisted,
                    path.as_str(),
                    "repository Markdown is denied until it has an explicit non-authoritative allowlist entry",
                ));
            }
            for path in allowlisted.difference(&discovered) {
                report.push(Diagnostic::error(
                    DiagnosticCode::MarkdownAllowlistedFileMissing,
                    path.as_str(),
                    "allowlisted Markdown path does not exist under the declared scan roots",
                ));
            }
        }
        Err(message) => report.push(Diagnostic::error(
            DiagnosticCode::MarkdownScanFailed,
            "scan_roots",
            message,
        )),
    }

    let mut debt_paths = BTreeSet::new();
    for entry in &document.debt_inventory {
        let valid_disposition = match entry.disposition {
            MarkdownDebtDisposition::Deleted => !root.join(&entry.path).exists(),
            MarkdownDebtDisposition::AllowlistedNonAuthority => {
                root.join(&entry.path).is_file() && allowlisted.contains(&entry.path)
            }
        };
        if !is_markdown_repo_path(&entry.path)
            || !debt_paths.insert(entry.path.as_str())
            || entry.former_role.trim().is_empty()
            || entry.typed_target_refs.is_empty()
            || entry.retirement_reason.trim().is_empty()
            || !valid_disposition
        {
            report.push(Diagnostic::error(
                DiagnosticCode::MarkdownDebtInvalid,
                &entry.path,
                "debt entries require a unique path, typed targets, retirement reason, and a disposition matching the repository",
            ));
        }
        for target_ref in &entry.typed_target_refs {
            if !is_typed_authority_reference(target_ref) || !root.join(target_ref).is_file() {
                report.push(Diagnostic::error(
                    DiagnosticCode::MarkdownDebtInvalid,
                    &entry.path,
                    format!("typed retirement target is not an allowed typed source: {target_ref}"),
                ));
            }
        }
    }
    let expected_debt_paths = HISTORICAL_DEBT_PATHS.into_iter().collect::<BTreeSet<_>>();
    if debt_paths != expected_debt_paths {
        report.push(Diagnostic::error(
            DiagnosticCode::MarkdownDebtInvalid,
            "debt_inventory",
            "the historical Markdown debt inventory must contain exactly the canonical 12 staging paths",
        ));
    }

    report
}

fn collect_markdown_paths(root: &Path, scan_roots: &[String]) -> Result<BTreeSet<String>, String> {
    let mut discovered = BTreeSet::new();
    for scan_root in scan_roots {
        if scan_root != "." {
            return Err(format!("invalid scan root: {scan_root}"));
        }
        let absolute = root.join(scan_root);
        if !absolute.exists() {
            return Err(format!("scan root does not exist: {scan_root}"));
        }
        collect_markdown_under(root, &absolute, &mut discovered)?;
    }
    Ok(discovered)
}

fn collect_markdown_under(
    root: &Path,
    path: &Path,
    discovered: &mut BTreeSet<String>,
) -> Result<(), String> {
    if is_excluded_scan_path(root, path) {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            return Err(format!(
                "repository Markdown may not be a symlink: {}",
                path.display()
            ));
        }
        return Ok(());
    }
    if metadata.is_dir() {
        let entries = fs::read_dir(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            collect_markdown_under(root, &entry.path(), discovered)?;
        }
    } else if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
    {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("Markdown path escaped repository root: {}", path.display()))?;
        discovered.insert(relative.to_string_lossy().replace('\\', "/"));
    }
    Ok(())
}

fn is_excluded_scan_path(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    (relative.components().count() == 1
        && matches!(
            relative.file_name().and_then(|value| value.to_str()),
            Some(".git" | "target" | "target-test")
        ))
        // Claude's isolated worktrees contain duplicate repository snapshots,
        // not additional Markdown governed by the current repository root.
        || relative == Path::new(".claude/worktrees")
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{
        MarkdownAllowlistEntry, MarkdownAuthorityBoundary, MarkdownDebtEntry, MarkdownLoadDecision,
        MarkdownProvenance, MarkdownRole, MARKDOWN_RETIREMENT_POLICY_ID,
        MARKDOWN_RETIREMENT_SCHEMA_VERSION,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("forge-markdown-{name}-{nonce}"))
    }

    fn document() -> MarkdownRetirementDocument {
        MarkdownRetirementDocument {
            schema_version: MARKDOWN_RETIREMENT_SCHEMA_VERSION.to_owned(),
            policy: MARKDOWN_RETIREMENT_POLICY_ID.to_owned(),
            authority: MarkdownAuthorityBoundary {
                markdown_is_authority: false,
                allowlist_is_exhaustive: true,
                unknown_path: MarkdownLoadDecision::Deny,
                runtime_default: MarkdownLoadDecision::Deny,
                agent_default: MarkdownLoadDecision::Deny,
            },
            scan_roots: vec![".".to_owned()],
            allowlist: vec![MarkdownAllowlistEntry {
                path: "docs/allowed.md".to_owned(),
                role: MarkdownRole::DistributionOnly,
                provenance: MarkdownProvenance::HandAuthored,
                runtime_load: MarkdownLoadDecision::Deny,
                agent_load: MarkdownLoadDecision::AllowNonAuthoritative,
                typed_authority_refs: Vec::new(),
                non_authority_reason: "distribution only".to_owned(),
            }],
            debt_inventory: Vec::new(),
        }
    }

    fn write_allowed(root: &Path) {
        fs::create_dir_all(root.join("docs")).expect("create docs");
        fs::write(root.join("docs/allowed.md"), "allowed").expect("write allowed");
    }

    fn write_authority(root: &Path, document: &MarkdownRetirementDocument) {
        let authority_path = root.join(MARKDOWN_RETIREMENT_AUTHORITY_PATH);
        fs::create_dir_all(authority_path.parent().expect("authority parent"))
            .expect("create authority parent");
        fs::write(
            authority_path,
            yaml_serde::to_string(document).expect("serialize authority"),
        )
        .expect("write authority");
    }

    #[test]
    fn newly_introduced_markdown_is_rejected() {
        let root = temp_root("unknown");
        write_allowed(&root);
        fs::write(root.join("docs/new-authority.md"), "new").expect("write unknown");
        let report = validate_markdown_retirement(&root, &document());
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MarkdownNotAllowlisted
                && diagnostic.path == "docs/new-authority.md"
        }));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn narrowed_scan_root_is_invalid() {
        let root = temp_root("narrow-scan");
        write_allowed(&root);
        let mut document = document();
        document.scan_roots = vec!["docs".to_owned()];
        let report = validate_markdown_retirement(&root, &document);
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MarkdownPolicyInvalid
                && diagnostic.path == "scan_roots"
        }));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn nested_target_directory_is_scanned() {
        let root = temp_root("nested-target");
        write_allowed(&root);
        fs::create_dir_all(root.join("docs/target")).expect("create nested target");
        fs::write(root.join("docs/target/new-authority.md"), "new").expect("write unknown");
        let report = validate_markdown_retirement(&root, &document());
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MarkdownNotAllowlisted
                && diagnostic.path == "docs/target/new-authority.md"
        }));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn claude_worktrees_are_excluded_without_weakening_repository_scan() {
        let root = temp_root("claude-worktrees");
        write_allowed(&root);
        let copied_docs = root.join(".claude/worktrees/agent/docs");
        fs::create_dir_all(&copied_docs).expect("create copied worktree docs");
        fs::write(copied_docs.join("copied.md"), "copied").expect("write copied Markdown");
        fs::write(root.join("docs/new-authority.md"), "new").expect("write unknown");

        let report = validate_markdown_retirement(&root, &document());
        assert!(!report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.path.starts_with(".claude/worktrees/")));
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MarkdownNotAllowlisted
                && diagnostic.path == "docs/new-authority.md"
        }));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn non_markdown_symlink_is_skipped_without_following() {
        use std::os::unix::fs::symlink;

        let root = temp_root("non-markdown-symlink");
        let outside = temp_root("non-markdown-symlink-outside");
        write_allowed(&root);
        fs::create_dir_all(root.join("fuzz")).expect("create fuzz");
        fs::create_dir_all(&outside).expect("create outside");
        fs::write(outside.join("hidden.md"), "outside").expect("write outside Markdown");
        symlink(&outside, root.join("fuzz/target")).expect("create target symlink");

        let report = validate_markdown_retirement(&root, &document());
        assert!(!report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MarkdownScanFailed
                || diagnostic.path.contains("hidden.md")
        }));

        fs::remove_dir_all(root).expect("cleanup root");
        fs::remove_dir_all(outside).expect("cleanup outside");
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn authorized_load_rejects_symlink_component_outside_root() {
        use std::os::unix::fs::symlink;

        let root = temp_root("authorized-symlink");
        let outside = temp_root("authorized-symlink-outside");
        let document = document();
        write_authority(&root, &document);
        write_allowed(&root);
        assert_eq!(
            load_authorized_markdown(&root, "docs/allowed.md", MarkdownLoadAudience::Agent)
                .expect("load regular Markdown"),
            b"allowed"
        );
        fs::remove_dir_all(root.join("docs")).expect("remove regular docs");
        fs::create_dir_all(&outside).expect("create outside");
        fs::write(outside.join("allowed.md"), "outside").expect("write outside Markdown");
        symlink(&outside, root.join("docs")).expect("create docs symlink");

        assert!(matches!(
            load_authorized_markdown(&root, "docs/allowed.md", MarkdownLoadAudience::Agent),
            Err(MarkdownFileLoadError::ContentRead(_))
        ));

        fs::remove_dir_all(root).expect("cleanup root");
        fs::remove_dir_all(outside).expect("cleanup outside");
    }

    #[test]
    fn uppercase_markdown_extension_is_scanned() {
        let root = temp_root("uppercase");
        write_allowed(&root);
        fs::write(root.join("AUTHORITY.MD"), "new").expect("write unknown");
        let report = validate_markdown_retirement(&root, &document());
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MarkdownNotAllowlisted
                && diagnostic.path == "AUTHORITY.MD"
        }));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn markdown_typed_target_is_rejected() {
        let root = temp_root("markdown-target");
        write_allowed(&root);
        fs::write(root.join("README.md"), "not typed").expect("write target");
        let mut document = document();
        document.debt_inventory.push(MarkdownDebtEntry {
            path: "docs/retired.md".to_owned(),
            former_role: "staging".to_owned(),
            disposition: MarkdownDebtDisposition::Deleted,
            typed_target_refs: vec!["README.md".to_owned()],
            retirement_reason: "retired".to_owned(),
        });
        let report = validate_markdown_retirement(&root, &document);
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MarkdownDebtInvalid
                && diagnostic.path == "docs/retired.md"
                && diagnostic.message.contains("README.md")
        }));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn generated_projection_requires_typed_source() {
        let root = temp_root("generated-no-source");
        write_allowed(&root);
        let mut document = document();
        document.allowlist[0].role = MarkdownRole::GeneratedProjection;
        document.allowlist[0].provenance = MarkdownProvenance::GeneratedFromTypedSource;
        let report = validate_markdown_retirement(&root, &document);
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::MarkdownEntryInvalid
                && diagnostic.path == "docs/allowed.md"
        }));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
