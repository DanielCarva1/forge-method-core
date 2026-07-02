//! Execute-operation pipeline.
//!
//! Pure path/payload resolution layer that lives between the CLI entrypoint
//! (`run_execute_operation_command` in `main.rs`) and the runtime executor
//! (`forge_core_kernel::execute_operation`).
//!
//! Responsibilities:
//! - Resolve operation/command/effect contract paths against the project root,
//!   enforcing that all of them stay inside `--root` (provenance boundary).
//! - Load runtime payloads from disk with size and scope enforcement
//!   (`PayloadLoadPolicy`).
//! - Translate the lossy `forge_core_store::EffectStoreLockError` /
//!   `ReferenceIndexBuildError` strings into a typed
//!   [`ExecuteOperationError`].
//!
//! This module is `pub(crate)`: it is re-exported from the crate root so that
//! the binary entrypoint (`main.rs`) and the integration tests in
//! `tests/validate.rs` keep importing `ExecuteOperationInput`,
//! `PayloadFileSpec`, `PayloadLoadPolicy`, `run_execute_operation` from
//! `forge_core_cli`.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use forge_core_contracts::{
    CliEnvelope, CommandContractDocument, ExitReason, FieldEvidenceRegistry,
    OperationContractDocument, RepoPath, ToolEffectContractDocument, TypedFailure,
};
use forge_core_kernel::{
    execute_operation, GateRejection, RuntimeEffectPayloadKind, RuntimeOperationCommandInput,
    RuntimeOperationEffectInput, RuntimeOperationEffectPayload, RuntimeOperationExecution,
    RuntimeOperationExecutionContext, RuntimeReadSnapshot,
};
use forge_core_store::{
    append_trace_event, build_reference_index, collect_validation_yaml_documents, WalDurability,
};
use forge_core_validate::risk_audit::{
    evaluate_risk_audit, validate_risk_audit_rule_set, RiskAuditRuleSet,
};
use forge_core_validate::validate_yaml_citation_references;

use crate::cli_error::ExitError;
use crate::cli_util::{
    next_arg_or_err, next_path_or_err, parse_payload_arg_or_err, parse_u64_or_err,
    resolve_stateful_roots_or_err, usage,
};
use crate::hex_sha256;

/// All inputs required to drive one operation execution.
///
/// Built by `main.rs::run_execute_operation_command` from CLI arguments and
/// handed wholesale to [`run_execute_operation`].
#[derive(Debug, Clone)]
pub struct ExecuteOperationInput {
    pub root: PathBuf,
    pub effect_store_root: Option<PathBuf>,
    pub operation_path: PathBuf,
    pub command_paths: Vec<PathBuf>,
    pub effect_paths: Vec<PathBuf>,
    pub payloads: Vec<PayloadFileSpec>,
    pub payload_policy: PayloadLoadPolicy,
    pub recorded_at: String,
    pub tx_id_prefix: String,
    /// WAL durability for this run (ADR-0009). Default `SyncOnAppend`;
    /// CLI sets `NoSync` when the user passes `--no-sync`.
    pub durability: WalDurability,
    /// F11.3: optional risk-audit gate. When `Some`, the rule set YAML is
    /// loaded and evaluated against the project tree BEFORE any WAL write.
    /// Fail-closed via `ExecuteOperationError::RiskAuditFailed` on errors.
    pub risk_audit_rules: Option<PathBuf>,
    /// F14.6: optional citation gate. When `true`, the Citation Check
    /// (`validate_yaml_citation_references`) runs over the workspace YAML
    /// BEFORE any WAL write, resolving `source_id`s against the curated
    /// Field Evidence Registry ∪ the runtime Source Ledger. Fail-closed via
    /// `ExecuteOperationError::CitationCheckFailed`. Opt-in: the default
    /// (`false`) preserves existing execute-operation behaviour.
    pub require_citation: bool,
}

/// One payload bound for one `target_ref` inside the operation.
#[derive(Debug, Clone)]
pub struct PayloadFileSpec {
    pub target_ref: String,
    pub path: PathBuf,
}

