//! `CliEnvelope` — the agent-facing API contract for every `guide/*` command.
//!
//! Implements R1 (stdout is contract) and R6 (versioned, never mutate published
//! shape) from the slice-3 spec. Every command emits exactly one
//! [`CliEnvelope`] as JSON to stdout; all diagnostics go to stderr.
//!
//! ## Exit-code taxonomy (DD10)
//!
//! | code | meaning                       | envelope.ok |
//! |------|-------------------------------|-------------|
//! | 0    | ok                            | true        |
//! | 2    | rejected by gate              | false       |
//! | 3    | invalid decision shape        | false       |
//! | 4    | conflict / WAL error          | false       |
//! | 5    | env / config error            | false       |
//!
//! The mapping is deterministic and documented in `guide describe`.

use crate::StableId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Current schema version of the CLI envelope contract.
pub const ENVELOPE_SCHEMA_VERSION: &str = "0.1";

/// The deterministic exit codes for agent-consumed commands (DD10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[repr(u8)]
pub enum ExitReason {
    /// ok — command succeeded.
    Ok = 0,
    /// rejected-by-gate — engine refused the decision (legal but blocked).
    RejectedByGate = 2,
    /// invalid-decision-shape — input could not be parsed/validated structurally.
    InvalidDecisionShape = 3,
    /// conflict — WAL/integrity conflict.
    Conflict = 4,
    /// env-config — environment or configuration error.
    EnvConfig = 5,
}

impl ExitReason {
    #[must_use]
    pub fn as_code(self) -> i32 {
        self as i32
    }

    /// The machine-readable reason string emitted in the envelope.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::RejectedByGate => "rejected_by_gate",
            Self::InvalidDecisionShape => "invalid_decision_shape",
            Self::Conflict => "conflict",
            Self::EnvConfig => "env_config",
        }
    }
}

/// A typed error in the envelope payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CliError {
    /// Machine-readable error code (matches [`ExitReason::as_str`] for the
    /// terminal cases; may carry sub-codes on `data`).
    pub code: StableId,
    /// Human-readable diagnostic (stderr-quality; included so a debugging host
    /// can surface it, but NOT required for routing).
    pub message: String,
}

/// The single envelope shape emitted to stdout by every `guide/*` command.
///
/// Generic over the success payload `T` so each command carries its own typed
/// `data` while sharing one stable envelope contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CliEnvelope<T> {
    /// Envelope contract version (currently [`ENVELOPE_SCHEMA_VERSION`]).
    pub schema_version: StableId,
    /// Which command produced this envelope (e.g. `guide.describe`).
    pub command: StableId,
    /// True iff the command succeeded (exit 0).
    pub ok: bool,
    /// Machine-readable terminal reason (see [`ExitReason`]).
    pub exit_reason: StableId,
    /// Success payload. Present iff `ok == true`, EXCEPT for rejection cases
    /// produced by [`CliEnvelope::reject`], which carry structured self-
    /// correction data alongside a non-zero exit code (e.g. `check-write`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Typed error. Present iff `ok == false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CliError>,
}

impl<T> CliEnvelope<T> {
    /// Build a success envelope carrying `data`. Exit code 0.
    #[must_use]
    pub fn ok(command: &str, data: T) -> Self {
        Self {
            schema_version: StableId(ENVELOPE_SCHEMA_VERSION.into()),
            command: StableId(command.into()),
            ok: true,
            exit_reason: StableId(ExitReason::Ok.as_str().into()),
            data: Some(data),
            error: None,
        }
    }

    /// Build a failure envelope. `exit` drives both `ok=false` and the
    /// `exit_reason`; `message` is the diagnostic. Exit code = `exit.as_code()`.
    #[must_use]
    pub fn err(command: &str, exit: ExitReason, message: impl Into<String>) -> Self {
        Self {
            schema_version: StableId(ENVELOPE_SCHEMA_VERSION.into()),
            command: StableId(command.into()),
            ok: false,
            exit_reason: StableId(exit.as_str().into()),
            data: None,
            error: Some(CliError {
                code: StableId(exit.as_str().into()),
                message: message.into(),
            }),
        }
    }

    /// Build an envelope that signals failure (exit code from `exit`) but ALSO
    /// carries structured `data` for self-correction.
    ///
    /// This is the rare case where a rejection is itself rich information the
    /// caller needs programmatically (e.g. `check-write` blocked lists the
    /// colliding paths, claim ids, and owners so the writer can plan). It keeps
    /// the DD10 exit code (the shell still sees the rejection) while preserving
    /// the DD17 machine-readable payload. Use sparingly — `err` (no data) is
    /// the default for ordinary failures.
    #[must_use]
    pub fn reject(command: &str, exit: ExitReason, message: impl Into<String>, data: T) -> Self {
        Self {
            schema_version: StableId(ENVELOPE_SCHEMA_VERSION.into()),
            command: StableId(command.into()),
            ok: false,
            exit_reason: StableId(exit.as_str().into()),
            data: Some(data),
            error: Some(CliError {
                code: StableId(exit.as_str().into()),
                message: message.into(),
            }),
        }
    }

