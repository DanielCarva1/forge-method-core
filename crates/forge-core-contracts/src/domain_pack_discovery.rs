//! Closed P7c domain-demand and candidate discovery wire types.
//!
//! Discovery is deliberately read-only and candidate-only. A host proposes a
//! typed demand bound to accepted intent; Forge validates and matches exact
//! reviewed package metadata without granting trust, installation, or runtime
//! authority.

use crate::{
    DomainPackCandidateAuthority, DomainPackContentDocument, DomainPackProjectRequirements,
    DomainPackReviewedRegistryEntry, DomainPackVersionReference, DurableAssuranceEpochBinding,
    StableId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDiscoveryRequestDocument {
    pub schema_version: String,
    pub domain_pack_discovery_request: DomainPackDiscoveryRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDiscoveryRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub assurance_binding: DurableAssuranceEpochBinding,
    pub requirements: DomainPackProjectRequirements,
    pub provenance: DomainPackDemandProvenance,
    pub uncertainties: Vec<String>,
    pub candidates: Vec<DomainPackDiscoveryCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDemandProvenance {
    pub source: DomainPackDemandSource,
    pub source_ref: String,
    pub source_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackDemandSource {
    HostProposal,
    ImportedProjectRequirement,
    ExistingLifecycleRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDiscoveryCandidate {
    pub reviewed_entry: DomainPackReviewedRegistryEntry,
    pub content: DomainPackContentDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDiscoveryProjectionDocument {
    pub schema_version: String,
    pub domain_pack_discovery_projection: DomainPackDiscoveryProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDiscoveryProjection {
    pub request_id: StableId,
    pub demand_digest: String,
    pub authority: DomainPackCandidateAuthority,
    pub assurance_binding: DurableAssuranceEpochBinding,
    pub uncertainties: Vec<String>,
    pub status: DomainPackDiscoveryStatus,
    pub matches: Vec<DomainPackDiscoveryMatch>,
    pub gaps: Vec<DomainPackDiscoveryGap>,
    pub projection_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackDiscoveryStatus {
    Matched,
    GapsPresent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDiscoveryMatch {
    pub candidate_id: StableId,
    pub requirement_ref: StableId,
    pub domain_id: StableId,
    pub pack: DomainPackVersionReference,
    pub package_digest: String,
    pub supply_chain_record_digest: String,
    pub reviewed_entry_digest: String,
    pub content_digest: String,
    pub matched_capability_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDiscoveryGap {
    pub requirement_ref: StableId,
    pub domain_id: StableId,
    pub code: DomainPackDiscoveryGapCode,
    pub message: String,
    pub next_action: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackDiscoveryGapCode {
    NoEligibleReviewedPack,
    VersionIncompatible,
    MissingRequiredCapability,
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEUTRAL_CORPUS: &str =
        include_str!("../../../contracts/domain-pack-discovery/neutral-reviewed-match.yaml");

    #[test]
    fn discovery_request_is_a_closed_typed_document() {
        let parsed: DomainPackDiscoveryRequestDocument =
            yaml_serde::from_str(NEUTRAL_CORPUS).expect("neutral discovery corpus");
        assert_eq!(parsed.schema_version, DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION);

        let with_unknown_field = format!("{NEUTRAL_CORPUS}\n  forbidden_runtime_authority: true\n");
        let error = yaml_serde::from_str::<DomainPackDiscoveryRequestDocument>(&with_unknown_field)
            .expect_err("unknown discovery field must fail closed");
        assert!(error.to_string().contains("forbidden_runtime_authority"));
    }
}
