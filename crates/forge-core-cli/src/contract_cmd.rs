//! Generic contract CLI helpers.
//!
//! `forge-core contract validate` loads one of the evolve-phase typed-contract
//! documents and reports whether it deserializes as the requested kind. The
//! driver wires this module into the top-level CLI dispatch.

use forge_core_contracts::{
    AgentRunContractDocument, AutonomyPolicyContractDocument, CheckpointContractDocument,
    CliEnvelope, EvalRunContractDocument, ExitReason, MemoryContractDocument,
    TelemetryContractDocument, VerificationGoalContractDocument,
};

use crate::cli_error::ExitError;

const SUPPORTED_KINDS: &[&str] = &[
    "autonomy_policy",
    "verification_goal",
    "agent_run",
    "memory",
    "checkpoint",
    "eval_run",
    "telemetry",
];

/// Successful `contract validate` payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ContractValidationResult {
    pub kind: String,
    pub valid: bool,
    pub schema_version: String,
}

/// Hand-rolled error enum for [`validate_kind`] / [`parse_document`] (no `thiserror`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractValidateError {
    /// The `kind` string did not match any of the supported contract kinds.
    UnsupportedKind {
        kind: String,
        supported: Vec<String>,
    },
    /// The YAML payload could not be deserialized as the typed document for `kind`.
    YamlInvalid { kind: String, source: String },
}

impl std::fmt::Display for ContractValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedKind { kind, supported } => write!(
                f,
                "unsupported contract kind '{kind}'. Supported kinds: {}",
                supported.join(", ")
            ),
            Self::YamlInvalid { kind, source } => write!(
                f,
                "{kind} file is not valid YAML for that contract type: {source}"
            ),
        }
    }
}

/// Parse and run `forge-core contract <subcommand>`.
pub fn run_contract_command(args: &[String]) -> Result<(), ExitError> {
    let sub = args.get(1).map(String::as_str).unwrap_or("--help");
    match sub {
        "validate" => run_validate(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("forge-core contract <subcommand> [options]");
            println!("  validate --kind <kind> --file <path> [--no-json]");
            println!("  supported kinds: {}", SUPPORTED_KINDS.join(", "));
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core contract: unknown subcommand '{other}'. Try: validate"
        ))),
    }
}

/// Handler for `forge-core contract validate`.
pub fn run_validate(args: &[String]) -> Result<(), ExitError> {
    let mut kind: Option<String> = None;
    let mut file: Option<std::path::PathBuf> = None;
    let mut want_json = true;

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--kind" => {
                idx += 1;
                kind = Some(require_value(args, idx, "validate", "kind")?);
            }
            "--file" => {
                idx += 1;
                file = Some(std::path::PathBuf::from(require_value(
                    args, idx, "validate", "file",
                )?));
            }
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("forge-core contract validate --kind <kind> --file <path> [--no-json]");
                println!("supported kinds: {}", SUPPORTED_KINDS.join(", "));
                return Ok(());
            }
            other => {
                return Err(ExitError::invalid_value(format!(
                    "forge-core contract validate: unknown argument '{other}'"
                )));
            }
        }
        idx += 1;
    }

    let kind =
        kind.ok_or_else(|| emit_err("contract validate", "--kind is required", want_json))?;
    let file =
        file.ok_or_else(|| emit_err("contract validate", "--file is required", want_json))?;

    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) => {
            return Err(emit_err(
                "contract validate",
                &format!("cannot read contract file '{}': {e}", file.display()),
                want_json,
            ));
        }
    };

    match validate_kind(&kind, &text) {
        Ok(payload) => emit(CliEnvelope::ok("contract validate", payload), want_json),
        Err(message) => Err(emit_err(
            "contract validate",
            &message.to_string(),
            want_json,
        )),
    }
}

/// Validate a typed-contract YAML string by kind without exiting the process.
///
/// This deliberately matches on the explicit kind string because the document
/// wrappers are concrete Rust types rather than a shared trait object.
pub fn validate_kind(
    kind: &str,
    text: &str,
) -> Result<ContractValidationResult, ContractValidateError> {
    match kind {
        "autonomy_policy" => parse_document::<AutonomyPolicyContractDocument>(kind, text),
        "verification_goal" => parse_document::<VerificationGoalContractDocument>(kind, text),
        "agent_run" => parse_document::<AgentRunContractDocument>(kind, text),
        "memory" => parse_document::<MemoryContractDocument>(kind, text),
        "checkpoint" => parse_document::<CheckpointContractDocument>(kind, text),
        "eval_run" => parse_document::<EvalRunContractDocument>(kind, text),
        "telemetry" => parse_document::<TelemetryContractDocument>(kind, text),
        other => Err(ContractValidateError::UnsupportedKind {
            kind: other.to_string(),
            supported: SUPPORTED_KINDS.iter().map(|s| (*s).to_string()).collect(),
        }),
    }
}

