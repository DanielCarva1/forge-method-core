//! Public, host-neutral solo journey conformance runner.

use crate::cli_error::ExitError;
use crate::cli_util::{command_surface_usage, emit_envelope};
use forge_core_command_surface::COMMAND_HOST_CONFORMANCE;
use forge_core_contracts::{
    CliEnvelope, SoloHostAdapterArgumentBinding, SoloHostAdapterArgumentKind,
    SoloHostAdapterInvocationBinding, SoloHostBridgeApplicability, SoloHostBundleFileRole,
    SoloHostCanonicalRootKind, SoloHostConformanceBindings, SoloHostConformanceBundleManifest,
    SoloHostConformanceCorpusDocument, SoloHostConformanceRequestDocument,
    SoloHostConformanceResponseDocument, SoloHostConformanceResultDocument,
    SoloHostDeclaredBindings, SoloHostFileIdentity, SoloHostObservedBindings,
    SoloHostObservedCanonicalRoot, SoloHostObservedPlatform, SOLO_HOST_CONFORMANCE_BUNDLE_VERSION,
    SOLO_HOST_CONFORMANCE_PROTOCOL_VERSION,
};
use forge_core_validate::{
    bundle_file, canonical_json_sha256, normalize_solo_host_response, sha256_digest,
    solo_host_manifest_digest, validate_and_derive_solo_host_result, validate_solo_host_corpus,
    validate_solo_host_manifest, MAX_SOLO_HOST_ADAPTER_TIMEOUT_MS, MAX_SOLO_HOST_BUNDLE_BYTES,
    MAX_SOLO_HOST_DIRECTORY_DEPTH, MAX_SOLO_HOST_FILES,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_BOUND_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const DEFAULT_ADAPTER_TIMEOUT_MS: u64 = 30_000;
const ADAPTER_POLL_INTERVAL_MS: u64 = 10;

const CORPUS_BYTES: &[u8] =
    include_bytes!("../../../contracts/hosts/solo-host-conformance-v1/corpus.json");
const KIT_README_BYTES: &[u8] =
    include_bytes!("../../../contracts/hosts/solo-host-conformance-v1/README.md");
const PROTOCOL_CONTRACT_BYTES: &[u8] =
    include_bytes!("../../../contracts/hosts/solo-host-conformance-v1/protocol-contract.json");
const RESPONSE_EXAMPLE_BYTES: &[u8] =
    include_bytes!("../../../contracts/hosts/solo-host-conformance-v1/response.example.json");
const REFERENCE_ADAPTER_BYTES: &[u8] =
    include_bytes!("../../../contracts/hosts/solo-host-conformance-v1/reference-adapter.py");

const EXPORTED_KIT_FILES: [(&str, &[u8]); 5] = [
    ("README.md", KIT_README_BYTES),
    ("corpus.json", CORPUS_BYTES),
    ("protocol-contract.json", PROTOCOL_CONTRACT_BYTES),
    ("response.example.json", RESPONSE_EXAMPLE_BYTES),
    ("reference-adapter.py", REFERENCE_ADAPTER_BYTES),
];

#[derive(Debug)]
enum Args {
    Corpus {
        output_dir: PathBuf,
        json: bool,
    },
    Run {
        adapter: PathBuf,
        adapter_args: Vec<String>,
        host_id: String,
        host_version: String,
        adapter_id: String,
        adapter_version: String,
        platform_label: String,
        environment_label: String,
        canonical_root: PathBuf,
        timeout_ms: u64,
        output_dir: PathBuf,
        json: bool,
    },
    Verify {
        bundle_dir: PathBuf,
        json: bool,
    },
}

pub fn run_host_conformance_command(args: &[String]) -> Result<(), ExitError> {
    if args
        .iter()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{}", command_surface_usage(&COMMAND_HOST_CONFORMANCE));
        return Ok(());
    }
    match parse_args(args)? {
        Args::Corpus { output_dir, json } => {
            let corpus = embedded_corpus().map_err(ExitError::failed)?;
            create_new_directory(&output_dir)?;
            for (name, bytes) in EXPORTED_KIT_FILES {
                write_new(&output_dir.join(name), bytes)?;
            }
            emit_envelope(CliEnvelope::ok("host-conformance.corpus", corpus), json)
        }
        Args::Run {
            adapter,
            adapter_args,
            host_id,
            host_version,
            adapter_id,
            adapter_version,
            platform_label,
            environment_label,
            canonical_root,
            timeout_ms,
            output_dir,
            json,
        } => {
            let result = run_adapter(RunInput {
                adapter,
                adapter_args,
                host_id,
                host_version,
                adapter_id,
                adapter_version,
                platform_label,
                environment_label,
                canonical_root,
                timeout_ms,
                output_dir,
            })
            .map_err(ExitError::failed)?;
            emit_envelope(CliEnvelope::ok("host-conformance.run", result), json)
        }
        Args::Verify { bundle_dir, json } => {
            let result = verify_bundle(&bundle_dir).map_err(ExitError::failed)?;
            emit_envelope(CliEnvelope::ok("host-conformance.verify", result), json)
        }
    }
}

