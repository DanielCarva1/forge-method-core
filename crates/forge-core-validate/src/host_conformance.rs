use forge_core_contracts::{
    CapabilityOutcome, CliJsonSurfaceDocument, DefaultAuthorityGrant, ForgeCoreResponsibility,
    HostClaimKind, HostClientResponsibility, HostConformanceDocument, HostContractAuthority,
    HostJourney, HostKind, McpDefaultMode, McpSurfaceDocument, ProjectedField,
    ProjectionProhibition, RecognitionCannotEstablish, SetupGapKind, SurfaceAuthority,
    SurfaceTransport, CLI_JSON_SURFACE_SCHEMA_VERSION, HOST_CONFORMANCE_SCHEMA_VERSION,
    MCP_SURFACE_SCHEMA_VERSION,
};
use std::collections::BTreeSet;

use crate::{Diagnostic, DiagnosticCode, ValidationReport};

const HOST_CONFORMANCE_CONTRACT_ID: &str = "host.common-borrowed-shell.v0";
const CLI_JSON_SURFACE_CONTRACT_ID: &str = "surface.cli-json.common-borrowed-shell.v0";
const MCP_SURFACE_CONTRACT_ID: &str = "surface.mcp.common-borrowed-shell.v0";
const HOST_ORIGIN_REF: &str = "contracts/spec/host-origin-broker-conformance-v0.yaml";
const ADAPTER_PROJECTION_REF: &str =
    "contracts/policies/host-adapter-manifest-projection-boundary.yaml";

/// Validate the complete core-owned common borrowed-shell authority packet.
///
/// This is intentionally one named aggregate: the host candidate contract and
/// both client surfaces must agree on the same non-authoritative projection and
/// fail-closed authority boundary.
#[must_use]
pub fn validate_common_borrowed_shell_contracts(
    host: &HostConformanceDocument,
    cli_json: &CliJsonSurfaceDocument,
    mcp: &McpSurfaceDocument,
) -> ValidationReport {
    let mut report = ValidationReport::new();
    validate_host_contract(host, &mut report);
    validate_cli_json_surface(cli_json, &mut report);
    validate_mcp_surface(mcp, &mut report);
    report
}