fn parse_document<T>(
    kind: &str,
    text: &str,
) -> Result<ContractValidationResult, ContractValidateError>
where
    T: serde::de::DeserializeOwned + HasSchemaVersion,
{
    let doc: T = yaml_serde::from_str(text).map_err(|e| ContractValidateError::YamlInvalid {
        kind: kind.to_string(),
        source: e.to_string(),
    })?;
    Ok(ContractValidationResult {
        kind: kind.to_owned(),
        valid: true,
        schema_version: doc.schema_version().to_owned(),
    })
}

trait HasSchemaVersion {
    fn schema_version(&self) -> &str;
}

impl HasSchemaVersion for AutonomyPolicyContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for VerificationGoalContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for AgentRunContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for MemoryContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for CheckpointContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for EvalRunContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for TelemetryContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

fn require_value(
    args: &[String],
    idx: usize,
    subcommand: &str,
    flag: &str,
) -> Result<String, ExitError> {
    match args.get(idx) {
        Some(v) => Ok(v.clone()),
        None => Err(ExitError::invalid_value(format!(
            "forge-core contract {subcommand}: --{flag} requires a value"
        ))),
    }
}

fn emit_err(command: &str, message: &str, want_json: bool) -> ExitError {
    let env: CliEnvelope<()> = CliEnvelope::err(command, ExitReason::InvalidDecisionShape, message);
    // Print the envelope in the same shape as `emit` so behavior is
    // byte-identical to the legacy exit-on-error path.
    if want_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&env).expect("serialize envelope")
        );
    } else {
        eprintln!(
            "{command} failed: {}",
            env.error
                .as_ref()
                .map(|e| e.message.as_str())
                .unwrap_or("unknown")
        );
    }
    ExitError::with_code(env.exit_code(), String::new())
}

fn emit<T: serde::Serialize>(env: CliEnvelope<T>, want_json: bool) -> Result<(), ExitError> {
    let code = env.exit_code();
    if want_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&env).expect("serialize envelope")
        );
    } else if env.ok {
        println!("contract validate: ok");
    } else {
        eprintln!(
            "contract validate failed: {}",
            env.error
                .as_ref()
                .map(|e| e.message.as_str())
                .unwrap_or("unknown")
        );
    }
    if code == 0 {
        Ok(())
    } else {
        Err(ExitError::with_code(code, String::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_autonomy_policy_yaml() -> &'static str {
        r#"
schema_version: "0.1"
autonomy_policy_contract:
  id: autonomy.fast_lane.default
  applies_to:
    kind: lane
    ids:
      - fast_lane
  default_mode: sandbox_auto
  tool_classes: []
  escalation:
    on_repeated_failure: 2
    on_high_risk_path: true
    on_semantic_uncertainty: true
    max_retries_before_human: 3
    cooldown_seconds: 300
  evidence_basis: null
"#
    }

    #[test]
    fn valid_autonomy_policy_yaml_is_accepted() {
        let path = std::env::temp_dir().join(format!(
            "forge-contract-cmd-autonomy-policy-{}.yaml",
            std::process::id()
        ));
        std::fs::write(&path, valid_autonomy_policy_yaml()).expect("write temp autonomy policy");
        let text = std::fs::read_to_string(&path).expect("read temp autonomy policy");
        let _ = std::fs::remove_file(&path);

        let result = validate_kind("autonomy_policy", &text).expect("valid autonomy policy");

        assert_eq!(result.kind, "autonomy_policy");
        assert!(result.valid);
        assert_eq!(result.schema_version, "0.1");
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let err = validate_kind("not_a_contract", valid_autonomy_policy_yaml())
            .expect_err("unknown kind must fail");

        assert!(
            err.to_string().contains("unsupported contract kind"),
            "got: {err}"
        );
    }

    #[test]
    fn malformed_yaml_is_rejected() {
        let err = validate_kind("autonomy_policy", "schema_version: [")
            .expect_err("malformed yaml must fail");

        assert!(
            err.to_string()
                .contains("autonomy_policy file is not valid YAML"),
            "got: {err}"
        );
    }
}
