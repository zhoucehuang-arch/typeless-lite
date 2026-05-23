use std::sync::Arc;

use reqwest::Client;
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};

use crate::asr::sherpa;
use crate::coordinator::Coordinator;
use crate::credentials::CredentialsVault;
use crate::hotkey;
use crate::openai_compat;
use crate::persistence;
use crate::recorder;
use crate::types::{
    AppStatus, ClearLocalDataOptions, CorrectionRule, CredentialsStatus, DictationSession,
    DictionaryEntry, LocalDataStatus, MicrophoneDevice, Preferences, SherpaDefaultModelStatus,
    SherpaModelInfo, StyleProfile,
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
pub fn local_data_status() -> Result<LocalDataStatus, String> {
    persistence::local_data_status(CredentialsVault::llm_configured())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn clear_local_data(
    app: AppHandle,
    coord: Coord<'_>,
    options: ClearLocalDataOptions,
) -> Result<LocalDataStatus, String> {
    if options.settings {
        Arc::clone(coord.inner()).reset_preferences()?;
    }
    if options.history {
        persistence::reset_history_file().map_err(|err| err.to_string())?;
        emit_changed(&app, "history:changed");
    }
    if options.dictionary {
        persistence::reset_dictionary_files().map_err(|err| err.to_string())?;
        emit_changed(&app, "dictionary:changed");
    }
    if options.styles {
        persistence::reset_styles_file().map_err(|err| err.to_string())?;
    }
    if options.api_key {
        CredentialsVault::set_llm_api_key("").map_err(|err| err.to_string())?;
    }
    persistence::local_data_status(CredentialsVault::llm_configured())
        .map_err(|err| err.to_string())
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
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": "You validate whether a chat model is available. Reply with ok." },
            { "role": "user", "content": "ok" }
        ],
        "max_tokens": 2
    });
    match openai_compat::post_chat_completion(&Client::new(), &base_url, &key, &body).await {
        Ok(_) => Ok(LlmValidationResult {
            ok: true,
            message: "模型可用".into(),
        }),
        Err(err) => Ok(LlmValidationResult {
            ok: false,
            message: err,
        }),
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
pub fn delete_history_entry(app: AppHandle, coord: Coord<'_>, id: String) -> Result<(), String> {
    coord.history().delete(&id).map_err(|err| err.to_string())?;
    emit_changed(&app, "history:changed");
    Ok(())
}

#[tauri::command]
pub fn clear_history(app: AppHandle, coord: Coord<'_>) -> Result<(), String> {
    coord.history().clear().map_err(|err| err.to_string())?;
    emit_changed(&app, "history:changed");
    Ok(())
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
    openai_compat::fetch_models(&Client::new(), base_url, api_key).await
}

fn emit_changed(app: &AppHandle, event: &str) {
    let _ = app.emit(event, ());
}
