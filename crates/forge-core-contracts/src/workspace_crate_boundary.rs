//! Exhaustive, host-neutral boundary contract for the Cargo workspace.
//!
//! This module intentionally models only reviewable crate metadata. In particular,
//! candidate documents are descriptive input: they do not select a host, carry key
//! material, or confer any authority to mutate Forge state.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

pub const WORKSPACE_CRATE_COUNT: usize = 23;

/// A candidate document is inert metadata. It cannot select a host.
pub struct CandidateDocumentBoundary;

impl CandidateDocumentBoundary {
    pub const SELECTED_HOST: Option<&'static str> = None;
    pub const PERMITS_SIGNING: bool = false;
    pub const PERMITS_TRUST: bool = false;
    pub const PERMITS_ADMISSION: bool = false;
    pub const PERMITS_INSTALLATION: bool = false;
    pub const PERMITS_ACTIVATION: bool = false;
    pub const PERMITS_LIFECYCLE_CONTROL: bool = false;
    pub const PERMITS_PROTECTED_ANCHOR_CONTROL: bool = false;
    pub const PERMITS_PRIVATE_KEY_MATERIAL: bool = false;
    pub const PERMITS_STATE_MUTATION: bool = false;
    pub const PERMITS_RELEASE_CONTROL: bool = false;
    pub const PERMITS_PHASE_TRANSITION: bool = false;
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CrateId {
    ContractValidator,
    Authority,
    Cli,
    CommandSurface,
    Contracts,
    Crypto,
    Decisions,
    DomainPackLearningStore,
    DomainPackTcb,
    Eval,
    EvalHarness,
    Eventlog,
    Governance,
    Graph,
    Kernel,
    Memory,
    ProtocolMcp,
    Research,
    Schema,
    Store,
    Trace,
    Validate,
    WorkflowGovernanceTcb,
}

impl CrateId {
    pub const ALL: [Self; WORKSPACE_CRATE_COUNT] = [
        Self::ContractValidator,
        Self::Authority,
        Self::Cli,
        Self::CommandSurface,
        Self::Contracts,
        Self::Crypto,
        Self::Decisions,
        Self::DomainPackLearningStore,
        Self::DomainPackTcb,
        Self::Eval,
        Self::EvalHarness,
        Self::Eventlog,
        Self::Governance,
        Self::Graph,
        Self::Kernel,
        Self::Memory,
        Self::ProtocolMcp,
        Self::Research,
        Self::Schema,
        Self::Store,
        Self::Trace,
        Self::Validate,
        Self::WorkflowGovernanceTcb,
    ];

    #[must_use]
    pub const fn package_name(self) -> &'static str {
        match self {
            Self::ContractValidator => "forge-contract-validator",
            Self::Authority => "forge-core-authority",
            Self::Cli => "forge-core-cli",
            Self::CommandSurface => "forge-core-command-surface",
            Self::Contracts => "forge-core-contracts",
            Self::Crypto => "forge-core-crypto",
            Self::Decisions => "forge-core-decisions",
            Self::DomainPackLearningStore => "forge-core-domain-pack-learning-store",
            Self::DomainPackTcb => "forge-core-domain-pack-tcb",
            Self::Eval => "forge-core-eval",
            Self::EvalHarness => "forge-core-eval-harness",
            Self::Eventlog => "forge-core-eventlog",
            Self::Governance => "forge-core-governance",
            Self::Graph => "forge-core-graph",
            Self::Kernel => "forge-core-kernel",
            Self::Memory => "forge-core-memory",
            Self::ProtocolMcp => "forge-core-protocol-mcp",
            Self::Research => "forge-core-research",
            Self::Schema => "forge-core-schema",
            Self::Store => "forge-core-store",
            Self::Trace => "forge-core-trace",
            Self::Validate => "forge-core-validate",
            Self::WorkflowGovernanceTcb => "forge-core-workflow-governance-tcb",
        }
    }

