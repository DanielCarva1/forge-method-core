//! Application session only. Codex owns conversation history; Forge owns project truth.
use crate::{
    codex_transport::{Transport, REQUEST_REJECTED},
    history, project,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{ipc::Channel, Manager, State};

// One turn per frame avoids an arbitrary batch becoming oversized. A single giant turn still
// fails through the transport's existing per-frame boundary rather than being truncated.
const HISTORY_PAGE_LIMIT: usize = 1;
const MAX_HISTORY_PAGES: usize = 4096;
const MAX_HISTORY_BYTES: usize = 128 * 1024 * 1024;
const HISTORY_LOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const HISTORY_COMPATIBILITY: &str = "Não foi possível carregar o histórico paginado. Confira a conexão e se o Codex CLI está atualizado. Nada foi apagado; a conversa continua disponível pelo Codex CLI.";
const HISTORY_INVALID: &str =
    "O Codex retornou um histórico incompatível. Nada foi apagado; tente novamente pelo Codex CLI.";
const HISTORY_LIMIT: &str = "O histórico ultrapassou o limite seguro de recuperação deste app. Nada foi apagado. Continue essa conversa pelo Codex CLI.";

trait Protocol {
    async fn request(&self, method: &'static str, params: Value) -> Result<Value, &'static str>;
}

impl Protocol for Transport {
    async fn request(&self, method: &'static str, params: Value) -> Result<Value, &'static str> {
        Transport::request(self, method, params).await
    }
}

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

#[derive(Serialize)]
pub struct ConversationChoice {
    id: String,
    title: String,
    updated_at: u64,
    active: bool,
}

#[derive(Serialize)]
pub struct ConversationPage {
    conversations: Vec<ConversationChoice>,
    next_cursor: Option<String>,
}

const CONVERSATION_LIST_INVALID: &str =
    "O Codex retornou uma lista de conversas incompatível. Nenhuma conversa foi aberta.";

fn conversation_page(value: Value, root: &str) -> Result<ConversationPage, &'static str> {
    let data = value["data"].as_array().ok_or(CONVERSATION_LIST_INVALID)?;
    if data.len() > 50 {
        return Err(CONVERSATION_LIST_INVALID);
    }
    let next_cursor = match value.get("nextCursor") {
        Some(Value::Null) => None,
        Some(Value::String(cursor)) if !cursor.is_empty() && cursor.len() <= 1024 => {
            Some(cursor.clone())
        }
        _ => return Err(CONVERSATION_LIST_INVALID),
    };
    let mut conversations = Vec::new();
    for thread in data {
        match thread.get("parentThreadId") {
            Some(Value::Null) => {}
            Some(Value::String(_)) => continue, // Not a separate user conversation.
            _ => return Err(CONVERSATION_LIST_INVALID),
        }
        let id = history::validate(thread, root, None).map_err(|_| CONVERSATION_LIST_INVALID)?;
        if id.len() > 200 {
            return Err(CONVERSATION_LIST_INVALID);
        }
        let title = thread["name"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .or_else(|| thread["preview"].as_str())
            .ok_or(CONVERSATION_LIST_INVALID)?
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .chars()
            .take(160)
            .collect::<String>();
        let title = if title.is_empty() {
            "Conversa sem título".into()
        } else {
            title
        };
        let updated_at = thread["updatedAt"]
            .as_u64()
            .filter(|time| *time <= 253_402_300_799)
            .ok_or(CONVERSATION_LIST_INVALID)?;
        let active = match thread["status"]["type"].as_str() {
            Some("active") => true,
            Some("idle" | "notLoaded" | "systemError") => false,
            _ => return Err(CONVERSATION_LIST_INVALID),
        };
        conversations.push(ConversationChoice {
            id,
            title,
            updated_at,
            active,
        });
    }
    Ok(ConversationPage {
        conversations,
        next_cursor,
    })
}

/// Read-only list from Codex's own index, scoped to an already confirmed Forge folder.
#[tauri::command]
pub async fn list_conversations(
    project_root: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
) -> Result<ConversationPage, &'static str> {
    if cursor
        .as_ref()
        .is_some_and(|cursor| cursor.is_empty() || cursor.len() > 1024)
    {
        return Err("A referência da próxima página é inválida.");
    }
    let project = project::inspect_project(project_root).await?;
    let root = Path::new(&project.project_root);
    let resource_dir = app.path().resource_dir().ok();
    let runtime = project::installed_runtime()?;
    let transport = Transport::start(&executable(resource_dir.as_deref())?, root, Some(&runtime), |_| {})?;
    let result = async {
        transport
            .request(
                "initialize",
                json!({"clientInfo":{"name":"forge_desktop","version":env!("CARGO_PKG_VERSION")}}),
            )
            .await?;
        let account = transport
            .request("account/read", json!({"refreshToken":false}))
            .await?;
        if account["account"]["type"] != "chatgpt" {
            return Err("Entre na sua conta ChatGPT pelo Forge e tente novamente.");
        }
        let page = transport
            .request(
                "thread/list",
                json!({"cwd":project.project_root,"cursor":cursor,"limit":6,"sortKey":"updated_at","sortDirection":"desc","useStateDbOnly":true}),
            )
            .await?;
        conversation_page(page, &project.project_root)
    }
    .await;
    transport.shutdown().await;
    result
}

#[derive(Default)]
pub struct AgentState {
    session: tokio::sync::Mutex<Option<Arc<Session>>>,
    login: tokio::sync::Mutex<Option<LoginSession>>,
    shutdown: tokio::sync::Mutex<()>,
    closing: AtomicBool,
}

struct LoginSession {
    transport: Transport,
    login_id: String,
    verification_url: String,
}

#[derive(Serialize)]
pub struct LoginChallenge {
    user_code: String,
    verification_url: String,
}

#[derive(Serialize)]
pub struct LoginEvent {
    success: bool,
}

const LOGIN_UNAVAILABLE: &str = "Não foi possível iniciar o acesso ao ChatGPT. Tente novamente mais tarde.";

fn login_challenge(value: &Value) -> Result<(String, LoginChallenge), &'static str> {
    let login_id = value["loginId"].as_str().filter(|s| !s.is_empty() && s.len() <= 200)
        .ok_or(LOGIN_UNAVAILABLE)?;
    let code = value["userCode"].as_str().filter(|s| !s.is_empty() && s.len() <= 40
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .ok_or(LOGIN_UNAVAILABLE)?;
    let url = value["verificationUrl"].as_str().filter(|s| *s == "https://auth.openai.com/codex/device")
        .ok_or(LOGIN_UNAVAILABLE)?;
    if value["type"] != "chatgptDeviceCode" { return Err(LOGIN_UNAVAILABLE); }
    Ok((login_id.to_owned(), LoginChallenge { user_code: code.to_owned(), verification_url: url.to_owned() }))
}

#[cfg(test)]
mod login_tests {
    use super::*;

    #[test]
    fn accepts_only_codex_device_login_with_expected_origin() {
        let valid = json!({"type":"chatgptDeviceCode","loginId":"opaque","userCode":"ABCD-1234","verificationUrl":"https://auth.openai.com/codex/device"});
        let (id, challenge) = login_challenge(&valid).unwrap();
        assert_eq!(id, "opaque");
        assert_eq!(challenge.user_code, "ABCD-1234");
        for bad in [
            json!({"type":"chatgptDeviceCode","loginId":"opaque","userCode":"ABCD-1234","verificationUrl":"https://evil.example/codex/device"}),
            json!({"type":"chatgpt","loginId":"opaque","userCode":"ABCD-1234","verificationUrl":"https://auth.openai.com/codex/device"}),
            json!({"type":"chatgptDeviceCode","loginId":"opaque","userCode":"bad code","verificationUrl":"https://auth.openai.com/codex/device"}),
        ] { assert!(login_challenge(&bad).is_err()); }
    }
}

/// Starts Codex-managed device authorization. Forge never receives account credentials.
#[tauri::command]
pub async fn start_login(
    events: Channel<LoginEvent>,
    state: State<'_, AgentState>,
    app: tauri::AppHandle,
) -> Result<LoginChallenge, &'static str> {
    let _guard = state.shutdown.lock().await;
    if state.closing.load(Ordering::SeqCst) { return Err("O aplicativo está fechando."); }
    if state.session.lock().await.is_some() { return Err("Desconecte a conversa antes de trocar a conta."); }
    let mut slot = state.login.lock().await;
    if slot.is_some() { return Err("O acesso já está em andamento."); }
    let resource_dir = app.path().resource_dir().ok();
    let root = std::env::current_dir().map_err(|_| LOGIN_UNAVAILABLE)?;
    let transport = Transport::start(&executable(resource_dir.as_deref())?, &root, None, move |value| {
        if value["method"] == "account/login/completed" {
            if let Some(success) = value["params"]["success"].as_bool() {
                let _ = events.send(LoginEvent { success });
            }
        }
    })?;
    let result = async {
        transport.request("initialize", json!({"clientInfo":{"name":"forge_desktop","version":env!("CARGO_PKG_VERSION")}})).await?;
        let response = transport.request("account/login/start", json!({"type":"chatgptDeviceCode"})).await?;
        login_challenge(&response)
    }.await;
    match result {
        Ok((login_id, challenge)) => {
            *slot = Some(LoginSession { transport, login_id, verification_url: challenge.verification_url.clone() });
            Ok(challenge)
        }
        Err(error) => { transport.shutdown().await; Err(error) }
    }
}

#[tauri::command]
pub async fn finish_login(state: State<'_, AgentState>) -> Result<bool, &'static str> {
    let _guard = state.shutdown.lock().await;
    let mut slot = state.login.lock().await;
    let session = slot.as_ref().ok_or("Nenhum acesso está em andamento.")?;
    let account = session.transport.request("account/read", json!({"refreshToken":false})).await?;
    if account["account"]["type"] != "chatgpt" { return Ok(false); }
    let session = slot.take().expect("checked above");
    session.transport.shutdown().await;
    Ok(true)
}

#[tauri::command]
pub async fn cancel_login(state: State<'_, AgentState>) -> Result<(), &'static str> {
    let _guard = state.shutdown.lock().await;
    let session = state.login.lock().await.take();
    if let Some(session) = session {
        let _ = session.transport.request("account/login/cancel", json!({"loginId":session.login_id})).await;
        session.transport.shutdown().await;
    }
    Ok(())
}

#[tauri::command]
pub async fn open_login_page(state: State<'_, AgentState>) -> Result<(), &'static str> {
    let slot = state.login.lock().await;
    let session = slot.as_ref().ok_or("Inicie o acesso primeiro.")?;
    if session.verification_url != "https://auth.openai.com/codex/device" { return Err(LOGIN_UNAVAILABLE); }
    #[cfg(windows)]
    {
        // Let Windows use the user's default HTTPS handler, not Explorer's undocumented URL arguments.
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(
                window: isize,
                operation: *const u16,
                file: *const u16,
                parameters: *const u16,
                directory: *const u16,
                show: i32,
            ) -> isize;
        }
        let operation: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
        let url: Vec<u16> = session.verification_url.encode_utf16().chain(std::iter::once(0)).collect();
        // ShellExecuteW reports an accepted launch request when its result is greater than 32.
        let result = unsafe {
            ShellExecuteW(0, operation.as_ptr(), url.as_ptr(), std::ptr::null(), std::ptr::null(), 1)
        };
        if result <= 32 { Err("Não foi possível abrir o navegador. Copie o endereço mostrado na tela.") }
        else { Ok(()) }
    }
    #[cfg(not(windows))]
    { Err("Abra o endereço mostrado na tela em seu navegador.") }
}

pub fn begin_close(state: &AgentState) -> bool {
    !state.closing.swap(true, Ordering::SeqCst)
}

fn executable(resource_dir: Option<&Path>) -> Result<PathBuf, &'static str> {
    let explicit = std::env::var_os("FORGE_CODEX_EXE").map(PathBuf::from);
    let bundled = resource_dir.and_then(bundled_codex);
    let desktop = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .and_then(|base| desktop_codex(&base));
    let npm = std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join(
        "npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe"));
    let path = preferred_executable(explicit, bundled, desktop, npm)
        .ok_or("Instale o Codex CLI e entre na sua conta antes de conectar.")?;
    if !path.is_absolute() || !path.is_file() {
        return Err("Codex CLI não encontrado. Confira a instalação.");
    }
    Ok(path)
}

fn bundled_codex(resource_dir: &Path) -> Option<PathBuf> {
    let path = resource_dir.join("codex-cli/bin/codex.exe");
    path.is_file().then_some(path)
}

fn preferred_executable(
    explicit: Option<PathBuf>,
    bundled: Option<PathBuf>,
    desktop: Option<PathBuf>,
    npm: Option<PathBuf>,
) -> Option<PathBuf> {
    explicit.or(bundled).or(desktop).or(npm)
}

fn desktop_codex(local_app_data: &Path) -> Option<PathBuf> {
    let bin = local_app_data.join("OpenAI/Codex/bin");
    std::fs::read_dir(bin)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if !entry.file_type().ok()?.is_dir() {
                return None;
            }
            let executable = entry.path().join("codex.exe");
            if !executable.symlink_metadata().ok()?.file_type().is_file() {
                return None;
            }
            let modified = executable.metadata().ok()?.modified().ok()?;
            Some((modified, executable))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, executable)| executable)
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
            event.text = match p["item"]["type"].as_str() {
                Some("commandExecution") => "checking",
                Some("fileChange") => "editing",
                Some("agentMessage") => "replying",
                _ => "working",
            }
            .into();
        }
        _ => return None,
    }
    Some(event)
}

