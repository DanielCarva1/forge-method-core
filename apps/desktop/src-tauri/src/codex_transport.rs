//! One stdio connection. Requests are correlated; notifications stay off the UI API.
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    sync::{mpsc, oneshot},
};
use tokio_util::codec::{FramedRead, LinesCodec, LinesCodecError};

pub(crate) const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(90);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
fn timeout_for(method: &str) -> Duration {
    if method == "initialize" {
        INITIALIZE_TIMEOUT
    } else {
        REQUEST_TIMEOUT
    }
}
pub(crate) const REQUEST_REJECTED: &str =
    "O Codex não aceitou a solicitação. Confira a conexão e tente novamente.";
pub(crate) const THREAD_IN_USE: &str =
    "Esta conversa está aberta em outro lugar. Feche-a lá ou escolha outra conversa; nada foi alterado.";
const OVERSIZED_REPLY: &str = "O histórico ou a resposta ultrapassou o limite de leitura deste app. Nada foi apagado. Continue essa conversa pelo Codex CLI; não é necessário repetir o trabalho.";

type Reply = Result<Value, &'static str>;
struct Request {
    method: &'static str,
    params: Value,
    reply: oneshot::Sender<Reply>,
}

pub struct Transport {
    requests: mpsc::Sender<Request>,
    task: tokio::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl Drop for Transport {
    fn drop(&mut self) {
        if let Some(task) = self.task.get_mut().take() {
            task.abort();
        }
    }
}

impl Transport {
    pub async fn shutdown(&self) {
        let mut slot = self.task.lock().await;
        if let Some(task) = slot.take() {
            task.abort();
            let _ = task.await;
        }
    }
    pub fn start(
        executable: &Path,
        root: &Path,
        event: impl Fn(Value) + Send + 'static,
    ) -> Result<Self, &'static str> {
        let mut command = tokio::process::Command::new(executable);
        command
            .args(["app-server", "--listen", "stdio://"])
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command
            .spawn()
            .map_err(|_| "Não foi possível abrir o Codex instalado.")?;
        let mut stdin = child.stdin.take().ok_or("O Codex não abriu a conexão.")?;
        let stdout = child.stdout.take().ok_or("O Codex não abriu a conexão.")?;
        let (requests, mut incoming) = mpsc::channel::<Request>(8);
        let task = tauri::async_runtime::spawn(async move {
            let mut lines =
                FramedRead::new(stdout, LinesCodec::new_with_max_length(MAX_FRAME_BYTES));
            let mut failure = "A conexão com o Codex foi encerrada.";
            let mut pending = HashMap::<u64, (&'static str, oneshot::Sender<Reply>)>::new();
            let mut id = 0u64;
            loop {
                tokio::select! {
                    request = incoming.recv() => {
                        let Some(request) = request else { break };
                        id += 1;
                        let mut bytes = json!({"id":id,"method":request.method,"params":request.params}).to_string().into_bytes();
                        bytes.push(b'\n');
                        pending.insert(id, (request.method, request.reply));
                        if stdin.write_all(&bytes).await.is_err() { break; }
                    }
                    line = lines.next() => {
                        let line = match line {
                            Some(Ok(line)) => line,
                            Some(Err(LinesCodecError::MaxLineLengthExceeded)) => {
                                failure = OVERSIZED_REPLY;
                                break;
                            }
                            _ => break,
                        };
                        let Ok(value) = serde_json::from_str::<Value>(&line) else { break };
                        if value.get("method").is_some() {
                            if let Some(request_id) = value.get("id") {
                                // Unsupported interactive requests fail explicitly; no implicit approvals.
                                let reply = json!({"id":request_id,"error":{"code":-32601,"message":"Interaction not supported by this client"}});
                                if stdin.write_all(format!("{reply}\n").as_bytes()).await.is_err() { break; }
                                event(json!({"method":"forge/unsupportedInteraction"}));
                            } else { event(value); }
                        } else if let Some((method, reply)) = value["id"].as_u64().and_then(|id| pending.remove(&id)) {
                            if let Some(error) = value.get("error") {
                                let _ = reply.send(Err(classify_rejection(method, error)));
                            } else if let Some(result) = value.get("result") {
                                // Protocol initialization requires a notification, not another request.
                                if method == "initialize" && stdin.write_all(b"{\"method\":\"initialized\",\"params\":{}}\n").await.is_err() { break; }
                                let _ = reply.send(Ok(result.clone()));
                            } else { let _ = reply.send(Err("Resposta incompatível do Codex.")); }
                        }
                    }
                }
            }
            for (_, (_, reply)) in pending {
                let _ = reply.send(Err(failure));
            }
            let _ = child.kill().await;
            event(json!({"method":"forge/disconnected"}));
        });
        Ok(Self {
            requests,
            task: tokio::sync::Mutex::new(Some(task)),
        })
    }

