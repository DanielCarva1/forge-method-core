//! Operational adapter for `forge-core project reinitialize`.
//!
//! Planning captures exact predecessor and diagnosis bytes into a sealed,
//! create-only plan. Applying revalidates those bytes and delegates the only
//! Project Link mutation to the retained Store CAS.

use crate::cli_error::ExitError;
use crate::cli_util::emit_envelope;
use crate::io_util::{read_regular_file_no_follow_bounded, RetainedDirectoryIdentity};
use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_store::project_reinitialize::{
    apply_retained_reinitialize, capture_reinitialize_plan_request, decode_reinitialize_plan,
    encode_reinitialize_plan, mint_reinitialize_operation_id, plan, ReinitializePlan,
    ReinitializeReceipt,
};
use serde::Serialize;
use std::path::{Component, Path, PathBuf};

const COMMAND_PLAN: &str = "project.reinitialize.plan";
const COMMAND_APPLY: &str = "project.reinitialize.apply";
const MAX_PLAN_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReinitializeArgs {
    pub root: PathBuf,
    pub destination: PathBuf,
    pub abandoned_authority_id: String,
    pub new_project_id: String,
    pub new_authority_id: String,
    pub state_loss_diagnosis: PathBuf,
    pub plan_file: PathBuf,
    pub plan_digest: Option<String>,
    pub confirmation: Option<String>,
    pub json: bool,
}

#[derive(Debug, Serialize)]
pub struct ProjectReinitializePlanPayload {
    pub planned: bool,
    pub operation_id: String,
    pub root: PathBuf,
    pub destination: PathBuf,
    pub abandoned_authority_id: String,
    pub new_project_id: String,
    pub new_authority_id: String,
    pub state_loss_diagnosis: PathBuf,
    pub plan_file: PathBuf,
    pub plan_digest: String,
    pub confirmation_token: String,
    pub selected_host: Option<String>,
}

#[derive(Debug)]
struct CommandFailure {
    reason: ExitReason,
    message: String,
}

impl CommandFailure {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            reason: ExitReason::InvalidDecisionShape,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            reason: ExitReason::Conflict,
            message: message.into(),
        }
    }

    fn environment(message: impl Into<String>) -> Self {
        Self {
            reason: ExitReason::EnvConfig,
            message: message.into(),
        }
    }
}

/// Parse and dispatch the public reinitialize command family.
///
/// `plan` is read-only with respect to project authority and writes one
/// create-only operator plan file. `apply` requires the exact plan digest and
/// plan-derived confirmation token before entering the retained Store protocol.
///
/// # Errors
///
/// Returns a usage error for malformed or incomplete arguments, or a typed
/// envelope exit error when planning or applying fails.
pub fn run_project_reinitialize_command(args: &[String]) -> Result<(), ExitError> {
    let Some(subcommand) = args.get(1).map(String::as_str) else {
        return Err(ExitError::usage(project_reinitialize_usage()));
    };
    if matches!(subcommand, "--help" | "-h" | "help") {
        println!("{}", project_reinitialize_usage());
        return Ok(());
    }
    let parsed = parse_args(args)?;
    match subcommand {
        "plan" => run_plan(parsed),
        "apply" => run_apply(parsed),
        _ => Err(ExitError::usage(project_reinitialize_usage())),
    }
}

fn run_plan(parsed: ProjectReinitializeArgs) -> Result<(), ExitError> {
    let envelope = match execute_plan(&parsed) {
        Ok(payload) => CliEnvelope::ok(COMMAND_PLAN, payload),
        Err(failure) => CliEnvelope::<ProjectReinitializePlanPayload>::err(
            COMMAND_PLAN,
            failure.reason,
            failure.message,
        ),
    };
    emit_envelope(envelope, parsed.json)
}

