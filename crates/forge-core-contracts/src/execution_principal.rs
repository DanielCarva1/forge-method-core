//! Portable identity projection for one verified execution actor.

use crate::operation::CallerRole;
use crate::{PrincipalId, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Credential-independent identity propagated across governed execution
/// surfaces. This DTO is evidence, not authority: only the authority crate can
/// produce the opaque authorization capability that permits a mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPrincipal {
    pub principal_id: PrincipalId,
    pub agent_id: StableId,
    pub role: CallerRole,
}

impl ExecutionPrincipal {
    #[must_use]
    pub const fn new(principal_id: PrincipalId, agent_id: StableId, role: CallerRole) -> Self {
        Self {
            principal_id,
            agent_id,
            role,
        }
    }
}
