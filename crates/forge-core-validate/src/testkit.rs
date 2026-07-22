#![allow(clippy::missing_errors_doc)]

//! Host-neutral conformance cases for candidate capability adapters.
//!
//! This module is deliberately observational. Its cases and reports neither select
//! a host nor grant signing, trust, admission, install, activation, lifecycle,
//! protected-anchor, mutation, release, phase-transition, or host-selection
//! authority. In particular, a conforming capability result always reports no
//! selected host.

use forge_core_contracts::{HostCapabilityResult, HostCapabilityResultAuthority, HostKind};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The fixed C1.4 conformance dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConformanceDimension {
    NativeOrigin,
    SignerNonOracle,
    ConfusedDeputy,
    Administration,
    WireAndHistory,
    ReplayAndCrash,
    Privacy,
    InstallAndRollback,
}

impl ConformanceDimension {
    pub const ALL: [Self; 8] = [
        Self::NativeOrigin,
        Self::SignerNonOracle,
        Self::ConfusedDeputy,
        Self::Administration,
        Self::WireAndHistory,
        Self::ReplayAndCrash,
        Self::Privacy,
        Self::InstallAndRollback,
    ];

    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::NativeOrigin => "native-origin",
            Self::SignerNonOracle => "signer-non-oracle",
            Self::ConfusedDeputy => "confused-deputy",
            Self::Administration => "administration",
            Self::WireAndHistory => "wire-and-history",
            Self::ReplayAndCrash => "replay-and-crash",
            Self::Privacy => "privacy",
            Self::InstallAndRollback => "install-and-rollback",
        }
    }
}

/// A read-only projection over a host capability observation.
///
/// The concrete typed [`HostCapabilityResult`] implements this view below.
/// Adapter-specific result types may also implement it while retaining ownership
/// of their host-specific observation semantics.
pub trait HostCapabilityResultView {
    /// A candidate result must never select a host.
    fn selected_host(&self) -> Option<&str>;
    /// Whether this result claims any authority prohibited for candidate data.
    fn claims_prohibited_authority(&self) -> bool;
    /// A stable, host-neutral description of the result's observable outcome.
    fn outcome_label(&self) -> &str;
}

impl HostCapabilityResultView for HostCapabilityResult {
    fn selected_host(&self) -> Option<&str> {
        self.selected_host.map(host_kind_identifier)
    }

    fn claims_prohibited_authority(&self) -> bool {
        self.authority != HostCapabilityResultAuthority::ObservationOnly
            || self.selected_host.is_some()
            || self.supported
            || self.released
    }

    fn outcome_label(&self) -> &'static str {
        "typed-host-capability-observation"
    }
}

const fn host_kind_identifier(host: HostKind) -> &'static str {
    match host {
        HostKind::Codex => "codex",
        HostKind::Cursor => "cursor",
        HostKind::Opencode => "opencode",
        HostKind::Claude => "claude",
        HostKind::Pidev => "pidev",
        HostKind::ForgeApp => "forge_app",
    }
}

/// A host-neutral assertion against a typed capability result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityObservation {
    pub outcome_label: String,
    pub selected_host_is_none: bool,
    pub authority_free: bool,
}

impl CapabilityObservation {
    #[must_use]
    pub fn observe(result: &impl HostCapabilityResultView) -> Self {
        Self {
            outcome_label: result.outcome_label().to_owned(),
            selected_host_is_none: result.selected_host().is_none(),
            authority_free: !result.claims_prohibited_authority(),
        }
    }

    #[must_use]
    pub const fn passes_candidate_boundary(&self) -> bool {
        self.selected_host_is_none && self.authority_free
    }
}

/// A deterministic test vector, expressed entirely as public case metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceCase {
    pub id: String,
    pub dimension: ConformanceDimension,
    pub public_input: String,
    pub expected_outcome: ExpectedOutcome,
}

impl ConformanceCase {
    #[must_use]
    pub fn digest(&self) -> DeterministicDigest {
        digest_fields(&[
            "forge-testkit-case-v1",
            self.dimension.identifier(),
            &self.id,
            &self.public_input,
            self.expected_outcome.identifier(),
        ])
    }
}

