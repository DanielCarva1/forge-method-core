//! Host-neutral, in-process handoff for one verified execution request.
//!
//! Transport adapters may parse their wire shape into [`ExecutionRequest`],
//! but verified authority and execution are never serialized into argv, env,
//! files, or a child process.

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::VerifiedExecutionAuthorization;

/// Trusted executor injected by a host process. Implementations must call the
/// kernel in-process; subprocess implementations violate this authority seam.
pub trait ExecutionExecutor: fmt::Debug + Send + Sync {
    /// Execute one already verified, structurally constrained call in-process.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError`] when policy rejects the call or the trusted
    /// executor cannot complete it.
    fn execute(&self, call: VerifiedExecutionCall) -> Result<ExecutionResult, ExecutionError>;
}

/// One verified and structurally constrained execution request.
///
/// The call is linear: it has private fields and implements neither serde nor
/// `Clone`. Constructing it consumes the opaque authorization capability.
///
/// ```compile_fail
/// use forge_core_authority::VerifiedExecutionCall;
/// let _: VerifiedExecutionCall = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use forge_core_authority::VerifiedExecutionCall;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<VerifiedExecutionCall>();
/// ```
pub struct VerifiedExecutionCall {
    authorization: VerifiedExecutionAuthorization,
    request: ExecutionRequest,
}

impl fmt::Debug for VerifiedExecutionCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedExecutionCall")
            .field("authorization", &self.authorization)
            .field("request", &self.request)
            .finish()
    }
}

impl VerifiedExecutionCall {
    /// Bind structured adapter input to a consumed verified capability.
    #[must_use]
    pub fn new(authorization: VerifiedExecutionAuthorization, request: ExecutionRequest) -> Self {
        Self {
            authorization,
            request,
        }
    }

    #[must_use]
    pub const fn authorization(&self) -> &VerifiedExecutionAuthorization {
        &self.authorization
    }

    #[must_use]
    pub const fn request(&self) -> &ExecutionRequest {
        &self.request
    }

    /// Consume the non-cloneable call so the kernel can retain authority and
    /// structured inputs in a prepared transaction.
    #[must_use]
    pub fn into_parts(self) -> (VerifiedExecutionAuthorization, ExecutionRequest) {
        (self.authorization, self.request)
    }
}

/// Structured execution inputs with caller-controlled authority knobs absent.
/// Root, durability, payload-size ceiling, transaction identity, and commit
/// timestamp must be supplied by the trusted executor/kernel.
#[derive(Debug, PartialEq, Eq)]
pub struct ExecutionRequest {
    operation_contract_ref: PathBuf,
    command_contract_refs: Vec<PathBuf>,
    effect_contract_refs: Vec<PathBuf>,
    payloads: Vec<ExecutionPayloadBinding>,
    risk_audit_rules_ref: Option<PathBuf>,
    require_citation: bool,
}

impl ExecutionRequest {
    #[must_use]
    pub fn new(
        operation_contract_ref: PathBuf,
        command_contract_refs: Vec<PathBuf>,
        effect_contract_ref: Option<PathBuf>,
        payloads: Vec<ExecutionPayloadBinding>,
        risk_audit_rules_ref: Option<PathBuf>,
        require_citation: bool,
    ) -> Self {
        Self {
            operation_contract_ref,
            command_contract_refs,
            effect_contract_refs: effect_contract_ref.into_iter().collect(),
            payloads,
            risk_audit_rules_ref,
            require_citation,
        }
    }

    /// Construct an operation-wide request whose complete ordered effect set
    /// is already covered by verified intent authority.
    #[must_use]
    pub fn new_operation_wide(
        operation_contract_ref: PathBuf,
        command_contract_refs: Vec<PathBuf>,
        effect_contract_refs: Vec<PathBuf>,
        payloads: Vec<ExecutionPayloadBinding>,
        risk_audit_rules_ref: Option<PathBuf>,
        require_citation: bool,
    ) -> Self {
        Self {
            operation_contract_ref,
            command_contract_refs,
            effect_contract_refs,
            payloads,
            risk_audit_rules_ref,
            require_citation,
        }
    }

    #[must_use]
    pub fn operation_contract_ref(&self) -> &Path {
        &self.operation_contract_ref
    }

    #[must_use]
    pub fn command_contract_refs(&self) -> &[PathBuf] {
        &self.command_contract_refs
    }

    #[must_use]
    pub fn effect_contract_ref(&self) -> Option<&Path> {
        (self.effect_contract_refs.len() == 1).then(|| self.effect_contract_refs[0].as_path())
    }

    /// Complete ordered effect set bound by this structured request.
    #[must_use]
    pub fn effect_contract_refs(&self) -> &[PathBuf] {
        &self.effect_contract_refs
    }

    #[must_use]
    pub fn payloads(&self) -> &[ExecutionPayloadBinding] {
        &self.payloads
    }

    #[must_use]
    pub fn risk_audit_rules_ref(&self) -> Option<&Path> {
        self.risk_audit_rules_ref.as_deref()
    }

    #[must_use]
    pub const fn require_citation(&self) -> bool {
        self.require_citation
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExecutionPayloadBinding {
    target_ref: String,
    path: PathBuf,
    expected_content_hash: Option<String>,
}

impl ExecutionPayloadBinding {
    #[must_use]
    pub fn new(target_ref: String, path: PathBuf) -> Self {
        Self {
            target_ref,
            path,
            expected_content_hash: None,
        }
    }

    /// Bind payload bytes to the digest signed in the adapter intent.
    #[must_use]
    pub fn new_verified(target_ref: String, path: PathBuf, expected_content_hash: String) -> Self {
        Self {
            target_ref,
            path,
            expected_content_hash: Some(expected_content_hash),
        }
    }

    #[must_use]
    pub fn target_ref(&self) -> &str {
        &self.target_ref
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn expected_content_hash(&self) -> Option<&str> {
        self.expected_content_hash.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExecutionStatus {
    Applied,
    Blocked,
    RecoveryRequired,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ExecutionResult {
    status: ExecutionStatus,
    payload: Value,
}

impl ExecutionResult {
    #[must_use]
    pub fn new(status: ExecutionStatus, payload: Value) -> Self {
        Self { status, payload }
    }

    #[must_use]
    pub const fn status(&self) -> ExecutionStatus {
        self.status
    }

    #[must_use]
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExecutionError {
    Rejected(String),
    Internal(String),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(reason) => write!(formatter, "executor rejected call: {reason}"),
            Self::Internal(message) => write!(formatter, "executor failed: {message}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::ExecutionRequest;

    #[test]
    fn legacy_constructor_preserves_exact_single_effect_access() {
        let request = ExecutionRequest::new(
            PathBuf::from("operation.yaml"),
            Vec::new(),
            Some(PathBuf::from("effect.yaml")),
            Vec::new(),
            None,
            false,
        );

        assert_eq!(
            request.effect_contract_ref(),
            Some(std::path::Path::new("effect.yaml"))
        );
        assert_eq!(request.effect_contract_refs().len(), 1);
    }

    #[test]
    fn operation_wide_request_cannot_masquerade_as_single_effect() {
        let request = ExecutionRequest::new_operation_wide(
            PathBuf::from("operation.yaml"),
            Vec::new(),
            vec![PathBuf::from("first.yaml"), PathBuf::from("second.yaml")],
            Vec::new(),
            None,
            false,
        );

        assert!(request.effect_contract_ref().is_none());
        assert_eq!(
            request.effect_contract_refs(),
            [PathBuf::from("first.yaml"), PathBuf::from("second.yaml")]
        );
    }
}
