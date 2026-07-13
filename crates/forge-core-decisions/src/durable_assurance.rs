//! Pure reconstruction of durable Assurance state from admitted workflow events.
//!
//! Unlike the proposal-only Obligation Engine, this projector accepts no
//! caller-authored claim status, readiness verdict, or evaluator. An accepted
//! human intent opens a new assurance epoch with every universal lens unknown.

use forge_core_contracts::workflow_governance::WorkflowBrokerOriginProfile;
use forge_core_contracts::{
    AssuranceClaimStatus, DurableAssuranceEpochBinding, DurableAssuranceLensProjection,
    DurableAssuranceProjection, DurableAssuranceReadinessState, HumanIntentRevisionAcceptedEvent,
    UniversalAssuranceLens, WorkflowGovernanceEvent, WorkflowGovernanceLedgerRecord,
    WorkflowHumanIntentRevision, MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES,
    MAX_WORKFLOW_INTENT_ITEM_BYTES, MAX_WORKFLOW_INTENT_LIST_ITEMS,
    MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES, MAX_WORKFLOW_INTENT_TOTAL_BYTES,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Defensive rejection from intent canonicalization or durable projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssuranceProjectionError {
    pub issues: Vec<AssuranceProjectionIssue>,
}

impl std::fmt::Display for AssuranceProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "durable Assurance projection rejected with {} issue(s)",
            self.issues.len()
        )
    }
}

impl std::error::Error for AssuranceProjectionError {}

/// Stable issue codes suitable for a typed adapter failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AssuranceProjectionIssue {
    EmptyField { field: String },
    FieldTooLarge { field: String, maximum_bytes: usize },
    TooManyItems { field: String, maximum_items: usize },
    AggregateIntentTooLarge { maximum_bytes: usize },
    InvalidDigest { field: String },
    InvalidInitialEpoch { found: u64 },
    InvalidInitialRevision { found: u64 },
    NonMonotonicEpoch { expected: u64, found: u64 },
    NonMonotonicRevision { expected: u64, found: u64 },
    IntentIdentityChanged,
    PreviousIntentDigestMismatch,
    IntentDigestMismatch,
    LedgerHeadBindingMismatch,
    AcceptanceTimeMismatch,
    MissingOriginCompanion,
    OriginBindingMismatch,
    ProjectIdentityChanged,
    CounterOverflow,
    ProjectionEncodingFailed,
}

/// Validate and canonically digest one kernel-assigned human intent revision.
///
/// The digest uses RFC 8785-style canonical JSON bytes and a lower-case
/// `sha256:` wire value. Validation is intentionally shared by the kernel and
/// projector so oversized or structurally ambiguous content fails closed.
///
/// # Errors
///
/// Returns an assurance projection error when the intent is malformed,
/// outside the bounded contract, or cannot be canonicalized.
pub fn workflow_human_intent_digest(
    intent: &WorkflowHumanIntentRevision,
) -> Result<String, AssuranceProjectionError> {
    let issues = validate_intent(intent);
    if !issues.is_empty() {
        return Err(AssuranceProjectionError { issues });
    }
    canonical_digest(intent)
}

