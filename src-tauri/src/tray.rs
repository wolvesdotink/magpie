use std::sync::Arc;

use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    App, AppHandle, Emitter, Manager,
};

use crate::models::{registry, storage};
use crate::state::AppState;
use crate::transcription::backend::TranscriptionBackend;
use crate::transcription::whisper_backend::WhisperBackend;

/// Set up the system tray icon and menu
pub fn setup_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle();
    let menu = build_tray_menu(app_handle)?;

    TrayIconBuilder::with_id("main-tray")
        .icon(
            Image::from_bytes(include_bytes!("../icons/tray-idle.png"))
                .expect("Failed to load tray icon"),
        )
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            match id {
                "quit" => {
                    app.exit(0);
                }
                "settings" => {
                    if let Some(window) = app.get_webview_window("settings") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "check_for_updates" => {
                    // Show settings window (where the updater UI lives) and
                    // emit the event the Vue composable listens for.
                    if let Some(window) = app.get_webview_window("settings") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit("menu://check-for-updates", ());
                    }
                }
                _ if id.starts_with("model:") => {
                    let model_id = &id["model:".len()..];
                    handle_model_selection(app, model_id);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

/// Build the complete tray menu, reflecting current model and recording state.
fn build_tray_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let state = app.state::<Arc<AppState>>();

    let status_text = if state.is_recording() {
        "Magpie \u{2014} Recording..."
    } else if state.is_processing() {
        "Magpie \u{2014} Transcribing..."
    } else {
        "Magpie \u{2014} Ready"
    };

    let status = MenuItem::with_id(app, "status", status_text, false, None::<&str>)?;
    let separator1 = PredefinedMenuItem::separator(app)?;

    let model_submenu = build_model_submenu(app, &state)?;

    let separator2 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let check_updates = MenuItem::with_id(
        app,
        "check_for_updates",
        "Check for Updates\u{2026}",
        true,
        None::<&str>,
    )?;
    let separator3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Magpie", true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::with_items(
        app,
        &[
            &status,
            &separator1,
            &model_submenu,
            &separator2,
            &settings,
            &check_updates,
            &separator3,
            &quit,
        ],
    )?;

    Ok(menu)
}

/// Build the "Model" submenu with a CheckMenuItem per available model.
fn build_model_submenu(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<Submenu<tauri::Wry>, Box<dyn std::error::Error>> {
    let all_models = registry::get_available_models();
    let downloaded_filenames = storage::list_downloaded_models().unwrap_or_default();
    let selected_model_id = match state.settings.lock() {
        Ok(settings) => settings.selected_model.clone(),
        Err(e) => {
            log::error!("Settings mutex poisoned in tray: {}", e);
            None
        }
    };

    let submenu = Submenu::with_id(app, "model-submenu", "Model", true)?;

    for model in &all_models {
        let is_downloaded = downloaded_filenames.contains(&model.filename);
        let is_selected = selected_model_id.as_deref() == Some(model.id.as_str());

        let item_id = format!("model:{}", model.id);

        let display_text = if is_downloaded {
            model.display_name.clone()
        } else {
            format!("{} (not downloaded)", model.display_name)
        };

        let item = CheckMenuItem::with_id(
            app,
            item_id,
            &display_text,
            is_downloaded, // enabled only if downloaded
            is_selected,   // checked if this is the active model
            None::<&str>,  // no accelerator
        )?;

        submenu.append(&item)?;
    }

    Ok(submenu)
}

/// Handle a model selection from the tray submenu.
fn handle_model_selection(app: &AppHandle, model_id: &str) {
    let state = app.state::<Arc<AppState>>();

    // Don't switch models while recording or processing
    if state.is_recording() || state.is_processing() {
        log::warn!("Cannot switch models while recording or processing");
        return;
    }

    let model_info = match registry::find_model(model_id) {
        Some(m) => m,
        None => {
            log::error!("Unknown model selected from tray: {}", model_id);
            return;
        }
    };

    let path = match storage::model_path(&model_info.filename) {
        Ok(p) if p.exists() => p,
        _ => {
            log::error!("Model {} is not downloaded", model_id);
            return;
        }
    };

    match WhisperBackend::load(&path) {
        Ok(backend) => {
            {
                let mut slot = state.backend.lock().unwrap();
                *slot = Some(Arc::new(backend) as Arc<dyn TranscriptionBackend>);
            }
            {
                let mut model_path = state.current_model_path.lock().unwrap();
                *model_path = Some(path);
            }
            {
                let mut settings = state.settings.lock().unwrap();
                settings.selected_model = Some(model_id.to_string());
                if let Err(e) = settings.save() {
                    log::error!("Failed to save settings: {}", e);
                }
            }

            log::info!("Switched to model: {}", model_id);
            rebuild_tray_menu(app);
        }
        Err(e) => {
            log::error!("Failed to load model {}: {}", model_id, e);
        }
    }
}

/// Rebuild and replace the tray menu to reflect current state.
pub fn rebuild_tray_menu(app: &AppHandle) {
    match build_tray_menu(app) {
        Ok(menu) => {
            if let Some(tray) = app.tray_by_id("main-tray") {
                if let Err(e) = tray.set_menu(Some(menu)) {
                    log::error!("Failed to set tray menu: {}", e);
                }
            }
        }
        Err(e) => {
            log::error!("Failed to build tray menu: {}", e);
        }
    }
}

/// Update the tray icon to reflect current state
pub fn set_tray_icon(app: &AppHandle, state: TrayState) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let icon_bytes: &[u8] = match state {
            TrayState::Idle => include_bytes!("../icons/tray-idle.png"),
            TrayState::Recording => include_bytes!("../icons/tray-recording.png"),
            TrayState::Processing => include_bytes!("../icons/tray-processing.png"),
        };

        if let Ok(icon) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(icon));
            let _ = tray.set_icon_as_template(true);
        }
    }
}

/// Update the tray menu status text by rebuilding the menu.
pub fn set_tray_status(app: &AppHandle, _status: &str) {
    rebuild_tray_menu(app);
}

/// Tray icon states
pub enum TrayState {
    Idle,
    Recording,
    Processing,
}