/// Candidate-safe expected outcomes. None denotes approval, admission, or
/// activation; `Rejected` is the only positive security outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedOutcome {
    Observed,
    Gated,
    Rejected,
    Recovered,
}

impl ExpectedOutcome {
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Gated => "gated",
            Self::Rejected => "rejected",
            Self::Recovered => "recovered",
        }
    }
}

/// The default complete C1.4 matrix. Inputs are opaque public fixture labels,
/// not credentials, private broker material, or host configuration.
#[must_use]
pub fn c1_4_cases() -> Vec<ConformanceCase> {
    vec![
        case(
            "c1.4.native-origin",
            ConformanceDimension::NativeOrigin,
            "forged-native-origin",
            ExpectedOutcome::Rejected,
        ),
        case(
            "c1.4.signer-non-oracle",
            ConformanceDimension::SignerNonOracle,
            "direct-signing-request",
            ExpectedOutcome::Rejected,
        ),
        case(
            "c1.4.confused-deputy",
            ConformanceDimension::ConfusedDeputy,
            "confirmation-packet-drift",
            ExpectedOutcome::Rejected,
        ),
        case(
            "c1.4.administration",
            ConformanceDimension::Administration,
            "forged-admin-approval",
            ExpectedOutcome::Rejected,
        ),
        case(
            "c1.4.wire-and-history",
            ConformanceDimension::WireAndHistory,
            "tampered-v0.2-provenance",
            ExpectedOutcome::Rejected,
        ),
        case(
            "c1.4.replay-and-crash",
            ConformanceDimension::ReplayAndCrash,
            "crash-before-replay-commit",
            ExpectedOutcome::Recovered,
        ),
        case(
            "c1.4.privacy",
            ConformanceDimension::Privacy,
            "private-broker-key-marker",
            ExpectedOutcome::Rejected,
        ),
        case(
            "c1.4.install-and-rollback",
            ConformanceDimension::InstallAndRollback,
            "unsupported-host-version",
            ExpectedOutcome::Gated,
        ),
    ]
}

fn case(
    id: &str,
    dimension: ConformanceDimension,
    public_input: &str,
    expected_outcome: ExpectedOutcome,
) -> ConformanceCase {
    ConformanceCase {
        id: id.to_owned(),
        dimension,
        public_input: public_input.to_owned(),
        expected_outcome,
    }
}

/// The result of observing one case. Reports are assertion records only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResult {
    pub case_id: String,
    pub observed_outcome: ExpectedOutcome,
    pub capability: CapabilityObservation,
    pub notes: Vec<String>,
}

impl CaseResult {
    #[must_use]
    pub fn passed(&self, case: &ConformanceCase) -> bool {
        self.case_id == case.id
            && self.observed_outcome == case.expected_outcome
            && self.capability.passes_candidate_boundary()
            && self.exact_digest_for(case).is_some()
    }

    /// Bind an observed result to the exact public C1.4 case digest. A result
    /// with a different case id cannot consume the case's digest.
    #[must_use]
    pub fn exact_digest_for(&self, case: &ConformanceCase) -> Option<DeterministicDigest> {
        (self.case_id == case.id).then(|| {
            let result_digest = self.digest();
            digest_fields(&[
                "forge-testkit-case-result-binding-v1",
                case.digest().as_str(),
                result_digest.as_str(),
            ])
        })
    }

    #[must_use]
    pub fn digest(&self) -> DeterministicDigest {
        let mut notes = self.notes.clone();
        notes.sort();
        let note_refs = notes.iter().map(String::as_str).collect::<Vec<_>>();
        let mut fields = vec![
            "forge-testkit-result-v1",
            self.case_id.as_str(),
            self.observed_outcome.identifier(),
            self.capability.outcome_label.as_str(),
            if self.capability.selected_host_is_none {
                "selected-host-none"
            } else {
                "selected-host-present"
            },
            if self.capability.authority_free {
                "authority-free"
            } else {
                "authority-claimed"
            },
        ];
        fields.extend(note_refs);
        digest_fields(&fields)
    }
}

/// An authority-free aggregate of case observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    pub case_results: Vec<CaseResult>,
    pub selected_host: Option<String>,
    pub authority_free: bool,
}

