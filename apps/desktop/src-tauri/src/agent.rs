//! Application session only. Codex owns conversation history; Forge owns project truth.
use crate::{codex_transport::Transport, project};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{ipc::Channel, State};

#[derive(Clone, Serialize)]
pub struct AgentEvent {
    kind: String,
    id: String,
    text: String,
}

#[derive(Default)]
struct Activity {
    turn: Option<String>,
    busy: bool,
    disconnected: bool,
}

struct Session {
    transport: Transport,
    thread: Mutex<Option<String>>,
    activity: Arc<Mutex<Activity>>,
}

#[derive(Default)]
pub struct AgentState {
    session: tokio::sync::Mutex<Option<Arc<Session>>>,
    shutdown: tokio::sync::Mutex<()>,
    closing: AtomicBool,
}

pub fn begin_close(state: &AgentState) -> bool {
    !state.closing.swap(true, Ordering::SeqCst)
}

fn executable() -> Result<PathBuf, &'static str> {
    let explicit = std::env::var_os("FORGE_CODEX_EXE").map(PathBuf::from);
    let npm = std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join(
        "npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe"));
    let path = explicit
        .or(npm)
        .ok_or("Instale o Codex CLI e entre na sua conta antes de conectar.")?;
    if !path.is_absolute() || !path.is_file() {
        return Err("Codex CLI não encontrado. Confira a instalação.");
    }
    Ok(path)
}

fn project_event(value: Value, activity: &Mutex<Activity>) -> Option<AgentEvent> {
    let mut activity = activity.lock().unwrap_or_else(|e| e.into_inner());
    let p = &value["params"];
    let mut event = AgentEvent {
        kind: String::new(),
        id: String::new(),
        text: String::new(),
    };
    match value["method"].as_str()? {
        "item/agentMessage/delta" => {
            event.kind = "delta".into();
            event.id = p["itemId"].as_str()?.into();
            event.text = p["delta"].as_str()?.into();
        }
        "item/completed" if p["item"]["type"] == "agentMessage" => {
            event.kind = "message".into();
            event.id = p["item"]["id"].as_str()?.into();
            event.text = p["item"]["text"].as_str()?.into();
        }
        "turn/started" => {
            activity.turn = p["turn"]["id"].as_str().map(str::to_owned);
            activity.busy = true;
            event.kind = "running".into();
        }
        "turn/completed" => {
            activity.turn = None;
            activity.busy = false;
            event.kind = match p["turn"]["status"].as_str() {
                Some("completed") => "completed",
                Some("interrupted") => "interrupted",
                Some("failed")
                    if p["turn"]["error"]["message"]
                        .as_str()
                        .is_some_and(|message| {
                            message.contains("requires a newer version of Codex")
                        }) =>
                {
                    "update_required"
                }
                _ => "failed",
            }
            .into();
        }
        "forge/disconnected" => {
            activity.busy = false;
            activity.turn = None;
            activity.disconnected = true;
            event.kind = "disconnected".into();
        }
        "forge/unsupportedInteraction" => {
            event.kind = "interaction_required".into();
        }
        "item/started" => {
            event.kind = "activity".into();
        }
        _ => return None,
    }
    Some(event)
}

#[tauri::command]
pub async fn connect_agent(
    project_root: String,
    events: Channel<AgentEvent>,
    state: State<'_, AgentState>,
) -> Result<String, &'static str> {
    let project = project::inspect_project(project_root).await?;
    let shutdown_guard = state.shutdown.lock().await;
    if state.closing.load(Ordering::SeqCst) {
        return Err("O aplicativo está fechando.");
    }
    let mut slot = state.session.lock().await;
    if slot.is_some() {
        return Err("Desconecte a conversa atual antes de abrir outra.");
    }
    let root = Path::new(&project.project_root);
    let activity = Arc::new(Mutex::new(Activity::default()));
    let observed = activity.clone();
    let transport = Transport::start(&executable()?, root, move |value| {
        if let Some(event) = project_event(value, &observed) {
            let _ = events.send(event);
        }
    })?;
    let session = Arc::new(Session {
        transport,
        thread: Mutex::new(None),
        activity,
    });
    *slot = Some(session.clone());
    drop(slot);
    drop(shutdown_guard);
    let result = initialize(&session, &project.project_root).await;
    if result.is_err() {
        release(&state, &session).await;
    }
    result
}

async fn initialize(session: &Session, project_root: &str) -> Result<String, &'static str> {
    let transport = &session.transport;
    transport
        .request(
            "initialize",
            json!({"clientInfo":{"name":"forge_desktop","version":"0.1.0"}}),
        )
        .await?;
    let account = transport
        .request("account/read", json!({"refreshToken":false}))
        .await?;
    if account["account"]["type"] != "chatgpt" {
        return Err("Entre na sua conta ChatGPT pelo Codex CLI e tente conectar novamente.");
    }
    let instructions = "You are the user's agent inside Forge desktop. Work only on the selected project unless the user explicitly requests otherwise. Use the installed start-forge skill once at the beginning of this conversation, and follow its structured handoff. Forge owns project continuity; use its public interfaces and do not create another state store. Explain progress in the user's language, clearly and simply. A completed response is not proof that the user's task is complete. Treat exploration as conversation, not acceptance. After interruption, reconcile actual effects before continuing. The interface currently cannot display interactive tool forms; ask the user in ordinary conversation when a decision is needed.";
    let result = transport.request("thread/start", json!({"cwd":project_root,"approvalPolicy":"never","sandbox":"danger-full-access","developerInstructions":instructions})).await?;
    let returned_root = result["thread"]["cwd"]
        .as_str()
        .ok_or("O Codex não confirmou a pasta.")?;
    let returned = Path::new(returned_root)
        .canonicalize()
        .map_err(|_| "Não foi possível conferir a pasta do Codex.")?;
    let requested = Path::new(project_root)
        .canonicalize()
        .map_err(|_| "A pasta do projeto não está mais disponível.")?;
    if returned != requested {
        return Err("O Codex abriu uma pasta diferente. A conversa não foi liberada.");
    }
    let thread = result["thread"]["id"]
        .as_str()
        .ok_or("O Codex não informou a conversa.")?
        .to_owned();
    *session.thread.lock().unwrap_or_else(|e| e.into_inner()) = Some(thread.clone());
    Ok(thread)
}

