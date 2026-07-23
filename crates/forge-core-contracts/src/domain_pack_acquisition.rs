//! Closed P7c reviewed-package acquisition planning contracts.
//!
//! Selection is bound to an integrity-checked discovery projection but remains
//! candidate-only. It cannot substitute for operator trust, signed registry
//! verification, lifecycle authorization, or commit authority.

use crate::{
    DomainPackCandidateAuthority, DomainPackCompositionRequestDocument, DomainPackCoordinate,
    DomainPackCoreBinding, DomainPackDiscoveryMatch, DomainPackDiscoveryProjectionDocument,
    DomainPackDiscoveryRequestDocument, DomainPackProjectRequirements,
    DomainPackResolutionCandidate, DomainPackResolutionProjectionDocument,
    DomainPackResolutionRequestDocument, DomainPackSupplyChainRegistryDocument,
    DurableAssuranceEpochBinding, StableId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION: &str = "0.1";
pub const DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAcquisitionIntentDocument {
    pub schema_version: String,
    pub domain_pack_acquisition_intent: DomainPackAcquisitionIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAcquisitionIntent {
    pub acquisition_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub assurance_binding: DurableAssuranceEpochBinding,
    pub discovery_projection_digest: String,
    pub demand_digest: String,
    pub candidate_id: StableId,
    pub requirement_ref: StableId,
    pub operation: DomainPackAcquisitionOperation,
    pub expected_project_snapshot_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAcquisitionOperation {
    Install,
    Upgrade,
    Rollback,
    Remove,
    RebaseCore,
}

/// High-level operator intent for changing an already initialized project.
///
/// This document is candidate-only input to deterministic derivation. It does
/// not grant package trust, lifecycle preflight authority, installation, or
/// activation authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackInitializedProjectIntentDocument {
    pub schema_version: String,
    pub domain_pack_initialized_project_intent: DomainPackInitializedProjectIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackInitializedProjectIntent {
    pub intent_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub project_id: StableId,
    pub principal_id: StableId,
    pub expected_state: DomainPackInitializedProjectStateBinding,
    pub operation: DomainPackInitializedProjectOperation,
}

/// Exact initialized state against which a high-level intent must be derived.
/// The lifecycle TCB still independently compares these values with durable
/// state before it may preflight or apply a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackInitializedProjectStateBinding {
    pub generation: u64,
    pub active_lock_digest: String,
    pub lifecycle_head_digest: String,
    pub project_snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackInitializedProjectOperation {
    Install {
        selection: DomainPackInitializedProjectCandidateSelection,
    },
    Upgrade {
        pack: DomainPackCoordinate,
        expected_from: String,
        target_requirement: String,
        required_content_digest: Option<String>,
        selection: DomainPackInitializedProjectCandidateSelection,
    },
    Rollback {
        target_receipt_digest: String,
        target_lock_digest: String,
    },
    Remove {
        pack: DomainPackCoordinate,
    },
    RebaseCore {
        target_release_id: StableId,
        expected_from_core_digest: String,
        target_core_digest: String,
    },
}

/// Exact discovery and candidate-selection evidence for operations that add or
/// replace a package. Explicit operator approval is deliberately represented as
/// a requirement, never as authority already carried by this document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackInitializedProjectCandidateSelection {
    pub acquisition_id: StableId,
    pub assurance_binding: DurableAssuranceEpochBinding,
    pub discovery_projection_digest: String,
    pub demand_digest: String,
    pub candidate_id: StableId,
    pub requirement_ref: StableId,
    pub approval: DomainPackCandidateApprovalRequirement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCandidateApprovalRequirement {
    ExplicitOperatorApprovalRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAcquisitionPlanDocument {
    pub schema_version: String,
    pub domain_pack_acquisition_plan: DomainPackAcquisitionPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAcquisitionPlan {
    pub acquisition_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub assurance_binding: DurableAssuranceEpochBinding,
    pub discovery_request_digest: String,
    pub discovery_projection_digest: String,
    pub demand_digest: String,
    pub requirements: DomainPackProjectRequirements,
    pub selected: DomainPackDiscoveryMatch,
    pub operation: DomainPackAcquisitionOperation,
    pub expected_project_snapshot_digest: String,
    pub status: DomainPackAcquisitionPlanStatus,
    pub required_ceremonies: Vec<DomainPackAcquisitionCeremony>,
    pub plan_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAcquisitionPlanStatus {
    TrustCeremonyRequired,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackAcquisitionCeremony {
    OperatorCandidateApproval,
    OperatorTrustPolicy,
    SupplyChainRegistryVerification,
    IndependentReviewedRegistryVerification,
    RuntimeCapabilityApproval,
    LifecyclePreflight,
}

/// Input pair for pure selection planning. Keeping the projection separate
/// forces callers to present the exact current discovery state rather than
/// copying one match into a new authority-bearing document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAcquisitionPlanningInput {
    pub intent: DomainPackAcquisitionIntentDocument,
    /// Exact demand and reviewed candidate material that produced `discovery`.
    /// Planning recomputes discovery rather than trusting a self-digested
    /// projection supplied in isolation.
    pub request: DomainPackDiscoveryRequestDocument,
    pub discovery: DomainPackDiscoveryProjectionDocument,
}

/// Release/catalog material that a host combines with current discovery state.
/// It contains no trust or lifecycle authority; signed registries are verified
/// again at apply time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAcquisitionCatalogDocument {
    pub schema_version: String,
    pub forge_core_version: String,
    pub core: DomainPackCoreBinding,
    pub registry: DomainPackSupplyChainRegistryDocument,
    pub candidates: Vec<DomainPackResolutionCandidate>,
}

/// Exact package-set and core inputs used to derive the existing P6 resolver
/// and composer requests. These remain candidate-only; the signed registries,
/// operator policy, runtime capabilities, and lifecycle TCB still decide
/// whether the prepared material may advance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAcquisitionDerivationInput {
    /// Original request, discovery projection, and explicit selection intent.
    /// Derivation replays this input and requires the exact same plan.
    pub planning_input: DomainPackAcquisitionPlanningInput,
    pub plan: DomainPackAcquisitionPlanDocument,
    pub forge_core_version: String,
    pub core: DomainPackCoreBinding,
    pub registry: DomainPackSupplyChainRegistryDocument,
    pub candidates: Vec<DomainPackResolutionCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAcquisitionDerivedInputsDocument {
    pub schema_version: String,
    pub domain_pack_acquisition_derived_inputs: DomainPackAcquisitionDerivedInputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackAcquisitionDerivedInputs {
    pub acquisition_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub acquisition_plan_digest: String,
    pub derivation_input_digest: String,
    pub resolution_request: DomainPackResolutionRequestDocument,
    pub resolution_projection: DomainPackResolutionProjectionDocument,
    pub composition_request: DomainPackCompositionRequestDocument,
    pub derivation_digest: String,
}