/// Defensive policy for loading runtime payload bytes from disk.
///
/// `max_payload_bytes` is a hard cap; `allow_outside_root` relaxes the
/// "payload must live under `--root`" rule for trusted callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadLoadPolicy {
    pub max_payload_bytes: u64,
    pub allow_outside_root: bool,
}

impl Default for PayloadLoadPolicy {
    fn default() -> Self {
        Self {
            max_payload_bytes: 1_048_576,
            allow_outside_root: false,
        }
    }
}

/// Discriminates which kind of contract path triggered a "outside root"
/// error so the message can point to the right CLI flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteOperationContractPathKind {
    Operation,
    Command,
    Effect,
}

impl ExecuteOperationContractPathKind {
    const fn label(self) -> &'static str {
        match self {
            ExecuteOperationContractPathKind::Operation => "operation contract path",
            ExecuteOperationContractPathKind::Command => "command contract path",
            ExecuteOperationContractPathKind::Effect => "effect contract path",
        }
    }
}

/// Hand-rolled error enum (no `thiserror`) for the execute-operation flow.
#[derive(Debug)]
pub enum ExecuteOperationError {
    ReferenceIndexBuild(String),
    ReadFile {
        path: PathBuf,
        source: io::Error,
    },
    ParseYaml {
        path: PathBuf,
        source: yaml_serde::Error,
    },
    InvalidEffectPath {
        root: PathBuf,
        path: PathBuf,
    },
    ContractPathOutsideRoot {
        kind: ExecuteOperationContractPathKind,
        root: PathBuf,
        path: PathBuf,
    },
    PayloadPathOutsideRoot {
        root: PathBuf,
        path: PathBuf,
    },
    PayloadTooLarge {
        path: PathBuf,
        byte_len: u64,
        max_payload_bytes: u64,
    },
    /// F11.3: risk-audit gate failed closed. `error_count` is the total of
    /// Error-severity findings; `first_error` is path+message of the first
    /// for quick context. The CLI may print the full report separately.
    RiskAuditFailed {
        error_count: usize,
        first_error: String,
    },
    /// F14.6: citation gate failed closed. The Citation Check
    /// (`validate_yaml_citation_references`) reported `error_count`
    /// `UnresolvedSourceId` diagnostics; `first_error` is path+message of the
    /// first. Nothing is persisted to the WAL when this fires — citing an
    /// unregistered source is a precondition failure, not a post-hoc finding.
    CitationCheckFailed {
        error_count: usize,
        first_error: String,
    },
    /// V2.C: a kernel-internal [`OperationGate`] rejected the mutation before
    /// any WAL append. Carries the typed kernel rejection verbatim. Nothing is
    /// persisted when this fires. The CLI's own risk-audit/citation gates
    /// (`RiskAuditFailed` / `CitationCheckFailed`) still run first in the CLI
    /// flow; this variant covers gates that run inside the kernel
    /// (V3.A will attach real gates there).
    GateRejected {
        rejection: GateRejection,
    },
}

impl fmt::Display for ExecuteOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecuteOperationError::ReferenceIndexBuild(message) => {
                write!(formatter, "reference index build failed: {message}")
            }
            ExecuteOperationError::ReadFile { path, source } => {
                write!(formatter, "read {} failed: {source}", path.display())
            }
            ExecuteOperationError::ParseYaml { path, source } => {
                write!(formatter, "parse {} failed: {source}", path.display())
            }
            ExecuteOperationError::InvalidEffectPath { root, path } => write!(
                formatter,
                "effect path {} is not under root {}",
                path.display(),
                root.display()
            ),
            ExecuteOperationError::ContractPathOutsideRoot { kind, root, path } => write!(
                formatter,
                "{} {} is outside project root {}; pass a path under --root so operation provenance stays within one project",
                kind.label(),
                path.display(),
                root.display()
            ),
            ExecuteOperationError::PayloadPathOutsideRoot { root, path } => write!(
                formatter,
                "payload path {} is outside root {}",
                path.display(),
                root.display()
            ),
            ExecuteOperationError::PayloadTooLarge {
                path,
                byte_len,
                max_payload_bytes,
            } => write!(
                formatter,
                "payload {} is too large: {byte_len} bytes > {max_payload_bytes} bytes",
                path.display()
            ),
            ExecuteOperationError::RiskAuditFailed {
                error_count,
                first_error,
            } => write!(
                formatter,
                "risk-audit gate failed with {error_count} error(s); first: {first_error}"
            ),
            ExecuteOperationError::CitationCheckFailed {
                error_count,
                first_error,
            } => write!(
                formatter,
                "citation gate failed with {error_count} unresolved source_id(s); first: {first_error}"
            ),
            ExecuteOperationError::GateRejected { rejection } => {
                write!(formatter, "mutation gate rejected: {rejection}")
            }
        }
    }
}