impl ConformanceReport {
    #[must_use]
    pub fn from_results(case_results: Vec<CaseResult>) -> Self {
        let authority_free = case_results
            .iter()
            .all(|result| result.capability.authority_free);
        Self {
            case_results,
            selected_host: None,
            authority_free,
        }
    }

    #[must_use]
    pub fn digest(&self) -> DeterministicDigest {
        let mut results = self.case_results.clone();
        results.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        let mut fields = vec![
            "forge-testkit-report-v1",
            if self.selected_host.is_none() {
                "selected-host-none"
            } else {
                "selected-host-present"
            },
            if self.authority_free {
                "authority-free"
            } else {
                "authority-claimed"
            },
        ];
        let digests = results.iter().map(CaseResult::digest).collect::<Vec<_>>();
        fields.extend(digests.iter().map(DeterministicDigest::as_str));
        digest_fields(&fields)
    }

    #[must_use]
    pub fn valid_boundary(&self) -> bool {
        let required = c1_4_cases()
            .into_iter()
            .map(|case| case.id)
            .collect::<BTreeSet<_>>();
        let actual = self
            .case_results
            .iter()
            .map(|result| result.case_id.clone())
            .collect::<BTreeSet<_>>();
        self.selected_host.is_none()
            && self.authority_free
            && self.case_results.len() == required.len()
            && actual == required
            && self.case_results.iter().all(|result| {
                result.capability.selected_host_is_none && result.capability.authority_free
            })
    }
}

/// An exact SHA-256 digest used to bind public test vectors and observations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeterministicDigest(String);

impl DeterministicDigest {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn digest_fields(fields: &[&str]) -> DeterministicDigest {
    // Length-prefix every public field, retaining unambiguous boundaries without
    // treating fixture contents as authority-bearing data.
    let mut hasher = Sha256::new();
    for field in fields {
        hasher.update(
            u64::try_from(field.len())
                .expect("usize fits in u64")
                .to_be_bytes(),
        );
        hasher.update(field.as_bytes());
    }
    DeterministicDigest(format!("sha256:{:x}", hasher.finalize()))
}

/// A process step specification. It describes a harness action; it does not run
/// shell commands and cannot inject shell strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessStep {
    pub program: String,
    pub argv: Vec<String>,
    pub checkpoint: Checkpoint,
}

/// Crash/replay checkpoints for subprocess and multiprocess harnesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checkpoint {
    BeforeStart,
    AfterPublicRegistryRead,
    BeforeDurableCheckpoint,
    AfterDurableCheckpoint,
}

/// A replay-crash scenario description. Each process has argv-only execution
/// metadata and no private key, selected host, or authority-bearing field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayCrashScenario {
    pub id: String,
    pub processes: Vec<ProcessStep>,
    pub crash_at: Checkpoint,
    pub required_public_registry_revision: u64,
}

impl ReplayCrashScenario {
    #[must_use]
    pub fn validate(&self) -> Result<(), HarnessError> {
        if self.id.is_empty()
            || self.processes.is_empty()
            || self.required_public_registry_revision == 0
        {
            return Err(HarnessError::InvalidScenario);
        }
        if self.processes.iter().any(|step| {
            step.program.is_empty()
                || contains_private_broker_key_marker(&step.program)
                || step.argv.iter().any(|arg| {
                    arg.is_empty() || arg.contains('\0') || contains_private_broker_key_marker(arg)
                })
        }) || !self
            .processes
            .iter()
            .any(|step| step.checkpoint == self.crash_at)
        {
            return Err(HarnessError::InvalidScenario);
        }
        Ok(())
    }

    #[must_use]
    pub fn digest(&self) -> DeterministicDigest {
        let mut fields = vec![
            "forge-testkit-replay-v1",
            self.id.as_str(),
            checkpoint_name(self.crash_at),
        ];
        let revision = self.required_public_registry_revision.to_string();
        fields.push(&revision);
        for process in &self.processes {
            fields.push(process.program.as_str());
            fields.push(checkpoint_name(process.checkpoint));
            fields.extend(process.argv.iter().map(String::as_str));
        }
        digest_fields(&fields)
    }
}

