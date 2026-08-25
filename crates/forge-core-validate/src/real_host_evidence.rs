//! Bounded, non-authoritative verification of P7F real-host evidence bundles.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::mem::MaybeUninit;
use std::path::{Component, Path, PathBuf};

use libyaml_rs as yaml_sys;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use yaml_serde::Value as YamlValue;

pub const SCHEMA_VERSION: &str = "forge_real_host_evidence_bundle_v0";
pub const COMMAND_LOG_SCHEMA_VERSION: &str = "forge_real_host_command_log_v0";
pub const AUTHORITY: &str = "non_authoritative_structural_content_integrity_evidence";
pub const DISCLAIMER: &str = "This result validates only structure and content integrity; it does not certify a production host, actor independence, publication, or P7F passage.";

const MAX_BUNDLE_BYTES: u64 = 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ARTIFACTS: usize = 256;
const MAX_NESTING_DEPTH: usize = 32;
const MAX_CONTAINER_ITEMS: usize = 100_000;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_ARGV_ITEMS: usize = 128;
const MAX_ARGV_BYTES: usize = 64 * 1024;

const SCENARIO_IDS: [&str; 3] = [
    "clean_host_journey",
    "concurrent_conflict",
    "replacement_session_resume",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RealHostEvidenceSummary {
    pub bundle_path: PathBuf,
    pub artifact_count: usize,
    pub governed_write_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealHostEvidenceError(String);

impl RealHostEvidenceError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for RealHostEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RealHostEvidenceError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceBundle {
    schema_version: String,
    authority: String,
    bundle_id: String,
    release_identity: ReleaseIdentity,
    artifacts: Vec<ArtifactRow>,
    scenarios: Vec<Scenario>,
    governed_writes: Vec<GovernedWrite>,
    ungoverned_writes: UngovernedWrites,
    residual_limitations: Vec<ResidualLimitation>,
    independent_review: IndependentReview,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactRow {
    id: String,
    path: String,
    sha256: String,
    size_bytes: u64,
    media_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseIdentity {
    release_id: String,
    product: String,
    version: String,
    platform: String,
    source_revision: String,
    archive_ref: String,
    release_manifest_ref: String,
    executable_ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    ordinal: u64,
    #[serde(rename = "scenario_id")]
    id: String,
    session_ids: Vec<String>,
    transcript_ref: String,
    command_log_ref: String,
    evidence_refs: Vec<String>,
    observation: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernedWrite {
    write_id: String,
    scenario_id: String,
    target: String,
    claim_ref: String,
    gate_result_ref: String,
    verified_principal_ref: String,
    admission_ref: String,
    pre_effect_wal_ref: String,
    effect_ref: String,
    receipt_ref: String,
}

impl GovernedWrite {
    fn evidence_refs(&self) -> [&str; 7] {
        [
            &self.claim_ref,
            &self.gate_result_ref,
            &self.verified_principal_ref,
            &self.admission_ref,
            &self.pre_effect_wal_ref,
            &self.effect_ref,
            &self.receipt_ref,
        ]
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UngovernedWrites {
    statement: String,
    observed: bool,
    entries: Vec<UngovernedWrite>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UngovernedWrite {
    scenario_id: String,
    target: String,
    method: String,
    reason: String,
    evidence_ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResidualLimitation {
    limitation_id: String,
    statement: String,
    impact: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndependentReview {
    reviewer_id: String,
    reviewed_at_utc: String,
    disposition: String,
    independence_statement: String,
    limitations: Vec<String>,
    review_record_ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandLog {
    schema_version: String,
    scenario_id: String,
    entries: Vec<CommandLogEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandLogEntry {
    sequence: u64,
    session_id: String,
    argv: YamlValue,
    working_directory: String,
    exit_code: i64,
    stdout_ref: String,
    stderr_ref: String,
}

type ArtifactIndex = HashMap<String, ArtifactRow>;
type ArtifactContents = HashMap<String, Vec<u8>>;

/// Verify the closed shape and declared byte integrity of a real-host evidence bundle.
///
/// Success remains deliberately narrow and does not certify the host, reviewer,
/// publication state, or P7F passage.
///
/// # Errors
///
/// Returns an error when the bundle, a referenced artifact, or a command log is
/// malformed, unsafe, out of bounds, or inconsistent with its declared digest.
pub fn verify_real_host_evidence(
    bundle_path: &Path,
) -> Result<RealHostEvidenceSummary, RealHostEvidenceError> {
    let raw_bundle = read_bounded_regular(bundle_path, MAX_BUNDLE_BYTES, "evidence bundle")?;
    let bundle: EvidenceBundle = decode_closed_document(&raw_bundle, "evidence bundle")?;
    validate_nonempty(&bundle.bundle_id, "$.bundle_id")?;
    if bundle.schema_version != SCHEMA_VERSION {
        return Err(RealHostEvidenceError::new(
            "$.schema_version: unsupported schema",
        ));
    }
    if bundle.authority != AUTHORITY {
        return Err(RealHostEvidenceError::new(
            "$.authority: must preserve the non-authoritative boundary",
        ));
    }

    let root = bundle_path.parent().unwrap_or_else(|| Path::new("."));
    let (artifacts, artifact_bytes) = load_artifacts(root, &bundle.artifacts)?;
    let mut used = HashSet::new();
    validate_release(&bundle.release_identity, &artifacts, &mut used)?;
    let command_logs = validate_scenarios(&bundle.scenarios, &artifacts, &mut used)?;
    validate_governed_writes(&bundle.governed_writes, &artifacts, &mut used)?;
    validate_disclosures(&bundle, &artifacts, &mut used)?;

    let mut seen_command_refs = HashSet::new();
    for (command_ref, scenario) in command_logs {
        if !seen_command_refs.insert(command_ref.clone()) {
            return Err(RealHostEvidenceError::new(
                "scenario command_log_ref values must be distinct",
            ));
        }
        let raw = artifact_bytes
            .get(&command_ref)
            .ok_or_else(|| RealHostEvidenceError::new("command log artifact bytes missing"))?;
        validate_command_log(raw, &command_ref, scenario, &artifacts, &mut used)?;
    }

    let mut unused: Vec<_> = artifacts
        .keys()
        .filter(|artifact_id| !used.contains(artifact_id.as_str()))
        .cloned()
        .collect();
    unused.sort();
    if !unused.is_empty() {
        return Err(RealHostEvidenceError::new(format!(
            "$.artifacts: unreferenced artifacts are forbidden: {unused:?}"
        )));
    }

    Ok(RealHostEvidenceSummary {
        bundle_path: bundle_path.to_path_buf(),
        artifact_count: artifacts.len(),
        governed_write_count: bundle.governed_writes.len(),
    })
}

fn load_artifacts(
    root: &Path,
    rows: &[ArtifactRow],
) -> Result<(ArtifactIndex, ArtifactContents), RealHostEvidenceError> {
    if rows.is_empty() || rows.len() > MAX_ARTIFACTS {
        return Err(RealHostEvidenceError::new(format!(
            "$.artifacts: must contain 1..{MAX_ARTIFACTS} rows"
        )));
    }
    let mut artifacts = HashMap::new();
    let mut contents = HashMap::new();
    let mut paths = HashSet::new();
    let mut total_size = 0_u64;
    for (index, row) in rows.iter().enumerate() {
        let path = format!("$.artifacts[{index}]");
        validate_nonempty(&row.id, &format!("{path}.id"))?;
        if artifacts.contains_key(&row.id) {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.id: duplicate artifact id {:?}",
                row.id
            )));
        }
        validate_canonical_relative_path(&row.path, &format!("{path}.path"))?;
        if !paths.insert(row.path.as_str()) {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.path: duplicate artifact path {:?}",
                row.path
            )));
        }
        if row.sha256.len() != 64
            || !row
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.sha256: must be lowercase SHA-256"
            )));
        }
        if row.size_bytes == 0 || row.size_bytes > MAX_ARTIFACT_BYTES {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.size_bytes: outside per-artifact byte limit"
            )));
        }
        validate_nonempty(&row.media_type, &format!("{path}.media_type"))?;
        let actual_path = resolve_artifact_path(root, &row.path, &row.id)?;
        let content = read_bounded_regular(
            &actual_path,
            MAX_ARTIFACT_BYTES,
            &format!("artifact {}", row.id),
        )?;
        if content.len() as u64 != row.size_bytes {
            return Err(RealHostEvidenceError::new(format!("{path}: size mismatch")));
        }
        if hex_lower(&Sha256::digest(&content)) != row.sha256 {
            return Err(RealHostEvidenceError::new(format!(
                "{path}: SHA-256 mismatch"
            )));
        }
        total_size = total_size.saturating_add(content.len() as u64);
        if total_size > MAX_TOTAL_ARTIFACT_BYTES {
            return Err(RealHostEvidenceError::new(
                "$.artifacts: total artifact bytes exceed limit",
            ));
        }
        artifacts.insert(row.id.clone(), row.clone());
        contents.insert(row.id.clone(), content);
    }
    Ok((artifacts, contents))
}

