//! Durable, Store-owned project reinitialization protocol.
//!
//! # Retained authority
//!
//! The public protocol and its concrete [`RetainedProjectLinkCas`] are exported
//! from `lib.rs`. Production mutation retains the consumer root, exact
//! predecessor, destination state root, and operation-scoped Store lock across
//! validation and descriptor-relative Project Link replacement.
//!
//! Candidate material is data only.  This protocol never accepts a candidate
//! as signing, trust, admission, activation, lifecycle, host-selection, or
//! private-key authority; `selected_host` is permanently `None` in all durable
//! records.

use crate::{
    retained_dir::{RetainedDirectory, RetainedFileAnchorBinding, RetainedLeafPolicy},
    sha256_content_hash, EffectStoreLock, RetainedEffectStoreRoot,
};
use forge_core_contracts::{
    BootstrapStateLossDiagnostic, ProjectLinkDocument, RepoPath, StableId,
    PROJECT_LINK_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{self, Read as _};
use std::path::{Component, Path, PathBuf};

const PLAN_SCHEMA: &str = "forge_project_reinitialize_plan_v1";
const WAL_SCHEMA: &str = "forge_project_reinitialize_wal_v1";
const RECEIPT_SCHEMA: &str = "forge_project_reinitialize_receipt_v1";
const CONFIRMATION_DOMAIN: &[u8] = b"forge-method:project-reinitialize-confirmation:v1\0";
const PLAN_DOMAIN: &[u8] = b"forge-method:project-reinitialize-plan:v1\0";
const RECEIPT_DOMAIN: &[u8] = b"forge-method:project-reinitialize-receipt:v1\0";
const RESERVATION_SCHEMA: &str = "forge_project_reinitialize_reservation_v1";
const RESERVATION_MARKER: &str = ".forge-reinitialize-reservation.json";
const STATE_ROOT_NAME: &str = ".forge-method";
const PROJECT_LINK_NAME: &str = ".forge-method.yaml";
const PROJECT_LINK_ANCHORS: &str = "project-reinitialize/project-link-anchors";
const MAX_AUTHORITY_DOCUMENT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PROJECT_LINK_BYTES: u64 = 64 * 1024;
const INITIAL_STATE_DIRECTORIES: &[&str] = &[
    "artifacts",
    "claims-active",
    "evidence",
    "handoffs",
    "handoffs/expired-claims",
    "index",
    "locks",
    "traces",
    "wal",
];

/// Relative names used beneath each operation's Store-owned effect-lock parent.
pub const REINITIALIZE_LOCK_ROOT: &str = "locks/project-reinitialize";
pub const REINITIALIZE_PLAN_FILE: &str = "plan.json";
pub const REINITIALIZE_WAL_PREFIX: &str = "wal";
pub const REINITIALIZE_RECEIPT_FILE: &str = "receipt.json";

/// Immutable, portable binding for a checked authority leaf.
///
/// Raw inode, device, volume, and file-index values are deliberately excluded:
/// persisted values must not become reusable filesystem authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeAuthorityBinding {
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
    pub anchor_nonce: String,
}

/// Exact diagnosis observed at planning time.  The caller supplies only
/// digest-addressed facts; the Store reopens and rechecks them before apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeDiagnosis {
    pub diagnosis_path: String,
    pub diagnosis_sha256: String,
    pub diagnostic: BootstrapStateLossDiagnostic,
}

/// Request for a single immutable reinitialization plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinitializePlanRequest {
    pub operation_id: String,
    pub project_root: PathBuf,
    pub destination: PathBuf,
    pub destination_state_root: PathBuf,
    pub diagnosis: ReinitializeDiagnosis,
    pub expected_project_link: ReinitializeAuthorityBinding,
    pub successor_project_link: Vec<u8>,
    pub predecessor_identity: String,
    pub successor_project_id: String,
    pub successor_identity: String,
}

/// Exact successor Project Link bytes and their destination-sidecar layout.
/// The bytes are data until the retained CAS installs them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeSuccessorProjectLink {
    pub path: String,
    pub bytes: String,
    pub sha256: String,
    pub byte_length: u64,
    pub sidecar_root: String,
    pub state_root: String,
}

/// Persisted Store-owned exact-file anchor binding. Platform object identifiers
/// are deliberately absent; the private anchor keeps the predecessor alive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeFileAnchorBinding {
    pub schema_version: String,
    pub anchor_relative_path: String,
    pub nonce: String,
    pub content_digest: String,
    pub byte_length: u64,
}

/// Reservation plus exact predecessor anchor minted beneath the new state root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinitializeDestinationReservation {
    pub destination: ReinitializeAuthorityBinding,
    pub project_link_anchor: ReinitializeFileAnchorBinding,
}

/// Durable plan.  `selected_host` is intentionally fixed to `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinitializePlan {
    pub schema_version: String,
    pub operation_id: String,
    pub project_root: String,
    pub destination: String,
    pub destination_state_root: String,
    pub diagnosis: ReinitializeDiagnosis,
    pub expected_project_link: ReinitializeAuthorityBinding,
    pub successor_project_link: ReinitializeSuccessorProjectLink,
    pub predecessor_identity: String,
    pub successor_project_id: String,
    pub successor_identity: String,
    pub selected_host: Option<String>,
    pub plan_digest: String,
    pub confirmation_token: String,
}

/// Durable progression.  States only ever advance; recovery cannot move an
/// installed link back to a predecessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReinitializeWalPhase {
    Reserved,
    ApplyPrepared,
    LinkInstalled,
    ReceiptPublished,
}

/// Immutable WAL record.  Every state binds the same plan digest, reservation,
/// predecessor, and successor, which makes an unrelated sidecar unusable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeWalRecord {
    pub schema_version: String,
    pub operation_id: String,
    pub plan_digest: String,
    pub phase: ReinitializeWalPhase,
    pub destination_reservation: ReinitializeAuthorityBinding,
    pub project_link_anchor: ReinitializeFileAnchorBinding,
    pub predecessor_identity: String,
    pub successor_project_id: String,
    pub successor_identity: String,
    pub project_link: ReinitializeAuthorityBinding,
}

/// Final immutable receipt and success linearization record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinitializeReceipt {
    pub schema_version: String,
    pub operation_id: String,
    pub plan_digest: String,
    pub predecessor_identity: String,
    pub successor_project_id: String,
    pub successor_identity: String,
    pub destination: String,
    pub destination_reservation: ReinitializeAuthorityBinding,
    pub project_link_anchor: ReinitializeFileAnchorBinding,
    pub project_link_before: ReinitializeAuthorityBinding,
    pub project_link_after: ReinitializeAuthorityBinding,
    pub selected_host: Option<String>,
    pub receipt_digest: String,
}

/// Store integration boundary.  Its opaque implementation must retain
/// descriptor handles across validation and publication, hold the Store lock,
/// reject symlinks/reparse points/hard links per platform support, and make
/// immutable writes byte-identical on retry.
///
/// `reserve_empty_destination` must atomically claim a previously unrelated
/// empty destination by no-replace publication.  It must reject preexisting,
/// linked, reparse-point, non-directory, or non-empty inputs.  `install_link`
/// is an exact expected-handle CAS and must never replace an unverified name.
pub trait ProjectLinkCas {
    type Error: std::error::Error + Send + Sync + 'static;

