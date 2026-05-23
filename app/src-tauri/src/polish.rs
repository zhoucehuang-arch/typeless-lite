use reqwest::Client;
use serde_json::json;

use crate::credentials::CredentialsVault;
use crate::openai_compat;
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
    let body = json!({
        "model": prefs.llm_model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });
    openai_compat::post_chat_completion(&Client::new(), &prefs.llm_base_url, &api_key, &body).await
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
