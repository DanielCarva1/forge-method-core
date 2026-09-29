//! Bounded read-only CLI queries. Forge remains the project/state owner.
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tauri_plugin_dialog::DialogExt;
use tauri::Manager;
use tokio::io::AsyncReadExt;

#[derive(Deserialize)]
struct Envelope<T> {
    command: String,
    ok: bool,
    data: Option<T>,
}
#[derive(Deserialize)]
struct FailureEnvelope {
    command: String,
    ok: bool,
    exit_reason: Option<String>,
    error: Option<FailureDetail>,
}
#[derive(Deserialize)]
struct FailureDetail {
    code: String,
    message: String,
}
enum QueryError {
    RetryableReadConflict,
    Other(&'static str),
}

#[derive(Clone, Copy)]
pub(crate) struct QueryTiming {
    total: Duration,
    attempt: Duration,
}

impl QueryTiming {
    const STANDARD: Self = Self {
        total: Duration::from_secs(20),
        attempt: Duration::from_secs(15),
    };
    // A first `start` may create a sidecar before workflow initialization.
    // Keep read-only project queries fast, but do not kill this explicit setup
    // after it has already begun writing the new project's authority.
    const INITIALIZE: Self = Self {
        total: Duration::from_secs(90),
        attempt: Duration::from_secs(85),
    };
    pub(crate) const RECORD: Self = Self {
        total: Duration::from_secs(90),
        attempt: Duration::from_secs(85),
    };
}

fn retryable_read_conflict(bytes: &[u8], expected: &str) -> bool {
    let Ok(response) = serde_json::from_slice::<FailureEnvelope>(bytes) else {
        return false;
    };
    response.command == expected
        && !response.ok
        && response.error.as_ref().is_some_and(|error| {
            response.exit_reason.as_deref() == Some(error.code.as_str())
                && matches!(error.code.as_str(), "conflict" | "rejected_by_gate")
                && (error.message.contains("this process already holds quiescence for ")
                    || error.message == "replacement continuity is unavailable: isolation or promotion state changed during read-only replacement inspection")
        })
}
fn missing_project_link(bytes: &[u8], expected: &str) -> bool {
    let Ok(response) = serde_json::from_slice::<FailureEnvelope>(bytes) else {
        return false;
    };
    response.command == expected
        && expected == "project.resolve"
        && !response.ok
        && response.exit_reason.as_deref() == Some("env_config")
        && response.error.as_ref().is_some_and(|error| {
            error.code == "env_config"
                && error.message.starts_with("missing Forge Project Link at '")
        })
}
#[derive(Deserialize)]
struct ResolvedProject {
    project_id: String,
    project_root: String,
    state_exists: bool,
}
#[derive(Deserialize)]
struct StartedProject {
    project: Option<ResolvedProject>,
    state_loss: Option<serde_json::Value>,
    next_step: Option<StartedNextStep>,
}
#[derive(Deserialize)]
struct StartedNextStep {
    argv: Vec<String>,
}
#[derive(Deserialize)]
struct InitializedWorkflow {
    project_id: String,
}

fn workflow_init_requested(next_step: &StartedNextStep, root: &Path) -> Result<bool, &'static str> {
    let argv = &next_step.argv;
    if !argv
        .get(0..3)
        .is_some_and(|prefix| prefix == ["forge-core", "workflow", "init"])
    {
        return Ok(false);
    }
    let target = match argv.as_slice() {
        [_, _, _, flag, target] if flag == "--root" => Path::new(target)
            .canonicalize()
            .map_err(|_| "O Forge indicou uma pasta de inicialização inválida.")?,
        _ => return Err("O Forge indicou uma inicialização incompatível."),
    };
    if target != root {
        return Err("O Forge indicou outra pasta para inicializar; nada foi alterado.");
    }
    Ok(true)
}
#[derive(Serialize)]
pub struct ProjectSummary {
    pub project_id: String,
    pub project_root: String,
}

