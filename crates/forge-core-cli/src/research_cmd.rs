//! `forge-core research` — the CLI surface for F14 Knowledge Orchestration
//! (ADR-0010).
//!
//! Four subcommands wrap the research source trust model:
//! - `source add`  — admit a `ResearchSource` (from a YAML `--source-file`)
//!   under a `--policy-file`. Calls `forge_core_research::admit_source`.
//! - `source list` — list live sources from the Source Ledger projection.
//! - `check`       — run the Citation Check over the workspace: resolve every
//!   `source_id` in the parsed YAML artifacts against the joint backing
//!   (curated Field Evidence Registry ∪ runtime Source Ledger). Calls
//!   `forge_core_validate::validate_yaml_citation_references`.
//! - `graph`       — build the Evidence Graph projection
//!   (`SourceId → citing claims`) from the parsed YAML artifacts. Calls
//!   `forge_core_research::evidence_graph`.
//! - `cite`        — resolve a single `--source-id` against the joint backing
//!   and report which side it resolves in. A point lookup companion to
//!   `check` (which scans every artifact).
//!
//! State writes go to `<state_root>/research/` (resolved via `resolve_project`,
//! same pattern as `memory`). No claim-governance (`check-write`) is required —
//! the research log is gated by its own file lock (ADR-0010).
//!
//! Output: standard [`CliEnvelope`] dual-output (JSON for agents, text for
//! humans), mirroring `memory_cmd.rs`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use forge_core_contracts::{
    CliEnvelope, ExitReason, FieldEvidenceRegistry, ResearchPolicy, ResearchSource,
};
use forge_core_research::{
    admit_source, evidence_graph, project as project_research, AdmissionStatus, ResearchProjection,
};
use forge_core_store::collect_validation_yaml_documents;
use forge_core_validate::validate_yaml_citation_references;

use crate::cli_error::ExitError;

const RESEARCH_COMMAND: &str = "research";
const SOURCE_ADD_COMMAND: &str = "research source add";
const SOURCE_LIST_COMMAND: &str = "research source list";
const CHECK_COMMAND: &str = "research check";
const GRAPH_COMMAND: &str = "research graph";
const CITE_COMMAND: &str = "research cite";

/// Parse and run `forge-core research <subcommand>`.
///
/// # Errors
///
/// Returns `ExitError::usage` (via envelope emission) when the subcommand is
/// unknown or argument parsing fails.
pub fn run_research_command(args: &[String]) -> Result<(), ExitError> {
    let sub = args.get(1).map_or("--help", String::as_str);
    match sub {
        "source" => {
            let inner = args.get(2).map_or("--help", String::as_str);
            match inner {
                "add" => run_source_add(&args[3..]),
                "list" => run_source_list(&args[3..]),
                "--help" | "-h" | "help" => {
                    print_source_usage();
                    Ok(())
                }
                other => {
                    let want_json = json_output_unless_text_selected(&args[3..]);
                    emit_err(
                        RESEARCH_COMMAND,
                        &format!("unknown source subcommand '{other}'. Try: add, list"),
                        want_json,
                    )
                }
            }
        }
        "check" => run_check(&args[2..]),
        "graph" => run_graph(&args[2..]),
        "cite" => run_cite(&args[2..]),
        "--help" | "-h" | "help" => {
            print_research_usage();
            Ok(())
        }
        other => {
            let want_json = json_output_unless_text_selected(&args[2..]);
            emit_err(
                RESEARCH_COMMAND,
                &format!("unknown subcommand '{other}'. Try: source add|list, check, graph, cite"),
                want_json,
            )
        }
    }
}

fn print_research_usage() {
    println!("forge-core research <subcommand> [options]");
    println!("  source add  --source-file <path> --policy-file <path> [--root <path>] [--allow-bootstrap-core] [--no-json]");
    println!("  source list [--root <path>] [--allow-bootstrap-core] [--no-json]");
    println!("  check       [--root <path>] [--allow-bootstrap-core] [--evidence-file <path>] [--no-json]");
    println!("  graph       [--root <path>] [--allow-bootstrap-core] [--no-json]");
    println!("  cite        --source-id <id> [--root <path>] [--allow-bootstrap-core] [--evidence-file <path>] [--no-json]");
    println!();
    println!("  State writes land under <state_root>/research/ (resolved from --root).");
}