fn validate_host_contract(document: &HostConformanceDocument, report: &mut ValidationReport) {
    let contract = &document.host_conformance;
    if document.schema_version != HOST_CONFORMANCE_SCHEMA_VERSION {
        host_error(
            report,
            "schema_version",
            "unsupported host conformance schema version",
        );
    }
    if contract.contract_id != HOST_CONFORMANCE_CONTRACT_ID {
        host_error(
            report,
            "host_conformance.contract_id",
            "host conformance contract identity must match the core-owned common borrowed-shell contract",
        );
    }
    if contract.authority != HostContractAuthority::CandidateOnly
        || contract.selected_host.is_some()
        || contract.released
        || contract.field_verified
        || contract.exact_host_execution != CapabilityOutcome::Unknown
    {
        host_error(
            report,
            "host_conformance.authority",
            "common host contract must remain candidate_only with no selected, released, or field-verified host and exact-host execution unknown",
        );
    }
    if contract.read_only_inputs.host_origin_broker_conformance_ref != HOST_ORIGIN_REF
        || contract.read_only_inputs.adapter_projection_policy_ref != ADAPTER_PROJECTION_REF
    {
        host_error(
            report,
            "host_conformance.read_only_inputs",
            "host-origin conformance and adapter projection may be consumed only through their fixed read-only inputs",
        );
    }

    let found_hosts = contract
        .candidates
        .iter()
        .map(|candidate| candidate.host)
        .collect::<BTreeSet<_>>();
    let expected_hosts = HostKind::ALL.into_iter().collect::<BTreeSet<_>>();
    if found_hosts != expected_hosts || contract.candidates.len() != HostKind::ALL.len() {
        host_error(
            report,
            "host_conformance.candidates",
            "candidate inventory must contain Codex, Cursor, OpenCode, Claude, pi.dev, and Forge App exactly once",
        );
    }
    for candidate in &contract.candidates {
        if candidate.disposition != CapabilityOutcome::Candidate
            || candidate.selected
            || candidate.supported
            || candidate.released
            || candidate.field_verified
        {
            host_error(
                report,
                format!("host_conformance.candidates.{:?}", candidate.host),
                "every named host must remain an unselected, unsupported, unreleased, unverified candidate",
            );
        }
        let claims = candidate
            .claims
            .iter()
            .map(|claim| (claim.claim, claim.outcome))
            .collect::<std::collections::BTreeMap<_, _>>();
        let expected = [
            (
                HostClaimKind::RuntimeRecognition,
                CapabilityOutcome::Candidate,
            ),
            (HostClaimKind::ReadOnlyMcp, CapabilityOutcome::Candidate),
            (HostClaimKind::Installability, CapabilityOutcome::Unknown),
            (
                HostClaimKind::HumanOriginAssurance,
                CapabilityOutcome::Unknown,
            ),
            (
                HostClaimKind::GovernedMutation,
                CapabilityOutcome::Unsupported,
            ),
            (HostClaimKind::Support, CapabilityOutcome::Unsupported),
        ]
        .into_iter()
        .collect();
        if claims != expected || candidate.claims.len() != HostClaimKind::ALL.len() {
            host_error(
                report,
                format!("host_conformance.candidates.{:?}.claims", candidate.host),
                "recognition, read-only MCP, installability, origin assurance, governed mutation, and support must remain distinct closed claims",
            );
        }
    }

    let separated = contract
        .claim_separation
        .iter()
        .map(|entry| entry.claim)
        .collect::<BTreeSet<_>>();
    if separated != HostClaimKind::ALL.into_iter().collect()
        || contract.claim_separation.len() != HostClaimKind::ALL.len()
        || contract.claim_separation.iter().any(|entry| {
            entry.independent_from.contains(&entry.claim)
                || entry.independent_from.len() != HostClaimKind::ALL.len() - 1
                || entry
                    .independent_from
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != HostClaimKind::ALL
                        .into_iter()
                        .filter(|claim| *claim != entry.claim)
                        .collect()
        })
    {
        host_error(
            report,
            "host_conformance.claim_separation",
            "every host claim must be explicitly independent from all five other claims",
        );
    }

    let found_journeys = contract
        .journeys
        .iter()
        .map(|expectation| expectation.journey)
        .collect::<BTreeSet<_>>();
    if found_journeys != HostJourney::ALL.into_iter().collect()
        || contract.journeys.len() != HostJourney::ALL.len()
    {
        host_error(
            report,
            "host_conformance.journeys",
            "common contract must cover install, invoke, update, diagnose, recover, unauthorized-mutation, and fresh-chat exactly once",
        );
    }
    for expectation in &contract.journeys {
        let (expected_outcome, expected_host_client, expected_core) =
            expected_journey(expectation.journey);
        let expected_host_client_set = expected_host_client
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let expected_core_set = expected_core.iter().copied().collect::<BTreeSet<_>>();
        let actual_host_client = expectation
            .host_client_responsibilities
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let actual_core = expectation
            .forge_core_responsibilities
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if expectation.outcome != expected_outcome
            || actual_host_client != expected_host_client_set
            || expectation.host_client_responsibilities.len() != expected_host_client.len()
            || actual_core != expected_core_set
            || expectation.forge_core_responsibilities.len() != expected_core.len()
        {
            host_error(
                report,
                format!("host_conformance.journeys.{:?}", expectation.journey),
                "journey outcome and responsibility sets must exactly match the core-owned common borrowed-shell boundary without duplicates",
            );
        }
    }
}

