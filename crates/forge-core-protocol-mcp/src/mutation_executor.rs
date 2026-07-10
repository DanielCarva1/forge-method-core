//! MCP argument projection into the host-neutral in-process execution seam.
//!
//! The public stdio server does not dispatch through this seam in P4b.2a. The
//! protocol crate validates its wire shape, then transfers the opaque authority
//! capability into `forge-core-authority` without subprocess serialization.

use std::fmt;
use std::path::PathBuf;

use forge_core_authority::VerifiedExecutionAuthorization;
use serde_json::{Map, Value};

pub use forge_core_authority::{
    ExecutionError as McpMutationExecutionError, ExecutionExecutor as McpMutationExecutor,
    ExecutionPayloadBinding as McpMutationPayloadBinding, ExecutionRequest as McpExecutionRequest,
    ExecutionResult as McpMutationExecutionResult, ExecutionStatus as McpMutationExecutionStatus,
    VerifiedExecutionCall as VerifiedMcpExecutionCall,
};

pub const MCP_EXECUTE_OPERATION_TOOL: &str = "execute-operation";

pub(crate) fn verified_call_from_arguments(
    authorization: VerifiedExecutionAuthorization,
    arguments: Option<&Map<String, Value>>,
) -> Result<VerifiedMcpExecutionCall, McpMutationRequestError> {
    let parsed = parse_execution_arguments(arguments)?;
    let request = McpExecutionRequest::new(
        parsed.operation_contract_ref,
        parsed.command_contract_refs,
        parsed.effect_contract_refs.into_iter().next(),
        parsed.payloads,
        parsed.risk_audit_rules_ref,
        parsed.require_citation,
    );
    Ok(VerifiedMcpExecutionCall::new(authorization, request))
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum McpMutationRequestError {
    MissingOperation,
    UnsupportedArgument(String),
    InvalidType {
        field: String,
        expected: &'static str,
    },
    BlankValue(String),
    InvalidPayloadBinding(String),
    MultipleEffectsUnsupported {
        count: usize,
    },
}

impl fmt::Display for McpMutationRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOperation => formatter.write_str("--operation is required"),
            Self::UnsupportedArgument(argument) => write!(
                formatter,
                "argument '{argument}' is not allowed by the typed MCP execution boundary"
            ),
            Self::InvalidType { field, expected } => {
                write!(formatter, "argument '{field}' must be {expected}")
            }
            Self::BlankValue(field) => write!(formatter, "argument '{field}' must not be blank"),
            Self::InvalidPayloadBinding(binding) => write!(
                formatter,
                "payload binding '{binding}' must be <target_ref>=<path> with non-blank parts"
            ),
            Self::MultipleEffectsUnsupported { count } => write!(
                formatter,
                "typed MCP execution admits at most one supplied effect in P4b.2 ({count} found)"
            ),
        }
    }
}

impl std::error::Error for McpMutationRequestError {}

#[derive(Debug, PartialEq, Eq)]
struct ParsedExecutionArguments {
    operation_contract_ref: PathBuf,
    command_contract_refs: Vec<PathBuf>,
    effect_contract_refs: Vec<PathBuf>,
    payloads: Vec<McpMutationPayloadBinding>,
    risk_audit_rules_ref: Option<PathBuf>,
    require_citation: bool,
}

fn parse_execution_arguments(
    arguments: Option<&Map<String, Value>>,
) -> Result<ParsedExecutionArguments, McpMutationRequestError> {
    let empty = Map::new();
    let arguments = arguments.unwrap_or(&empty);
    for key in arguments.keys() {
        if !matches!(
            key.as_str(),
            "--operation"
                | "--command"
                | "--effect"
                | "--payload"
                | "--require-risk-audit"
                | "--require-citation"
        ) {
            return Err(McpMutationRequestError::UnsupportedArgument(key.clone()));
        }
    }

    let operation_contract_ref = arguments
        .get("--operation")
        .ok_or(McpMutationRequestError::MissingOperation)
        .and_then(|value| parse_single_path("--operation", value))?;
    let command_contract_refs = parse_path_list(arguments.get("--command"), "--command")?;
    let effect_contract_refs = parse_path_list(arguments.get("--effect"), "--effect")?;
    if effect_contract_refs.len() > 1 {
        return Err(McpMutationRequestError::MultipleEffectsUnsupported {
            count: effect_contract_refs.len(),
        });
    }
    let payloads = parse_payloads(arguments.get("--payload"))?;
    let risk_audit_rules_ref = arguments
        .get("--require-risk-audit")
        .map(|value| parse_single_path("--require-risk-audit", value))
        .transpose()?;
    let require_citation = match arguments.get("--require-citation") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            return Err(McpMutationRequestError::InvalidType {
                field: "--require-citation".to_owned(),
                expected: "a boolean",
            });
        }
    };

    Ok(ParsedExecutionArguments {
        operation_contract_ref,
        command_contract_refs,
        effect_contract_refs,
        payloads,
        risk_audit_rules_ref,
        require_citation,
    })
}