fn execute_plan(
    parsed: &ProjectReinitializeArgs,
) -> Result<ProjectReinitializePlanPayload, CommandFailure> {
    validate_normalized_absolute_path(&parsed.plan_file, "plan file")?;
    let operation_id = mint_reinitialize_operation_id().map_err(|error| {
        CommandFailure::environment(format!("cannot mint reinitialize operation id: {error}"))
    })?;
    let request = capture_reinitialize_plan_request(
        operation_id,
        &parsed.root,
        &parsed.destination,
        &parsed.state_loss_diagnosis,
        parsed.abandoned_authority_id.clone(),
        parsed.new_project_id.clone(),
        parsed.new_authority_id.clone(),
    )
    .map_err(|error| {
        CommandFailure::invalid(format!("cannot capture exact plan inputs: {error}"))
    })?;
    let sealed = plan(request).map_err(|error| {
        CommandFailure::invalid(format!("cannot seal reinitialize plan: {error}"))
    })?;
    let bytes = encode_reinitialize_plan(&sealed).map_err(|error| {
        CommandFailure::environment(format!("cannot encode reinitialize plan: {error}"))
    })?;
    write_create_only_plan(&parsed.plan_file, &bytes)?;

    Ok(ProjectReinitializePlanPayload {
        planned: true,
        operation_id: sealed.operation_id,
        root: PathBuf::from(sealed.project_root),
        destination: PathBuf::from(sealed.destination),
        abandoned_authority_id: sealed.predecessor_identity,
        new_project_id: sealed.successor_project_id,
        new_authority_id: sealed.successor_identity,
        state_loss_diagnosis: PathBuf::from(sealed.diagnosis.diagnosis_path),
        plan_file: parsed.plan_file.clone(),
        plan_digest: sealed.plan_digest,
        confirmation_token: sealed.confirmation_token,
        selected_host: sealed.selected_host,
    })
}

fn run_apply(parsed: ProjectReinitializeArgs) -> Result<(), ExitError> {
    let envelope = match execute_apply(&parsed) {
        Ok(receipt) => CliEnvelope::ok(COMMAND_APPLY, receipt),
        Err(failure) => {
            CliEnvelope::<ReinitializeReceipt>::err(COMMAND_APPLY, failure.reason, failure.message)
        }
    };
    emit_envelope(envelope, parsed.json)
}

fn execute_apply(parsed: &ProjectReinitializeArgs) -> Result<ReinitializeReceipt, CommandFailure> {
    validate_normalized_absolute_path(&parsed.plan_file, "plan file")?;
    let bytes = read_regular_file_no_follow_bounded(&parsed.plan_file, MAX_PLAN_BYTES).map_err(
        |error| {
            CommandFailure::environment(format!(
                "cannot read sealed reinitialize plan '{}': {error}",
                parsed.plan_file.display()
            ))
        },
    )?;
    let sealed = decode_reinitialize_plan(&bytes)
        .map_err(|error| CommandFailure::invalid(format!("invalid sealed plan: {error}")))?;
    validate_apply_arguments(parsed, &sealed)?;
    let confirmation = parsed
        .confirmation
        .as_deref()
        .ok_or_else(|| CommandFailure::invalid("missing plan-derived confirmation token"))?;
    apply_retained_reinitialize(&sealed, confirmation).map_err(|error| {
        CommandFailure::conflict(format!("retained reinitialize apply failed: {error}"))
    })
}

fn validate_apply_arguments(
    parsed: &ProjectReinitializeArgs,
    sealed: &ReinitializePlan,
) -> Result<(), CommandFailure> {
    let supplied_digest = parsed
        .plan_digest
        .as_deref()
        .ok_or_else(|| CommandFailure::invalid("missing sealed plan digest"))?;
    if supplied_digest != sealed.plan_digest {
        return Err(CommandFailure::conflict(
            "supplied plan digest differs from the sealed plan",
        ));
    }
    if parsed.root != Path::new(&sealed.project_root)
        || parsed.destination != Path::new(&sealed.destination)
        || parsed.abandoned_authority_id != sealed.predecessor_identity
        || parsed.new_project_id != sealed.successor_project_id
        || parsed.new_authority_id != sealed.successor_identity
        || parsed.state_loss_diagnosis != Path::new(&sealed.diagnosis.diagnosis_path)
    {
        return Err(CommandFailure::conflict(
            "apply arguments differ from the sealed reinitialize plan",
        ));
    }
    if parsed.confirmation.as_deref() != Some(sealed.confirmation_token.as_str()) {
        return Err(CommandFailure::conflict(
            "confirmation does not exactly match the sealed plan",
        ));
    }
    Ok(())
}