    /// The deterministic process exit code for this envelope.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        let reason = self.exit_reason.0.as_str();
        match reason {
            "ok" => ExitReason::Ok.as_code(),
            "rejected_by_gate" => ExitReason::RejectedByGate.as_code(),
            "invalid_decision_shape" => ExitReason::InvalidDecisionShape.as_code(),
            // Unknown/unexpected reason: default to env/config (5) — surfaces a contract bug loudly,
            // since a well-formed envelope always carries a known reason.
            _ => ExitReason::EnvConfig.as_code(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Payload {
        phase: String,
    }

    #[test]
    fn ok_envelope_carries_data_and_exit_zero() {
        let env: CliEnvelope<Payload> = CliEnvelope::ok(
            "guide.status",
            Payload {
                phase: "1-discovery".into(),
            },
        );
        assert!(env.ok);
        assert_eq!(env.exit_reason.0, "ok");
        assert_eq!(env.exit_code(), 0);
        assert_eq!(env.data.as_ref().unwrap().phase, "1-discovery");
        assert!(env.error.is_none());
    }

    #[test]
    fn err_envelope_carries_typed_reason_and_nonzero_exit() {
        let env: CliEnvelope<Payload> = CliEnvelope::err(
            "guide.decide",
            ExitReason::RejectedByGate,
            "missing system-design gate",
        );
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, "rejected_by_gate");
        assert_eq!(env.exit_code(), 2);
        let err = env.error.as_ref().unwrap();
        assert_eq!(err.code.0, "rejected_by_gate");
        assert!(err.message.contains("system-design"));
    }

    #[test]
    fn every_exit_reason_maps_to_expected_code() {
        assert_eq!(ExitReason::Ok.as_code(), 0);
        assert_eq!(ExitReason::RejectedByGate.as_code(), 2);
        assert_eq!(ExitReason::InvalidDecisionShape.as_code(), 3);
        assert_eq!(ExitReason::Conflict.as_code(), 4);
        assert_eq!(ExitReason::EnvConfig.as_code(), 5);
    }

    #[test]
    fn envelope_round_trips_through_json() {
        let env = CliEnvelope::ok("guide.describe", vec!["a".to_string(), "b".to_string()]);
        let json = serde_json::to_string_pretty(&env).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"ok\": true"));
        let back: CliEnvelope<Vec<String>> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn err_envelope_omits_data_and_ok_envelope_omits_error() {
        let ok: CliEnvelope<Payload> = CliEnvelope::ok("x", Payload { phase: "p".into() });
        let ok_json = serde_json::to_string_pretty(&ok).unwrap();
        assert!(!ok_json.contains("\"error\""));
        assert!(ok_json.contains("\"data\""));

        let err: CliEnvelope<Payload> = CliEnvelope::err("x", ExitReason::Conflict, "boom");
        let err_json = serde_json::to_string_pretty(&err).unwrap();
        assert!(!err_json.contains("\"data\""));
        assert!(err_json.contains("\"error\""));
    }

    #[test]
    fn reject_envelope_carries_both_data_and_error_with_nonzero_exit() {
        // N2: the `reject` constructor is the one case where structured data
        // appears alongside a rejection (check-write blocked lists colliding
        // paths). Lock the contract: ok:false, exit nonzero, BOTH data and
        // error present, and it round-trips through JSON.
        let rej: CliEnvelope<Payload> = CliEnvelope::reject(
            "x",
            ExitReason::RejectedByGate,
            "blocked",
            Payload { phase: "p".into() },
        );
        assert!(!rej.ok);
        assert_eq!(rej.exit_code(), ExitReason::RejectedByGate.as_code());
        assert!(rej.data.is_some(), "reject must carry structured data");
        assert!(rej.error.is_some(), "reject must carry an error");
        let json = serde_json::to_string(&rej).unwrap();
        assert!(json.contains("\"data\""));
        assert!(json.contains("\"error\""));
        let back: CliEnvelope<Payload> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rej);
    }

    #[test]
    fn unknown_exit_reason_defaults_loudly_to_env_config() {
        let env: CliEnvelope<Payload> = CliEnvelope {
            schema_version: StableId("0.1".into()),
            command: StableId("x".into()),
            ok: false,
            exit_reason: StableId("nonsense".into()),
            data: None,
            error: None,
        };
        // A bogus reason must not silently exit 0 — it surfaces as env/config (5).
        assert_eq!(env.exit_code(), 5);
    }
}