#[tauri::command]
pub async fn connect_agent(
    project_root: String,
    thread_id: Option<String>,
    events: Channel<AgentEvent>,
    state: State<'_, AgentState>,
    app: tauri::AppHandle,
) -> Result<history::Conversation, &'static str> {
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
    let resource_dir = app.path().resource_dir().ok();
    let runtime = project::installed_runtime()?;
    let transport = Transport::start(&executable(resource_dir.as_deref())?, root, Some(&runtime), move |value| {
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
    let result = initialize(
        &session,
        &project.project_root,
        thread_id.as_deref(),
        resource_dir.as_deref(),
        &runtime,
    )
    .await;
    if result.is_err() {
        release(&state, &session).await;
    }
    result
}

async fn initialize(
    session: &Session,
    project_root: &str,
    thread_id: Option<&str>,
    resource_dir: Option<&Path>,
    runtime: &Path,
) -> Result<history::Conversation, &'static str> {
    let transport = &session.transport;
    transport
        .request(
            "initialize",
            json!({"clientInfo":{"name":"forge_desktop","version":env!("CARGO_PKG_VERSION")}}),
        )
        .await?;
    let account = transport
        .request("account/read", json!({"refreshToken":false}))
        .await?;
    if account["account"]["type"] != "chatgpt" {
        return Err("Entre na sua conta ChatGPT pelo Forge e tente conectar novamente.");
    }
    let instructions = developer_instructions(resource_dir, runtime)?;
    let mut params = json!({"cwd":project_root,"approvalPolicy":"never","sandbox":"danger-full-access","developerInstructions":instructions});
    let (thread, messages) = if let Some(id) = thread_id {
        if id.is_empty() || id.len() > 200 {
            return Err("A referência da conversa é inválida.");
        }
        params["threadId"] = json!(id);
        resume_saved(transport, project_root, id, params).await?
    } else {
        let result = transport.request("thread/start", params).await?;
        if result["thread"]["status"]["type"] == "active" {
            return Err("Esta conversa ainda está em execução. Aguarde antes de retomá-la.");
        }
        let thread = history::validate(&result["thread"], project_root, None)?;
        (thread, Vec::new())
    };
    *session.thread.lock().unwrap_or_else(|e| e.into_inner()) = Some(thread.clone());
    Ok(history::Conversation {
        thread_id: thread,
        messages,
        resumed: thread_id.is_some(),
    })
}

