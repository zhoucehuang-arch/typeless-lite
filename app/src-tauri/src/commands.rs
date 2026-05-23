use std::sync::Arc;

use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::asr::sherpa;
use crate::coordinator::Coordinator;
use crate::credentials::CredentialsVault;
use crate::hotkey;
use crate::recorder;
use crate::types::{
    AppStatus, CorrectionRule, CredentialsStatus, DictationSession, DictionaryEntry,
    MicrophoneDevice, Preferences, SherpaDefaultModelStatus, SherpaModelInfo, StyleProfile,
};

type Coord<'a> = State<'a, Arc<Coordinator>>;

#[tauri::command]
pub fn app_status() -> AppStatus {
    AppStatus {
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
    }
}

#[tauri::command]
pub fn get_settings(coord: Coord<'_>) -> Preferences {
    coord.prefs().get()
}

#[tauri::command]
pub fn set_settings(coord: Coord<'_>, prefs: Preferences) -> Result<(), String> {
    let previous = coord.prefs().get();
    let hotkey_changed =
        previous.hotkey != prefs.hotkey || previous.hotkey_mode != prefs.hotkey_mode;
    hotkey::validate_hotkey_binding(&prefs.hotkey)?;
    coord.prefs().set(prefs).map_err(|err| err.to_string())?;
    if hotkey_changed {
        Arc::clone(coord.inner()).refresh_hotkey()?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_credentials() -> CredentialsStatus {
    CredentialsStatus {
        llm_configured: CredentialsVault::llm_configured(),
    }
}

#[tauri::command]
pub fn set_llm_api_key(api_key: String) -> Result<(), String> {
    CredentialsVault::set_llm_api_key(&api_key).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn validate_hotkey(binding: String) -> Result<(), String> {
    hotkey::validate_hotkey_binding(&binding)
}

#[tauri::command]
pub fn set_shortcut_recording_active(coord: Coord<'_>, active: bool) -> Result<(), String> {
    Arc::clone(coord.inner()).set_shortcut_recording_active(active)
}

#[tauri::command]
pub async fn list_llm_models(
    base_url: String,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    let key = effective_api_key(api_key);
    fetch_models(&base_url, key.as_deref()).await
}

#[tauri::command]
pub async fn validate_llm_model(
    base_url: String,
    model: String,
    api_key: Option<String>,
) -> Result<LlmValidationResult, String> {
    let model = model.trim().to_string();
    if model.is_empty() {
        return Ok(LlmValidationResult {
            ok: false,
            message: "模型名称为空".into(),
        });
    }
    let key = effective_api_key(api_key).ok_or_else(|| "缺少 LLM API Key".to_string())?;
    let url = chat_completions_url(&base_url);
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": "You validate whether a chat model is available. Reply with ok." },
            { "role": "user", "content": "ok" }
        ],
        "max_tokens": 2
    });
    let response = Client::new()
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|err| err.to_string())?;
    if status.is_success() {
        Ok(LlmValidationResult {
            ok: true,
            message: "模型可用".into(),
        })
    } else {
        Ok(LlmValidationResult {
            ok: false,
            message: format!("HTTP {}: {}", status.as_u16(), preview(&text)),
        })
    }
}

#[tauri::command]
pub async fn start_dictation(coord: Coord<'_>) -> Result<(), String> {
    Arc::clone(coord.inner()).start_dictation().await
}

#[tauri::command]
pub async fn stop_dictation(coord: Coord<'_>) -> Result<(), String> {
    Arc::clone(coord.inner()).stop_dictation().await
}

#[tauri::command]
pub fn cancel_dictation(coord: Coord<'_>) {
    coord.cancel_dictation();
}

