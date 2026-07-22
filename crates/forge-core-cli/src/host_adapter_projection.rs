//! Host adapter manifest-derived surface builders.
//!
//! This module owns the five public builders that derive host-facing policy,
//! projection, and admission documents from the authoritative
//! [`crate::run_host_adapter_manifest`] output:
//!
//! - [`run_host_adapter_projection`] projects the manifest into MCP tools,
//!   borrowed-shell argv prefixes, or app-UI surface groups.
//! - [`run_host_adapter_process_security_policy`] derives the per-target
//!   process security policy (argv/cwd/env/stdio guardrails).
//! - [`run_host_adapter_invocation_admission`] decides whether a specific
//!   invocation request is allowed or blocked.
//! - [`run_host_adapter_distribution_policy`] emits the static distribution
//!   policy (channels, required evidence, updater rules).
//! - [`run_host_adapter_distribution_admission`] admits or blocks a
//!   distribution evidence bundle against the policy.
//!
//! The private helpers in this module (`project_host_command`,
//! `command_process_admission`, `projection_target_id`, `process_target_id`,
//! `mcp_tool_name`, `command_title`, `command_description`, `mcp_annotations`,
//! `command_input_schema`) are used only by the five public builders above.

use forge_core_command_surface as command_surface;
use forge_core_contracts::{ProjectionProhibition, RuntimeKind};
use serde_json::{json, Value};

use crate::host_adapter_manifest::run_host_adapter_manifest;
use crate::host_adapter_types::{
    HostAdapterAppUiProjection, HostAdapterArgvPolicy, HostAdapterBorrowedShellProjection,
    HostAdapterCommand, HostAdapterCommandKind, HostAdapterCommandProcessAdmission,
    HostAdapterCwdPolicy, HostAdapterDistributionAdmission, HostAdapterDistributionAdmissionStatus,
    HostAdapterDistributionChannelPolicy, HostAdapterDistributionEvidence,
    HostAdapterDistributionPolicy, HostAdapterDistributionRequiredEvidence, HostAdapterEnvPolicy,
    HostAdapterEvidenceProjection, HostAdapterInvocationAdmission,
    HostAdapterInvocationAdmissionStatus, HostAdapterInvocationRequest,
    HostAdapterMcpToolAnnotations, HostAdapterMcpToolProjection, HostAdapterMutationClass,
    HostAdapterProcessSecurityPolicy, HostAdapterProcessTarget, HostAdapterProjectedCommand,
    HostAdapterProjection, HostAdapterProjectionAuthorityBoundary, HostAdapterProjectionTarget,
    HostAdapterStdioPolicy, HostAdapterUpdateChannel, HostAdapterUpdaterPolicy,
};
use crate::host_command::{
    argv_has_shell_control, env_key_is_forbidden, source_ref_is_immutable, version_like,
};
use crate::valid_sha256_digest;

/// Build a host adapter projection for the requested target surface.
///
/// The projection adapts manifest command metadata for a specific host
/// surface (MCP tools, borrowed shell, or app UI) while preserving the
/// mutation class, authority class, and required contracts from the manifest.
#[must_use]
pub fn run_host_adapter_projection(target: HostAdapterProjectionTarget) -> HostAdapterProjection {
    let manifest = run_host_adapter_manifest();
    HostAdapterProjection {
        schema_version: "0.1".to_string(),
        projection_id: format!(
            "forge_core_host_adapter_projection_{}",
            projection_target_id(target)
        ),
        target,
        derived_from_manifest: manifest.manifest_id.clone(),
        projection_authoritative: false,
        authority_boundary: HostAdapterProjectionAuthorityBoundary {
            source_of_truth: "forge_core_host_adapter_manifest_v0".to_string(),
            projection_rule: "Projection may adapt metadata for a host surface, but the Rust manifest and validated Forge contracts remain authoritative.".to_string(),
            projected_metadata_must_preserve: vec![
                "canonical_usage".to_string(),
                "command_kind".to_string(),
                "mutation_class".to_string(),
                "authority_class".to_string(),
                "safe_auto_invocation_triggers".to_string(),
                "output_treatment".to_string(),
                "required_contracts".to_string(),
                "setup_gaps".to_string(),
            ],
            projections_must_not: ProjectionProhibition::ALL
                .into_iter()
                .map(projection_prohibition_text)
                .map(str::to_string)
                .collect(),
        },
        commands: manifest
            .commands
            .iter()
            .filter(|command| {
                target != HostAdapterProjectionTarget::McpTools
                    || command.mutation_class == HostAdapterMutationClass::ReadOnly
            })
            .map(|command| project_host_command(command, target))
            .collect(),
    }
}