fn developer_instructions(
    resource_dir: Option<&Path>,
    runtime: &Path,
) -> Result<String, &'static str> {
    let skill = resource_dir
        .map(|dir| dir.join("forge-core/start-forge/SKILL.md"))
        .filter(|path| path.is_file());
    let start = if let Some(path) = skill {
        format!(
            "Read the opening guidance and Guided activation contract at `{}` once at the beginning of this conversation; then read only the workflow sections relevant to this project and the current effect. Follow the structured handoff. The native Forge executable is already resolved below, so do not perform unrelated binary or WSL discovery. A small reversible local result does not require Work Focus, phase closeout, or evidence admission merely to proceed. Do not use a separately installed Start Forge skill instead.",
            path.display()
        )
    } else {
        #[cfg(not(debug_assertions))]
        return Err("A instalação do Forge está incompleta. Reinstale o aplicativo para conectar o agente.");
        #[cfg(debug_assertions)]
        "Use the installed start-forge skill once at the beginning of this conversation, and follow its structured handoff.".to_string()
    };
    Ok(format!("You are the user's agent inside Forge desktop. Work only on the selected project unless the user explicitly requests otherwise. {start} The app already resolved the exact Forge executable for this project: `{}`. Use that absolute executable for every Forge command in this conversation. Do not select another copy from PATH, Cargo bin, a global installer, or WSL. Check its --version before activation; if the host cannot access it, explain the problem instead of silently switching copies. Forge owns project continuity; use its public interfaces and do not create another state store. For a Forge command requiring a temporary JSON input, write the file, invoke Forge, and clean up in separate tool calls; never compose all three operations into one shell command. If the host blocks an operation, do not bypass its policy. Explain progress in the user's language, clearly and simply. A completed response is not proof that the user's task is complete. Treat exploration as conversation, not acceptance. After interruption, reconcile actual effects before continuing. When you create or substantially change a reviewable local file, identify only files that actually exist and include a Markdown link with a path relative to the project root, such as [Ver página](site/index.html), so the user can inspect it in the app. Do not imply a local file is published or that an unsupported output has a visual preview. The interface currently cannot display interactive tool forms; ask the user in ordinary conversation when a decision is needed.", runtime.display()))
}