    fn with_reinitialize_lock<T>(
        &mut self,
        operation_id: &str,
        action: impl FnOnce(&mut Self) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;

    /// Construct a fail-closed protocol error without losing the Store's
    /// concrete error type.
    fn reject(&self, reason: &'static str) -> Self::Error;

    fn write_immutable(&mut self, relative: &Path, bytes: &[u8]) -> Result<(), Self::Error>;
    fn read_immutable(&mut self, relative: &Path) -> Result<Option<Vec<u8>>, Self::Error>;
    fn reserve_empty_destination(
        &mut self,
        destination: &Path,
        operation_id: &str,
    ) -> Result<ReinitializeDestinationReservation, Self::Error>;
    fn revalidate_diagnosis(
        &mut self,
        diagnosis: &ReinitializeDiagnosis,
    ) -> Result<(), Self::Error>;
    fn revalidate_project_link(
        &mut self,
        expected: &ReinitializeAuthorityBinding,
        anchor: &ReinitializeFileAnchorBinding,
    ) -> Result<(), Self::Error>;
    fn install_link(
        &mut self,
        expected: &ReinitializeAuthorityBinding,
        anchor: &ReinitializeFileAnchorBinding,
        successor: &ReinitializeSuccessorProjectLink,
    ) -> Result<ReinitializeAuthorityBinding, Self::Error>;
    fn project_link_is_successor(
        &mut self,
        successor: &ReinitializeSuccessorProjectLink,
    ) -> Result<Option<ReinitializeAuthorityBinding>, Self::Error>;
}

#[derive(Debug)]
pub enum ReinitializeError<E> {
    InvalidInput(&'static str),
    Serialization(String),
    Collision { relative: PathBuf },
    ConfirmationMismatch,
    PlanMismatch,
    RecoveryConflict(&'static str),
    Store(E),
}

impl<E: fmt::Display> fmt::Display for ReinitializeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(reason) => write!(f, "invalid reinitialize input: {reason}"),
            Self::Serialization(reason) => write!(f, "serialize reinitialize authority: {reason}"),
            Self::Collision { relative } => write!(
                f,
                "immutable reinitialize collision at {}",
                relative.display()
            ),
            Self::ConfirmationMismatch => write!(
                f,
                "reinitialize confirmation does not exactly match the plan"
            ),
            Self::PlanMismatch => write!(
                f,
                "existing operation id binds different reinitialize material"
            ),
            Self::RecoveryConflict(reason) => write!(
                f,
                "reinitialize recovery requires operator investigation: {reason}"
            ),
            Self::Store(error) => write!(f, "reinitialize Store operation failed: {error}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ReinitializeError<E> {}

fn map_validation<E>(error: ReinitializeError<std::convert::Infallible>) -> ReinitializeError<E> {
    match error {
        ReinitializeError::InvalidInput(reason) => ReinitializeError::InvalidInput(reason),
        ReinitializeError::Serialization(reason) => ReinitializeError::Serialization(reason),
        ReinitializeError::Collision { relative } => ReinitializeError::Collision { relative },
        ReinitializeError::ConfirmationMismatch => ReinitializeError::ConfirmationMismatch,
        ReinitializeError::PlanMismatch => ReinitializeError::PlanMismatch,
        ReinitializeError::RecoveryConflict(reason) => ReinitializeError::RecoveryConflict(reason),
        ReinitializeError::Store(never) => match never {},
    }
}

/// Build an immutable plan.  Confirmation is derived from the canonical plan
/// material and is never accepted as a partial phrase or a mutable UI label.
pub fn plan(
    request: ReinitializePlanRequest,
) -> Result<ReinitializePlan, ReinitializeError<std::convert::Infallible>> {
    validate_request(&request)?;
    let mut plan = ReinitializePlan {
        schema_version: PLAN_SCHEMA.to_owned(),
        operation_id: request.operation_id,
        project_root: normalized_absolute_path(&request.project_root)?,
        destination: normalized_absolute_path(&request.destination)?,
        destination_state_root: normalized_absolute_path(&request.destination_state_root)?,
        diagnosis: request.diagnosis,
        expected_project_link: request.expected_project_link,
        successor_project_link: successor_project_link(
            &request.successor_project_link,
            &request.project_root,
            &request.destination,
            &request.destination_state_root,
            &request.successor_project_id,
        )?,
        predecessor_identity: request.predecessor_identity,
        successor_project_id: request.successor_project_id,
        successor_identity: request.successor_identity,
        selected_host: None,
        plan_digest: String::new(),
        confirmation_token: String::new(),
    };
    plan.plan_digest = digest_canonical(PLAN_DOMAIN, &plan)?;
    plan.confirmation_token = confirmation_for(&plan.plan_digest);
    Ok(plan)
}

/// Apply or exactly resume an immutable plan.  A receipt is returned only
/// after immutable publication.  An exact retry returns that same receipt;
/// divergent material under the same operation id fails closed.
pub fn apply<S: ProjectLinkCas>(
    store: &mut S,
    plan: &ReinitializePlan,
    confirmation: &str,
) -> Result<ReinitializeReceipt, ReinitializeError<S::Error>> {
    validate_plan(plan).map_err(map_validation)?;
    if confirmation != plan.confirmation_token {
        return Err(ReinitializeError::ConfirmationMismatch);
    }
    let operation_id = plan.operation_id.clone();
    store
        .with_reinitialize_lock(&operation_id, |store| apply_locked(store, plan))
        .map_err(ReinitializeError::Store)
}

fn apply_locked<S: ProjectLinkCas>(
    store: &mut S,
    plan: &ReinitializePlan,
) -> Result<ReinitializeReceipt, S::Error> {
    let plan_path = plan_path(&plan.operation_id);
    let plan_bytes = canonical_bytes(plan).expect("validated plan serialization");
    match store.read_immutable(&plan_path)? {
        Some(existing) if existing != plan_bytes => return Err(store.reject("plan collision")),
        Some(_) => {}
        None => store.write_immutable(&plan_path, &plan_bytes)?,
    }

    if let Some(receipt) = load_receipt(store, plan)? {
        match store.project_link_is_successor(&plan.successor_project_link)? {
            Some(binding) if binding == receipt.project_link_after => return Ok(receipt),
            Some(_) => return Err(store.reject("receipt successor binding was substituted")),
            None => return Err(store.reject("receipt successor is absent")),
        }
    }

    // Recovery accepts only a marker that binds this exact plan.  It never
    // restores the predecessor: an installed successor is completed instead.
    let mut wal = load_wal(store, plan)?;
    if wal.is_none() {
        let reservation =
            store.reserve_empty_destination(Path::new(&plan.destination), &plan.operation_id)?;
        let record = wal_record(plan, ReinitializeWalPhase::Reserved, reservation);
        write_wal(store, &record)?;
        wal = Some(record);
    }
    let wal = wal.expect("record initialized");

    let installed = match wal.phase {
        ReinitializeWalPhase::Reserved => install_successor(store, plan, &wal)?,
        // A crash between the link CAS and the next immutable WAL publication
        // is resolved by observing the exact successor.  If it is absent, the
        // expected predecessor must still validate before the CAS is retried.
        ReinitializeWalPhase::ApplyPrepared => {
            if let Some(binding) = store.project_link_is_successor(&plan.successor_project_link)? {
                write_wal(
                    store,
                    &ReinitializeWalRecord {
                        phase: ReinitializeWalPhase::LinkInstalled,
                        project_link: binding.clone(),
                        ..wal.clone()
                    },
                )?;
                binding
            } else {
                install_successor(store, plan, &wal)?
            }
        }
        ReinitializeWalPhase::LinkInstalled | ReinitializeWalPhase::ReceiptPublished => match store
            .project_link_is_successor(&plan.successor_project_link)?
        {
            Some(binding) => binding,
            None => {
                return Err(store
                    .reject("installed successor is absent; predecessor restoration is forbidden"))
            }
        },
    };

    let receipt = receipt_for(
        plan,
        &wal.destination_reservation,
        &wal.project_link_anchor,
        &installed,
    )
    .expect("validated receipt serialization");
    let receipt_path = receipt_path(&plan.operation_id);
    let bytes = canonical_bytes(&receipt).expect("validated receipt serialization");
    store.write_immutable(&receipt_path, &bytes)?;
    write_wal(
        store,
        &ReinitializeWalRecord {
            phase: ReinitializeWalPhase::ReceiptPublished,
            project_link: installed,
            ..wal
        },
    )?;
    Ok(receipt)
}

fn install_successor<S: ProjectLinkCas>(
    store: &mut S,
    plan: &ReinitializePlan,
    wal: &ReinitializeWalRecord,
) -> Result<ReinitializeAuthorityBinding, S::Error> {
    // The order is deliberate: all mutable facts are checked immediately
    // before CAS, under the same Store lock that owns the reservation/WAL.
    store.revalidate_diagnosis(&plan.diagnosis)?;
    store.revalidate_project_link(&plan.expected_project_link, &wal.project_link_anchor)?;
    let prepared = ReinitializeWalRecord {
        phase: ReinitializeWalPhase::ApplyPrepared,
        ..wal.clone()
    };
    write_wal(store, &prepared)?;
    let after = store.install_link(
        &plan.expected_project_link,
        &wal.project_link_anchor,
        &plan.successor_project_link,
    )?;
    write_wal(
        store,
        &ReinitializeWalRecord {
            phase: ReinitializeWalPhase::LinkInstalled,
            project_link: after.clone(),
            ..prepared
        },
    )?;
    Ok(after)
}

fn load_receipt<S: ProjectLinkCas>(
    store: &mut S,
    plan: &ReinitializePlan,
) -> Result<Option<ReinitializeReceipt>, S::Error> {
    let Some(bytes) = store.read_immutable(&receipt_path(&plan.operation_id))? else {
        return Ok(None);
    };
    let receipt: ReinitializeReceipt =
        serde_json::from_slice(&bytes).map_err(|_| store.reject("receipt is not valid JSON"))?;
    if receipt.schema_version != RECEIPT_SCHEMA
        || receipt.plan_digest != plan.plan_digest
        || receipt.operation_id != plan.operation_id
        || receipt.selected_host.is_some()
        || receipt.predecessor_identity != plan.predecessor_identity
        || receipt.successor_project_id != plan.successor_project_id
        || receipt.successor_identity != plan.successor_identity
        || receipt.destination != plan.destination
        || !reservation_binds_plan(&receipt.destination_reservation, plan)
        || receipt.project_link_before != plan.expected_project_link
        || !anchor_binds_predecessor(&receipt.project_link_anchor, plan)
        || !binding_binds_successor(&receipt.project_link_after, plan)
    {
        return Err(store.reject("receipt does not bind this immutable plan"));
    }
    let receipt_digest = digest_canonical(
        RECEIPT_DOMAIN,
        &ReinitializeReceipt {
            receipt_digest: String::new(),
            ..receipt.clone()
        },
    )
    .map_err(|_| store.reject("receipt cannot be canonically verified"))?;
    if receipt_digest != receipt.receipt_digest {
        return Err(store.reject("receipt digest differs from immutable receipt content"));
    }
    Ok(Some(receipt))
}

fn load_wal<S: ProjectLinkCas>(
    store: &mut S,
    plan: &ReinitializePlan,
) -> Result<Option<ReinitializeWalRecord>, S::Error> {
    // Phase-addressed, immutable records are read latest-first.  This means a
    // crash after any fsync resumes from its exact point without replacing a
    // marker or attempting predecessor restoration.
    for phase in [
        ReinitializeWalPhase::ReceiptPublished,
        ReinitializeWalPhase::LinkInstalled,
        ReinitializeWalPhase::ApplyPrepared,
        ReinitializeWalPhase::Reserved,
    ] {
        let Some(bytes) = store.read_immutable(&wal_path_phase(&plan.operation_id, phase))? else {
            continue;
        };
        let record: ReinitializeWalRecord = serde_json::from_slice(&bytes)
            .map_err(|_| store.reject("WAL record is not valid JSON"))?;
        if record.schema_version != WAL_SCHEMA
            || record.phase != phase
            || record.plan_digest != plan.plan_digest
            || record.operation_id != plan.operation_id
            || record.predecessor_identity != plan.predecessor_identity
            || record.successor_project_id != plan.successor_project_id
            || record.successor_identity != plan.successor_identity
            || !reservation_binds_plan(&record.destination_reservation, plan)
            || !anchor_binds_predecessor(&record.project_link_anchor, plan)
            || match phase {
                ReinitializeWalPhase::Reserved | ReinitializeWalPhase::ApplyPrepared => {
                    record.project_link != plan.expected_project_link
                }
                ReinitializeWalPhase::LinkInstalled | ReinitializeWalPhase::ReceiptPublished => {
                    !binding_binds_successor(&record.project_link, plan)
                }
            }
        {
            return Err(store.reject("WAL record does not bind this immutable plan"));
        }
        return Ok(Some(record));
    }
    Ok(None)
}

fn write_wal<S: ProjectLinkCas>(
    store: &mut S,
    record: &ReinitializeWalRecord,
) -> Result<(), S::Error> {
    // Each phase is a separate immutable record; the path is phase-addressed,
    // avoiding overwrite even under interruption.
    store.write_immutable(
        &wal_path_phase(&record.operation_id, record.phase),
        &canonical_bytes(record).expect("WAL serializable"),
    )
}

fn wal_record(
    plan: &ReinitializePlan,
    phase: ReinitializeWalPhase,
    reservation: ReinitializeDestinationReservation,
) -> ReinitializeWalRecord {
    ReinitializeWalRecord {
        schema_version: WAL_SCHEMA.to_owned(),
        operation_id: plan.operation_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        phase,
        destination_reservation: reservation.destination,
        project_link_anchor: reservation.project_link_anchor,
        predecessor_identity: plan.predecessor_identity.clone(),
        successor_project_id: plan.successor_project_id.clone(),
        successor_identity: plan.successor_identity.clone(),
        project_link: plan.expected_project_link.clone(),
    }
}

fn receipt_for(
    plan: &ReinitializePlan,
    reservation: &ReinitializeAuthorityBinding,
    project_link_anchor: &ReinitializeFileAnchorBinding,
    after: &ReinitializeAuthorityBinding,
) -> Result<ReinitializeReceipt, ReinitializeError<std::convert::Infallible>> {
    let mut receipt = ReinitializeReceipt {
        schema_version: RECEIPT_SCHEMA.to_owned(),
        operation_id: plan.operation_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        predecessor_identity: plan.predecessor_identity.clone(),
        successor_project_id: plan.successor_project_id.clone(),
        successor_identity: plan.successor_identity.clone(),
        destination: plan.destination.clone(),
        destination_reservation: reservation.clone(),
        project_link_anchor: project_link_anchor.clone(),
        project_link_before: plan.expected_project_link.clone(),
        project_link_after: after.clone(),
        selected_host: None,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = digest_canonical(RECEIPT_DOMAIN, &receipt)?;
    Ok(receipt)
}

fn reservation_binds_plan(
    reservation: &ReinitializeAuthorityBinding,
    plan: &ReinitializePlan,
) -> bool {
    reservation.path == plan.destination
        && is_sha256_digest(&reservation.sha256)
        && reservation.byte_length > 0
        && reservation.byte_length <= MAX_AUTHORITY_DOCUMENT_BYTES
        && reservation.anchor_nonce == plan.operation_id
}

fn anchor_binds_predecessor(
    anchor: &ReinitializeFileAnchorBinding,
    plan: &ReinitializePlan,
) -> bool {
    !anchor.schema_version.is_empty()
        && !anchor.anchor_relative_path.is_empty()
        && !anchor.nonce.is_empty()
        && anchor.content_digest == plan.expected_project_link.sha256
        && anchor.byte_length == plan.expected_project_link.byte_length
}

fn binding_binds_successor(
    binding: &ReinitializeAuthorityBinding,
    plan: &ReinitializePlan,
) -> bool {
    binding.path == PROJECT_LINK_NAME
        && binding.sha256 == plan.successor_project_link.sha256
        && binding.byte_length == plan.successor_project_link.byte_length
        && binding.anchor_nonce == plan.operation_id
}

fn plan_path(_operation: &str) -> PathBuf {
    PathBuf::from(REINITIALIZE_PLAN_FILE)
}
fn wal_path_phase(_operation: &str, phase: ReinitializeWalPhase) -> PathBuf {
    PathBuf::from(format!("{REINITIALIZE_WAL_PREFIX}-{phase:?}.json").to_ascii_lowercase())
}
fn receipt_path(_operation: &str) -> PathBuf {
    PathBuf::from(REINITIALIZE_RECEIPT_FILE)
}

fn confirmation_for(plan_digest: &str) -> String {
    digest_bytes(CONFIRMATION_DOMAIN, plan_digest.as_bytes())
}
fn digest_bytes(domain: &[u8], value: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(domain);
    h.update(value);
    format!("sha256:{:x}", h.finalize())
}
fn digest_canonical<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<String, ReinitializeError<std::convert::Infallible>> {
    canonical_bytes(value).map(|bytes| digest_bytes(domain, &bytes))
}
fn canonical_bytes<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, ReinitializeError<std::convert::Infallible>> {
    serde_json_canonicalizer::to_vec(value)
        .map_err(|e| ReinitializeError::Serialization(e.to_string()))
}

fn validate_request(
    request: &ReinitializePlanRequest,
) -> Result<(), ReinitializeError<std::convert::Infallible>> {
    validate_operation(&request.operation_id)?;
    let project_root = normalized_absolute_path(&request.project_root)?;
    let destination = normalized_absolute_path(&request.destination)?;
    let destination_state_root = normalized_absolute_path(&request.destination_state_root)?;
    if Path::new(&destination_state_root) != Path::new(&destination).join(STATE_ROOT_NAME) {
        return Err(ReinitializeError::InvalidInput(
            "destination state root must be the exact .forge-method child",
        ));
    }
    let project_root_path = Path::new(&project_root);
    let destination_path = Path::new(&destination);
    if project_root_path == destination_path
        || project_root_path.starts_with(destination_path)
        || destination_path.starts_with(project_root_path)
    {
        return Err(ReinitializeError::InvalidInput(
            "destination must be unrelated to the consumer project root",
        ));
    }
    validate_diagnosis(&request.diagnosis)?;
    validate_project_link_binding(&request.expected_project_link)?;
    if request.expected_project_link.anchor_nonce != request.operation_id {
        return Err(ReinitializeError::InvalidInput(
            "predecessor binding nonce must equal the operation id",
        ));
    }
    if request.diagnosis.diagnostic.project_link_sha256.as_deref()
        != request.expected_project_link.sha256.strip_prefix("sha256:")
    {
        return Err(ReinitializeError::InvalidInput(
            "diagnosis does not bind the exact predecessor Project Link",
        ));
    }
    if request.successor_project_link.is_empty()
        || u64::try_from(request.successor_project_link.len()).unwrap_or(u64::MAX)
            > MAX_PROJECT_LINK_BYTES
        || std::str::from_utf8(&request.successor_project_link).is_err()
    {
        return Err(ReinitializeError::InvalidInput(
            "successor Project Link must be nonempty bounded UTF-8",
        ));
    }
    for identity in [
        request.predecessor_identity.as_str(),
        request.successor_project_id.as_str(),
        request.successor_identity.as_str(),
    ] {
        validate_identity(identity)?;
    }
    if request.predecessor_identity == request.successor_identity {
        return Err(ReinitializeError::InvalidInput(
            "authority identities must be distinct",
        ));
    }
    if request.successor_project_id == request.diagnosis.diagnostic.project_id {
        return Err(ReinitializeError::InvalidInput(
            "successor project identity must differ from the abandoned project",
        ));
    }
    Ok(())
}
fn validate_plan(
    plan: &ReinitializePlan,
) -> Result<(), ReinitializeError<std::convert::Infallible>> {
    if plan.schema_version != PLAN_SCHEMA
        || plan.selected_host.is_some()
        || !is_sha256_digest(&plan.plan_digest)
        || !is_sha256_digest(&plan.confirmation_token)
        || plan.confirmation_token != confirmation_for(&plan.plan_digest)
    {
        return Err(ReinitializeError::InvalidInput("invalid sealed plan"));
    }
    let computed = digest_canonical(
        PLAN_DOMAIN,
        &ReinitializePlan {
            plan_digest: String::new(),
            confirmation_token: String::new(),
            ..plan.clone()
        },
    )?;
    if computed != plan.plan_digest {
        return Err(ReinitializeError::PlanMismatch);
    }
    let request = ReinitializePlanRequest {
        operation_id: plan.operation_id.clone(),
        project_root: PathBuf::from(&plan.project_root),
        destination: PathBuf::from(&plan.destination),
        destination_state_root: PathBuf::from(&plan.destination_state_root),
        diagnosis: plan.diagnosis.clone(),
        expected_project_link: plan.expected_project_link.clone(),
        successor_project_link: plan.successor_project_link.bytes.as_bytes().to_vec(),
        predecessor_identity: plan.predecessor_identity.clone(),
        successor_project_id: plan.successor_project_id.clone(),
        successor_identity: plan.successor_identity.clone(),
    };
    validate_request(&request)?;
    if successor_project_link(
        &request.successor_project_link,
        &request.project_root,
        &request.destination,
        &request.destination_state_root,
        &request.successor_project_id,
    )? != plan.successor_project_link
    {
        return Err(ReinitializeError::PlanMismatch);
    }
    Ok(())
}

fn validate_diagnosis(
    diagnosis: &ReinitializeDiagnosis,
) -> Result<(), ReinitializeError<std::convert::Infallible>> {
    normalized_absolute_path(Path::new(&diagnosis.diagnosis_path))?;
    if !is_sha256_digest(&diagnosis.diagnosis_sha256)
        || diagnosis.diagnostic.validate().is_err()
        || diagnosis.diagnostic.project_link_sha256.is_none()
    {
        return Err(ReinitializeError::InvalidInput(
            "diagnosis binding is incomplete or invalid",
        ));
    }
    Ok(())
}

fn validate_project_link_binding(
    binding: &ReinitializeAuthorityBinding,
) -> Result<(), ReinitializeError<std::convert::Infallible>> {
    validate_relative_path(Path::new(&binding.path))?;
    if binding.path != PROJECT_LINK_NAME
        || !is_sha256_digest(&binding.sha256)
        || binding.byte_length == 0
        || binding.byte_length > MAX_PROJECT_LINK_BYTES
        || binding.anchor_nonce.is_empty()
    {
        return Err(ReinitializeError::InvalidInput(
            "project-link binding is incomplete or non-canonical",
        ));
    }
    Ok(())
}

fn validate_operation(value: &str) -> Result<(), ReinitializeError<std::convert::Infallible>> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Err(ReinitializeError::InvalidInput(
            "operation id must be a safe component",
        ))
    } else {
        Ok(())
    }
}
fn validate_identity(value: &str) -> Result<(), ReinitializeError<std::convert::Infallible>> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(ReinitializeError::InvalidInput(
            "identity must be a bounded safe value",
        ));
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<(), ReinitializeError<std::convert::Infallible>> {
    if path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ReinitializeError::InvalidInput(
            "path must be a nonempty normalized relative path",
        ));
    }
    Ok(())
}

fn normalized_absolute_path(
    path: &Path,
) -> Result<String, ReinitializeError<std::convert::Infallible>> {
    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(ReinitializeError::InvalidInput(
            "path must be a normalized absolute path",
        ));
    }
    path.to_str()
        .map(str::to_owned)
        .ok_or(ReinitializeError::InvalidInput("path must be UTF-8"))
}

fn successor_project_link(
    bytes: &[u8],
    project_root: &Path,
    destination: &Path,
    destination_state_root: &Path,
    successor_project_id: &str,
) -> Result<ReinitializeSuccessorProjectLink, ReinitializeError<std::convert::Infallible>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ReinitializeError::InvalidInput("successor Project Link must be UTF-8"))?;
    if text.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_PROJECT_LINK_BYTES {
        return Err(ReinitializeError::InvalidInput(
            "successor Project Link must be nonempty and bounded",
        ));
    }
    let document: ProjectLinkDocument = yaml_serde::from_str(text).map_err(|_| {
        ReinitializeError::InvalidInput("successor Project Link must be closed valid YAML")
    })?;
    let destination = normalized_absolute_path(destination)?;
    let destination_state_root = normalized_absolute_path(destination_state_root)?;
    if document.schema_version != PROJECT_LINK_SCHEMA_VERSION
        || document.project_id.0 != successor_project_id
        || resolve_project_link_path(project_root, &document.sidecar_root.0)? != destination
        || resolve_project_link_path(project_root, &document.state_root.0)?
            != destination_state_root
    {
        return Err(ReinitializeError::InvalidInput(
            "successor Project Link does not bind the sealed identity and layout",
        ));
    }
    Ok(ReinitializeSuccessorProjectLink {
        path: PROJECT_LINK_NAME.to_owned(),
        bytes: text.to_owned(),
        sha256: sha256_content_hash(bytes),
        byte_length: u64::try_from(bytes.len())
            .map_err(|_| ReinitializeError::InvalidInput("successor Project Link is too large"))?,
        sidecar_root: destination,
        state_root: destination_state_root,
    })
}

