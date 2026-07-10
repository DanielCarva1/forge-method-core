// The operation executor and the staged-read-only-command runner carry
// long bodies by design: they walk the runtime effect plan step by step,
// capturing per-step evidence into the typed execution result. Splitting
// them just to satisfy `clippy::too_many_lines` would obscure the linear
// step ordering.
#![allow(clippy::too_many_lines)]
// The four internal modules (planning/staging/evidence/wal_orchestration)
// use `use super::*;` to inherit the shared external imports declared once
// in this facade (lib.rs). This is the idiomatic pattern for a mechanical
// code split: it keeps the move byte-faithful and lets each module reach the
// crate-level `use` block + sibling re-exports without restating 30+ names.
// Explicit per-module imports would add churn without aiding navigation here.
#![allow(clippy::wildcard_imports)]

//! `forge-core-kernel` — the deterministic mutation kernel (ADR-0001).
//!
//! This is the only crate in the workspace that mutates state. The crate is
//! organised as a thin facade over private internal modules, following the
//! `rustc_middle` / `DataFusion` pattern of deep internals behind a small public
//! surface:
//!
//! - `planning` — read-only analysis: `plan_operation`, `preview_operation`,
//!   `ready_operation` and their supporting report types. Never mutates.
//! - `staging` — `stage_operation_effects` and the staging-plan types that
//!   gate which commands/effects are eligible for application.
//! - `evidence` — `run_staged_read_only_command` and the evidence-record
//!   builder that durable-logs each command execution.
//! - `wal_orchestration` — `execute_operation`, the single public mutation
//!   entrypoint, plus `prepare_effect_transaction` and the execution-result
//!   types.
//! - `prepared_execution` — the dormant P4b.2b authority path: canonical
//!   commit descriptor, retained effect/replay locks, double file preflight,
//!   trusted late snapshot, and pre-WAL Execution Admission typestate. It has
//!   no effect-commit method yet.
//! - [`gate`] — the `OperationGate` trait and `GateRejection` type: mutation
//!   preconditions the kernel runs before any WAL append. V2.C builds the seam
//!   (the trait + typestate context); V3.A fills it with real gates.
//! - [`builtin_gates`] — V3.A's two built-in gates (`RiskAuditGate`,
//!   `CitationGate`) that the CLI attaches via `.with_gate(...)`.
//!
//! Every previously-top-level `pub` item is re-exported at the crate root
//! below, so downstream crates see an unchanged public interface
//! (`forge_core_kernel::execute_operation`, etc.).
//!

// Shared external imports. Every internal module begins with
// `use super::*;`, so these are visible to all four modules without
// re-declaration. Module-specific imports are added locally where needed.
use forge_core_contracts::command::{
    CommandExecutor, CommandSideEffectPolicy, CwdPolicy, EnvInheritPolicy, EnvPolicy,
    NetworkPolicy, Platform,
};
use forge_core_contracts::operation::{
    AutonomyMode, CommandRef, ExecutionMode, ForgeOperation, HostAction, HumanInputRequirement,
    HumanPrompt, MutationPolicy, NextActor, OperationGateStatus, OperationSideEffectPolicy,
    RequiredGate,
};
use forge_core_contracts::tool_effect::{AccessMode, InverseKind, ToolEffectContractDocument};
use forge_core_contracts::{
    CommandContractDocument, OperationContractDocument, RepoPath, StableId,
};
use forge_core_store::{
    append_effect_target_metadata_records_with_durability, append_json_line_with_durability,
    apply_file_effect_transaction_with_wal_lock_with_durability, EffectApplicationPayload,
    EffectApplicationResult, EffectApplicationStatus, WalDurability,
};
use forge_core_validate::{
    validate_command, validate_operation, validate_operation_cross_references,
    validate_tool_effect, DiagnosticSeverity, ReferenceIndex,
};
use serde::Serialize;
use std::env;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tracing::instrument;

mod evidence;
pub mod gate;
mod planning;
mod prepared_execution;
mod staging;
mod wal_orchestration;

// V3.A: the two built-in mutation gates (risk-audit, citation). Public so the
// CLI can construct them from config and attach them to the execution context.
pub mod builtin_gates;