async fn resume_saved<P: Protocol>(
    protocol: &P,
    project_root: &str,
    thread_id: &str,
    mut params: Value,
) -> Result<(String, Vec<history::Message>), &'static str> {
    let stored = protocol
        .request(
            "thread/read",
            json!({"threadId":thread_id,"includeTurns":false}),
        )
        .await
        .map_err(|_| "Não foi possível abrir a conversa salva. Tente novamente ou escolha começar outra; o histórico não será apagado.")?;
    history::validate(&stored["thread"], project_root, Some(thread_id))?;

    params["threadId"] = json!(thread_id);
    params["excludeTurns"] = json!(true);
    let result = protocol
        .request("thread/resume", params)
        .await
        .map_err(|error| {
            if error == REQUEST_REJECTED {
                HISTORY_COMPATIBILITY
            } else {
                error
            }
        })?;
    let thread = history::validate(&result["thread"], project_root, Some(thread_id))?;
    if result["thread"]["status"]["type"] == "active" {
        return Err("Esta conversa ainda está em execução. Aguarde antes de retomá-la.");
    }

    let turns = result["thread"]["turns"]
        .as_array()
        .ok_or(HISTORY_INVALID)?;
    // Older app-servers may ignore excludeTurns and return a bounded history directly.
    let messages = if turns.is_empty() {
        tokio::time::timeout(HISTORY_LOAD_TIMEOUT, paginated_history(protocol, thread_id))
            .await
            .map_err(|_| HISTORY_LIMIT)??
    } else {
        history::messages(&result["thread"])?
    };
    Ok((thread, messages))
}

