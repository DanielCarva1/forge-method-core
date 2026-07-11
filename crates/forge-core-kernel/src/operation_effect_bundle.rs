//! Canonical operation-wide transaction envelope for file-backed effects.
//!
//! The effect store already provides atomic before-image WAL semantics for one
//! `ToolEffectContractDocument`. This Module deepens that Interface by
//! compiling a complete ordered operation effect set into one internal effect
//! envelope, so every constituent write enters one lock, one Begin/Commit, and
//! one rollback domain. Original effect identities remain separately bound in
//! execution provenance; the envelope is implementation, never caller input.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use forge_core_contracts::tool_effect::{
    ConflictCode, ConflictDetection, ConflictPolicy, EffectKind, EffectNotification, EffectRepair,
    EffectTargetKind, InverseKind, InverseMetadata, InverseSource, RepairStrategy,
    ToolEffectContract,
};
use forge_core_contracts::{
    OperationContractDocument, RepoPath, StableId, ToolEffectContractDocument,
};
use forge_core_store::{
    apply_file_effect_transaction_with_wal_lock_with_durability, resolve_effect_physical_ref,
    EffectApplicationPayload, EffectApplicationResult, WalDurability,
};
use forge_core_validate::{validate_operation, validate_tool_effect, DiagnosticSeverity};

pub const OPERATION_EFFECT_BUNDLE_SCHEMA_VERSION: &str = "0.1";

/// One verified, deterministic operation-wide transaction envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationEffectBundle {
    operation_id: StableId,
    effect_refs: Vec<RepoPath>,
    effect_ids: Vec<StableId>,
    transaction_effect: ToolEffectContractDocument,
}

impl OperationEffectBundle {
    #[must_use]
    pub const fn operation_id(&self) -> &StableId {
        &self.operation_id
    }

    #[must_use]
    pub fn effect_refs(&self) -> &[RepoPath] {
        &self.effect_refs
    }

    #[must_use]
    pub fn effect_ids(&self) -> &[StableId] {
        &self.effect_ids
    }

    #[must_use]
    pub const fn transaction_effect(&self) -> &ToolEffectContractDocument {
        &self.transaction_effect
    }

    #[must_use]
    pub fn into_transaction_effect(self) -> ToolEffectContractDocument {
        self.transaction_effect
    }
}