impl std::error::Error for ExecuteOperationError {}

/// Fold an [`ExecuteOperationError`] into the envelope-level
/// [`TypedFailure`] (V2.D).
///
/// This is a lossy-but-typed mapping: the kernel's typed `GateRejection`
/// stays in the kernel; the envelope carries only the stringified rejection
/// (or a more specific variant when the kernel variant is one we recognise).
/// `&self` is used (not a consuming `From`) so the caller can still format the
/// original error for the human `error.message`.
impl From<&ExecuteOperationError> for TypedFailure {
    fn from(error: &ExecuteOperationError) -> Self {
        match error {
            ExecuteOperationError::ReferenceIndexBuild(message) => Self::StoreError {
                path: String::new(),
                source: message.clone(),
            },
            ExecuteOperationError::ReadFile { path, source } => Self::StoreError {
                path: path.to_string_lossy().into_owned(),
                source: source.to_string(),
            },
            ExecuteOperationError::ParseYaml { path, source } => Self::InvalidContract {
                reasons: vec![format!("{}: parse failed: {source}", path.display())],
            },
            ExecuteOperationError::InvalidEffectPath { path, .. } => {
                Self::ContractPathOutsideRoot {
                    path: format!("effect path {}", path.display()),
                }
            }
            ExecuteOperationError::ContractPathOutsideRoot { kind, path, .. } => {
                Self::ContractPathOutsideRoot {
                    path: format!("{} {}", kind.label(), path.display()),
                }
            }
            ExecuteOperationError::PayloadPathOutsideRoot { path, .. } => {
                Self::PayloadOutsideRoot {
                    path: path.to_string_lossy().into_owned(),
                }
            }
            ExecuteOperationError::PayloadTooLarge {
                path,
                max_payload_bytes,
                ..
            } => Self::PayloadTooLarge {
                path: path.to_string_lossy().into_owned(),
                max_bytes: *max_payload_bytes,
            },
            ExecuteOperationError::RiskAuditFailed {
                error_count,
                first_error,
            } => Self::RiskAuditFailed {
                error_count: *error_count,
                // The CLI error only carries the first error's text; surface it
                // as the single finding path. (V3 may widen the source to a
                // full report.)
                finding_paths: vec![first_error.clone()],
            },
            ExecuteOperationError::CitationCheckFailed { first_error, .. } => {
                Self::CitationCheckFailed {
                    // The CLI error does not retain the unresolved source_id list,
                    // only the first diagnostic text. Carry it so the consumer has
                    // something to act on; a V3 widening can replace it with the
                    // actual id set.
                    unresolved_source_ids: vec![first_error.clone()],
                }
            }
            ExecuteOperationError::GateRejected { rejection } => match rejection {
                // Map recognised kernel gate variants to the envelope's more
                // specific typed variants, so a consumer can branch without
                // re-parsing the stringified rejection. Falls back to the
                // generic GateRejected { rejection } string for Custom.
                GateRejection::RiskAuditFailed {
                    error_count,
                    finding_paths,
                } => Self::RiskAuditFailed {
                    error_count: *error_count,
                    finding_paths: finding_paths.clone(),
                },
                GateRejection::CitationCheckFailed {
                    unresolved_source_ids,
                } => Self::CitationCheckFailed {
                    unresolved_source_ids: unresolved_source_ids.clone(),
                },
                GateRejection::Custom { .. } => Self::GateRejected {
                    rejection: rejection.to_string(),
                },
            },
        }
    }
}