    #[must_use]
    pub fn parse(package_name: &str) -> Option<Self> {
        WORKSPACE_CRATE_BOUNDARIES
            .iter()
            .find(|boundary| boundary.id.package_name() == package_name)
            .map(|boundary| boundary.id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchitecturalLayer {
    Contract,
    Foundation,
    Validation,
    Security,
    Storage,
    Domain,
    Runtime,
    Protocol,
    Adapter,
    Compatibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureBoundary {
    NoOptionalFeatures,
    TestOnly(&'static [&'static str]),
    DenyInProduction(&'static [&'static str]),
}

impl FeatureBoundary {
    fn permits(self, feature: &str) -> bool {
        match self {
            Self::NoOptionalFeatures => false,
            Self::TestOnly(features) | Self::DenyInProduction(features) => {
                features.contains(&feature)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityClass {
    None,
    Verification,
    ExecutionAdmission,
    PolicyDecision,
    TrustedComputingBase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationClass {
    None,
    ReadOnlyProjection,
    DurableState,
    PreparedEffectOnly,
    HostAdapter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicApiRole {
    TypedContracts,
    ValidationDiagnostics,
    CryptographicVerification,
    AuthorityCapability,
    StorageProjection,
    DomainService,
    RuntimeService,
    ProtocolAdapter,
    CommandAdapter,
    SchemaGenerator,
    CompatibilityBinary,
    Observability,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceTestExpectations {
    pub unit_tests: bool,
    pub boundary_rejection_tests: bool,
    pub serialization_tests: bool,
    pub integration_tests: bool,
    pub no_host_authority_tests: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceCrateBoundary {
    pub id: CrateId,
    pub path: &'static str,
    pub layer: ArchitecturalLayer,
    pub allowed_dependency_layers: &'static [ArchitecturalLayer],
    /// Dependencies that deliberately cross the normal layer direction.
    pub explicit_exceptions: &'static [CrateId],
    pub dependencies: &'static [CrateId],
    pub feature_boundary: FeatureBoundary,
    pub authority_class: AuthorityClass,
    pub mutation_class: MutationClass,
    pub public_api_role: PublicApiRole,
    pub source_test_expectations: SourceTestExpectations,
}

const NONE: &[ArchitecturalLayer] = &[];
const CONTRACT: &[ArchitecturalLayer] = &[ArchitecturalLayer::Contract];
const CONTRACT_FOUNDATION: &[ArchitecturalLayer] =
    &[ArchitecturalLayer::Contract, ArchitecturalLayer::Foundation];
const CONTRACT_VALIDATION: &[ArchitecturalLayer] =
    &[ArchitecturalLayer::Contract, ArchitecturalLayer::Validation];
const CONTRACT_VALIDATION_STORAGE: &[ArchitecturalLayer] = &[
    ArchitecturalLayer::Contract,
    ArchitecturalLayer::Validation,
    ArchitecturalLayer::Storage,
];
const CONTRACT_STORAGE: &[ArchitecturalLayer] =
    &[ArchitecturalLayer::Contract, ArchitecturalLayer::Storage];
const CONTRACT_FOUNDATION_VALIDATION_SECURITY: &[ArchitecturalLayer] = &[
    ArchitecturalLayer::Contract,
    ArchitecturalLayer::Foundation,
    ArchitecturalLayer::Validation,
    ArchitecturalLayer::Security,
];
const CONTRACT_FOUNDATION_VALIDATION_SECURITY_STORAGE: &[ArchitecturalLayer] = &[
    ArchitecturalLayer::Contract,
    ArchitecturalLayer::Foundation,
    ArchitecturalLayer::Validation,
    ArchitecturalLayer::Security,
    ArchitecturalLayer::Storage,
];
const CONTRACT_FOUNDATION_VALIDATION_SECURITY_STORAGE_DOMAIN: &[ArchitecturalLayer] = &[
    ArchitecturalLayer::Contract,
    ArchitecturalLayer::Foundation,
    ArchitecturalLayer::Validation,
    ArchitecturalLayer::Security,
    ArchitecturalLayer::Storage,
    ArchitecturalLayer::Domain,
];
const ALL_CORE_LAYERS: &[ArchitecturalLayer] = &[
    ArchitecturalLayer::Contract,
    ArchitecturalLayer::Foundation,
    ArchitecturalLayer::Validation,
    ArchitecturalLayer::Security,
    ArchitecturalLayer::Storage,
    ArchitecturalLayer::Domain,
    ArchitecturalLayer::Runtime,
    ArchitecturalLayer::Protocol,
];
const STANDARD_TESTS: SourceTestExpectations = SourceTestExpectations {
    unit_tests: true,
    boundary_rejection_tests: true,
    serialization_tests: false,
    integration_tests: false,
    no_host_authority_tests: true,
};
const SERIALIZED_TESTS: SourceTestExpectations = SourceTestExpectations {
    serialization_tests: true,
    ..STANDARD_TESTS
};
const INTEGRATION_TESTS: SourceTestExpectations = SourceTestExpectations {
    integration_tests: true,
    ..STANDARD_TESTS
};

/// The complete workspace inventory. Keep this in lockstep with root `Cargo.toml`.
pub static WORKSPACE_CRATE_BOUNDARIES: &[WorkspaceCrateBoundary] = &[
    WorkspaceCrateBoundary {
        id: CrateId::ContractValidator,
        path: "crates/forge-contract-validator",
        layer: ArchitecturalLayer::Compatibility,
        allowed_dependency_layers: &[ArchitecturalLayer::Adapter],
        explicit_exceptions: &[],
        dependencies: &[CrateId::Cli],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::ReadOnlyProjection,
        public_api_role: PublicApiRole::CompatibilityBinary,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Authority,
        path: "crates/forge-core-authority",
        layer: ArchitecturalLayer::Security,
        allowed_dependency_layers: CONTRACT_FOUNDATION,
        explicit_exceptions: &[CrateId::Decisions],
        dependencies: &[
            CrateId::CommandSurface,
            CrateId::Contracts,
            CrateId::Decisions,
        ],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::ExecutionAdmission,
        mutation_class: MutationClass::PreparedEffectOnly,
        public_api_role: PublicApiRole::AuthorityCapability,
        source_test_expectations: SERIALIZED_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Cli,
        path: "crates/forge-core-cli",
        layer: ArchitecturalLayer::Adapter,
        allowed_dependency_layers: ALL_CORE_LAYERS,
        explicit_exceptions: &[CrateId::Authority],
        dependencies: &[
            CrateId::Authority,
            CrateId::CommandSurface,
            CrateId::Contracts,
            CrateId::Crypto,
            CrateId::Decisions,
            CrateId::DomainPackLearningStore,
            CrateId::DomainPackTcb,
            CrateId::Eval,
            CrateId::EvalHarness,
            CrateId::Eventlog,
            CrateId::Governance,
            CrateId::Graph,
            CrateId::Kernel,
            CrateId::Memory,
            CrateId::ProtocolMcp,
            CrateId::Research,
            CrateId::Store,
            CrateId::Trace,
            CrateId::Validate,
        ],
        feature_boundary: FeatureBoundary::TestOnly(&["expensive-p6d-e2e"]),
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::HostAdapter,
        public_api_role: PublicApiRole::CommandAdapter,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::CommandSurface,
        path: "crates/forge-core-command-surface",
        layer: ArchitecturalLayer::Foundation,
        allowed_dependency_layers: NONE,
        explicit_exceptions: &[],
        dependencies: &[],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::None,
        public_api_role: PublicApiRole::TypedContracts,
        source_test_expectations: SERIALIZED_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Contracts,
        path: "crates/forge-core-contracts",
        layer: ArchitecturalLayer::Contract,
        allowed_dependency_layers: NONE,
        explicit_exceptions: &[],
        dependencies: &[],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::None,
        public_api_role: PublicApiRole::TypedContracts,
        source_test_expectations: SERIALIZED_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Crypto,
        path: "crates/forge-core-crypto",
        layer: ArchitecturalLayer::Security,
        allowed_dependency_layers: CONTRACT,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Contracts],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::Verification,
        mutation_class: MutationClass::ReadOnlyProjection,
        public_api_role: PublicApiRole::CryptographicVerification,
        source_test_expectations: SERIALIZED_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Decisions,
        path: "crates/forge-core-decisions",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Contracts],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::PolicyDecision,
        mutation_class: MutationClass::ReadOnlyProjection,
        public_api_role: PublicApiRole::DomainService,
        source_test_expectations: SERIALIZED_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::DomainPackLearningStore,
        path: "crates/forge-core-domain-pack-learning-store",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT_STORAGE,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Contracts, CrateId::Store],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::DurableState,
        public_api_role: PublicApiRole::StorageProjection,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::DomainPackTcb,
        path: "crates/forge-core-domain-pack-tcb",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT_FOUNDATION_VALIDATION_SECURITY_STORAGE,
        explicit_exceptions: &[CrateId::Authority, CrateId::Decisions],
        dependencies: &[
            CrateId::Authority,
            CrateId::Contracts,
            CrateId::Decisions,
            CrateId::Store,
        ],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::TrustedComputingBase,
        mutation_class: MutationClass::DurableState,
        public_api_role: PublicApiRole::DomainService,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Eval,
        path: "crates/forge-core-eval",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT_VALIDATION,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Contracts, CrateId::Validate],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::ReadOnlyProjection,
        public_api_role: PublicApiRole::DomainService,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::EvalHarness,
        path: "crates/forge-core-eval-harness",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT_VALIDATION,
        explicit_exceptions: &[CrateId::Eval],
        dependencies: &[CrateId::Contracts, CrateId::Eval, CrateId::Validate],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::ReadOnlyProjection,
        public_api_role: PublicApiRole::DomainService,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Eventlog,
        path: "crates/forge-core-eventlog",
        layer: ArchitecturalLayer::Storage,
        allowed_dependency_layers: CONTRACT_STORAGE,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Contracts, CrateId::Store],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::DurableState,
        public_api_role: PublicApiRole::Observability,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Governance,
        path: "crates/forge-core-governance",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT_STORAGE,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Eventlog],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::PolicyDecision,
        mutation_class: MutationClass::DurableState,
        public_api_role: PublicApiRole::DomainService,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Graph,
        path: "crates/forge-core-graph",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT_VALIDATION,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Contracts, CrateId::Validate],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::ReadOnlyProjection,
        public_api_role: PublicApiRole::DomainService,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Kernel,
        path: "crates/forge-core-kernel",
        layer: ArchitecturalLayer::Runtime,
        allowed_dependency_layers: CONTRACT_FOUNDATION_VALIDATION_SECURITY_STORAGE_DOMAIN,
        explicit_exceptions: &[CrateId::Authority],
        dependencies: &[
            CrateId::Authority,
            CrateId::Contracts,
            CrateId::Decisions,
            CrateId::DomainPackTcb,
            CrateId::Store,
            CrateId::Trace,
            CrateId::Validate,
            CrateId::WorkflowGovernanceTcb,
        ],
        feature_boundary: FeatureBoundary::DenyInProduction(&["dangerous-bypass"]),
        authority_class: AuthorityClass::ExecutionAdmission,
        mutation_class: MutationClass::PreparedEffectOnly,
        public_api_role: PublicApiRole::RuntimeService,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Memory,
        path: "crates/forge-core-memory",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT_STORAGE,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Eventlog],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::DurableState,
        public_api_role: PublicApiRole::DomainService,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::ProtocolMcp,
        path: "crates/forge-core-protocol-mcp",
        layer: ArchitecturalLayer::Protocol,
        allowed_dependency_layers: CONTRACT_FOUNDATION_VALIDATION_SECURITY_STORAGE_DOMAIN,
        explicit_exceptions: &[CrateId::Authority, CrateId::Kernel],
        dependencies: &[
            CrateId::Authority,
            CrateId::CommandSurface,
            CrateId::Contracts,
            CrateId::Decisions,
            CrateId::Kernel,
            CrateId::Research,
            CrateId::Store,
            CrateId::Validate,
        ],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::HostAdapter,
        public_api_role: PublicApiRole::ProtocolAdapter,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Research,
        path: "crates/forge-core-research",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT_VALIDATION_STORAGE,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Eventlog, CrateId::Validate],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::DurableState,
        public_api_role: PublicApiRole::DomainService,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Schema,
        path: "crates/forge-core-schema",
        layer: ArchitecturalLayer::Foundation,
        allowed_dependency_layers: CONTRACT,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Contracts],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::ReadOnlyProjection,
        public_api_role: PublicApiRole::SchemaGenerator,
        source_test_expectations: SERIALIZED_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Store,
        path: "crates/forge-core-store",
        layer: ArchitecturalLayer::Storage,
        allowed_dependency_layers: CONTRACT_FOUNDATION_VALIDATION_SECURITY,
        explicit_exceptions: &[CrateId::Authority],
        dependencies: &[
            CrateId::Authority,
            CrateId::Contracts,
            CrateId::Trace,
            CrateId::Validate,
        ],
        feature_boundary: FeatureBoundary::TestOnly(&["fuzz"]),
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::DurableState,
        public_api_role: PublicApiRole::StorageProjection,
        source_test_expectations: INTEGRATION_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Trace,
        path: "crates/forge-core-trace",
        layer: ArchitecturalLayer::Foundation,
        allowed_dependency_layers: NONE,
        explicit_exceptions: &[],
        dependencies: &[],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::ReadOnlyProjection,
        public_api_role: PublicApiRole::Observability,
        source_test_expectations: STANDARD_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::Validate,
        path: "crates/forge-core-validate",
        layer: ArchitecturalLayer::Validation,
        allowed_dependency_layers: CONTRACT,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Contracts],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::None,
        mutation_class: MutationClass::ReadOnlyProjection,
        public_api_role: PublicApiRole::ValidationDiagnostics,
        source_test_expectations: SERIALIZED_TESTS,
    },
    WorkspaceCrateBoundary {
        id: CrateId::WorkflowGovernanceTcb,
        path: "crates/forge-core-workflow-governance-tcb",
        layer: ArchitecturalLayer::Domain,
        allowed_dependency_layers: CONTRACT_STORAGE,
        explicit_exceptions: &[],
        dependencies: &[CrateId::Contracts, CrateId::Store],
        feature_boundary: FeatureBoundary::NoOptionalFeatures,
        authority_class: AuthorityClass::TrustedComputingBase,
        mutation_class: MutationClass::DurableState,
        public_api_role: PublicApiRole::DomainService,
        source_test_expectations: INTEGRATION_TESTS,
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceCrateObservation<'a> {
    pub package_name: &'a str,
    pub path: &'a str,
    /// Local workspace package names only; registry dependencies are outside this contract.
    pub workspace_dependencies: &'a [&'a str],
    pub enabled_features: &'a [&'a str],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceProjection<'a> {
    pub crates: &'a [WorkspaceCrateObservation<'a>],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceBoundaryViolation {
    MissingCrate {
        package_name: &'static str,
    },
    ExtraCrate {
        package_name: String,
    },
    DuplicatePackage {
        package_name: String,
    },
    DuplicatePath {
        path: String,
    },
    PathEscape {
        package_name: String,
        path: String,
    },
    PathMismatch {
        package_name: String,
        expected: &'static str,
        found: String,
    },
    UnknownDependencyEdge {
        from: String,
        to: String,
    },
    UndeclaredDependencyEdge {
        from: String,
        to: String,
    },
    ForbiddenLayerEdge {
        from: String,
        to: String,
    },
    UnreviewedAuthorityEdge {
        from: String,
        to: String,
    },
    ForbiddenFeature {
        package_name: String,
        feature: String,
    },
    DependencyCycle {
        crates: Vec<String>,
    },
    PartialEightCrateProjection {
        found: Vec<String>,
    },
}