/// Build the host adapter process security policy for the requested target.
///
/// The policy pins argv, cwd, env, and stdio guardrails for local process
/// execution and enumerates per-command admission decisions.
#[must_use]
pub fn run_host_adapter_process_security_policy(
    target: HostAdapterProcessTarget,
) -> HostAdapterProcessSecurityPolicy {
    let manifest = run_host_adapter_manifest();
    HostAdapterProcessSecurityPolicy {
        schema_version: "0.1".to_string(),
        policy_id: format!(
            "forge_core_host_adapter_process_security_{}",
            process_target_id(target)
        ),
        target,
        derived_from_manifest: manifest.manifest_id,
        default_admission: HostAdapterInvocationAdmissionStatus::Blocked,
        argv_policy: HostAdapterArgvPolicy {
            shell_strings_allowed: false,
            argv_must_start_with: vec!["forge-core".to_string()],
            unknown_commands_allowed: false,
        },
        cwd_policy: HostAdapterCwdPolicy {
            repo_root_scoped: true,
            outside_root_allowed_by_default: false,
        },
        env_policy: HostAdapterEnvPolicy {
            inherit_full_environment: false,
            allowed_keys: vec![
                "PATH".to_string(),
                "HOME".to_string(),
                "USERPROFILE".to_string(),
                "SYSTEMROOT".to_string(),
                "TEMP".to_string(),
                "TMP".to_string(),
            ],
            forbidden_key_patterns: vec![
                "TOKEN".to_string(),
                "SECRET".to_string(),
                "KEY".to_string(),
                "PASSWORD".to_string(),
            ],
        },
        stdio_policy: HostAdapterStdioPolicy {
            stdin_format: "json_arguments_only".to_string(),
            stdout_format: "json_result_only".to_string(),
            stderr_policy: "diagnostics_without_payload_bytes".to_string(),
            raw_payload_bytes_allowed: false,
        },
        command_admissions: manifest
            .commands
            .iter()
            .map(|command| command_process_admission(command, target))
            .collect(),
    }
}

/// Admit or block a specific host adapter invocation request.
///
/// The admission consults the manifest command metadata, the process
/// admission policy, and runtime guards (shell control tokens, forbidden
/// env keys, cwd escape) before returning an allowed/blocked verdict.
#[must_use]
pub fn run_host_adapter_invocation_admission(
    request: HostAdapterInvocationRequest,
) -> HostAdapterInvocationAdmission {
    let manifest = run_host_adapter_manifest();
    let Some(command) = manifest
        .commands
        .iter()
        .find(|command| command.name == request.command_name)
    else {
        return HostAdapterInvocationAdmission {
            status: HostAdapterInvocationAdmissionStatus::Blocked,
            command_name: request.command_name,
            target: request.target,
            mutation_class: None,
            authority_class: None,
            reasons: vec!["unknown_command".to_string()],
            required_controls: vec!["manifest_command_required".to_string()],
            allowed_argv_prefix: vec![],
        };
    };

    let process_admission = command_process_admission(command, request.target);
    let mut reasons = Vec::new();
    if request.target == HostAdapterProcessTarget::McpStdio
        && command.mutation_class == HostAdapterMutationClass::MutatingOperation
    {
        reasons.push("mcp_stdio_mutating_operation_deferred".to_string());
    }
    if process_admission.explicit_invocation_required && !request.explicit_invocation {
        reasons.push("explicit_invocation_required".to_string());
    }
    if argv_has_shell_control(&request.argv) {
        reasons.push("shell_control_token_rejected".to_string());
    }
    if request
        .env_keys
        .iter()
        .any(|key| env_key_is_forbidden(key.as_str()))
    {
        reasons.push("forbidden_environment_key".to_string());
    }
    if request.cwd.as_deref().is_some_and(|cwd| cwd.contains("..")) {
        reasons.push("cwd_escape_rejected".to_string());
    }

    HostAdapterInvocationAdmission {
        status: if reasons.is_empty() {
            HostAdapterInvocationAdmissionStatus::Allowed
        } else {
            HostAdapterInvocationAdmissionStatus::Blocked
        },
        command_name: command.name.clone(),
        target: request.target,
        mutation_class: Some(command.mutation_class),
        authority_class: Some(command.authority_class),
        reasons,
        required_controls: process_admission.required_controls,
        allowed_argv_prefix: vec!["forge-core".to_string(), command.name.clone()],
    }
}