struct RunInput {
    adapter: PathBuf,
    adapter_args: Vec<String>,
    host_id: String,
    host_version: String,
    adapter_id: String,
    adapter_version: String,
    platform_label: String,
    environment_label: String,
    canonical_root: PathBuf,
    timeout_ms: u64,
    output_dir: PathBuf,
}

fn run_adapter(input: RunInput) -> Result<SoloHostConformanceResultDocument, String> {
    let corpus = embedded_corpus()?;
    let forge_executable = std::env::current_exe()
        .map_err(|error| format!("cannot identify the running Forge executable: {error}"))?;
    let forge_identity = file_identity(&forge_executable, MAX_BOUND_FILE_BYTES)?;
    let adapter_program = resolve_program(&input.adapter)?;
    let adapter_identity = file_identity(&adapter_program, MAX_BOUND_FILE_BYTES)?;
    let (adapter_args, argument_bindings) = bind_adapter_arguments(&input.adapter_args)?;
    let (adapter_working_dir, canonical_root) = observe_canonical_root(&input.canonical_root)?;
    let bindings = SoloHostConformanceBindings {
        declared: SoloHostDeclaredBindings {
            host_id: input.host_id,
            host_version: input.host_version,
            adapter_id: input.adapter_id,
            adapter_version: input.adapter_version,
            platform_label: input.platform_label,
            environment_label: input.environment_label,
        },
        observed: SoloHostObservedBindings {
            forge_package: env!("CARGO_PKG_NAME").to_owned(),
            forge_version: env!("CARGO_PKG_VERSION").to_owned(),
            forge_executable_sha256: forge_identity.sha256,
            platform: SoloHostObservedPlatform {
                os: std::env::consts::OS.to_owned(),
                architecture: std::env::consts::ARCH.to_owned(),
            },
            canonical_root,
            adapter_invocation: SoloHostAdapterInvocationBinding {
                executable: adapter_identity,
                argv_sha256: canonical_json_sha256(&argument_bindings)?,
                arguments: argument_bindings,
                timeout_ms: input.timeout_ms,
                output_limit_bytes: MAX_SOLO_HOST_BUNDLE_BYTES as u64,
            },
        },
        corpus_sha256: sha256_digest(CORPUS_BYTES),
    };
    let request = SoloHostConformanceRequestDocument {
        schema_version: SOLO_HOST_CONFORMANCE_PROTOCOL_VERSION.to_owned(),
        bindings,
        accepted_native_proof_schemes: Vec::new(),
        cases: corpus.cases,
    };
    let request_bytes = pretty_json(&request)?;
    let captured = invoke_adapter_bounded(
        &adapter_program,
        &adapter_args,
        &adapter_working_dir,
        &request_bytes,
        input.timeout_ms,
        MAX_SOLO_HOST_BUNDLE_BYTES,
    )?;
    reject_obvious_secret_or_personal_path(&captured)?;
    let response: SoloHostConformanceResponseDocument = serde_json::from_slice(&captured)
        .map_err(|error| format!("adapter did not return one valid JSON response: {error}"))?;
    let response = normalize_solo_host_response(&request, &response)?;
    let result = validate_and_derive_solo_host_result(&request, &response)?;
    write_bundle(
        &input.output_dir,
        &request,
        &request_bytes,
        &response,
        &result,
    )?;
    verify_bundle(&input.output_dir)
}

