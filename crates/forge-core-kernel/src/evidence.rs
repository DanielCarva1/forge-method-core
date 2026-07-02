//! Command execution and evidence recording.
//!
//! [`run_staged_read_only_command`] spawns a validated read-only command and
//! captures its result; [`command_execution_evidence_record`] turns the result
//! into the durable NDJSON evidence record appended to the evidence log.

use super::*;

#[derive(Debug, Clone, Copy)]
pub struct CommandExecutionContext<'a> {
    pub repo_root: &'a Path,
    pub project_root: &'a Path,
    pub package_root: &'a Path,
}

impl<'a> CommandExecutionContext<'a> {
    #[must_use]
    pub fn single_root(root: &'a Path) -> Self {
        Self {
            repo_root: root,
            project_root: root,
            package_root: root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCommandExecution {
    pub status: RuntimeCommandExecutionStatus,
    pub command_id: StableId,
    pub executor: CommandExecutor,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub reasons: Vec<RuntimeCommandExecutionReason>,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCommandEvidenceRecord {
    pub schema_version: String,
    pub record_kind: RuntimeEvidenceKind,
    pub recorded_at: String,
    pub operation_id: StableId,
    pub command_id: StableId,
    pub executor: CommandExecutor,
    pub status: RuntimeCommandExecutionStatus,
    pub reasons: Vec<RuntimeCommandExecutionReason>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEvidenceKind {
    CommandExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCommandExecutionStatus {
    Succeeded,
    Failed,
    TimedOut,
    Blocked,
    NotRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCommandExecutionReason {
    StagingPlanNotStaged,
    CommandNotStaged,
    CommandValidationErrors,
    NonReadOnlyCommand,
    UnsafeCommandSafetyFlags,
    NetworkNotDisabled,
    ShellExecutorBlocked,
    UnsupportedPlatform,
    TimeoutMustBePositive,
    RequiredEnvMissing,
    ForbiddenEnvPresent,
    SpawnFailed,
    CommandSucceeded,
    CommandFailed,
    CommandTimedOut,
}

#[must_use]
pub fn run_staged_read_only_command(
    staging: &RuntimeEffectStagingPlan,
    command: &CommandContractDocument,
    context: &CommandExecutionContext<'_>,
) -> RuntimeCommandExecution {
    let command_contract = &command.command_contract;
    let mut reasons = Vec::new();
    let validation = validate_command(command);
    let validation_error_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .count();
    let validation_warning_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .count();

    if staging.status != RuntimeEffectStagingStatus::Staged {
        reasons.push(RuntimeCommandExecutionReason::StagingPlanNotStaged);
        return command_result(
            RuntimeCommandExecutionStatus::NotRun,
            command_contract,
            reasons,
            validation_error_count,
            validation_warning_count,
        );
    }

    if !staging_command_matches(staging, &command_contract.id) {
        reasons.push(RuntimeCommandExecutionReason::CommandNotStaged);
        return command_result(
            RuntimeCommandExecutionStatus::NotRun,
            command_contract,
            reasons,
            validation_error_count,
            validation_warning_count,
        );
    }

    if validation_error_count > 0 {
        reasons.push(RuntimeCommandExecutionReason::CommandValidationErrors);
    }
    if command_contract.side_effect_policy != CommandSideEffectPolicy::ReadOnly {
        reasons.push(RuntimeCommandExecutionReason::NonReadOnlyCommand);
    }
    if command_contract.network_policy != NetworkPolicy::Disabled {
        reasons.push(RuntimeCommandExecutionReason::NetworkNotDisabled);
    }
    if command_contract.safety.shell_string_allowed
        || command_contract.safety.writes_files
        || command_contract.safety.publishes
        || command_contract.safety.installs_packages
    {
        reasons.push(RuntimeCommandExecutionReason::UnsafeCommandSafetyFlags);
    }
    if shell_executor(command_contract.executor) {
        reasons.push(RuntimeCommandExecutionReason::ShellExecutorBlocked);
    }
    if !command_contract.platforms.contains(&current_platform()) {
        reasons.push(RuntimeCommandExecutionReason::UnsupportedPlatform);
    }
    if command_contract.timeout_ms == 0 {
        reasons.push(RuntimeCommandExecutionReason::TimeoutMustBePositive);
    }
    if missing_required_env(&command_contract.env_policy) {
        reasons.push(RuntimeCommandExecutionReason::RequiredEnvMissing);
    }
    if forbidden_env_present(&command_contract.env_policy) {
        reasons.push(RuntimeCommandExecutionReason::ForbiddenEnvPresent);
    }

    if !reasons.is_empty() {
        return command_result(
            RuntimeCommandExecutionStatus::Blocked,
            command_contract,
            reasons,
            validation_error_count,
            validation_warning_count,
        );
    }

    let started = Instant::now();
    let mut process = Command::new(executor_program(command_contract.executor));
    process
        .args(&command_contract.args)
        .current_dir(resolve_cwd(command_contract.cwd_policy, context))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_env_policy(&mut process, &command_contract.env_policy);

    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => {
            let mut result = command_result(
                RuntimeCommandExecutionStatus::Failed,
                command_contract,
                vec![RuntimeCommandExecutionReason::SpawnFailed],
                validation_error_count,
                validation_warning_count,
            );
            result.stderr = error.to_string();
            result.duration_ms = duration_millis(started.elapsed());
            return result;
        }
    };

    let output_limit =
        usize::try_from(command_contract.output_policy.max_bytes).unwrap_or(usize::MAX);
    let stdout_handle = child
        .stdout
        .take()
        .map(|stdout| spawn_limited_capture(stdout, output_limit));
    let stderr_handle = child
        .stderr
        .take()
        .map(|stderr| spawn_limited_capture(stderr, output_limit));
    let timeout = Duration::from_millis(command_contract.timeout_ms);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = join_capture(stdout_handle);
                let stderr = join_capture(stderr_handle);
                let reason = if status.success() {
                    RuntimeCommandExecutionReason::CommandSucceeded
                } else {
                    RuntimeCommandExecutionReason::CommandFailed
                };
                return RuntimeCommandExecution {
                    status: if status.success() {
                        RuntimeCommandExecutionStatus::Succeeded
                    } else {
                        RuntimeCommandExecutionStatus::Failed
                    },
                    command_id: command_contract.id.clone(),
                    executor: command_contract.executor,
                    exit_code: status.code(),
                    timed_out: false,
                    duration_ms: duration_millis(started.elapsed()),
                    stdout: stdout.text,
                    stderr: stderr.text,
                    stdout_truncated: stdout.truncated,
                    stderr_truncated: stderr.truncated,
                    reasons: vec![reason],
                    validation_error_count,
                    validation_warning_count,
                };
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let stdout = join_capture(stdout_handle);
                let stderr = join_capture(stderr_handle);
                return RuntimeCommandExecution {
                    status: RuntimeCommandExecutionStatus::TimedOut,
                    command_id: command_contract.id.clone(),
                    executor: command_contract.executor,
                    exit_code: None,
                    timed_out: true,
                    duration_ms: duration_millis(started.elapsed()),
                    stdout: stdout.text,
                    stderr: stderr.text,
                    stdout_truncated: stdout.truncated,
                    stderr_truncated: stderr.truncated,
                    reasons: vec![RuntimeCommandExecutionReason::CommandTimedOut],
                    validation_error_count,
                    validation_warning_count,
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let mut result = command_result(
                    RuntimeCommandExecutionStatus::Failed,
                    command_contract,
                    vec![RuntimeCommandExecutionReason::CommandFailed],
                    validation_error_count,
                    validation_warning_count,
                );
                result.stderr = error.to_string();
                result.duration_ms = duration_millis(started.elapsed());
                return result;
            }
        }
    }
}

pub fn command_execution_evidence_record(
    staging: &RuntimeEffectStagingPlan,
    execution: &RuntimeCommandExecution,
    recorded_at: impl Into<String>,
) -> RuntimeCommandEvidenceRecord {
    RuntimeCommandEvidenceRecord {
        schema_version: "0.1".to_string(),
        record_kind: RuntimeEvidenceKind::CommandExecution,
        recorded_at: recorded_at.into(),
        operation_id: staging.contract_id.clone(),
        command_id: execution.command_id.clone(),
        executor: execution.executor,
        status: execution.status,
        reasons: execution.reasons.clone(),
        exit_code: execution.exit_code,
        timed_out: execution.timed_out,
        duration_ms: execution.duration_ms,
        stdout: execution.stdout.clone(),
        stderr: execution.stderr.clone(),
        stdout_truncated: execution.stdout_truncated,
        stderr_truncated: execution.stderr_truncated,
        validation_error_count: execution.validation_error_count,
        validation_warning_count: execution.validation_warning_count,
    }
}

fn command_result(
    status: RuntimeCommandExecutionStatus,
    command: &forge_core_contracts::CommandContract,
    reasons: Vec<RuntimeCommandExecutionReason>,
    validation_error_count: usize,
    validation_warning_count: usize,
) -> RuntimeCommandExecution {
    RuntimeCommandExecution {
        status,
        command_id: command.id.clone(),
        executor: command.executor,
        exit_code: None,
        timed_out: false,
        duration_ms: 0,
        stdout: String::new(),
        stderr: String::new(),
        stdout_truncated: false,
        stderr_truncated: false,
        reasons,
        validation_error_count,
        validation_warning_count,
    }
}

fn staging_command_matches(staging: &RuntimeEffectStagingPlan, command_id: &StableId) -> bool {
    staging
        .command_refs
        .iter()
        .any(|command_ref| &command_ref.id == command_id)
}

fn shell_executor(executor: CommandExecutor) -> bool {
    matches!(executor, CommandExecutor::Powershell | CommandExecutor::Sh)
}

fn executor_program(executor: CommandExecutor) -> &'static str {
    match executor {
        CommandExecutor::Cargo => "cargo",
        CommandExecutor::Node => "node",
        CommandExecutor::Bun => "bun",
        CommandExecutor::Powershell => "powershell",
        CommandExecutor::Sh => "sh",
        CommandExecutor::Git => "git",
    }
}

fn current_platform() -> Platform {
    if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::Macos
    } else {
        Platform::Linux
    }
}

fn resolve_cwd<'a>(policy: CwdPolicy, context: &'a CommandExecutionContext<'_>) -> &'a Path {
    match policy {
        CwdPolicy::ProjectRoot => context.project_root,
        CwdPolicy::RepoRoot => context.repo_root,
        CwdPolicy::PackageRoot => context.package_root,
    }
}

fn apply_env_policy(process: &mut Command, policy: &EnvPolicy) {
    match policy.inherit {
        EnvInheritPolicy::None => {
            process.env_clear();
        }
        EnvInheritPolicy::Minimal => {
            process.env_clear();
            for key in minimal_env_allowlist() {
                if let Some(value) = env::var_os(key) {
                    process.env(key, value);
                }
            }
        }
        EnvInheritPolicy::Project => {}
    }
}

fn minimal_env_allowlist() -> &'static [&'static str] {
    &[
        "PATH",
        "Path",
        "PATHEXT",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "HOME",
        "USERPROFILE",
    ]
}

fn missing_required_env(policy: &EnvPolicy) -> bool {
    policy.required.iter().any(|key| !env_key_exists(key))
}

fn forbidden_env_present(policy: &EnvPolicy) -> bool {
    policy.forbidden.iter().any(|key| env_key_exists(key))
}

fn env_key_exists(expected: &str) -> bool {
    env::vars_os().any(|(key, _)| {
        let actual = key.to_string_lossy();
        if cfg!(windows) {
            actual.eq_ignore_ascii_case(expected)
        } else {
            actual == expected
        }
    })
}

#[derive(Debug)]
struct CapturedOutput {
    text: String,
    truncated: bool,
}

fn spawn_limited_capture<R>(mut reader: R, max_bytes: usize) -> thread::JoinHandle<CapturedOutput>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut captured = Vec::new();
        let mut truncated = false;
        let mut buffer = [0_u8; 8192];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(bytes_read) => {
                    if captured.len() < max_bytes {
                        let remaining = max_bytes - captured.len();
                        let keep = remaining.min(bytes_read);
                        captured.extend_from_slice(&buffer[..keep]);
                        if keep < bytes_read {
                            truncated = true;
                        }
                    } else if bytes_read > 0 {
                        truncated = true;
                    }
                }
            }
        }

        CapturedOutput {
            text: String::from_utf8_lossy(&captured).to_string(),
            truncated,
        }
    })
}

fn join_capture(handle: Option<thread::JoinHandle<CapturedOutput>>) -> CapturedOutput {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_else(|| CapturedOutput {
            text: String::new(),
            truncated: false,
        })
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
