pub mod catalog;
pub mod claim;
pub mod command;
pub mod common;
pub mod envelope;

pub mod completion;
pub mod coordination_eval;
pub mod decision;
pub mod evidence;
pub mod gate;
pub mod guide_decision;
pub mod inventory;
pub mod isolation;
pub mod operation;
pub mod operation_reference;
pub mod phase;
pub mod recovery;
pub mod request;
pub mod runtime;
pub mod tool_effect;
pub mod workflow;

pub use claim::{ClaimContract, ClaimContractDocument};
pub use command::{CommandContract, CommandContractDocument};
pub use common::{ClaimId, RepoPath, ScopeId, SourceId, StableId};
pub use completion::{CompletionContract, CompletionContractDocument};
pub use coordination_eval::{
    CoordinationDimension, CoordinationEvalContract, CoordinationEvalContractDocument,
    CoordinationMetricKind,
};
pub use decision::{
    DecisionCloseContract, DecisionCloseContractDocument, DecisionEvidenceKind, DecisionKind,
    DecisionStatus,
};
pub use evidence::{EvidenceSource, FieldEvidenceRegistry};
pub use gate::{GateContract, GateContractDocument};
pub use inventory::{ContractFamily, ContractFamilyInventory, ContractFamilyInventoryDocument};
pub use isolation::{
    GitAction, IsolationContract, IsolationContractDocument, IsolationError, IsolationStatus,
    MergePlan, MergePolicy, MergeStep,
};
pub use operation::{OperationContract, OperationContractDocument};
pub use operation_reference::OperationReferencePolicyDocument;
pub use recovery::{
    HealthRecoveryContract, HealthRecoveryContractDocument, HealthStatus, RecoveryAction,
};
pub use request::{RequestContract, RequestContractDocument};
pub use runtime::{
    RuntimeBlockedReason, RuntimeCapability, RuntimeCapabilityDocument, RuntimeCapabilityKind,
    RuntimeHandoffContract, RuntimeHandoffContractDocument, RuntimeHandoffStatus, RuntimeKind,
    RuntimeRegistryEntryDocument,
};
pub use tool_effect::{ToolEffectContract, ToolEffectContractDocument};

pub use catalog::{Catalog, CatalogDocument, CatalogEntry};
pub use envelope::{CliEnvelope, CliError, ExitReason, ENVELOPE_SCHEMA_VERSION};
pub use guide_decision::{GuideDecision, GuideDecisionDocument};
pub use phase::Phase;
pub use workflow::{Workflow, WorkflowDocument};
