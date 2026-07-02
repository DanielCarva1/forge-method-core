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

/// A mutation precondition. Implementations live OUTSIDE the kernel (the
/// risk-audit evaluator, the citation checker); the kernel only calls
/// [`evaluate`](OperationGate::evaluate).
///
/// The gate receives the plan (what WILL happen) and may reject it. It must be
/// fail-closed: returning `Err(GateRejection)` blocks the mutation; returning
/// `Ok(())` allows it but does NOT authorize it (the kernel's own
/// `OperationContract` authorization is separate and still runs).
///
/// # Object safety
///
/// Both methods take `&self` and return owned/sized data, so the trait is
/// object-safe: `Box<dyn OperationGate>` works and is what the typestate
/// execution context stores in its gate chain.
pub trait OperationGate {
    /// Evaluate the gate against the planned mutation.
    ///
    /// Returns `Err(GateRejection)` to fail-closed (block the WAL append) or
    /// `Ok(())` to allow the mutation to proceed to the next gate / the kernel's
    /// own authorization. The plan is the read-only projection of what the
    /// operation WILL do; the gate never mutates it.
    ///
    /// # Errors
    ///
    /// Returns `Err(GateRejection)` when the gate rejects the mutation. Gates are
    /// fail-closed: any rejection blocks the WAL append, so the mutation does not
    /// take effect. Returning `Ok(())` permits the mutation but does NOT authorize
    /// it (the kernel's own `OperationContract` authorization runs separately).
    fn evaluate(&self, plan: &RuntimePlan) -> Result<(), GateRejection>;

    /// Human-readable name for audit/trace (e.g. `"risk-audit"`, `"citation"`).
    fn name(&self) -> &'static str;
}