fn resolve_project_link_path(
    project_root: &Path,
    value: &str,
) -> Result<String, ReinitializeError<std::convert::Infallible>> {
    let value = Path::new(value);
    let resolved = if value.is_absolute() {
        value.to_path_buf()
    } else {
        project_root.join(value)
    };
    normalized_absolute_path(&resolved)
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReinitializeReservationMarker {
    schema_version: String,
    operation_id: String,
    plan_digest: String,
    project_root: String,
    destination: String,
    destination_state_root: String,
}

/// Error returned by the retained Project Link authority adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedProjectLinkCasError {
    message: String,
}

impl RetainedProjectLinkCasError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn context(context: &str, error: impl fmt::Display) -> Self {
        Self::new(format!("{context}: {error}"))
    }
}

impl fmt::Display for RetainedProjectLinkCasError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RetainedProjectLinkCasError {}

/// Mint one unguessable operation identifier suitable for immutable Store paths.
pub fn mint_reinitialize_operation_id() -> Result<String, RetainedProjectLinkCasError> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).map_err(|error| {
        RetainedProjectLinkCasError::context("mint reinitialize operation id", error)
    })?;
    let mut encoded = String::with_capacity(32);
    use std::fmt::Write as _;
    for byte in nonce {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(format!("reinitialize-{encoded}"))
}