/// Compile two or more declared effects into one atomic file transaction.
///
/// Effects must be supplied in the exact order declared by the operation.
/// Every source contract is validated, actor/operation bindings must agree,
/// and normalized write targets must be disjoint. These restrictions make
/// ordering-independent atomicity explicit instead of silently inventing
/// sequential semantics for overlapping writes.
///
/// # Errors
///
/// Returns [`OperationEffectBundleError`] on incomplete, reordered, invalid,
/// non-file-backed, cross-actor, or overlapping effect sets.
pub fn compose_operation_effect_bundle(
    effect_store_root: impl AsRef<Path>,
    operation: &OperationContractDocument,
    effect_refs: &[RepoPath],
    effects: &[ToolEffectContractDocument],
) -> Result<OperationEffectBundle, OperationEffectBundleError> {
    let effect_store_root = effect_store_root.as_ref();
    let operation_id = &operation.operation_contract.contract_id;
    let declared_refs = &operation.operation_contract.effect_contract_refs;
    let operation_validation = validate_operation(operation);
    if operation_validation
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err(OperationEffectBundleError::InvalidOperation);
    }
    if effects.len() < 2 {
        return Err(OperationEffectBundleError::MultipleEffectsRequired {
            observed: effects.len(),
        });
    }
    if effect_refs.len() != effects.len() || declared_refs != effect_refs {
        return Err(OperationEffectBundleError::EffectSetMismatch);
    }

    let mut effect_ids = Vec::with_capacity(effects.len());
    let mut seen_ids = BTreeSet::new();
    let mut actor = None;
    let mut reads_by_target = BTreeMap::new();
    let mut writes = Vec::new();
    let mut seen_write_targets = BTreeMap::<String, String>::new();
    let mut recipients = BTreeSet::new();
    let mut notification_required = false;
    let mut destructive = false;

    for document in effects {
        let effect = &document.tool_effect_contract;
        let validation = validate_tool_effect(document);
        if validation
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        {
            return Err(OperationEffectBundleError::InvalidEffect {
                effect_id: effect.id.0.clone(),
                reasons: validation
                    .diagnostics()
                    .iter()
                    .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect(),
            });
        }
        if effect.operation_ref != *operation_id {
            return Err(OperationEffectBundleError::EffectBindingMismatch {
                effect_id: effect.id.0.clone(),
            });
        }
        if !seen_ids.insert(effect.id.0.clone()) {
            return Err(OperationEffectBundleError::DuplicateEffectId(
                effect.id.0.clone(),
            ));
        }
        if let Some(expected) = &actor {
            if expected != &effect.actor {
                return Err(OperationEffectBundleError::ActorMismatch {
                    effect_id: effect.id.0.clone(),
                });
            }
        } else {
            actor = Some(effect.actor.clone());
        }
        for read in &effect.read_set {
            let key = if file_backed_target(read.target_kind) {
                normalized_target(effect_store_root, read.target_kind, &read.reference).map_err(
                    |()| OperationEffectBundleError::UnsupportedTarget {
                        effect_id: effect.id.0.clone(),
                        reference: read.reference.clone(),
                    },
                )?
            } else {
                format!("logical:{:?}:{}", read.target_kind, read.reference).to_lowercase()
            };
            if let Some(existing) = reads_by_target.get(&key) {
                if existing != read {
                    return Err(OperationEffectBundleError::ConflictingRead {
                        reference: read.reference.clone(),
                    });
                }
            } else {
                reads_by_target.insert(key, read.clone());
            }
        }
        for write in &effect.write_set {
            if !file_backed_target(write.target_kind) {
                return Err(OperationEffectBundleError::UnsupportedTarget {
                    effect_id: effect.id.0.clone(),
                    reference: write.reference.clone(),
                });
            }
            let key = normalized_target(effect_store_root, write.target_kind, &write.reference)
                .map_err(|()| OperationEffectBundleError::UnsupportedTarget {
                    effect_id: effect.id.0.clone(),
                    reference: write.reference.clone(),
                })?;
            if let Some(first_effect) = seen_write_targets.insert(key, effect.id.0.clone()) {
                return Err(OperationEffectBundleError::OverlappingWrite {
                    reference: write.reference.clone(),
                    first_effect,
                    second_effect: effect.id.0.clone(),
                });
            }
            destructive |= write.destructive;
            writes.push(write.clone());
        }
        notification_required |= effect.notification.required;
        recipients.extend(
            effect
                .notification
                .recipients
                .iter()
                .map(|recipient| recipient.0.clone()),
        );
        effect_ids.push(effect.id.clone());
    }

    let actor = actor.ok_or(OperationEffectBundleError::EffectSetMismatch)?;
    let transaction_effect = ToolEffectContractDocument {
        schema_version: OPERATION_EFFECT_BUNDLE_SCHEMA_VERSION.to_owned(),
        tool_effect_contract: ToolEffectContract {
            id: StableId(format!("operation-wide.{}", operation_id.0)),
            contract_ref: RepoPath(format!(
                ".forge-method/runtime/operation-wide/{}.yaml",
                operation_id.0
            )),
            effect_kind: EffectKind::OperationTransaction,
            operation_ref: operation_id.clone(),
            actor,
            read_set: reads_by_target.into_values().collect(),
            write_set: writes,
            conflict_detection: ConflictDetection {
                check_against: StableId("operation-wide-effect-set".to_owned()),
                granularity: StableId("normalized-target".to_owned()),
                conflict_codes: vec![
                    ConflictCode::ReadTargetChanged,
                    ConflictCode::WriteTargetChanged,
                    ConflictCode::WriteTargetClaimed,
                    ConflictCode::OverlappingWriteSet,
                ],
                policy: ConflictPolicy::Block,
            },
            notification: EffectNotification {
                required: notification_required,
                recipients: recipients.into_iter().map(StableId).collect(),
                request_contract_ref: None,
            },
            repair: operation_wide_repair(destructive),
        },
    };
    let validation = validate_tool_effect(&transaction_effect);
    if validation
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err(OperationEffectBundleError::InvalidTransactionEnvelope);
    }

    Ok(OperationEffectBundle {
        operation_id: operation_id.clone(),
        effect_refs: effect_refs.to_vec(),
        effect_ids,
        transaction_effect,
    })
}

/// Apply a verified operation bundle through one effect-store lock and WAL.
///
/// Callers cannot substitute a constituent effect here: only the transaction
/// envelope owned by [`OperationEffectBundle`] reaches the store.
#[must_use]
pub fn apply_operation_effect_bundle_with_wal_lock(
    effect_store_root: impl AsRef<Path>,
    bundle: &OperationEffectBundle,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    lock_relative_path: &str,
    tx_id: impl Into<String>,
) -> EffectApplicationResult {
    apply_operation_effect_bundle_with_wal_lock_and_durability(
        effect_store_root,
        bundle,
        payloads,
        wal_relative_path,
        lock_relative_path,
        tx_id,
        WalDurability::default(),
    )
}