async fn paginated_history<P: Protocol>(
    protocol: &P,
    thread_id: &str,
) -> Result<Vec<history::Message>, &'static str> {
    let mut messages = Vec::new();
    let mut cursor: Option<String> = None;
    let mut seen = HashSet::new();
    let mut seen_turns = HashSet::new();
    let mut received_bytes = 0usize;

    for _ in 0..MAX_HISTORY_PAGES {
        let page = protocol
            .request(
                "thread/turns/list",
                json!({
                    "threadId": thread_id,
                    "cursor": cursor,
                    "limit": HISTORY_PAGE_LIMIT,
                    "sortDirection": "asc",
                    "itemsView": "full"
                }),
            )
            .await
            .map_err(|error| {
                if error == REQUEST_REJECTED {
                    HISTORY_COMPATIBILITY
                } else {
                    error
                }
            })?;
        let page_bytes = serde_json::to_vec(&page)
            .map_err(|_| HISTORY_INVALID)?
            .len();
        received_bytes = add_history_bytes(received_bytes, page_bytes)?;
        let turns = page["data"].as_array().ok_or(HISTORY_INVALID)?;
        if turns.len() > HISTORY_PAGE_LIMIT {
            return Err(HISTORY_INVALID);
        }
        for turn in turns {
            let turn_id = turn["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or(HISTORY_INVALID)?;
            if !seen_turns.insert(turn_id.to_owned()) {
                return Err(HISTORY_INVALID);
            }
        }
        messages.extend(history::messages_from_turns(turns)?);

        let next = match page.get("nextCursor") {
            Some(Value::Null) => return Ok(messages),
            Some(Value::String(next)) if !next.is_empty() => next.clone(),
            _ => return Err(HISTORY_INVALID),
        };
        if turns.is_empty() || !seen.insert(next.clone()) {
            return Err(HISTORY_INVALID);
        }
        cursor = Some(next);
    }
    Err(HISTORY_LIMIT)
}

