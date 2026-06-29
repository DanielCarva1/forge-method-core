//! Typed CLI exit errors.
//!
//! Before R8 every dispatcher in the `forge-core-cli` crate called
//! `std::process::exit(N)` directly when it hit a usage error, a verification
//! failure, or a governance rejection. That made the command modules
//! impossible to unit-test as plain functions: any error path killed the
//! process before the caller could observe it.
//!
//! `ExitError` replaces those bare exits with a typed enum that the command
//! dispatchers return as `Result<(), ExitError>`. The single remaining
//! `process::exit` lives at the top of `main.rs`, which converts an
//! `ExitError` into the right exit code and stderr message.
//!
//! See
//! `docs/dev-docs/forge-method-core-dev-docs-v2/09_system_design_roadmap.md`
//! (Fase 2 / R8) for the full rationale.

use std::fmt;

/// The terminal error returned by every CLI dispatcher.
///
/// Variants correspond to the stable exit-code contract already documented
/// in `forge_core_contracts::envelope::ExitReason`. Each variant knows its
/// own exit code via [`ExitError::exit_code`].
///
/// `Clone` is derived so callers can store a copy for diagnostics before
/// propagating (mirrors the `Clone` discipline of the project's other
/// hand-rolled error enums).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitError {
    /// Usage / parse error: missing flag, wrong command, etc.
    ///
    /// Maps to exit code 2 (matches `ExitReason::RejectedByGate` as used
    /// historically for argv-shape failures in `main.rs`).
    Usage {
        /// Stderr-quality human-readable diagnostic.
        message: String,
    },
    /// Command failed: verification returned errors, IO failed, store
    /// rejected the operation, etc.
    ///
    /// Maps to exit code 1 (the historical "command failed" code used by
    /// `emit_envelope` and the verify family).
    Failed {
        /// Stderr-quality human-readable diagnostic.
        message: String,
    },
    /// A flag value was missing or looked like another flag, in a governance
    /// context where silent coercion is forbidden (review S4.4 medium).
    ///
    /// Maps to exit code 3 (matches `ExitReason::InvalidDecisionShape`).
    InvalidValue {
        /// Stderr-quality human-readable diagnostic.
        message: String,
    },
    /// WAL or integrity conflict detected while committing state.
    ///
    /// Maps to exit code 4 (matches `ExitReason::Conflict`).
    Conflict {
        /// Stderr-quality human-readable diagnostic.
        message: String,
    },
    /// Environment or configuration error: missing sidecar, malformed
    /// `.forge-method.yaml`, etc.
    ///
    /// Maps to exit code 5 (matches `ExitReason::EnvConfig`).
    EnvConfig {
        /// Stderr-quality human-readable diagnostic.
        message: String,
    },
    /// Escape hatch for callers that have already computed a dynamic exit
    /// code (for example by inspecting a fully built `CliEnvelope`).
    ///
    /// Prefer one of the typed variants above when the code is known
    /// statically. This variant exists so `emit_envelope` can be migrated
    /// without losing its dynamic-code behavior.
    WithCode {
        /// Exit code to return to the shell.
        code: i32,
        /// Stderr-quality human-readable diagnostic.
        message: String,
    },
}

impl ExitError {
    /// Construct a `Usage` variant from anything that formats into a
    /// `String`. Convenience for the common case of `format!("...")`.
    #[must_use]
    pub fn usage(message: impl Into<String>) -> Self {
        Self::Usage {
            message: message.into(),
        }
    }

    /// Construct a `Failed` variant.
    #[must_use]
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    /// Construct an `InvalidValue` variant.
    #[must_use]
    pub fn invalid_value(message: impl Into<String>) -> Self {
        Self::InvalidValue {
            message: message.into(),
        }
    }

