use std::sync::Arc;

use tauri::State;

use crate::asr::sherpa;
use crate::coordinator::Coordinator;
use crate::credentials::CredentialsVault;
use crate::recorder;
use crate::types::{
    AppStatus, CorrectionRule, CredentialsStatus, DictationSession, DictionaryEntry,
    MicrophoneDevice, Preferences, SherpaModelInfo, StyleProfile,
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
