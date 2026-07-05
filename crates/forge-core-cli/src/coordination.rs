//! Governance CLI for the coordination-eval gate (S4.7).
//!
//! The engine ships NO model (DC9): it cannot self-prove a coordination
//! dimension. So this surface offers a STRUCTURAL gate the operator / host LLM
//! runs before spending tokens gathering per-dimension evidence:
//!
//! `forge-core coordination validate --suite <path>` — load + deserialize +
//! structurally validate + report dangling fixture/evidence refs. Exit 0 =
//! the suite is REAL (all 22 refs resolve, all 9 dims well-formed). Exit 2 =
//! structural problems or dangling refs reported as typed `CoordinationValidationError`s.
//!
//! Scoring ([`forge_core_decisions::score_coordination`]) stays engine-only (like
//! `score_router`): the host supplies outcomes (file-backed, DC10) and the
//! engine is the deterministic gate. A future MCP surface (slice 6) exposes it.

use forge_core_command_surface::COMMAND_COORDINATION;
use forge_core_contracts::coordination_eval::CoordinationEvalContractDocument;
use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_decisions::{coordination_fixture_gaps, validate_coordination_contract};
use std::path::{Path, PathBuf};

use crate::cli_error::ExitError;

// ---------------------------------------------------------------------------
// payload
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct CoordinationValidatePayload {
    /// Canonical path of the suite that was validated.
    pub suite_path: String,
    /// Number of dimensions in the suite (expect 9).
    pub dimension_count: usize,
    /// Structural validation errors (empty == well-formed).
    pub structural_errors: Vec<yaml_serde::Value>,
    /// Dangling fixture/evidence refs (empty == every ref resolves = REAL suite).
    pub dangling_refs: Vec<String>,
    /// True when the suite is structurally valid AND has no dangling refs.
    pub is_real: bool,
}

/// Load + structurally validate + dangling-ref check a coordination suite.
///
/// # Arguments
/// * `suite_path` - Path to the YAML suite (a `CoordinationEvalContractDocument`).
/// * `repo_root` - Repository root used to resolve `fixture_refs`/`evidence_refs`.
#[must_use]
pub fn run_validate(
    suite_path: &Path,
    repo_root: &Path,
) -> CliEnvelope<CoordinationValidatePayload> {
    let text = match std::fs::read_to_string(suite_path) {
        Ok(t) => t,
        Err(e) => {
            return CliEnvelope::err(
                "coordination validate",
                ExitReason::EnvConfig,
                format!("cannot read suite '{}': {e}", suite_path.display()),
            );
        }
    };
    let doc: CoordinationEvalContractDocument = match yaml_serde::from_str(&text) {
        Ok(d) => d,
        Err(e) => {
            return CliEnvelope::err(
                "coordination validate",
                ExitReason::EnvConfig,
                format!("cannot deserialize suite '{}': {e}", suite_path.display()),
            );
        }
    };
    let contract = &doc.coordination_eval_contract;

    let structural = validate_coordination_contract(contract);
    let gaps = coordination_fixture_gaps(contract, repo_root);

    // Serialize the typed errors losslessly (they carry enum dimension tags).
    let structural_ser: Vec<yaml_serde::Value> = structural
        .iter()
        .map(|e| yaml_serde::to_value(e).unwrap_or(yaml_serde::Value::Null))
        .collect();

    let is_real = structural.is_empty() && gaps.is_empty();

    let payload = CoordinationValidatePayload {
        suite_path: suite_path.display().to_string(),
        dimension_count: contract.dimensions.len(),
        structural_errors: structural_ser,
        dangling_refs: gaps,
        is_real,
    };

    // M2 (review S4.7): a governance gate MUST fail loud. A readable+deserializable
    // but structurally-broken or dangling-ref suite is NOT real — emit a
    // rejection (exit 2) carrying the payload so CI / shell consumers that
    // check `$?` cannot treat a broken suite as passing the gate. This matches
    // the module doc comment (exit 0 == real, exit 2 == structural problems /
    // dangling refs).
    if is_real {
        CliEnvelope::ok("coordination validate", payload)
    } else {
        let msg = format!(
            "suite is not real: {} structural error(s), {} dangling ref(s)",
            payload.structural_errors.len(),
            payload.dangling_refs.len()
        );
        CliEnvelope::reject(
            "coordination validate",
            ExitReason::RejectedByGate,
            msg,
            payload,
        )
    }
}

