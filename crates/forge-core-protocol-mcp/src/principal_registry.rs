//! Operator-owned principal registry for mutating MCP calls.
//!
//! A valid signature proves only possession of a key. This registry binds an
//! operator-issued credential to the principal identity, role, audience,
//! allowed tools, and grants that Forge may trust. The registry is loaded once
//! at server startup and is never sourced from caller-provided project data.

use std::collections::BTreeSet;
use std::fmt;

use forge_core_command_surface::command_by_name;
use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::{PrincipalId, StableId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::attestation::{
    decode_verifying_key, AttestationError, AttestationInput, AttestationVerifier, CanonicalIntent,
};

pub const PRINCIPAL_REGISTRY_SCHEMA_VERSION: &str = "0.1";
pub const DEFAULT_MAX_ATTESTATION_AGE_SECONDS: u64 = 300;
pub const DEFAULT_MAX_FUTURE_SKEW_SECONDS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalRegistryDocument {
    pub schema_version: String,
    pub principal_registry: PrincipalRegistryContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalRegistryContract {
    pub audience: String,
    pub principals: Vec<PrincipalRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalRegistryEntry {
    pub credential_id: String,
    pub principal_id: PrincipalId,
    pub agent_id: StableId,
    pub role: CallerRole,
    pub public_key_hex: String,
    pub allowed_tools: Vec<StableId>,
    pub authority_grants: Vec<StableId>,
    pub status: PrincipalCredentialStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalCredentialStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedPrincipalRegistry {
    audience: String,
    principals: Vec<PrincipalRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizedPrincipal {
    pub credential_id: String,
    pub principal_id: PrincipalId,
    pub agent_id: StableId,
    pub role: CallerRole,
    pub audience: String,
    pub authority_grants: Vec<StableId>,
    pub public_key_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrincipalRegistryError {
    Parse(String),
    UnsupportedSchemaVersion { found: String },
    Invalid(Vec<PrincipalRegistryIssue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalRegistryIssue {
    pub code: PrincipalRegistryIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalRegistryIssueCode {
    EmptyAudience,
    EmptyRegistry,
    EmptyCredentialId,
    EmptyPrincipalId,
    EmptyAgentId,
    UnknownRole,
    InvalidPublicKey,
    EmptyAllowedTools,
    UnknownTool,
    EmptyAuthorityGrants,
    DuplicateCredentialId,
    DuplicatePublicKey,
    DuplicateAllowedTool,
    DuplicateAuthorityGrant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrincipalAuthorizationError {
    MissingCredentialId,
    MissingAudience,
    MissingExecutionIntentDigest,
    InvalidExecutionIntentDigest,
    InvalidNonce,
    UnknownCredential(String),
    CredentialRevoked(String),
    AudienceMismatch {
        expected: String,
        found: String,
    },
    PublicKeyMismatch(String),
    ToolNotAllowed {
        credential_id: String,
        tool: String,
    },
    MissingAuthorityGrant {
        credential_id: String,
        grant: &'static str,
    },
    Attestation(AttestationError),
}

impl AuthorizedPrincipalRegistry {
    /// Parse and validate an operator principal registry.
    ///
    /// # Errors
    ///
    /// Returns [`PrincipalRegistryError`] for YAML/schema errors or any
    /// accumulated semantic issue.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, PrincipalRegistryError> {
        let document: PrincipalRegistryDocument = yaml_serde::from_str(yaml)
            .map_err(|error| PrincipalRegistryError::Parse(error.to_string()))?;
        Self::from_document(document)
    }

    /// Validate a typed registry document.
    ///
    /// # Errors
    ///
    /// Returns [`PrincipalRegistryError::UnsupportedSchemaVersion`] or
    /// [`PrincipalRegistryError::Invalid`] when the document is not safe to
    /// activate.
    pub fn from_document(
        document: PrincipalRegistryDocument,
    ) -> Result<Self, PrincipalRegistryError> {
        if document.schema_version != PRINCIPAL_REGISTRY_SCHEMA_VERSION {
            return Err(PrincipalRegistryError::UnsupportedSchemaVersion {
                found: document.schema_version,
            });
        }
        let mut issues = Vec::new();
        validate_contract(&document.principal_registry, &mut issues);
        if !issues.is_empty() {
            return Err(PrincipalRegistryError::Invalid(issues));
        }
        Ok(Self {
            audience: document.principal_registry.audience,
            principals: document.principal_registry.principals,
        })
    }

    #[must_use]
    pub fn audience(&self) -> &str {
        &self.audience
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.principals.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.principals.is_empty()
    }

    /// Verify and authorize one mutating attestation using only the
    /// operator-selected registry key.
    ///
    /// # Errors
    ///
    /// Returns [`PrincipalAuthorizationError`] when identity, audience, tool,
    /// freshness, key binding, or signature verification fails.
    pub fn authorize(
        &self,
        verifier: &AttestationVerifier,
        intent: &CanonicalIntent,
        attestation: &AttestationInput,
        now_unix: i64,
        max_age_seconds: u64,
        max_future_skew_seconds: u64,
    ) -> Result<AuthorizedPrincipal, PrincipalAuthorizationError> {
        let credential_id = nonempty_option(attestation.credential_id.as_ref())
            .ok_or(PrincipalAuthorizationError::MissingCredentialId)?;
        let audience = nonempty_option(attestation.audience.as_ref())
            .ok_or(PrincipalAuthorizationError::MissingAudience)?;
        if intent.tool == "execute-operation" {
            let intent_digest = nonempty_option(attestation.execution_intent_digest.as_ref())
                .ok_or(PrincipalAuthorizationError::MissingExecutionIntentDigest)?;
            if !is_sha256_token(intent_digest) {
                return Err(PrincipalAuthorizationError::InvalidExecutionIntentDigest);
            }
        }
        validate_nonce(&attestation.nonce)?;

        let entry = self
            .principals
            .iter()
            .find(|entry| entry.credential_id == credential_id)
            .ok_or_else(|| {
                PrincipalAuthorizationError::UnknownCredential(credential_id.to_owned())
            })?;
        if entry.status != PrincipalCredentialStatus::Active {
            return Err(PrincipalAuthorizationError::CredentialRevoked(
                credential_id.to_owned(),
            ));
        }
        if audience != self.audience {
            return Err(PrincipalAuthorizationError::AudienceMismatch {
                expected: self.audience.clone(),
                found: audience.to_owned(),
            });
        }
        if !entry
            .public_key_hex
            .eq_ignore_ascii_case(&attestation.public_key_hex)
        {
            return Err(PrincipalAuthorizationError::PublicKeyMismatch(
                credential_id.to_owned(),
            ));
        }
        if !entry
            .allowed_tools
            .iter()
            .any(|allowed| allowed.0 == intent.tool)
        {
            return Err(PrincipalAuthorizationError::ToolNotAllowed {
                credential_id: credential_id.to_owned(),
                tool: intent.tool.clone(),
            });
        }
        if intent.tool == "execute-operation"
            && !entry
                .authority_grants
                .iter()
                .any(|grant| grant.0 == "operation.execute")
        {
            return Err(PrincipalAuthorizationError::MissingAuthorityGrant {
                credential_id: credential_id.to_owned(),
                grant: "operation.execute",
            });
        }

        verifier
            .verify_freshness(
                attestation,
                now_unix,
                max_age_seconds,
                max_future_skew_seconds,
            )
            .map_err(PrincipalAuthorizationError::Attestation)?;
        verifier
            .verify_with_public_key(intent, attestation, &entry.public_key_hex)
            .map_err(PrincipalAuthorizationError::Attestation)?;

        Ok(AuthorizedPrincipal {
            credential_id: entry.credential_id.clone(),
            principal_id: entry.principal_id.clone(),
            agent_id: entry.agent_id.clone(),
            role: entry.role,
            audience: self.audience.clone(),
            authority_grants: entry.authority_grants.clone(),
            public_key_fingerprint: public_key_fingerprint(&entry.public_key_hex),
        })
    }
}

fn validate_contract(
    contract: &PrincipalRegistryContract,
    issues: &mut Vec<PrincipalRegistryIssue>,
) {
    if contract.audience.trim().is_empty() {
        issue(
            issues,
            PrincipalRegistryIssueCode::EmptyAudience,
            "principal_registry.audience",
            "audience must not be empty",
        );
    }
    if contract.principals.is_empty() {
        issue(
            issues,
            PrincipalRegistryIssueCode::EmptyRegistry,
            "principal_registry.principals",
            "at least one principal is required",
        );
    }

    let mut credentials = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for (index, entry) in contract.principals.iter().enumerate() {
        let base = format!("principal_registry.principals[{index}]");
        if entry.credential_id.trim().is_empty() {
            issue(
                issues,
                PrincipalRegistryIssueCode::EmptyCredentialId,
                &format!("{base}.credential_id"),
                "credential_id must not be empty",
            );
        } else if !credentials.insert(entry.credential_id.clone()) {
            issue(
                issues,
                PrincipalRegistryIssueCode::DuplicateCredentialId,
                &format!("{base}.credential_id"),
                "credential_id must be unique",
            );
        }
        if entry.principal_id.0.trim().is_empty() {
            issue(
                issues,
                PrincipalRegistryIssueCode::EmptyPrincipalId,
                &format!("{base}.principal_id"),
                "principal_id must not be empty",
            );
        }
        if entry.agent_id.0.trim().is_empty() {
            issue(
                issues,
                PrincipalRegistryIssueCode::EmptyAgentId,
                &format!("{base}.agent_id"),
                "agent_id must not be empty",
            );
        }
        if entry.role == CallerRole::Unknown {
            issue(
                issues,
                PrincipalRegistryIssueCode::UnknownRole,
                &format!("{base}.role"),
                "unknown role cannot authorize mutation",
            );
        }
        let key = entry.public_key_hex.to_ascii_lowercase();
        if decode_verifying_key(&key).is_err() {
            issue(
                issues,
                PrincipalRegistryIssueCode::InvalidPublicKey,
                &format!("{base}.public_key_hex"),
                "public_key_hex must encode a valid 32-byte ed25519 key",
            );
        } else if !keys.insert(key) {
            issue(
                issues,
                PrincipalRegistryIssueCode::DuplicatePublicKey,
                &format!("{base}.public_key_hex"),
                "public keys must not authorize multiple credentials",
            );
        }
        validate_stable_id_list(
            &entry.allowed_tools,
            issues,
            &format!("{base}.allowed_tools"),
            PrincipalRegistryIssueCode::EmptyAllowedTools,
            PrincipalRegistryIssueCode::DuplicateAllowedTool,
        );
        for (tool_index, tool) in entry.allowed_tools.iter().enumerate() {
            if command_by_name(&tool.0).is_none() {
                issue(
                    issues,
                    PrincipalRegistryIssueCode::UnknownTool,
                    &format!("{base}.allowed_tools[{tool_index}]"),
                    "tool is not present in the shared Command Surface",
                );
            }
        }
        validate_stable_id_list(
            &entry.authority_grants,
            issues,
            &format!("{base}.authority_grants"),
            PrincipalRegistryIssueCode::EmptyAuthorityGrants,
            PrincipalRegistryIssueCode::DuplicateAuthorityGrant,
        );
    }
}

fn validate_stable_id_list(
    values: &[StableId],
    issues: &mut Vec<PrincipalRegistryIssue>,
    path: &str,
    empty_code: PrincipalRegistryIssueCode,
    duplicate_code: PrincipalRegistryIssueCode,
) {
    if values.is_empty() {
        issue(issues, empty_code, path, "list must not be empty");
    }
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.0.trim().is_empty() || !seen.insert(value.0.clone()) {
            issue(
                issues,
                duplicate_code,
                &format!("{path}[{index}]"),
                "values must be non-empty and unique",
            );
        }
    }
}

fn issue(
    issues: &mut Vec<PrincipalRegistryIssue>,
    code: PrincipalRegistryIssueCode,
    path: &str,
    message: &str,
) {
    issues.push(PrincipalRegistryIssue {
        code,
        path: path.to_owned(),
        message: message.to_owned(),
    });
}

fn nonempty_option(value: Option<&String>) -> Option<&str> {
    value
        .map(String::as_str)
        .filter(|item| !item.trim().is_empty())
}

fn validate_nonce(nonce: &str) -> Result<(), PrincipalAuthorizationError> {
    if !(16..=256).contains(&nonce.len()) || nonce.chars().any(char::is_control) {
        return Err(PrincipalAuthorizationError::InvalidNonce);
    }
    Ok(())
}

fn is_sha256_token(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn public_key_fingerprint(public_key_hex: &str) -> String {
    let digest = Sha256::digest(public_key_hex.to_ascii_lowercase().as_bytes());
    format!("sha256:{digest:x}")
}

impl fmt::Display for PrincipalRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(message) => write!(formatter, "principal registry parse failed: {message}"),
            Self::UnsupportedSchemaVersion { found } => write!(
                formatter,
                "unsupported principal registry schema '{found}'; expected {PRINCIPAL_REGISTRY_SCHEMA_VERSION}"
            ),
            Self::Invalid(issues) => {
                write!(
                    formatter,
                    "principal registry has {} validation issue(s)",
                    issues.len()
                )?;
                for issue in issues {
                    write!(
                        formatter,
                        "; {} [{:?}]: {}",
                        issue.path, issue.code, issue.message
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for PrincipalRegistryError {}

impl fmt::Display for PrincipalAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCredentialId => {
                formatter.write_str("credential_id is required for mutation")
            }
            Self::MissingAudience => formatter.write_str("audience is required for mutation"),
            Self::MissingExecutionIntentDigest => {
                formatter.write_str("execution_intent_digest is required for execute-operation")
            }
            Self::InvalidExecutionIntentDigest => formatter
                .write_str("execution_intent_digest must be a lowercase sha256:<64-hex> token"),
            Self::InvalidNonce => {
                formatter.write_str("nonce must be 16-256 non-control characters")
            }
            Self::UnknownCredential(id) => write!(formatter, "unknown credential_id '{id}'"),
            Self::CredentialRevoked(id) => write!(formatter, "credential '{id}' is revoked"),
            Self::AudienceMismatch { expected, found } => write!(
                formatter,
                "attestation audience mismatch: expected '{expected}', found '{found}'"
            ),
            Self::PublicKeyMismatch(id) => {
                write!(formatter, "caller key does not match credential '{id}'")
            }
            Self::ToolNotAllowed {
                credential_id,
                tool,
            } => write!(
                formatter,
                "credential '{credential_id}' is not authorized for tool '{tool}'"
            ),
            Self::MissingAuthorityGrant {
                credential_id,
                grant,
            } => write!(
                formatter,
                "credential '{credential_id}' is missing required authority grant '{grant}'"
            ),
            Self::Attestation(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for PrincipalAuthorizationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::hex_encode;
    use ed25519_dalek::{Signer, SigningKey};

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn repository_example_is_valid_and_safe_by_default() {
        let registry = AuthorizedPrincipalRegistry::from_yaml_str(include_str!(
            "../../../contracts/examples/mcp-principal-registry.yaml"
        ))
        .expect("repository principal registry example");

        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.principals[0].status,
            PrincipalCredentialStatus::Revoked,
            "published test key must never authorize mutation"
        );
    }

    fn registry_for(
        key: &SigningKey,
        status: PrincipalCredentialStatus,
    ) -> AuthorizedPrincipalRegistry {
        AuthorizedPrincipalRegistry::from_document(PrincipalRegistryDocument {
            schema_version: PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
            principal_registry: PrincipalRegistryContract {
                audience: "forge-core:mcp:stdio:test-project".to_owned(),
                principals: vec![PrincipalRegistryEntry {
                    credential_id: "key.codex-main.2026-01".to_owned(),
                    principal_id: PrincipalId("principal.codex-main".to_owned()),
                    agent_id: StableId("codex-main".to_owned()),
                    role: CallerRole::Driver,
                    public_key_hex: hex_encode(&key.verifying_key().to_bytes()),
                    allowed_tools: vec![StableId("execute-operation".to_owned())],
                    authority_grants: vec![StableId("operation.execute".to_owned())],
                    status,
                }],
            },
        })
        .expect("valid registry")
    }

    fn signed_attestation(
        key: &SigningKey,
        audience: &str,
        credential_id: &str,
        ts: i64,
    ) -> (CanonicalIntent, AttestationInput) {
        let intent = CanonicalIntent {
            tool: "execute-operation".to_owned(),
            arguments: serde_json::json!({"--operation": "contracts/op.yaml"}),
            credential_id: Some(credential_id.to_owned()),
            audience: Some(audience.to_owned()),
            execution_intent_digest: Some(format!("sha256:{}", "a".repeat(64))),
            nonce: "nonce-registry-test-0001".to_owned(),
            ts,
        };
        let signature = key.sign(&intent.canonical_bytes().expect("canonical intent"));
        let attestation = AttestationInput {
            credential_id: intent.credential_id.clone(),
            audience: intent.audience.clone(),
            execution_intent_digest: intent.execution_intent_digest.clone(),
            nonce: intent.nonce.clone(),
            ts: intent.ts,
            signature: hex_encode(&signature.to_bytes()),
            public_key_hex: hex_encode(&key.verifying_key().to_bytes()),
        };
        (intent, attestation)
    }

    #[test]
    fn registered_key_maps_to_typed_principal() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let registry = registry_for(&key, PrincipalCredentialStatus::Active);
        let (intent, attestation) =
            signed_attestation(&key, registry.audience(), "key.codex-main.2026-01", NOW - 5);

        let principal = registry
            .authorize(
                &AttestationVerifier::new(crate::attestation::AttestationPolicy::Default),
                &intent,
                &attestation,
                NOW,
                DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
                DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            )
            .expect("authorized principal");

        assert_eq!(principal.principal_id.0, "principal.codex-main");
        assert_eq!(principal.agent_id.0, "codex-main");
        assert_eq!(principal.role, CallerRole::Driver);
        assert_eq!(principal.authority_grants[0].0, "operation.execute");
        assert!(principal.public_key_fingerprint.starts_with("sha256:"));
    }

    #[test]
    fn execute_operation_requires_digest_and_authority_grant() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let registry = registry_for(&key, PrincipalCredentialStatus::Active);
        let (mut intent, mut attestation) =
            signed_attestation(&key, registry.audience(), "key.codex-main.2026-01", NOW);
        intent.execution_intent_digest = None;
        attestation.execution_intent_digest = None;
        let missing_digest = registry
            .authorize(
                &AttestationVerifier::new(crate::attestation::AttestationPolicy::Default),
                &intent,
                &attestation,
                NOW,
                DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
                DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            )
            .expect_err("execute-operation digest is mandatory");
        assert_eq!(
            missing_digest,
            PrincipalAuthorizationError::MissingExecutionIntentDigest
        );

        intent.execution_intent_digest = Some("SHA256:not-canonical".to_owned());
        attestation.execution_intent_digest = intent.execution_intent_digest.clone();
        let invalid_digest = registry
            .authorize(
                &AttestationVerifier::new(crate::attestation::AttestationPolicy::Default),
                &intent,
                &attestation,
                NOW,
                DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
                DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            )
            .expect_err("execute-operation digest must be canonical");
        assert_eq!(
            invalid_digest,
            PrincipalAuthorizationError::InvalidExecutionIntentDigest
        );

        let (intent, attestation) =
            signed_attestation(&key, registry.audience(), "key.codex-main.2026-01", NOW);
        let mut no_execute_grant = registry;
        no_execute_grant.principals[0].authority_grants = vec![StableId("claim.write".to_owned())];
        let missing_grant = no_execute_grant
            .authorize(
                &AttestationVerifier::new(crate::attestation::AttestationPolicy::Default),
                &intent,
                &attestation,
                NOW,
                DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
                DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            )
            .expect_err("operation.execute grant is mandatory");
        assert!(matches!(
            missing_grant,
            PrincipalAuthorizationError::MissingAuthorityGrant {
                grant: "operation.execute",
                ..
            }
        ));
    }

    #[test]
    fn valid_signature_from_unregistered_key_is_rejected() {
        let registered = SigningKey::from_bytes(&[7; 32]);
        let attacker = SigningKey::from_bytes(&[9; 32]);
        let registry = registry_for(&registered, PrincipalCredentialStatus::Active);
        let (intent, attestation) = signed_attestation(
            &attacker,
            registry.audience(),
            "key.codex-main.2026-01",
            NOW,
        );

        assert!(matches!(
            registry.authorize(
                &AttestationVerifier::new(crate::attestation::AttestationPolicy::Default),
                &intent,
                &attestation,
                NOW,
                300,
                30,
            ),
            Err(PrincipalAuthorizationError::PublicKeyMismatch(_))
        ));
    }

    #[test]
    fn wrong_audience_and_expired_timestamp_are_rejected() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let registry = registry_for(&key, PrincipalCredentialStatus::Active);
        let (wrong_intent, wrong_attestation) =
            signed_attestation(&key, "another-resource", "key.codex-main.2026-01", NOW);
        assert!(matches!(
            registry.authorize(
                &AttestationVerifier::new(crate::attestation::AttestationPolicy::Default),
                &wrong_intent,
                &wrong_attestation,
                NOW,
                300,
                30,
            ),
            Err(PrincipalAuthorizationError::AudienceMismatch { .. })
        ));

        let (expired_intent, expired_attestation) = signed_attestation(
            &key,
            registry.audience(),
            "key.codex-main.2026-01",
            NOW - 301,
        );
        assert!(matches!(
            registry.authorize(
                &AttestationVerifier::new(crate::attestation::AttestationPolicy::Default),
                &expired_intent,
                &expired_attestation,
                NOW,
                300,
                30,
            ),
            Err(PrincipalAuthorizationError::Attestation(
                AttestationError::Expired
            ))
        ));
    }

    #[test]
    fn revoked_credential_is_rejected() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let registry = registry_for(&key, PrincipalCredentialStatus::Revoked);
        let (intent, attestation) =
            signed_attestation(&key, registry.audience(), "key.codex-main.2026-01", NOW);

        assert!(matches!(
            registry.authorize(
                &AttestationVerifier::new(crate::attestation::AttestationPolicy::Default),
                &intent,
                &attestation,
                NOW,
                300,
                30,
            ),
            Err(PrincipalAuthorizationError::CredentialRevoked(_))
        ));
    }

    #[test]
    fn registry_accumulates_duplicate_and_unknown_tool_issues() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let public_key_hex = hex_encode(&key.verifying_key().to_bytes());
        let entry = PrincipalRegistryEntry {
            credential_id: "duplicate".to_owned(),
            principal_id: PrincipalId("principal".to_owned()),
            agent_id: StableId("agent".to_owned()),
            role: CallerRole::Driver,
            public_key_hex,
            allowed_tools: vec![StableId("not-a-command".to_owned())],
            authority_grants: vec![StableId("operation.execute".to_owned())],
            status: PrincipalCredentialStatus::Active,
        };
        let rejection = AuthorizedPrincipalRegistry::from_document(PrincipalRegistryDocument {
            schema_version: PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
            principal_registry: PrincipalRegistryContract {
                audience: "audience".to_owned(),
                principals: vec![entry.clone(), entry],
            },
        })
        .expect_err("invalid registry");

        let PrincipalRegistryError::Invalid(issues) = rejection else {
            panic!("expected validation issues")
        };
        assert!(issues
            .iter()
            .any(|issue| issue.code == PrincipalRegistryIssueCode::DuplicateCredentialId));
        assert!(issues
            .iter()
            .any(|issue| issue.code == PrincipalRegistryIssueCode::DuplicatePublicKey));
        assert!(issues
            .iter()
            .any(|issue| issue.code == PrincipalRegistryIssueCode::UnknownTool));
    }

    #[test]
    fn optional_fields_keep_legacy_canonicalization_stable() {
        let intent = CanonicalIntent {
            tool: "preview".to_owned(),
            arguments: serde_json::json!({}),
            credential_id: None,
            audience: None,
            execution_intent_digest: None,
            nonce: "legacy".to_owned(),
            ts: 1,
        };
        let value = serde_json::to_value(intent).expect("serialize");
        assert!(value.get("credential_id").is_none());
        assert!(value.get("audience").is_none());
        assert!(value.get("execution_intent_digest").is_none());
    }
}