#[tauri::command]
pub fn list_microphones() -> Result<Vec<MicrophoneDevice>, String> {
    recorder::list_input_devices().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn sherpa_catalog() -> Vec<SherpaModelInfo> {
    sherpa::catalog()
}

#[tauri::command]
pub fn sherpa_model_dir(alias: String) -> Result<String, String> {
    sherpa::model_dir(&alias)
        .map(|path| path.display().to_string())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn sherpa_default_model_status() -> Result<SherpaDefaultModelStatus, String> {
    sherpa::default_model_status().map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn sherpa_prepare_default_model(
    app: AppHandle,
) -> Result<SherpaDefaultModelStatus, String> {
    sherpa::prepare_default_model(app)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_history(coord: Coord<'_>) -> Result<Vec<DictationSession>, String> {
    coord.history().list().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_history_entry(coord: Coord<'_>, id: String) -> Result<(), String> {
    coord.history().delete(&id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn clear_history(coord: Coord<'_>) -> Result<(), String> {
    coord.history().clear().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_dictionary(coord: Coord<'_>) -> Result<Vec<DictionaryEntry>, String> {
    coord.dictionary().list().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn add_dictionary_entry(
    coord: Coord<'_>,
    phrase: String,
    note: Option<String>,
) -> Result<DictionaryEntry, String> {
    coord
        .dictionary()
        .add(phrase, note)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn remove_dictionary_entry(coord: Coord<'_>, id: String) -> Result<(), String> {
    coord.dictionary().remove(&id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_dictionary_entry_enabled(
    coord: Coord<'_>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    coord
        .dictionary()
        .set_enabled(&id, enabled)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_correction_rules(coord: Coord<'_>) -> Result<Vec<CorrectionRule>, String> {
    coord.corrections().list().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn add_correction_rule(
    coord: Coord<'_>,
    pattern: String,
    replacement: String,
) -> Result<CorrectionRule, String> {
    coord
        .corrections()
        .add(pattern, replacement)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn remove_correction_rule(coord: Coord<'_>, id: String) -> Result<(), String> {
    coord.corrections().remove(&id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_correction_rule_enabled(
    coord: Coord<'_>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    coord
        .corrections()
        .set_enabled(&id, enabled)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_styles(coord: Coord<'_>) -> Result<Vec<StyleProfile>, String> {
    coord.styles().list().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_style(coord: Coord<'_>, style: StyleProfile) -> Result<StyleProfile, String> {
    coord.styles().save(style).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn reset_builtin_style(coord: Coord<'_>, id: String) -> Result<StyleProfile, String> {
    coord
        .styles()
        .reset_builtin(&id)
        .map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmValidationResult {
    pub ok: bool,
    pub message: String,
}

fn effective_api_key(api_key: Option<String>) -> Option<String> {
    api_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(CredentialsVault::get_llm_api_key)
}

async fn fetch_models(base_url: &str, api_key: Option<&str>) -> Result<Vec<String>, String> {
    let url = models_url(base_url);
    let mut request = Client::new().get(url);
    if let Some(key) = api_key.filter(|value| !value.trim().is_empty()) {
        request = request.bearer_auth(key);
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status.as_u16(), preview(&body)));
    }
    parse_model_ids(&body)
}

fn chat_completions_url(base_url: &str) -> String {
    let base = normalized_base_url(base_url);
    if base.ends_with("/chat/completions") {
        base
    } else {
        format!("{base}/chat/completions")
    }
}

fn models_url(base_url: &str) -> String {
    let trimmed = normalized_base_url(base_url);
    if trimmed.ends_with("/models") {
        trimmed
    } else if let Some(prefix) = trimmed.strip_suffix("/chat/completions") {
        format!("{prefix}/models")
    } else {
        format!("{trimmed}/models")
    }
}

fn normalized_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "https://api.openai.com/v1".to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_model_ids(body: &str) -> Result<Vec<String>, String> {
    let json: Value =
        serde_json::from_str(body).map_err(|err| format!("模型列表不是有效 JSON: {err}"))?;
    let data = json
        .get("data")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "模型列表缺少 data 数组".to_string())?;
    let mut models = data
        .iter()
        .filter_map(|item| item.get("id").and_then(|id| id.as_str()))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    Ok(models)
}

fn preview(value: &str) -> String {
    value.chars().take(200).collect()
}