/// Capture the exact predecessor Project Link and state-loss diagnosis and build
/// a request for one sealed reinitialize-as-new plan. This function is read-only.
pub fn capture_reinitialize_plan_request(
    operation_id: String,
    project_root: &Path,
    destination: &Path,
    diagnosis_path: &Path,
    predecessor_identity: String,
    successor_project_id: String,
    successor_identity: String,
) -> Result<ReinitializePlanRequest, RetainedProjectLinkCasError> {
    let project_root_text = normalized_absolute_path(project_root)
        .map_err(|error| RetainedProjectLinkCasError::new(error.to_string()))?;
    let destination_text = normalized_absolute_path(destination)
        .map_err(|error| RetainedProjectLinkCasError::new(error.to_string()))?;
    let diagnosis_path_text = normalized_absolute_path(diagnosis_path)
        .map_err(|error| RetainedProjectLinkCasError::new(error.to_string()))?;
    let destination_state_root = Path::new(&destination_text).join(STATE_ROOT_NAME);

    let project_root_retained = open_absolute_directory_nofollow(Path::new(&project_root_text))?;
    let project_link_bytes = project_root_retained
        .read_authority_bounded(Path::new(PROJECT_LINK_NAME), MAX_PROJECT_LINK_BYTES)
        .map_err(|error| {
            RetainedProjectLinkCasError::context("read exact predecessor Project Link", error)
        })?;
    let predecessor_document: ProjectLinkDocument = yaml_serde::from_slice(&project_link_bytes)
        .map_err(|error| {
            RetainedProjectLinkCasError::context("parse predecessor Project Link", error)
        })?;
    if predecessor_document.schema_version != PROJECT_LINK_SCHEMA_VERSION {
        return Err(RetainedProjectLinkCasError::new(
            "predecessor Project Link has an unsupported schema",
        ));
    }

    let diagnosis_bytes = read_absolute_authority_file(
        Path::new(&diagnosis_path_text),
        MAX_AUTHORITY_DOCUMENT_BYTES,
    )?;
    let diagnostic: BootstrapStateLossDiagnostic = yaml_serde::from_slice(&diagnosis_bytes)
        .map_err(|error| {
            RetainedProjectLinkCasError::context("parse state-loss diagnosis", error)
        })?;
    diagnostic.validate().map_err(|error| {
        RetainedProjectLinkCasError::context("validate state-loss diagnosis", format!("{error:?}"))
    })?;
    let project_link_sha256 = sha256_content_hash(&project_link_bytes);
    if diagnostic.project_id != predecessor_document.project_id.0
        || diagnostic.project_link_sha256.as_deref() != project_link_sha256.strip_prefix("sha256:")
    {
        return Err(RetainedProjectLinkCasError::new(
            "state-loss diagnosis does not bind the exact predecessor Project Link",
        ));
    }

    let successor_document = ProjectLinkDocument {
        schema_version: PROJECT_LINK_SCHEMA_VERSION.to_owned(),
        project_id: StableId(successor_project_id.clone()),
        sidecar_root: RepoPath(destination_text.clone()),
        state_root: RepoPath(
            destination_state_root
                .to_str()
                .ok_or_else(|| RetainedProjectLinkCasError::new("state root must be UTF-8"))?
                .to_owned(),
        ),
    };
    let mut successor_project_link =
        yaml_serde::to_string(&successor_document).map_err(|error| {
            RetainedProjectLinkCasError::context("serialize successor Project Link", error)
        })?;
    if !successor_project_link.ends_with('\n') {
        successor_project_link.push('\n');
    }

    Ok(ReinitializePlanRequest {
        operation_id: operation_id.clone(),
        project_root: PathBuf::from(project_root_text),
        destination: PathBuf::from(destination_text),
        destination_state_root,
        diagnosis: ReinitializeDiagnosis {
            diagnosis_path: diagnosis_path_text,
            diagnosis_sha256: sha256_content_hash(&diagnosis_bytes),
            diagnostic,
        },
        expected_project_link: ReinitializeAuthorityBinding {
            path: PROJECT_LINK_NAME.to_owned(),
            sha256: project_link_sha256,
            byte_length: u64::try_from(project_link_bytes.len())
                .map_err(|_| RetainedProjectLinkCasError::new("Project Link is too large"))?,
            anchor_nonce: operation_id,
        },
        successor_project_link: successor_project_link.into_bytes(),
        predecessor_identity,
        successor_project_id,
        successor_identity,
    })
}

