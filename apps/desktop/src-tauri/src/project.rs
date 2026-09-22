//! Bounded read-only CLI queries. Forge remains the project/state owner.
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tauri_plugin_dialog::DialogExt;
use tokio::io::AsyncReadExt;

#[derive(Deserialize)]
struct Envelope<T> {
    command: String,
    ok: bool,
    data: Option<T>,
}
#[derive(Deserialize)]
struct ResolvedProject {
    project_id: String,
    project_root: String,
    state_exists: bool,
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

fn installed_runtime() -> Result<PathBuf, &'static str> {
    let path = std::env::var_os("FORGE_CORE_EXE")
        .map(PathBuf::from)
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

pub async fn query<T: DeserializeOwned>(
    root: &Path,
    args: &[&str],
    expected: &str,
) -> Result<T, &'static str> {
    let runtime = installed_runtime()?;
    let mut command = tokio::process::Command::new(&runtime);
    command
        .args(args)
        .arg("--root")
        .arg(root)
        .arg("--json")
        .current_dir(runtime.parent().ok_or("Instalação do Forge inválida.")?)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command
        .spawn()
        .map_err(|_| "Não foi possível iniciar a consulta ao Forge.")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Não foi possível ler a consulta.")?;
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let mut bytes = Vec::new();
        stdout.take(65537).read_to_end(&mut bytes).await.map_err(|_| "Não foi possível ler a resposta do Forge.")?;
        if bytes.len() > 65536 { return Err("A resposta do Forge excedeu o tamanho esperado."); }
        if !child.wait().await.map_err(|_| "A consulta ao Forge falhou.")?.success() {
            return Err("Não foi possível consultar o projeto. Confira a pasta e o vínculo com o Forge; nada foi alterado.");
        }
        let envelope: Envelope<T> = serde_json::from_slice(&bytes).map_err(|_| "O Forge retornou uma resposta incompatível.")?;
        if !envelope.ok || envelope.command != expected { return Err("O Forge não confirmou essa consulta."); }
        envelope.data.ok_or("O Forge não informou os dados da consulta.")
    }).await;
    if child.id().is_some() {
        let _ = child.kill().await;
    }
    result.map_err(|_| "O Forge demorou para responder. Você pode tentar novamente.")?
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