fn validate_release(
    release: &ReleaseIdentity,
    artifacts: &HashMap<String, ArtifactRow>,
    used: &mut HashSet<String>,
) -> Result<(), RealHostEvidenceError> {
    for (field, value) in [
        ("release_id", &release.release_id),
        ("version", &release.version),
        ("platform", &release.platform),
        ("source_revision", &release.source_revision),
    ] {
        validate_nonempty(value, &format!("$.release_identity.{field}"))?;
    }
    if release.product != "forge-method-core" {
        return Err(RealHostEvidenceError::new(
            "$.release_identity.product: must be forge-method-core",
        ));
    }
    let refs = [
        ("archive_ref", &release.archive_ref),
        ("release_manifest_ref", &release.release_manifest_ref),
        ("executable_ref", &release.executable_ref),
    ];
    let mut distinct = HashSet::new();
    for (field, reference) in refs {
        validate_ref(
            reference,
            &format!("$.release_identity.{field}"),
            artifacts,
            used,
        )?;
        distinct.insert(reference);
    }
    if distinct.len() != 3 {
        return Err(RealHostEvidenceError::new(
            "$.release_identity: archive, manifest, and executable refs must differ",
        ));
    }
    Ok(())
}

fn validate_scenarios<'a>(
    scenarios: &'a [Scenario],
    artifacts: &HashMap<String, ArtifactRow>,
    used: &mut HashSet<String>,
) -> Result<Vec<(String, &'a Scenario)>, RealHostEvidenceError> {
    if scenarios.len() != SCENARIO_IDS.len() {
        return Err(RealHostEvidenceError::new(
            "$.scenarios: exact three-scenario sequence is required",
        ));
    }
    let mut sessions = HashSet::new();
    let mut logs = Vec::new();
    for (index, scenario) in scenarios.iter().enumerate() {
        let path = format!("$.scenarios[{index}]");
        if scenario.ordinal != (index + 1) as u64 {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.ordinal: scenarios must be ordered 1..3"
            )));
        }
        if scenario.id != SCENARIO_IDS[index] {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.scenario_id: scenario order is fixed"
            )));
        }
        validate_string_list(&scenario.session_ids, &format!("{path}.session_ids"), true)?;
        let unique: HashSet<_> = scenario.session_ids.iter().collect();
        if unique.len() != scenario.session_ids.len()
            || scenario
                .session_ids
                .iter()
                .any(|session| !sessions.insert(session))
        {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.session_ids: session ids must be globally distinct"
            )));
        }
        if index > 0 && scenario.session_ids.len() < 2 {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.session_ids: this scenario requires at least two sessions"
            )));
        }
        validate_ref(
            &scenario.transcript_ref,
            &format!("{path}.transcript_ref"),
            artifacts,
            used,
        )?;
        validate_ref(
            &scenario.command_log_ref,
            &format!("{path}.command_log_ref"),
            artifacts,
            used,
        )?;
        validate_string_list(
            &scenario.evidence_refs,
            &format!("{path}.evidence_refs"),
            true,
        )?;
        let evidence: HashSet<_> = scenario.evidence_refs.iter().collect();
        if evidence.len() != scenario.evidence_refs.len() {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.evidence_refs: duplicate references are forbidden"
            )));
        }
        for (ref_index, reference) in scenario.evidence_refs.iter().enumerate() {
            validate_ref(
                reference,
                &format!("{path}.evidence_refs[{ref_index}]"),
                artifacts,
                used,
            )?;
        }
        validate_nonempty(&scenario.observation, &format!("{path}.observation"))?;
        logs.push((scenario.command_log_ref.clone(), scenario));
    }
    Ok(logs)
}