/// Reconstruct the latest durable Assurance epoch from governance history.
///
/// `None` means no human intent has yet been admitted. Every successful
/// projection contains each universal lens exactly once in canonical order,
/// with unknown status, no evidence, no evaluator, and unknown readiness.
///
/// # Errors
///
/// Returns an assurance projection error when intent history is malformed,
/// non-monotonic, or lacks the exact adjacent broker-origin binding.
pub fn project_durable_assurance(
    records: &[WorkflowGovernanceLedgerRecord],
) -> Result<Option<DurableAssuranceProjection>, AssuranceProjectionError> {
    let mut previous: Option<(
        &HumanIntentRevisionAcceptedEvent,
        &WorkflowGovernanceLedgerRecord,
    )> = None;
    let mut issues = Vec::new();

    for (index, record) in records.iter().enumerate() {
        let WorkflowGovernanceEvent::HumanIntentRevisionAccepted(event) = &record.event else {
            continue;
        };

        issues.extend(validate_intent(&event.intent));
        for (field, digest) in [
            ("intent_digest", event.intent_digest.as_str()),
            ("snapshot_digest", event.snapshot_digest.as_str()),
            (
                "acceptance_action_packet_digest",
                event.acceptance_action_packet_digest.as_str(),
            ),
            ("record_digest", record.record_digest.as_str()),
        ] {
            if !is_sha256_digest(digest) {
                issues.push(AssuranceProjectionIssue::InvalidDigest {
                    field: field.to_owned(),
                });
            }
        }
        if event.accepted_by.0.trim().is_empty() {
            issues.push(AssuranceProjectionIssue::EmptyField {
                field: "accepted_by".to_owned(),
            });
        }
        if record.previous_record_digest.as_deref() != Some(event.ledger_head_digest.as_str()) {
            issues.push(AssuranceProjectionIssue::LedgerHeadBindingMismatch);
        }
        if event.accepted_at_unix != record.recorded_at_unix {
            issues.push(AssuranceProjectionIssue::AcceptanceTimeMismatch);
        }
        match records.get(index + 1) {
            Some(companion_record) => match &companion_record.event {
                WorkflowGovernanceEvent::BrokerOriginApplied(companion) => {
                    if companion.action_record_digest != record.record_digest
                        || companion.action_packet_digest != event.acceptance_action_packet_digest
                        || companion.origin_principal_id != event.accepted_by
                        || companion.issuer_profile != WorkflowBrokerOriginProfile::Human
                        || companion.issued_at_unix != event.accepted_at_unix
                        || companion_record.project_id != record.project_id
                        || companion_record.previous_record_digest.as_deref()
                            != Some(record.record_digest.as_str())
                    {
                        issues.push(AssuranceProjectionIssue::OriginBindingMismatch);
                    }
                }
                _ => issues.push(AssuranceProjectionIssue::MissingOriginCompanion),
            },
            None => issues.push(AssuranceProjectionIssue::MissingOriginCompanion),
        }
        match workflow_human_intent_digest(&event.intent) {
            Ok(digest) if digest == event.intent_digest => {}
            Ok(_) => issues.push(AssuranceProjectionIssue::IntentDigestMismatch),
            Err(_) => {}
        }

        match previous {
            None => {
                if event.assurance_epoch != 1 {
                    issues.push(AssuranceProjectionIssue::InvalidInitialEpoch {
                        found: event.assurance_epoch,
                    });
                }
                if event.intent.revision != 1 {
                    issues.push(AssuranceProjectionIssue::InvalidInitialRevision {
                        found: event.intent.revision,
                    });
                }
                if event.previous_intent_digest.is_some() {
                    issues.push(AssuranceProjectionIssue::PreviousIntentDigestMismatch);
                }
            }
            Some((prior_event, prior_record)) => {
                if prior_record.project_id != record.project_id {
                    issues.push(AssuranceProjectionIssue::ProjectIdentityChanged);
                }
                match prior_event.assurance_epoch.checked_add(1) {
                    Some(expected) if event.assurance_epoch == expected => {}
                    Some(expected) => issues.push(AssuranceProjectionIssue::NonMonotonicEpoch {
                        expected,
                        found: event.assurance_epoch,
                    }),
                    None => issues.push(AssuranceProjectionIssue::CounterOverflow),
                }
                match prior_event.intent.revision.checked_add(1) {
                    Some(expected) if event.intent.revision == expected => {}
                    Some(expected) => {
                        issues.push(AssuranceProjectionIssue::NonMonotonicRevision {
                            expected,
                            found: event.intent.revision,
                        });
                    }
                    None => issues.push(AssuranceProjectionIssue::CounterOverflow),
                }
                if event.intent.intent_id != prior_event.intent.intent_id {
                    issues.push(AssuranceProjectionIssue::IntentIdentityChanged);
                }
                if event.previous_intent_digest.as_deref()
                    != Some(prior_event.intent_digest.as_str())
                {
                    issues.push(AssuranceProjectionIssue::PreviousIntentDigestMismatch);
                }
            }
        }
        previous = Some((event, record));
    }

    if !issues.is_empty() {
        return Err(AssuranceProjectionError { issues });
    }

    let Some((event, record)) = previous else {
        return Ok(None);
    };
    let lenses = UniversalAssuranceLens::ALL
        .into_iter()
        .map(|lens| DurableAssuranceLensProjection {
            lens,
            claim_status: AssuranceClaimStatus::Unknown,
            evidence_refs: Vec::new(),
            evaluator_ref: None,
        })
        .collect::<Vec<_>>();
    let mut projection = DurableAssuranceProjection {
        binding: DurableAssuranceEpochBinding {
            project_id: record.project_id.clone(),
            assurance_epoch: event.assurance_epoch,
            intent_id: event.intent.intent_id.clone(),
            intent_revision: event.intent.revision,
            intent_digest: event.intent_digest.clone(),
            accepted_record_digest: record.record_digest.clone(),
            accepted_state_version: record.state_version,
            snapshot_digest: event.snapshot_digest.clone(),
            ledger_head_before_acceptance: event.ledger_head_digest.clone(),
        },
        intent: event.intent.clone(),
        lenses,
        readiness: DurableAssuranceReadinessState::Unknown,
        blocker_lenses: UniversalAssuranceLens::ALL.to_vec(),
        projection_digest: String::new(),
    };
    projection.projection_digest = canonical_digest(&DurableAssuranceProjectionDigestSubject {
        binding: &projection.binding,
        intent: &projection.intent,
        lenses: &projection.lenses,
        readiness: projection.readiness,
        blocker_lenses: &projection.blocker_lenses,
    })?;
    Ok(Some(projection))
}

