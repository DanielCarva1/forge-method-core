//! Validation and Forge-owned outcome derivation for solo host conformance.

use forge_core_contracts::{
    SoloHostAdapterArgumentKind, SoloHostAssertionResult, SoloHostAssertionStatus,
    SoloHostBundleFile, SoloHostBundleFileRole, SoloHostCanonicalRootKind, SoloHostCapability,
    SoloHostCapabilityResult, SoloHostConformanceBindings, SoloHostConformanceBundleManifest,
    SoloHostConformanceCorpusDocument, SoloHostConformanceGap, SoloHostConformanceOutcome,
    SoloHostConformanceRequestDocument, SoloHostConformanceResponseDocument,
    SoloHostConformanceResultDocument, SoloHostGapKind, SoloHostProofState,
    SOLO_HOST_CONFORMANCE_BUNDLE_VERSION, SOLO_HOST_CONFORMANCE_PROTOCOL_VERSION,
    SOLO_HOST_CONFORMANCE_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SOLO_HOST_FILES: usize = 64;
pub const MAX_SOLO_HOST_ARTIFACT_BYTES: usize = 256 * 1024;
pub const MAX_SOLO_HOST_BUNDLE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_SOLO_HOST_DIRECTORY_DEPTH: usize = 4;
pub const DEFAULT_SOLO_HOST_ADAPTER_TIMEOUT_MS: u64 = 30_000;
pub const MAX_SOLO_HOST_ADAPTER_TIMEOUT_MS: u64 = 300_000;
pub const SOLO_HOST_AUTHENTICITY_NOTE: &str = "Forge verified this bundle's structure, completeness, and digests. This build has no trusted host-native proof verifier, so adapter-reported success remains unverified and can be at most partially_supported. Integrity is not proof that the host really performed an action.";
const BRIDGE_ASSERTION: &str = "windows_wsl_bridge_applied_only_when_required";
const AUTOMATIC_AUTHENTICITY_CODE: &str = "forge_native_verifier_unavailable";
const AUTOMATIC_BRIDGE_CODE: &str = "forge_bridge_applicability_indeterminate";

#[must_use]
pub fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Canonically encode and digest one serializable value.
///
/// # Errors
///
/// Returns an error when the value cannot be encoded as canonical JSON.
pub fn canonical_json_sha256<T: serde::Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| format!("cannot canonicalize conformance data: {error}"))?;
    Ok(sha256_digest(&bytes))
}

/// Validate the complete Solo host conformance corpus.
///
/// # Errors
///
/// Returns an error when the corpus schema, cases, or capability coverage is invalid.
pub fn validate_solo_host_corpus(corpus: &SoloHostConformanceCorpusDocument) -> Result<(), String> {
    if corpus.schema_version != SOLO_HOST_CONFORMANCE_SCHEMA_VERSION {
        return Err("unsupported solo host corpus schema_version".to_owned());
    }
    validate_safe_label("corpus_id", &corpus.corpus_id, 160)?;
    if corpus.cases.len() != SoloHostCapability::ALL.len() {
        return Err("corpus must contain exactly the eight required capabilities".to_owned());
    }
    let mut capabilities = BTreeSet::new();
    let mut case_ids = BTreeSet::new();
    for case in &corpus.cases {
        validate_token("case_id", &case.case_id)?;
        validate_public_text("case description", &case.description, 1024)?;
        if !case_ids.insert(case.case_id.as_str()) || !capabilities.insert(case.capability) {
            return Err("corpus case ids and capabilities must be unique".to_owned());
        }
        if case.required_assertions.is_empty() || case.required_assertions.len() > 16 {
            return Err(format!(
                "case {} has an invalid assertion count",
                case.case_id
            ));
        }
        let mut assertions = BTreeSet::new();
        for assertion in &case.required_assertions {
            validate_token("assertion", assertion)?;
            if !assertions.insert(assertion) {
                return Err(format!("case {} repeats an assertion", case.case_id));
            }
        }
    }
    if capabilities != SoloHostCapability::ALL.into_iter().collect() {
        return Err("corpus does not cover the exact required capabilities".to_owned());
    }
    Ok(())
}