/// Build the static host adapter distribution policy.
///
/// The policy enumerates supported runtime targets, default admission,
/// required distribution evidence, channel rules, and updater constraints.
#[must_use]
pub fn run_host_adapter_distribution_policy() -> HostAdapterDistributionPolicy {
    HostAdapterDistributionPolicy {
        schema_version: "0.1".to_string(),
        policy_id: "forge_core_host_adapter_distribution_policy_v0".to_string(),
        supported_runtime_targets: vec![
            RuntimeKind::Codex,
            RuntimeKind::Cursor,
            RuntimeKind::Claude,
            RuntimeKind::Opencode,
            RuntimeKind::Vscode,
            RuntimeKind::Pidev,
            RuntimeKind::ForgeStandalone,
            RuntimeKind::Custom,
        ],
        default_admission: HostAdapterDistributionAdmissionStatus::Blocked,
        required_evidence: HostAdapterDistributionRequiredEvidence {
            immutable_source_ref: true,
            artifact_checksum_or_signature: true,
            provenance_ref: true,
            version_compatibility: true,
            rollback_ref: true,
        },
        channel_policy: HostAdapterDistributionChannelPolicy {
            stable_allowed: true,
            canary_allowed_with_explicit_opt_in: true,
            dev_allowed_for_general_install: false,
        },
        updater_policy: HostAdapterUpdaterPolicy {
            update_summary_required: true,
            rollback_metadata_required: true,
            preserve_local_project_state: true,
            self_update_may_bypass_admission: false,
        },
    }
}

/// Admit or block a distribution evidence bundle against the distribution policy.
///
/// The admission checks artifact name, channel rules, immutable source ref,
/// checksum/signature, provenance ref, version compatibility, rollback ref,
/// and update summary ref before returning an allowed/blocked verdict.
pub fn run_host_adapter_distribution_admission(
    evidence: HostAdapterDistributionEvidence,
) -> HostAdapterDistributionAdmission {
    let policy = run_host_adapter_distribution_policy();
    let mut reasons = Vec::new();
    let mut accepted_evidence = Vec::new();

    if evidence.artifact_name.trim().is_empty() {
        reasons.push("artifact_name_required".to_string());
    } else {
        accepted_evidence.push("artifact_name".to_string());
    }

    match evidence.channel {
        HostAdapterUpdateChannel::Stable => accepted_evidence.push("stable_channel".to_string()),
        HostAdapterUpdateChannel::Canary => {
            if evidence.explicit_canary_opt_in {
                accepted_evidence.push("canary_explicit_opt_in".to_string());
            } else {
                reasons.push("canary_requires_explicit_opt_in".to_string());
            }
        }
        HostAdapterUpdateChannel::Dev => {
            reasons.push("dev_channel_not_for_general_install".to_string());
        }
    }

    match evidence.source_ref.as_deref() {
        Some(source_ref) if source_ref_is_immutable(source_ref) => {
            accepted_evidence.push("immutable_source_ref".to_string());
        }
        Some(_) => reasons.push("immutable_source_ref_required".to_string()),
        None => reasons.push("source_ref_required".to_string()),
    }

    let has_valid_checksum = evidence
        .artifact_sha256
        .as_deref()
        .is_some_and(valid_sha256_digest);
    let has_signature = evidence
        .signature_ref
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    if has_valid_checksum || has_signature {
        accepted_evidence.push("artifact_checksum_or_signature".to_string());
    } else {
        reasons.push("artifact_checksum_or_signature_required".to_string());
    }

    if evidence
        .provenance_ref
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        accepted_evidence.push("provenance_ref".to_string());
    } else {
        reasons.push("provenance_ref_required".to_string());
    }

    if evidence.version.as_deref().is_some_and(version_like)
        && evidence
            .compatible_core_version
            .as_deref()
            .is_some_and(version_like)
    {
        accepted_evidence.push("version_compatibility".to_string());
    } else {
        reasons.push("version_compatibility_required".to_string());
    }

    if evidence
        .rollback_ref
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        accepted_evidence.push("rollback_ref".to_string());
    } else {
        reasons.push("rollback_ref_required".to_string());
    }

    if evidence
        .update_summary_ref
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        accepted_evidence.push("update_summary_ref".to_string());
    } else {
        reasons.push("update_summary_ref_required".to_string());
    }

    HostAdapterDistributionAdmission {
        status: if reasons.is_empty() {
            HostAdapterDistributionAdmissionStatus::Allowed
        } else {
            HostAdapterDistributionAdmissionStatus::Blocked
        },
        target: evidence.target,
        channel: evidence.channel,
        artifact_name: evidence.artifact_name,
        reasons,
        required_evidence: policy.required_evidence,
        accepted_evidence,
    }
}

