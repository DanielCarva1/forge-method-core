//! The supported Codex question request, not an execution-approval interface.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Questions {
    pub thread_id: String,
    pub turn_id: String,
    pub questions: Vec<Question>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub is_secret: bool,
    pub options: Option<Vec<OptionChoice>>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct OptionChoice {
    pub label: String,
    pub description: String,
}

pub type Answers = HashMap<String, String>;
pub const STALE: &str = "Esta pergunta já foi encerrada. Confira a conversa antes de continuar.";

fn bounded(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.len() <= max && !text.contains('\0')
}

pub fn parse(params: &Value) -> Option<Questions> {
    if params.to_string().len() > 32000 {
        return None;
    }
    let request: Questions = serde_json::from_value(params.clone()).ok()?;
    if !bounded(&request.thread_id, 200)
        || !bounded(&request.turn_id, 200)
        || request.questions.is_empty()
        || request.questions.len() > 3
    {
        return None;
    }
    let mut ids = HashSet::new();
    for question in &request.questions {
        if question.is_secret
            || !bounded(&question.id, 200)
            || !ids.insert(&question.id)
            || !bounded(&question.question, 8000)
            || question.header.len() > 200
        {
            return None;
        }
        if let Some(options) = &question.options {
            if options.len() > 12
                || options
                    .iter()
                    .any(|option| !bounded(&option.label, 500) || option.description.len() > 4000)
            {
                return None;
            }
        }
    }
    Some(request)
}

pub fn response(request: &Questions, answers: Answers) -> Result<Value, &'static str> {
    if answers.len() != request.questions.len()
        || answers.values().map(String::len).sum::<usize>() > 24000
    {
        return Err("Responda às perguntas antes de enviar.");
    }
    let mut result = serde_json::Map::new();
    for question in &request.questions {
        let answer = answers
            .get(&question.id)
            .filter(|answer| bounded(answer, 8000))
            .ok_or("Responda às perguntas antes de enviar.")?;
        result.insert(question.id.clone(), serde_json::json!({"answers":[answer]}));
    }
    Ok(serde_json::json!({"answers":result}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> Value {
        json!({"threadId":"thread","turnId":"turn","questions":[{"id":"style","header":"Estilo","question":"Qual estilo?","options":[{"label":"Claro","description":"Leve"}]}]})
    }
    #[test]
    fn questions_are_bounded_and_never_collect_secrets() {
        assert!(parse(&fixture()).is_some());
        let mut bad = fixture();
        bad["questions"][0]["isSecret"] = json!(true);
        assert!(parse(&bad).is_none());
        let mut bad = fixture();
        bad["questions"][0]["question"] = json!("x".repeat(8001));
        assert!(parse(&bad).is_none());
        let mut bad = fixture();
        let duplicate = bad["questions"][0].clone();
        bad["questions"].as_array_mut().unwrap().push(duplicate);
        assert!(parse(&bad).is_none());
    }
    #[test]
    fn only_complete_explicit_answers_are_protocol_responses() {
        let request = parse(&fixture()).unwrap();
        assert!(response(&request, HashMap::new()).is_err());
        assert!(response(&request, HashMap::from([("wrong".into(), "Claro".into())])).is_err());
        assert_eq!(
            response(
                &request,
                HashMap::from([("style".into(), "Meu próprio estilo".into())])
            )
            .unwrap(),
            json!({"answers":{"style":{"answers":["Meu próprio estilo"]}}})
        );
    }
}