fn invoke_adapter_bounded(
    program: &Path,
    args: &[String],
    working_dir: &Path,
    request: &[u8],
    timeout_ms: u64,
    output_limit: usize,
) -> Result<Vec<u8>, String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start the bound adapter program: {error}"))?;
    if let Err(error) = write_child_request(&mut child, request) {
        terminate_child(&mut child);
        return Err(error);
    }
    let Some(stdout) = child.stdout.take() else {
        terminate_child(&mut child);
        return Err("adapter stdout was not available".to_owned());
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_child(&mut child);
        return Err("adapter stderr was not available".to_owned());
    };
    let mut stdout_reader = Some(thread::spawn(move || {
        read_stream_bounded(stdout, output_limit)
    }));
    let mut stderr_reader = Some(thread::spawn(move || {
        read_stream_bounded(stderr, output_limit)
    }));
    let mut stdout_bytes = None;
    let mut stderr_bytes = None;
    let started = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut status = None;
    let mut stream_failed = false;

    while status.is_none() {
        if take_finished_reader(&mut stdout_reader, &mut stdout_bytes).is_err()
            || take_finished_reader(&mut stderr_reader, &mut stderr_bytes).is_err()
        {
            stream_failed = true;
            break;
        }
        match child.try_wait() {
            Ok(Some(exit)) => status = Some(exit),
            Ok(None) => {}
            Err(_) => {
                terminate_child(&mut child);
                join_reader_quietly(stdout_reader.take());
                join_reader_quietly(stderr_reader.take());
                return Err("cannot observe adapter completion".to_owned());
            }
        }
        if status.is_none() && started.elapsed() >= timeout {
            terminate_child(&mut child);
            join_reader_quietly(stdout_reader.take());
            join_reader_quietly(stderr_reader.take());
            return Err("adapter exceeded the finite execution timeout".to_owned());
        }
        if status.is_none() {
            thread::sleep(Duration::from_millis(ADAPTER_POLL_INTERVAL_MS));
        }
    }
    if stream_failed {
        terminate_child(&mut child);
        join_reader_quietly(stdout_reader.take());
        join_reader_quietly(stderr_reader.take());
        return Err("adapter output exceeded the protocol byte limit".to_owned());
    }
    let status = status.ok_or_else(|| "adapter completion was not observed".to_owned())?;
    let stdout_joined = match stdout_bytes {
        Some(bytes) => Ok(bytes),
        None => join_reader(stdout_reader.take()),
    };
    let stderr_joined = match stderr_bytes {
        Some(bytes) => Ok(bytes),
        None => join_reader(stderr_reader.take()),
    };
    let stdout_bytes = stdout_joined?;
    let _withheld_stderr = stderr_joined?;
    if !status.success() {
        return Err(
            "adapter exited unsuccessfully; stderr was withheld to avoid disclosing secrets"
                .to_owned(),
        );
    }
    Ok(stdout_bytes)
}

fn reject_obvious_secret_or_personal_path(bytes: &[u8]) -> Result<(), String> {
    let lower = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    let forbidden = [
        "password",
        "passwd",
        "api_key",
        "apikey",
        "access_key",
        "private_key",
        "authorization",
        "bearer ",
        "client_secret",
        "secret_value",
        "ssh-rsa",
        "sk-proj",
        "/home/",
        "c:\\\\users\\\\",
        "\\\\\\\\wsl$\\\\",
        "\\\\\\\\wsl.localhost\\\\",
    ];
    if forbidden.iter().any(|pattern| lower.contains(pattern)) {
        return Err(
            "adapter response contains a forbidden secret-like field or personal path pattern"
                .to_owned(),
        );
    }
    Ok(())
}

fn write_child_request(child: &mut Child, request: &[u8]) -> Result<(), String> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "adapter stdin was not available".to_owned())?;
    stdin
        .write_all(request)
        .map_err(|_| "cannot send protocol request to adapter".to_owned())?;
    drop(stdin);
    Ok(())
}