/// Map an [`ExecuteOperationError`] to the envelope [`ExitReason`] (V2.D).
///
/// Pre-V2.D the whole flow collapsed to exit code 1 (`ExitError::failed`). The
/// typed mapping restores the DD10 taxonomy: gate failures (risk-audit /
/// citation / kernel `GateRejected`) surface as `RejectedByGate` (2); input
/// shape problems (paths outside root, malformed YAML) surface as
/// `InvalidDecisionShape` (3); IO/store failures surface as `EnvConfig` (5).
/// `Conflict` (4) is reserved for WAL/integrity conflicts, which this layer
/// does not produce.
fn exit_reason_for(error: &ExecuteOperationError) -> ExitReason {
    match error {
        ExecuteOperationError::RiskAuditFailed { .. }
        | ExecuteOperationError::CitationCheckFailed { .. }
        | ExecuteOperationError::GateRejected { .. } => ExitReason::RejectedByGate,
        ExecuteOperationError::ContractPathOutsideRoot { .. }
        | ExecuteOperationError::PayloadPathOutsideRoot { .. }
        | ExecuteOperationError::PayloadTooLarge { .. }
        | ExecuteOperationError::InvalidEffectPath { .. }
        | ExecuteOperationError::ParseYaml { .. } => ExitReason::InvalidDecisionShape,
        ExecuteOperationError::ReferenceIndexBuild(_) | ExecuteOperationError::ReadFile { .. } => {
            ExitReason::EnvConfig
        }
    }
}