/// Canonical durable encoding written to and read from an explicit plan file.
pub fn encode_reinitialize_plan(
    plan: &ReinitializePlan,
) -> Result<Vec<u8>, RetainedProjectLinkCasError> {
    validate_plan(plan).map_err(|error| RetainedProjectLinkCasError::new(error.to_string()))?;
    canonical_bytes(plan).map_err(|error| RetainedProjectLinkCasError::new(error.to_string()))
}

/// Decode and fully validate one closed durable plan file.
pub fn decode_reinitialize_plan(
    bytes: &[u8],
) -> Result<ReinitializePlan, RetainedProjectLinkCasError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_AUTHORITY_DOCUMENT_BYTES {
        return Err(RetainedProjectLinkCasError::new(
            "reinitialize plan exceeds the byte limit",
        ));
    }
    let plan: ReinitializePlan = serde_json::from_slice(bytes)
        .map_err(|error| RetainedProjectLinkCasError::context("parse reinitialize plan", error))?;
    validate_plan(&plan).map_err(|error| RetainedProjectLinkCasError::new(error.to_string()))?;
    Ok(plan)
}

/// Apply a sealed plan through the concrete retained Store authority adapter.
pub fn apply_retained_reinitialize(
    plan: &ReinitializePlan,
    confirmation: &str,
) -> Result<ReinitializeReceipt, ReinitializeError<RetainedProjectLinkCasError>> {
    let mut store = RetainedProjectLinkCas::open(plan).map_err(ReinitializeError::Store)?;
    apply(&mut store, plan, confirmation)
}

/// Concrete Project Link CAS backed by retained roots, a Store effect lock, a
/// private predecessor lifetime anchor, and descriptor-relative replacement.
pub struct RetainedProjectLinkCas {
    plan: ReinitializePlan,
    project_root: RetainedDirectory,
    destination_root: Option<RetainedDirectory>,
    state_root: Option<RetainedDirectory>,
    lock: Option<EffectStoreLock>,
}

impl fmt::Debug for RetainedProjectLinkCas {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedProjectLinkCas")
            .field("operation_id", &self.plan.operation_id)
            .field("project_root", &self.plan.project_root)
            .field("destination", &self.plan.destination)
            .field("lock_held", &self.lock.is_some())
            .finish_non_exhaustive()
    }
}

impl RetainedProjectLinkCas {
    /// Retain the exact consumer project root without mutating it.
    pub fn open(plan: &ReinitializePlan) -> Result<Self, RetainedProjectLinkCasError> {
        validate_plan(plan).map_err(|error| RetainedProjectLinkCasError::new(error.to_string()))?;
        let project_root = open_absolute_directory_nofollow(Path::new(&plan.project_root))?;
        revalidate_absolute_directory(&project_root, Path::new(&plan.project_root))?;
        Ok(Self {
            plan: plan.clone(),
            project_root,
            destination_root: None,
            state_root: None,
            lock: None,
        })
    }

    fn prepare_for_apply(&mut self) -> Result<(), RetainedProjectLinkCasError> {
        revalidate_absolute_directory(&self.project_root, Path::new(&self.plan.project_root))?;
        let destination_root = prepare_destination_root(&self.plan)?;
        let state_root = destination_root
            .open_directory(Path::new(STATE_ROOT_NAME))
            .map_err(|error| {
                RetainedProjectLinkCasError::context("retain destination state root", error)
            })?;
        revalidate_absolute_directory(&state_root, Path::new(&self.plan.destination_state_root))?;
        let effect_root = RetainedEffectStoreRoot::acquire(&self.plan.destination_state_root)
            .map_err(|error| {
                RetainedProjectLinkCasError::context(
                    "acquire reinitialize effect-store root",
                    error,
                )
            })?;
        let lock_relative_path = format!(
            "{REINITIALIZE_LOCK_ROOT}/{}/effect.lock",
            self.plan.operation_id
        );
        let lock = effect_root
            .acquire_effect_store_lock(&lock_relative_path)
            .map_err(|error| {
                RetainedProjectLinkCasError::context("acquire reinitialize Store lock", error)
            })?;
        self.destination_root = Some(destination_root);
        self.state_root = Some(state_root);
        self.lock = Some(lock);
        Ok(())
    }

    fn lock(&self) -> Result<&EffectStoreLock, RetainedProjectLinkCasError> {
        self.lock
            .as_ref()
            .ok_or_else(|| RetainedProjectLinkCasError::new("reinitialize Store lock is not held"))
    }

    fn state_root(&self) -> Result<&RetainedDirectory, RetainedProjectLinkCasError> {
        self.state_root.as_ref().ok_or_else(|| {
            RetainedProjectLinkCasError::new("destination state root is not retained")
        })
    }