/// Returns the static boundary for a workspace crate.
///
/// # Panics
///
/// Panics only if the exhaustive static catalog omits a `CrateId` variant.
#[must_use]
pub fn boundary_for(id: CrateId) -> &'static WorkspaceCrateBoundary {
    WORKSPACE_CRATE_BOUNDARIES
        .iter()
        .find(|boundary| boundary.id == id)
        .expect("the exhaustive workspace boundary catalog includes every CrateId")
}

/// Validates a complete Cargo workspace projection against this static contract.
#[must_use]
pub fn validate_workspace_projection(
    projection: &WorkspaceProjection<'_>,
) -> Vec<WorkspaceBoundaryViolation> {
    let mut violations = Vec::new();
    let mut observed = BTreeMap::new();
    let mut paths = BTreeSet::new();

    for crate_observation in projection.crates {
        let package_name = crate_observation.package_name.to_owned();
        if observed
            .insert(package_name.clone(), crate_observation)
            .is_some()
        {
            violations.push(WorkspaceBoundaryViolation::DuplicatePackage { package_name });
            continue;
        }
        if !is_workspace_relative_path(crate_observation.path) {
            violations.push(WorkspaceBoundaryViolation::PathEscape {
                package_name: crate_observation.package_name.to_owned(),
                path: crate_observation.path.to_owned(),
            });
        }
        if !paths.insert(crate_observation.path) {
            violations.push(WorkspaceBoundaryViolation::DuplicatePath {
                path: crate_observation.path.to_owned(),
            });
        }
    }

    if projection.crates.len() == 8 {
        let mut found = projection
            .crates
            .iter()
            .map(|crate_observation| crate_observation.package_name.to_owned())
            .collect::<Vec<_>>();
        found.sort();
        violations.push(WorkspaceBoundaryViolation::PartialEightCrateProjection { found });
    }

    for boundary in WORKSPACE_CRATE_BOUNDARIES {
        if !observed.contains_key(boundary.id.package_name()) {
            violations.push(WorkspaceBoundaryViolation::MissingCrate {
                package_name: boundary.id.package_name(),
            });
        }
    }

    for (package_name, crate_observation) in &observed {
        let Some(id) = CrateId::parse(package_name) else {
            violations.push(WorkspaceBoundaryViolation::ExtraCrate {
                package_name: package_name.clone(),
            });
            continue;
        };
        let boundary = boundary_for(id);
        if crate_observation.path != boundary.path {
            violations.push(WorkspaceBoundaryViolation::PathMismatch {
                package_name: package_name.clone(),
                expected: boundary.path,
                found: crate_observation.path.to_owned(),
            });
        }
        for feature in crate_observation.enabled_features {
            if !boundary.feature_boundary.permits(feature) {
                violations.push(WorkspaceBoundaryViolation::ForbiddenFeature {
                    package_name: package_name.clone(),
                    feature: (*feature).to_owned(),
                });
            }
        }
        for dependency_name in crate_observation.workspace_dependencies {
            let Some(dependency_id) = CrateId::parse(dependency_name) else {
                violations.push(WorkspaceBoundaryViolation::UnknownDependencyEdge {
                    from: package_name.clone(),
                    to: (*dependency_name).to_owned(),
                });
                continue;
            };
            let dependency = boundary_for(dependency_id);
            if !boundary.dependencies.contains(&dependency_id) {
                violations.push(WorkspaceBoundaryViolation::UndeclaredDependencyEdge {
                    from: package_name.clone(),
                    to: (*dependency_name).to_owned(),
                });
            }
            if !boundary
                .allowed_dependency_layers
                .contains(&dependency.layer)
                && !boundary.explicit_exceptions.contains(&dependency_id)
            {
                violations.push(WorkspaceBoundaryViolation::ForbiddenLayerEdge {
                    from: package_name.clone(),
                    to: (*dependency_name).to_owned(),
                });
            }
            if dependency.authority_class == AuthorityClass::ExecutionAdmission
                && !boundary.explicit_exceptions.contains(&dependency_id)
            {
                violations.push(WorkspaceBoundaryViolation::UnreviewedAuthorityEdge {
                    from: package_name.clone(),
                    to: (*dependency_name).to_owned(),
                });
            }
        }
    }

    violations.extend(find_dependency_cycles(&observed));
    violations
}

