use reqwest::Client;
use serde_json::{json, Value};

use crate::credentials::CredentialsVault;
use crate::types::{OutputLanguage, PolishMode, Preferences, StyleProfile};

pub async fn polish_text(
    raw_text: &str,
    style: &StyleProfile,
    prefs: &Preferences,
    hotwords: &[String],
) -> Result<String, String> {
    if style.mode == PolishMode::Raw {
        return Ok(raw_text.trim().to_string());
    }
    let api_key = CredentialsVault::get_llm_api_key().ok_or_else(|| "缺少 LLM API Key".to_string())?;
    let system = compose_system_prompt(style, prefs.output_language, hotwords);
    let user = format!("请整理下面这段语音转写：\n\n{raw_text}");
    let url = chat_completions_url(&prefs.llm_base_url);
    let body = json!({
        "model": prefs.llm_model,
        "temperature": prefs.llm_temperature,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });
    let response = Client::new()
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("LLM HTTP {}: {}", status.as_u16(), preview(&text)));
    }
    extract_content(&text)
}

fn compose_system_prompt(style: &StyleProfile, output_language: OutputLanguage, hotwords: &[String]) -> String {
    let language = match output_language {
        OutputLanguage::Auto => "输出语言跟随用户原文。",
        OutputLanguage::ZhCn => "中文内容优先输出简体中文。",
        OutputLanguage::En => "最终输出优先使用英文，除非用户明确要求保留原语言。",
    };
    let hotword_block = if hotwords.is_empty() {
        String::new()
    } else {
        format!("\n\n用户词典：\n{}", hotwords.join("、"))
    };
    format!("{}\n\n{}{}", style.prompt, language, hotword_block)
}

fn chat_completions_url(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

fn extract_content(body: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(body).map_err(|err| err.to_string())?;
    value["choices"][0]["message"]["content"]
        .as_str()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("无法解析 LLM 返回: {}", preview(body)))
}

fn preview(value: &str) -> String {
    value.chars().take(200).collect()
}
