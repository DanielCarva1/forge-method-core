//! Read projection only: Codex remains the owner of persisted conversations.
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

#[derive(Serialize)]
pub struct Conversation {
    pub thread_id: String,
    pub messages: Vec<Message>,
    pub resumed: bool,
}

#[derive(Debug, Serialize)]
pub struct Message {
    pub id: String,
    pub role: &'static str,
    pub text: String,
    pub incomplete: bool,
}

pub fn validate(
    thread: &Value,
    root: &str,
    expected: Option<&str>,
) -> Result<String, &'static str> {
    let id = thread["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or("O Codex não informou a conversa.")?;
    if expected.is_some_and(|expected| expected != id) {
        return Err("O Codex retornou outra conversa. Nada foi retomado.");
    }
    let returned = thread["cwd"]
        .as_str()
        .and_then(|p| Path::new(p).canonicalize().ok());
    let requested = Path::new(root).canonicalize().ok();
    if requested.is_none() || returned != requested {
        return Err("Esta conversa pertence a outra pasta ou a pasta não está disponível.");
    }
    Ok(id.to_owned())
}

pub fn messages(thread: &Value) -> Result<Vec<Message>, &'static str> {
    let turns = thread["turns"]
        .as_array()
        .ok_or("O Codex não retornou o histórico. Tente conectar novamente.")?;
    messages_from_turns(turns)
}

pub fn messages_from_turns(turns: &[Value]) -> Result<Vec<Message>, &'static str> {
    let mut messages = Vec::new();
    for turn in turns {
        match turn.get("itemsView") {
            None => {}
            Some(Value::String(view)) if view == "full" => {}
            _ => return Err("O histórico recebido está incompleto."),
        }
        for item in turn["items"]
            .as_array()
            .ok_or("O histórico recebido está incompleto.")?
        {
            let text = match item["type"].as_str() {
                Some("agentMessage") => item["text"].as_str().map(str::to_owned),
                Some("userMessage") => item["content"].as_array().map(|content| {
                    content
                        .iter()
                        .map(|part| part["text"].as_str().unwrap_or("[Conteúdo não textual]"))
                        .collect::<Vec<_>>()
                        .join("\n")
                }),
                _ => continue, // Tool traces are not conversation bubbles.
            }
            .ok_or("O histórico recebido está incompleto.")?;
            messages.push(Message {
                incomplete: item["type"] == "agentMessage" && turn["status"] != "completed",
                id: item["id"]
                    .as_str()
                    .ok_or("O histórico recebido está incompleto.")?
                    .into(),
                role: if item["type"] == "userMessage" {
                    "user"
                } else {
                    "agent"
                },
                text,
            });
        }
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn history_preserves_partial_text_without_replaying_tools() {
        let result = messages(&json!({"turns":[{"status":"interrupted","items":[
            {"type":"userMessage","id":"u","content":[{"type":"text","text":"Keep this decision"}]},
            {"type":"commandExecution","id":"tool","command":"never replay"},
            {"type":"agentMessage","id":"a","text":"Partial reply"}
        ]}]}))
        .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[1].text, "Partial reply");
        assert!(result[1].incomplete);
        assert!(messages(&json!({})).is_err());
    }

    #[test]
    fn rejects_wrong_thread_or_directory_before_resume() {
        let root = std::env::current_dir().unwrap();
        let root = root.to_str().unwrap();
        let thread = json!({"id":"one","cwd":root});
        assert!(validate(&thread, root, Some("one")).is_ok());
        assert!(validate(&thread, root, Some("two")).is_err());
        assert!(validate(&thread, "missing-directory", Some("one")).is_err());
    }
}