fn write_create_only_plan(path: &Path, bytes: &[u8]) -> Result<(), CommandFailure> {
    let parent = path
        .parent()
        .ok_or_else(|| CommandFailure::invalid("plan file has no parent"))?;
    let leaf = path
        .file_name()
        .ok_or_else(|| CommandFailure::invalid("plan file has no leaf"))?;
    let retained_parent = RetainedDirectoryIdentity::capture(parent).map_err(|error| {
        CommandFailure::environment(format!(
            "cannot retain plan-file parent '{}': {error}",
            parent.display()
        ))
    })?;
    retained_parent
        .write_new_direct_file_synced(Path::new(leaf), bytes)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                CommandFailure::conflict(format!(
                    "plan file '{}' already exists; immutable plans are create-only",
                    path.display()
                ))
            } else {
                CommandFailure::environment(format!(
                    "cannot publish immutable plan '{}': {error}",
                    path.display()
                ))
            }
        })
}

fn validate_normalized_absolute_path(path: &Path, label: &str) -> Result<(), CommandFailure> {
    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(CommandFailure::invalid(format!(
            "{label} must be a normalized absolute path"
        )));
    }
    Ok(())
}

fn parse_args(args: &[String]) -> Result<ProjectReinitializeArgs, ExitError> {
    let Some(subcommand) = args.get(1).map(String::as_str) else {
        return Err(ExitError::usage(project_reinitialize_usage()));
    };
    if !matches!(subcommand, "plan" | "apply") {
        return Err(ExitError::usage(project_reinitialize_usage()));
    }

    let mut root = None;
    let mut destination = None;
    let mut abandoned_authority_id = None;
    let mut new_project_id = None;
    let mut new_authority_id = None;
    let mut state_loss_diagnosis = None;
    let mut plan_file = None;
    let mut plan_digest = None;
    let mut confirmation = None;
    let mut json = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
                continue;
            }
            "--no-json" => {
                json = false;
                index += 1;
                continue;
            }
            _ => {}
        }
        let flag = args[index].as_str();
        index += 1;
        let Some(value) = args.get(index) else {
            return Err(ExitError::usage(project_reinitialize_usage()));
        };
        let duplicate = match flag {
            "--root" => root.replace(PathBuf::from(value)).is_some(),
            "--destination" => destination.replace(PathBuf::from(value)).is_some(),
            "--abandoned-authority-id" => abandoned_authority_id.replace(value.clone()).is_some(),
            "--new-project-id" => new_project_id.replace(value.clone()).is_some(),
            "--new-authority-id" => new_authority_id.replace(value.clone()).is_some(),
            "--state-loss-diagnosis" => {
                state_loss_diagnosis.replace(PathBuf::from(value)).is_some()
            }
            "--plan-file" => plan_file.replace(PathBuf::from(value)).is_some(),
            "--plan-digest" => plan_digest.replace(value.clone()).is_some(),
            "--confirm" => confirmation.replace(value.clone()).is_some(),
            _ => return Err(ExitError::usage(project_reinitialize_usage())),
        };
        if duplicate {
            return Err(ExitError::usage(project_reinitialize_usage()));
        }
        index += 1;
    }

    let parsed = ProjectReinitializeArgs {
        root: root.ok_or_else(|| ExitError::usage(project_reinitialize_usage()))?,
        destination: destination.ok_or_else(|| ExitError::usage(project_reinitialize_usage()))?,
        abandoned_authority_id: required_identity(abandoned_authority_id)?,
        new_project_id: required_identity(new_project_id)?,
        new_authority_id: required_identity(new_authority_id)?,
        state_loss_diagnosis: state_loss_diagnosis
            .ok_or_else(|| ExitError::usage(project_reinitialize_usage()))?,
        plan_file: plan_file.ok_or_else(|| ExitError::usage(project_reinitialize_usage()))?,
        plan_digest,
        confirmation,
        json,
    };
    match subcommand {
        "plan" if parsed.plan_digest.is_some() || parsed.confirmation.is_some() => {
            Err(ExitError::usage(project_reinitialize_usage()))
        }
        "apply"
            if !parsed.plan_digest.as_deref().is_some_and(is_sha256_digest)
                || !parsed.confirmation.as_deref().is_some_and(is_sha256_digest) =>
        {
            Err(ExitError::usage(project_reinitialize_usage()))
        }
        _ => Ok(parsed),
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn required_identity(value: Option<String>) -> Result<String, ExitError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ExitError::usage(project_reinitialize_usage()))
}

