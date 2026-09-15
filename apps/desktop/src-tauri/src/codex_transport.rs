//! One stdio connection. Requests are correlated; notifications stay off the UI API.
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    sync::{mpsc, oneshot},
};
use tokio_util::codec::{FramedRead, LinesCodec};

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
            let mut lines = FramedRead::new(stdout, LinesCodec::new_with_max_length(1024 * 1024));
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
                        let Some(Ok(line)) = line else { break };
                        let Ok(value) = serde_json::from_str::<Value>(&line) else { break };
                        if value.get("method").is_some() {
                            if let Some(request_id) = value.get("id") {
                                // Unsupported interactive requests fail explicitly; no implicit approvals.
                                let reply = json!({"id":request_id,"error":{"code":-32601,"message":"Interaction not supported by this client"}});
                                if stdin.write_all(format!("{reply}\n").as_bytes()).await.is_err() { break; }
                                event(json!({"method":"forge/unsupportedInteraction"}));
                            } else { event(value); }
                        } else if let Some((method, reply)) = value["id"].as_u64().and_then(|id| pending.remove(&id)) {
                            if value.get("error").is_some() {
                                let _ = reply.send(Err("O Codex não aceitou a solicitação. Confira a conexão e tente novamente."));
                            } else if let Some(result) = value.get("result") {
                                // Protocol initialization requires a notification, not another request.
                                if method == "initialize" {
                                    if stdin.write_all(b"{\"method\":\"initialized\",\"params\":{}}\n").await.is_err() { break; }
                                }
                                let _ = reply.send(Ok(result.clone()));
                            } else { let _ = reply.send(Err("Resposta incompatível do Codex.")); }
                        }
                    }
                }
            }
            for (_, (_, reply)) in pending {
                let _ = reply.send(Err("A conexão com o Codex foi encerrada."));
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
        tokio::time::timeout(Duration::from_secs(30), async {
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
