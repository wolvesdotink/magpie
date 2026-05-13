use std::sync::Arc;

use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    App, AppHandle, Emitter, Manager,
};

use crate::models::{registry, storage};
use crate::state::{lock_or_recover, AppState};
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
                "history" => {
                    if let Some(window) = app.get_webview_window("history") {
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

    // Passive acceleration-mode indicator. Reads the currently loaded model
    // path and probes for the sibling `.mlmodelc` directory to decide between
    // ANE (CoreML), Metal, or CPU. Omitted when no model is loaded — nothing
    // meaningful to report. Disabled item; updates on the next menu rebuild.
    let acceleration_item = build_acceleration_status(app, &state)?;

    let separator1 = PredefinedMenuItem::separator(app)?;

    let model_submenu = build_model_submenu(app, &state)?;

    let separator2 = PredefinedMenuItem::separator(app)?;
    // Omit "History…" entirely when history is disabled — there's nothing
    // useful behind it. Matches the acceleration-item pattern above.
    let history_item = {
        let s = lock_or_recover(&state.settings);
        if s.history_enabled && s.history_max_entries > 0 {
            Some(MenuItem::with_id(
                app,
                "history",
                "History\u{2026}",
                true,
                None::<&str>,
            )?)
        } else {
            None
        }
    };
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

    let mut items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = vec![&status];
    if let Some(item) = acceleration_item.as_ref() {
        items.push(item);
    }
    items.extend([
        &separator1 as &dyn tauri::menu::IsMenuItem<tauri::Wry>,
        &model_submenu,
        &separator2,
    ]);
    if let Some(item) = history_item.as_ref() {
        items.push(item);
    }
    items.extend([
        &settings as &dyn tauri::menu::IsMenuItem<tauri::Wry>,
        &check_updates,
        &separator3,
        &quit,
    ]);

    let menu = Menu::with_items(app, &items)?;

    Ok(menu)
}

/// Probe the active model's `.mlmodelc` sibling and the `MAGPIE_DISABLE_COREML`
/// escape hatch to produce a disabled "Acceleration: …" menu item. Returns
/// `Ok(None)` when no model is loaded — there's nothing meaningful to label
/// in that state, so we omit the line entirely.
fn build_acceleration_status(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<Option<MenuItem<tauri::Wry>>, Box<dyn std::error::Error>> {
    let model_path = match lock_or_recover(&state.current_model_path).clone() {
        Some(p) => p,
        None => return Ok(None),
    };

    let disable_coreml = std::env::var("MAGPIE_DISABLE_COREML")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false);

    let label = if disable_coreml {
        "Acceleration: CPU"
    } else {
        let encoder_present = model_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|stem| model_path.with_file_name(format!("{}-encoder.mlmodelc", stem)))
            .map(|p| p.exists())
            .unwrap_or(false);
        if encoder_present {
            "Acceleration: ANE (CoreML)"
        } else {
            "Acceleration: Metal"
        }
    };

    let item = MenuItem::with_id(app, "acceleration", label, false, None::<&str>)?;
    Ok(Some(item))
}

/// Build the "Model" submenu with a CheckMenuItem per available model.
fn build_model_submenu(
    app: &AppHandle,
    state: &Arc<AppState>,
) -> Result<Submenu<tauri::Wry>, Box<dyn std::error::Error>> {
    let all_models = registry::get_available_models();
    let downloaded_filenames = storage::list_downloaded_models().unwrap_or_default();
    let selected_model_id = lock_or_recover(&state.settings).selected_model.clone();

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
            // Each lock scope drops its guard before the next acquires —
            // settings(#1), backend/current_model_path(#2) are touched in
            // *reverse* protocol order here, which is safe ONLY because
            // the guards never co-exist. If a future refactor merges any
            // two of these scopes into one, the merged scope MUST acquire
            // settings (rank 1) first to stay protocol-compliant. See
            // `state::AppState` lock-ordering doc.
            {
                let mut slot = lock_or_recover(&state.backend);
                *slot = Some(Arc::new(backend) as Arc<dyn TranscriptionBackend>);
            }
            {
                let mut model_path = lock_or_recover(&state.current_model_path);
                *model_path = Some(path);
            }
            {
                let mut settings = lock_or_recover(&state.settings);
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