fn validate_governed_writes(
    writes: &[GovernedWrite],
    artifacts: &HashMap<String, ArtifactRow>,
    used: &mut HashSet<String>,
) -> Result<(), RealHostEvidenceError> {
    if writes.is_empty() {
        return Err(RealHostEvidenceError::new(
            "$.governed_writes: at least one claimed governed write is required",
        ));
    }
    let mut write_ids = HashSet::new();
    for (index, write) in writes.iter().enumerate() {
        let path = format!("$.governed_writes[{index}]");
        validate_nonempty(&write.write_id, &format!("{path}.write_id"))?;
        if !write_ids.insert(&write.write_id) {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.write_id: duplicate write id"
            )));
        }
        if !SCENARIO_IDS.contains(&write.scenario_id.as_str()) {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.scenario_id: unknown scenario"
            )));
        }
        validate_nonempty(&write.target, &format!("{path}.target"))?;
        let fields = [
            "claim_ref",
            "gate_result_ref",
            "verified_principal_ref",
            "admission_ref",
            "pre_effect_wal_ref",
            "effect_ref",
            "receipt_ref",
        ];
        let refs = write.evidence_refs();
        let mut distinct = HashSet::new();
        for (field, reference) in fields.into_iter().zip(refs) {
            validate_ref(reference, &format!("{path}.{field}"), artifacts, used)?;
            distinct.insert(reference);
        }
        if distinct.len() != fields.len() {
            return Err(RealHostEvidenceError::new(format!(
                "{path}: governed-write evidence links must be distinct"
            )));
        }
    }
    Ok(())
}

fn validate_disclosures(
    bundle: &EvidenceBundle,
    artifacts: &HashMap<String, ArtifactRow>,
    used: &mut HashSet<String>,
) -> Result<(), RealHostEvidenceError> {
    let disclosure = &bundle.ungoverned_writes;
    validate_nonempty(&disclosure.statement, "$.ungoverned_writes.statement")?;
    if disclosure.observed == disclosure.entries.is_empty() {
        return Err(RealHostEvidenceError::new(
            "$.ungoverned_writes: observed must exactly match whether entries exist",
        ));
    }
    for (index, entry) in disclosure.entries.iter().enumerate() {
        let path = format!("$.ungoverned_writes.entries[{index}]");
        if !SCENARIO_IDS.contains(&entry.scenario_id.as_str()) {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.scenario_id: unknown scenario"
            )));
        }
        for (field, value) in [
            ("target", &entry.target),
            ("method", &entry.method),
            ("reason", &entry.reason),
        ] {
            validate_nonempty(value, &format!("{path}.{field}"))?;
        }
        validate_ref(
            &entry.evidence_ref,
            &format!("{path}.evidence_ref"),
            artifacts,
            used,
        )?;
    }

    if bundle.residual_limitations.is_empty() {
        return Err(RealHostEvidenceError::new(
            "$.residual_limitations: explicit non-empty disclosure is required",
        ));
    }
    let mut limitation_ids = HashSet::new();
    for (index, limitation) in bundle.residual_limitations.iter().enumerate() {
        let path = format!("$.residual_limitations[{index}]");
        validate_nonempty(&limitation.limitation_id, &format!("{path}.limitation_id"))?;
        if !limitation_ids.insert(&limitation.limitation_id) {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.limitation_id: duplicate id"
            )));
        }
        validate_nonempty(&limitation.statement, &format!("{path}.statement"))?;
        validate_nonempty(&limitation.impact, &format!("{path}.impact"))?;
    }

    let review = &bundle.independent_review;
    for (field, value) in [
        ("reviewer_id", &review.reviewer_id),
        ("reviewed_at_utc", &review.reviewed_at_utc),
        ("independence_statement", &review.independence_statement),
    ] {
        validate_nonempty(value, &format!("$.independent_review.{field}"))?;
    }
    if !matches!(
        review.disposition.as_str(),
        "reviewed" | "qualified" | "changes_requested"
    ) {
        return Err(RealHostEvidenceError::new(
            "$.independent_review.disposition: unsupported disposition",
        ));
    }
    validate_string_list(
        &review.limitations,
        "$.independent_review.limitations",
        true,
    )?;
    validate_ref(
        &review.review_record_ref,
        "$.independent_review.review_record_ref",
        artifacts,
        used,
    )
}