#[derive(Serialize)]
struct DurableAssuranceProjectionDigestSubject<'a> {
    binding: &'a DurableAssuranceEpochBinding,
    intent: &'a WorkflowHumanIntentRevision,
    lenses: &'a [DurableAssuranceLensProjection],
    readiness: DurableAssuranceReadinessState,
    blocker_lenses: &'a [UniversalAssuranceLens],
}

fn validate_intent(intent: &WorkflowHumanIntentRevision) -> Vec<AssuranceProjectionIssue> {
    let mut issues = Vec::new();
    if intent.intent_id.0.trim().is_empty() {
        issues.push(AssuranceProjectionIssue::EmptyField {
            field: "intent_id".to_owned(),
        });
    }
    validate_required_text(
        &mut issues,
        "desired_outcome",
        &intent.desired_outcome,
        MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES,
    );
    validate_required_text(
        &mut issues,
        "source_conversation_ref",
        &intent.source_conversation_ref,
        MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES,
    );
    if !is_sha256_digest(&intent.source_conversation_digest) {
        issues.push(AssuranceProjectionIssue::InvalidDigest {
            field: "source_conversation_digest".to_owned(),
        });
    }
    for (field, values) in [
        ("constraints", intent.constraints.as_slice()),
        ("preferences", intent.preferences.as_slice()),
        (
            "unacceptable_outcomes",
            intent.unacceptable_outcomes.as_slice(),
        ),
        ("uncertainties", intent.uncertainties.as_slice()),
    ] {
        if values.len() > MAX_WORKFLOW_INTENT_LIST_ITEMS {
            issues.push(AssuranceProjectionIssue::TooManyItems {
                field: field.to_owned(),
                maximum_items: MAX_WORKFLOW_INTENT_LIST_ITEMS,
            });
        }
        for (index, value) in values.iter().enumerate() {
            validate_required_text(
                &mut issues,
                &format!("{field}[{index}]"),
                value,
                MAX_WORKFLOW_INTENT_ITEM_BYTES,
            );
        }
    }
    let aggregate_bytes = intent.desired_outcome.len()
        + intent.constraints.iter().map(String::len).sum::<usize>()
        + intent.preferences.iter().map(String::len).sum::<usize>()
        + intent
            .unacceptable_outcomes
            .iter()
            .map(String::len)
            .sum::<usize>()
        + intent.uncertainties.iter().map(String::len).sum::<usize>()
        + intent.source_conversation_ref.len()
        + intent.source_conversation_digest.len();
    if aggregate_bytes > MAX_WORKFLOW_INTENT_TOTAL_BYTES {
        issues.push(AssuranceProjectionIssue::AggregateIntentTooLarge {
            maximum_bytes: MAX_WORKFLOW_INTENT_TOTAL_BYTES,
        });
    }
    issues
}

