//! Execute-operation pipeline.
//!
//! Pure path/payload resolution layer that lives between the CLI entrypoint
//! (`run_execute_operation_command` in `main.rs`) and the runtime executor
//! (`forge_core_runtime::execute_operation`).
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
    CommandContractDocument, OperationContractDocument, RepoPath, ToolEffectContractDocument,
};
use forge_core_runtime::{
    execute_operation, CommandExecutionContext, RuntimeEffectPayloadKind,
    RuntimeOperationCommandInput, RuntimeOperationEffectInput, RuntimeOperationEffectPayload,
    RuntimeOperationExecution, RuntimeOperationExecutionContext, RuntimeReadSnapshot,
};
use forge_core_store::build_reference_index;

use crate::cli_util::{
    next_arg, next_path, parse_payload_arg, parse_u64, resolve_stateful_roots_or_exit, usage,
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
        source: serde_yaml::Error,
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
        }
    }
}

impl std::error::Error for ExecuteOperationError {}

/// Drive one operation execution end-to-end.
///
/// Resolves all paths against `root`, builds the reference index, loads
/// contract documents and runtime payloads, then hands everything off to
/// [`forge_core_runtime::execute_operation`].
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
    let context = RuntimeOperationExecutionContext {
        command_context: CommandExecutionContext::single_root(&root),
        effect_store_root: &effect_store_root,
        evidence_log_relative_path: ".forge-method/evidence/command-execution.ndjson",
        wal_relative_path: ".forge-method/wal/effects.ndjson",
        lock_relative_path: ".forge-method/locks/effects.lock",
        effect_metadata_index_relative_path: ".forge-method/index/effect-targets.ndjson",
        recorded_at: &input.recorded_at,
        tx_id_prefix: &input.tx_id_prefix,
    };

    Ok(execute_operation(
        &operation,
        RuntimeReadSnapshot::new(&index),
        &commands,
        &effects,
        &payloads,
        &context,
    ))
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
    serde_yaml::from_str(&text).map_err(|source| ExecuteOperationError::ParseYaml {
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
pub fn run_execute_operation_command(args: &[String]) {
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
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = next_path(args, index);
            }
            "--operation" => {
                index += 1;
                operation_path = Some(next_path(args, index));
            }
            "--command" => {
                index += 1;
                command_paths.push(next_path(args, index));
            }
            "--effect" => {
                index += 1;
                effect_paths.push(next_path(args, index));
            }
            "--payload" => {
                index += 1;
                payloads.push(parse_payload_arg(next_arg(args, index)));
            }
            "--max-payload-bytes" => {
                index += 1;
                payload_policy.max_payload_bytes = parse_u64(next_arg(args, index));
            }
            "--allow-payload-outside-root" => {
                payload_policy.allow_outside_root = true;
            }
            "--allow-bootstrap-core" => {
                allow_bootstrap_core = true;
            }
            "--recorded-at" => {
                index += 1;
                recorded_at = next_arg(args, index).to_string();
            }
            "--tx-id-prefix" => {
                index += 1;
                tx_id_prefix = next_arg(args, index).to_string();
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let Some(operation_path) = operation_path else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };
    let roots = resolve_stateful_roots_or_exit("execute-operation", &root, allow_bootstrap_core);
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
    };
    let execution = match run_execute_operation(input) {
        Ok(execution) => execution,
        Err(error) => {
            eprintln!("execute-operation failed: {error}");
            std::process::exit(1);
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
    if execution.status != forge_core_runtime::RuntimeOperationExecutionStatus::Completed {
        std::process::exit(1);
    }
}