fn validate_command_log(
    raw: &[u8],
    artifact_id: &str,
    scenario: &Scenario,
    artifacts: &HashMap<String, ArtifactRow>,
    used: &mut HashSet<String>,
) -> Result<(), RealHostEvidenceError> {
    let log: CommandLog = decode_closed_document(raw, &format!("command log {artifact_id}"))?;
    if log.schema_version != COMMAND_LOG_SCHEMA_VERSION {
        return Err(RealHostEvidenceError::new(format!(
            "command log {artifact_id}: unsupported schema_version"
        )));
    }
    if log.scenario_id != scenario.id {
        return Err(RealHostEvidenceError::new(format!(
            "command log {artifact_id}: scenario_id mismatch"
        )));
    }
    if log.entries.is_empty() {
        return Err(RealHostEvidenceError::new(format!(
            "command log {artifact_id}.entries: must be a non-empty array"
        )));
    }
    let allowed: HashSet<_> = scenario.session_ids.iter().collect();
    let mut represented = HashSet::new();
    for (index, entry) in log.entries.iter().enumerate() {
        let path = format!("command log {artifact_id}.entries[{index}]");
        if entry.sequence != (index + 1) as u64 {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.sequence: must be exact contiguous log order"
            )));
        }
        validate_nonempty(&entry.session_id, &format!("{path}.session_id"))?;
        if !allowed.contains(&entry.session_id) {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.session_id: not declared by the scenario"
            )));
        }
        represented.insert(&entry.session_id);
        let YamlValue::Sequence(argv_values) = &entry.argv else {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.argv: must be an exact bounded argument vector, not shell text"
            )));
        };
        let mut argv = Vec::with_capacity(argv_values.len());
        for (argv_index, value) in argv_values.iter().enumerate() {
            let YamlValue::String(value) = value else {
                return Err(RealHostEvidenceError::new(format!(
                    "{path}.argv[{argv_index}]: must be a non-empty NUL-free string"
                )));
            };
            validate_nonempty(value, &format!("{path}.argv[{argv_index}]"))?;
            argv.push(value.as_str());
        }
        if argv.is_empty() {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.argv: must be a non-empty array"
            )));
        }
        let argv_bytes = argv.iter().map(|arg| arg.len()).sum::<usize>();
        if argv.len() > MAX_ARGV_ITEMS
            || argv_bytes > MAX_ARGV_BYTES
            || argv.iter().any(|arg| arg.contains(['\n', '\r']))
        {
            return Err(RealHostEvidenceError::new(format!(
                "{path}.argv: must be an exact bounded argument vector, not shell text"
            )));
        }
        validate_nonempty(
            &entry.working_directory,
            &format!("{path}.working_directory"),
        )?;
        let _ = entry.exit_code;
        validate_ref(
            &entry.stdout_ref,
            &format!("{path}.stdout_ref"),
            artifacts,
            used,
        )?;
        validate_ref(
            &entry.stderr_ref,
            &format!("{path}.stderr_ref"),
            artifacts,
            used,
        )?;
    }
    if represented != allowed {
        return Err(RealHostEvidenceError::new(format!(
            "command log {artifact_id}: every declared session must have an argv entry"
        )));
    }
    Ok(())
}

fn validate_ref(
    reference: &str,
    path: &str,
    artifacts: &HashMap<String, ArtifactRow>,
    used: &mut HashSet<String>,
) -> Result<(), RealHostEvidenceError> {
    validate_nonempty(reference, path)?;
    if !artifacts.contains_key(reference) {
        return Err(RealHostEvidenceError::new(format!(
            "{path}: unknown artifact reference {reference:?}"
        )));
    }
    used.insert(reference.to_owned());
    Ok(())
}

fn validate_nonempty(value: &str, path: &str) -> Result<(), RealHostEvidenceError> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(RealHostEvidenceError::new(format!(
            "{path}: must be a non-empty NUL-free string"
        )));
    }
    Ok(())
}