fn checkpoint_name(checkpoint: Checkpoint) -> &'static str {
    match checkpoint {
        Checkpoint::BeforeStart => "before_start",
        Checkpoint::AfterPublicRegistryRead => "after_public_registry_read",
        Checkpoint::BeforeDurableCheckpoint => "before_durable_checkpoint",
        Checkpoint::AfterDurableCheckpoint => "after_durable_checkpoint",
    }
}

/// Harness errors represent malformed descriptions, not host outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessError {
    InvalidScenario,
}

/// An executor interface for opt-in subprocess or multiprocess harnesses.
/// Implementations own process creation, isolation, and cleanup.
pub trait ReplayHarness {
    type Error;
    fn execute(&self, scenario: &ReplayCrashScenario) -> Result<ReplayExecution, Self::Error>;
}

/// Read-only execution evidence returned by a harness implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayExecution {
    pub recovered: bool,
    pub public_registry_revision: u64,
    pub selected_host: Option<String>,
    pub authority_free: bool,
}

impl ReplayExecution {
    #[must_use]
    pub const fn conforms(&self, scenario: &ReplayCrashScenario) -> bool {
        self.recovered
            && self.public_registry_revision >= scenario.required_public_registry_revision
            && self.selected_host.is_none()
            && self.authority_free
    }
}

/// A public-registry backup, intentionally unable to carry private broker keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicRegistryBackup {
    pub format_version: u16,
    pub registry_revision: u64,
    pub entries: Vec<PublicRegistryEntry>,
}

/// Public data permissible in a portable registry backup.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PublicRegistryEntry {
    pub subject: String,
    pub public_reference: String,
    pub status: PublicRegistryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PublicRegistryStatus {
    Active,
    Revoked,
}

/// Validate strict compatibility before accepting a public-registry backup.
#[must_use]
pub fn strict_public_registry_backup_compatible(
    backup: &PublicRegistryBackup,
    expected_format_version: u16,
    minimum_registry_revision: u64,
) -> bool {
    if backup.format_version != expected_format_version
        || backup.registry_revision < minimum_registry_revision
    {
        return false;
    }
    let mut entries = BTreeSet::new();
    backup.entries.iter().all(|entry| {
        !entry.subject.is_empty()
            && !entry.public_reference.is_empty()
            && !contains_private_broker_key_marker(&entry.subject)
            && !contains_private_broker_key_marker(&entry.public_reference)
            && entries.insert((entry.subject.as_str(), entry.public_reference.as_str()))
    })
}

/// A privacy sentinel finding. It only identifies a marker and byte offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyFinding {
    pub marker: &'static str,
    pub byte_offset: usize,
}

/// Scan text intended for fixtures, reports, or backups for private external
/// broker-key markers. The sentinel is conservative and case-insensitive.
#[must_use]
pub fn scan_privacy_sentinels(text: &str) -> Vec<PrivacyFinding> {
    const MARKERS: [&str; 8] = [
        "private broker key",
        "private_broker_key",
        "broker_private_key",
        "external_broker_private_key",
        "-----begin private key-----",
        "-----begin rsa private key-----",
        "-----begin ec private key-----",
        "-----begin openssh private key-----",
    ];
    let lowered = text.to_ascii_lowercase();
    MARKERS
        .iter()
        .flat_map(|marker| {
            lowered
                .match_indices(marker)
                .map(move |(byte_offset, _)| PrivacyFinding {
                    marker,
                    byte_offset,
                })
        })
        .collect()
}

fn contains_private_broker_key_marker(text: &str) -> bool {
    !scan_privacy_sentinels(text).is_empty()
}

/// Rendering fails closed rather than projecting an authority-bearing report as
/// safe. This makes golden output unsuitable for laundering a selected host or
/// a prohibited grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportRenderError {
    InvalidCandidateBoundary,
    PrivateBrokerKeyMarker,
}