fn is_workspace_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn find_dependency_cycles(
    observed: &BTreeMap<String, &WorkspaceCrateObservation<'_>>,
) -> Vec<WorkspaceBoundaryViolation> {
    fn visit(
        package_name: &str,
        observed: &BTreeMap<String, &WorkspaceCrateObservation<'_>>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        stack: &mut Vec<String>,
        cycles: &mut BTreeSet<Vec<String>>,
    ) {
        if visited.contains(package_name) {
            return;
        }
        if !visiting.insert(package_name.to_owned()) {
            if let Some(start) = stack.iter().position(|entry| entry == package_name) {
                let mut cycle = stack[start..].to_vec();
                cycle.push(package_name.to_owned());
                cycles.insert(canonical_cycle(cycle));
            }
            return;
        }
        stack.push(package_name.to_owned());
        if let Some(crate_observation) = observed.get(package_name) {
            for dependency in crate_observation.workspace_dependencies {
                if observed.contains_key(*dependency) {
                    visit(dependency, observed, visiting, visited, stack, cycles);
                }
            }
        }
        stack.pop();
        visiting.remove(package_name);
        visited.insert(package_name.to_owned());
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack = Vec::new();
    let mut cycles = BTreeSet::new();
    for package_name in observed.keys() {
        visit(
            package_name,
            observed,
            &mut visiting,
            &mut visited,
            &mut stack,
            &mut cycles,
        );
    }
    cycles
        .into_iter()
        .map(|crates| WorkspaceBoundaryViolation::DependencyCycle { crates })
        .collect()
}

fn canonical_cycle(mut cycle: Vec<String>) -> Vec<String> {
    cycle.pop();
    let Some((start, _)) = cycle.iter().enumerate().min_by_key(|(_, value)| *value) else {
        return cycle;
    };
    cycle.rotate_left(start);
    let first = cycle[0].clone();
    cycle.push(first);
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_projection<'a>() -> Vec<WorkspaceCrateObservation<'a>> {
        WORKSPACE_CRATE_BOUNDARIES
            .iter()
            .map(|boundary| WorkspaceCrateObservation {
                package_name: boundary.id.package_name(),
                path: boundary.path,
                workspace_dependencies: &[],
                enabled_features: &[],
            })
            .collect()
    }

    #[test]
    fn catalog_covers_each_workspace_crate_once() {
        assert_eq!(WORKSPACE_CRATE_BOUNDARIES.len(), WORKSPACE_CRATE_COUNT);
        let packages = WORKSPACE_CRATE_BOUNDARIES
            .iter()
            .map(|boundary| boundary.id.package_name())
            .collect::<BTreeSet<_>>();
        let paths = WORKSPACE_CRATE_BOUNDARIES
            .iter()
            .map(|boundary| boundary.path)
            .collect::<BTreeSet<_>>();
        assert_eq!(packages.len(), WORKSPACE_CRATE_COUNT);
        assert_eq!(paths.len(), WORKSPACE_CRATE_COUNT);
        assert_eq!(CrateId::ALL.len(), WORKSPACE_CRATE_COUNT);
        assert_eq!(
            CrateId::ALL
                .iter()
                .map(|id| id.package_name())
                .collect::<BTreeSet<_>>(),
            packages
        );
    }

    #[test]
    fn rejects_missing_extra_duplicate_and_legacy_eight_projections() {
        let mut crates = complete_projection();
        crates.truncate(7);
        crates.push(WorkspaceCrateObservation {
            package_name: "unknown-workspace-crate",
            path: "crates/unknown-workspace-crate",
            workspace_dependencies: &[],
            enabled_features: &[],
        });
        let violations = validate_workspace_projection(&WorkspaceProjection { crates: &crates });
        assert!(violations.iter().any(|violation| matches!(
            violation,
            WorkspaceBoundaryViolation::PartialEightCrateProjection { .. }
        )));
        assert!(violations
            .iter()
            .any(|violation| matches!(violation, WorkspaceBoundaryViolation::ExtraCrate { .. })));
        assert!(violations
            .iter()
            .any(|violation| matches!(violation, WorkspaceBoundaryViolation::MissingCrate { .. })));
    }

    #[test]
    fn rejects_escape_authority_and_cycle_edges() {
        let mut crates = complete_projection();
        let cli = crates
            .iter_mut()
            .find(|crate_observation| crate_observation.package_name == "forge-core-cli")
            .unwrap();
        cli.path = "../outside";
        let contracts = crates
            .iter_mut()
            .find(|crate_observation| crate_observation.package_name == "forge-core-contracts")
            .unwrap();
        contracts.workspace_dependencies = &["forge-core-authority"];
        let authority = crates
            .iter_mut()
            .find(|crate_observation| crate_observation.package_name == "forge-core-authority")
            .unwrap();
        authority.workspace_dependencies = &["forge-core-contracts"];
        let violations = validate_workspace_projection(&WorkspaceProjection { crates: &crates });
        assert!(violations
            .iter()
            .any(|violation| matches!(violation, WorkspaceBoundaryViolation::PathEscape { .. })));
        assert!(violations.iter().any(|violation| matches!(
            violation,
            WorkspaceBoundaryViolation::UnreviewedAuthorityEdge { .. }
        )));
        assert!(violations.iter().any(|violation| matches!(
            violation,
            WorkspaceBoundaryViolation::DependencyCycle { .. }
        )));
    }

    #[test]
    fn candidate_documents_remain_inert_and_hostless() {
        assert_eq!(CandidateDocumentBoundary::SELECTED_HOST, None);
        assert!(!CandidateDocumentBoundary::PERMITS_SIGNING);
        assert!(!CandidateDocumentBoundary::PERMITS_TRUST);
        assert!(!CandidateDocumentBoundary::PERMITS_ADMISSION);
        assert!(!CandidateDocumentBoundary::PERMITS_PRIVATE_KEY_MATERIAL);
        assert!(!CandidateDocumentBoundary::PERMITS_STATE_MUTATION);
    }
}