/// Drive one operation execution end-to-end.
///
/// Resolves all paths against `root`, builds the reference index, loads
/// contract documents and runtime payloads, then hands everything off to
/// [`forge_core_kernel::execute_operation`].
///
/// # Errors
///
/// Returns [`ExecuteOperationError`] if any contract path falls outside
/// `--root`, the reference index cannot be built, a YAML contract fails to
/// read or parse, a payload path falls outside the allowed scope, or a
/// payload exceeds [`PayloadLoadPolicy::max_payload_bytes`].
pub fn run_execute_operation(
    input: ExecuteOperationInput,
) -> Result<RuntimeOperationExecution, ExecuteOperationError> {
    let root = input.root;
    let effect_store_root = input.effect_store_root.unwrap_or_else(|| root.clone());
    let canonical_root = canonicalize_existing_path(&root)?;
    // F11.3: Risk Audit Gate. Run as the FIRST step after root canonicalization,
    // before any contract parse or WAL write. Auditing is a precondition for
    // mutation, not a post-parse check, so the gate never depends on the
    // operation/command/effect contracts being valid. Fail-closed: nothing is
    // persisted to the WAL if the rule set is structurally invalid or any
    // Error-severity finding is reported against the project tree.
    //
    // F11.4: the gate also emits TraceEvents (started + passed/failed) to the
    // project's trace log so `forge explain` can narrate the audit. Trace
    // persistence is best-effort: it must never mask the gate's fail-closed
    // contract, so a trace write failure is logged to stderr but does not
    // change the gate outcome.
    if let Some(rules_path) = &input.risk_audit_rules {
        let rules_yaml =
            fs::read_to_string(rules_path).map_err(|source| ExecuteOperationError::ReadFile {
                path: rules_path.clone(),
                source,
            })?;
        let ruleset: RiskAuditRuleSet = yaml_serde::from_str(&rules_yaml).map_err(|source| {
            ExecuteOperationError::ParseYaml {
                path: rules_path.clone(),
                source,
            }
        })?;
        let structure_report = validate_risk_audit_rule_set(&ruleset);
        let (gate_error, error_count, warning_count, target_count, structural_error) =
            if structure_report.has_errors() {
                let first_error = structure_report
                    .diagnostics()
                    .iter()
                    .find(|d| d.severity == forge_core_validate::DiagnosticSeverity::Error)
                    .map_or_else(
                        || "unknown structural error".to_string(),
                        |d| format!("{}: {}", d.path, d.message),
                    );
                (
                    Some(ExecuteOperationError::RiskAuditFailed {
                        error_count: structure_report.diagnostics().len(),
                        first_error: first_error.clone(),
                    }),
                    structure_report.diagnostics().len(),
                    0,
                    0,
                    Some(first_error),
                )
            } else {
                let targets = crate::risk_audit_cmd::collect_targets(&root).map_err(|source| {
                    ExecuteOperationError::ReferenceIndexBuild(format!(
                        "risk-audit collect_targets: {source}"
                    ))
                })?;
                let findings = evaluate_risk_audit(&ruleset, &targets);
                let error_count = findings
                    .diagnostics()
                    .iter()
                    .filter(|d| d.severity == forge_core_validate::DiagnosticSeverity::Error)
                    .count();
                let warning_count = findings
                    .diagnostics()
                    .iter()
                    .filter(|d| d.severity == forge_core_validate::DiagnosticSeverity::Warning)
                    .count();
                if findings.has_errors() {
                    let first_error = findings
                        .diagnostics()
                        .iter()
                        .find(|d| d.severity == forge_core_validate::DiagnosticSeverity::Error)
                        .map_or_else(
                            || "unknown error".to_string(),
                            |d| format!("{}: {}", d.path, d.message),
                        );
                    (
                        Some(ExecuteOperationError::RiskAuditFailed {
                            error_count,
                            first_error,
                        }),
                        error_count,
                        warning_count,
                        targets.len(),
                        None,
                    )
                } else {
                    (None, error_count, warning_count, targets.len(), None)
                }
            };
        // F11.4: emit TraceEvents (started + outcome) before returning. The
        // trace log lives under the same `.forge-method` state root as the
        // WAL; create the dir best-effort and ignore persistence failures so
        // observability never overrides the gate decision.
        let trace_id = format!("{}.risk-audit", input.tx_id_prefix);
        let run_id = format!("{}.risk-audit.{}", input.tx_id_prefix, input.recorded_at);
        let rule_set_ref = rules_path.to_string_lossy().to_string();
        let ctx = crate::risk_audit_trace::RiskAuditTraceContext {
            trace_id: &trace_id,
            run_id: &run_id,
            recorded_at: &input.recorded_at,
            principal_id: "forge-core",
            agent_id: "execute-operation",
            rule_set_ref: &rule_set_ref,
        };
        let events = crate::risk_audit_trace::build_risk_audit_events(
            &ctx,
            error_count,
            warning_count,
            target_count,
            structural_error.as_deref(),
        );
        let trace_state_root = effect_store_root.join(".forge-method");
        let _ = fs::create_dir_all(&trace_state_root);
        for event in &events {
            if let Err(source) = append_trace_event(&trace_state_root, event) {
                eprintln!("forge-core: risk-audit trace append failed (non-fatal): {source}");
            }
        }
        if let Some(error) = gate_error {
            return Err(error);
        }
    }
    // F14.6: Citation Gate. When `require_citation` is set, run the Citation
    // Check over the workspace YAML BEFORE any contract parse or WAL write,
    // resolving `source_id`s against the curated Field Evidence Registry ∪
    // the runtime Source Ledger. Fail-closed via
    // `ExecuteOperationError::CitationCheckFailed`: citing an unregistered
    // source is a precondition failure, not a post-hoc finding. Opt-in — the
    // default (false) preserves existing execute-operation behaviour.
    //
    // The runtime ledger is read best-effort: if the sidecar/state root is not
    // resolvable, the runtime half of the union is empty and the gate checks
    // curated citations only. The curated registry is read from the canonical
    // repo path; if absent, it too is empty (the gate then passes iff no
    // `source_id` occurs in the workspace, which is the correct degenerate
    // behaviour for a repo with no citations to check).
    if input.require_citation {
        if let Some(error) = run_citation_gate(&root, &effect_store_root) {
            return Err(error);
        }
    }
    let operation_path = resolve_contract_input_path(
        &root,
        &canonical_root,
        &input.operation_path,
        ExecuteOperationContractPathKind::Operation,
    )?;
    let command_paths = input
        .command_paths
        .iter()
        .map(|path| {
            resolve_contract_input_path(
                &root,
                &canonical_root,
                path,
                ExecuteOperationContractPathKind::Command,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let effect_paths = input
        .effect_paths
        .iter()
        .map(|path| {
            resolve_contract_input_path(
                &root,
                &canonical_root,
                path,
                ExecuteOperationContractPathKind::Effect,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let index = build_reference_index(&root)
        .map_err(|error| ExecuteOperationError::ReferenceIndexBuild(error.to_string()))?;
    let operation = read_yaml_result::<OperationContractDocument>(&operation_path)?;
    let commands = command_paths
        .iter()
        .map(|path| {
            read_yaml_result::<CommandContractDocument>(path)
                .map(|document| RuntimeOperationCommandInput { document })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let effects = effect_paths
        .iter()
        .map(|path| {
            let document = read_yaml_result::<ToolEffectContractDocument>(path)?;
            let effect_ref = RepoPath(repo_relative_checked(&canonical_root, path)?);
            Ok(RuntimeOperationEffectInput {
                effect_ref,
                document,
            })
        })
        .collect::<Result<Vec<_>, ExecuteOperationError>>()?;
    let payloads = input
        .payloads
        .iter()
        .map(|payload| runtime_payload_from_file(&root, payload, input.payload_policy))
        .collect::<Result<Vec<_>, _>>()?;
    // V2.C: the kernel's `RuntimeOperationExecutionContext` is now typestate'd.
    // The CLI attaches NO real gates here yet — V3.A will add
    // `.with_gate(Box::new(RiskAuditGate { ... }))`. For now `.audited()`
    // transitions the context with an empty gate chain, preserving the
    // historical execute-operation behaviour (the CLI's own risk-audit and
    // citation gates already ran above, before this point).
    let mut context = RuntimeOperationExecutionContext::single_root(&root);
    context.effect_store_root = &effect_store_root;
    context.evidence_log_relative_path = ".forge-method/evidence/command-execution.ndjson";
    context.wal_relative_path = ".forge-method/wal/effects.ndjson";
    context.lock_relative_path = ".forge-method/locks/effects.lock";
    context.effect_metadata_index_relative_path = ".forge-method/index/effect-targets.ndjson";
    context.recorded_at = &input.recorded_at;
    context.tx_id_prefix = &input.tx_id_prefix;
    context.durability = input.durability;
    let context = context.audited();

    let execution = execute_operation(
        &operation,
        RuntimeReadSnapshot::new(&index),
        &commands,
        &effects,
        &payloads,
        &context,
    )
    .map_err(|rejection| ExecuteOperationError::GateRejected { rejection })?;

    Ok(execution)
}

/// F14.6: run the Citation Check gate. Returns `Some(CitationCheckFailed)` when
/// the check reports any `UnresolvedSourceId` error, `None` when it passes (or
/// when neither backing is readable — the degenerate "no citations to check"
/// case). Best-effort on both backings: a missing curated registry or an
/// unresolvable runtime ledger does not itself fail the gate (the union is
/// simply emptier); only an unresolved `source_id` fails it.
fn run_citation_gate(root: &Path, effect_store_root: &Path) -> Option<ExecuteOperationError> {
    let documents = collect_validation_yaml_documents(root);
    // Curated half: canonical repo path, best-effort.
    let evidence_path = root.join("contracts/research/field-evidence-20260625.yaml");
    let evidence = fs::read_to_string(&evidence_path)
        .ok()
        .and_then(|text| yaml_serde::from_str::<FieldEvidenceRegistry>(&text).ok())
        .unwrap_or_else(crate::research_cmd::empty_evidence);
    // Runtime half: the Source Ledger under the state root. Best-effort — if
    // the state root does not resolve, the runtime union is empty.
    let runtime_ids = forge_core_research::project(effect_store_root)
        .map(|projection| {
            projection
                .sources
                .keys()
                .cloned()
                .collect::<std::collections::HashSet<String>>()
        })
        .unwrap_or_default();
    let report = validate_yaml_citation_references(&documents.documents, &evidence, &runtime_ids);
    if report.has_errors() {
        let error_count = report
            .diagnostics()
            .iter()
            .filter(|d| d.severity == forge_core_validate::DiagnosticSeverity::Error)
            .count();
        let first_error = report
            .diagnostics()
            .iter()
            .find(|d| d.severity == forge_core_validate::DiagnosticSeverity::Error)
            .map_or_else(
                || "unknown error".to_string(),
                |d| format!("{}: {}", d.path, d.message),
            );
        Some(ExecuteOperationError::CitationCheckFailed {
            error_count,
            first_error,
        })
    } else {
        None
    }
}

/// Read and parse one YAML contract, mapping IO and parse errors into
/// typed [`ExecuteOperationError`] variants.
///
/// Distinct from the `read_yaml` helper that lives in `lib.rs` (which
/// targets the validation flow and produces `ValidateSummary` diagnostics
/// rather than typed errors).
fn read_yaml_result<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<T, ExecuteOperationError> {
    let text = fs::read_to_string(path).map_err(|source| ExecuteOperationError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    yaml_serde::from_str(&text).map_err(|source| ExecuteOperationError::ParseYaml {
        path: path.to_path_buf(),
        source,
    })
}

fn runtime_payload_from_file(
    root: &Path,
    payload: &PayloadFileSpec,
    policy: PayloadLoadPolicy,
) -> Result<RuntimeOperationEffectPayload, ExecuteOperationError> {
    let path = resolve_input_path(root, &payload.path);
    validate_payload_scope(root, &path, policy.allow_outside_root)?;
    let metadata = fs::metadata(&path).map_err(|source| ExecuteOperationError::ReadFile {
        path: path.clone(),
        source,
    })?;
    let byte_len = metadata.len();
    if byte_len > policy.max_payload_bytes {
        return Err(ExecuteOperationError::PayloadTooLarge {
            path,
            byte_len,
            max_payload_bytes: policy.max_payload_bytes,
        });
    }
    let content = fs::read(&path).map_err(|source| ExecuteOperationError::ReadFile {
        path: path.clone(),
        source,
    })?;
    Ok(RuntimeOperationEffectPayload {
        target_ref: payload.target_ref.clone(),
        payload_kind: RuntimeEffectPayloadKind::RuntimeGenerated,
        content_hash: format!("sha256:{}", hex_sha256(&content)),
        content,
    })
}

fn canonicalize_existing_path(path: &Path) -> Result<PathBuf, ExecuteOperationError> {
    fs::canonicalize(path).map_err(|source| ExecuteOperationError::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}

fn resolve_contract_input_path(
    root: &Path,
    canonical_root: &Path,
    path: &Path,
    kind: ExecuteOperationContractPathKind,
) -> Result<PathBuf, ExecuteOperationError> {
    let path = canonicalize_existing_path(&resolve_input_path(root, path))?;
    if path.starts_with(canonical_root) {
        Ok(path)
    } else {
        Err(ExecuteOperationError::ContractPathOutsideRoot {
            kind,
            root: canonical_root.to_path_buf(),
            path,
        })
    }
}

fn validate_payload_scope(
    root: &Path,
    path: &Path,
    allow_outside_root: bool,
) -> Result<(), ExecuteOperationError> {
    if allow_outside_root {
        return Ok(());
    }
    let root = fs::canonicalize(root).map_err(|source| ExecuteOperationError::ReadFile {
        path: root.to_path_buf(),
        source,
    })?;
    let path = fs::canonicalize(path).map_err(|source| ExecuteOperationError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    if path.starts_with(&root) {
        Ok(())
    } else {
        Err(ExecuteOperationError::PayloadPathOutsideRoot { root, path })
    }
}

fn resolve_input_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn repo_relative_checked(root: &Path, path: &Path) -> Result<String, ExecuteOperationError> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| ExecuteOperationError::InvalidEffectPath {
            root: root.to_path_buf(),
            path: path.to_path_buf(),
        })
}
/// Runs the `forge-core execute-operation` command.
///
/// Loads an operation contract plus command/effect/payload inputs and
/// drives the runtime to apply the declared effects.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present or a value
/// helper reports a missing/malformed argument, `ExitError::failed` when
/// project resolution, the operation load, or the runtime execution
/// reports a non-`Completed` status.
///
/// # Panics
///
/// Panics in JSON mode if the execution result cannot be serialized. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_execute_operation_command(args: &[String]) -> Result<(), ExitError> {
    let mut root = PathBuf::from(".");
    let mut operation_path: Option<PathBuf> = None;
    let mut command_paths = Vec::new();
    let mut effect_paths = Vec::new();
    let mut payloads = Vec::new();
    let mut payload_policy = PayloadLoadPolicy::default();
    let mut allow_bootstrap_core = false;
    let mut recorded_at = "unknown".to_string();
    let mut tx_id_prefix = "cli-execute-operation".to_string();
    let mut json = false;
    let mut no_sync = false;
    let mut risk_audit_rules: Option<PathBuf> = None;
    let mut require_citation = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = next_path_or_err(args, index)?;
            }
            "--operation" => {
                index += 1;
                operation_path = Some(next_path_or_err(args, index)?);
            }
            "--command" => {
                index += 1;
                command_paths.push(next_path_or_err(args, index)?);
            }
            "--effect" => {
                index += 1;
                effect_paths.push(next_path_or_err(args, index)?);
            }
            "--payload" => {
                index += 1;
                payloads.push(parse_payload_arg_or_err(next_arg_or_err(args, index)?)?);
            }
            "--max-payload-bytes" => {
                index += 1;
                payload_policy.max_payload_bytes = parse_u64_or_err(next_arg_or_err(args, index)?)?;
            }
            "--allow-payload-outside-root" => {
                payload_policy.allow_outside_root = true;
            }
            "--allow-bootstrap-core" => {
                allow_bootstrap_core = true;
            }
            "--recorded-at" => {
                index += 1;
                recorded_at = next_arg_or_err(args, index)?.to_string();
            }
            "--tx-id-prefix" => {
                index += 1;
                tx_id_prefix = next_arg_or_err(args, index)?.to_string();
            }
            "--no-sync" => {
                no_sync = true;
            }
            "--require-risk-audit" => {
                index += 1;
                risk_audit_rules = Some(next_path_or_err(args, index)?);
            }
            "--require-citation" => {
                require_citation = true;
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(usage()));
            }
        }
        index += 1;
    }

    let Some(operation_path) = operation_path else {
        return Err(ExitError::usage(usage()));
    };
    let roots = resolve_stateful_roots_or_err("execute-operation", &root, allow_bootstrap_core)?;
    let durability = if no_sync {
        // ADR-0009: emit a one-line stderr warning the first time the flag is
        // honoured, so a CI log makes the durability trade-off visible.
        eprintln!("forge-core: --no-sync active; WAL appends are not durable for this process");
        WalDurability::NoSync
    } else {
        WalDurability::default()
    };
    let input = ExecuteOperationInput {
        root: roots.project_root,
        effect_store_root: Some(roots.effect_store_root),
        operation_path,
        command_paths,
        effect_paths,
        payloads,
        payload_policy,
        recorded_at,
        tx_id_prefix,
        durability,
        risk_audit_rules,
        require_citation,
    };
    let execution = match run_execute_operation(input) {
        Ok(execution) => execution,
        Err(error) => {
            // V2.D: collapse site. The human message stays (stderr-quality),
            // and a typed failure variant rides in the envelope JSON so a
            // programmatic consumer (MCP/agent) can branch on WHY the
            // operation failed instead of re-parsing the message. In `--json`
            // mode the envelope is printed to stdout and the matching exit
            // code is returned; in text mode the lossy stderr line is kept for
            // operator parity with the pre-V2.D behaviour.
            let message = format!("execute-operation failed: {error}");
            let typed = TypedFailure::from(&error);
            if json {
                // JSON mode: print the typed failure envelope to stdout and
                // return the matching exit code (the envelope's diagnostic
                // already went to stdout; main.rs's empty-message guard keeps
                // stderr clean for the MCP consumer).
                let exit = exit_reason_for(&error);
                let env: CliEnvelope<()> =
                    CliEnvelope::err_typed("execute-operation", exit, message.clone(), typed);
                return crate::cli_util::emit_envelope_or_err("execute-operation", env, json);
            }
            return Err(ExitError::failed(message));
        }
    };
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&execution).expect("serialize execution")
        );
    } else {
        println!(
            "forge_core_operation_execution status={:?} reasons={:?}",
            execution.status, execution.reasons
        );
    }
    if execution.status != forge_core_kernel::RuntimeOperationExecutionStatus::Completed {
        return Err(ExitError::failed("execution did not complete"));
    }
    Ok(())
}