/// Only returns the chosen path. Project validity remains owned by inspect_project.
#[tauri::command]
pub async fn choose_project_folder(
    window: tauri::WebviewWindow,
) -> Result<Option<String>, &'static str> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_title("Escolha a pasta do projeto")
        .set_parent(&window)
        .pick_folder(move |selection| {
            let _ = sender.send(selection);
        });
    let selection = receiver
        .await
        .map_err(|_| "Não foi possível abrir a seleção de pastas.")?;
    selection
        .map(|path| {
            path.into_path()
                .map_err(|_| "A pasta escolhida não tem um caminho local válido.")
                .and_then(|path| {
                    path.into_os_string()
                        .into_string()
                        .map_err(|_| "A pasta escolhida não tem um caminho de texto válido.")
                })
        })
        .transpose()
}

fn suggested_project_name(idea: &str) -> String {
    let first = idea.split(['.', '!', '?', '\n']).next().unwrap_or("").trim();
    let first = first.strip_prefix("Quero ").unwrap_or(first);
    let clean = first
        .chars()
        .take(64)
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { ' ' })
        .collect::<String>();
    let words = clean.split_whitespace().take(6).collect::<Vec<_>>().join(" ");
    if words.is_empty() { "Meu projeto".to_string() } else { words }
}

/// The default is one click: create a new, unique folder in Documents/Projetos
/// Forge. The chosen folder remains visible after start; custom paths use the
/// separate native picker and never become an invisible fallback.
#[tauri::command]
pub async fn create_default_project(
    window: tauri::WebviewWindow,
    idea: String,
) -> Result<String, &'static str> {
    if idea.len() > 16_384 {
        return Err("A ideia está longa demais para iniciar o projeto.");
    }
    #[cfg(debug_assertions)]
    let test_base = std::env::var_os("FORGE_DESKTOP_PROJECTS_DIR").map(PathBuf::from);
    #[cfg(not(debug_assertions))]
    let test_base: Option<PathBuf> = None;
    let base = match test_base {
        Some(path) => path,
        None => window
            .app_handle()
            .path()
            .document_dir()
            .map_err(|_| "Não foi possível localizar a pasta Documentos deste usuário.")?
            .join("Projetos Forge"),
    };
    std::fs::create_dir_all(&base)
        .map_err(|_| "Não foi possível preparar a pasta Projetos Forge em Documentos.")?;
    let visible_base = base.clone();
    let base = base
        .canonicalize()
        .map_err(|_| "Não foi possível conferir a pasta de projetos.")?;
    let name = suggested_project_name(&idea);
    for index in 1..=100 {
        let suffix = if index == 1 { String::new() } else { format!(" ({index})") };
        let child = base.join(format!("{name}{suffix}"));
        match std::fs::create_dir(&child) {
            Ok(()) => {
                let visible_child = visible_base.join(format!("{name}{suffix}"));
                if visible_child.canonicalize().ok().as_ref() != Some(&child) {
                    return Err("A pasta criada não corresponde ao local de projetos.");
                }
                return visible_child.into_os_string().into_string()
                    .map_err(|_| "A pasta criada não tem um caminho de texto válido.");
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("Não foi possível criar o novo projeto em Documentos."),
        }
    }
    Err("Há muitas pastas com esse nome. Escolha outro local para o projeto.")
}

pub(crate) fn installed_runtime() -> Result<PathBuf, &'static str> {
    let path = std::env::var_os("FORGE_CORE_EXE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().and_then(bundled_runtime))
        })
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(|base| PathBuf::from(base).join("Programs/forge-core/bin/forge-core.exe"))
        })
        .ok_or("O Forge não foi encontrado nesta máquina.")?;
    if !path.is_absolute() || !path.is_file() {
        return Err("O Forge não foi encontrado nesta máquina.");
    }
    Ok(path)
}

fn bundled_runtime(executable_dir: &Path) -> Option<PathBuf> {
    let path = executable_dir.join("forge-core/forge-core.exe");
    path.is_file().then_some(path)
}