/// Validate the bindings shared by a Solo host conformance request and response.
///
/// # Errors
///
/// Returns an error when a required binding is empty, malformed, or inconsistent.
pub fn validate_solo_host_bindings(bindings: &SoloHostConformanceBindings) -> Result<(), String> {
    for (field, value) in [
        ("host_id", &bindings.declared.host_id),
        ("host_version", &bindings.declared.host_version),
        ("adapter_id", &bindings.declared.adapter_id),
        ("adapter_version", &bindings.declared.adapter_version),
        ("platform_label", &bindings.declared.platform_label),
        ("environment_label", &bindings.declared.environment_label),
        ("forge_package", &bindings.observed.forge_package),
        ("forge_version", &bindings.observed.forge_version),
        ("observed os", &bindings.observed.platform.os),
        (
            "observed architecture",
            &bindings.observed.platform.architecture,
        ),
    ] {
        validate_safe_label(field, value, 256)?;
    }
    for (field, digest) in [
        (
            "forge_executable_sha256",
            &bindings.observed.forge_executable_sha256,
        ),
        (
            "resolved_path_sha256",
            &bindings.observed.canonical_root.resolved_path_sha256,
        ),
        (
            "adapter executable sha256",
            &bindings.observed.adapter_invocation.executable.sha256,
        ),
        (
            "adapter argv_sha256",
            &bindings.observed.adapter_invocation.argv_sha256,
        ),
        ("corpus_sha256", &bindings.corpus_sha256),
    ] {
        require_sha256(field, digest)?;
    }
    validate_file_identity(
        "adapter executable",
        &bindings.observed.adapter_invocation.executable,
    )?;
    if !bindings.observed.canonical_root.exists || !bindings.observed.canonical_root.is_directory {
        return Err("canonical root must be an existing directory observed by Forge".to_owned());
    }
    let expected_bridge = match (
        bindings.observed.platform.os.as_str(),
        bindings.observed.canonical_root.kind,
    ) {
        ("windows", SoloHostCanonicalRootKind::WslNetworkShare) => {
            forge_core_contracts::SoloHostBridgeApplicability::Applicable
        }
        ("windows", SoloHostCanonicalRootKind::NativeOrOther) | ("linux" | "macos", _) => {
            forge_core_contracts::SoloHostBridgeApplicability::NotApplicable
        }
        _ => forge_core_contracts::SoloHostBridgeApplicability::Indeterminate,
    };
    if bindings.observed.canonical_root.windows_to_wsl_bridge != expected_bridge {
        return Err(
            "Windows-to-WSL applicability contradicts Forge-observed platform and root facts"
                .to_owned(),
        );
    }
    let invocation = &bindings.observed.adapter_invocation;
    if invocation.timeout_ms == 0 || invocation.timeout_ms > MAX_SOLO_HOST_ADAPTER_TIMEOUT_MS {
        return Err("adapter timeout is outside the closed safety bounds".to_owned());
    }
    if invocation.output_limit_bytes == 0
        || invocation.output_limit_bytes > MAX_SOLO_HOST_BUNDLE_BYTES as u64
    {
        return Err("adapter output limit is outside the closed safety bounds".to_owned());
    }
    if invocation.arguments.len() > 32 {
        return Err("adapter has too many separated arguments".to_owned());
    }
    for (index, argument) in invocation.arguments.iter().enumerate() {
        if argument.position != u32::try_from(index).unwrap_or(u32::MAX) {
            return Err("adapter argument positions are not exact and contiguous".to_owned());
        }
        require_sha256("adapter argument_sha256", &argument.argument_sha256)?;
        match (argument.kind, &argument.file_identity) {
            (SoloHostAdapterArgumentKind::File, Some(identity)) => {
                validate_file_identity("adapter file argument", identity)?;
                if argument.safe_display != identity.file_name {
                    return Err(
                        "adapter file argument display does not match its basename".to_owned()
                    );
                }
            }
            (SoloHostAdapterArgumentKind::LiteralDigest, None)
                if argument.safe_display == "<literal-digest>" => {}
            _ => return Err("adapter argument kind and identity disagree".to_owned()),
        }
    }
    if invocation.argv_sha256 != canonical_json_sha256(&invocation.arguments)? {
        return Err(
            "adapter argv digest does not match the separated ordered arguments".to_owned(),
        );
    }
    Ok(())
}