/// Derive the per-command process admission for a target.
///
/// Read-only commands may be auto-invoked; mutating commands require explicit
/// invocation; MCP stdio mutating operations are deferred to the future
/// installer trust boundary.
fn command_process_admission(
    command: &HostAdapterCommand,
    target: HostAdapterProcessTarget,
) -> HostAdapterCommandProcessAdmission {
    let mut required_controls = vec![
        "manifest_authority_classes_preserved".to_string(),
        "json_schema_arguments".to_string(),
        "repo_root_scoped_cwd".to_string(),
        "minimal_environment".to_string(),
    ];
    if command.mutation_class != HostAdapterMutationClass::ReadOnly {
        required_controls.push("explicit_human_or_driver_invocation".to_string());
    }
    if target == HostAdapterProcessTarget::McpStdio
        && command.mutation_class == HostAdapterMutationClass::MutatingOperation
    {
        required_controls.push("future_installer_trust_boundary_required".to_string());
    }

    HostAdapterCommandProcessAdmission {
        command_name: command.name.clone(),
        mutation_class: command.mutation_class,
        authority_class: command.authority_class,
        automatic_invocation_allowed: command.mutation_class == HostAdapterMutationClass::ReadOnly,
        explicit_invocation_required: command.mutation_class != HostAdapterMutationClass::ReadOnly,
        mcp_stdio_enabled: !(target == HostAdapterProcessTarget::McpStdio
            && command.mutation_class == HostAdapterMutationClass::MutatingOperation),
        required_controls,
    }
}