#[tauri::command]
pub async fn send_message(text: String, state: State<'_, AgentState>) -> Result<(), &'static str> {
    if text.trim().is_empty() || text.len() > 64000 {
        return Err("Escreva uma mensagem de até 64 mil bytes.");
    }
    let session = state
        .session
        .lock()
        .await
        .clone()
        .ok_or("Conecte o agente primeiro.")?;
    let thread = session
        .thread
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or("A conexão ainda está iniciando.")?;
    {
        let mut activity = session.activity.lock().unwrap_or_else(|e| e.into_inner());
        if activity.disconnected {
            return Err("A conexão foi encerrada. Desconecte e conecte novamente.");
        }
        if activity.busy {
            return Err("Aguarde a resposta ou interrompa antes de enviar outra mensagem.");
        }
        activity.busy = true;
    }
    let result = session
        .transport
        .request(
            "turn/start",
            json!({"threadId":thread,"input":[{"type":"text","text":text}]}),
        )
        .await;
    if result.is_err() {
        release(&state, &session).await;
    }
    result.map(|_| ())
}

#[tauri::command]
pub async fn interrupt_agent(state: State<'_, AgentState>) -> Result<(), &'static str> {
    let session = state
        .session
        .lock()
        .await
        .clone()
        .ok_or("Nenhuma conversa conectada.")?;
    let thread = session
        .thread
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or("A conexão ainda está iniciando.")?;
    let turn = session
        .activity
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .turn
        .clone()
        .ok_or("Não há uma resposta em andamento para interromper.")?;
    session
        .transport
        .request("turn/interrupt", json!({"threadId":thread,"turnId":turn}))
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn disconnect_agent(state: State<'_, AgentState>) -> Result<(), &'static str> {
    shutdown(&state).await;
    Ok(())
}

pub async fn shutdown(state: &AgentState) {
    let _shutdown = state.shutdown.lock().await;
    let session = { state.session.lock().await.take() };
    if let Some(session) = session {
        let turn = session
            .activity
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .turn
            .clone();
        let thread = session
            .thread
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(turn) = turn {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                session
                    .transport
                    .request("turn/interrupt", json!({"threadId":thread,"turnId":turn})),
            )
            .await;
        }
        session.transport.shutdown().await;
    }
}

async fn release(state: &AgentState, session: &Arc<Session>) {
    let _shutdown = state.shutdown.lock().await;
    {
        let mut slot = state.session.lock().await;
        if slot
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, session))
        {
            *slot = None;
        }
    }
    session.transport.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_window_close_starts_cleanup() {
        let state = AgentState::default();
        assert!(begin_close(&state));
        assert!(!begin_close(&state));
    }

    #[test]
    fn turn_events_release_admission_without_claiming_task_completion() {
        let activity = Mutex::new(Activity::default());
        let started = project_event(
            json!({"method":"turn/started","params":{"turn":{"id":"turn-1"}}}),
            &activity,
        )
        .unwrap();
        assert_eq!(started.kind, "running");
        assert!(activity.lock().unwrap().busy);
        for status in ["completed", "interrupted", "failed"] {
            let event = project_event(
                json!({"method":"turn/completed","params":{"turn":{"status":status}}}),
                &activity,
            )
            .unwrap();
            assert_eq!(event.kind, status);
            assert!(!activity.lock().unwrap().busy);
            assert!(activity.lock().unwrap().turn.is_none());
        }
    }

    #[test]
    fn protocol_details_do_not_leak_through_curated_status_events() {
        let activity = Mutex::new(Activity::default());
        assert!(project_event(
            json!({"method":"account/updated","params":{"secret":"private"}}),
            &activity
        )
        .is_none());
        let event = project_event(
            json!({"method":"forge/disconnected","params":{"error":"private"}}),
            &activity,
        )
        .unwrap();
        assert_eq!(event.kind, "disconnected");
        assert!(event.text.is_empty());
        assert!(activity.lock().unwrap().disconnected);
    }

    #[test]
    fn obsolete_cli_gets_actionable_status_without_forwarding_provider_error() {
        let activity = Mutex::new(Activity::default());
        let event = project_event(json!({"method":"turn/completed","params":{"turn":{"status":"failed","error":{"message":"The model requires a newer version of Codex. private details"}}}}), &activity).unwrap();
        assert_eq!(event.kind, "update_required");
        assert!(event.text.is_empty());
        assert!(!activity.lock().unwrap().busy);
    }
}
