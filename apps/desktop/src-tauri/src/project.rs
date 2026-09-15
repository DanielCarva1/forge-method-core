//! Read-only adapter to Forge's project resolver; no project state is owned here.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;

#[derive(Deserialize)]
struct Envelope {
    command: String,
    ok: bool,
    data: Option<ResolvedProject>,
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

fn installed_runtime() -> Result<PathBuf, &'static str> {
    // Explicit host configuration, never supplied by webview content.
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

#[tauri::command]
pub async fn inspect_project(project_root: String) -> Result<ProjectSummary, &'static str> {
    let requested = Path::new(&project_root);
    if !requested.is_absolute() || !requested.is_dir() {
        return Err("Informe o caminho completo de uma pasta que existe.");
    }
    let root = requested
        .canonicalize()
        .map_err(|_| "Não foi possível acessar essa pasta.")?;
    let runtime = installed_runtime()?;
    let mut command = tokio::process::Command::new(&runtime);
    command
        .args(["project", "resolve", "--root"])
        .arg(&root)
        .arg("--json")
        .current_dir(runtime.parent().ok_or("Instalação do Forge inválida.")?)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let mut child = command
        .spawn()
        .map_err(|_| "Não foi possível iniciar a consulta ao Forge.")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Não foi possível ler a consulta.")?;
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        let mut bytes = Vec::new();
        stdout.take(65537).read_to_end(&mut bytes).await
            .map_err(|_| "Não foi possível ler a resposta do Forge.")?;
        if bytes.len() > 65536 {
            return Err("A resposta do Forge excedeu o tamanho esperado.");
        }
        let status = child.wait().await.map_err(|_| "A consulta ao Forge falhou.")?;
        if !status.success() {
            return Err("Não foi possível identificar o projeto. Confira a pasta e o vínculo com o Forge; nada foi alterado.");
        }
        let envelope: Envelope = serde_json::from_slice(&bytes)
            .map_err(|_| "O Forge retornou uma resposta incompatível.")?;
        if !envelope.ok || envelope.command != "project.resolve" {
            return Err("O Forge não confirmou esse projeto.");
        }
        let data = envelope.data.ok_or("O Forge não informou o projeto.")?;
        if !data.state_exists {
            return Err("O estado deste projeto não está disponível. Nada foi recriado ou alterado.");
        }
        let resolved = Path::new(&data.project_root).canonicalize()
            .map_err(|_| "Não foi possível conferir a pasta retornada pelo Forge.")?;
        if resolved != root || data.project_id.trim().is_empty() {
            return Err("A resposta do Forge não corresponde à pasta informada.");
        }
        Ok(ProjectSummary { project_id: data.project_id, project_root: data.project_root })
    }).await;
    // Also terminate a malformed or oversized response producer before returning.
    if child.id().is_some() {
        let _ = child.kill().await;
    }
    result.map_err(|_| "O Forge demorou para responder. Você pode tentar novamente.")?
}
