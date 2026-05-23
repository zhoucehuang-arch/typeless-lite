use reqwest::Client;
use serde_json::Value;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub fn normalized_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        DEFAULT_BASE_URL.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn chat_url_candidates(base_url: &str) -> Vec<String> {
    endpoint_candidates(base_url, "chat/completions")
}

pub fn models_url_candidates(base_url: &str) -> Vec<String> {
    endpoint_candidates(base_url, "models")
}

fn endpoint_candidates(base_url: &str, endpoint: &str) -> Vec<String> {
    let base = normalized_base_url(base_url);
    let mut candidates = Vec::new();

    match endpoint {
        "models" if base.ends_with("/models") => push_unique(&mut candidates, base.clone()),
        "models" if base.ends_with("/chat/completions") => {
            if let Some(prefix) = base.strip_suffix("/chat/completions") {
                push_unique(&mut candidates, format!("{prefix}/models"));
            }
        }
        "chat/completions" if base.ends_with("/chat/completions") => {
            push_unique(&mut candidates, base.clone());
        }
        "chat/completions" if base.ends_with("/models") => {
            if let Some(prefix) = base.strip_suffix("/models") {
                push_unique(&mut candidates, format!("{prefix}/chat/completions"));
            }
        }
        _ => {}
    }

    if candidates.is_empty() {
        push_unique(&mut candidates, format!("{base}/{endpoint}"));
    }

    if !base.ends_with("/v1")
        && !base.ends_with("/models")
        && !base.ends_with("/chat/completions")
    {
        push_unique(&mut candidates, format!("{base}/v1/{endpoint}"));
    }

    candidates
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|item| item == &value) {
        values.push(value);
    }
}

pub async fn fetch_models(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>, String> {
    let candidates = models_url_candidates(base_url);
    let mut last_error = String::new();

    for url in candidates {
        let mut request = client.get(&url);
        if let Some(key) = api_key.filter(|value| !value.trim().is_empty()) {
            request = request.bearer_auth(key);
        }
        let response = match request.send().await {
            Ok(value) => value,
            Err(err) => {
                last_error = format!("{url}: {err}");
                continue;
            }
        };
        let status = response.status();
        let body = response.text().await.map_err(|err| err.to_string())?;
        if !status.is_success() {
            last_error = format!("{url}: HTTP {}: {}", status.as_u16(), preview(&body));
            continue;
        }
        match parse_model_ids(&body) {
            Ok(models) => return Ok(models),
            Err(err) => {
                last_error = format!("{url}: {err}");
            }
        }
    }

    Err(if last_error.is_empty() {
        "无法获取模型列表".into()
    } else {
        last_error
    })
}

pub async fn post_chat_completion(
    client: &Client,
    base_url: &str,
    api_key: &str,
    body: &Value,
) -> Result<String, String> {
    let candidates = chat_url_candidates(base_url);
    let mut last_error = String::new();

    for url in candidates {
        let response = match client.post(&url).bearer_auth(api_key).json(body).send().await {
            Ok(value) => value,
            Err(err) => {
                last_error = format!("{url}: {err}");
                continue;
            }
        };
        let status = response.status();
        let text = response.text().await.map_err(|err| err.to_string())?;
        if !status.is_success() {
            last_error = format!("{url}: HTTP {}: {}", status.as_u16(), preview(&text));
            continue;
        }
        match extract_content(&text) {
            Ok(content) => return Ok(content),
            Err(err) => {
                last_error = format!("{url}: {err}");
            }
        }
    }

    Err(if last_error.is_empty() {
        "LLM 请求失败".into()
    } else {
        last_error
    })
}

pub fn parse_model_ids(body: &str) -> Result<Vec<String>, String> {
    let json: Value =
        serde_json::from_str(body).map_err(|err| format!("模型列表不是有效 JSON: {err}"))?;
    let data = json
        .get("data")
        .or_else(|| json.get("models"))
        .unwrap_or(&json);
    let array = data
        .as_array()
        .ok_or_else(|| "模型列表缺少 data 数组".to_string())?;
    let mut models = array
        .iter()
        .filter_map(model_id_from_value)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    Ok(models)
}

fn model_id_from_value(value: &Value) -> Option<&str> {
    value
        .get("id")
        .or_else(|| value.get("name"))
        .and_then(|id| id.as_str())
        .or_else(|| value.as_str())
}

pub fn extract_content(body: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(body).map_err(|err| err.to_string())?;
    let choice = value
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first());

    let content = choice
        .and_then(|choice| choice.pointer("/message/content"))
        .and_then(content_value_to_string)
        .or_else(|| {
            choice
                .and_then(|choice| choice.get("text"))
                .and_then(|text| text.as_str())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            value.get("output_text")
                .and_then(|text| text.as_str())
                .map(ToOwned::to_owned)
        });

    content
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("无法解析 LLM 返回: {}", preview(body)))
}

fn content_value_to_string(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    let parts = value.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| {
            part.get("text")
                .and_then(|text| text.as_str())
                .or_else(|| part.get("content").and_then(|text| text.as_str()))
        })
        .collect::<Vec<_>>()
        .join("");
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub fn preview(value: &str) -> String {
    value.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::{chat_url_candidates, extract_content, models_url_candidates, parse_model_ids};

    #[test]
    fn candidates_try_plain_and_v1_for_host_root() {
        assert_eq!(
            models_url_candidates("https://ai.input.im"),
            vec![
                "https://ai.input.im/models".to_string(),
                "https://ai.input.im/v1/models".to_string()
            ]
        );
        assert_eq!(
            chat_url_candidates("https://ai.input.im"),
            vec![
                "https://ai.input.im/chat/completions".to_string(),
                "https://ai.input.im/v1/chat/completions".to_string()
            ]
        );
    }

    #[test]
    fn candidates_keep_existing_v1_base() {
        assert_eq!(
            models_url_candidates("https://api.openai.com/v1"),
            vec!["https://api.openai.com/v1/models".to_string()]
        );
        assert_eq!(
            chat_url_candidates("https://api.openai.com/v1"),
            vec!["https://api.openai.com/v1/chat/completions".to_string()]
        );
    }

    #[test]
    fn parses_model_shapes() {
        let ids = parse_model_ids(r#"{ "data": [{ "id": "b" }, { "id": "a" }, { "id": "b" }] }"#)
            .unwrap();
        assert_eq!(ids, vec!["a", "b"]);
        let ids = parse_model_ids(r#"{ "models": [{ "name": "gpt-x" }] }"#).unwrap();
        assert_eq!(ids, vec!["gpt-x"]);
    }

    #[test]
    fn extracts_chat_content_variants() {
        let body = r#"{ "choices": [{ "message": { "content": " ok " } }] }"#;
        assert_eq!(extract_content(body).unwrap(), "ok");
        let body = r#"{ "choices": [{ "message": { "content": [{ "text": "o" }, { "text": "k" }] } }] }"#;
        assert_eq!(extract_content(body).unwrap(), "ok");
    }
}
