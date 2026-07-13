//! Read-only agent surface for P6a Domain Pack validation and composition.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use forge_core_command_surface::COMMAND_DOMAIN_PACK;
use forge_core_contracts::{
    CliEnvelope, DomainPackArtifactBinding, DomainPackCandidateAuthority, DomainPackCandidateInput,
    DomainPackCompositionIssue, DomainPackCompositionRequestDocument, DomainPackContentDocument,
    DomainPackManifestDocument, RepoPath,
};
use forge_core_decisions::{
    compose_domain_packs, validate_domain_pack_candidate, DomainPackCandidateMaterial,
    MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES,
};
use sha2::{Digest, Sha256};

use crate::cli_error::ExitError;

#[derive(Debug, serde::Serialize)]
struct DomainPackValidationPayload {
    authority: DomainPackCandidateAuthority,
    structurally_valid: bool,
    publisher: String,
    name: String,
    version: String,
    issues: Vec<DomainPackCompositionIssue>,
    boundary: &'static str,
}

/// Dispatch `forge-core domain-pack` without performing any writes.
pub fn run_domain_pack_command(args: &[String]) -> Result<(), ExitError> {
    match args.get(1).map_or("--help", String::as_str) {
        "validate" => run_validate(&args[2..]),
        "compose" => run_compose(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core domain-pack: unknown subcommand '{other}'\n{}",
            usage()
        ))),
    }
}

fn run_validate(args: &[String]) -> Result<(), ExitError> {
    let mut manifest_file: Option<PathBuf> = None;
    let mut content_file: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut forge_core_version = env!("CARGO_PKG_VERSION").to_owned();
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--manifest-file" => {
                manifest_file = Some(PathBuf::from(value));
                true
            }
            "--content-file" => {
                content_file = Some(PathBuf::from(value));
                true
            }
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            "--forge-core-version" => {
                forge_core_version = value.to_owned();
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let manifest_file = manifest_file.ok_or_else(|| ExitError::usage(usage()))?;
    let content_file = content_file.ok_or_else(|| ExitError::usage(usage()))?;
    let manifest_raw = read(&manifest_file, "manifest")?;
    let content_raw = read(&content_file, "content")?;
    let manifest: DomainPackManifestDocument = parse(&manifest_raw, &manifest_file)?;
    let content: DomainPackContentDocument = parse(&content_raw, &content_file)?;
    let license_path = safe_join(
        &artifact_root,
        &manifest
            .domain_pack_manifest
            .provenance
            .license_text
            .artifact_ref
            .0,
    )?;
    let license_raw = read(&license_path, "license artifact")?;
    let identity = manifest.domain_pack_manifest.identity.clone();
    let candidate = DomainPackCandidateInput {
        // Standalone validation receives the authored manifest as an explicit
        // CLI input rather than through a composition request. Bind those exact
        // bytes and their closed typed semantics before handing the candidate
        // to the pure validator.
        manifest_binding: DomainPackArtifactBinding {
            artifact_ref: RepoPath("manifest-input.yaml".to_owned()),
            raw_sha256: sha256_bytes(&manifest_raw),
            canonical_sha256: canonical_digest(&manifest)?,
        },
        manifest,
        content,
    };
    let material = DomainPackCandidateMaterial {
        publisher: &identity.publisher.0,
        name: &identity.name.0,
        version: &identity.version,
        manifest_raw: &manifest_raw,
        content_raw: &content_raw,
        license_raw: &license_raw,
    };
    let issues = validate_domain_pack_candidate(&candidate, &material, &forge_core_version);
    let payload = DomainPackValidationPayload {
        authority: DomainPackCandidateAuthority::CandidateOnly,
        structurally_valid: issues.is_empty(),
        publisher: identity.publisher.0,
        name: identity.name.0,
        version: identity.version,
        issues,
        boundary: "candidate_only; no install, trust, activation, execution, or mutation authority",
    };
    crate::cli_util::emit_envelope(CliEnvelope::ok("domain-pack validate", payload), want_json)
}