/// Normalize a host response against the exact conformance request.
///
/// # Errors
///
/// Returns an error when bindings or case results are invalid, missing, duplicated, or unexpected.
pub fn normalize_solo_host_response(
    request: &SoloHostConformanceRequestDocument,
    response: &SoloHostConformanceResponseDocument,
) -> Result<SoloHostConformanceResponseDocument, String> {
    if request.schema_version != SOLO_HOST_CONFORMANCE_PROTOCOL_VERSION
        || response.schema_version != SOLO_HOST_CONFORMANCE_PROTOCOL_VERSION
    {
        return Err("unsupported solo host adapter protocol version".to_owned());
    }
    validate_solo_host_bindings(&request.bindings)?;
    if response.bindings != request.bindings {
        return Err("adapter response bindings do not exactly match the request".to_owned());
    }
    if !request.accepted_native_proof_schemes.is_empty() {
        return Err("this Forge build has no trusted native proof verifier".to_owned());
    }
    validate_solo_host_corpus(&SoloHostConformanceCorpusDocument {
        schema_version: SOLO_HOST_CONFORMANCE_SCHEMA_VERSION.to_owned(),
        corpus_id: "request-projection".to_owned(),
        cases: request.cases.clone(),
    })?;
    if response.cases.len() != request.cases.len() {
        return Err("adapter response is incomplete or contains extra cases".to_owned());
    }
    let mut observations = BTreeMap::new();
    for observation in &response.cases {
        validate_token("case_id", &observation.case_id)?;
        if observations
            .insert(observation.case_id.as_str(), observation)
            .is_some()
        {
            return Err("adapter response repeats a case".to_owned());
        }
    }

    let mut normalized_cases = Vec::with_capacity(request.cases.len());
    for case in &request.cases {
        let observation = observations
            .get(case.case_id.as_str())
            .ok_or_else(|| format!("adapter response is missing case {}", case.case_id))?;
        let required = case.required_assertions.iter().collect::<BTreeSet<_>>();
        let actual = observation.assertions.keys().collect::<BTreeSet<_>>();
        if actual != required {
            return Err(format!(
                "case {} assertion set is incomplete or contains extras",
                case.case_id
            ));
        }
        for claim in observation.assertions.values() {
            if let Some(proof) = &claim.native_proof_claim {
                validate_token("native proof scheme", &proof.scheme)?;
                require_sha256("native proof digest", &proof.proof_sha256)?;
                return Err("adapter supplied a native proof claim, but this Forge build accepts no native proof scheme".to_owned());
            }
        }
        if observation.gaps.len() > 16 {
            return Err(format!("case {} has too many typed gaps", case.case_id));
        }
        let mut gaps = observation.gaps.clone();
        let mut gap_codes = BTreeSet::new();
        for gap in &gaps {
            validate_token("gap code", &gap.code)?;
            if !gap_codes.insert(gap.code.as_str()) {
                return Err(format!("case {} repeats a typed gap", case.case_id));
            }
        }
        gaps.sort();

        if observation.evidence.fact_codes.is_empty() || observation.evidence.fact_codes.len() > 32
        {
            return Err(format!(
                "case {} must provide bounded closed evidence",
                case.case_id
            ));
        }
        let mut fact_codes = observation.evidence.fact_codes.clone();
        for fact in &fact_codes {
            validate_token("evidence fact code", fact)?;
        }
        fact_codes.sort();
        if fact_codes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(format!("case {} repeats an evidence fact", case.case_id));
        }

        let mut normalized = (*observation).clone();
        normalized.gaps = gaps;
        normalized.evidence.fact_codes = fact_codes;
        normalized_cases.push(normalized);
    }
    Ok(SoloHostConformanceResponseDocument {
        schema_version: response.schema_version.clone(),
        bindings: response.bindings.clone(),
        cases: normalized_cases,
    })
}