    fn current_project_link(
        &self,
    ) -> Result<(Vec<u8>, crate::retained_dir::RetainedFileIdentity), RetainedProjectLinkCasError>
    {
        revalidate_absolute_directory(&self.project_root, Path::new(&self.plan.project_root))?;
        let file = self
            .project_root
            .open_leaf_read(Path::new(PROJECT_LINK_NAME), RetainedLeafPolicy::Authority)
            .map_err(|error| {
                RetainedProjectLinkCasError::context("retain current Project Link", error)
            })?;
        let identity = RetainedDirectory::identity_of(&file).map_err(|error| {
            RetainedProjectLinkCasError::context("identify current Project Link", error)
        })?;
        let bytes = read_retained_file_bounded(&file, MAX_PROJECT_LINK_BYTES)?;
        self.project_root
            .verify_retained_authority_binding(Path::new(PROJECT_LINK_NAME), &file, &identity)
            .map_err(|error| {
                RetainedProjectLinkCasError::context("revalidate current Project Link", error)
            })?;
        Ok((bytes, identity))
    }
}

impl ProjectLinkCas for RetainedProjectLinkCas {
    type Error = RetainedProjectLinkCasError;

    fn with_reinitialize_lock<T>(
        &mut self,
        operation_id: &str,
        action: impl FnOnce(&mut Self) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error> {
        if operation_id != self.plan.operation_id {
            return Err(self.reject("operation id differs from the retained plan"));
        }
        if self.lock.is_none() {
            self.prepare_for_apply()?;
        }
        self.lock()?
            .validate_retained_lock_file()
            .map_err(|error| RetainedProjectLinkCasError::context("validate Store lock", error))?;
        action(self)
    }

    fn reject(&self, reason: &'static str) -> Self::Error {
        RetainedProjectLinkCasError::new(reason)
    }

    fn write_immutable(&mut self, relative: &Path, bytes: &[u8]) -> Result<(), Self::Error> {
        let io = self.lock()?.retained_store_io().map_err(|error| {
            RetainedProjectLinkCasError::context("retain reinitialize Store I/O", error)
        })?;
        match io.write_new_file_synced(relative, bytes, MAX_AUTHORITY_DOCUMENT_BYTES) {
            Ok(mut witness) => witness.revalidate().map_err(|error| {
                RetainedProjectLinkCasError::context("revalidate immutable Store write", error)
            }),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let mut existing = io
                    .read_optional_bounded(relative, MAX_AUTHORITY_DOCUMENT_BYTES)
                    .map_err(|read_error| {
                        RetainedProjectLinkCasError::context(
                            "read immutable Store collision",
                            read_error,
                        )
                    })?
                    .ok_or_else(|| self.reject("immutable Store collision disappeared"))?;
                existing.revalidate().map_err(|read_error| {
                    RetainedProjectLinkCasError::context(
                        "revalidate immutable Store collision",
                        read_error,
                    )
                })?;
                if existing.raw_bytes() == bytes {
                    Ok(())
                } else {
                    Err(self.reject("immutable Store path binds different bytes"))
                }
            }
            Err(error) => Err(RetainedProjectLinkCasError::context(
                "write immutable reinitialize record",
                error,
            )),
        }
    }

    fn read_immutable(&mut self, relative: &Path) -> Result<Option<Vec<u8>>, Self::Error> {
        let io = self.lock()?.retained_store_io().map_err(|error| {
            RetainedProjectLinkCasError::context("retain reinitialize Store I/O", error)
        })?;
        let Some(mut witness) = io
            .read_optional_bounded(relative, MAX_AUTHORITY_DOCUMENT_BYTES)
            .map_err(|error| {
                RetainedProjectLinkCasError::context("read immutable reinitialize record", error)
            })?
        else {
            return Ok(None);
        };
        witness.revalidate().map_err(|error| {
            RetainedProjectLinkCasError::context("revalidate immutable Store read", error)
        })?;
        Ok(Some(witness.raw_bytes().to_vec()))
    }

    fn reserve_empty_destination(
        &mut self,
        destination: &Path,
        operation_id: &str,
    ) -> Result<ReinitializeDestinationReservation, Self::Error> {
        if destination != Path::new(&self.plan.destination)
            || operation_id != self.plan.operation_id
        {
            return Err(self.reject("destination reservation differs from the retained plan"));
        }
        let destination_root = self
            .destination_root
            .as_ref()
            .ok_or_else(|| self.reject("destination root is not retained"))?;
        revalidate_absolute_directory(destination_root, destination)?;
        let marker_bytes = destination_root
            .read_authority_bounded(Path::new(RESERVATION_MARKER), MAX_AUTHORITY_DOCUMENT_BYTES)
            .map_err(|error| {
                RetainedProjectLinkCasError::context("read destination reservation", error)
            })?;
        let marker: ReinitializeReservationMarker =
            serde_json::from_slice(&marker_bytes).map_err(|error| {
                RetainedProjectLinkCasError::context("parse destination reservation", error)
            })?;
        validate_reservation_marker(&marker, &self.plan)?;

        let (project_link_bytes, project_link_identity) = self.current_project_link()?;
        if sha256_content_hash(&project_link_bytes) != self.plan.expected_project_link.sha256
            || u64::try_from(project_link_bytes.len()).unwrap_or(u64::MAX)
                != self.plan.expected_project_link.byte_length
        {
            return Err(self.reject("predecessor Project Link changed before anchoring"));
        }
        let project_link_file = self
            .project_root
            .open_leaf_read(Path::new(PROJECT_LINK_NAME), RetainedLeafPolicy::Authority)
            .map_err(|error| {
                RetainedProjectLinkCasError::context("retain predecessor for anchoring", error)
            })?;
        if RetainedDirectory::identity_of(&project_link_file).map_err(|error| {
            RetainedProjectLinkCasError::context("identify predecessor for anchoring", error)
        })? != project_link_identity
        {
            return Err(self.reject("predecessor Project Link changed before anchoring"));
        }
        let anchor = self
            .state_root()?
            .retain_file_lifetime_anchor(
                Path::new(PROJECT_LINK_ANCHORS),
                &project_link_file,
                &project_link_identity,
                &self.plan.expected_project_link.sha256,
                self.plan.expected_project_link.byte_length,
            )
            .map_err(|error| {
                RetainedProjectLinkCasError::context("anchor predecessor Project Link", error)
            })?;
        anchor.revalidate().map_err(|error| {
            RetainedProjectLinkCasError::context("revalidate predecessor anchor", error)
        })?;
        Ok(ReinitializeDestinationReservation {
            destination: ReinitializeAuthorityBinding {
                path: self.plan.destination.clone(),
                sha256: sha256_content_hash(&marker_bytes),
                byte_length: u64::try_from(marker_bytes.len()).map_err(|_| {
                    RetainedProjectLinkCasError::new("reservation marker is too large")
                })?,
                anchor_nonce: self.plan.operation_id.clone(),
            },
            project_link_anchor: external_anchor_binding(anchor.binding()),
        })
    }

    fn revalidate_diagnosis(
        &mut self,
        diagnosis: &ReinitializeDiagnosis,
    ) -> Result<(), Self::Error> {
        if diagnosis != &self.plan.diagnosis {
            return Err(self.reject("diagnosis differs from the retained plan"));
        }
        let bytes = read_absolute_authority_file(
            Path::new(&diagnosis.diagnosis_path),
            MAX_AUTHORITY_DOCUMENT_BYTES,
        )?;
        if sha256_content_hash(&bytes) != diagnosis.diagnosis_sha256 {
            return Err(self.reject("state-loss diagnosis bytes changed after planning"));
        }
        let current: BootstrapStateLossDiagnostic =
            yaml_serde::from_slice(&bytes).map_err(|error| {
                RetainedProjectLinkCasError::context("parse fresh state-loss diagnosis", error)
            })?;
        current.validate().map_err(|error| {
            RetainedProjectLinkCasError::context(
                "validate fresh state-loss diagnosis",
                format!("{error:?}"),
            )
        })?;
        if current != diagnosis.diagnostic {
            return Err(self.reject("state-loss diagnosis changed after planning"));
        }
        Ok(())
    }

    fn revalidate_project_link(
        &mut self,
        expected: &ReinitializeAuthorityBinding,
        anchor: &ReinitializeFileAnchorBinding,
    ) -> Result<(), Self::Error> {
        if expected != &self.plan.expected_project_link
            || !anchor_binds_predecessor(anchor, &self.plan)
        {
            return Err(self.reject("predecessor binding differs from the retained plan"));
        }
        let internal = internal_anchor_binding(anchor);
        let lifetime = self
            .state_root()?
            .open_file_lifetime_anchor(&internal)
            .map_err(|error| {
                RetainedProjectLinkCasError::context("open predecessor lifetime anchor", error)
            })?;
        let (file, identity) = lifetime
            .retain_target(&self.project_root, Path::new(PROJECT_LINK_NAME))
            .map_err(|error| {
                RetainedProjectLinkCasError::context("retain anchored predecessor target", error)
            })?;
        let bytes = read_retained_file_bounded(&file, MAX_PROJECT_LINK_BYTES)?;
        if RetainedDirectory::identity_of(&file).map_err(|error| {
            RetainedProjectLinkCasError::context("identify anchored predecessor", error)
        })? != identity
            || sha256_content_hash(&bytes) != expected.sha256
            || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != expected.byte_length
        {
            return Err(self.reject("anchored predecessor Project Link changed"));
        }
        Ok(())
    }

