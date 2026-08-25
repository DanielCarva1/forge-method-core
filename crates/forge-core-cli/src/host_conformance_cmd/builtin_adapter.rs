//! Built-in protocol adapters executed through the same bounded process seam as external adapters.

use super::read_regular_bounded;
use crate::cli_error::ExitError;
use forge_core_contracts::{
    SoloHostAssertionClaim, SoloHostCaseObservation, SoloHostClosedEvidence,
    SoloHostConformanceCorpusDocument, SoloHostConformanceGap, SoloHostConformanceRequestDocument,
    SoloHostConformanceResponseDocument, SoloHostGapKind, SOLO_HOST_CONFORMANCE_PROTOCOL_VERSION,
    SOLO_HOST_CONFORMANCE_SCHEMA_VERSION,
};
use forge_core_validate::{validate_solo_host_corpus, MAX_SOLO_HOST_BUNDLE_BYTES};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

const REFERENCE_ADAPTER_ID: &str = "forge.reference.protocol-only";
const CODEX_ADAPTER_ID: &str = "forge.codex.cooperative";
const BUILTIN_ADAPTER_VERSION: &str = "1.0.0";
const CODEX_OBSERVATION_VERSION: &str = "forge_codex_host_observation_v1";
const MAX_CODEX_OBSERVATION_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Reference,
    Codex,
}

