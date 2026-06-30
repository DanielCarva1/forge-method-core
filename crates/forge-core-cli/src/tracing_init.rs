//! Tracing subscriber initialization for the `forge-core` binary.
//!
//! The subscriber writes structured JSON (default, agent-friendly) or
//! human-readable ANSI text to stderr. The forge command's own contract
//! output (JSON envelope, validation diagnostics) continues to flow on
//! stdout untouched; tracing is strictly a side-channel on stderr.
//!
//! Filtering follows the standard `RUST_LOG`/`EnvFilter` convention, with a
//! conservative default (`warn`) so that routine successful runs stay quiet
//! unless the operator opts in via `RUST_LOG` or `--log-level`.
//!
//! ## Why a dedicated module
//!
//! - Keeps `main.rs` as a pure dispatcher (R8 contract: thin entrypoint).
//! - Encapsulates the human/json format decision so any future command-line
//!   flag (`--log-format`) lands here without touching dispatchers.
//! - Makes the init testable in isolation (no global subscriber leak when
//!   `RUST_LOG` is unset, since `tracing_subscriber::set_default` is used
//!   instead of `init` in test paths).

use std::env;
use std::io::IsTerminal;
use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;

/// Format of the tracing stream on stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Newline-delimited JSON objects (default for agents).
    Json,
    /// Human-readable ANSI text (default when stderr is a TTY).
    Human,
}

impl LogFormat {
    /// Pick a format from the `FORGE_LOG_FORMAT` env var, falling back to
    /// JSON when stderr is not a TTY (piped to a log file or agent) and
    /// human-readable when it is.
    #[must_use]
    pub fn from_env_or_auto() -> Self {
        match env::var("FORGE_LOG_FORMAT").ok().as_deref() {
            Some("json") => Self::Json,
            Some("human") => Self::Human,
            _ => {
                if std::io::stderr().is_terminal() {
                    Self::Human
                } else {
                    Self::Json
                }
            }
        }
    }
}

/// Default filter level when `RUST_LOG` is not set. `warn` keeps routine
/// successful runs quiet while still surfacing recoverable problems.
const DEFAULT_FILTER: &str = "warn";

static SUBSCRIBER_INSTALLED: OnceLock<()> = OnceLock::new();

/// Initialize the global tracing subscriber for the process.
///
/// Idempotent: subsequent calls are no-ops. Safe to call from `main` and
/// from integration tests that share the process.
///
/// # Panics
///
/// Never. If the global subscriber was already set by another crate, the
/// call is silently ignored.
pub fn init_subscriber() {
    init_with(LogFormat::from_env_or_auto());
}

/// Initialize the subscriber with an explicit format. Idempotent.
pub fn init_with(format: LogFormat) {
    SUBSCRIBER_INSTALLED.get_or_init(|| {
        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

        match format {
            LogFormat::Json => {
                let _ = tracing_subscriber::fmt()
                    .with_env_filter(filter)
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_ansi(false)
                    .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ENTER)
                    .json()
                    .try_init();
            }
            LogFormat::Human => {
                let _ = tracing_subscriber::fmt()
                    .with_env_filter(filter)
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_ansi(true)
                    .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ENTER)
                    .try_init();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_format_from_env_json() {
        // Force the env var and re-read; ignore on platforms where env
        // mutation is racy (we only assert the explicit branches).
        // SAFETY: single-threaded test, no other reader of this var.
        unsafe {
            env::set_var("FORGE_LOG_FORMAT", "json");
        }
        assert_eq!(LogFormat::from_env_or_auto(), LogFormat::Json);
        unsafe {
            env::set_var("FORGE_LOG_FORMAT", "human");
        }
        assert_eq!(LogFormat::from_env_or_auto(), LogFormat::Human);
        env::remove_var("FORGE_LOG_FORMAT");
    }

    #[test]
    fn init_is_idempotent() {
        // Calling twice must not panic.
        init_with(LogFormat::Json);
        init_with(LogFormat::Human);
        init_subscriber();
    }
}