/// Project a single manifest command into a host-surface-specific command view.
fn project_host_command(
    command: &HostAdapterCommand,
    target: HostAdapterProjectionTarget,
) -> HostAdapterProjectedCommand {
    let surface = command_surface::command_by_name(&command.name).unwrap_or_else(|| {
        panic!(
            "host adapter command '{}' is missing from forge-core-command-surface",
            command.name
        )
    });
    let title = command_title(&command.name);
    let description = command_description(command);
    HostAdapterProjectedCommand {
        name: command.name.clone(),
        source_command: command.name.clone(),
        canonical_usage: surface.canonical_usage().trim_start().to_string(),
        title: title.clone(),
        description: description.clone(),
        command_kind: command.command_kind,
        mutation_class: command.mutation_class,
        authority_class: command.authority_class,
        required_contracts: command.required_contracts.clone(),
        setup_gaps: command.setup_gaps.clone(),
        evidence_projection: if command.name == "host-adapter-verify-certificate-ocsp-status" {
            vec![
                HostAdapterEvidenceProjection::OcspResponderAuthorityIdentity,
                HostAdapterEvidenceProjection::OcspVerifiedAuthorityEvidence,
            ]
        } else {
            Vec::new()
        },
        safe_auto_invocation_triggers: command.safe_auto_invocation_triggers.clone(),
        output_treatment: command.output_treatment.clone(),
        mcp_tool: (target == HostAdapterProjectionTarget::McpTools).then(|| {
            HostAdapterMcpToolProjection {
                name: mcp_tool_name(&command.name),
                title: title.clone(),
                description: description.clone(),
                input_schema: command_input_schema(&command.name),
                annotations: mcp_annotations(command, &title),
            }
        }),
        borrowed_shell: (target == HostAdapterProjectionTarget::BorrowedShell).then(|| {
            HostAdapterBorrowedShellProjection {
                argv_prefix: vec!["forge-core".to_string(), command.name.clone()],
                json_flag: "--json".to_string(),
                explicit_invocation_required: command.mutation_class
                    != HostAdapterMutationClass::ReadOnly,
                may_auto_invoke: command.mutation_class == HostAdapterMutationClass::ReadOnly,
            }
        }),
        app_ui: (target == HostAdapterProjectionTarget::AppUi).then(|| {
            HostAdapterAppUiProjection {
                surface_group: match command.command_kind {
                    HostAdapterCommandKind::Validation => "validation".to_string(),
                    HostAdapterCommandKind::OperationExecution => "runtime_execution".to_string(),
                    HostAdapterCommandKind::OperationalRepair => "maintenance".to_string(),
                    HostAdapterCommandKind::AdvisoryLookup => "advisory_context".to_string(),
                    HostAdapterCommandKind::CapabilityManifest => {
                        "capability_discovery".to_string()
                    }
                },
                confirmation_required: command.mutation_class
                    == HostAdapterMutationClass::MutatingOperation,
                display_authority_badge: format!("{:?}", command.authority_class),
            }
        }),
    }
}

fn projection_prohibition_text(prohibition: ProjectionProhibition) -> &'static str {
    match prohibition {
        ProjectionProhibition::NetworkRetrievalAuthority => "perform network retrieval",
        ProjectionProhibition::InstallAuthority => "grant install authority",
        ProjectionProhibition::UpdateAuthority => "grant update authority",
        ProjectionProhibition::CrlAuthority => "grant CRL authority",
        ProjectionProhibition::CertificateTransparencyAuthority => {
            "grant Certificate Transparency authority"
        }
        ProjectionProhibition::RekorAuthority => "grant Rekor authority",
        ProjectionProhibition::TufAuthority => "grant TUF authority",
        ProjectionProhibition::SigningAuthority => "grant signing authority",
        ProjectionProhibition::MutationAuthority => "grant mutation authority",
        ProjectionProhibition::HostSelection => "select a host",
        ProjectionProhibition::HostSupportClaim => "claim host support",
        ProjectionProhibition::HostReleaseClaim => "claim a host release",
        ProjectionProhibition::ProjectionAuthorityPromotion => {
            "promote projection metadata into authority"
        }
    }
}

fn projection_target_id(target: HostAdapterProjectionTarget) -> &'static str {
    match target {
        HostAdapterProjectionTarget::McpTools => "mcp_tools",
        HostAdapterProjectionTarget::BorrowedShell => "borrowed_shell",
        HostAdapterProjectionTarget::AppUi => "app_ui",
    }
}

fn process_target_id(target: HostAdapterProcessTarget) -> &'static str {
    match target {
        HostAdapterProcessTarget::McpStdio => "mcp_stdio",
        HostAdapterProcessTarget::BorrowedShell => "borrowed_shell",
        HostAdapterProcessTarget::AppBridge => "app_bridge",
    }
}

fn mcp_tool_name(name: &str) -> String {
    format!("forge_core_{}", name.replace('-', "_"))
}