fn read_stream_bounded<R: Read>(mut reader: R, max: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| "cannot read bounded adapter output".to_owned())?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > max {
            return Err("adapter stream exceeded its byte limit".to_owned());
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

fn take_finished_reader(
    handle: &mut Option<thread::JoinHandle<Result<Vec<u8>, String>>>,
    target: &mut Option<Vec<u8>>,
) -> Result<(), String> {
    if handle.as_ref().is_some_and(thread::JoinHandle::is_finished) {
        *target = Some(join_reader(handle.take())?);
    }
    Ok(())
}

fn join_reader(
    handle: Option<thread::JoinHandle<Result<Vec<u8>, String>>>,
) -> Result<Vec<u8>, String> {
    handle
        .ok_or_else(|| "adapter stream reader was unavailable".to_owned())?
        .join()
        .map_err(|_| "adapter stream reader failed".to_owned())?
}

fn join_reader_quietly(handle: Option<thread::JoinHandle<Result<Vec<u8>, String>>>) {
    if let Some(handle) = handle {
        let _ = handle.join();
    }
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn write_bundle(
    root: &Path,
    request: &SoloHostConformanceRequestDocument,
    request_bytes: &[u8],
    response: &SoloHostConformanceResponseDocument,
    result: &SoloHostConformanceResultDocument,
) -> Result<(), String> {
    create_new_directory_string(root)?;
    let response_bytes = pretty_json(response)?;
    let result_bytes = pretty_json(result)?;
    let mut payloads = vec![
        (
            "request.json".to_owned(),
            SoloHostBundleFileRole::ProtocolRequest,
            request_bytes.to_vec(),
        ),
        (
            "response.json".to_owned(),
            SoloHostBundleFileRole::ProtocolResponse,
            response_bytes,
        ),
        (
            "result.json".to_owned(),
            SoloHostBundleFileRole::DerivedResult,
            result_bytes,
        ),
    ];
    for observation in &response.cases {
        payloads.push((
            artifact_path(&observation.case_id),
            SoloHostBundleFileRole::EvidenceArtifact,
            pretty_json(observation)?,
        ));
    }
    payloads.sort_by(|left, right| left.0.cmp(&right.0));
    let files = payloads
        .iter()
        .map(|(path, role, bytes)| bundle_file(path.clone(), *role, bytes))
        .collect();
    let mut manifest = SoloHostConformanceBundleManifest {
        schema_version: SOLO_HOST_CONFORMANCE_BUNDLE_VERSION.to_owned(),
        bindings: request.bindings.clone(),
        bundle_digest: String::new(),
        files,
    };
    manifest.bundle_digest = solo_host_manifest_digest(&manifest)?;
    validate_solo_host_manifest(&manifest)?;
    for (relative, _, bytes) in &payloads {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create bundle directory: {error}"))?;
        }
        write_new_string(&path, bytes)?;
    }
    write_new_string(&root.join("manifest.json"), &pretty_json(&manifest)?)
}

fn verify_bundle(root: &Path) -> Result<SoloHostConformanceResultDocument, String> {
    let root_metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("cannot inspect bundle root: {error}"))?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err("bundle root must be a real directory, not a link".to_owned());
    }
    let manifest_bytes =
        read_regular_bounded(&root.join("manifest.json"), MAX_SOLO_HOST_BUNDLE_BYTES)?;
    let manifest: SoloHostConformanceBundleManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("invalid bundle manifest: {error}"))?;
    validate_solo_host_manifest(&manifest)?;

    let actual = collect_bundle_files(root)?;
    let mut expected = manifest
        .files
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    expected.insert("manifest.json".to_owned());
    if actual != expected {
        return Err("bundle contains missing or extra files".to_owned());
    }
    for entry in &manifest.files {
        let bytes = read_regular_bounded(&root.join(&entry.path), MAX_SOLO_HOST_BUNDLE_BYTES)?;
        if entry.byte_length != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
            || entry.sha256 != sha256_digest(&bytes)
        {
            return Err(format!("bundle file {} failed digest readback", entry.path));
        }
    }

    let request: SoloHostConformanceRequestDocument = read_json_file(&root.join("request.json"))?;
    let response: SoloHostConformanceResponseDocument =
        read_json_file(&root.join("response.json"))?;
    let recorded: SoloHostConformanceResultDocument = read_json_file(&root.join("result.json"))?;
    let public_corpus = embedded_corpus()?;
    if request.cases != public_corpus.cases
        || request.bindings != manifest.bindings
        || request.bindings.corpus_sha256 != sha256_digest(CORPUS_BYTES)
    {
        return Err("bundle bindings do not match the public corpus and manifest".to_owned());
    }
    let response = normalize_solo_host_response(&request, &response)?;
    let expected_payloads = ["request.json", "response.json", "result.json"]
        .into_iter()
        .map(str::to_owned)
        .chain(
            response
                .cases
                .iter()
                .map(|observation| artifact_path(&observation.case_id)),
        )
        .collect::<BTreeSet<_>>();
    let manifested_payloads = manifest
        .files
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    if expected_payloads != manifested_payloads {
        return Err("manifest payloads do not exactly match the normalized response".to_owned());
    }
    for observation in &response.cases {
        let path = artifact_path(&observation.case_id);
        let entry = manifest
            .files
            .iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| format!("artifact {path} is not manifested"))?;
        if entry.role != SoloHostBundleFileRole::EvidenceArtifact
            || read_regular_bounded(&root.join(&path), MAX_SOLO_HOST_BUNDLE_BYTES)?
                != pretty_json(observation)?
        {
            return Err(format!(
                "artifact {path} does not match the closed response"
            ));
        }
    }
    let derived = validate_and_derive_solo_host_result(&request, &response)?;
    if derived != recorded {
        return Err("recorded result does not match Forge-owned derivation".to_owned());
    }
    Ok(derived)
}