pub async fn query<T: DeserializeOwned>(
    root: &Path,
    args: &[&str],
    expected: &str,
) -> Result<T, &'static str> {
    query_with_limit(root, args, expected, 65_536).await
}

/// The full read-only workflow report may be larger than the compact resume.
/// Callers must project it before returning data to the WebView.
pub async fn query_with_limit<T: DeserializeOwned>(
    root: &Path,
    args: &[&str],
    expected: &str,
    max_bytes: usize,
) -> Result<T, &'static str> {
    query_with_timing(root, args, expected, max_bytes, QueryTiming::STANDARD).await
}

/// Only record reads use the longer budget; project identity and start stay fast-failing.
pub(crate) async fn query_with_timing<T: DeserializeOwned>(
    root: &Path,
    args: &[&str],
    expected: &str,
    max_bytes: usize,
    timing: QueryTiming,
) -> Result<T, &'static str> {
    tokio::time::timeout(timing.total, async {
        for attempt in 0..4 {
            match query_once(root, args, expected, max_bytes, timing.attempt).await {
                Ok(value) => return Ok(value),
                Err(QueryError::RetryableReadConflict) if attempt < 3 => {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                Err(QueryError::RetryableReadConflict) => {
                    return Err("O Forge está ocupado. Tente consultar o registro novamente.");
                }
                Err(QueryError::Other(message)) => return Err(message),
            }
        }
        unreachable!("bounded query attempts return above")
    })
    .await
    .map_err(|_| "O Forge demorou para responder. Você pode tentar novamente.")?
}

async fn query_once<T: DeserializeOwned>(
    root: &Path,
    args: &[&str],
    expected: &str,
    max_bytes: usize,
    timeout: Duration,
) -> Result<T, QueryError> {
    let runtime = installed_runtime().map_err(QueryError::Other)?;
    let mut command = tokio::process::Command::new(&runtime);
    command
        .args(args)
        .arg("--root")
        .arg(root)
        .arg("--json")
        .current_dir(
            runtime
                .parent()
                .ok_or(QueryError::Other("Instalação do Forge inválida."))?,
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command
        .spawn()
        .map_err(|_| QueryError::Other("Não foi possível iniciar a consulta ao Forge."))?;
    let stdout = child
        .stdout
        .take()
        .ok_or(QueryError::Other("Não foi possível ler a consulta."))?;
    let result = tokio::time::timeout(timeout, async {
        let mut bytes = Vec::new();
        stdout.take(max_bytes as u64 + 1).read_to_end(&mut bytes).await.map_err(|_| QueryError::Other("Não foi possível ler a resposta do Forge."))?;
        if bytes.len() > max_bytes { return Err(QueryError::Other("A resposta do Forge excedeu o tamanho esperado.")); }
        if !child.wait().await.map_err(|_| QueryError::Other("A consulta ao Forge falhou."))?.success() {
            if retryable_read_conflict(&bytes, expected) {
                return Err(QueryError::RetryableReadConflict);
            }
            if missing_project_link(&bytes, expected) {
                return Err(QueryError::Other("Esta pasta ainda não usa o Forge. Escolha-a em Minha conversa e clique em “Continuar nesta pasta”."));
            }
            return Err(QueryError::Other("Não foi possível consultar o projeto. Confira a pasta e o vínculo com o Forge; nada foi alterado."));
        }
        let envelope: Envelope<T> = serde_json::from_slice(&bytes).map_err(|_| QueryError::Other("O Forge retornou uma resposta incompatível."))?;
        if !envelope.ok || envelope.command != expected { return Err(QueryError::Other("O Forge não confirmou essa consulta.")); }
        envelope.data.ok_or(QueryError::Other("O Forge não informou os dados da consulta."))
    }).await;
    if child.id().is_some() {
        let _ = child.kill().await;
    }
    result.map_err(|_| {
        QueryError::Other("O Forge demorou para responder. Você pode tentar novamente.")
    })?
}

#[tauri::command]
pub async fn inspect_project(project_root: String) -> Result<ProjectSummary, &'static str> {
    let requested = Path::new(&project_root);
    if !requested.is_absolute() || !requested.is_dir() {
        return Err("Informe o caminho completo de uma pasta que existe.");
    }
    let root = requested
        .canonicalize()
        .map_err(|_| "Não foi possível acessar essa pasta.")?;
    let data: ResolvedProject = query(&root, &["project", "resolve"], "project.resolve").await?;
    if !data.state_exists {
        return Err("O estado deste projeto não está disponível. Nada foi recriado ou alterado.");
    }
    let resolved = Path::new(&data.project_root)
        .canonicalize()
        .map_err(|_| "Não foi possível conferir a pasta retornada pelo Forge.")?;
    if resolved != root || data.project_id.trim().is_empty() {
        return Err("A resposta do Forge não corresponde à pasta informada.");
    }
    Ok(ProjectSummary {
        project_id: data.project_id,
        project_root: data.project_root,
    })
}

