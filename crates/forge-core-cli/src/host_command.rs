//! Host command builder and admission safety helpers.
//!
//! Small private helpers that build a [`HostAdapterCommand`] from a compact
//! metadata struct and the four safety predicates used by the host-adapter
//! invocation/distribution admission gates (`argv_has_shell_control`,
//! `env_key_is_forbidden`, `source_ref_is_immutable`, `version_like`).
//!
//! The manifest keeps host-specific security metadata here, but top-level
//! command identity and JSON capability are anchored in
//! `forge-core-command-surface`. That makes the host-adapter manifest a narrow
//! adapter over the shared Command Surface seam instead of a rival command
//! registry.

use forge_core_command_surface::{self as command_surface, JsonMode};

use crate::host_adapter_types::{
    HostAdapterAuthorityClass, HostAdapterAutoTrigger, HostAdapterCommand, HostAdapterCommandKind,
    HostAdapterMutationClass, HostAdapterOutputTreatment,
};

/// Compact metadata used to materialize a [`HostAdapterCommand`] via
/// [`host_command`]. Holding `&str`/`Vec<&str>` here keeps the manifest builder
/// call sites allocation-light; the `host_command` constructor turns each
/// borrowed field into an owned `String` on the produced command.
pub(crate) struct HostCommandMetadata<'a> {
    pub(crate) name: &'a str,
    pub(crate) command_kind: HostAdapterCommandKind,
    pub(crate) mutation_class: HostAdapterMutationClass,
    pub(crate) authority_class: HostAdapterAuthorityClass,
    pub(crate) required_contracts: Vec<&'a str>,
    pub(crate) safe_auto_invocation_triggers: Vec<HostAdapterAutoTrigger>,
    pub(crate) output_treatment: Vec<HostAdapterOutputTreatment>,
    pub(crate) policy_refs: Vec<&'a str>,
    pub(crate) adapters_must_not: Vec<&'a str>,
}

/// Materialize a [`HostAdapterCommand`] from a [`HostCommandMetadata`],
/// promoting borrowed `&str` fields into owned `String` values and marking the
/// command as JSON-capable (`json_supported: true`).
pub(crate) fn host_command(metadata: HostCommandMetadata<'_>) -> HostAdapterCommand {
    let surface = command_surface::command_by_name(metadata.name).unwrap_or_else(|| {
        panic!(
            "host adapter command '{}' is missing from forge-core-command-surface",
            metadata.name
        )
    });
    HostAdapterCommand {
        name: surface.name.to_string(),
        command_kind: metadata.command_kind,
        mutation_class: metadata.mutation_class,
        authority_class: metadata.authority_class,
        json_supported: matches!(surface.json_mode, JsonMode::EnvelopeOptional),
        required_contracts: metadata
            .required_contracts
            .into_iter()
            .map(str::to_string)
            .collect(),
        safe_auto_invocation_triggers: metadata.safe_auto_invocation_triggers,
        output_treatment: metadata.output_treatment,
        policy_refs: metadata
            .policy_refs
            .into_iter()
            .map(str::to_string)
            .collect(),
        adapters_must_not: metadata
            .adapters_must_not
            .into_iter()
            .map(str::to_string)
            .collect(),
    }
}

/// Returns `true` when any argv entry contains a shell control token
/// (`&&`, `||`, `;`, `|`, `` ` ``, `$(`, `>`, `<`). Used by the invocation
/// admission gate to reject command lines that smuggle shell metacharacters.
pub(crate) fn argv_has_shell_control(argv: &[String]) -> bool {
    argv.iter().any(|arg| {
        ["&&", "||", ";", "|", "`", "$(", ">", "<"]
            .iter()
            .any(|token| arg.contains(token))
    })
}

/// Returns `true` when an environment variable name carries a sensitive-looking
/// fragment (`TOKEN`, `SECRET`, `KEY`, `PASSWORD`). Used by the invocation
/// admission gate to forbid forwarding secret-bearing env keys to host
/// commands.
pub(crate) fn env_key_is_forbidden(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    ["TOKEN", "SECRET", "KEY", "PASSWORD"]
        .iter()
        .any(|pattern| upper.contains(pattern))
}

/// Returns `true` when `source_ref` contains a 40-character ASCII hex segment,
/// treating it as a git SHA-1-style immutable commit reference. Used by the
/// distribution admission and artifact-verification gates to require an
/// immutable source anchor.
pub(crate) fn source_ref_is_immutable(source_ref: &str) -> bool {
    source_ref
        .split(|character: char| !character.is_ascii_hexdigit())
        .any(|segment| segment.len() == 40 && segment.chars().all(|item| item.is_ascii_hexdigit()))
}

/// Returns `true` when `value` looks like a version string: non-empty after
/// trimming and composed only of `[A-Za-z0-9.\-+_]+`. Used by the distribution
/// admission and artifact-verification gates to validate version evidence.
pub(crate) fn version_like(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|item| item.is_ascii_alphanumeric() || matches!(item, '.' | '-' | '_' | '+'))
}

#[cfg(test)]
mod tests {
    use forge_core_command_surface::JsonMode;

    use super::*;

    #[test]
    fn host_command_is_anchored_in_command_surface() {
        let command = host_command(HostCommandMetadata {
            name: "validate",
            command_kind: HostAdapterCommandKind::Validation,
            mutation_class: HostAdapterMutationClass::ReadOnly,
            authority_class: HostAdapterAuthorityClass::NoWorkflowAuthority,
            required_contracts: vec![],
            safe_auto_invocation_triggers: vec![HostAdapterAutoTrigger::Diagnostics],
            output_treatment: vec![HostAdapterOutputTreatment::ValidationEvidence],
            policy_refs: vec![],
            adapters_must_not: vec![],
        });
        let surface = command_surface::command_by_name("validate").expect("validate metadata");
        assert_eq!(command.name, surface.name);
        assert_eq!(
            command.json_supported,
            matches!(surface.json_mode, JsonMode::EnvelopeOptional)
        );
    }
}