fn expected_journey(
    journey: HostJourney,
) -> (
    CapabilityOutcome,
    &'static [HostClientResponsibility],
    &'static [ForgeCoreResponsibility],
) {
    match journey {
        HostJourney::Install => (
            CapabilityOutcome::Candidate,
            &[
                HostClientResponsibility::DiscoverCandidate,
                HostClientResponsibility::InstallOwnedFiles,
                HostClientResponsibility::RenderTypedDiagnostics,
            ],
            &[
                ForgeCoreResponsibility::ValidateContracts,
                ForgeCoreResponsibility::ReturnTypedSetupGap,
                ForgeCoreResponsibility::PreserveProjectAuthority,
            ],
        ),
        HostJourney::Invoke => (
            CapabilityOutcome::Candidate,
            &[
                HostClientResponsibility::TranslateManifest,
                HostClientResponsibility::InvokeArgvWithoutShellString,
            ],
            &[
                ForgeCoreResponsibility::ClassifyMutation,
                ForgeCoreResponsibility::ClassifyAuthority,
                ForgeCoreResponsibility::ValidateContracts,
            ],
        ),
        HostJourney::Update => (
            CapabilityOutcome::Candidate,
            &[
                HostClientResponsibility::PreservePriorVersionDuringUpdate,
                HostClientResponsibility::InstallOwnedFiles,
            ],
            &[
                ForgeCoreResponsibility::ValidateContracts,
                ForgeCoreResponsibility::PreserveProjectAuthority,
                ForgeCoreResponsibility::ReturnTypedSetupGap,
            ],
        ),
        HostJourney::Diagnose => (
            CapabilityOutcome::Candidate,
            &[HostClientResponsibility::RenderTypedDiagnostics],
            &[
                ForgeCoreResponsibility::ReturnTypedSetupGap,
                ForgeCoreResponsibility::RejectUnknownOrUnauthorizedInput,
            ],
        ),
        HostJourney::Recover => (
            CapabilityOutcome::Candidate,
            &[
                HostClientResponsibility::RemoveOwnedIntegrationOnly,
                HostClientResponsibility::PreservePriorVersionDuringUpdate,
            ],
            &[
                ForgeCoreResponsibility::PreserveProjectAuthority,
                ForgeCoreResponsibility::ValidateContracts,
            ],
        ),
        HostJourney::UnauthorizedMutation => (
            CapabilityOutcome::Unsupported,
            &[HostClientResponsibility::RejectUnauthorizedMutationRequest],
            &[
                ForgeCoreResponsibility::AdmitGovernedMutation,
                ForgeCoreResponsibility::RejectUnknownOrUnauthorizedInput,
            ],
        ),
        HostJourney::FreshChat => (
            CapabilityOutcome::Candidate,
            &[HostClientResponsibility::StartWithoutHiddenChatState],
            &[
                ForgeCoreResponsibility::RequireExplicitContextEachChat,
                ForgeCoreResponsibility::ValidateContracts,
            ],
        ),
    }
}

fn validate_cli_json_surface(document: &CliJsonSurfaceDocument, report: &mut ValidationReport) {
    let surface = &document.cli_json_surface;
    if document.schema_version != CLI_JSON_SURFACE_SCHEMA_VERSION {
        surface_error(
            report,
            "cli_json.schema_version",
            "unsupported CLI-JSON schema version",
        );
    }
    if surface.contract_id != CLI_JSON_SURFACE_CONTRACT_ID {
        surface_error(
            report,
            "cli_json_surface.contract_id",
            "CLI-JSON contract identity must match the core-owned common borrowed-shell surface",
        );
    }
    validate_common_surface(
        surface.transport,
        SurfaceTransport::CliJson,
        surface.authority,
        surface.derived_from_manifest,
        surface.projection_authoritative,
        &surface.preserved_fields,
        &surface.setup_gap_types,
        &surface.projections_must_not,
        &surface.recognition,
        "cli_json_surface",
        report,
    );
    validate_ocsp_delegated_responder_surface(
        &surface.ocsp_delegated_responder,
        "cli_json_surface.ocsp_delegated_responder",
        report,
    );
    if !surface.invocation.argv_only
        || surface.invocation.shell_strings_allowed
        || !surface.invocation.mutation_requires_core_admission
        || surface.invocation.signing_available
    {
        surface_error(
            report,
            "cli_json_surface.invocation",
            "CLI-JSON must be argv-only, expose no signing, and route mutation through Forge Core admission",
        );
    }
}

fn validate_mcp_surface(document: &McpSurfaceDocument, report: &mut ValidationReport) {
    let surface = &document.mcp_surface;
    if document.schema_version != MCP_SURFACE_SCHEMA_VERSION {
        surface_error(
            report,
            "mcp.schema_version",
            "unsupported MCP schema version",
        );
    }
    if surface.contract_id != MCP_SURFACE_CONTRACT_ID {
        surface_error(
            report,
            "mcp_surface.contract_id",
            "MCP contract identity must match the core-owned common borrowed-shell surface",
        );
    }
    validate_common_surface(
        surface.transport,
        SurfaceTransport::Mcp,
        surface.authority,
        surface.derived_from_manifest,
        surface.projection_authoritative,
        &surface.preserved_fields,
        &surface.setup_gap_types,
        &surface.projections_must_not,
        &surface.recognition,
        "mcp_surface",
        report,
    );
    validate_ocsp_delegated_responder_surface(
        &surface.ocsp_delegated_responder,
        "mcp_surface.ocsp_delegated_responder",
        report,
    );
    if surface.default_mode != McpDefaultMode::ReadOnly
        || surface.mutation_authority != DefaultAuthorityGrant::Forbidden
        || surface.signing_authority != DefaultAuthorityGrant::Forbidden
        || surface.signer_tool_exposed
        || !surface.mutation_policy.default_tools_read_only
        || !surface.mutation_policy.explicit_core_admission_required
        || !surface.mutation_policy.client_annotations_are_advisory
    {
        surface_error(
            report,
            "mcp_surface",
            "MCP must remain read-only by default and cannot grant signing or mutation authority",
        );
    }
}