fn validate_string_list(
    values: &[String],
    path: &str,
    nonempty: bool,
) -> Result<(), RealHostEvidenceError> {
    if nonempty && values.is_empty() {
        return Err(RealHostEvidenceError::new(format!(
            "{path}: must be a non-empty array"
        )));
    }
    for (index, value) in values.iter().enumerate() {
        validate_nonempty(value, &format!("{path}[{index}]"))?;
    }
    Ok(())
}

fn validate_canonical_relative_path(value: &str, path: &str) -> Result<(), RealHostEvidenceError> {
    validate_nonempty(value, path)?;
    if value.contains('\\') || value.as_bytes().get(1) == Some(&b':') {
        return Err(RealHostEvidenceError::new(format!(
            "{path}: must use canonical relative POSIX syntax"
        )));
    }
    let candidate = Path::new(value);
    if candidate.is_absolute()
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(RealHostEvidenceError::new(format!(
            "{path}: must be a traversal-free relative path"
        )));
    }
    Ok(())
}

fn resolve_artifact_path(
    root: &Path,
    relative: &str,
    artifact_id: &str,
) -> Result<PathBuf, RealHostEvidenceError> {
    let mut current = root.to_path_buf();
    for part in relative.split('/') {
        current.push(part);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            RealHostEvidenceError::new(format!(
                "artifact {artifact_id}: cannot stat {}: {error}",
                current.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(RealHostEvidenceError::new(format!(
                "artifact {artifact_id}: symlink path components are forbidden: {}",
                current.display()
            )));
        }
    }
    Ok(current)
}

fn read_bounded_regular(
    path: &Path,
    limit: u64,
    label: &str,
) -> Result<Vec<u8>, RealHostEvidenceError> {
    let before = fs::symlink_metadata(path).map_err(|error| {
        RealHostEvidenceError::new(format!("{label}: cannot stat {}: {error}", path.display()))
    })?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(RealHostEvidenceError::new(format!(
            "{label}: must be a regular non-symlink file: {}",
            path.display()
        )));
    }
    if before.len() == 0 || before.len() > limit {
        return Err(RealHostEvidenceError::new(format!(
            "{label}: byte size {} is outside 1..{limit}: {}",
            before.len(),
            path.display()
        )));
    }
    let raw = fs::read(path).map_err(|error| {
        RealHostEvidenceError::new(format!("{label}: cannot read {}: {error}", path.display()))
    })?;
    if raw.len() as u64 != before.len() {
        return Err(RealHostEvidenceError::new(format!(
            "{label}: file changed while being read: {}",
            path.display()
        )));
    }
    Ok(raw)
}

fn decode_closed_document<T: DeserializeOwned>(
    raw: &[u8],
    label: &str,
) -> Result<T, RealHostEvidenceError> {
    let text = std::str::from_utf8(raw)
        .map_err(|_| RealHostEvidenceError::new(format!("{label}: document is not UTF-8")))?;
    if text.trim_start().starts_with(['{', '[']) {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
            RealHostEvidenceError::new(format!("{label}: invalid JSON: {error}"))
        })?;
        walk_json_bounded(&value, "$", 0, &mut 0)?;
        // Deserialize the source again so Serde can reject duplicate struct fields;
        // parsing through Value alone would discard that evidence.
        serde_json::from_str(text)
            .map_err(|error| RealHostEvidenceError::new(format!("{label}: invalid JSON: {error}")))
    } else {
        reject_yaml_references(raw, label)?;
        let value: YamlValue = yaml_serde::from_str(text).map_err(|error| {
            RealHostEvidenceError::new(format!("{label}: invalid YAML: {error}"))
        })?;
        walk_yaml_bounded(&value, "$", 0, &mut 0)?;
        yaml_serde::from_value(value)
            .map_err(|error| RealHostEvidenceError::new(format!("{label}: invalid YAML: {error}")))
    }
}

fn walk_json_bounded(
    value: &serde_json::Value,
    path: &str,
    depth: usize,
    count: &mut usize,
) -> Result<(), RealHostEvidenceError> {
    bump_bounds(path, depth, count)?;
    match value {
        serde_json::Value::String(text) => validate_text_bound(text, path),
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                walk_json_bounded(item, &format!("{path}[{index}]"), depth + 1, count)?;
            }
            Ok(())
        }
        serde_json::Value::Object(mapping) => {
            for (key, item) in mapping {
                bump_bounds(&format!("{path}.<key>"), depth + 1, count)?;
                validate_text_bound(key, path)?;
                if key == "<<" {
                    return Err(RealHostEvidenceError::new(format!(
                        "{path}: YAML merge keys are forbidden"
                    )));
                }
                walk_json_bounded(item, &format!("{path}.{key}"), depth + 1, count)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn walk_yaml_bounded(
    value: &YamlValue,
    path: &str,
    depth: usize,
    count: &mut usize,
) -> Result<(), RealHostEvidenceError> {
    bump_bounds(path, depth, count)?;
    match value {
        YamlValue::String(text) => validate_text_bound(text, path),
        YamlValue::Sequence(items) => {
            for (index, item) in items.iter().enumerate() {
                walk_yaml_bounded(item, &format!("{path}[{index}]"), depth + 1, count)?;
            }
            Ok(())
        }
        YamlValue::Mapping(mapping) => {
            for (key, item) in mapping {
                let YamlValue::String(key) = key else {
                    return Err(RealHostEvidenceError::new(format!(
                        "{path}: mapping keys must be strings"
                    )));
                };
                bump_bounds(&format!("{path}.<key>"), depth + 1, count)?;
                validate_text_bound(key, path)?;
                if key == "<<" {
                    return Err(RealHostEvidenceError::new(format!(
                        "{path}: YAML merge keys are forbidden"
                    )));
                }
                walk_yaml_bounded(item, &format!("{path}.{key}"), depth + 1, count)?;
            }
            Ok(())
        }
        YamlValue::Tagged(_) => Err(RealHostEvidenceError::new(format!(
            "{path}: explicit YAML tags are forbidden"
        ))),
        _ => Ok(()),
    }
}

fn bump_bounds(path: &str, depth: usize, count: &mut usize) -> Result<(), RealHostEvidenceError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(RealHostEvidenceError::new(format!(
            "{path}: nesting exceeds {MAX_NESTING_DEPTH}"
        )));
    }
    *count = count.saturating_add(1);
    if *count > MAX_CONTAINER_ITEMS {
        return Err(RealHostEvidenceError::new(
            "parsed document exceeds container item limit",
        ));
    }
    Ok(())
}

