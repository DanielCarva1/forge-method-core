pub mod catalog;
pub mod claim;
pub mod command;
pub mod common;
pub mod envelope;
pub mod typed_failure;

pub mod agent_run;
pub mod autonomy_policy;
pub mod checkpoint;
pub mod completion;
pub mod coordination_eval;
pub mod decision;
pub mod eval_run;
pub mod evidence;
pub mod gate;
pub mod governance;
pub mod guide_decision;
pub mod inventory;
pub mod isolation;
pub mod memory;
pub mod operation;
pub mod operation_reference;
pub mod phase;
pub mod project_link;
pub mod recovery;
pub mod request;
pub mod research;
pub mod runtime;
pub mod telemetry;
pub mod tool_effect;
pub mod verification_goal;
pub mod workflow;

pub use agent_run::{AgentRunContract, AgentRunContractDocument};
pub use autonomy_policy::{AutonomyPolicyContract, AutonomyPolicyContractDocument};
pub use checkpoint::{CheckpointContract, CheckpointContractDocument};
pub use claim::{ClaimContract, ClaimContractDocument};
pub use command::{CommandContract, CommandContractDocument};
pub use common::{ClaimId, PrincipalId, RepoPath, ScopeId, SourceId, StableId};
pub use completion::{CompletionContract, CompletionContractDocument};
pub use coordination_eval::{
    CoordinationDimension, CoordinationEvalContract, CoordinationEvalContractDocument,
    CoordinationMetricKind,
};
pub use decision::{
    DecisionCloseContract, DecisionCloseContractDocument, DecisionEvidenceKind, DecisionKind,
    DecisionStatus,
};
pub use eval_run::{EvalRunContract, EvalRunContractDocument};
pub use evidence::{EvidenceSource, FieldEvidenceRegistry};
pub use gate::{GateContract, GateContractDocument};
pub use governance::{
    ConflictContract, ConflictDetectionReason, ConflictPolicy, ConflictResolutionState,
    GovernancePolicy, IntentContract, IntentScope, IntentScopeKind, ResolutionDecision,
};
pub use inventory::{ContractFamily, ContractFamilyInventory, ContractFamilyInventoryDocument};
pub use isolation::{
    GitAction, IsolationContract, IsolationContractDocument, IsolationError, IsolationStatus,
    MergePlan, MergePolicy, MergeStep,
};
pub use memory::{
    AdmissionDecision, AdmissionDenialReason, AdmissionEvidence, ApprovalState, AuthorityLevel,
    EvidenceField, Freshness, MemoryContract, MemoryContractDocument, MemoryEntry, MemoryKind,
    MemoryPolicy, MemoryProvenance, MemoryScope, MemoryScopeKind, ReviewState,
};
pub use operation::{OperationContract, OperationContractDocument};
pub use operation_reference::OperationReferencePolicyDocument;
pub use project_link::{ProjectLinkDocument, PROJECT_LINK_FILE_NAME, PROJECT_LINK_SCHEMA_VERSION};
pub use recovery::{
    HealthRecoveryContract, HealthRecoveryContractDocument, HealthStatus, RecoveryAction,
};
pub use request::{RequestContract, RequestContractDocument};
pub use runtime::{
    RuntimeBlockedReason, RuntimeCapability, RuntimeCapabilityDocument, RuntimeCapabilityKind,
    RuntimeHandoffContract, RuntimeHandoffContractDocument, RuntimeHandoffStatus, RuntimeKind,
    RuntimeRegistryEntryDocument,
};
pub use telemetry::{TelemetryContract, TelemetryContractDocument};
pub use tool_effect::{ToolEffectContract, ToolEffectContractDocument};
pub use verification_goal::{VerificationGoalContract, VerificationGoalContractDocument};

pub use catalog::{Catalog, CatalogDocument, CatalogEntry};
pub use envelope::{CliEnvelope, CliError, ExitReason, ENVELOPE_SCHEMA_VERSION};
pub use guide_decision::{GuideDecision, GuideDecisionDocument};
pub use phase::Phase;
pub use research::{
    ResearchAdmissionDecision, ResearchAdmissionDenialReason, ResearchContract, ResearchPolicy,
    ResearchSource, ResearchSourceKind,
};
pub use typed_failure::TypedFailure;
pub use workflow::{Workflow, WorkflowDocument};
