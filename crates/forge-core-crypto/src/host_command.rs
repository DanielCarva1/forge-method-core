//! Host command admission safety predicates.
//!
//! Small, dependency-free predicates used by the host-adapter artifact /
//! provenance verification entrypoints in
//! [`host_adapter_verification`](crate::host_adapter_verification) to validate
//! immutable source references and version-like evidence.
//!
//! The remaining host-command admission helpers (the `host_command` builder,
//! `argv_has_shell_control`, `env_key_is_forbidden`) stay in the
//! `forge-core-cli` crate, where the host-adapter projection / invocation /
//! distribution admission gates live.

/// Returns `true` when `source_ref` contains a 40-character ASCII hex segment,
/// treating it as a git SHA-1-style immutable commit reference. Used by the
/// distribution admission and artifact-verification gates to require an
/// immutable source anchor.
pub fn source_ref_is_immutable(source_ref: &str) -> bool {
    source_ref
        .split(|character: char| !character.is_ascii_hexdigit())
        .any(|segment| segment.len() == 40 && segment.chars().all(|item| item.is_ascii_hexdigit()))
}

/// Returns `true` when `value` looks like a version string: non-empty after
/// trimming and composed only of `[A-Za-z0-9.\-+_]+`. Used by the distribution
/// admission and artifact-verification gates to validate version evidence.
pub fn version_like(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|item| item.is_ascii_alphanumeric() || matches!(item, '.' | '-' | '_' | '+'))
}