/// Validate a host response and derive the core-owned conformance result.
///
/// # Errors
///
/// Returns an error when the response cannot be normalized or the request corpus is invalid.
pub fn validate_and_derive_solo_host_result(
    request: &SoloHostConformanceRequestDocument,
    response: &SoloHostConformanceResponseDocument,
) -> Result<SoloHostConformanceResultDocument, String> {
    let response = normalize_solo_host_response(request, response)?;
    let mut capabilities = Vec::with_capacity(request.cases.len());
    for case in &request.cases {
        let observation = response
            .cases
            .iter()
            .find(|observation| observation.case_id == case.case_id)
            .ok_or_else(|| format!("adapter response is missing case {}", case.case_id))?;
        let mut assertions = Vec::with_capacity(case.required_assertions.len());
        let mut gaps = observation.gaps.clone();
        let mut any_passed = false;
        let mut every_applicable_passed_and_verified = true;
        for assertion in &case.required_assertions {
            let claim = observation
                .assertions
                .get(assertion)
                .ok_or_else(|| format!("case {} is missing assertion {assertion}", case.case_id))?;
            let (status, proof_state) = if assertion == BRIDGE_ASSERTION {
                match request
                    .bindings
                    .observed
                    .canonical_root
                    .windows_to_wsl_bridge
                {
                    forge_core_contracts::SoloHostBridgeApplicability::NotApplicable => (
                        SoloHostAssertionStatus::NotApplicable,
                        SoloHostProofState::ForgeObserved,
                    ),
                    forge_core_contracts::SoloHostBridgeApplicability::Applicable => {
                        adapter_claim_result(claim.passed)
                    }
                    forge_core_contracts::SoloHostBridgeApplicability::Indeterminate => {
                        if !gaps.iter().any(|gap| gap.code == AUTOMATIC_BRIDGE_CODE) {
                            gaps.push(SoloHostConformanceGap {
                                kind: SoloHostGapKind::PlatformBoundaryUnavailable,
                                code: AUTOMATIC_BRIDGE_CODE.to_owned(),
                            });
                        }
                        (
                            SoloHostAssertionStatus::Failed,
                            SoloHostProofState::ForgeObserved,
                        )
                    }
                }
            } else {
                adapter_claim_result(claim.passed)
            };
            if status == SoloHostAssertionStatus::Passed {
                any_passed = true;
                if !matches!(
                    proof_state,
                    SoloHostProofState::ForgeVerified | SoloHostProofState::NativeAuthenticated
                ) {
                    every_applicable_passed_and_verified = false;
                }
            } else if status == SoloHostAssertionStatus::Failed {
                every_applicable_passed_and_verified = false;
            }
            assertions.push(SoloHostAssertionResult {
                assertion: assertion.clone(),
                status,
                proof_state,
            });
        }
        if any_passed
            && assertions.iter().any(|assertion| {
                assertion.status == SoloHostAssertionStatus::Passed
                    && assertion.proof_state == SoloHostProofState::AdapterReportedUnverified
            })
            && !gaps
                .iter()
                .any(|gap| gap.code == AUTOMATIC_AUTHENTICITY_CODE)
        {
            gaps.push(SoloHostConformanceGap {
                kind: SoloHostGapKind::NativeAuthenticityUnavailable,
                code: AUTOMATIC_AUTHENTICITY_CODE.to_owned(),
            });
        }
        gaps.sort();
        let outcome = if every_applicable_passed_and_verified && gaps.is_empty() {
            SoloHostConformanceOutcome::Supported
        } else if any_passed {
            SoloHostConformanceOutcome::PartiallySupported
        } else {
            SoloHostConformanceOutcome::Unsupported
        };
        capabilities.push(SoloHostCapabilityResult {
            capability: case.capability,
            outcome,
            assertions,
            gaps,
            artifact_paths: vec![format!("artifacts/{}.json", case.case_id)],
        });
    }

    Ok(SoloHostConformanceResultDocument {
        schema_version: SOLO_HOST_CONFORMANCE_SCHEMA_VERSION.to_owned(),
        bindings: request.bindings.clone(),
        capabilities,
        integrity_proves_authenticity: false,
        authenticity_note: SOLO_HOST_AUTHENTICITY_NOTE.to_owned(),
    })
}