fn artifact_path(case_id: &str) -> String {
    format!("artifacts/{case_id}.json")
}

fn embedded_corpus() -> Result<SoloHostConformanceCorpusDocument, String> {
    let corpus = serde_json::from_slice(CORPUS_BYTES)
        .map_err(|error| format!("embedded host corpus is invalid JSON: {error}"))?;
    validate_solo_host_corpus(&corpus)?;
    Ok(corpus)
}

fn pretty_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot serialize conformance JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn read_json_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = read_regular_bounded(path, MAX_SOLO_HOST_BUNDLE_BYTES)?;
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid bundle JSON: {error}"))
}

fn read_regular_bounded(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let (mut file, metadata) = open_regular_once(path, max as u64, true)?;
    let capacity = usize::try_from(metadata.len()).unwrap_or(max).min(max);
    let mut bytes = Vec::with_capacity(capacity);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read bundle file: {error}"))?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > max {
            return Err("bundle file exceeds the byte limit while reading".to_owned());
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

fn file_identity(path: &Path, max: u64) -> Result<SoloHostFileIdentity, String> {
    // Installed executables may legitimately have more than one hard-link name.
    // Their exact bytes are bound below; only evidence-bundle files forbid
    // hard links because those files must be self-contained.
    let (mut file, metadata) = open_regular_once(path, max, false)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot hash bound file: {error}"))?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| "bound file exceeds the byte limit".to_owned())?;
        if total > max {
            return Err("bound file exceeds the byte limit".to_owned());
        }
        hasher.update(&buffer[..read]);
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "bound file has no safe UTF-8 basename".to_owned())?
        .to_owned();
    Ok(SoloHostFileIdentity {
        file_name,
        sha256: format!("sha256:{:x}", hasher.finalize()),
        byte_length: metadata.len(),
    })
}

fn open_regular_once(
    path: &Path,
    max: u64,
    reject_multiple_links: bool,
) -> Result<(File, fs::Metadata), String> {
    let initial = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect bound regular file: {error}"))?;
    if initial.file_type().is_symlink() {
        return Err("bound file must not be a symbolic link".to_owned());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("cannot open bound regular file: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened regular file: {error}"))?;
    if !metadata.is_file() {
        return Err("bound path must be a regular file".to_owned());
    }
    if reject_multiple_links {
        reject_hardlink(&metadata)?;
    }
    if metadata.len() == 0 || metadata.len() > max {
        return Err("bound regular file is empty or exceeds the byte limit".to_owned());
    }
    Ok((file, metadata))
}

