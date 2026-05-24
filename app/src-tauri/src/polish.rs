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
    let user = user_prompt(raw_text);
    let body = json!({
        "model": prefs.llm_model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });
    openai_compat::post_chat_completion(&Client::new(), &prefs.llm_base_url, &api_key, &body)
        .await
        .map(|content| clean_polish_output(&content))
}

fn compose_system_prompt(style: &StyleProfile, output_language: OutputLanguage, hotwords: &[String]) -> String {
    let language = match output_language {
        OutputLanguage::Auto => "输出语言跟随用户原文。",
        OutputLanguage::ZhCn => "中文内容优先输出简体中文。",
        OutputLanguage::En => "最终输出优先使用英文，除非用户明确要求保留原语言。",
    };
    let mut prompt = replace_hotwords(&style.prompt, hotwords);
    prompt.push_str("\n\n# 输出语言\n");
    prompt.push_str(language);
    prompt
}

fn replace_hotwords(prompt: &str, hotwords: &[String]) -> String {
    let base = prompt.trim_end();
    let block = build_hotword_block(hotwords);
    if base.contains(crate::types::HOTWORDS_PLACEHOLDER) {
        return base.replace(crate::types::HOTWORDS_PLACEHOLDER, &block);
    }
    if hotwords.iter().any(|item| !item.trim().is_empty()) {
        format!("{base}\n\n{block}")
    } else {
        base.to_string()
    }
}

fn build_hotword_block(hotwords: &[String]) -> String {
    let cleaned = hotwords
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();

    if cleaned.is_empty() {
        return "# 热词与纠错（系统内置）\n\
你接到的转写来自 ASR，可能含错别字、同音误识别、英文术语音译或形近词。\
按上下文自动纠回正确字面；人名、品牌名、代码、路径、URL、配置 key 和含义会变化的词原样保留。"
            .to_string();
    }

    let bullets = cleaned
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "# 热词与纠错（系统内置）\n\
你接到的转写来自 ASR，可能含错别字。用户希望以下写法在输出中保持准确；\
当转写中出现这些词的同音、近音或形近误识别时，优先按下列写法输出，不做无关词的机械替换：\n\
{bullets}\n\n\
热词纠偏优先于“原样保留”：如果转写词看起来像英文或专有名词，但上下文明显指向热词，请按热词写法输出。"
    )
}

fn user_prompt(raw_text: &str) -> String {
    let escaped = raw_text.replace("</raw_transcript>", "<\\/raw_transcript>");
    format!(
        "下面是本次语音输入的原始转写。请按 system prompt 中当前风格的任务描述整理后输出，\
整理结果会被原样插入到当前光标位置。\n\n\
<raw_transcript>\n{escaped}\n</raw_transcript>\n\n\
只输出整理后的文本正文。"
    )
}

fn clean_polish_output(content: &str) -> String {
    let without_thinking = strip_thinking_blocks(content);
    let trimmed = without_thinking.trim();
    let stripped = strip_markdown_fence(trimmed);
    let mut output = stripped.to_string();

    loop {
        let before_len = output.len();
        output = strip_leading_boilerplate(&output).trim_start().to_string();
        if output.len() == before_len {
            break;
        }
    }

    output.trim().to_string()
}

fn strip_thinking_blocks(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(open_start) = find_case_insensitive(rest, "<think") else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..open_start]);
        let after_open = &rest[open_start..];
        let Some(open_end) = after_open.find('>') else {
            output.push_str(after_open);
            break;
        };
        let after_tag = &after_open[open_end + 1..];
        let Some(close_start) = find_case_insensitive(after_tag, "</think>") else {
            break;
        };
        rest = &after_tag[close_start + "</think>".len()..];
    }
    output
}

fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
}

fn strip_markdown_fence(text: &str) -> &str {
    if !(text.starts_with("```") && text.ends_with("```")) {
        return text;
    }
    let Some(after_first_line) = text.find('\n').map(|index| index + 1) else {
        return text;
    };
    let Some(before_last_fence) = text.rfind("```") else {
        return text;
    };
    if before_last_fence <= after_first_line {
        return text;
    }
    text[after_first_line..before_last_fence].trim_matches(['\n', ' ', '\t', '\r'].as_ref())
}

const LEADING_BOILERPLATE_PREFIXES: &[&str] = &[
    "根据您给的内容",
    "根据您提供的内容",
    "根据你给的内容",
    "根据你提供的内容",
    "以下是整理后的内容",
    "以下是优化后的内容",
    "以下为整理后的内容",
    "以下是结构化整理后的内容",
    "我整理如下",
    "我已整理如下",
    "整理如下",
    "优化如下",
    "结构化整理如下",
];

fn strip_leading_boilerplate(text: &str) -> &str {
    for prefix in LEADING_BOILERPLATE_PREFIXES {
        if let Some(after_prefix) = text.strip_prefix(prefix) {
            for (idx, c) in after_prefix.char_indices() {
                if matches!(c, '。' | '：' | ':' | '，' | ',' | '\n') {
                    let cut = prefix.len() + idx + c.len_utf8();
                    return &text[cut..];
                }
            }
            return after_prefix;
        }
    }
    text
}