/// Explicit user action: the core owns fresh-project initialization and refuses
/// to recreate missing linked authority. Folder selection and inspection stay read-only.
#[tauri::command]
pub async fn start_project(project_root: String) -> Result<ProjectSummary, &'static str> {
    let requested = Path::new(&project_root);
    if !requested.is_absolute() || !requested.is_dir() {
        return Err("Informe o caminho completo de uma pasta que existe.");
    }
    let root = requested
        .canonicalize()
        .map_err(|_| "Não foi possível acessar essa pasta.")?;
    let started: StartedProject = query_with_timing(
        &root,
        &["start"],
        "start",
        65_536,
        QueryTiming::INITIALIZE,
    )
        .await
        .map_err(|_| "Não foi possível iniciar o projeto. Confira a pasta e o estado do Forge antes de tentar novamente.")?;
    let Some(project) = started.project else {
        return Err(
            "O Forge não confirmou o projeto iniciado. Confira a pasta antes de tentar novamente.",
        );
    };
    if started.state_loss.is_some() || !project.state_exists || project.project_id.trim().is_empty()
    {
        return Err(
            "O Forge não confirmou um projeto pronto. Confira o estado antes de tentar novamente.",
        );
    }
    let resolved = Path::new(&project.project_root)
        .canonicalize()
        .map_err(|_| "Não foi possível conferir a pasta retornada pelo Forge.")?;
    if resolved != root {
        return Err("A resposta do Forge não corresponde à pasta informada.");
    }
    if let Some(next_step) = started.next_step {
        if workflow_init_requested(&next_step, &root)? {
            let initialized: InitializedWorkflow =
                query_with_timing(
                    &root,
                    &["workflow", "init"],
                    "workflow.init",
                    65_536,
                    QueryTiming::INITIALIZE,
                )
                    .await
                    .map_err(|_| "O projeto foi vinculado, mas o registro não pôde ser preparado. Tente continuar nesta pasta novamente.")?;
            if initialized.project_id != project.project_id {
                return Err("O Forge inicializou um registro que não corresponde ao projeto.");
            }
        }
    }
    inspect_project(project.project_root).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_project_name_comes_from_idea_without_a_path() {
        assert_eq!(suggested_project_name("Quero criar algo artístico. Me ajude."), "criar algo artístico");
        assert_eq!(suggested_project_name(""), "Meu projeto");
        assert_eq!(suggested_project_name("../fora"), "Meu projeto");
    }

    #[test]
    fn initialization_waits_longer_than_read_only_project_queries() {
        assert!(QueryTiming::INITIALIZE.attempt > QueryTiming::STANDARD.attempt);
        assert!(QueryTiming::INITIALIZE.total > QueryTiming::STANDARD.total);
        assert!(QueryTiming::INITIALIZE.total >= QueryTiming::INITIALIZE.attempt);
    }

    #[test]
    fn bundled_core_requires_an_existing_file_beside_the_app() {
        let base =
            std::env::temp_dir().join(format!("forge-core-resource-{}", uuid::Uuid::new_v4()));
        let core = base.join("forge-core/forge-core.exe");
        assert_eq!(bundled_runtime(&base), None);
        std::fs::create_dir_all(core.parent().unwrap()).unwrap();
        std::fs::write(&core, b"fixture").unwrap();
        assert_eq!(bundled_runtime(&base), Some(core));
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn retries_only_matching_transient_read_conflicts() {
        let conflict = br#"{"command":"workflow.resume","ok":false,"exit_reason":"conflict","error":{"code":"conflict","message":"governance ledger failed: workflow-governance lock failed: producer boundary: this process already holds quiescence for project"}}"#;
        assert!(retryable_read_conflict(conflict, "workflow.resume"));
        assert!(!retryable_read_conflict(conflict, "project.resolve"));
        let claim_wal_conflict = br#"{"command":"workflow.resume","ok":false,"exit_reason":"rejected_by_gate","error":{"code":"rejected_by_gate","message":"claim WAL projection failed: recover claim WAL failed: lock WAL failed: this process already holds quiescence for project"}}"#;
        assert!(retryable_read_conflict(
            claim_wal_conflict,
            "workflow.resume"
        ));
        let snapshot_drift = br#"{"command":"workflow.resume","ok":false,"exit_reason":"rejected_by_gate","error":{"code":"rejected_by_gate","message":"replacement continuity is unavailable: isolation or promotion state changed during read-only replacement inspection"}}"#;
        assert!(retryable_read_conflict(snapshot_drift, "workflow.resume"));
        let other_conflict = br#"{"command":"workflow.resume","ok":false,"exit_reason":"conflict","error":{"code":"conflict","message":"durable workflow state changed"}}"#;
        assert!(!retryable_read_conflict(other_conflict, "workflow.resume"));
        assert!(!retryable_read_conflict(b"not-json", "workflow.resume"));
    }

    #[test]
    fn identifies_only_missing_project_link_from_project_resolve() {
        let missing = br#"{"command":"project.resolve","ok":false,"exit_reason":"env_config","error":{"code":"env_config","message":"missing Forge Project Link at 'C:\\new\\.forge-method.yaml'; consumer projects must point at a Forge Runtime Sidecar"}}"#;
        assert!(missing_project_link(missing, "project.resolve"));
        assert!(!missing_project_link(missing, "start"));
        let damaged = br#"{"command":"project.resolve","ok":false,"exit_reason":"env_config","error":{"code":"env_config","message":"linked sidecar is missing"}}"#;
        assert!(!missing_project_link(damaged, "project.resolve"));
    }

    #[test]
    fn initializes_only_the_exact_project_selected_by_start() {
        let root = std::env::current_dir().unwrap().canonicalize().unwrap();
        let target = root.to_string_lossy().to_string();
        let init = StartedNextStep {
            argv: vec![
                "forge-core".into(),
                "workflow".into(),
                "init".into(),
                "--root".into(),
                target,
            ],
        };
        assert!(workflow_init_requested(&init, &root).unwrap());
        let resume = StartedNextStep {
            argv: vec![
                "forge-core".into(),
                "workflow".into(),
                "resume".into(),
                "--root".into(),
                root.to_string_lossy().to_string(),
            ],
        };
        assert!(!workflow_init_requested(&resume, &root).unwrap());
        let malformed = StartedNextStep {
            argv: vec![
                "forge-core".into(),
                "workflow".into(),
                "init".into(),
                "--bad".into(),
                root.to_string_lossy().to_string(),
            ],
        };
        assert!(workflow_init_requested(&malformed, &root).is_err());
        let other = StartedNextStep {
            argv: vec![
                "forge-core".into(),
                "workflow".into(),
                "init".into(),
                "--root".into(),
                root.parent().unwrap().to_string_lossy().to_string(),
            ],
        };
        assert!(workflow_init_requested(&other, &root).is_err());
    }
}