fn adapter_claim_result(passed: bool) -> (SoloHostAssertionStatus, SoloHostProofState) {
    (
        if passed {
            SoloHostAssertionStatus::Passed
        } else {
            SoloHostAssertionStatus::Failed
        },
        SoloHostProofState::AdapterReportedUnverified,
    )
}

/// Digest a Solo host bundle manifest without its self-referential bundle digest.
///
/// # Errors
///
/// Returns an error when the manifest cannot be encoded as canonical JSON.
pub fn solo_host_manifest_digest(
    manifest: &SoloHostConformanceBundleManifest,
) -> Result<String, String> {
    let mut unsigned = manifest.clone();
    unsigned.bundle_digest.clear();
    canonical_json_sha256(&unsigned)
}

/// Validate the structure and self-digest of a Solo host bundle manifest.
///
/// # Errors
///
/// Returns an error when the manifest is malformed or its declared digest does not match.
pub fn validate_solo_host_manifest(
    manifest: &SoloHostConformanceBundleManifest,
) -> Result<(), String> {
    if manifest.schema_version != SOLO_HOST_CONFORMANCE_BUNDLE_VERSION {
        return Err("unsupported solo host bundle schema_version".to_owned());
    }
    validate_solo_host_bindings(&manifest.bindings)?;
    if manifest.files.is_empty() || manifest.files.len() > MAX_SOLO_HOST_FILES {
        return Err("bundle manifest has an invalid file count".to_owned());
    }
    if manifest.bundle_digest != solo_host_manifest_digest(manifest)? {
        return Err("bundle manifest digest mismatch".to_owned());
    }
    let mut paths = BTreeSet::new();
    let mut total = 0u64;
    for file in &manifest.files {
        validate_bundle_path(&file.path)?;
        if file.path == "manifest.json" || !paths.insert(file.path.as_str()) {
            return Err("manifest contains a forbidden or repeated path".to_owned());
        }
        require_sha256("manifest file digest", &file.sha256)?;
        if file.byte_length > MAX_SOLO_HOST_BUNDLE_BYTES as u64 {
            return Err(format!("manifest entry {} has invalid bounds", file.path));
        }
        total = total
            .checked_add(file.byte_length)
            .ok_or_else(|| "bundle byte total overflow".to_owned())?;
    }
    if total > MAX_SOLO_HOST_BUNDLE_BYTES as u64 {
        return Err("bundle exceeds the total byte limit".to_owned());
    }
    for (path, role) in [
        ("request.json", SoloHostBundleFileRole::ProtocolRequest),
        ("response.json", SoloHostBundleFileRole::ProtocolResponse),
        ("result.json", SoloHostBundleFileRole::DerivedResult),
    ] {
        if !manifest
            .files
            .iter()
            .any(|file| file.path == path && file.role == role)
        {
            return Err(format!("bundle is missing required file {path}"));
        }
    }
    Ok(())
}

#[must_use]
pub fn bundle_file(path: String, role: SoloHostBundleFileRole, bytes: &[u8]) -> SoloHostBundleFile {
    SoloHostBundleFile {
        path,
        role,
        sha256: sha256_digest(bytes),
        byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    }
}

fn validate_file_identity(
    field: &str,
    identity: &forge_core_contracts::SoloHostFileIdentity,
) -> Result<(), String> {
    validate_safe_file_name(field, &identity.file_name)?;
    require_sha256(field, &identity.sha256)?;
    if identity.byte_length == 0 || identity.byte_length > 1024 * 1024 * 1024 {
        return Err(format!("{field} has an invalid byte length"));
    }
    Ok(())
}

fn validate_bundle_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.len() > 240
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(format!("unsafe bundle path {path:?}"));
    }
    Ok(())
}

fn validate_safe_file_name(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || value.contains(['/', '\\'])
        || matches!(value, "." | "..")
        || value.chars().any(char::is_control)
    {
        return Err(format!("{field} is not a safe basename"));
    }
    reject_sensitive_text(field, value)
}

fn validate_token(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        return Err(format!("{field} is not a safe token"));
    }
    reject_sensitive_text(field, value)
}