// ---------------------------------------------------------------------------
// arg parsing + dispatch (called from main.rs)
// ---------------------------------------------------------------------------

/// Parse and run `forge-core coordination <subcommand>`.
///
/// Emits the result envelope via the single shared path
/// ([`crate::cli_util::emit_envelope`] / [`crate::cli_util::emit_envelope_with`])
/// and returns a `Result<(), ExitError>` carrying the DD10 exit code, so this
/// composes with every other command dispatcher and the registry handler.
///
/// The validate text-mode success summary is the one place coordination
/// renders richer output than the standard `"{command}: ok"` line — it lists
/// the suite, dimensions, structural errors, and dangling refs (the whole
/// point of the gate). That detail is passed to `emit_envelope_with` as the
/// text-mode success override; JSON mode still prints the full envelope.
///
/// # Errors
///
/// Returns `ExitError::with_code` carrying the dispatcher's non-zero exit
/// code (3 for argv-shape errors, 2 for unknown subcommand, or the envelope's
/// own code for a validate result) so the entrypoint can translate it into
/// `process::exit(code)`.
pub fn dispatch(args: &[String]) -> Result<(), ExitError> {
    let sub = args.get(1).map_or("--help", String::as_str);
    match sub {
        "validate" => {
            let mut suite = PathBuf::from("contracts/evals/minimal-coordination-eval-suite.yaml");
            let mut repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let mut want_json = true;

            let mut idx = 2usize;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--suite" => {
                        // M3 (review S4.7): a flag with no value MUST NOT silently
                        // fall back to the default suite — that lets an operator
                        // believe they validated their own suite. Fail loud.
                        idx += 1;
                        let Some(val) = args.get(idx) else {
                            eprintln!("coordination validate: --suite requires a value");
                            return Err(ExitError::with_code(3, String::new()));
                        };
                        suite = PathBuf::from(val);
                    }
                    "--repo-root" => {
                        idx += 1;
                        let Some(val) = args.get(idx) else {
                            eprintln!("coordination validate: --repo-root requires a value");
                            return Err(ExitError::with_code(3, String::new()));
                        };
                        repo_root = PathBuf::from(val);
                    }
                    "--json" => want_json = true,
                    "--no-json" => want_json = false,
                    "--help" | "-h" => {
                        print_validate_help();
                        return Ok(());
                    }
                    other => {
                        eprintln!("coordination validate: unknown flag '{other}'");
                        return Err(ExitError::with_code(3, String::new()));
                    }
                }
                idx += 1;
            }

            let env = run_validate(&suite, &repo_root);
            let report = render_validate_human(&env);
            crate::cli_util::emit_envelope_with(env, want_json, Some(&report))
        }
        "--help" | "-h" | "help" | "" => {
            println!("{}", coordination_usage());
            println!();
            println!("    Loads + structurally validates a coordination-eval suite and reports");
            println!("    dangling fixture/evidence refs. Exit 0 = suite is REAL (all refs");
            println!("    resolve, all 9 dims well-formed).");
            Ok(())
        }
        other => {
            eprintln!(
                "forge-core coordination: unknown subcommand '{other}'. Try: {hint}",
                hint = coordination_subcommand_hint()
            );
            Err(ExitError::with_code(2, String::new()))
        }
    }
}

fn coordination_usage() -> String {
    let mut usage = String::from("forge-core coordination <subcommand> [options]");
    for line in COMMAND_COORDINATION.local_usage_lines() {
        usage.push('\n');
        usage.push_str("  ");
        usage.push_str(line);
    }
    usage
}

fn coordination_subcommand_hint() -> String {
    COMMAND_COORDINATION.concrete_subcommand_hint()
}

fn coordination_command_surface_usage_line_for(subcommand: &str) -> &'static str {
    COMMAND_COORDINATION
        .usage_line_for_subcommand(subcommand)
        .unwrap_or("forge-core coordination <subcommand> [options]")
}

fn print_validate_help() {
    println!(
        "{}",
        coordination_command_surface_usage_line_for("validate")
    );
}