    /// Construct a `Conflict` variant.
    #[must_use]
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict {
            message: message.into(),
        }
    }

    /// Construct an `EnvConfig` variant.
    #[must_use]
    pub fn env_config(message: impl Into<String>) -> Self {
        Self::EnvConfig {
            message: message.into(),
        }
    }

    /// Construct a `WithCode` variant.
    #[must_use]
    pub fn with_code(code: i32, message: impl Into<String>) -> Self {
        Self::WithCode {
            code,
            message: message.into(),
        }
    }

    /// The shell exit code this error maps to.
    ///
    /// Matches the historical contract baked into the codebase before R8:
    ///
    /// | Variant        | Code | Historical meaning                          |
    /// |----------------|------|---------------------------------------------|
    /// | `Usage`        | 2    | argv shape error (`next_arg`, `parse_*`)    |
    /// | `Failed`       | 1    | verification / IO / generic failure         |
    /// | `InvalidValue` | 3    | governance strict-value rejection           |
    /// | `Conflict`     | 4    | WAL / integrity conflict                    |
    /// | `EnvConfig`    | 5    | environment / configuration error           |
    /// | `WithCode`     | n    | dynamic code carried by the caller          |
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage { .. } => 2,
            Self::Failed { .. } => 1,
            Self::InvalidValue { .. } => 3,
            Self::Conflict { .. } => 4,
            Self::EnvConfig { .. } => 5,
            Self::WithCode { code, .. } => *code,
        }
    }

    /// The stderr-quality message, without the variant prefix.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Usage { message }
            | Self::Failed { message }
            | Self::InvalidValue { message }
            | Self::Conflict { message }
            | Self::EnvConfig { message }
            | Self::WithCode { message, .. } => message,
        }
    }
}

impl fmt::Display for ExitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display only the message; the variant is encoded in the exit code
        // and does not need to clutter stderr. This keeps stderr output
        // byte-identical to the pre-R8 `eprintln!("... failed: {msg}")`
        // pattern.
        f.write_str(self.message())
    }
}

impl std::error::Error for ExitError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_historical_contract() {
        assert_eq!(ExitError::usage("x").exit_code(), 2);
        assert_eq!(ExitError::failed("x").exit_code(), 1);
        assert_eq!(ExitError::invalid_value("x").exit_code(), 3);
        assert_eq!(ExitError::conflict("x").exit_code(), 4);
        assert_eq!(ExitError::env_config("x").exit_code(), 5);
        assert_eq!(ExitError::with_code(7, "x").exit_code(), 7);
    }

    #[test]
    fn display_returns_only_message_for_stable_stderr() {
        assert_eq!(ExitError::usage("bad flag").to_string(), "bad flag");
        assert_eq!(
            ExitError::with_code(0, "envelope ok payload").to_string(),
            "envelope ok payload"
        );
    }

    #[test]
    fn constructors_build_expected_variants() {
        assert_eq!(
            ExitError::usage("m"),
            ExitError::Usage {
                message: "m".to_string()
            }
        );
        assert_eq!(
            ExitError::failed("m"),
            ExitError::Failed {
                message: "m".to_string()
            }
        );
        assert_eq!(
            ExitError::invalid_value("m"),
            ExitError::InvalidValue {
                message: "m".to_string()
            }
        );
        assert_eq!(
            ExitError::conflict("m"),
            ExitError::Conflict {
                message: "m".to_string()
            }
        );
        assert_eq!(
            ExitError::env_config("m"),
            ExitError::EnvConfig {
                message: "m".to_string()
            }
        );
        assert_eq!(
            ExitError::with_code(9, "m"),
            ExitError::WithCode {
                code: 9,
                message: "m".to_string()
            }
        );
    }

    #[test]
    fn message_accessor_is_variant_agnostic() {
        for err in [
            ExitError::usage("a"),
            ExitError::failed("b"),
            ExitError::invalid_value("c"),
            ExitError::conflict("d"),
            ExitError::env_config("e"),
            ExitError::with_code(0, "f"),
        ] {
            assert_eq!(err.message(), err.to_string());
        }
    }

    #[test]
    fn implements_std_error_for_anyhow_free_composition() {
        fn takes_error<E: std::error::Error>(_: &E) {}
        takes_error(&ExitError::failed("x"));
    }
}
