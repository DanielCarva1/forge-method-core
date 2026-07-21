#![allow(clippy::missing_errors_doc)]

//! Typed authority boundary for repository Markdown.
//!
//! Markdown is never Forge runtime authority. The allowlist is exhaustive for
//! repository Markdown and grants only an explicitly bounded non-authoritative
//! distribution or agent-guidance role. Unknown paths and audiences fail closed.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

pub const MARKDOWN_RETIREMENT_SCHEMA_VERSION: &str = "0.3";
pub const MARKDOWN_RETIREMENT_POLICY_ID: &str = "markdown_retirement_authority";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkdownRetirementDocument {
    pub schema_version: String,
    pub policy: String,
    pub authority: MarkdownAuthorityBoundary,
    pub scan_roots: Vec<String>,
    pub allowlist: Vec<MarkdownAllowlistEntry>,
    pub debt_inventory: Vec<MarkdownDebtEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkdownAuthorityBoundary {
    pub markdown_is_authority: bool,
    pub allowlist_is_exhaustive: bool,
    pub unknown_path: MarkdownLoadDecision,
    pub runtime_default: MarkdownLoadDecision,
    pub agent_default: MarkdownLoadDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkdownAllowlistEntry {
    pub path: String,
    pub role: MarkdownRole,
    pub provenance: MarkdownProvenance,
    pub runtime_load: MarkdownLoadDecision,
    pub agent_load: MarkdownLoadDecision,
    pub typed_authority_refs: Vec<String>,
    pub non_authority_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkdownDebtEntry {
    pub path: String,
    pub former_role: String,
    pub disposition: MarkdownDebtDisposition,
    pub typed_target_refs: Vec<String>,
    pub retirement_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownRole {
    DistributionOnly,
    GeneratedProjection,
    AgentGuidance,
    FixtureExplanation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownProvenance {
    HandAuthored,
    GeneratedFromTypedSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownLoadDecision {
    Deny,
    AllowNonAuthoritative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownDebtDisposition {
    Deleted,
    AllowlistedNonAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownLoadAudience {
    Runtime,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownLoadError {
    InvalidPolicy,
    NotAllowlisted,
    AudienceDenied,
}

/// Return whether a path is a normalized repository-relative Markdown path.
#[must_use]
pub fn is_markdown_repo_path(value: &str) -> bool {
    is_normalized_repo_path(value)
        && Path::new(value)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

/// Return whether a reference belongs to a typed authority class.
///
/// Typed contract data is owned below `contracts/` as YAML or JSON. Explicit
/// Rust authority references are limited to crate source trees, never tests or
/// fixtures. Markdown is consequently rejected regardless of location.
#[must_use]
pub fn is_typed_authority_reference(value: &str) -> bool {
    if !is_normalized_repo_path(value) {
        return false;
    }
    let path = Path::new(value);
    let extension = path.extension().and_then(|extension| extension.to_str());
    if value.starts_with("contracts/") {
        return extension.is_some_and(|extension| {
            extension.eq_ignore_ascii_case("yaml")
                || extension.eq_ignore_ascii_case("yml")
                || extension.eq_ignore_ascii_case("json")
        });
    }
    let components = value.split('/').collect::<Vec<_>>();
    components.len() >= 4
        && components[0] == "crates"
        && components[2] == "src"
        && extension.is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
}

/// Validate all filesystem-independent invariants required before any Markdown
/// authorization decision can be trusted.
pub fn validate_markdown_policy(
    document: &MarkdownRetirementDocument,
) -> Result<(), MarkdownLoadError> {
    if document.schema_version != MARKDOWN_RETIREMENT_SCHEMA_VERSION
        || document.policy != MARKDOWN_RETIREMENT_POLICY_ID
        || document.authority.markdown_is_authority
        || !document.authority.allowlist_is_exhaustive
        || document.authority.unknown_path != MarkdownLoadDecision::Deny
        || document.authority.runtime_default != MarkdownLoadDecision::Deny
        || document.authority.agent_default != MarkdownLoadDecision::Deny
        || document.scan_roots.len() != 1
        || document.scan_roots[0] != "."
    {
        return Err(MarkdownLoadError::InvalidPolicy);
    }

    for (index, entry) in document.allowlist.iter().enumerate() {
        validate_markdown_allowlist_entry(entry)?;
        if document.allowlist[..index]
            .iter()
            .any(|candidate| candidate.path == entry.path)
        {
            return Err(MarkdownLoadError::InvalidPolicy);
        }
    }
    Ok(())
}

/// Validate one allowlist entry without consulting the filesystem.
pub fn validate_markdown_allowlist_entry(
    entry: &MarkdownAllowlistEntry,
) -> Result<(), MarkdownLoadError> {
    let generated = matches!(entry.role, MarkdownRole::GeneratedProjection);
    let refs_required = generated || matches!(entry.role, MarkdownRole::AgentGuidance);
    if !is_markdown_repo_path(&entry.path)
        || entry.runtime_load != MarkdownLoadDecision::Deny
        || entry.non_authority_reason.trim().is_empty()
        || generated != (entry.provenance == MarkdownProvenance::GeneratedFromTypedSource)
        || refs_required && entry.typed_authority_refs.is_empty()
        || entry
            .typed_authority_refs
            .iter()
            .any(|authority_ref| !is_typed_authority_reference(authority_ref))
        || matches!(
            entry.role,
            MarkdownRole::AgentGuidance | MarkdownRole::GeneratedProjection
        ) && entry.agent_load != MarkdownLoadDecision::AllowNonAuthoritative
    {
        return Err(MarkdownLoadError::InvalidPolicy);
    }
    Ok(())
}

/// Authorize a Markdown load without granting authority to its contents.
///
/// The typed policy must itself be fail-closed. An unknown path, a permissive
/// default, a malformed entry, or an audience not explicitly allowed for the
/// exact normalized path is denied.
pub fn authorize_markdown_load<'a>(
    document: &'a MarkdownRetirementDocument,
    path: &str,
    audience: MarkdownLoadAudience,
) -> Result<&'a MarkdownAllowlistEntry, MarkdownLoadError> {
    validate_markdown_policy(document)?;
    if !is_markdown_repo_path(path) {
        return Err(MarkdownLoadError::InvalidPolicy);
    }
    let entry = document
        .allowlist
        .iter()
        .find(|entry| entry.path == path)
        .ok_or(MarkdownLoadError::NotAllowlisted)?;
    let decision = match audience {
        MarkdownLoadAudience::Runtime => entry.runtime_load,
        MarkdownLoadAudience::Agent => entry.agent_load,
    };
    if decision != MarkdownLoadDecision::AllowNonAuthoritative {
        return Err(MarkdownLoadError::AudienceDenied);
    }
    Ok(entry)
}

fn is_normalized_repo_path(value: &str) -> bool {
    if value == "." {
        return true;
    }
    if value.is_empty()
        || value.contains('\\')
        || value.starts_with('/')
        || value.ends_with('/')
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return false;
    }
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> MarkdownRetirementDocument {
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
                path: "docs/agent-guide.md".to_owned(),
                role: MarkdownRole::AgentGuidance,
                provenance: MarkdownProvenance::HandAuthored,
                runtime_load: MarkdownLoadDecision::Deny,
                agent_load: MarkdownLoadDecision::AllowNonAuthoritative,
                typed_authority_refs: vec!["contracts/policies/example.yaml".to_owned()],
                non_authority_reason: "navigation only".to_owned(),
            }],
            debt_inventory: Vec::new(),
        }
    }

    #[test]
    fn unknown_markdown_load_fails_closed() {
        assert_eq!(
            authorize_markdown_load(
                &policy(),
                "docs/new-authority.md",
                MarkdownLoadAudience::Agent,
            ),
            Err(MarkdownLoadError::NotAllowlisted)
        );
    }

    #[test]
    fn runtime_cannot_load_agent_only_markdown() {
        assert_eq!(
            authorize_markdown_load(
                &policy(),
                "docs/agent-guide.md",
                MarkdownLoadAudience::Runtime,
            ),
            Err(MarkdownLoadError::AudienceDenied)
        );
    }

    #[test]
    fn permissive_defaults_invalidate_the_policy() {
        let mut document = policy();
        document.authority.unknown_path = MarkdownLoadDecision::AllowNonAuthoritative;
        assert_eq!(
            authorize_markdown_load(
                &document,
                "docs/agent-guide.md",
                MarkdownLoadAudience::Agent,
            ),
            Err(MarkdownLoadError::InvalidPolicy)
        );
    }

    #[test]
    fn absolute_allowlist_path_invalidates_authorization() {
        let mut document = policy();
        document.allowlist[0].path = "/docs/agent-guide.md".to_owned();
        assert_eq!(
            authorize_markdown_load(
                &document,
                "/docs/agent-guide.md",
                MarkdownLoadAudience::Agent,
            ),
            Err(MarkdownLoadError::InvalidPolicy)
        );
    }

    #[test]
    fn provenance_mismatch_invalidates_authorization() {
        let mut document = policy();
        document.allowlist[0].provenance = MarkdownProvenance::GeneratedFromTypedSource;
        assert_eq!(
            authorize_markdown_load(
                &document,
                "docs/agent-guide.md",
                MarkdownLoadAudience::Agent,
            ),
            Err(MarkdownLoadError::InvalidPolicy)
        );
    }

    #[test]
    fn missing_required_refs_invalidates_authorization() {
        let mut document = policy();
        document.allowlist[0].typed_authority_refs.clear();
        assert_eq!(
            authorize_markdown_load(
                &document,
                "docs/agent-guide.md",
                MarkdownLoadAudience::Agent,
            ),
            Err(MarkdownLoadError::InvalidPolicy)
        );
    }

    #[test]
    fn policy_requires_the_exact_repository_scan_root() {
        for scan_roots in [
            Vec::new(),
            vec!["docs".to_owned()],
            vec![".".to_owned(), ".".to_owned()],
        ] {
            let mut document = policy();
            document.scan_roots = scan_roots;
            assert_eq!(
                validate_markdown_policy(&document),
                Err(MarkdownLoadError::InvalidPolicy)
            );
        }
    }

    #[test]
    fn noncanonical_raw_path_spellings_are_rejected() {
        for path in ["docs//agent-guide.md", "docs/./agent-guide.md"] {
            assert!(!is_markdown_repo_path(path));
            let mut document = policy();
            document.allowlist[0].path = path.to_owned();
            assert_eq!(
                validate_markdown_policy(&document),
                Err(MarkdownLoadError::InvalidPolicy)
            );
            assert_eq!(
                authorize_markdown_load(&policy(), path, MarkdownLoadAudience::Agent),
                Err(MarkdownLoadError::InvalidPolicy)
            );
        }
    }

    #[test]
    fn crate_authority_requires_the_crate_root_src_segment() {
        assert!(is_typed_authority_reference("crates/example/src/lib.rs"));
        for path in [
            "crates/example/tests/src/fake.rs",
            "crates/example/fixtures/src/fake.rs",
            "crates/example/nested/src/fake.rs",
        ] {
            assert!(!is_typed_authority_reference(path));
        }
    }
}