    fn install_link(
        &mut self,
        expected: &ReinitializeAuthorityBinding,
        anchor: &ReinitializeFileAnchorBinding,
        successor: &ReinitializeSuccessorProjectLink,
    ) -> Result<ReinitializeAuthorityBinding, Self::Error> {
        self.revalidate_project_link(expected, anchor)?;
        if successor != &self.plan.successor_project_link {
            return Err(self.reject("successor differs from the retained plan"));
        }
        let internal = internal_anchor_binding(anchor);
        let lifetime = self
            .state_root()?
            .open_file_lifetime_anchor(&internal)
            .map_err(|error| {
                RetainedProjectLinkCasError::context("open predecessor anchor for CAS", error)
            })?;
        let authority = self.project_root.retain_authority().map_err(|error| {
            RetainedProjectLinkCasError::context("retain Project Link mutation authority", error)
        })?;
        authority
            .replace_file_with_validation(
                Path::new(PROJECT_LINK_NAME),
                successor.bytes.as_bytes(),
                |root, _temporary, target| {
                    let _retained = lifetime.retain_target(root, target)?;
                    Ok(())
                },
            )
            .map_err(|error| {
                RetainedProjectLinkCasError::context(
                    "replace exact predecessor Project Link",
                    error,
                )
            })?;
        self.project_link_is_successor(successor)?
            .ok_or_else(|| self.reject("successor verification failed after Project Link CAS"))
    }

    fn project_link_is_successor(
        &mut self,
        successor: &ReinitializeSuccessorProjectLink,
    ) -> Result<Option<ReinitializeAuthorityBinding>, Self::Error> {
        if successor != &self.plan.successor_project_link {
            return Err(self.reject("successor differs from the retained plan"));
        }
        let bytes = match self
            .project_root
            .read_authority_bounded(Path::new(PROJECT_LINK_NAME), MAX_PROJECT_LINK_BYTES)
        {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(RetainedProjectLinkCasError::context(
                    "read successor Project Link",
                    error,
                ))
            }
        };
        if bytes != successor.bytes.as_bytes()
            || sha256_content_hash(&bytes) != successor.sha256
            || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != successor.byte_length
        {
            return Ok(None);
        }
        Ok(Some(ReinitializeAuthorityBinding {
            path: PROJECT_LINK_NAME.to_owned(),
            sha256: successor.sha256.clone(),
            byte_length: successor.byte_length,
            anchor_nonce: self.plan.operation_id.clone(),
        }))
    }
}

fn prepare_destination_root(
    plan: &ReinitializePlan,
) -> Result<RetainedDirectory, RetainedProjectLinkCasError> {
    let destination = Path::new(&plan.destination);
    let parent_path = destination
        .parent()
        .ok_or_else(|| RetainedProjectLinkCasError::new("destination has no parent"))?;
    let leaf = destination
        .file_name()
        .ok_or_else(|| RetainedProjectLinkCasError::new("destination has no leaf"))?;
    let parent = open_absolute_directory_nofollow(parent_path)?;
    revalidate_absolute_directory(&parent, parent_path)?;
    parent.create_dir_all(Path::new(leaf)).map_err(|error| {
        RetainedProjectLinkCasError::context("create or retain empty destination", error)
    })?;
    let destination_root = parent.open_directory(Path::new(leaf)).map_err(|error| {
        RetainedProjectLinkCasError::context("retain destination directory", error)
    })?;
    revalidate_absolute_directory(&destination_root, destination)?;

    let marker = ReinitializeReservationMarker {
        schema_version: RESERVATION_SCHEMA.to_owned(),
        operation_id: plan.operation_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        project_root: plan.project_root.clone(),
        destination: plan.destination.clone(),
        destination_state_root: plan.destination_state_root.clone(),
    };
    let marker_bytes = serde_json_canonicalizer::to_vec(&marker).map_err(|error| {
        RetainedProjectLinkCasError::context("encode destination reservation", error)
    })?;
    match destination_root
        .read_authority_bounded(Path::new(RESERVATION_MARKER), MAX_AUTHORITY_DOCUMENT_BYTES)
    {
        Ok(existing) => {
            if existing != marker_bytes {
                return Err(RetainedProjectLinkCasError::new(
                    "destination reservation binds a different plan",
                ));
            }
            validate_destination_entries(&destination_root, destination, true)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            validate_destination_entries(&destination_root, destination, false)?;
            destination_root
                .retain_authority()
                .and_then(|authority| {
                    authority.write_new_file_synced(Path::new(RESERVATION_MARKER), &marker_bytes)
                })
                .map_err(|write_error| {
                    RetainedProjectLinkCasError::context(
                        "publish destination reservation",
                        write_error,
                    )
                })?;
        }
        Err(error) => {
            return Err(RetainedProjectLinkCasError::context(
                "read destination reservation",
                error,
            ))
        }
    }

    destination_root
        .create_dir_all(Path::new(STATE_ROOT_NAME))
        .map_err(|error| {
            RetainedProjectLinkCasError::context("create destination state root", error)
        })?;
    let state_root = destination_root
        .open_directory(Path::new(STATE_ROOT_NAME))
        .map_err(|error| {
            RetainedProjectLinkCasError::context("retain created state root", error)
        })?;
    for relative in INITIAL_STATE_DIRECTORIES {
        state_root
            .create_dir_all(Path::new(relative))
            .map_err(|error| {
                RetainedProjectLinkCasError::context("create fresh state directory", error)
            })?;
    }
    ensure_exact_state_file(&state_root, Path::new("ledger.ndjson"), b"")?;
    ensure_exact_state_file(
        &state_root,
        Path::new("state.yaml"),
        b"schema_version: forge_project_state_v1\ncurrent_phase: \"1-discovery\"\nupdated_at: null\n",
    )?;
    crate::replay_wal::initialize_replay_wal(&plan.destination_state_root).map_err(|error| {
        RetainedProjectLinkCasError::context("initialize fresh replay WAL", error)
    })?;
    state_root.sync_root().map_err(|error| {
        RetainedProjectLinkCasError::context("sync fresh destination state root", error)
    })?;
    validate_destination_entries(&destination_root, destination, true)?;
    revalidate_absolute_directory(&destination_root, destination)?;
    Ok(destination_root)
}

fn validate_reservation_marker(
    marker: &ReinitializeReservationMarker,
    plan: &ReinitializePlan,
) -> Result<(), RetainedProjectLinkCasError> {
    if marker.schema_version != RESERVATION_SCHEMA
        || marker.operation_id != plan.operation_id
        || marker.plan_digest != plan.plan_digest
        || marker.project_root != plan.project_root
        || marker.destination != plan.destination
        || marker.destination_state_root != plan.destination_state_root
    {
        return Err(RetainedProjectLinkCasError::new(
            "destination reservation does not bind the retained plan",
        ));
    }
    Ok(())
}

