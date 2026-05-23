mod asr;
mod commands;
mod coordinator;
mod credentials;
mod hotkey;
mod insertion;
mod persistence;
mod polish;
mod recorder;
mod types;

use std::sync::Arc;

use coordinator::Coordinator;
use tauri::Manager;

pub fn run() {
    env_logger::init();
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let coord = Arc::new(Coordinator::new(app.handle().clone())?);
            if let Err(err) = coord.install_hotkey() {
                log::warn!("[hotkey] install failed: {err}");
            }
            app.manage(coord);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_status,
            commands::get_settings,
            commands::set_settings,
            commands::get_credentials,
            commands::set_llm_api_key,
            commands::validate_hotkey,
            commands::set_shortcut_recording_active,
            commands::list_llm_models,
            commands::validate_llm_model,
            commands::start_dictation,
            commands::stop_dictation,
            commands::cancel_dictation,
            commands::list_microphones,
            commands::sherpa_catalog,
            commands::sherpa_model_dir,
            commands::sherpa_default_model_status,
            commands::sherpa_prepare_default_model,
            commands::list_history,
            commands::delete_history_entry,
            commands::clear_history,
            commands::list_dictionary,
            commands::add_dictionary_entry,
            commands::remove_dictionary_entry,
            commands::set_dictionary_entry_enabled,
            commands::list_correction_rules,
            commands::add_correction_rule,
            commands::remove_correction_rule,
            commands::set_correction_rule_enabled,
            commands::list_styles,
            commands::save_style,
            commands::reset_builtin_style,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Typeless Lite");
}