fn command_title(name: &str) -> String {
    match name {
        "validate" => "Validate Forge contracts",
        "execute-operation" => "Execute Forge operation",
        "rebuild-effect-index" => "Rebuild effect metadata index",
        "query-effect-index" => "Query effect metadata index",
        "host-adapter-manifest" => "Read host adapter manifest",
        "host-adapter-projection" => "Read host adapter projection",
        "host-adapter-verify-artifact" => "Verify host adapter artifact",
        "host-adapter-verify-provenance" => "Verify host adapter provenance",
        "host-adapter-verify-rekor-entry" => "Verify Rekor log entry",
        "host-adapter-verify-sigstore-trust-policy" => "Verify Sigstore trust policy",
        "host-adapter-verify-certificate-transparency-sct" => "Verify certificate transparency SCT",
        "host-adapter-verify-certificate-revocation-policy" => {
            "Verify certificate revocation policy"
        }
        "host-adapter-verify-tuf-trusted-root-freshness" => "Verify TUF trusted-root freshness",
        "host-adapter-verify-certificate-crl-status" => "Verify certificate CRL status",
        "host-adapter-verify-certificate-ocsp-status" => "Verify certificate OCSP status",
        _ => "Forge Core command",
    }
    .to_string()
}

fn command_description(command: &HostAdapterCommand) -> String {
    match command.name.as_str() {
        "validate" => "Read-only contract validation evidence for a Forge Core workspace.",
        "execute-operation" => {
            "Mutating runtime execution path admitted only by validated OperationContract inputs."
        }
        "rebuild-effect-index" => {
            "Operational repair that rebuilds append-only effect metadata from committed WAL records."
        }
        "query-effect-index" => {
            "Read-only advisory metadata lookup for evidence discovery, diagnostics, or handoff context."
        }
        "host-adapter-manifest" => {
            "Read-only source manifest for host command authority and mutation metadata."
        }
        "host-adapter-projection" => {
            "Read-only derived projection for MCP, borrowed-shell, or app surfaces."
        }
        "host-adapter-verify-artifact" => {
            "Read-only local verification that artifact bytes and required distribution metadata match before install/update mutation."
        }
        "host-adapter-verify-provenance" => {
            "Read-only cryptographic and semantic verification for signed SLSA/in-toto provenance before install/update mutation."
        }
        "host-adapter-verify-rekor-entry" => {
            "Read-only verification that a Rekor log entry has a signed checkpoint and valid Merkle inclusion proof under the expected Rekor key."
        }
        "host-adapter-verify-sigstore-trust-policy" => {
            "Read-only verification that Forge has explicit Sigstore trusted-root, identity, and timestamp policy before deeper bundle/Fulcio verification."
        }
        "host-adapter-verify-certificate-transparency-sct" => {
            "Read-only offline verification that supplied SCT bytes are signed by policy-declared Certificate Transparency logs for a supplied certificate."
        }
        "host-adapter-verify-certificate-revocation-policy" => {
            "Read-only policy verification for Sigstore-style short-lived certificate revocation strategy without claiming CRL or OCSP status."
        }
        "host-adapter-verify-tuf-trusted-root-freshness" => {
            "Read-only freshness verification for supplied TUF trusted-root metadata and optional top-level metadata without claiming signature or update authority."
        }
        "host-adapter-verify-certificate-crl-status" => {
            "Read-only explicit CRL revocation status verification for a supplied certificate, issuer certificate, and local CRL without claiming OCSP or update authority."
        }
        "host-adapter-verify-certificate-ocsp-status" => {
            "Read-only offline verification of a supplied OCSP response for a supplied certificate and issuer certificate, optionally using a strictly supplied delegated responder certificate and ordered issuer chain; projects typed selected-authority identity and evidence without network, signing, mutation, install, or update authority."
        }
        _ => "Forge Core command projection.",
    }
    .to_string()
}