fn print_source_usage() {
    println!("forge-core research source <subcommand> [options]");
    println!("  add  --source-file <path> --policy-file <path> [--root <path>] [--allow-bootstrap-core] [--no-json]");
    println!("  list [--root <path>] [--allow-bootstrap-core] [--no-json]");
}

// --- shared resolution -------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResearchResolveError {
    message: String,
}

impl ResearchResolveError {
    #[must_use]
    fn message(&self) -> String {
        self.message.clone()
    }
}

/// Resolve the research state root: `<state_root>` from `--root`. The Source
/// Ledger lives at `<state_root>/research/sources.ndjson` (per
/// `forge_core_research::RESEARCH_LOG_RELATIVE_PATH`). Mirrors
/// `resolve_memory_dir` in `memory_cmd.rs`.
fn resolve_state_root(
    root: Option<&str>,
    allow_bootstrap_core: bool,
) -> Result<PathBuf, ResearchResolveError> {
    let root_str = root.unwrap_or(".");
    let root_path = PathBuf::from(root_str);
    let project = crate::project_cmd::resolve_project(&root_path, allow_bootstrap_core).map_err(
        |source| ResearchResolveError {
            message: format!("cannot resolve Forge project from --root '{root_str}': {source}"),
        },
    )?;
    let state_root = PathBuf::from(&project.state_root);
    if !state_root.is_dir() {
        return Err(ResearchResolveError {
            message: format!(
                "resolved Forge state_root is not a directory: {}; create the sidecar .forge-method directory",
                state_root.display()
            ),
        });
    }
    Ok(state_root)
}

// --- common option fields ----------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CommonOptions {
    root: Option<String>,
    allow_bootstrap_core: bool,
    evidence_file: Option<PathBuf>,
    want_json: bool,
}

enum CommonFlag {
    Consumed,
    Unknown(String),
}

fn parse_common_flag(
    args: &[String],
    idx: &mut usize,
    common: &mut CommonOptions,
) -> Result<CommonFlag, ResearchParseError> {
    let want_json = common.want_json;
    match args[*idx].as_str() {
        "--root" => {
            *idx += 1;
            let value = require_value(args, *idx, "root", want_json)?;
            common.root = Some(value);
            Ok(CommonFlag::Consumed)
        }
        "--allow-bootstrap-core" => {
            common.allow_bootstrap_core = true;
            Ok(CommonFlag::Consumed)
        }
        "--evidence-file" => {
            *idx += 1;
            let value = require_value(args, *idx, "evidence-file", want_json)?;
            common.evidence_file = Some(PathBuf::from(value));
            Ok(CommonFlag::Consumed)
        }
        "--no-json" | "--text" => {
            common.want_json = false;
            Ok(CommonFlag::Consumed)
        }
        "--json" => Ok(CommonFlag::Consumed),
        other => Ok(CommonFlag::Unknown(other.to_string())),
    }
}

// --- source add --------------------------------------------------------------

