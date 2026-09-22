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
use tauri::{ipc::Channel, State};

// One turn per frame avoids an arbitrary batch becoming oversized. A single giant turn still
// fails through the transport's existing per-frame boundary rather than being truncated.
const HISTORY_PAGE_LIMIT: usize = 1;
const MAX_HISTORY_PAGES: usize = 4096;
const MAX_HISTORY_BYTES: usize = 32 * 1024 * 1024;
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
    thread_id: Option<String>,
    events: Channel<AgentEvent>,
    state: State<'_, AgentState>,
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
    let result = initialize(&session, &project.project_root, thread_id.as_deref()).await;
    if result.is_err() {
        release(&state, &session).await;
    }
    result
}

async fn initialize(
    session: &Session,
    project_root: &str,
    thread_id: Option<&str>,
) -> Result<history::Conversation, &'static str> {
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

    #[tokio::test]
    async fn lean_resume_pages_more_than_one_frame_without_replaying_a_turn() {
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
        const { assert!(1_200_000 > MAX_FRAME_BYTES) };
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