fn mcp_annotations(command: &HostAdapterCommand, title: &str) -> HostAdapterMcpToolAnnotations {
    let read_only = command.mutation_class == HostAdapterMutationClass::ReadOnly;
    HostAdapterMcpToolAnnotations {
        title: title.to_string(),
        read_only_hint: read_only,
        destructive_hint: command.mutation_class == HostAdapterMutationClass::MutatingOperation,
        idempotent_hint: matches!(
            command.command_kind,
            HostAdapterCommandKind::Validation
                | HostAdapterCommandKind::AdvisoryLookup
                | HostAdapterCommandKind::CapabilityManifest
        ),
        open_world_hint: false,
    }
}

fn command_input_schema(name: &str) -> Value {
    match name {
        "validate" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "root": { "type": "string" }
            }
        }),
        "execute-operation" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["operation"],
            "properties": {
                "root": { "type": "string" },
                "operation": { "type": "string" },
                "command": { "type": "array", "items": { "type": "string" } },
                "effect": { "type": "array", "items": { "type": "string" } },
                "payload": { "type": "array", "items": { "type": "string" } },
                "max_payload_bytes": { "type": "integer", "minimum": 0 },
                "allow_payload_outside_root": { "type": "boolean" },
                "recorded_at": { "type": "string" },
                "tx_id_prefix": { "type": "string" }
            }
        }),
        "rebuild-effect-index" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "root": { "type": "string" },
                "wal": { "type": "string" },
                "index": { "type": "string" },
                "lock": { "type": "string" },
                "recorded_at": { "type": "string" }
            }
        }),
        "query-effect-index" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "root": { "type": "string" },
                "index": { "type": "string" },
                "logical_ref": { "type": "string" },
                "effect_id": { "type": "string" },
                "operation_id": { "type": "string" },
                "target_kind": { "type": "string", "enum": ["file_path", "glob", "state_key", "artifact_id", "evidence_id", "ledger_stream", "request_stream", "completion_id"] },
                "consumer_use": { "type": "string", "enum": ["discovery", "diagnostics", "handoff_context"] },
                "context": { "type": "boolean" },
                "max_context_groups": { "type": "integer", "minimum": 0 },
                "adapter_kind": { "type": "string", "enum": ["codex", "cursor", "claude", "opencode", "vscode", "pidev", "forge_standalone", "custom"] },
                "adapter_trigger": { "type": "string", "enum": ["evidence_discovery", "diagnostics", "handoff_preparation", "manual_inspection"] },
                "latest": { "type": "boolean" }
            }
        }),
        "host-adapter-projection" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["target"],
            "properties": {
                "target": { "type": "string", "enum": ["mcp_tools", "borrowed_shell", "app_ui"] }
            }
        }),
        "host-adapter-process-policy" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "target": { "type": "string", "enum": ["mcp_stdio", "borrowed_shell", "app_bridge"] }
            }
        }),
        "host-adapter-admit-invocation" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["command", "target"],
            "properties": {
                "command": { "type": "string" },
                "target": { "type": "string", "enum": ["mcp_stdio", "borrowed_shell", "app_bridge"] },
                "explicit": { "type": "boolean" },
                "argv": { "type": "array", "items": { "type": "string" } },
                "cwd": { "type": "string" },
                "env_key": { "type": "array", "items": { "type": "string" } }
            }
        }),
        "host-adapter-admit-distribution" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["target", "channel", "artifact"],
            "properties": {
                "target": { "type": "string", "enum": ["codex", "cursor", "claude", "opencode", "vscode", "pidev", "forge_standalone", "custom"] },
                "channel": { "type": "string", "enum": ["stable", "canary", "dev"] },
                "artifact": { "type": "string" },
                "sha256": { "type": "string" },
                "signature_ref": { "type": "string" },
                "provenance_ref": { "type": "string" },
                "source_ref": { "type": "string" },
                "version": { "type": "string" },
                "compatible_core_version": { "type": "string" },
                "rollback_ref": { "type": "string" },
                "update_summary_ref": { "type": "string" },
                "explicit_canary_opt_in": { "type": "boolean" }
            }
        }),
        "host-adapter-verify-artifact" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["artifact_path", "sha256"],
            "properties": {
                "artifact_path": { "type": "string" },
                "sha256": { "type": "string" },
                "signature_ref": { "type": "string" },
                "provenance_ref": { "type": "string" },
                "source_ref": { "type": "string" },
                "version": { "type": "string" },
                "compatible_core_version": { "type": "string" },
                "rollback_ref": { "type": "string" },
                "update_summary_ref": { "type": "string" }
            }
        }),
        "host-adapter-verify-provenance" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["artifact_path", "provenance_path", "signature_path", "public_key_path", "transparency_log_path", "sha256", "expected_builder_id", "expected_source_uri", "expected_source_ref"],
            "properties": {
                "artifact_path": { "type": "string" },
                "provenance_path": { "type": "string" },
                "signature_path": { "type": "string" },
                "public_key_path": { "type": "string" },
                "transparency_log_path": { "type": "string" },
                "sha256": { "type": "string" },
                "expected_builder_id": { "type": "string" },
                "expected_source_uri": { "type": "string" },
                "expected_source_ref": { "type": "string" }
            }
        }),
        "host-adapter-verify-rekor-entry" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["log_entry_path", "public_key_path", "expected_log_id"],
            "properties": {
                "log_entry_path": { "type": "string" },
                "public_key_path": { "type": "string" },
                "expected_log_id": { "type": "string" }
            }
        }),
        "host-adapter-verify-sigstore-trust-policy" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["policy_path"],
            "properties": {
                "policy_path": { "type": "string" }
            }
        }),
        "host-adapter-verify-certificate-transparency-sct" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["trust_policy_path", "certificate_path", "sct_path", "verification_time_unix_ms"],
            "properties": {
                "trust_policy_path": { "type": "string" },
                "certificate_path": { "type": "string" },
                "sct_path": { "type": "array", "items": { "type": "string" } },
                "verification_time_unix_ms": { "type": "integer", "minimum": 0 }
            }
        }),
        "host-adapter-verify-certificate-revocation-policy" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["trust_policy_path", "certificate_path", "trusted_signing_time_unix"],
            "properties": {
                "trust_policy_path": { "type": "string" },
                "certificate_path": { "type": "string" },
                "trusted_signing_time_unix": { "type": "integer" }
            }
        }),
        "host-adapter-verify-tuf-trusted-root-freshness" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["trust_policy_path", "root_metadata_path", "update_start_time_unix"],
            "properties": {
                "trust_policy_path": { "type": "string" },
                "root_metadata_path": { "type": "string" },
                "timestamp_metadata_path": { "type": "string" },
                "snapshot_metadata_path": { "type": "string" },
                "targets_metadata_path": { "type": "string" },
                "update_start_time_unix": { "type": "integer" },
                "min_root_version": { "type": "integer" },
                "min_timestamp_version": { "type": "integer" },
                "min_snapshot_version": { "type": "integer" },
                "min_targets_version": { "type": "integer" }
            }
        }),
        "host-adapter-verify-certificate-crl-status" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["trust_policy_path", "certificate_path", "issuer_certificate_path", "crl_path", "verification_time_unix"],
            "properties": {
                "trust_policy_path": { "type": "string" },
                "certificate_path": { "type": "string" },
                "issuer_certificate_path": { "type": "string" },
                "crl_path": { "type": "string" },
                "verification_time_unix": { "type": "integer" }
            }
        }),
        "host-adapter-verify-certificate-ocsp-status" => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["trust_policy_path", "certificate_path", "issuer_certificate_path", "ocsp_response_path", "verification_time_unix"],
            "dependentRequired": {
                "ocsp_responder_issuer_certificate_path": ["ocsp_responder_certificate_path"]
            },
            "properties": {
                "trust_policy_path": { "type": "string" },
                "certificate_path": { "type": "string" },
                "issuer_certificate_path": { "type": "string" },
                "ocsp_response_path": { "type": "string" },
                "verification_time_unix": { "type": "integer" },
                "expected_nonce_hex": { "type": "string" },
                "ocsp_responder_certificate_path": { "type": "string", "minLength": 1 },
                "ocsp_responder_issuer_certificate_path": { "type": "array", "items": { "type": "string", "minLength": 1 } }
            }
        }),
        _ => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        }),
    }
}