fn run_compose(args: &[String]) -> Result<(), ExitError> {
    let mut request_file: Option<PathBuf> = None;
    let mut artifact_root = PathBuf::from(".");
    let mut want_json = true;
    parse_flags(
        args,
        |flag, value| match flag {
            "--request-file" => {
                request_file = Some(PathBuf::from(value));
                true
            }
            "--artifact-root" => {
                artifact_root = PathBuf::from(value);
                true
            }
            _ => false,
        },
        &mut want_json,
    )?;
    let request_file = request_file.ok_or_else(|| ExitError::usage(usage()))?;
    let request_raw = read(&request_file, "composition request")?;
    let request: DomainPackCompositionRequestDocument = parse(&request_raw, &request_file)?;

    struct OwnedMaterial {
        manifest: Vec<u8>,
        content: Vec<u8>,
        license: Vec<u8>,
    }
    let owned = request
        .domain_pack_composition_request
        .candidates
        .iter()
        .map(|candidate| {
            let manifest = &candidate.manifest.domain_pack_manifest;
            let manifest_path =
                safe_join(&artifact_root, &candidate.manifest_binding.artifact_ref.0)?;
            let content_path = safe_join(&artifact_root, &manifest.content.content_ref.0)?;
            let license_path = safe_join(
                &artifact_root,
                &manifest.provenance.license_text.artifact_ref.0,
            )?;
            Ok(OwnedMaterial {
                manifest: read(&manifest_path, "pack manifest")?,
                content: read(&content_path, "pack content")?,
                license: read(&license_path, "license artifact")?,
            })
        })
        .collect::<Result<Vec<_>, ExitError>>()?;
    let materials = request
        .domain_pack_composition_request
        .candidates
        .iter()
        .zip(&owned)
        .map(|(candidate, owned)| {
            let identity = &candidate.manifest.domain_pack_manifest.identity;
            DomainPackCandidateMaterial {
                publisher: &identity.publisher.0,
                name: &identity.name.0,
                version: &identity.version,
                manifest_raw: &owned.manifest,
                content_raw: &owned.content,
                license_raw: &owned.license,
            }
        })
        .collect::<Vec<_>>();
    let projection = compose_domain_packs(&request, &materials);
    crate::cli_util::emit_envelope(
        CliEnvelope::ok("domain-pack compose", projection),
        want_json,
    )
}

fn parse_flags(
    args: &[String],
    mut set_value: impl FnMut(&str, &str) -> bool,
    want_json: &mut bool,
) -> Result<(), ExitError> {
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => *want_json = true,
            "--no-json" | "--text" => *want_json = false,
            "--help" | "-h" => return Err(ExitError::usage(usage())),
            flag if flag.starts_with("--") => {
                index += 1;
                let value = args
                    .get(index)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| ExitError::usage(usage()))?;
                if !set_value(flag, value) {
                    return Err(ExitError::usage(usage()));
                }
            }
            _ => return Err(ExitError::usage(usage())),
        }
        index += 1;
    }
    Ok(())
}

fn read(path: &Path, label: &str) -> Result<Vec<u8>, ExitError> {
    let file = std::fs::File::open(path).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot read {label} '{}': {error}",
            path.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot inspect {label} '{}': {error}",
            path.display()
        ))
    })?;
    if metadata.len() > MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES as u64 {
        return Err(document_too_large(path, label, metadata.len()));
    }

    // Metadata is only an early rejection. The capped reader also closes the
    // race where a regular file grows between metadata and the read, and it
    // bounds streams whose metadata does not expose a useful byte length.
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES)
            .min(MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES),
    );
    file.take((MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ExitError::failed(format!(
                "domain-pack: cannot read {label} '{}': {error}",
                path.display()
            ))
        })?;
    if bytes.len() > MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES {
        return Err(document_too_large(path, label, bytes.len() as u64));
    }
    Ok(bytes)
}

fn document_too_large(path: &Path, label: &str, observed: u64) -> ExitError {
    ExitError::failed(format!(
        "domain-pack: {label} '{}' is {observed} bytes and exceeds maximum {} bytes",
        path.display(),
        MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES
    ))
}

fn canonical_digest<T: serde::Serialize>(value: &T) -> Result<String, ExitError> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot canonicalize manifest input: {error}"
        ))
    })?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn parse<T: serde::de::DeserializeOwned>(raw: &[u8], path: &Path) -> Result<T, ExitError> {
    let text = std::str::from_utf8(raw).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: '{}' is not UTF-8: {error}",
            path.display()
        ))
    })?;
    yaml_serde::from_str(text).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: '{}' is not a closed typed document: {error}",
            path.display()
        ))
    })
}

fn safe_join(root: &Path, reference: &str) -> Result<PathBuf, ExitError> {
    let reference = Path::new(reference);
    if reference.is_absolute()
        || reference.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ExitError::failed(format!(
            "domain-pack: artifact ref must remain relative to --artifact-root: {}",
            reference.display()
        )));
    }
    let canonical_root = std::fs::canonicalize(root).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve --artifact-root '{}': {error}",
            root.display()
        ))
    })?;
    let candidate = root.join(reference);
    let canonical_candidate = std::fs::canonicalize(&candidate).map_err(|error| {
        ExitError::failed(format!(
            "domain-pack: cannot resolve artifact '{}' below --artifact-root '{}': {error}",
            candidate.display(),
            root.display()
        ))
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(ExitError::failed(format!(
            "domain-pack: artifact ref escapes canonical --artifact-root: {}",
            reference.display()
        )));
    }
    Ok(canonical_candidate)
}

fn usage() -> String {
    let mut output = String::from("usage:");
    for line in COMMAND_DOMAIN_PACK.usage_lines {
        output.push('\n');
        output.push_str("  ");
        output.push_str(line.trim_start());
    }
    output.push_str("\n  both subcommands are read-only and emit candidate-only results");
    output
}