#[cfg(unix)]
fn reject_hardlink(metadata: &fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() > 1 {
        return Err("hard-linked files are forbidden in conformance evidence".to_owned());
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_hardlink(_metadata: &fs::Metadata) -> Result<(), String> {
    // Rust's portable Metadata API does not expose link count on every target.
    Ok(())
}

fn resolve_program(program: &Path) -> Result<PathBuf, String> {
    if program.is_absolute() || program.components().count() > 1 {
        return fs::canonicalize(program)
            .map_err(|error| format!("cannot resolve the adapter program: {error}"));
    }
    let path = std::env::var_os("PATH").ok_or_else(|| "PATH is unavailable".to_owned())?;
    #[cfg(not(windows))]
    let names = vec![program.to_path_buf()];
    #[cfg(windows)]
    let names = {
        let mut names = vec![program.to_path_buf()];
        if program.extension().is_none() {
            for extension in std::env::var_os("PATHEXT")
                .unwrap_or_else(|| ".EXE;.CMD;.BAT;.COM".into())
                .to_string_lossy()
                .split(';')
            {
                names.push(PathBuf::from(format!(
                    "{}{}",
                    program.to_string_lossy(),
                    extension.to_ascii_lowercase()
                )));
            }
        }
        names
    };
    for directory in std::env::split_paths(&path) {
        for name in &names {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return fs::canonicalize(&candidate)
                    .map_err(|error| format!("cannot resolve the adapter program: {error}"));
            }
        }
    }
    Err("adapter program was not found on PATH".to_owned())
}

fn bind_adapter_arguments(
    args: &[String],
) -> Result<(Vec<String>, Vec<SoloHostAdapterArgumentBinding>), String> {
    if args.len() > 32 {
        return Err("adapter has too many separated arguments".to_owned());
    }
    let mut invocation_args = Vec::with_capacity(args.len());
    let mut bindings = Vec::with_capacity(args.len());
    for (position, argument) in args.iter().enumerate() {
        let candidate = Path::new(argument);
        let (invocation_arg, kind, safe_display, file_identity) = if candidate.exists() {
            let resolved = fs::canonicalize(candidate)
                .map_err(|error| format!("cannot resolve adapter file argument: {error}"))?;
            let identity = file_identity(&resolved, MAX_BOUND_FILE_BYTES)?;
            let invocation_arg = resolved
                .to_str()
                .ok_or_else(|| "adapter file argument is not valid UTF-8".to_owned())?
                .to_owned();
            (
                invocation_arg,
                SoloHostAdapterArgumentKind::File,
                identity.file_name.clone(),
                Some(identity),
            )
        } else {
            (
                argument.clone(),
                SoloHostAdapterArgumentKind::LiteralDigest,
                "<literal-digest>".to_owned(),
                None,
            )
        };
        let argument_sha256 = sha256_digest(invocation_arg.as_bytes());
        invocation_args.push(invocation_arg);
        bindings.push(SoloHostAdapterArgumentBinding {
            position: u32::try_from(position)
                .map_err(|_| "adapter argument index overflow".to_owned())?,
            kind,
            safe_display,
            argument_sha256,
            file_identity,
        });
    }
    Ok((invocation_args, bindings))
}
fn observe_canonical_root(root: &Path) -> Result<(PathBuf, SoloHostObservedCanonicalRoot), String> {
    let resolved = fs::canonicalize(root)
        .map_err(|error| format!("cannot resolve the canonical project root: {error}"))?;
    let metadata = fs::metadata(&resolved)
        .map_err(|error| format!("cannot inspect the canonical project root: {error}"))?;
    if !metadata.is_dir() {
        return Err("canonical project root must be an existing directory".to_owned());
    }
    let display = resolved.to_string_lossy();
    let lower = display.to_ascii_lowercase();
    let kind = if lower.starts_with(r"\\wsl$\") || lower.starts_with(r"\\wsl.localhost\") {
        SoloHostCanonicalRootKind::WslNetworkShare
    } else {
        SoloHostCanonicalRootKind::NativeOrOther
    };
    let windows_to_wsl_bridge = match (std::env::consts::OS, kind) {
        ("windows", SoloHostCanonicalRootKind::WslNetworkShare) => {
            SoloHostBridgeApplicability::Applicable
        }
        ("windows", SoloHostCanonicalRootKind::NativeOrOther) | ("linux" | "macos", _) => {
            SoloHostBridgeApplicability::NotApplicable
        }
        _ => SoloHostBridgeApplicability::Indeterminate,
    };
    let observation = SoloHostObservedCanonicalRoot {
        resolved_path_sha256: sha256_digest(display.as_bytes()),
        kind,
        exists: true,
        is_directory: true,
        windows_to_wsl_bridge,
    };
    Ok((resolved, observation))
}

fn collect_bundle_files(root: &Path) -> Result<BTreeSet<String>, String> {
    fn visit(
        root: &Path,
        dir: &Path,
        depth: usize,
        entries_seen: &mut usize,
        found: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        if depth > MAX_SOLO_HOST_DIRECTORY_DEPTH {
            return Err("bundle directory depth exceeds the closed limit".to_owned());
        }
        for entry in
            fs::read_dir(dir).map_err(|error| format!("cannot enumerate bundle: {error}"))?
        {
            *entries_seen = entries_seen.saturating_add(1);
            if *entries_seen > MAX_SOLO_HOST_FILES * 2 {
                return Err("bundle has too many directory entries".to_owned());
            }
            let entry = entry.map_err(|error| format!("cannot enumerate bundle: {error}"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("cannot inspect bundle entry: {error}"))?;
            if metadata.file_type().is_symlink() {
                return Err("bundle contains a forbidden symbolic link".to_owned());
            }
            if metadata.is_dir() {
                visit(root, &path, depth + 1, entries_seen, found)?;
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| "bundle path escaped its root".to_owned())?
                    .to_string_lossy()
                    .replace('\\', "/");
                if !found.insert(relative) || found.len() > MAX_SOLO_HOST_FILES + 1 {
                    return Err("bundle has repeated or too many files".to_owned());
                }
            } else {
                return Err("bundle contains a non-regular entry".to_owned());
            }
        }
        Ok(())
    }
    let mut found = BTreeSet::new();
    let mut entries_seen = 0;
    visit(root, root, 0, &mut entries_seen, &mut found)?;
    Ok(found)
}

fn create_new_directory(path: &Path) -> Result<(), ExitError> {
    create_new_directory_string(path).map_err(ExitError::failed)
}

fn create_new_directory_string(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err("output directory already exists; refusing to mix evidence".to_owned());
    }
    fs::create_dir(path).map_err(|error| format!("cannot create output directory: {error}"))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), ExitError> {
    write_new_string(path, bytes).map_err(ExitError::failed)
}