/// Render a stable, authority-free text summary suitable for golden fixtures.
pub fn render_report(report: &ConformanceReport) -> Result<String, ReportRenderError> {
    if !report.valid_boundary() {
        return Err(ReportRenderError::InvalidCandidateBoundary);
    }
    let mut output = String::new();
    let _ = writeln!(output, "schema=forge-testkit-report-v1");
    let _ = writeln!(output, "selected_host=none");
    let _ = writeln!(output, "authority_free=true");
    let _ = writeln!(output, "digest={}", report.digest().as_str());
    if scan_privacy_sentinels(&output).is_empty() {
        Ok(output)
    } else {
        Err(ReportRenderError::PrivateBrokerKeyMarker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{
        ExactHostObservation, HostCapabilityFinding, HostCapabilityKind, HostCapabilityOutcome,
        HostCapabilityOutcomeBasis, StableId,
    };

    struct Candidate;
    impl HostCapabilityResultView for Candidate {
        fn selected_host(&self) -> Option<&str> {
            None
        }
        fn claims_prohibited_authority(&self) -> bool {
            false
        }
        fn outcome_label(&self) -> &'static str {
            "candidate"
        }
    }

    fn finding(capability: HostCapabilityKind) -> HostCapabilityFinding {
        HostCapabilityFinding {
            capability,
            outcome: HostCapabilityOutcome::Unknown,
            outcome_basis: HostCapabilityOutcomeBasis::NotObserved,
            evidence_ids: Vec::new(),
            limitation_ids: Vec::new(),
            conformance_case_results: Vec::new(),
        }
    }

    fn typed_candidate_result() -> HostCapabilityResult {
        HostCapabilityResult {
            result_id: StableId("host-capability-result.testkit".to_owned()),
            authority: HostCapabilityResultAuthority::ObservationOnly,
            observation: ExactHostObservation {
                host: HostKind::ForgeApp,
                host_version: "1.2.3".to_owned(),
                observation_id: StableId("host-observation.testkit".to_owned()),
                observed_at_unix: 1_900_000_000,
            },
            selected_host: None,
            supported: false,
            released: false,
            manifest_recognition: finding(HostCapabilityKind::ManifestRecognition),
            installability: finding(HostCapabilityKind::Installability),
            read_only_mcp: finding(HostCapabilityKind::ReadOnlyMcp),
            native_human_origin_assurance: finding(HostCapabilityKind::NativeHumanOriginAssurance),
            governed_mutation: finding(HostCapabilityKind::GovernedMutation),
            signer_isolation: finding(HostCapabilityKind::SignerIsolation),
            lifecycle: finding(HostCapabilityKind::Lifecycle),
            field_evidence: Vec::new(),
            known_limitations: Vec::new(),
        }
    }

    #[test]
    fn typed_capability_result_is_observed_without_authority_promotion() {
        let result = typed_candidate_result();
        result.validate().expect("typed candidate observation");
        let observation = CapabilityObservation::observe(&result);
        assert_eq!(
            observation.outcome_label,
            "typed-host-capability-observation"
        );
        assert!(observation.passes_candidate_boundary());
    }

    #[test]
    fn typed_capability_view_detects_forbidden_host_selection() {
        let mut result = typed_candidate_result();
        result.selected_host = Some(HostKind::Claude);
        let observation = CapabilityObservation::observe(&result);
        assert_eq!(result.selected_host(), Some("claude"));
        assert!(!observation.selected_host_is_none);
        assert!(!observation.authority_free);
        assert!(!observation.passes_candidate_boundary());
    }

    #[test]
    fn matrix_covers_each_c1_4_dimension_once() {
        let cases = c1_4_cases();
        let dimensions = cases
            .iter()
            .map(|case| case.dimension)
            .collect::<BTreeSet<_>>();
        assert_eq!(cases.len(), ConformanceDimension::ALL.len());
        assert_eq!(dimensions.len(), ConformanceDimension::ALL.len());
    }

    #[test]
    fn every_c1_4_case_uses_exact_case_and_result_digests() {
        for case in c1_4_cases() {
            let result = CaseResult {
                case_id: case.id.clone(),
                observed_outcome: case.expected_outcome,
                capability: CapabilityObservation::observe(&Candidate),
                notes: Vec::new(),
            };
            assert!(case.digest().as_str().starts_with("sha256:"));
            assert!(result.digest().as_str().starts_with("sha256:"));
            assert!(result
                .exact_digest_for(&case)
                .expect("matching case id")
                .as_str()
                .starts_with("sha256:"));
            assert!(result.passed(&case));
        }
    }

    #[test]
    fn result_digest_is_stable_when_notes_are_reordered() {
        let capability = CapabilityObservation::observe(&Candidate);
        let left = CaseResult {
            case_id: "c".into(),
            observed_outcome: ExpectedOutcome::Observed,
            capability: capability.clone(),
            notes: vec!["b".into(), "a".into()],
        };
        let right = CaseResult {
            notes: vec!["a".into(), "b".into()],
            ..left.clone()
        };
        assert_eq!(left.digest(), right.digest());
    }

    #[test]
    fn report_cannot_claim_host_selection() {
        let results = c1_4_cases()
            .into_iter()
            .map(|case| CaseResult {
                case_id: case.id,
                observed_outcome: case.expected_outcome,
                capability: CapabilityObservation::observe(&Candidate),
                notes: Vec::new(),
            })
            .collect();
        let report = ConformanceReport::from_results(results);
        assert!(report.valid_boundary());
        assert!(render_report(&report)
            .expect("valid candidate report")
            .contains("selected_host=none"));
    }

    #[test]
    fn rendering_rejects_selected_host_or_authority_claims() {
        let mut report = ConformanceReport::from_results(Vec::new());
        report.selected_host = Some("candidate-host".into());
        assert_eq!(
            render_report(&report),
            Err(ReportRenderError::InvalidCandidateBoundary)
        );

        report.selected_host = None;
        report.authority_free = false;
        assert_eq!(
            render_report(&report),
            Err(ReportRenderError::InvalidCandidateBoundary)
        );
    }

    #[test]
    fn sentinel_detects_private_external_broker_key_markers() {
        assert!(!scan_privacy_sentinels("EXTERNAL_BROKER_PRIVATE_KEY=value").is_empty());
        assert!(scan_privacy_sentinels("public-reference-only").is_empty());
    }

    #[test]
    fn backup_rejects_private_key_material_and_duplicates() {
        let backup = PublicRegistryBackup {
            format_version: 1,
            registry_revision: 2,
            entries: vec![PublicRegistryEntry {
                subject: "subject".into(),
                public_reference: "broker_private_key=value".into(),
                status: PublicRegistryStatus::Active,
            }],
        };
        assert!(!strict_public_registry_backup_compatible(&backup, 1, 2));
    }

    #[test]
    fn replay_harness_rejects_private_key_markers_and_unreachable_crash_points() {
        let private_material = ReplayCrashScenario {
            id: "replay".into(),
            processes: vec![ProcessStep {
                program: "forge".into(),
                argv: vec!["EXTERNAL_BROKER_PRIVATE_KEY=value".into()],
                checkpoint: Checkpoint::BeforeDurableCheckpoint,
            }],
            crash_at: Checkpoint::BeforeDurableCheckpoint,
            required_public_registry_revision: 1,
        };
        assert_eq!(
            private_material.validate(),
            Err(HarnessError::InvalidScenario)
        );

        let unreachable_crash = ReplayCrashScenario {
            processes: vec![ProcessStep {
                program: "forge".into(),
                argv: vec!["recover".into()],
                checkpoint: Checkpoint::BeforeStart,
            }],
            crash_at: Checkpoint::AfterDurableCheckpoint,
            ..private_material
        };
        assert_eq!(
            unreachable_crash.validate(),
            Err(HarnessError::InvalidScenario)
        );
    }

    #[test]
    fn scenario_is_argv_only_description_and_validates() {
        let scenario = ReplayCrashScenario {
            id: "replay".into(),
            processes: vec![ProcessStep {
                program: "forge".into(),
                argv: vec!["recover".into()],
                checkpoint: Checkpoint::BeforeDurableCheckpoint,
            }],
            crash_at: Checkpoint::BeforeDurableCheckpoint,
            required_public_registry_revision: 1,
        };
        assert!(scenario.validate().is_ok());
        assert!(scenario.digest().as_str().starts_with("sha256:"));
    }
}