fn project_reinitialize_usage() -> &'static str {
    "forge-core project reinitialize plan --root <absolute-path> --destination <absolute-path> --abandoned-authority-id <id> --new-project-id <id> --new-authority-id <id> --state-loss-diagnosis <absolute-file> --plan-file <absolute-file> [--json]\nforge-core project reinitialize apply --root <absolute-path> --destination <absolute-path> --abandoned-authority-id <id> --new-project-id <id> --new-authority-id <id> --state-loss-diagnosis <absolute-file> --plan-file <absolute-file> --plan-digest sha256:<digest> --confirm sha256:<plan-derived-token> [--json]"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_args() -> Vec<String> {
        [
            "forge-core",
            "plan",
            "--root",
            "/project",
            "--destination",
            "/new-sidecar",
            "--abandoned-authority-id",
            "lost-authority",
            "--new-project-id",
            "new-project",
            "--new-authority-id",
            "new-authority",
            "--state-loss-diagnosis",
            "/diagnosis.json",
            "--plan-file",
            "/plan.json",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    #[test]
    fn plan_requires_every_explicit_input() {
        let mut args = plan_args();
        args.drain(2..4);
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn abandoned_authority_identity_is_required_for_plan_and_apply() {
        let mut plan = plan_args();
        let abandoned = plan
            .iter()
            .position(|arg| arg == "--abandoned-authority-id")
            .expect("abandoned authority flag");
        plan.drain(abandoned..=abandoned + 1);
        assert!(parse_args(&plan).is_err());

        plan[1] = "apply".to_owned();
        plan.extend([
            "--plan-digest".to_owned(),
            format!("sha256:{}", "a".repeat(64)),
            "--confirm".to_owned(),
            format!("sha256:{}", "b".repeat(64)),
        ]);
        assert!(parse_args(&plan).is_err());
    }

    #[test]
    fn apply_requires_plan_derived_digest_shapes() {
        let mut args = plan_args();
        args[1] = "apply".to_owned();
        args.extend([
            "--plan-digest".to_owned(),
            format!("sha256:{}", "a".repeat(64)),
            "--confirm".to_owned(),
            format!("sha256:{}", "b".repeat(64)),
        ]);
        assert!(parse_args(&args).is_ok());
        let digest = args
            .iter()
            .position(|arg| arg == "--plan-digest")
            .expect("plan digest flag");
        args[digest + 1] = "a".repeat(64);
        assert!(parse_args(&args).is_err());
        args[digest + 1] = format!("sha256:{}", "a".repeat(64));
        *args.last_mut().expect("confirmation value") = "yes".to_owned();
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parser_rejects_host_selection_and_duplicate_inputs() {
        let mut args = plan_args();
        args.extend(["--host".to_owned(), "remote".to_owned()]);
        assert!(parse_args(&args).is_err());
        let mut duplicate = plan_args();
        duplicate.extend(["--root".to_owned(), "/other".to_owned()]);
        assert!(parse_args(&duplicate).is_err());
    }
}
