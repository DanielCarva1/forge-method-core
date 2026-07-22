#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

//! Closed classification of EventLog-owned state artifacts.
//!
//! This module deliberately exposes names and classifications, never an
//! authority or an override.  Generic state mutation callers use it only to
//! reject paths before opening them.

use crate::backup_manifest::{
    GOVERNANCE_CONFLICT_EVENT_LOG_LOCK_RELATIVE_PATH, GOVERNANCE_CONFLICT_EVENT_LOG_RELATIVE_PATH,
    MEMORY_EVENT_LOG_LOCK_RELATIVE_PATH, MEMORY_EVENT_LOG_RELATIVE_PATH,
    RESEARCH_EVENT_LOG_LOCK_RELATIVE_PATH, RESEARCH_EVENT_LOG_RELATIVE_PATH,
};
use std::path::{Component, Path, PathBuf};

pub const LEGACY_STATE_ROOT_COMPONENT: &str = ".forge-method";

/// The only `EventLog` artifacts whose writer authority is not generic Store
/// authority.  This enum is descriptive data; it is not a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReservedStateArtifact {
    MemoryEventLog,
    ResearchEventLog,
    GovernanceConflictEventLog,
    MemoryEventLogLock,
    ResearchEventLogLock,
    GovernanceConflictEventLogLock,
}

impl ReservedStateArtifact {
    #[must_use]
    pub const fn canonical_relative_path(self) -> &'static str {
        match self {
            Self::MemoryEventLog => MEMORY_EVENT_LOG_RELATIVE_PATH,
            Self::ResearchEventLog => RESEARCH_EVENT_LOG_RELATIVE_PATH,
            Self::GovernanceConflictEventLog => GOVERNANCE_CONFLICT_EVENT_LOG_RELATIVE_PATH,
            Self::MemoryEventLogLock => MEMORY_EVENT_LOG_LOCK_RELATIVE_PATH,
            Self::ResearchEventLogLock => RESEARCH_EVENT_LOG_LOCK_RELATIVE_PATH,
            Self::GovernanceConflictEventLogLock => {
                GOVERNANCE_CONFLICT_EVENT_LOG_LOCK_RELATIVE_PATH
            }
        }
    }

    #[must_use]
    pub const fn is_log(self) -> bool {
        matches!(
            self,
            Self::MemoryEventLog | Self::ResearchEventLog | Self::GovernanceConflictEventLog
        )
    }
}

pub const RESERVED_STATE_ARTIFACTS: &[ReservedStateArtifact] = &[
    ReservedStateArtifact::MemoryEventLog,
    ReservedStateArtifact::ResearchEventLog,
    ReservedStateArtifact::GovernanceConflictEventLog,
    ReservedStateArtifact::MemoryEventLogLock,
    ReservedStateArtifact::ResearchEventLogLock,
    ReservedStateArtifact::GovernanceConflictEventLogLock,
];

/// Why a state-relative path belongs to the EventLog-reserved namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReservedStatePath {
    Artifact(ReservedStateArtifact),
    CrashReplaceSibling(ReservedStateArtifact),
}

impl ReservedStatePath {
    #[must_use]
    pub const fn artifact(self) -> ReservedStateArtifact {
        match self {
            Self::Artifact(artifact) | Self::CrashReplaceSibling(artifact) => artifact,
        }
    }
}

/// Input rejected before it can be interpreted as a state-root-relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateRelativePathError {
    Empty,
    Absolute,
    Parent,
    Dot,
    Prefix,
    NonUtf8,
    SeparatorAmbiguity,
}

/// Normalize either a state-root spelling or the legacy project-root
/// `.forge-method/<path>` spelling into a canonical state-root-relative path.
///
/// The function does not collapse `.` or `..`: accepting either would let
/// callers obtain a classification different from the file operation.
pub fn normalize_state_relative_path(
    value: impl AsRef<Path>,
) -> Result<PathBuf, StateRelativePathError> {
    let value = value.as_ref();
    if value.as_os_str().is_empty() {
        return Err(StateRelativePathError::Empty);
    }
    if value.is_absolute() {
        return Err(StateRelativePathError::Absolute);
    }
    let text = value.to_str().ok_or(StateRelativePathError::NonUtf8)?;
    if text.contains('\\') {
        return Err(StateRelativePathError::SeparatorAmbiguity);
    }

    let mut components = Vec::new();
    for component in value.components() {
        match component {
            Component::Normal(component) => components.push(component),
            Component::CurDir => return Err(StateRelativePathError::Dot),
            Component::ParentDir => return Err(StateRelativePathError::Parent),
            Component::RootDir => return Err(StateRelativePathError::Absolute),
            Component::Prefix(_) => return Err(StateRelativePathError::Prefix),
        }
    }
    if components.is_empty() {
        return Err(StateRelativePathError::Empty);
    }
    if components.first().and_then(|component| component.to_str())
        == Some(LEGACY_STATE_ROOT_COMPONENT)
    {
        components.remove(0);
        if components.is_empty() {
            return Err(StateRelativePathError::Empty);
        }
    }
    Ok(components.into_iter().collect())
}

/// Classify a normalized or legacy state-relative spelling without granting an
/// exception to the caller.
#[must_use]
pub fn classify_reserved_state_path(value: impl AsRef<Path>) -> Option<ReservedStatePath> {
    let normalized = normalize_state_relative_path(value).ok()?;
    for artifact in RESERVED_STATE_ARTIFACTS {
        let canonical = Path::new(artifact.canonical_relative_path());
        if normalized == canonical {
            return Some(ReservedStatePath::Artifact(*artifact));
        }
        if crash_replace_siblings(*artifact)
            .iter()
            .any(|sibling| normalized.as_path() == sibling.as_path())
        {
            return Some(ReservedStatePath::CrashReplaceSibling(*artifact));
        }
    }
    None
}

#[must_use]
pub fn is_reserved_state_path(value: impl AsRef<Path>) -> bool {
    classify_reserved_state_path(value).is_some()
}

/// The fixed crash-replace sidecars generated for a protected target.
#[must_use]
pub fn crash_replace_siblings(artifact: ReservedStateArtifact) -> [PathBuf; 3] {
    let target = Path::new(artifact.canonical_relative_path());
    let parent = target.parent().unwrap_or_else(|| Path::new(""));
    let name = target
        .file_name()
        .expect("reserved artifact has a name")
        .to_string_lossy();
    [
        parent.join(format!(".{name}.forge-next")),
        parent.join(format!(".{name}.forge-previous")),
        parent.join(format!(".{name}.forge-transaction")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_canonical_legacy_and_protocol_siblings() {
        for artifact in RESERVED_STATE_ARTIFACTS {
            let path = artifact.canonical_relative_path();
            assert_eq!(
                classify_reserved_state_path(path),
                Some(ReservedStatePath::Artifact(*artifact))
            );
            assert_eq!(
                classify_reserved_state_path(format!(".forge-method/{path}")),
                Some(ReservedStatePath::Artifact(*artifact))
            );
            for sibling in crash_replace_siblings(*artifact) {
                assert_eq!(
                    classify_reserved_state_path(sibling),
                    Some(ReservedStatePath::CrashReplaceSibling(*artifact))
                );
            }
        }
    }

    #[test]
    fn rejects_ambiguous_paths_instead_of_normalizing_them() {
        for path in [
            "",
            "/memory/events.ndjson",
            "../memory/events.ndjson",
            "./memory/events.ndjson",
            ".forge-method",
        ] {
            assert!(normalize_state_relative_path(path).is_err(), "{path}");
        }
    }
}