fn validate_text_bound(text: &str, path: &str) -> Result<(), RealHostEvidenceError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(RealHostEvidenceError::new(format!(
            "{path}: text exceeds {MAX_TEXT_BYTES} bytes"
        )));
    }
    Ok(())
}

fn reject_yaml_references(raw: &[u8], label: &str) -> Result<(), RealHostEvidenceError> {
    struct ParserGuard(yaml_sys::yaml_parser_t);
    impl Drop for ParserGuard {
        fn drop(&mut self) {
            // SAFETY: the parser was initialized once and is deleted once by this guard.
            unsafe { yaml_sys::yaml_parser_delete(&raw mut self.0) };
        }
    }

    let mut parser = MaybeUninit::<yaml_sys::yaml_parser_t>::uninit();
    // SAFETY: libyaml initializes the pointed-to storage before it is assumed initialized.
    if unsafe { yaml_sys::yaml_parser_initialize(parser.as_mut_ptr()) }.fail {
        return Err(RealHostEvidenceError::new(format!(
            "{label}: cannot initialize YAML parser"
        )));
    }
    // SAFETY: initialization succeeded above.
    let mut parser = ParserGuard(unsafe { parser.assume_init() });
    // SAFETY: `raw` stays alive for the full parser lifetime and libyaml only reads it.
    unsafe {
        yaml_sys::yaml_parser_set_encoding(&raw mut parser.0, yaml_sys::YAML_UTF8_ENCODING);
        yaml_sys::yaml_parser_set_input_string(&raw mut parser.0, raw.as_ptr(), raw.len() as u64);
    }
    loop {
        let mut token = MaybeUninit::<yaml_sys::yaml_token_t>::uninit();
        // SAFETY: the initialized parser owns the scan state and fills `token` on success.
        if unsafe { yaml_sys::yaml_parser_scan(&raw mut parser.0, token.as_mut_ptr()) }.fail {
            return Err(RealHostEvidenceError::new(format!("{label}: invalid YAML")));
        }
        // SAFETY: a successful scan initialized the token.
        let mut token = unsafe { token.assume_init() };
        let token_type = token.type_;
        let forbidden = matches!(
            token_type,
            yaml_sys::YAML_ANCHOR_TOKEN | yaml_sys::YAML_ALIAS_TOKEN | yaml_sys::YAML_TAG_TOKEN
        );
        let done = token_type == yaml_sys::YAML_STREAM_END_TOKEN;
        // SAFETY: the token was initialized by a successful scan and is deleted once.
        unsafe { yaml_sys::yaml_token_delete(&raw mut token) };
        if forbidden {
            return Err(RealHostEvidenceError::new(format!(
                "{label}: YAML anchors, aliases, and explicit tags are forbidden"
            )));
        }
        if done {
            return Ok(());
        }
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[(byte >> 4) as usize]));
        output.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "forge-real-host-evidence-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Fixture {
        directory: TestDirectory,
        rows: Vec<Value>,
        contents: HashMap<String, Vec<u8>>,
        bundle_path: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = TestDirectory::new();
            fs::create_dir(directory.0.join("artifacts")).expect("artifact directory");
            let bundle_path = directory.0.join("bundle.yaml");
            Self {
                directory,
                rows: Vec::new(),
                contents: HashMap::new(),
                bundle_path,
            }
        }

        fn add(&mut self, id: &str, content: impl AsRef<[u8]>, media_type: &str) -> String {
            let raw = content.as_ref().to_vec();
            let relative = format!("artifacts/{id}.dat");
            fs::write(self.directory.0.join(&relative), &raw).expect("write artifact");
            self.contents.insert(id.to_owned(), raw.clone());
            self.rows.push(json!({
                "id": id,
                "path": relative,
                "sha256": hex_lower(&Sha256::digest(&raw)),
                "size_bytes": raw.len(),
                "media_type": media_type,
            }));
            id.to_owned()
        }

        fn text(&mut self, id: &str, content: impl AsRef<[u8]>) -> String {
            self.add(id, content, "text/plain")
        }

        fn command_log(&mut self, scenario_id: &str, sessions: &[&str]) -> String {
            let mut entries = Vec::new();
            for (index, session) in sessions.iter().enumerate() {
                let stdout = self.text(
                    &format!("stdout-{scenario_id}-{}", index + 1),
                    format!("stdout {}\n", index + 1),
                );
                let stderr = self.text(
                    &format!("stderr-{scenario_id}-{}", index + 1),
                    format!("stderr {}\n", index + 1),
                );
                entries.push(json!({
                    "sequence": index + 1,
                    "session_id": session,
                    "argv": ["forge-core", "status", "--json", format!("--session={session}")],
                    "working_directory": "/clean/consumer-project",
                    "exit_code": 0,
                    "stdout_ref": stdout,
                    "stderr_ref": stderr,
                }));
            }
            let id = format!("log-{scenario_id}");
            let raw = serde_json::to_vec(&json!({
                "schema_version": COMMAND_LOG_SCHEMA_VERSION,
                "scenario_id": scenario_id,
                "entries": entries,
            }))
            .expect("serialize command log");
            self.add(&id, raw, "application/json")
        }

        fn build(&mut self) -> Value {
            let archive = self.add(
                "release-archive",
                b"release archive bytes\n",
                "application/octet-stream",
            );
            let manifest = self.add(
                "release-manifest",
                br#"{"version":"0.9.0"}\n"#,
                "application/json",
            );
            let executable = self.add(
                "release-executable",
                b"executable bytes\n",
                "application/octet-stream",
            );
            let scenario_data = [
                ("clean_host_journey", vec!["session-clean"]),
                (
                    "concurrent_conflict",
                    vec!["session-conflict-a", "session-conflict-b"],
                ),
                (
                    "replacement_session_resume",
                    vec!["session-original", "session-replacement"],
                ),
            ];
            let mut scenarios = Vec::new();
            for (index, (scenario_id, sessions)) in scenario_data.iter().enumerate() {
                let transcript = self.text(
                    &format!("transcript-{scenario_id}"),
                    format!("transcript {scenario_id}\n"),
                );
                let evidence = self.text(
                    &format!("scenario-evidence-{scenario_id}"),
                    format!("evidence {scenario_id}\n"),
                );
                scenarios.push(json!({
                    "ordinal": index + 1,
                    "scenario_id": scenario_id,
                    "session_ids": sessions,
                    "transcript_ref": transcript,
                    "command_log_ref": self.command_log(scenario_id, sessions),
                    "evidence_refs": [evidence],
                    "observation": format!("Recorded observation for {scenario_id}; not a pass verdict."),
                }));
            }

            let mut links = serde_json::Map::new();
            for field in [
                "claim_ref",
                "gate_result_ref",
                "verified_principal_ref",
                "admission_ref",
                "pre_effect_wal_ref",
                "effect_ref",
                "receipt_ref",
            ] {
                links.insert(
                    field.to_owned(),
                    Value::String(
                        self.text(&format!("write-{field}"), format!("{field} evidence\n")),
                    ),
                );
            }
            let mut governed_write = serde_json::Map::from_iter([
                ("write_id".to_owned(), json!("governed-write-1")),
                ("scenario_id".to_owned(), json!("clean_host_journey")),
                (
                    "target".to_owned(),
                    json!(".forge-method/artifacts/result.yaml"),
                ),
            ]);
            governed_write.extend(links);
            let review_record = self.text("review-record", "independent review record\n");
            let mut bundle = json!({
                "schema_version": SCHEMA_VERSION,
                "authority": AUTHORITY,
                "bundle_id": "bundle.p7f.fixture.v0",
                "release_identity": {
                    "release_id": "forge-method-core-v0.9.0-linux-x86_64",
                    "product": "forge-method-core",
                    "version": "0.9.0",
                    "platform": "linux-x86_64",
                    "source_revision": "0123456789abcdef",
                    "archive_ref": archive,
                    "release_manifest_ref": manifest,
                    "executable_ref": executable,
                },
                "artifacts": [],
                "scenarios": scenarios,
                "governed_writes": [Value::Object(governed_write)],
                "ungoverned_writes": {
                    "statement": "No ungoverned writes were observed; this is an explicit disclosure.",
                    "observed": false,
                    "entries": [],
                },
                "residual_limitations": [{
                    "limitation_id": "same-principal-boundary",
                    "statement": "The run did not establish hostile same-principal filesystem isolation.",
                    "impact": "A cooperating local-principal assumption remains.",
                }],
                "independent_review": {
                    "reviewer_id": "reviewer.fixture",
                    "reviewed_at_utc": "2026-07-14T12:00:00Z",
                    "disposition": "qualified",
                    "independence_statement": "Reviewer reports no authorship of the captured run.",
                    "limitations": ["The checker cannot verify that statement or actor independence."],
                    "review_record_ref": review_record,
                },
            });
            bundle["artifacts"] = Value::Array(self.rows.clone());
            self.write_bundle(&bundle);
            bundle
        }

        fn write_bundle(&self, bundle: &Value) {
            fs::write(
                &self.bundle_path,
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(bundle).expect("serialize bundle")
                ),
            )
            .expect("write bundle");
        }

        fn rewrite_artifact(&mut self, bundle: &mut Value, id: &str, document: &Value) {
            let raw = serde_json::to_vec(document).expect("serialize artifact");
            let row = self
                .rows
                .iter_mut()
                .find(|row| row["id"] == id)
                .expect("artifact row");
            fs::write(self.directory.0.join(row["path"].as_str().unwrap()), &raw)
                .expect("rewrite artifact");
            row["size_bytes"] = json!(raw.len());
            row["sha256"] = json!(hex_lower(&Sha256::digest(&raw)));
            self.contents.insert(id.to_owned(), raw);
            bundle["artifacts"] = Value::Array(self.rows.clone());
            self.write_bundle(bundle);
        }
    }

    fn new_fixture() -> (Fixture, Value) {
        let mut fixture = Fixture::new();
        let bundle = fixture.build();
        (fixture, bundle)
    }

    #[test]
    fn valid_bundle_reports_narrow_non_authoritative_result() {
        let (fixture, _) = new_fixture();
        let result = verify_real_host_evidence(&fixture.bundle_path).expect("valid fixture");
        assert!(result.artifact_count > 0);
        assert_eq!(result.governed_write_count, 1);
        assert!(DISCLAIMER.contains("does not certify a production host"));
        assert!(DISCLAIMER.contains("actor independence, publication, or P7F passage"));
    }

    #[test]
    fn rejects_digest_drift() {
        let (fixture, bundle) = new_fixture();
        let row = bundle["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "write-receipt_ref")
            .unwrap();
        fs::write(
            fixture.directory.0.join(row["path"].as_str().unwrap()),
            b"tampered receipt\n",
        )
        .unwrap();
        let error = verify_real_host_evidence(&fixture.bundle_path).unwrap_err();
        assert!(error.to_string().contains("mismatch"));
    }

    #[test]
    fn rejects_wrong_scenario_order_and_reused_session() {
        let (fixture, mut bundle) = new_fixture();
        bundle["scenarios"].as_array_mut().unwrap().swap(0, 1);
        fixture.write_bundle(&bundle);
        assert!(verify_real_host_evidence(&fixture.bundle_path).is_err());

        let (fixture, mut bundle) = new_fixture();
        bundle["scenarios"][1]["session_ids"][0] = bundle["scenarios"][0]["session_ids"][0].clone();
        fixture.write_bundle(&bundle);
        assert!(verify_real_host_evidence(&fixture.bundle_path).is_err());
    }

    #[test]
    fn rejects_missing_governed_write_link() {
        let (fixture, mut bundle) = new_fixture();
        bundle["governed_writes"][0]
            .as_object_mut()
            .unwrap()
            .remove("admission_ref");
        fixture.write_bundle(&bundle);
        let error = verify_real_host_evidence(&fixture.bundle_path).unwrap_err();
        assert!(error.to_string().contains("admission_ref"));
    }

    #[test]
    fn rejects_shell_text_instead_of_exact_argv() {
        let (mut fixture, mut bundle) = new_fixture();
        let log_id = bundle["scenarios"][0]["command_log_ref"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut log: Value = serde_json::from_slice(&fixture.contents[&log_id]).unwrap();
        log["entries"][0]["argv"] = json!("forge-core status --json");
        fixture.rewrite_artifact(&mut bundle, &log_id, &log);
        let error = verify_real_host_evidence(&fixture.bundle_path).unwrap_err();
        assert!(error.to_string().contains("argv"));
    }

    #[test]
    fn rejects_missing_mandatory_disclosures_or_review() {
        for field in [
            "ungoverned_writes",
            "residual_limitations",
            "independent_review",
        ] {
            let (fixture, mut bundle) = new_fixture();
            bundle.as_object_mut().unwrap().remove(field);
            fixture.write_bundle(&bundle);
            let error = verify_real_host_evidence(&fixture.bundle_path).unwrap_err();
            assert!(error.to_string().contains(field), "{error}");
        }
    }

    #[test]
    fn rejects_yaml_references_duplicate_keys_and_oversize_bundle() {
        for document in [
            "schema_version: &v forge_real_host_evidence_bundle_v0\nauthority: *v\n",
            "schema_version: !custom one\n",
            "schema_version: one\nschema_version: two\n",
        ] {
            let directory = TestDirectory::new();
            let path = directory.0.join("bundle.yaml");
            fs::write(&path, document).unwrap();
            assert!(verify_real_host_evidence(&path).is_err());
        }
        let directory = TestDirectory::new();
        let path = directory.0.join("bundle.json");
        let mut raw = b"{}".to_vec();
        raw.resize(
            usize::try_from(MAX_BUNDLE_BYTES).expect("1 MiB fits usize") + 1,
            b' ',
        );
        fs::write(&path, raw).unwrap();
        let error = verify_real_host_evidence(&path).unwrap_err();
        assert!(error.to_string().contains("byte size"));
    }

    #[test]
    fn rejects_unreferenced_artifact_and_noncanonical_path() {
        let (mut fixture, mut bundle) = new_fixture();
        fixture.text("orphan", "orphan\n");
        bundle["artifacts"] = Value::Array(fixture.rows.clone());
        fixture.write_bundle(&bundle);
        let error = verify_real_host_evidence(&fixture.bundle_path).unwrap_err();
        assert!(error.to_string().contains("unreferenced"));

        let (fixture, mut bundle) = new_fixture();
        bundle["artifacts"][0]["path"] = json!("artifacts/../escape");
        fixture.write_bundle(&bundle);
        let error = verify_real_host_evidence(&fixture.bundle_path).unwrap_err();
        assert!(error.to_string().contains("traversal-free"));
    }
}