fn validate_ocsp_delegated_responder_surface(
    boundary: &forge_core_contracts::OcspDelegatedResponderSurfaceBoundary,
    path: &str,
    report: &mut ValidationReport,
) {
    if !boundary.supplied_responder_certificate_input
        || !boundary.ordered_issuer_chain_input
        || !boundary.selected_authority_identity_output
        || !boundary.verified_authority_evidence_output
        || boundary.network_authority
        || boundary.install_authority
        || boundary.update_authority
        || boundary.crl_authority
        || boundary.certificate_transparency_authority
        || boundary.rekor_authority
        || boundary.tuf_authority
        || boundary.signing_authority
        || boundary.mutation_authority
    {
        surface_error(
            report,
            path,
            "delegated OCSP projection must accept only supplied offline responder/path material, project typed selected-authority identity and evidence, and grant no network, install, update, CRL, CT, Rekor, TUF, signing, or mutation authority",
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_common_surface(
    actual_transport: SurfaceTransport,
    expected_transport: SurfaceTransport,
    authority: SurfaceAuthority,
    derived_from_manifest: bool,
    projection_authoritative: bool,
    preserved_fields: &[ProjectedField],
    setup_gap_types: &[SetupGapKind],
    projections_must_not: &[ProjectionProhibition],
    recognition: &forge_core_contracts::RecognitionBoundary,
    path: &str,
    report: &mut ValidationReport,
) {
    if actual_transport != expected_transport
        || authority != SurfaceAuthority::CoreOnly
        || !derived_from_manifest
        || projection_authoritative
    {
        surface_error(
            report,
            path,
            "surface translation must be manifest-derived, non-authoritative, and retain core-only authority",
        );
    }
    if preserved_fields.iter().copied().collect::<BTreeSet<_>>()
        != ProjectedField::ALL.into_iter().collect()
        || preserved_fields.len() != ProjectedField::ALL.len()
    {
        surface_error(
            report,
            format!("{path}.preserved_fields"),
            "surface must preserve command kind, mutation class, authority class, safe auto-invocation triggers, output treatment, required contracts, and typed setup gaps",
        );
    }
    if setup_gap_types.iter().copied().collect::<BTreeSet<_>>()
        != SetupGapKind::ALL.into_iter().collect()
        || setup_gap_types.len() != SetupGapKind::ALL.len()
    {
        surface_error(
            report,
            format!("{path}.setup_gap_types"),
            "surface must preserve the complete closed setup-gap vocabulary",
        );
    }
    if projections_must_not
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        != ProjectionProhibition::ALL.into_iter().collect()
        || projections_must_not.len() != ProjectionProhibition::ALL.len()
    {
        surface_error(
            report,
            format!("{path}.projections_must_not"),
            "surface must preserve the complete closed prohibition set for retrieval, security subsystems, mutation, host claims, and projection authority",
        );
    }
    if recognition.runtime_kind_sufficient
        || recognition.manifest_recognition_sufficient
        || recognition
            .cannot_establish
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            != RecognitionCannotEstablish::ALL.into_iter().collect()
        || recognition.cannot_establish.len() != RecognitionCannotEstablish::ALL.len()
    {
        surface_error(
            report,
            format!("{path}.recognition"),
            "runtime-kind or manifest recognition cannot establish installability, origin assurance, governed mutation, or support",
        );
    }
}

fn host_error(report: &mut ValidationReport, path: impl Into<String>, message: impl Into<String>) {
    report.push(Diagnostic::error(
        DiagnosticCode::HostConformanceInvalid,
        path,
        message,
    ));
}

fn surface_error(
    report: &mut ValidationReport,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    report.push(Diagnostic::error(
        DiagnosticCode::HostSurfaceContractInvalid,
        path,
        message,
    ));
}
