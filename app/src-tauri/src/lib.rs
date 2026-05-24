mod asr;
mod commands;
mod coordinator;
mod credentials;
mod hotkey;
mod insertion;
mod openai_compat;
mod persistence;
mod polish;
mod recorder;
mod types;

use std::sync::Arc;

use coordinator::Coordinator;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    LogicalPosition, LogicalSize, Manager, RunEvent, WindowEvent,
};

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
            coord.warm_up_asr();
            app.manage(coord);
            setup_tray(app)?;
            if let Some(capsule) = app.get_webview_window("capsule") {
                if let Err(err) = position_capsule_bottom_center(&capsule) {
                    log::warn!("[capsule] position failed: {err}");
                }
                let _ = capsule.hide();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_status,
            commands::get_settings,
            commands::set_settings,
            commands::get_credentials,
            commands::set_llm_api_key,
            commands::local_data_status,
            commands::clear_local_data,
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
        .build(tauri::generate_context!())
        .expect("failed to build Typeless Lite")
        .run(|app, event| match event {
            RunEvent::WindowEvent { label, event, .. } => {
                if label == "main" {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                }
            }
            _ => {}
        });
}

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show_i = MenuItemBuilder::with_id("show", "显示 Typeless Lite").build(app)?;
    let quit_i = MenuItemBuilder::with_id("quit", "退出 Typeless Lite").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&show_i, &quit_i]).build()?;
    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("Typeless Lite")
        .icon_as_template(false)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.show_menu_on_left_click(false).build(app)?;
    Ok(())
}

fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub(crate) fn position_capsule_bottom_center<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> tauri::Result<()> {
    let monitor = match window.current_monitor()? {
        Some(monitor) => monitor,
        None => return Ok(()),
    };
    let width = 214.0;
    let height = 84.0;
    window.set_size(LogicalSize::new(width, height))?;

    let scale = monitor.scale_factor();
    let size = monitor.size();
    let logical_width = size.width as f64 / scale;
    let logical_height = size.height as f64 / scale;
    let x = ((logical_width - width) / 2.0).max(0.0);
    let y = (logical_height - 52.0 - 80.0).max(0.0);
    window.set_position(LogicalPosition::new(x, y))?;
    Ok(())
}