fn ensure_exact_state_file(
    root: &RetainedDirectory,
    relative: &Path,
    bytes: &[u8],
) -> Result<(), RetainedProjectLinkCasError> {
    match root.read_authority_bounded(relative, MAX_AUTHORITY_DOCUMENT_BYTES) {
        Ok(existing) if existing == bytes => Ok(()),
        Ok(_) => Err(RetainedProjectLinkCasError::new(
            "fresh destination contains conflicting state bytes",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => root
            .retain_authority()
            .and_then(|authority| authority.write_new_file_synced(relative, bytes))
            .map_err(|write_error| {
                RetainedProjectLinkCasError::context("publish initial state file", write_error)
            }),
        Err(error) => Err(RetainedProjectLinkCasError::context(
            "inspect initial state file",
            error,
        )),
    }
}

fn validate_destination_entries(
    retained: &RetainedDirectory,
    path: &Path,
    reservation_exists: bool,
) -> Result<(), RetainedProjectLinkCasError> {
    revalidate_absolute_directory(retained, path)?;
    let entries = fs::read_dir(path).map_err(|error| {
        RetainedProjectLinkCasError::context("inspect destination directory", error)
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            RetainedProjectLinkCasError::context("inspect destination entry", error)
        })?;
        let name = entry.file_name();
        let permitted = reservation_exists
            && (name == std::ffi::OsStr::new(RESERVATION_MARKER)
                || name == std::ffi::OsStr::new(STATE_ROOT_NAME));
        if !permitted {
            return Err(RetainedProjectLinkCasError::new(
                "destination must be empty or contain only this exact reservation",
            ));
        }
    }
    revalidate_absolute_directory(retained, path)
}

fn open_absolute_directory_nofollow(
    path: &Path,
) -> Result<RetainedDirectory, RetainedProjectLinkCasError> {
    normalized_absolute_path(path)
        .map_err(|error| RetainedProjectLinkCasError::new(error.to_string()))?;
    let mut anchor = path;
    while let Some(parent) = anchor.parent() {
        anchor = parent;
    }
    let root = RetainedDirectory::open_root(anchor)
        .map_err(|error| RetainedProjectLinkCasError::context("retain filesystem anchor", error))?;
    let relative = path.strip_prefix(anchor).map_err(|error| {
        RetainedProjectLinkCasError::context("derive retained absolute path", error)
    })?;
    if relative.as_os_str().is_empty() {
        Ok(root)
    } else {
        root.open_directory(relative).map_err(|error| {
            RetainedProjectLinkCasError::context("open absolute directory without links", error)
        })
    }
}

fn revalidate_absolute_directory(
    retained: &RetainedDirectory,
    path: &Path,
) -> Result<(), RetainedProjectLinkCasError> {
    let reopened = open_absolute_directory_nofollow(path)?;
    let retained_identity = retained.identity().map_err(|error| {
        RetainedProjectLinkCasError::context("identify retained directory", error)
    })?;
    let reopened_identity = reopened.identity().map_err(|error| {
        RetainedProjectLinkCasError::context("identify reopened directory", error)
    })?;
    if retained_identity != reopened_identity {
        return Err(RetainedProjectLinkCasError::new(
            "absolute directory binding changed identity",
        ));
    }
    Ok(())
}

fn read_absolute_authority_file(
    path: &Path,
    maximum: u64,
) -> Result<Vec<u8>, RetainedProjectLinkCasError> {
    let parent = path
        .parent()
        .ok_or_else(|| RetainedProjectLinkCasError::new("authority file has no parent"))?;
    let leaf = path
        .file_name()
        .ok_or_else(|| RetainedProjectLinkCasError::new("authority file has no leaf"))?;
    let retained_parent = open_absolute_directory_nofollow(parent)?;
    let bytes = retained_parent
        .read_authority_bounded(Path::new(leaf), maximum)
        .map_err(|error| {
            RetainedProjectLinkCasError::context("read absolute authority file", error)
        })?;
    revalidate_absolute_directory(&retained_parent, parent)?;
    Ok(bytes)
}

fn read_retained_file_bounded(
    file: &fs::File,
    maximum: u64,
) -> Result<Vec<u8>, RetainedProjectLinkCasError> {
    let before = file
        .metadata()
        .map_err(|error| RetainedProjectLinkCasError::context("inspect retained file", error))?;
    if before.len() > maximum {
        return Err(RetainedProjectLinkCasError::new(
            "retained authority file exceeds the byte limit",
        ));
    }
    let mut reader = file
        .try_clone()
        .map_err(|error| RetainedProjectLinkCasError::context("clone retained file", error))?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| RetainedProjectLinkCasError::context("read retained file", error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != before.len()
        || file
            .metadata()
            .map_err(|error| {
                RetainedProjectLinkCasError::context("reinspect retained file", error)
            })?
            .len()
            != before.len()
    {
        return Err(RetainedProjectLinkCasError::new(
            "retained authority file changed while reading",
        ));
    }
    Ok(bytes)
}

fn external_anchor_binding(binding: &RetainedFileAnchorBinding) -> ReinitializeFileAnchorBinding {
    ReinitializeFileAnchorBinding {
        schema_version: binding.schema_version.clone(),
        anchor_relative_path: binding.anchor_relative_path.clone(),
        nonce: binding.nonce.clone(),
        content_digest: binding.content_digest.clone(),
        byte_length: binding.byte_length,
    }
}

fn internal_anchor_binding(binding: &ReinitializeFileAnchorBinding) -> RetainedFileAnchorBinding {
    RetainedFileAnchorBinding {
        schema_version: binding.schema_version.clone(),
        anchor_relative_path: binding.anchor_relative_path.clone(),
        nonce: binding.nonce.clone(),
        content_digest: binding.content_digest.clone(),
        byte_length: binding.byte_length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ReinitializePlanRequest {
        ReinitializePlanRequest {
            operation_id: "r-1".into(),
            project_root: PathBuf::from("/tmp/forge-project"),
            destination: PathBuf::from("/tmp/forge-new-sidecar"),
            destination_state_root: PathBuf::from("/tmp/forge-new-sidecar/.forge-method"),
            diagnosis: ReinitializeDiagnosis {
                diagnosis_path: "/tmp/state-loss.json".into(),
                diagnosis_sha256: format!("sha256:{}", "a".repeat(64)),
                diagnostic: BootstrapStateLossDiagnostic {
                    schema_version: forge_core_contracts::BOOTSTRAP_STATE_LOSS_SCHEMA_VERSION
                        .into(),
                    diagnosis_digest: "a".repeat(64),
                    kind: forge_core_contracts::StateLossKind::LinkedStateUnavailable,
                    cause: forge_core_contracts::StateLossCause::MissingSidecar,
                    project_id: "project".into(),
                    project_link_schema_version: PROJECT_LINK_SCHEMA_VERSION.into(),
                    project_link_sha256: Some("b".repeat(64)),
                    workflow_release_id: None,
                    workflow_release_status:
                        forge_core_contracts::StateLossReleaseStatus::UnavailableUntrustedState,
                    choices: forge_core_contracts::BootstrapRecoveryChoices::for_project_root(
                        "/tmp/forge-project",
                    ),
                },
            },
            expected_project_link: ReinitializeAuthorityBinding {
                path: PROJECT_LINK_NAME.into(),
                sha256: format!("sha256:{}", "b".repeat(64)),
                byte_length: 3,
                anchor_nonce: "r-1".into(),
            },
            successor_project_link: format!(
                "schema_version: {PROJECT_LINK_SCHEMA_VERSION}\nproject_id: successor-project\nsidecar_root: /tmp/forge-new-sidecar\nstate_root: /tmp/forge-new-sidecar/.forge-method\n"
            )
            .into_bytes(),
            predecessor_identity: "old".into(),
            successor_project_id: "successor-project".into(),
            successor_identity: "new".into(),
        }
    }

    #[test]
    fn durable_record_paths_are_direct_operation_lock_children() {
        for path in [
            plan_path("r-1"),
            wal_path_phase("r-1", ReinitializeWalPhase::Reserved),
            receipt_path("r-1"),
        ] {
            assert!(!path.is_absolute());
            assert_eq!(path.components().count(), 1);
        }
    }

    #[test]
    fn plan_has_exact_confirmation_and_no_selected_host() {
        let plan = plan(request()).expect("plan");
        assert_eq!(plan.selected_host, None);
        assert_eq!(plan.confirmation_token, confirmation_for(&plan.plan_digest));
        assert!(validate_plan(&plan).is_ok());
    }

    #[test]
    fn destination_cannot_escape_the_retained_root() {
        let mut request = request();
        request.destination = PathBuf::from("../outside");
        assert!(plan(request).is_err());
    }

    #[test]
    fn sealed_plan_revalidates_every_request_path() {
        let mut sealed = plan(request()).expect("plan");
        sealed.destination = "../outside".into();
        sealed.plan_digest = digest_canonical(
            PLAN_DOMAIN,
            &ReinitializePlan {
                plan_digest: String::new(),
                confirmation_token: String::new(),
                ..sealed.clone()
            },
        )
        .expect("digest");
        sealed.confirmation_token = confirmation_for(&sealed.plan_digest);
        assert!(validate_plan(&sealed).is_err());
    }

    #[test]
    fn receipt_binds_both_identities_and_destination() {
        let plan = plan(request()).expect("plan");
        let receipt = receipt_for(
            &plan,
            &plan.expected_project_link,
            &ReinitializeFileAnchorBinding {
                schema_version: "anchor-v1".into(),
                anchor_relative_path: "project-reinitialize/project-link-anchors/r-1".into(),
                nonce: "anchor".into(),
                content_digest: plan.expected_project_link.sha256.clone(),
                byte_length: 3,
            },
            &plan.expected_project_link,
        )
        .expect("receipt");
        assert_eq!(receipt.predecessor_identity, "old");
        assert_eq!(receipt.successor_identity, "new");
        assert_eq!(receipt.destination, "/tmp/forge-new-sidecar");
        assert_eq!(receipt.selected_host, None);
    }
}