fn validate_safe_label(field: &str, value: &str, max: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > max
        || value != value.trim()
        || value.chars().any(char::is_control)
        || value.contains(['/', '\\'])
        || (value.len() >= 3
            && value.as_bytes()[0].is_ascii_alphabetic()
            && value.as_bytes()[1] == b':')
    {
        return Err(format!(
            "{field} is unsafe, path-like, untrimmed, or too long"
        ));
    }
    reject_sensitive_text(field, value)
}

fn validate_public_text(field: &str, value: &str, max: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > max
        || value != value.trim()
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{field} is empty, untrimmed, controlled, or too long"
        ));
    }
    reject_sensitive_text(field, value)
}

fn reject_sensitive_text(field: &str, value: &str) -> Result<(), String> {
    let normalized = value.to_ascii_lowercase().replace(['-', '.', ':'], "_");
    let forbidden = [
        "password",
        "passwd",
        "api_key",
        "apikey",
        "access_key",
        "private_key",
        "authorization",
        "bearer_",
        "client_secret",
        "secret_value",
        "ssh_rsa",
        "sk_proj",
    ];
    if forbidden.iter().any(|pattern| normalized.contains(pattern)) {
        return Err(format!(
            "{field} contains a forbidden secret-bearing pattern"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    if !valid_sha256(value) {
        return Err(format!("{field} must be a lowercase sha256 digest"));
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}
#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{
        SoloHostAdapterInvocationBinding, SoloHostAssertionClaim, SoloHostBridgeApplicability,
        SoloHostClosedEvidence, SoloHostConformanceResponseDocument, SoloHostDeclaredBindings,
        SoloHostFileIdentity, SoloHostObservedBindings, SoloHostObservedCanonicalRoot,
        SoloHostObservedPlatform,
    };

    fn digest() -> String {
        format!("sha256:{}", "0".repeat(64))
    }

    fn request() -> SoloHostConformanceRequestDocument {
        let corpus: SoloHostConformanceCorpusDocument = serde_json::from_str(include_str!(
            "../../../contracts/hosts/solo-host-conformance-v1/corpus.json"
        ))
        .expect("corpus");
        let arguments = Vec::new();
        SoloHostConformanceRequestDocument {
            schema_version: SOLO_HOST_CONFORMANCE_PROTOCOL_VERSION.to_owned(),
            bindings: SoloHostConformanceBindings {
                declared: SoloHostDeclaredBindings {
                    host_id: "example.host".to_owned(),
                    host_version: "1.0.0".to_owned(),
                    adapter_id: "example.adapter".to_owned(),
                    adapter_version: "1.0.0".to_owned(),
                    platform_label: "example-platform".to_owned(),
                    environment_label: "isolated-test".to_owned(),
                },
                observed: SoloHostObservedBindings {
                    forge_package: "forge-core-cli".to_owned(),
                    forge_version: "0.0.0".to_owned(),
                    forge_executable_sha256: digest(),
                    platform: SoloHostObservedPlatform {
                        os: "linux".to_owned(),
                        architecture: "x86_64".to_owned(),
                    },
                    canonical_root: SoloHostObservedCanonicalRoot {
                        resolved_path_sha256: digest(),
                        kind: SoloHostCanonicalRootKind::NativeOrOther,
                        exists: true,
                        is_directory: true,
                        windows_to_wsl_bridge: SoloHostBridgeApplicability::NotApplicable,
                    },
                    adapter_invocation: SoloHostAdapterInvocationBinding {
                        executable: SoloHostFileIdentity {
                            file_name: "python3".to_owned(),
                            sha256: digest(),
                            byte_length: 1,
                        },
                        arguments: arguments.clone(),
                        argv_sha256: canonical_json_sha256(&arguments).expect("argv digest"),
                        timeout_ms: 1000,
                        output_limit_bytes: 4096,
                    },
                },
                corpus_sha256: digest(),
            },
            accepted_native_proof_schemes: Vec::new(),
            cases: corpus.cases,
        }
    }

    fn response(
        request: &SoloHostConformanceRequestDocument,
        passed: bool,
    ) -> SoloHostConformanceResponseDocument {
        SoloHostConformanceResponseDocument {
            schema_version: request.schema_version.clone(),
            bindings: request.bindings.clone(),
            cases: request
                .cases
                .iter()
                .map(|case| forge_core_contracts::SoloHostCaseObservation {
                    case_id: case.case_id.clone(),
                    assertions: case
                        .required_assertions
                        .iter()
                        .map(|assertion| {
                            (
                                assertion.clone(),
                                SoloHostAssertionClaim {
                                    passed,
                                    native_proof_claim: None,
                                },
                            )
                        })
                        .collect(),
                    gaps: Vec::new(),
                    evidence: SoloHostClosedEvidence {
                        fact_codes: vec!["z_closed_fact".to_owned(), "a_closed_fact".to_owned()],
                    },
                })
                .collect(),
        }
    }

    #[test]
    fn fabricated_all_true_adapter_can_never_be_supported() {
        let request = request();
        let result = validate_and_derive_solo_host_result(&request, &response(&request, true))
            .expect("derive");
        assert!(result.capabilities.iter().all(|capability| {
            capability.outcome == SoloHostConformanceOutcome::PartiallySupported
                && capability.gaps.iter().any(|gap| {
                    gap.kind == SoloHostGapKind::NativeAuthenticityUnavailable
                        && gap.code == AUTOMATIC_AUTHENTICITY_CODE
                })
        }));
        let root = result
            .capabilities
            .iter()
            .find(|capability| capability.capability == SoloHostCapability::CanonicalProjectRoot)
            .expect("root result");
        assert!(root.assertions.iter().any(|assertion| {
            assertion.assertion == BRIDGE_ASSERTION
                && assertion.status == SoloHostAssertionStatus::NotApplicable
                && assertion.proof_state == SoloHostProofState::ForgeObserved
        }));
    }

    #[test]
    fn all_false_adapter_is_unsupported_not_invalid() {
        let request = request();
        let result = validate_and_derive_solo_host_result(&request, &response(&request, false))
            .expect("derive");
        assert!(result
            .capabilities
            .iter()
            .all(|capability| capability.outcome == SoloHostConformanceOutcome::Unsupported));
    }

    #[test]
    fn equivalent_response_order_normalizes_to_identical_json() {
        let request = request();
        let left = response(&request, true);
        let mut right = left.clone();
        right.cases.reverse();
        for case in &mut right.cases {
            case.evidence.fact_codes.reverse();
        }
        let left = normalize_solo_host_response(&request, &left).expect("left");
        let right = normalize_solo_host_response(&request, &right).expect("right");
        assert_eq!(left, right);
        assert_eq!(
            canonical_json_sha256(&left).expect("left digest"),
            canonical_json_sha256(&right).expect("right digest")
        );
        let left_result =
            validate_and_derive_solo_host_result(&request, &left).expect("left result");
        let right_result =
            validate_and_derive_solo_host_result(&request, &right).expect("right result");
        assert_eq!(left_result, right_result);
        assert_eq!(
            canonical_json_sha256(&left_result).expect("left result digest"),
            canonical_json_sha256(&right_result).expect("right result digest")
        );
    }

    #[test]
    fn secret_like_closed_fact_is_invalid() {
        let request = request();
        let mut response = response(&request, true);
        response.cases[0]
            .evidence
            .fact_codes
            .push("client_secret_value".to_owned());
        let error = normalize_solo_host_response(&request, &response).expect_err("reject");
        assert!(error.contains("secret-bearing"));
        assert!(!error.contains("client_secret_value"));
    }

    #[test]
    fn absolute_personal_path_label_is_invalid() {
        let mut request = request();
        request.bindings.declared.environment_label = "/home/alice/private".to_owned();
        let response = response(&request, true);
        let error = normalize_solo_host_response(&request, &response).expect_err("reject");
        assert!(error.contains("path-like"));
        assert!(!error.contains("/home/alice/private"));
    }

    #[test]
    fn altered_binding_is_invalid_before_outcome_derivation() {
        let request = request();
        let mut response = response(&request, true);
        response.bindings.observed.adapter_invocation.argv_sha256 = digest();
        let error = normalize_solo_host_response(&request, &response).expect_err("reject");
        assert!(error.contains("bindings do not exactly match"));
    }
}