/// Build the multi-line text-mode success report for `coordination validate`.
/// Returned as a single `String` (newlines embedded) so it can be handed to
/// [`crate::cli_util::emit_envelope_with`] as the text-mode success override.
fn render_validate_human(env: &CliEnvelope<CoordinationValidatePayload>) -> String {
    let mut out = String::new();
    if let Some(d) = &env.data {
        use std::fmt::Write;
        let _ = writeln!(out, "suite: {}", d.suite_path);
        let _ = writeln!(out, "dimensions: {}", d.dimension_count);
        let _ = writeln!(out, "structural errors: {}", d.structural_errors.len());
        for e in &d.structural_errors {
            let _ = writeln!(out, "  - {e:?}");
        }
        let _ = writeln!(out, "dangling refs: {}", d.dangling_refs.len());
        for r in &d.dangling_refs {
            let _ = writeln!(out, "  - {r}");
        }
        let _ = write!(out, "is_real: {}", d.is_real);
    } else {
        let msg = env
            .error
            .as_ref()
            .map_or("(no error detail)", |e| e.message.as_str());
        out.push_str(msg);
    }
    out
}

/// Dispatch entrypoint for the `forge-core coordination` command tree.
/// Delegates to [`dispatch`], which emits via the single shared path and
/// returns the `Result<(), ExitError>` the registry expects.
///
/// # Errors
///
/// Returns `ExitError::with_code` carrying the dispatcher's non-zero exit
/// code so the entrypoint can translate it into `process::exit(code)`.
pub fn run_coordination_command(args: &[String]) -> Result<(), ExitError> {
    dispatch(args)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root")
    }

    fn real_suite() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/evals/minimal-coordination-eval-suite.yaml")
            .canonicalize()
            .expect("suite")
    }

    #[test]
    fn validate_real_suite_is_real() {
        let env = run_validate(&real_suite(), &repo_root());
        assert!(env.ok, "envelope should be ok");
        let d = env.data.as_ref().expect("payload");
        assert_eq!(d.dimension_count, 9);
        assert!(
            d.structural_errors.is_empty(),
            "structural errors: {:?}",
            d.structural_errors
        );
        assert!(
            d.dangling_refs.is_empty(),
            "dangling refs: {:?}",
            d.dangling_refs
        );
        assert!(d.is_real, "the real suite must be REAL");
    }

    #[test]
    fn validate_missing_suite_is_env_config() {
        let env = run_validate(Path::new("/nonexistent/suite.yaml"), &repo_root());
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, "env_config");
        assert!(env.data.is_none());
    }

    /// Exit code carried by a `dispatch` result: 0 for `Ok(())`, otherwise the
    /// `ExitError::with_code` code. `dispatch` now returns `Result<(), ExitError>`
    /// (V1.D — folded the legacy `(String, i32)` return-tuple style into the
    /// single shared emit path), so tests assert on the exit code it carries.
    fn exit_of(result: Result<(), ExitError>) -> i32 {
        match result {
            Ok(()) => 0,
            Err(err) => err.exit_code(),
        }
    }

    #[test]
    fn dispatch_validate_returns_zero_for_real_suite() {
        let args: Vec<String> = vec![
            "coordination".into(),
            "validate".into(),
            "--suite".into(),
            real_suite().to_string_lossy().into_owned(),
            "--repo-root".into(),
            repo_root().to_string_lossy().into_owned(),
        ];
        // dispatch now emits via the shared path and returns Result; the JSON
        // envelope shape ("ok"/"is_real") is still covered by
        // `validate_real_suite_is_real` calling `run_validate` directly.
        let result = dispatch(&args);
        assert_eq!(exit_of(result), 0);
    }

    #[test]
    fn dispatch_validate_missing_file_exits_5() {
        // M2: unreadable suite is an env-config error (exit 5), NOT exit 0.
        let args: Vec<String> = vec![
            "coordination".into(),
            "validate".into(),
            "--suite".into(),
            "/nonexistent/suite.yaml".into(),
        ];
        let result = dispatch(&args);
        assert_eq!(exit_of(result), 5);
    }

    #[test]
    fn dispatch_unknown_subcommand_exits_2() {
        let args: Vec<String> = vec!["coordination".into(), "bogus".into()];
        let result = dispatch(&args);
        assert_eq!(exit_of(result), 2);
    }

    #[test]
    fn coordination_usage_projects_command_surface_lines() {
        let usage = coordination_usage();
        assert!(
            usage.starts_with("forge-core coordination <subcommand> [options]"),
            "coordination usage should keep the local command-tree header: {usage}"
        );
        for line in COMMAND_COORDINATION.usage_lines {
            let subcommand_usage = COMMAND_COORDINATION.local_usage_line(line);
            assert!(
                usage.contains(subcommand_usage),
                "coordination usage should include projected Command Surface line {subcommand_usage:?}: {usage}"
            );
        }
        assert_eq!(coordination_subcommand_hint(), "validate");
    }

    #[test]
    fn coordination_subcommand_help_lookup_projects_full_command_surface_lines() {
        assert_eq!(
            coordination_command_surface_usage_line_for("validate"),
            COMMAND_COORDINATION.canonical_usage().trim_start()
        );
    }

    #[test]
    fn dispatch_validate_accepts_explicit_json_flag() {
        let args: Vec<String> = vec![
            "coordination".into(),
            "validate".into(),
            "--suite".into(),
            real_suite().to_string_lossy().into_owned(),
            "--repo-root".into(),
            repo_root().to_string_lossy().into_owned(),
            "--json".into(),
        ];
        let result = dispatch(&args);
        assert_eq!(exit_of(result), 0);
    }

    #[test]
    fn dispatch_unknown_flag_exits_3() {
        let args: Vec<String> = vec!["coordination".into(), "validate".into(), "--bogus".into()];
        let result = dispatch(&args);
        assert_eq!(exit_of(result), 3);
    }

    // --- review S4.7 fixes (CLI) ------------------------------------------

    #[test]
    fn dispatch_suite_without_value_exits_3() {
        // M3: `--suite` as the LAST token must NOT silently fall back to the
        // default suite — fail loud so the operator notices the typo.
        let args: Vec<String> = vec![
            "coordination".into(),
            "validate".into(),
            "--suite".into(), // no value follows
        ];
        let result = dispatch(&args);
        assert_eq!(exit_of(result), 3);
    }

    #[test]
    fn dispatch_repo_root_without_value_exits_3() {
        // M3 echo for --repo-root.
        let args: Vec<String> = vec![
            "coordination".into(),
            "validate".into(),
            "--repo-root".into(),
        ];
        let result = dispatch(&args);
        assert_eq!(exit_of(result), 3);
    }

    #[test]
    fn dispatch_broken_suite_exits_2() {
        // M2: a suite that DESERIALIZES but fails structural validation (only
        // 1 of 9 dims -> 8 MissingDimension errors) must be a gate REJECTION
        // (exit 2), not a silent pass (exit 0). Built from explicit lines so
        // the YAML round-trips through serde cleanly.
        let tmp =
            std::env::temp_dir().join(format!("forge-broken-suite-{}.yaml", std::process::id()));
        let content = [
            "schema_version: \"0.1\"",
            "coordination_eval_contract:",
            "  id: x",
            "  contract_ref: x.yaml",
            "  status: draft",
            "  dimensions:",
            "    - dimension: lane_collision_detection",
            "      metric_kind: fixture_pass",
            "      required_level: must_pass",
            "      fixture_refs: [x.yaml]",
            "      threshold: null",
            "      failure_signal: x",
            "      evidence_refs: []",
            "  pass_policy:",
            "    required_level: must_pass",
            "    all_must_pass_dimensions_required: true",
            "    manual_review_blocks_release: true",
            "",
        ]
        .join("\n");
        std::fs::write(&tmp, content).unwrap();
        let args: Vec<String> = vec![
            "coordination".into(),
            "validate".into(),
            "--suite".into(),
            tmp.to_string_lossy().into_owned(),
            "--repo-root".into(),
            ".".into(),
        ];
        // Exit 2 = gate rejection. The envelope's "ok":false / "is_real":false
        // JSON shape is still asserted via `run_validate` + serialization below
        // (dispatch no longer returns the JSON string after V1.D).
        let result = dispatch(&args);
        assert_eq!(exit_of(result), 2, "broken suite must reject");
        let env = run_validate(&tmp, Path::new("."));
        let json = serde_json::to_string(&env).expect("serialize");
        // Compact JSON (serde_json::to_string) emits no space after the colon.
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("\"is_real\":false"));
        let _ = std::fs::remove_file(&tmp);
    }
}
