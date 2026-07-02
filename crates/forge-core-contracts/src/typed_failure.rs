//! `TypedFailure` — the typed failure vocabulary carried by [`CliEnvelope`]
//! (V2.D).
//!
//! The mutate path's failures used to be stringified at the collapse site,
//! losing variant info that a programmatic consumer (MCP/agent) then had to
//! re-parse out of free text. `TypedFailure` rides alongside the existing
//! human-readable `error.message` in [`CliEnvelope`], so consumers can branch
//! on *why* an operation failed without parsing prose.
//!
//! # Serialization — adjacently tagged, never `untagged`
//!
//! The enum uses serde's **adjacently-tagged** representation:
//! `{"type": "<variant>", "data": {...}}`. This round-trips the variant,
//! accepts any payload shape (unit / newtype / struct), and is unambiguous.
//! `untagged` is forbidden here — it loses variant fidelity (serde issue
//! #1307). See `forge-core-validate/src/failure.rs` for the full rationale.
//!
//! The tag/content field names `"type"` and `"data"` are hardcoded below as
//! string literals because serde attributes require literals (a `const` ref is
//! not allowed in an attribute). They MUST stay in sync with the constants in
//! `forge-core-validate/src/failure.rs` (`ADJACENT_TAG = "type"`,
//! `ADJACENT_CONTENT = "data"`) — those are the single source of truth and are
//! pinned by a test there. (`forge-core-contracts` cannot depend on
//! `forge-core-validate` — the dependency direction is validate → contracts —
//! so the names are duplicated here as literals with a cross-reference, not a
//! code link.)
//!
//! [`CliEnvelope`]: crate::envelope::CliEnvelope

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A typed failure reason carried by [`CliEnvelope`] alongside the human
/// `error.message`.
///
/// Variants are folded from the mutate path's error enums
/// (`ExecuteOperationError` in `forge-core-cli`, `GateRejection` in
/// `forge-core-kernel`) so a programmatic consumer can branch on the failure
/// category. The human-readable diagnostic still rides in
/// [`CliEnvelope`]'s `error.message` for stderr; this enum is the
/// machine-readable mirror.
///
/// Field names are wire-stable: renaming a variant or a field is a breaking
/// change to the envelope contract.
///
/// [`CliEnvelope`]: crate::envelope::CliEnvelope
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum TypedFailure {
    /// A contract/input file could not be parsed, or an effect path was not
    /// under its expected root. `reasons` carries one entry per problem.
    InvalidContract { reasons: Vec<String> },
    /// An operation/command/effect contract path resolved outside the
    /// project `--root` provenance boundary.
    ContractPathOutsideRoot { path: String },
    /// A payload path resolved outside the allowed scope (outside `--root`
    /// when `--allow-payload-outside-root` is not set).
    PayloadOutsideRoot { path: String },
    /// A payload exceeded the configured `--max-payload-bytes` cap.
    PayloadTooLarge { path: String, max_bytes: u64 },
    /// The risk-audit gate failed closed. `error_count` is the total of
    /// Error-severity findings; `finding_paths` lists them (path + message).
    RiskAuditFailed {
        error_count: usize,
        finding_paths: Vec<String>,
    },
    /// The citation gate failed closed: one or more `source_id`s did not
    /// resolve against the curated Field Evidence Registry ∪ the runtime
    /// Source Ledger.
    CitationCheckFailed { unresolved_source_ids: Vec<String> },
    /// A kernel mutation gate rejected the operation. `rejection` is the
    /// stringified kernel `GateRejection` (the typed enum stays in the
    /// kernel; the envelope carries only the lossy string for display).
    GateRejected { rejection: String },
    /// A store or IO read failure (contract read, reference-index build,
    /// WAL). `path` is the affected file when known, else empty.
    StoreError { path: String, source: String },
    /// The operation reached the runtime but did not complete for a reason
    /// not covered by a more specific variant.
    ExecutionFailed { reason: String },
    /// Catch-all for failures with no clean typed home.
    Other { message: String },
}