    pub async fn request(&self, method: &'static str, params: Value) -> Reply {
        let (reply, response) = oneshot::channel();
        tokio::time::timeout(timeout_for(method), async {
            self.requests
                .send(Request {
                    method,
                    params,
                    reply,
                })
                .await
                .map_err(|_| "A conexão com o Codex foi encerrada.")?;
            response
                .await
                .map_err(|_| "A conexão com o Codex foi encerrada.")?
        })
        .await
        .map_err(|_| "O Codex demorou para responder. Desconecte e tente novamente.")?
    }
}

fn classify_rejection(method: &str, error: &Value) -> &'static str {
    if method == "thread/resume"
        && error["code"].as_i64() == Some(-32600)
        && error["message"].as_str().is_some_and(|message| {
            message.starts_with("thread ") && message.ends_with(" already has an active writer")
        })
    {
        THREAD_IN_USE
    } else {
        REQUEST_REJECTED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_writer_rejection_is_distinct_without_exposing_protocol_detail() {
        let error =
            json!({"code":-32600,"message":"thread private-id already has an active writer"});
        assert_eq!(classify_rejection("thread/resume", &error), THREAD_IN_USE);
        assert!(!THREAD_IN_USE.contains("private-id"));
        assert_eq!(classify_rejection("thread/read", &error), REQUEST_REJECTED);
        assert_eq!(
            classify_rejection("thread/resume", &json!({"code":-32600,"message":"other"})),
            REQUEST_REJECTED
        );
    }

    #[test]
    fn cold_start_has_a_longer_bound_without_extending_turn_requests() {
        assert_eq!(timeout_for("initialize"), Duration::from_secs(90));
        assert_eq!(timeout_for("account/read"), Duration::from_secs(30));
        assert_eq!(timeout_for("turn/start"), Duration::from_secs(30));
    }

    #[tokio::test]
    async fn oversized_history_is_rejected_without_truncation() {
        let bytes = vec![b'x'; MAX_FRAME_BYTES + 1];
        let mut lines = FramedRead::new(
            bytes.as_slice(),
            LinesCodec::new_with_max_length(MAX_FRAME_BYTES),
        );
        assert!(matches!(
            lines.next().await,
            Some(Err(LinesCodecError::MaxLineLengthExceeded))
        ));
        assert!(OVERSIZED_REPLY.contains("Nada foi apagado"));
    }

    #[tokio::test]
    async fn multi_megabyte_history_turn_remains_readable() {
        let bytes = vec![b'x'; 4 * 1024 * 1024];
        let mut lines = FramedRead::new(
            bytes.as_slice(),
            LinesCodec::new_with_max_length(MAX_FRAME_BYTES),
        );
        assert!(matches!(lines.next().await, Some(Ok(_))));
    }

    #[tokio::test]
    async fn multiple_bounded_history_frames_are_read_separately() {
        let line = format!("{{\"data\":\"{}\"}}\n", "x".repeat(600_000));
        assert!(line.len() < MAX_FRAME_BYTES);
        let bytes = format!("{line}{line}");
        let mut lines = FramedRead::new(
            bytes.as_bytes(),
            LinesCodec::new_with_max_length(MAX_FRAME_BYTES),
        );
        assert!(matches!(lines.next().await, Some(Ok(_))));
        assert!(matches!(lines.next().await, Some(Ok(_))));
        assert!(lines.next().await.is_none());
    }
}