impl Kind {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "reference" => Some(Self::Reference),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Codex => "codex",
        }
    }

    pub(super) const fn adapter_id(self) -> &'static str {
        match self {
            Self::Reference => REFERENCE_ADAPTER_ID,
            Self::Codex => CODEX_ADAPTER_ID,
        }
    }

    pub(super) const fn adapter_version() -> &'static str {
        BUILTIN_ADAPTER_VERSION
    }

    pub(super) const fn requires_observation_file(self) -> bool {
        matches!(self, Self::Codex)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceMode {
    Reference,
    Fabricated,
    Unsupported,
    Timeout,
    Oversize,
    UnsafeSecret,
    UnsafePath,
    Reversed,
}

impl ReferenceMode {
    fn parse(value: Option<&String>) -> Option<Self> {
        match value.map_or("reference", String::as_str) {
            "reference" => Some(Self::Reference),
            "fabricated" => Some(Self::Fabricated),
            "unsupported" => Some(Self::Unsupported),
            "timeout" => Some(Self::Timeout),
            "oversize" => Some(Self::Oversize),
            "unsafe-secret" => Some(Self::UnsafeSecret),
            "unsafe-path" => Some(Self::UnsafePath),
            "reversed" => Some(Self::Reversed),
            _ => None,
        }
    }
}

pub(super) fn run_process(args: &[String]) -> Result<(), ExitError> {
    let kind = args
        .first()
        .and_then(|value| Kind::parse(value))
        .ok_or_else(|| ExitError::failed("built-in adapter invocation is invalid"))?;
    match kind {
        Kind::Reference => run_reference(&args[1..]),
        Kind::Codex => run_codex(&args[1..]),
    }
    .map_err(ExitError::failed)
}

fn run_reference(args: &[String]) -> Result<(), String> {
    if args.len() > 1 {
        return Err("built-in reference adapter invocation is invalid".to_owned());
    }
    let mode = ReferenceMode::parse(args.first())
        .ok_or_else(|| "built-in reference adapter mode is invalid".to_owned())?;
    if mode == ReferenceMode::Timeout {
        thread::sleep(Duration::from_secs(5));
        return Ok(());
    }
    if mode == ReferenceMode::Oversize {
        let byte_count = u64::try_from(MAX_SOLO_HOST_BUNDLE_BYTES)
            .map_err(|_| "adapter output limit cannot be represented".to_owned())?
            .saturating_add(1);
        io::copy(&mut io::repeat(b'x').take(byte_count), &mut io::stdout())
            .map_err(|error| format!("cannot write oversized adapter fixture: {error}"))?;
        return Ok(());
    }

    let request = read_request()?;
    validate_request(&request, Kind::Reference)?;
    let passed = mode != ReferenceMode::Unsupported;
    let mut cases = request
        .cases
        .iter()
        .map(|case| {
            let gaps = match mode {
                ReferenceMode::Reference => vec![SoloHostConformanceGap {
                    kind: SoloHostGapKind::NativeAuthenticityUnavailable,
                    code: "reference_adapter_only".to_owned(),
                }],
                ReferenceMode::Unsupported => vec![SoloHostConformanceGap {
                    kind: SoloHostGapKind::AdapterFailure,
                    code: "example_host_action_unavailable".to_owned(),
                }],
                _ => Vec::new(),
            };
            let mut fact_codes = vec![
                "closed_example_observation".to_owned(),
                format!("case_{}", case.case_id.replace('-', "_")),
            ];
            if mode == ReferenceMode::UnsafeSecret {
                fact_codes.push("client_secret_value".to_owned());
            }
            if mode == ReferenceMode::UnsafePath {
                fact_codes.push("/home/alice/private".to_owned());
            }
            SoloHostCaseObservation {
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
                gaps,
                evidence: SoloHostClosedEvidence { fact_codes },
            }
        })
        .collect::<Vec<_>>();
    if mode == ReferenceMode::Reversed {
        cases.reverse();
        for case in &mut cases {
            case.evidence.fact_codes.reverse();
        }
    }
    write_response(&SoloHostConformanceResponseDocument {
        schema_version: request.schema_version.clone(),
        bindings: request.bindings,
        cases,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodexObservation {
    schema_version: String,
    evidence_mode: CodexEvidenceMode,
    cases: Vec<CodexObservationCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CodexEvidenceMode {
    CooperativeSameOwner,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodexObservationCase {
    case_id: String,
    assertions: BTreeMap<String, bool>,
    gaps: Vec<SoloHostConformanceGap>,
    fact_codes: Vec<String>,
}

fn run_codex(args: &[String]) -> Result<(), String> {
    let [flag, observation_file] = args else {
        return Err("built-in Codex adapter invocation is invalid".to_owned());
    };
    if flag != "--observation-file" {
        return Err("built-in Codex adapter invocation is invalid".to_owned());
    }
    let request = read_request()?;
    validate_request(&request, Kind::Codex)?;
    let observation = read_codex_observation(Path::new(observation_file))?;
    let observations = validate_codex_observation(observation, &request)?;
    let cases = request
        .cases
        .iter()
        .map(|case| {
            let observation = observations
                .get(&case.case_id)
                .expect("validated observation contains every request case");
            SoloHostCaseObservation {
                case_id: case.case_id.clone(),
                assertions: case
                    .required_assertions
                    .iter()
                    .map(|assertion| {
                        (
                            assertion.clone(),
                            SoloHostAssertionClaim {
                                passed: observation.assertions[assertion],
                                native_proof_claim: None,
                            },
                        )
                    })
                    .collect(),
                gaps: observation.gaps.clone(),
                evidence: SoloHostClosedEvidence {
                    fact_codes: observation.fact_codes.clone(),
                },
            }
        })
        .collect();
    write_response(&SoloHostConformanceResponseDocument {
        schema_version: request.schema_version.clone(),
        bindings: request.bindings,
        cases,
    })
}

fn read_request() -> Result<SoloHostConformanceRequestDocument, String> {
    serde_json::from_reader(io::stdin().lock())
        .map_err(|_| "built-in adapter request was rejected".to_owned())
}

fn validate_request(
    request: &SoloHostConformanceRequestDocument,
    kind: Kind,
) -> Result<(), String> {
    if request.schema_version != SOLO_HOST_CONFORMANCE_PROTOCOL_VERSION
        || request.bindings.declared.adapter_id != kind.adapter_id()
        || request.bindings.declared.adapter_version != Kind::adapter_version()
        || request.cases.len() != 8
    {
        return Err("built-in adapter request was rejected".to_owned());
    }
    validate_solo_host_corpus(&SoloHostConformanceCorpusDocument {
        schema_version: SOLO_HOST_CONFORMANCE_SCHEMA_VERSION.to_owned(),
        corpus_id: "built-in-adapter-request".to_owned(),
        cases: request.cases.clone(),
    })
    .map_err(|_| "built-in adapter request was rejected".to_owned())
}

fn read_codex_observation(path: &Path) -> Result<CodexObservation, String> {
    let bytes = read_regular_bounded(path, MAX_CODEX_OBSERVATION_BYTES)
        .map_err(|_| "Codex observation file was rejected".to_owned())?;
    serde_json::from_slice(&bytes).map_err(|_| "Codex observation file was rejected".to_owned())
}

fn validate_codex_observation(
    observation: CodexObservation,
    request: &SoloHostConformanceRequestDocument,
) -> Result<BTreeMap<String, CodexObservationCase>, String> {
    if observation.schema_version != CODEX_OBSERVATION_VERSION
        || observation.cases.len() != request.cases.len()
    {
        return Err("Codex observation was rejected".to_owned());
    }
    let CodexEvidenceMode::CooperativeSameOwner = observation.evidence_mode;
    let mut by_id = BTreeMap::new();
    for case in observation.cases {
        validate_token(&case.case_id)?;
        if case.gaps.len() > 16 || case.fact_codes.is_empty() || case.fact_codes.len() > 32 {
            return Err("Codex observation was rejected".to_owned());
        }
        let mut gap_keys = BTreeSet::new();
        for gap in &case.gaps {
            validate_token(&gap.code)?;
            if !gap_keys.insert((gap.kind, gap.code.as_str())) {
                return Err("Codex observation was rejected".to_owned());
            }
        }
        let mut fact_codes = BTreeSet::new();
        for fact in &case.fact_codes {
            validate_token(fact)?;
            if !fact_codes.insert(fact.as_str()) {
                return Err("Codex observation was rejected".to_owned());
            }
        }
        for assertion in case.assertions.keys() {
            validate_token(assertion)?;
        }
        if case.assertions.values().any(|passed| !passed) && case.gaps.is_empty() {
            return Err("Codex observation was rejected".to_owned());
        }
        if by_id.insert(case.case_id.clone(), case).is_some() {
            return Err("Codex observation was rejected".to_owned());
        }
    }
    for request_case in &request.cases {
        let Some(observation_case) = by_id.get(&request_case.case_id) else {
            return Err("Codex observation was rejected".to_owned());
        };
        let expected = request_case
            .required_assertions
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let actual = observation_case
            .assertions
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err("Codex observation was rejected".to_owned());
        }
    }
    Ok(by_id)
}

fn validate_token(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        return Err("Codex observation was rejected".to_owned());
    }
    Ok(())
}

fn write_response(response: &SoloHostConformanceResponseDocument) -> Result<(), String> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, response)
        .map_err(|error| format!("cannot serialize built-in adapter response: {error}"))?;
    output
        .write_all(b"\n")
        .map_err(|error| format!("cannot write built-in adapter response: {error}"))
}