impl fmt::Display for TypedFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract { reasons } => {
                if reasons.is_empty() {
                    f.write_str("invalid contract")
                } else {
                    write!(f, "invalid contract: {}", reasons.join("; "))
                }
            }
            Self::ContractPathOutsideRoot { path } => {
                write!(f, "contract path outside root: {path}")
            }
            Self::PayloadOutsideRoot { path } => {
                write!(f, "payload path outside root: {path}")
            }
            Self::PayloadTooLarge { path, max_bytes } => {
                write!(f, "payload {path} too large (max {max_bytes} bytes)")
            }
            Self::RiskAuditFailed {
                error_count,
                finding_paths,
            } => {
                write!(f, "risk-audit gate failed with {error_count} error(s)")?;
                if finding_paths.is_empty() {
                    Ok(())
                } else {
                    write!(f, "; findings: {}", finding_paths.join(", "))
                }
            }
            Self::CitationCheckFailed {
                unresolved_source_ids,
            } => {
                if unresolved_source_ids.is_empty() {
                    f.write_str("citation gate failed")
                } else {
                    write!(
                        f,
                        "citation gate failed; unresolved source_id(s): {}",
                        unresolved_source_ids.join(", ")
                    )
                }
            }
            Self::GateRejected { rejection } => {
                write!(f, "mutation gate rejected: {rejection}")
            }
            Self::StoreError { path, source } => {
                if path.is_empty() {
                    write!(f, "store error: {source}")
                } else {
                    write!(f, "store error at {path}: {source}")
                }
            }
            Self::ExecutionFailed { reason } => write!(f, "execution failed: {reason}"),
            Self::Other { message } => f.write_str(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_tag_round_trips_through_json() {
        // The adjacently-tagged representation must survive a JSON round-trip
        // so an MCP consumer can deserialize the variant it branched on.
        let fail = TypedFailure::PayloadTooLarge {
            path: "/tmp/big.bin".into(),
            max_bytes: 1_048_576,
        };
        let json = serde_json::to_string(&fail).unwrap();
        assert!(
            json.contains(r#""type":"payload_too_large""#),
            "expected snake_case adjacent tag, got: {json}"
        );
        assert!(
            json.contains(r#""data""#),
            "expected adjacent content field"
        );
        let back: TypedFailure = serde_json::from_str(&json).unwrap();
        assert_eq!(back, fail);
    }

    #[test]
    fn all_variants_round_trip() {
        // Pin wire-stability: every variant must round-trip and keep its tag.
        let cases = vec![
            (
                TypedFailure::InvalidContract {
                    reasons: vec!["a".into()],
                },
                "invalid_contract",
            ),
            (
                TypedFailure::ContractPathOutsideRoot { path: "/x".into() },
                "contract_path_outside_root",
            ),
            (
                TypedFailure::PayloadOutsideRoot { path: "/x".into() },
                "payload_outside_root",
            ),
            (
                TypedFailure::PayloadTooLarge {
                    path: "/x".into(),
                    max_bytes: 10,
                },
                "payload_too_large",
            ),
            (
                TypedFailure::RiskAuditFailed {
                    error_count: 2,
                    finding_paths: vec!["p".into()],
                },
                "risk_audit_failed",
            ),
            (
                TypedFailure::CitationCheckFailed {
                    unresolved_source_ids: vec!["s1".into()],
                },
                "citation_check_failed",
            ),
            (
                TypedFailure::GateRejected {
                    rejection: "custom".into(),
                },
                "gate_rejected",
            ),
            (
                TypedFailure::StoreError {
                    path: "/x".into(),
                    source: "io".into(),
                },
                "store_error",
            ),
            (
                TypedFailure::ExecutionFailed {
                    reason: "boom".into(),
                },
                "execution_failed",
            ),
            (
                TypedFailure::Other {
                    message: "misc".into(),
                },
                "other",
            ),
        ];
        for (fail, expected_tag) in cases {
            let json = serde_json::to_string(&fail).unwrap();
            assert!(
                json.contains(&format!(r#""type":"{expected_tag}""#)),
                "{fail:?} did not serialize with tag {expected_tag}: {json}"
            );
            let back: TypedFailure = serde_json::from_str(&json).unwrap();
            assert_eq!(back, fail, "round-trip mismatch for tag {expected_tag}");
        }
    }

    #[test]
    fn display_is_nonempty_for_every_variant() {
        let cases = [
            TypedFailure::InvalidContract { reasons: vec![] },
            TypedFailure::InvalidContract {
                reasons: vec!["x".into()],
            },
            TypedFailure::ContractPathOutsideRoot { path: "/p".into() },
            TypedFailure::PayloadOutsideRoot { path: "/p".into() },
            TypedFailure::PayloadTooLarge {
                path: "/p".into(),
                max_bytes: 1,
            },
            TypedFailure::RiskAuditFailed {
                error_count: 1,
                finding_paths: vec![],
            },
            TypedFailure::RiskAuditFailed {
                error_count: 1,
                finding_paths: vec!["f".into()],
            },
            TypedFailure::CitationCheckFailed {
                unresolved_source_ids: vec![],
            },
            TypedFailure::CitationCheckFailed {
                unresolved_source_ids: vec!["s".into()],
            },
            TypedFailure::GateRejected {
                rejection: "r".into(),
            },
            TypedFailure::StoreError {
                path: String::new(),
                source: "io".into(),
            },
            TypedFailure::StoreError {
                path: "/p".into(),
                source: "io".into(),
            },
            TypedFailure::ExecutionFailed { reason: "r".into() },
            TypedFailure::Other {
                message: "m".into(),
            },
        ];
        for fail in cases {
            assert!(!fail.to_string().is_empty(), "{fail:?} displayed empty");
        }
    }
}