fn write_new_string(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create evidence file: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write evidence file: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync evidence file: {error}"))
}

fn parse_args(args: &[String]) -> Result<Args, ExitError> {
    let usage = || ExitError::usage(command_surface_usage(&COMMAND_HOST_CONFORMANCE));
    let subcommand = args.get(1).map(String::as_str).ok_or_else(usage)?;
    let mut json = false;
    let mut values = std::collections::BTreeMap::<&str, String>::new();
    let mut adapter_args = Vec::new();
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--no-json" => json = false,
            flag @ ("--output-dir" | "--bundle-dir" | "--adapter" | "--host-id"
            | "--host-version" | "--adapter-id" | "--adapter-version" | "--platform-id"
            | "--environment-id" | "--canonical-root" | "--timeout-ms") => {
                index += 1;
                let value = args
                    .get(index)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(usage)?;
                if values.insert(flag, value.clone()).is_some() {
                    return Err(usage());
                }
            }
            "--adapter-arg" => {
                index += 1;
                adapter_args.push(args.get(index).cloned().ok_or_else(usage)?);
            }
            _ => return Err(usage()),
        }
        index += 1;
    }
    let take = |values: &mut std::collections::BTreeMap<&str, String>, flag| {
        values.remove(flag).ok_or_else(usage)
    };
    let parsed = match subcommand {
        "corpus" if adapter_args.is_empty() => Args::Corpus {
            output_dir: PathBuf::from(take(&mut values, "--output-dir")?),
            json,
        },
        "verify" if adapter_args.is_empty() => Args::Verify {
            bundle_dir: PathBuf::from(take(&mut values, "--bundle-dir")?),
            json,
        },
        "run" => {
            let timeout_ms = values
                .remove("--timeout-ms")
                .map(|value| value.parse::<u64>().map_err(|_| usage()))
                .transpose()?
                .unwrap_or(DEFAULT_ADAPTER_TIMEOUT_MS);
            if timeout_ms == 0 || timeout_ms > MAX_SOLO_HOST_ADAPTER_TIMEOUT_MS {
                return Err(usage());
            }
            Args::Run {
                adapter: PathBuf::from(take(&mut values, "--adapter")?),
                adapter_args,
                host_id: take(&mut values, "--host-id")?,
                host_version: take(&mut values, "--host-version")?,
                adapter_id: take(&mut values, "--adapter-id")?,
                adapter_version: take(&mut values, "--adapter-version")?,
                platform_label: take(&mut values, "--platform-id")?,
                environment_label: take(&mut values, "--environment-id")?,
                canonical_root: PathBuf::from(take(&mut values, "--canonical-root")?),
                timeout_ms,
                output_dir: PathBuf::from(take(&mut values, "--output-dir")?),
                json,
            }
        }
        _ => return Err(usage()),
    };
    if values.is_empty() {
        Ok(parsed)
    } else {
        Err(usage())
    }
}