fn run_source_add(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_source_add_args(args) {
        Ok(ResearchParseOutcome::Help) => {
            println!("forge-core research source add --source-file <path> --policy-file <path> [--root <path>] [--allow-bootstrap-core] [--no-json]");
            return Ok(());
        }
        Ok(ResearchParseOutcome::Run(opts)) => opts,
        Err(error) => return emit_err(SOURCE_ADD_COMMAND, &error.message(), error.want_json()),
    };
    let source = match load_source_file(&outcome.source_file) {
        Ok(source) => source,
        Err(error) => {
            return emit_err(
                SOURCE_ADD_COMMAND,
                &error.message(),
                outcome.common.want_json,
            )
        }
    };
    let policy = match load_policy_file(&outcome.policy_file) {
        Ok(policy) => policy,
        Err(error) => {
            return emit_err(
                SOURCE_ADD_COMMAND,
                &error.message(),
                outcome.common.want_json,
            )
        }
    };
    let state_root = match resolve_state_root(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
    ) {
        Ok(root) => root,
        Err(error) => {
            return emit_err(
                SOURCE_ADD_COMMAND,
                &error.message(),
                outcome.common.want_json,
            )
        }
    };

    let result = admit_source(&state_root, source, &policy);
    let env: CliEnvelope<serde_json::Value> = match result.status {
        AdmissionStatus::Admitted { sequence } => CliEnvelope::ok(
            SOURCE_ADD_COMMAND,
            serde_json::to_value(SourceAddOkData {
                source_id: result.source_id.0,
                sequence,
            })
            .expect("serialize source add ok"),
        ),
        AdmissionStatus::DeniedByGate(reasons) => CliEnvelope::reject(
            SOURCE_ADD_COMMAND,
            ExitReason::RejectedByGate,
            "admission gate denied the source",
            serde_json::to_value(reasons_iter(reasons)).expect("serialize denial reasons"),
        ),
        AdmissionStatus::StoreError(error) => CliEnvelope::err(
            SOURCE_ADD_COMMAND,
            ExitReason::Conflict,
            format!("research store error: {error}"),
        ),
    };
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct SourceAddOkData {
    source_id: String,
    sequence: u64,
}

// --- source list -------------------------------------------------------------

fn run_source_list(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_common_only_args(args) {
        Ok(ResearchParseOutcome::Help) => {
            println!("forge-core research source list [--root <path>] [--allow-bootstrap-core] [--no-json]");
            return Ok(());
        }
        Ok(ResearchParseOutcome::Run(common)) => common,
        Err(error) => return emit_err(SOURCE_LIST_COMMAND, &error.message(), error.want_json()),
    };
    let state_root = match resolve_state_root(outcome.root.as_deref(), outcome.allow_bootstrap_core)
    {
        Ok(root) => root,
        Err(error) => return emit_err(SOURCE_LIST_COMMAND, &error.message(), outcome.want_json),
    };
    let projection = match project_research(&state_root) {
        Ok(projection) => projection,
        Err(error) => {
            return emit_err(
                SOURCE_LIST_COMMAND,
                &format!("cannot read research source ledger: {error}"),
                outcome.want_json,
            )
        }
    };
    let env = CliEnvelope::ok(
        SOURCE_LIST_COMMAND,
        SourceListOkData {
            sequence: projection.sequence,
            count: projection.len(),
            sources: projection
                .sources
                .values()
                .map(source_view)
                .collect::<Vec<_>>(),
        },
    );
    crate::cli_util::emit_envelope(env, outcome.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct SourceListOkData {
    sequence: u64,
    count: usize,
    sources: Vec<SourceView>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct SourceView {
    source_id: String,
    kind: String,
    title: String,
    locator: String,
}

fn source_view(source: &ResearchSource) -> SourceView {
    SourceView {
        source_id: source.id.0.clone(),
        // serde rename_all = "snake_case" gives the wire form (e.g. "web_url");
        // Debug formatting would collapse it to "weburl".
        kind: serde_json::to_value(source.kind)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("{:?}", source.kind).to_lowercase()),
        title: source.title.clone(),
        locator: source.locator.clone(),
    }
}

// --- check (citation check over the workspace) -------------------------------

fn run_check(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_common_only_args(args) {
        Ok(ResearchParseOutcome::Help) => {
            println!("forge-core research check [--root <path>] [--allow-bootstrap-core] [--evidence-file <path>] [--no-json]");
            return Ok(());
        }
        Ok(ResearchParseOutcome::Run(common)) => common,
        Err(error) => return emit_err(CHECK_COMMAND, &error.message(), error.want_json()),
    };
    let root_str = outcome.root.as_deref().unwrap_or(".");
    let root_path = PathBuf::from(root_str);

    let evidence = match load_evidence(outcome.evidence_file.as_deref(), &root_path) {
        Ok(evidence) => evidence,
        Err(error) => return emit_err(CHECK_COMMAND, &error.to_string(), outcome.want_json),
    };
    let projection = match resolve_state_root(outcome.root.as_deref(), outcome.allow_bootstrap_core)
    {
        Ok(state_root) => project_research(&state_root).unwrap_or_default(),
        // No sidecar = no runtime ledger; check against the curated registry
        // alone (the runtime half of the union is empty).
        Err(_) => ResearchProjection::default(),
    };
    let runtime_ids: HashSet<String> = projection.sources.keys().cloned().collect();

    let documents = collect_validation_yaml_documents(&root_path);
    let report = validate_yaml_citation_references(&documents.documents, &evidence, &runtime_ids);

    let diagnostics: Vec<CheckDiagnosticView> = report
        .diagnostics()
        .iter()
        .map(CheckDiagnosticView::from_diagnostic)
        .collect();
    let env: CliEnvelope<serde_json::Value> = if report.has_errors() {
        CliEnvelope::reject(
            CHECK_COMMAND,
            ExitReason::RejectedByGate,
            "citation check reported unresolved source_ids",
            serde_json::to_value(&diagnostics).expect("serialize citation diagnostics"),
        )
    } else {
        CliEnvelope::ok(
            CHECK_COMMAND,
            serde_json::to_value(CheckOkData {
                runtime_sources: projection.len(),
                diagnostics,
            })
            .expect("serialize check ok"),
        )
    };
    crate::cli_util::emit_envelope(env, outcome.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct CheckOkData {
    runtime_sources: usize,
    diagnostics: Vec<CheckDiagnosticView>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct CheckDiagnosticView {
    severity: String,
    code: String,
    path: String,
    message: String,
}

impl CheckDiagnosticView {
    fn from_diagnostic(diagnostic: &forge_core_validate::Diagnostic) -> Self {
        Self {
            severity: match diagnostic.severity {
                forge_core_validate::DiagnosticSeverity::Error => "error".into(),
                forge_core_validate::DiagnosticSeverity::Warning => "warning".into(),
            },
            code: format!("{:?}", diagnostic.code),
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        }
    }
}

// --- graph (evidence graph projection) ---------------------------------------

fn run_graph(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_common_only_args(args) {
        Ok(ResearchParseOutcome::Help) => {
            println!(
                "forge-core research graph [--root <path>] [--allow-bootstrap-core] [--no-json]"
            );
            return Ok(());
        }
        Ok(ResearchParseOutcome::Run(common)) => common,
        Err(error) => return emit_err(GRAPH_COMMAND, &error.message(), error.want_json()),
    };
    let root_str = outcome.root.as_deref().unwrap_or(".");
    let root_path = PathBuf::from(root_str);

    let projection = match resolve_state_root(outcome.root.as_deref(), outcome.allow_bootstrap_core)
    {
        Ok(state_root) => project_research(&state_root).unwrap_or_default(),
        Err(_) => ResearchProjection::default(),
    };
    let documents = collect_validation_yaml_documents(&root_path);
    let graph = evidence_graph(&documents.documents, &projection);

    let env = CliEnvelope::ok(
        GRAPH_COMMAND,
        GraphOkData {
            source_count: graph.len(),
            claim_count: graph.values().map(Vec::len).sum(),
            graph: graph
                .into_iter()
                .map(|(source_id, claims)| GraphEntry { source_id, claims })
                .collect(),
        },
    );
    crate::cli_util::emit_envelope(env, outcome.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct GraphOkData {
    source_count: usize,
    claim_count: usize,
    graph: Vec<GraphEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct GraphEntry {
    source_id: String,
    claims: Vec<forge_core_research::ClaimRef>,
}

// --- cite (single source_id resolution lookup) -------------------------------

fn run_cite(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_cite_args(args) {
        Ok(ResearchParseOutcome::Help) => {
            println!("forge-core research cite --source-id <id> [--root <path>] [--allow-bootstrap-core] [--evidence-file <path>] [--no-json]");
            return Ok(());
        }
        Ok(ResearchParseOutcome::Run(opts)) => opts,
        Err(error) => return emit_err(CITE_COMMAND, &error.message(), error.want_json()),
    };
    let root_str = outcome.common.root.as_deref().unwrap_or(".");
    let root_path = PathBuf::from(root_str);

    let evidence = match load_evidence(outcome.common.evidence_file.as_deref(), &root_path) {
        Ok(evidence) => evidence,
        Err(error) => return emit_err(CITE_COMMAND, &error.to_string(), outcome.common.want_json),
    };
    let projection = match resolve_state_root(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
    ) {
        Ok(state_root) => project_research(&state_root).unwrap_or_default(),
        Err(_) => ResearchProjection::default(),
    };

    let curated: HashSet<String> = evidence.sources.iter().map(|s| s.id.0.clone()).collect();
    let in_curated = curated.contains(&outcome.source_id);
    let in_runtime = projection.sources.contains_key(&outcome.source_id);

    let env: CliEnvelope<serde_json::Value> = if in_curated || in_runtime {
        CliEnvelope::ok(
            CITE_COMMAND,
            serde_json::to_value(CiteOkData {
                source_id: outcome.source_id,
                resolved: true,
                backing: if in_curated && in_runtime {
                    "both".into()
                } else if in_curated {
                    "curated".into()
                } else {
                    "runtime".into()
                },
            })
            .expect("serialize cite ok"),
        )
    } else {
        CliEnvelope::reject(
            CITE_COMMAND,
            ExitReason::RejectedByGate,
            "source_id is unresolved — not in field evidence registry or source ledger",
            serde_json::to_value(CiteRejectedData {
                source_id: outcome.source_id,
                resolved: false,
            })
            .expect("serialize cite rejection"),
        )
    };
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct CiteOkData {
    source_id: String,
    resolved: bool,
    backing: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct CiteRejectedData {
    source_id: String,
    resolved: bool,
}

// --- arg parsing -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResearchParseOutcome<T> {
    Help,
    Run(T),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResearchParseError {
    MissingValue {
        flag: &'static str,
        want_json: bool,
    },
    FlagAsValue {
        flag: &'static str,
        value: String,
        want_json: bool,
    },
    MissingRequired {
        flag: &'static str,
        want_json: bool,
    },
    UnknownArgument {
        argument: String,
        want_json: bool,
    },
}

impl ResearchParseError {
    #[must_use]
    fn want_json(&self) -> bool {
        match self {
            Self::MissingValue { want_json, .. }
            | Self::FlagAsValue { want_json, .. }
            | Self::MissingRequired { want_json, .. }
            | Self::UnknownArgument { want_json, .. } => *want_json,
        }
    }

    #[must_use]
    fn message(&self) -> String {
        match self {
            Self::MissingValue { flag, .. } => format!("--{flag} requires a value"),
            Self::FlagAsValue { flag, value, .. } => {
                format!("--{flag} requires a value, got another flag '{value}'")
            }
            Self::MissingRequired { flag, .. } => format!("--{flag} is required"),
            Self::UnknownArgument { argument, .. } => format!("unknown argument '{argument}'"),
        }
    }
}

fn require_value(
    args: &[String],
    idx: usize,
    flag: &'static str,
    want_json: bool,
) -> Result<String, ResearchParseError> {
    match args.get(idx) {
        Some(value) if value.starts_with('-') && value.len() > 1 => {
            Err(ResearchParseError::FlagAsValue {
                flag,
                value: value.clone(),
                want_json,
            })
        }
        Some(value) => Ok(value.clone()),
        None => Err(ResearchParseError::MissingValue { flag, want_json }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceAddOptions {
    common: CommonOptions,
    source_file: PathBuf,
    policy_file: PathBuf,
}

fn parse_source_add_args(
    args: &[String],
) -> Result<ResearchParseOutcome<SourceAddOptions>, ResearchParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut source_file: Option<PathBuf> = None;
    let mut policy_file: Option<PathBuf> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--source-file" => {
                    idx += 1;
                    source_file = Some(PathBuf::from(require_value(
                        args,
                        idx,
                        "source-file",
                        common.want_json,
                    )?));
                }
                "--policy-file" => {
                    idx += 1;
                    policy_file = Some(PathBuf::from(require_value(
                        args,
                        idx,
                        "policy-file",
                        common.want_json,
                    )?));
                }
                "--help" | "-h" => return Ok(ResearchParseOutcome::Help),
                other => {
                    return Err(ResearchParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    let source_file = source_file.ok_or(ResearchParseError::MissingRequired {
        flag: "source-file",
        want_json: common.want_json,
    })?;
    let policy_file = policy_file.ok_or(ResearchParseError::MissingRequired {
        flag: "policy-file",
        want_json: common.want_json,
    })?;
    Ok(ResearchParseOutcome::Run(SourceAddOptions {
        common,
        source_file,
        policy_file,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CiteOptions {
    common: CommonOptions,
    source_id: String,
}

fn parse_cite_args(
    args: &[String],
) -> Result<ResearchParseOutcome<CiteOptions>, ResearchParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut source_id: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--source-id" => {
                    idx += 1;
                    source_id = Some(require_value(args, idx, "source-id", common.want_json)?);
                }
                "--help" | "-h" => return Ok(ResearchParseOutcome::Help),
                other => {
                    return Err(ResearchParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    let source_id = source_id.ok_or(ResearchParseError::MissingRequired {
        flag: "source-id",
        want_json: common.want_json,
    })?;
    Ok(ResearchParseOutcome::Run(CiteOptions { common, source_id }))
}

/// Parse only the common flags (for `source list`, `check`, `graph`).
fn parse_common_only_args(
    args: &[String],
) -> Result<ResearchParseOutcome<CommonOptions>, ResearchParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--help" | "-h" => return Ok(ResearchParseOutcome::Help),
                other => {
                    return Err(ResearchParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    Ok(ResearchParseOutcome::Run(common))
}

// --- file loaders ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadError {
    message: String,
}

impl LoadError {
    #[must_use]
    fn message(&self) -> String {
        self.message.clone()
    }
}

fn load_source_file(path: &Path) -> Result<ResearchSource, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError {
        message: format!("cannot read source file '{}': {source}", path.display()),
    })?;
    yaml_serde::from_str::<ResearchSource>(&text).map_err(|source| LoadError {
        message: format!(
            "source file '{}' is not a valid ResearchSource YAML: {source}",
            path.display()
        ),
    })
}

fn load_policy_file(path: &Path) -> Result<ResearchPolicy, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError {
        message: format!("cannot read policy file '{}': {source}", path.display()),
    })?;
    yaml_serde::from_str::<ResearchPolicy>(&text).map_err(|source| LoadError {
        message: format!(
            "policy file '{}' is not a valid ResearchPolicy YAML: {source}",
            path.display()
        ),
    })
}

/// Load the curated `FieldEvidenceRegistry`, preferring `--evidence-file`, then
/// the canonical repo path, then an empty registry (citation check degrades to
/// runtime-only). Returns an error when an explicit `--evidence-file` cannot
/// be read or parsed.
///
/// # Errors
///
/// Returns [`EvidenceLoadError`] only when an explicit `--evidence-file` is
/// given and fails to read or parse. The canonical-path fallback never
/// errors — a missing or malformed canonical file degrades to an empty
/// registry.
fn load_evidence(
    explicit: Option<&Path>,
    root: &Path,
) -> Result<FieldEvidenceRegistry, EvidenceLoadError> {
    if let Some(path) = explicit {
        let text =
            std::fs::read_to_string(path).map_err(|source| EvidenceLoadError::ReadFailed {
                path: path.display().to_string(),
                source: source.to_string(),
            })?;
        return yaml_serde::from_str::<FieldEvidenceRegistry>(&text).map_err(|source| {
            EvidenceLoadError::ParseFailed {
                path: path.display().to_string(),
                source: source.to_string(),
            }
        });
    }
    let canonical = root.join("contracts/research/field-evidence-20260625.yaml");
    match std::fs::read_to_string(&canonical) {
        Ok(text) => Ok(yaml_serde::from_str::<FieldEvidenceRegistry>(&text)
            .unwrap_or_else(|_| empty_evidence())),
        Err(_) => Ok(empty_evidence()),
    }
}

/// Failures loading an explicit `--evidence-file`. Hand-rolled per AGENTS.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceLoadError {
    /// The evidence file could not be read.
    ReadFailed {
        /// The file path, display-formatted.
        path: String,
        /// The underlying IO error, as a lossy String.
        source: String,
    },
    /// The evidence file is not valid `FieldEvidenceRegistry` YAML.
    ParseFailed {
        /// The file path, display-formatted.
        path: String,
        /// The underlying serde error, as a lossy String.
        source: String,
    },
}

impl std::fmt::Display for EvidenceLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadFailed { path, source } => {
                write!(f, "cannot read evidence file '{path}': {source}")
            }
            Self::ParseFailed { path, source } => {
                write!(
                    f,
                    "evidence file '{path}' is not a valid FieldEvidenceRegistry YAML: {source}"
                )
            }
        }
    }
}

impl std::error::Error for EvidenceLoadError {}

/// Construct an empty curated registry. Used as the fallback when no evidence
/// file is present so the citation check degrades to runtime-only rather than
/// erroring (the curated half of the union is simply empty).
pub(crate) fn empty_evidence() -> FieldEvidenceRegistry {
    FieldEvidenceRegistry {
        schema_version: String::new(),
        research: String::new(),
        created_at: String::new(),
        status: String::new(),
        policy: forge_core_contracts::evidence::EvidencePolicy {
            purpose: String::new(),
            evidence_tiers: Vec::new(),
            rule: String::new(),
            geographic_coverage: forge_core_contracts::evidence::GeographicCoverage {
                rule: String::new(),
                rationale: String::new(),
                minimum_behavior: Vec::new(),
            },
        },
        sources: Vec::new(),
        plan_level_implications: Vec::new(),
        open_research_gaps: Vec::new(),
    }
}

// --- emit helpers (mirror memory_cmd.rs) -------------------------------------

#[must_use]
fn json_output_unless_text_selected(args: &[String]) -> bool {
    !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"))
}

/// Convert a `Vec<ResearchAdmissionDenialReason>` into a serializable list of
/// strings for the envelope's `data` on a rejection (`snake_case` wire form).
#[must_use]
fn reasons_iter(reasons: Vec<forge_core_contracts::ResearchAdmissionDenialReason>) -> Vec<String> {
    reasons
        .into_iter()
        .map(|reason| {
            serde_json::to_value(reason)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| format!("{reason:?}").to_lowercase())
        })
        .collect()
}

fn emit_err(command: &str, message: &str, want_json: bool) -> Result<(), ExitError> {
    let env: CliEnvelope<()> = CliEnvelope::err(command, ExitReason::InvalidDecisionShape, message);
    crate::cli_util::emit_envelope(env, want_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parse_source_add_requires_source_and_policy_file() {
        let error = parse_source_add_args(&args(&[])).expect_err("missing required");
        assert_eq!(
            error,
            ResearchParseError::MissingRequired {
                flag: "source-file",
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_source_add_accepts_complete_args() {
        let outcome = parse_source_add_args(&args(&[
            "--source-file",
            "source.yaml",
            "--policy-file",
            "policy.yaml",
            "--no-json",
        ]))
        .expect("parse");
        let ResearchParseOutcome::Run(opts) = outcome else {
            panic!("expected Run");
        };
        assert_eq!(opts.source_file, PathBuf::from("source.yaml"));
        assert_eq!(opts.policy_file, PathBuf::from("policy.yaml"));
        assert!(!opts.common.want_json);
    }

    #[test]
    fn parse_cite_requires_source_id() {
        let error = parse_cite_args(&args(&[])).expect_err("missing required");
        assert_eq!(
            error,
            ResearchParseError::MissingRequired {
                flag: "source-id",
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_common_only_rejects_unknown_flag() {
        let error = parse_common_only_args(&args(&["--bogus"])).expect_err("unknown");
        assert!(matches!(error, ResearchParseError::UnknownArgument { .. }));
    }

    #[test]
    fn json_output_defaults_to_json() {
        assert!(json_output_unless_text_selected(&args(&[])));
        assert!(!json_output_unless_text_selected(&args(&["--no-json"])));
        assert!(!json_output_unless_text_selected(&args(&["--text"])));
    }

    #[test]
    fn run_research_unknown_subcommand_emits_usage_error() {
        let result = run_research_command(&args(&["research", "frobnicate", "--no-json"]));
        assert!(result.is_err(), "unknown subcommand must error");
    }

    #[test]
    fn run_research_help_prints_usage_and_succeeds() {
        let result = run_research_command(&args(&["research", "--help"]));
        assert!(result.is_ok(), "--help must succeed");
    }

    #[test]
    fn run_research_source_unknown_inner_emits_usage_error() {
        let result =
            run_research_command(&args(&["research", "source", "frobnicate", "--no-json"]));
        assert!(result.is_err(), "unknown source subcommand must error");
    }

    #[test]
    fn reasons_iter_lowercases_variants() {
        let reasons = vec![
            forge_core_contracts::ResearchAdmissionDenialReason::KindNotPermitted,
            forge_core_contracts::ResearchAdmissionDenialReason::MissingContentHash,
        ];
        let out = reasons_iter(reasons);
        assert_eq!(out, vec!["kind_not_permitted", "missing_content_hash"]);
    }

    #[test]
    fn source_view_maps_fields_lowercasing_kind() {
        let source = ResearchSource {
            id: forge_core_contracts::SourceId("s.x".into()),
            kind: forge_core_contracts::ResearchSourceKind::WebUrl,
            title: "A page".into(),
            locator: "https://example.org".into(),
            fetched_at: 1,
            content_hash: None,
            harvested_by: "agent.1".into(),
            trace_ref: None,
        };
        let view = source_view(&source);
        assert_eq!(view.source_id, "s.x");
        assert_eq!(view.kind, "web_url");
        assert_eq!(view.title, "A page");
    }
}
