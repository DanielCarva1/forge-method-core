//! Mutation preconditions that the kernel runs internally before any WAL append.
//!
//! A gate is a Policy Enforcement Point that the kernel consults; the Policy
//! Decision Point is whatever backs the gate (a risk-audit ruleset, a citation
//! policy). The kernel does not know what a gate checks — it only knows that a
//! gate either passes or rejects with a typed reason. This inverts the
//! dependency: the CLI/risk-audit crate implements [`OperationGate`], the kernel
//! only consumes the trait.
//!
//! Modeled on Tower's `Service`/`Layer` pattern, but synchronous (no async) to
//! honor ADR-0001's deterministic-kernel constraint.

#![allow(clippy::missing_errors_doc)]

use crate::planning::RuntimePlan;

/// A typed rejection from a mutation gate. Carries enough structure for the
/// envelope (V2.D) and MCP consumer to branch on, not just a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateRejection {
    /// The risk-audit ruleset found anti-pattern matches at Error severity.
    RiskAuditFailed {
        error_count: usize,
        finding_paths: Vec<String>,
    },
    /// A claim's `source_id` does not resolve against the joint backing.
    CitationCheckFailed { unresolved_source_ids: Vec<String> },
    /// A gate the kernel doesn't recognize the internals of rejected. Used by
    /// generic/custom gates.
    Custom { code: String, message: String },
}

impl std::fmt::Display for GateRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateRejection::RiskAuditFailed {
                error_count,
                finding_paths,
            } => {
                write!(
                    formatter,
                    "risk-audit gate failed with {error_count} error(s)"
                )?;
                if finding_paths.is_empty() {
                    Ok(())
                } else {
                    write!(formatter, "; findings: {}", finding_paths.join(", "))
                }
            }
            GateRejection::CitationCheckFailed {
                unresolved_source_ids,
            } => {
                write!(
                    formatter,
                    "citation gate failed; unresolved source_id(s): {}",
                    unresolved_source_ids.join(", ")
                )
            }
            GateRejection::Custom { code, message } => {
                write!(formatter, "gate `{code}` rejected: {message}")
            }
        }
    }
}

/// A non-forgeable, operation-scoped gate capability. It exposes only trace
/// emission under the effect-producer admission retained by `execute_operation`;
/// gates cannot choose a state root or self-admit a separate producer boundary.
pub struct GateEvaluationContext<'a> {
    state_root: &'a std::path::Path,
    boundary: &'a forge_core_store::producer_quiescence::EffectProducerGuard,
}

impl<'a> GateEvaluationContext<'a> {
    pub(crate) fn new(
        state_root: &'a std::path::Path,
        boundary: &'a forge_core_store::producer_quiescence::EffectProducerGuard,
    ) -> Self {
        Self {
            state_root,
            boundary,
        }
    }

    /// Persist a trace through the operation's retained producer boundary.
    pub fn append_trace_event(
        &self,
        event: &forge_core_trace::TraceEvent,
    ) -> Result<std::path::PathBuf, forge_core_store::AppendJsonLineError> {
        forge_core_store::append_trace_event_under_boundary(self.boundary, self.state_root, event)
    }
}

/// A mutation precondition. Implementations receive the read-only plan and an
/// operation-bound trace capability. The capability is minted only after the
/// kernel validates layout and acquires its one producer admission.
pub trait OperationGate {
    /// Return `Err(GateRejection)` to fail-closed or `Ok(())` to permit the
    /// next gate. Persistent gate telemetry must use `context`, never a
    /// caller-selected root or a generic self-admitting Store wrapper.
    fn evaluate(
        &self,
        plan: &RuntimePlan,
        context: &GateEvaluationContext<'_>,
    ) -> Result<(), GateRejection>;

    /// Human-readable name for audit/trace (e.g. `"risk-audit"`, `"citation"`).
    fn name(&self) -> &'static str;
}