// Re-export the entire public API at the crate root. This preserves every
// historical path 1:1 — downstream crates need no import changes.
pub use builtin_gates::*;
pub use evidence::*;
pub use gate::*;
pub use planning::*;
pub use prepared_execution::*;
pub use staging::*;
pub use wal_orchestration::*;

// Shared private helper used by both `staging` (`stage_operation_effects`
//) and `planning` (`preview_operation_from_plan`, `preview_runtime_plan`).
// Kept here so neither module owns it over the other.
fn mutating_side_effect(policy: OperationSideEffectPolicy) -> bool {
    matches!(
        policy,
        OperationSideEffectPolicy::WriteProjectFiles
            | OperationSideEffectPolicy::RunCommands
            | OperationSideEffectPolicy::Publish
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::tool_effect::EffectTargetKind;
    use forge_core_store::{build_reference_index, sha256_content_hash};
    use std::fs;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn operation_fixture(name: &str) -> OperationContractDocument {
        let path = repo_root()
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0")
            .join(name);
        let input = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        yaml_serde::from_str(&input)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }

    fn effect_fixture(name: &str) -> ToolEffectContractDocument {
        let path = repo_root().join("contracts").join("effects").join(name);
        let input = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        yaml_serde::from_str(&input)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }

    fn fresh_temp_root(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-core-kernel-lib-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    fn runtime_payload(target_ref: &str, content: &[u8]) -> RuntimeOperationEffectPayload {
        RuntimeOperationEffectPayload {
            target_ref: target_ref.to_string(),
            payload_kind: RuntimeEffectPayloadKind::RuntimeGenerated,
            content_hash: sha256_content_hash(content),
            content: content.to_vec(),
        }
    }

    #[test]
    fn execute_operation_reports_applied_but_metadata_missing_when_index_append_fails() {
        let mut document = operation_fixture("mechanical-story-execute.yaml");
        document.operation_contract.command_refs.clear();
        let index = build_reference_index(repo_root()).expect("reference index");
        let mut effect = effect_fixture("story-artifact-write-effect.yaml");
        effect.tool_effect_contract.read_set.truncate(1);
        effect.tool_effect_contract.read_set[0].target_kind = EffectTargetKind::FilePath;
        effect.tool_effect_contract.read_set[0].reference =
            ".forge-method/stories/current.yaml".to_string();
        effect.tool_effect_contract.read_set[0].expected_hash = None;
        effect.tool_effect_contract.read_set[0].expected_version = None;
        let effect_input = RuntimeOperationEffectInput {
            effect_ref: RepoPath("contracts/effects/story-artifact-write-effect.yaml".to_string()),
            document: effect,
        };
        let artifact_payload = runtime_payload(
            ".forge-method/artifacts/story-current-result.yaml",
            b"story: completed\n",
        );
        let evidence_payload = runtime_payload(
            ".forge-method/evidence/story-validation.json",
            br#"{"status":"passed"}"#,
        );
        let temp_root = fresh_temp_root("metadata-append-failure");
        let index_path = temp_root.join(".forge-method/index/effect-targets.ndjson");
        fs::create_dir_all(&index_path).expect("create directory where metadata file should be");
        let mut context = RuntimeOperationExecutionContext::single_root(&temp_root);
        context.recorded_at = "2026-06-25T00:00:00Z";
        context.tx_id_prefix = "test-execute-operation";
        let context = context.audited();

        let execution = execute_operation(
            &document,
            RuntimeReadSnapshot::new(&index),
            &[],
            &[effect_input],
            &[artifact_payload, evidence_payload],
            &context,
        )
        .expect("execute_operation should not be rejected by gates");

        assert_eq!(
            execution.status,
            RuntimeOperationExecutionStatus::AppliedButMetadataMissing,
            "{execution:#?}"
        );
        assert_eq!(
            execution.reasons,
            vec![
                RuntimeOperationExecutionReason::EffectMetadataAppendFailed,
                RuntimeOperationExecutionReason::RebuildEffectIndexSuggested,
            ]
        );
        assert_eq!(execution.effect_applications.len(), 1);
        assert_eq!(
            execution.effect_applications[0].status,
            EffectApplicationStatus::Applied
        );
        assert!(temp_root
            .join(".forge-method/artifacts/story-current-result.yaml")
            .exists());
        assert!(temp_root.join(".forge-method/wal/effects.ndjson").exists());
        assert!(index_path.is_dir());
    }
}