fn validate_required_text(
    issues: &mut Vec<AssuranceProjectionIssue>,
    field: &str,
    value: &str,
    maximum_bytes: usize,
) {
    if value.trim().is_empty() {
        issues.push(AssuranceProjectionIssue::EmptyField {
            field: field.to_owned(),
        });
    }
    if value.len() > maximum_bytes {
        issues.push(AssuranceProjectionIssue::FieldTooLarge {
            field: field.to_owned(),
            maximum_bytes,
        });
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, AssuranceProjectionError> {
    serde_json_canonicalizer::to_vec(value)
        .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)))
        .map_err(|_| AssuranceProjectionError {
            issues: vec![AssuranceProjectionIssue::ProjectionEncodingFailed],
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::workflow_governance::BrokerOriginAppliedEvent;
    use forge_core_contracts::{PrincipalId, StableId};

    fn hash(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn intent(revision: u64, outcome: &str) -> WorkflowHumanIntentRevision {
        WorkflowHumanIntentRevision {
            intent_id: StableId("intent.product".to_owned()),
            revision,
            desired_outcome: outcome.to_owned(),
            constraints: vec!["Remain portable".to_owned()],
            preferences: Vec::new(),
            unacceptable_outcomes: vec!["Silent data loss".to_owned()],
            uncertainties: vec!["Deployment topology".to_owned()],
            source_conversation_ref: format!("conversation:turn:{revision}"),
            source_conversation_digest: hash('c'),
        }
    }

    fn record(
        sequence: u64,
        state_version: u64,
        prior_head: &str,
        assurance_epoch: u64,
        intent: WorkflowHumanIntentRevision,
        previous_intent_digest: Option<String>,
    ) -> WorkflowGovernanceLedgerRecord {
        let intent_digest = workflow_human_intent_digest(&intent).expect("valid intent");
        WorkflowGovernanceLedgerRecord {
            record_id: StableId(format!("record.intent.{sequence}")),
            sequence,
            project_id: StableId("project.example".to_owned()),
            bundle_id: StableId("bundle.example".to_owned()),
            bundle_digest: hash('b'),
            state_version,
            previous_record_digest: Some(prior_head.to_owned()),
            record_digest: hash(char::from_digit((sequence % 10) as u32, 10).unwrap_or('a')),
            recorded_at_unix: 100 + sequence,
            event: WorkflowGovernanceEvent::HumanIntentRevisionAccepted(
                HumanIntentRevisionAcceptedEvent {
                    assurance_epoch,
                    intent,
                    intent_digest,
                    previous_intent_digest,
                    snapshot_digest: hash('d'),
                    ledger_head_digest: prior_head.to_owned(),
                    acceptance_action_packet_digest: hash('a'),
                    accepted_by: PrincipalId("principal.human".to_owned()),
                    accepted_at_unix: 100 + sequence,
                },
            ),
        }
    }

    fn origin_companion(
        action_record: &WorkflowGovernanceLedgerRecord,
        sequence: u64,
    ) -> WorkflowGovernanceLedgerRecord {
        let WorkflowGovernanceEvent::HumanIntentRevisionAccepted(action) = &action_record.event
        else {
            panic!("action record must contain accepted intent");
        };
        WorkflowGovernanceLedgerRecord {
            record_id: StableId(format!("record.origin.{sequence}")),
            sequence,
            project_id: action_record.project_id.clone(),
            bundle_id: action_record.bundle_id.clone(),
            bundle_digest: action_record.bundle_digest.clone(),
            state_version: action_record.state_version,
            previous_record_digest: Some(action_record.record_digest.clone()),
            record_digest: hash(char::from_digit((sequence % 10) as u32, 10).unwrap_or('a')),
            recorded_at_unix: action_record.recorded_at_unix + 1,
            event: WorkflowGovernanceEvent::BrokerOriginApplied(BrokerOriginAppliedEvent {
                action_packet_digest: action.acceptance_action_packet_digest.clone(),
                broker_event_digest: hash('e'),
                action_record_digest: action_record.record_digest.clone(),
                origin_principal_id: action.accepted_by.clone(),
                separation_domain: StableId("human-presence".to_owned()),
                nonce_fingerprint: hash('a'),
                issuer_id: StableId("broker.human".to_owned()),
                issuer_profile: WorkflowBrokerOriginProfile::Human,
                public_key_fingerprint: hash('b'),
                signature_fingerprint: hash('c'),
                enrollment_ceremony_digest: hash('d'),
                broker_registry_digest: hash('e'),
                issued_at_unix: action.accepted_at_unix,
                expires_at_unix: action.accepted_at_unix + 300,
            }),
        }
    }

    #[test]
    fn no_accepted_intent_has_no_durable_assurance_epoch() {
        assert_eq!(project_durable_assurance(&[]).expect("project"), None);
    }

    #[test]
    fn accepted_intent_creates_exactly_every_unknown_lens() {
        let record = record(1, 3, &hash('0'), 1, intent(1, "Ship safely"), None);
        let companion = origin_companion(&record, 2);

        let projection = project_durable_assurance(&[record, companion])
            .expect("valid projection")
            .expect("accepted intent");

        assert_eq!(projection.binding.assurance_epoch, 1);
        assert_eq!(projection.binding.intent_revision, 1);
        assert_eq!(projection.lenses.len(), UniversalAssuranceLens::ALL.len());
        assert_eq!(
            projection
                .lenses
                .iter()
                .map(|entry| entry.lens)
                .collect::<Vec<_>>(),
            UniversalAssuranceLens::ALL
        );
        assert!(projection.lenses.iter().all(|entry| {
            entry.claim_status == AssuranceClaimStatus::Unknown
                && entry.evidence_refs.is_empty()
                && entry.evaluator_ref.is_none()
        }));
        assert_eq!(
            projection.readiness,
            DurableAssuranceReadinessState::Unknown
        );
        assert_eq!(projection.blocker_lenses, UniversalAssuranceLens::ALL);
        assert!(is_sha256_digest(&projection.projection_digest));
    }

    #[test]
    fn latest_revision_advances_epoch_and_rebuilds_unknown_state() {
        let first = record(1, 3, &hash('0'), 1, intent(1, "First"), None);
        let first_companion = origin_companion(&first, 2);
        let first_intent_digest = match &first.event {
            WorkflowGovernanceEvent::HumanIntentRevisionAccepted(event) => {
                event.intent_digest.clone()
            }
            _ => unreachable!(),
        };
        let second = record(
            3,
            5,
            &hash('2'),
            2,
            intent(2, "Revised"),
            Some(first_intent_digest),
        );
        let second_companion = origin_companion(&second, 4);

        let projection =
            project_durable_assurance(&[first, first_companion, second, second_companion])
                .expect("valid revisions")
                .expect("accepted intent");

        assert_eq!(projection.binding.assurance_epoch, 2);
        assert_eq!(projection.binding.intent_revision, 2);
        assert_eq!(projection.intent.desired_outcome, "Revised");
        assert!(projection
            .lenses
            .iter()
            .all(|lens| lens.claim_status == AssuranceClaimStatus::Unknown));
    }

    #[test]
    fn forged_digest_or_non_monotonic_revision_fails_closed() {
        let first = record(1, 3, &hash('0'), 1, intent(1, "First"), None);
        let first_companion = origin_companion(&first, 2);
        let mut second = record(2, 4, &hash('1'), 3, intent(3, "Skip"), Some(hash('f')));
        let WorkflowGovernanceEvent::HumanIntentRevisionAccepted(event) = &mut second.event else {
            unreachable!();
        };
        event.intent_digest = hash('f');
        let second_companion = origin_companion(&second, 4);

        let error = project_durable_assurance(&[first, first_companion, second, second_companion])
            .expect_err("must fail closed");

        assert!(error
            .issues
            .contains(&AssuranceProjectionIssue::IntentDigestMismatch));
        assert!(error.issues.iter().any(|issue| matches!(
            issue,
            AssuranceProjectionIssue::NonMonotonicEpoch { .. }
                | AssuranceProjectionIssue::NonMonotonicRevision { .. }
        )));
        assert!(error
            .issues
            .contains(&AssuranceProjectionIssue::PreviousIntentDigestMismatch));
    }

    #[test]
    fn oversized_or_blank_human_content_is_rejected_before_digest() {
        let mut invalid = intent(1, " ");
        invalid.constraints = vec!["x".repeat(MAX_WORKFLOW_INTENT_ITEM_BYTES + 1)];

        let error = workflow_human_intent_digest(&invalid).expect_err("must reject");

        assert!(error.issues.iter().any(|issue| matches!(
            issue,
            AssuranceProjectionIssue::EmptyField { field } if field == "desired_outcome"
        )));
        assert!(error.issues.iter().any(|issue| matches!(
            issue,
            AssuranceProjectionIssue::FieldTooLarge { field, .. } if field == "constraints[0]"
        )));
    }

    #[test]
    fn orphan_or_mismatched_origin_companion_fails_closed() {
        let record = record(1, 3, &hash('0'), 1, intent(1, "Ship safely"), None);

        let orphan_error =
            project_durable_assurance(std::slice::from_ref(&record)).expect_err("orphan rejected");
        assert!(orphan_error
            .issues
            .contains(&AssuranceProjectionIssue::MissingOriginCompanion));

        let mut companion = origin_companion(&record, 2);
        let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = &mut companion.event else {
            unreachable!();
        };
        origin.origin_principal_id = PrincipalId("principal.impostor".to_owned());

        let mismatch_error = project_durable_assurance(&[record.clone(), companion])
            .expect_err("mismatched origin rejected");
        assert!(mismatch_error
            .issues
            .contains(&AssuranceProjectionIssue::OriginBindingMismatch));

        let mut companion = origin_companion(&record, 2);
        let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = &mut companion.event else {
            unreachable!();
        };
        origin.issued_at_unix = origin.issued_at_unix.saturating_add(1);

        let clock_mismatch_error = project_durable_assurance(&[record, companion])
            .expect_err("mismatched signed-origin clock rejected");
        assert!(clock_mismatch_error
            .issues
            .contains(&AssuranceProjectionIssue::OriginBindingMismatch));
    }
}