/// [`apply_operation_effect_bundle_with_wal_lock`] with explicit durability.
#[must_use]
pub fn apply_operation_effect_bundle_with_wal_lock_and_durability(
    effect_store_root: impl AsRef<Path>,
    bundle: &OperationEffectBundle,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    lock_relative_path: &str,
    tx_id: impl Into<String>,
    durability: WalDurability,
) -> EffectApplicationResult {
    apply_file_effect_transaction_with_wal_lock_with_durability(
        effect_store_root,
        bundle.transaction_effect(),
        payloads,
        wal_relative_path,
        lock_relative_path,
        tx_id,
        durability,
    )
}

fn operation_wide_repair(destructive: bool) -> EffectRepair {
    if destructive {
        EffectRepair {
            strategy: RepairStrategy::CompensateThenRetry,
            automatic_repair_allowed: false,
            inverse_operation_ref: None,
            stop_if_inverse_missing: false,
            inverse: InverseMetadata {
                kind: InverseKind::RestoreSnapshot,
                source: InverseSource::Snapshot,
                reference: Some("effect_wal_before_images".to_owned()),
                input_mapping_refs: Vec::new(),
                validation_gate_refs: Vec::new(),
                review_required: false,
            },
        }
    } else {
        EffectRepair {
            strategy: RepairStrategy::None,
            automatic_repair_allowed: false,
            inverse_operation_ref: None,
            stop_if_inverse_missing: false,
            inverse: InverseMetadata {
                kind: InverseKind::None,
                source: InverseSource::Unavailable,
                reference: None,
                input_mapping_refs: Vec::new(),
                validation_gate_refs: Vec::new(),
                review_required: false,
            },
        }
    }
}

fn file_backed_target(kind: EffectTargetKind) -> bool {
    matches!(
        kind,
        EffectTargetKind::FilePath
            | EffectTargetKind::ArtifactId
            | EffectTargetKind::EvidenceId
            | EffectTargetKind::LedgerStream
            | EffectTargetKind::RequestStream
    )
}

fn normalized_target(
    effect_store_root: &Path,
    kind: EffectTargetKind,
    reference: &str,
) -> Result<String, ()> {
    let physical =
        resolve_effect_physical_ref(effect_store_root, kind, reference).map_err(|_| ())?;
    let normalized = physical
        .0
        .replace('\\', "/")
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase();
    Ok(normalized)
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OperationEffectBundleError {
    MultipleEffectsRequired {
        observed: usize,
    },
    EffectSetMismatch,
    InvalidOperation,
    InvalidEffect {
        effect_id: String,
        reasons: Vec<String>,
    },
    InvalidTransactionEnvelope,
    EffectBindingMismatch {
        effect_id: String,
    },
    DuplicateEffectId(String),
    ActorMismatch {
        effect_id: String,
    },
    UnsupportedTarget {
        effect_id: String,
        reference: String,
    },
    ConflictingRead {
        reference: String,
    },
    OverlappingWrite {
        reference: String,
        first_effect: String,
        second_effect: String,
    },
}

impl fmt::Display for OperationEffectBundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MultipleEffectsRequired { observed } => write!(
                formatter,
                "operation-wide WAL requires at least two effects, found {observed}"
            ),
            Self::EffectSetMismatch => formatter.write_str(
                "loaded effect refs must exactly match the operation-declared ordered effect set",
            ),
            Self::InvalidOperation => {
                formatter.write_str("operation failed semantic validation")
            }
            Self::InvalidEffect { effect_id, reasons } => write!(
                formatter,
                "effect {effect_id} failed semantic validation: {}",
                reasons.join("; ")
            ),
            Self::InvalidTransactionEnvelope => {
                formatter.write_str("derived operation-wide transaction envelope is invalid")
            }
            Self::EffectBindingMismatch { effect_id } => write!(
                formatter,
                "effect {effect_id} is not bound to the declared operation"
            ),
            Self::DuplicateEffectId(effect_id) => {
                write!(formatter, "duplicate operation effect id {effect_id}")
            }
            Self::ActorMismatch { effect_id } => {
                write!(formatter, "effect {effect_id} uses a different actor")
            }
            Self::UnsupportedTarget {
                effect_id,
                reference,
            } => write!(
                formatter,
                "effect {effect_id} target {reference} is not file-backed"
            ),
            Self::ConflictingRead { reference } => {
                write!(formatter, "operation effects declare conflicting reads for {reference}")
            }
            Self::OverlappingWrite {
                reference,
                first_effect,
                second_effect,
            } => write!(
                formatter,
                "operation effects {first_effect} and {second_effect} overlap write target {reference}"
            ),
        }
    }
}

impl std::error::Error for OperationEffectBundleError {}