fn parse_single_path(field: &str, value: &Value) -> Result<PathBuf, McpMutationRequestError> {
    let Value::String(value) = value else {
        return Err(McpMutationRequestError::InvalidType {
            field: field.to_owned(),
            expected: "a path string",
        });
    };
    if value.trim().is_empty() {
        return Err(McpMutationRequestError::BlankValue(field.to_owned()));
    }
    Ok(PathBuf::from(value))
}

fn parse_path_list(
    value: Option<&Value>,
    field: &str,
) -> Result<Vec<PathBuf>, McpMutationRequestError> {
    match value {
        None => Ok(Vec::new()),
        Some(value @ Value::String(_)) => Ok(vec![parse_single_path(field, value)?]),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| parse_single_path(field, value))
            .collect(),
        Some(_) => Err(McpMutationRequestError::InvalidType {
            field: field.to_owned(),
            expected: "a path string or array of path strings",
        }),
    }
}

fn parse_payloads(
    value: Option<&Value>,
) -> Result<Vec<McpMutationPayloadBinding>, McpMutationRequestError> {
    let raw = match value {
        None => return Ok(Vec::new()),
        Some(Value::String(value)) => vec![value.as_str()],
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| McpMutationRequestError::InvalidType {
                        field: "--payload".to_owned(),
                        expected: "a binding string or array of binding strings",
                    })
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(McpMutationRequestError::InvalidType {
                field: "--payload".to_owned(),
                expected: "a binding string or array of binding strings",
            });
        }
    };

    raw.into_iter()
        .map(|binding| {
            let Some((target_ref, path)) = binding.split_once('=') else {
                return Err(McpMutationRequestError::InvalidPayloadBinding(
                    binding.to_owned(),
                ));
            };
            if target_ref.trim().is_empty() || path.trim().is_empty() {
                return Err(McpMutationRequestError::InvalidPayloadBinding(
                    binding.to_owned(),
                ));
            }
            Ok(McpMutationPayloadBinding::new(
                target_ref.to_owned(),
                PathBuf::from(path),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(value: &Value) -> Map<String, Value> {
        value.as_object().expect("object").clone()
    }

    #[test]
    fn typed_arguments_accept_only_the_narrow_execution_shape() {
        let arguments = map(&serde_json::json!({
            "--operation": "contracts/op.yaml",
            "--command": ["contracts/cmd-a.yaml", "contracts/cmd-b.yaml"],
            "--effect": "contracts/effect.yaml",
            "--payload": ["target.a=payload/a.bin", "target.b=payload/b.bin"],
            "--require-risk-audit": "contracts/risk.yaml",
            "--require-citation": true
        }));

        let parsed = parse_execution_arguments(Some(&arguments)).expect("typed arguments");
        assert_eq!(
            parsed.operation_contract_ref,
            PathBuf::from("contracts/op.yaml")
        );
        assert_eq!(parsed.command_contract_refs.len(), 2);
        assert_eq!(parsed.effect_contract_refs.len(), 1);
        assert_eq!(parsed.payloads.len(), 2);
        assert_eq!(parsed.payloads[0].target_ref(), "target.a");
        assert_eq!(
            parsed.risk_audit_rules_ref,
            Some(PathBuf::from("contracts/risk.yaml"))
        );
        assert!(parsed.require_citation);
    }

    #[test]
    fn mutation_boundary_rejects_caller_controlled_root_durability_and_limits() {
        for forbidden in [
            "--root",
            "--no-sync",
            "--allow-payload-outside-root",
            "--max-payload-bytes",
            "--tx-id-prefix",
            "--recorded-at",
            "--json",
            "--no-json",
        ] {
            let mut arguments = Map::new();
            arguments.insert(
                "--operation".to_owned(),
                Value::String("op.yaml".to_owned()),
            );
            arguments.insert(forbidden.to_owned(), Value::Bool(true));
            let rejection = parse_execution_arguments(Some(&arguments))
                .expect_err("caller-controlled authority flag must fail");
            assert_eq!(
                rejection,
                McpMutationRequestError::UnsupportedArgument(forbidden.to_owned())
            );
        }
    }

    #[test]
    fn mutation_boundary_rejects_missing_operation_bad_types_and_multiple_effects() {
        assert!(matches!(
            parse_execution_arguments(None),
            Err(McpMutationRequestError::MissingOperation)
        ));

        let bad_operation = map(&serde_json::json!({"--operation": ["op.yaml"]}));
        assert!(matches!(
            parse_execution_arguments(Some(&bad_operation)),
            Err(McpMutationRequestError::InvalidType { .. })
        ));

        let multiple_effects = map(&serde_json::json!({
            "--operation": "op.yaml",
            "--effect": ["a.yaml", "b.yaml"]
        }));
        assert_eq!(
            parse_execution_arguments(Some(&multiple_effects)),
            Err(McpMutationRequestError::MultipleEffectsUnsupported { count: 2 })
        );
    }

    #[test]
    fn mutation_boundary_rejects_malformed_payload_bindings() {
        for binding in ["missing-separator", "=missing-target", "missing-path="] {
            let arguments = map(&serde_json::json!({
                "--operation": "op.yaml",
                "--payload": binding
            }));
            assert!(matches!(
                parse_execution_arguments(Some(&arguments)),
                Err(McpMutationRequestError::InvalidPayloadBinding(_))
            ));
        }
    }
}