fn add_history_bytes(current: usize, page: usize) -> Result<usize, &'static str> {
    current
        .checked_add(page)
        .filter(|size| *size <= MAX_HISTORY_BYTES)
        .ok_or(HISTORY_LIMIT)
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
    if let Some(login) = state.login.lock().await.take() {
        let _ = login.transport.request("account/login/cancel", json!({"loginId":login.login_id})).await;
        login.transport.shutdown().await;
    }
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
    use crate::codex_transport::MAX_FRAME_BYTES;
    use std::collections::VecDeque;

    #[test]
    fn finds_only_a_direct_codex_desktop_binary() {
        let base =
            std::env::temp_dir().join(format!("forge-codex-discovery-{}", uuid::Uuid::new_v4()));
        let bin = base.join("OpenAI/Codex/bin");
        let current = bin.join("current");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(bin.join("incomplete")).unwrap();
        let executable = current.join("codex.exe");
        std::fs::write(&executable, b"fixture").unwrap();
        assert_eq!(desktop_codex(&base), Some(executable));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn bundled_codex_is_used_only_when_present_and_precedes_machine_installs() {
        let base =
            std::env::temp_dir().join(format!("forge-codex-resource-{}", uuid::Uuid::new_v4()));
        let bundled = base.join("codex-cli/bin/codex.exe");
        let old = PathBuf::from("C:/old/codex.exe");
        assert_eq!(bundled_codex(&base), None);
        std::fs::create_dir_all(bundled.parent().unwrap()).unwrap();
        std::fs::write(&bundled, b"fixture").unwrap();
        assert_eq!(bundled_codex(&base), Some(bundled.clone()));
        assert_eq!(
            preferred_executable(None, Some(bundled.clone()), Some(old.clone()), None),
            Some(bundled.clone())
        );
        assert_eq!(
            preferred_executable(Some(old.clone()), Some(bundled), None, None),
            Some(old)
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn agent_prefers_bundled_start_forge_guidance() {
        let base =
            std::env::temp_dir().join(format!("forge-start-guidance-{}", uuid::Uuid::new_v4()));
        let skill = base.join("forge-core/start-forge/SKILL.md");
        std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
        std::fs::write(&skill, b"# Start Forge").unwrap();
        let runtime = base.join("forge-core/forge-core.exe");
        let instructions = developer_instructions(Some(&base), &runtime).unwrap();
        assert!(instructions.contains(&skill.display().to_string()));
        assert!(instructions.contains(&runtime.display().to_string()));
        assert!(instructions.contains("Do not select another copy from PATH"));
        assert!(instructions.contains("Do not use a separately installed Start Forge skill"));
        assert!(instructions.contains("does not require Work Focus, phase closeout, or evidence admission"));
        assert!(instructions
            .contains("write the file, invoke Forge, and clean up in separate tool calls"));
        assert!(!instructions.contains("Use the installed start-forge skill"));
        std::fs::remove_dir_all(base).unwrap();
    }

    struct FakeProtocol {
        replies: Mutex<VecDeque<Result<Value, &'static str>>>,
        requests: Mutex<Vec<(&'static str, Value)>>,
    }

    impl FakeProtocol {
        fn new(replies: Vec<Result<Value, &'static str>>) -> Self {
            Self {
                replies: Mutex::new(replies.into()),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    impl Protocol for FakeProtocol {
        async fn request(
            &self,
            method: &'static str,
            params: Value,
        ) -> Result<Value, &'static str> {
            self.requests.lock().unwrap().push((method, params));
            self.replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected protocol request")
        }
    }

    fn thread(root: &str, turns: Value) -> Value {
        json!({"id":"saved-thread","cwd":root,"status":{"type":"idle"},"turns":turns})
    }

    fn text_turn(id: &str, status: &str, item_type: &str, text: String) -> Value {
        if item_type == "userMessage" {
            json!({"id":id,"status":status,"items":[{"id":format!("{id}-item"),"type":"userMessage","content":[{"type":"text","text":text}]}]})
        } else {
            json!({"id":id,"status":status,"items":[{"id":format!("{id}-item"),"type":"agentMessage","text":text}]})
        }
    }

    #[test]
    fn conversation_list_projects_only_same_folder_top_level_threads() {
        let root = std::env::current_dir().unwrap();
        let root = root.to_str().unwrap();
        let page = conversation_page(
            json!({"data":[
                {"id":"older","cwd":root,"parentThreadId":null,"name":"  Chosen title  ","preview":"Other preview","updatedAt":100,"status":{"type":"notLoaded"}},
                {"id":"agent","cwd":root,"parentThreadId":"older","name":null,"preview":"Subagent text","updatedAt":90,"status":{"type":"idle"}},
                {"id":"busy","cwd":root,"parentThreadId":null,"name":null,"preview":"<script>literal</script>\nrest","updatedAt":80,"status":{"type":"active"}}
            ],"nextCursor":"next"}),
            root,
        )
        .unwrap();
        assert_eq!(page.conversations.len(), 2);
        assert_eq!(page.conversations[0].id, "older");
        assert_eq!(page.conversations[0].title, "Chosen title");
        assert_eq!(page.conversations[1].title, "<script>literal</script>");
        assert!(page.conversations[1].active);
        assert_eq!(page.next_cursor.as_deref(), Some("next"));
    }

    #[test]
    fn conversation_list_rejects_wrong_folder_or_malformed_page() {
        let root = std::env::current_dir().unwrap();
        let root = root.to_str().unwrap();
        let thread = json!({"id":"other","cwd":"missing-project","parentThreadId":null,"name":null,"preview":"Another project","updatedAt":100,"status":{"type":"idle"}});
        assert!(conversation_page(json!({"data":[thread],"nextCursor":null}), root).is_err());
        assert!(conversation_page(json!({"data":[],"nextCursor":123}), root).is_err());
        assert!(conversation_page(
            json!({"data":[{"id":"missing-fields","cwd":root}],"nextCursor":null}),
            root
        )
        .is_err());
    }

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
        for (item_type, expected) in [
            ("commandExecution", "checking"),
            ("fileChange", "editing"),
            ("agentMessage", "replying"),
            ("unexpected", "working"),
        ] {
            let event = project_event(
                json!({"method":"item/started","params":{"item":{"type":item_type,"command":"private","text":"private"}}}),
                &activity,
            ).unwrap();
            assert_eq!(event.kind, "activity");
            assert_eq!(event.text, expected);
            assert!(!event.text.contains("private"));
        }
    }

    #[test]
    fn obsolete_cli_gets_actionable_status_without_forwarding_provider_error() {
        let activity = Mutex::new(Activity::default());
        let event = project_event(json!({"method":"turn/completed","params":{"turn":{"status":"failed","error":{"message":"The model requires a newer version of Codex. private details"}}}}), &activity).unwrap();
        assert_eq!(event.kind, "update_required");
        assert!(event.text.is_empty());
        assert!(!activity.lock().unwrap().busy);
    }

    #[tokio::test]
    async fn lean_resume_pages_without_replaying_a_turn() {
        let root = std::env::current_dir().unwrap();
        let root = root.to_str().unwrap();
        let first = json!({
            "data":[text_turn("first", "completed", "userMessage", "u".repeat(600_000))],
            "nextCursor":"page-two"
        });
        let second = json!({
            "data":[{"id":"second","status":"interrupted","items":[
                {"id":"tool","type":"commandExecution","command":"never replay"},
                {"id":"second-item","type":"agentMessage","text":"a".repeat(600_000)}
            ]}],
            "nextCursor":null
        });
        assert!(serde_json::to_vec(&first).unwrap().len() < MAX_FRAME_BYTES);
        assert!(serde_json::to_vec(&second).unwrap().len() < MAX_FRAME_BYTES);
        let fake = FakeProtocol::new(vec![
            Ok(json!({"thread":thread(root, json!([]))})),
            Ok(json!({"thread":thread(root, json!([]))})),
            Ok(first),
            Ok(second),
        ]);

        let (_, messages) = resume_saved(&fake, root, "saved-thread", json!({}))
            .await
            .unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, "first-item");
        assert_eq!(messages[1].id, "second-item");
        assert!(!messages[0].incomplete);
        assert!(messages[1].incomplete);
        let requests = fake.requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .map(|(method, _)| *method)
                .collect::<Vec<_>>(),
            [
                "thread/read",
                "thread/resume",
                "thread/turns/list",
                "thread/turns/list"
            ]
        );
        assert_eq!(requests[1].1["excludeTurns"], true);
        assert_eq!(requests[2].1["itemsView"], "full");
        assert_eq!(requests[2].1["sortDirection"], "asc");
        assert_eq!(requests[3].1["cursor"], "page-two");
        assert!(!requests.iter().any(|(method, _)| *method == "turn/start"));
    }

    #[tokio::test]
    async fn legacy_bounded_resume_is_projected_without_pagination() {
        let root = std::env::current_dir().unwrap();
        let root = root.to_str().unwrap();
        let turns = json!([text_turn(
            "legacy",
            "completed",
            "agentMessage",
            "saved".into()
        )]);
        let fake = FakeProtocol::new(vec![
            Ok(json!({"thread":thread(root, json!([]))})),
            Ok(json!({"thread":thread(root, turns)})),
        ]);

        let (_, messages) = resume_saved(&fake, root, "saved-thread", json!({}))
            .await
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "saved");
        assert_eq!(fake.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn legacy_resume_rejects_partial_item_view() {
        let root = std::env::current_dir().unwrap();
        let root = root.to_str().unwrap();
        let partial_turns = json!([{
            "id":"legacy",
            "status":"completed",
            "itemsView":"summary",
            "items":[{"id":"message","type":"agentMessage","text":"partial"}]
        }]);
        let fake = FakeProtocol::new(vec![
            Ok(json!({"thread":thread(root, json!([]))})),
            Ok(json!({"thread":thread(root, partial_turns)})),
        ]);

        assert!(resume_saved(&fake, root, "saved-thread", json!({}))
            .await
            .is_err());
        assert_eq!(fake.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn pagination_rejects_malformed_and_cyclic_pages() {
        let malformed = FakeProtocol::new(vec![Ok(json!({"data":{},"nextCursor":null}))]);
        assert_eq!(
            paginated_history(&malformed, "saved-thread")
                .await
                .unwrap_err(),
            HISTORY_INVALID
        );

        let cyclic = FakeProtocol::new(vec![
            Ok(
                json!({"data":[text_turn("one", "completed", "agentMessage", "one".into())],"nextCursor":"same"}),
            ),
            Ok(
                json!({"data":[text_turn("two", "completed", "agentMessage", "two".into())],"nextCursor":"same"}),
            ),
        ]);
        assert_eq!(
            paginated_history(&cyclic, "saved-thread")
                .await
                .unwrap_err(),
            HISTORY_INVALID
        );
    }

    #[tokio::test]
    async fn pagination_rejects_partial_item_views_and_duplicate_turns() {
        for view in ["summary", "notLoaded", "unknown"] {
            let partial = FakeProtocol::new(vec![Ok(json!({
                "data":[{
                    "id":"partial",
                    "status":"completed",
                    "itemsView":view,
                    "items":[]
                }],
                "nextCursor":null
            }))]);
            assert!(paginated_history(&partial, "saved-thread").await.is_err());
        }

        let duplicate = FakeProtocol::new(vec![
            Ok(json!({
                "data":[text_turn("same-turn", "completed", "agentMessage", "one".into())],
                "nextCursor":"different-page"
            })),
            Ok(json!({
                "data":[text_turn("same-turn", "completed", "agentMessage", "two".into())],
                "nextCursor":null
            })),
        ]);
        assert_eq!(
            paginated_history(&duplicate, "saved-thread")
                .await
                .unwrap_err(),
            HISTORY_INVALID
        );

        let missing_id = FakeProtocol::new(vec![Ok(json!({
            "data":[{"status":"completed","items":[]}],
            "nextCursor":null
        }))]);
        assert_eq!(
            paginated_history(&missing_id, "saved-thread")
                .await
                .unwrap_err(),
            HISTORY_INVALID
        );
    }

    #[tokio::test]
    async fn pagination_has_finite_page_and_aggregate_limits() {
        let mut replies = Vec::new();
        for index in 0..MAX_HISTORY_PAGES {
            replies.push(Ok(json!({
                "data":[text_turn(&format!("turn-{index}"), "completed", "agentMessage", "x".into())],
                "nextCursor":format!("cursor-{index}")
            })));
        }
        let fake = FakeProtocol::new(replies);
        assert_eq!(
            paginated_history(&fake, "saved-thread").await.unwrap_err(),
            HISTORY_LIMIT
        );
        assert_eq!(fake.requests.lock().unwrap().len(), MAX_HISTORY_PAGES);
        assert_eq!(
            add_history_bytes(MAX_HISTORY_BYTES - 1, 1),
            Ok(MAX_HISTORY_BYTES)
        );
        assert_eq!(add_history_bytes(MAX_HISTORY_BYTES, 1), Err(HISTORY_LIMIT));
    }

    #[test]
    fn substantial_paged_conversation_fits_aggregate_budget() {
        assert_eq!(
            add_history_bytes(95 * 1024 * 1024, 1024),
            Ok(95 * 1024 * 1024 + 1024)
        );
    }

    #[tokio::test]
    async fn rejected_lean_resume_has_a_compatibility_error() {
        let root = std::env::current_dir().unwrap();
        let root = root.to_str().unwrap();
        let fake = FakeProtocol::new(vec![
            Ok(json!({"thread":thread(root, json!([]))})),
            Err(REQUEST_REJECTED),
        ]);
        assert_eq!(
            resume_saved(&fake, root, "saved-thread", json!({}))
                .await
                .unwrap_err(),
            HISTORY_COMPATIBILITY
        );
    }

    #[tokio::test]
    async fn active_writer_resume_keeps_specific_user_message() {
        let root = std::env::current_dir().unwrap();
        let root = root.to_str().unwrap();
        let fake = FakeProtocol::new(vec![
            Ok(json!({"thread":thread(root, json!([]))})),
            Err(crate::codex_transport::THREAD_IN_USE),
        ]);
        assert_eq!(
            resume_saved(&fake, root, "saved-thread", json!({}))
                .await
                .unwrap_err(),
            crate::codex_transport::THREAD_IN_USE
        );
    }

    #[tokio::test]
    async fn preflight_and_resume_identity_mismatches_stop_recovery() {
        let root = std::env::current_dir().unwrap();
        let root = root.to_str().unwrap();
        let wrong_preflight = FakeProtocol::new(vec![Ok(json!({
            "thread": {"id":"other","cwd":root,"turns":[]}
        }))]);
        assert!(
            resume_saved(&wrong_preflight, root, "saved-thread", json!({}))
                .await
                .is_err()
        );
        assert_eq!(wrong_preflight.requests.lock().unwrap().len(), 1);

        let wrong_resume = FakeProtocol::new(vec![
            Ok(json!({"thread":thread(root, json!([]))})),
            Ok(json!({"thread":{"id":"other","cwd":root,"status":{"type":"idle"},"turns":[]}})),
        ]);
        assert!(resume_saved(&wrong_resume, root, "saved-thread", json!({}))
            .await
            .is_err());
        assert_eq!(wrong_resume.requests.lock().unwrap().len(), 2);
    }
}
