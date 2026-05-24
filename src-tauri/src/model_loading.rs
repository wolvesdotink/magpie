//! Startup model-loading flow for whisper + correction models.
//!
//! Called from [`crate::setup::setup_app`] on every launch. The whisper
//! path also handles the post-load CoreML encoder backfill (re-fetching
//! the `.mlmodelc` if it's missing and reloading the backend), the
//! deferred-reload race when an encoder lands mid-recording, and the
//! public `flush_pending_reload` hook used by `stop_recording`.
//!
//! Both load paths run the actual FFI work on a dedicated `std::thread`
//! so a C++ exception from whisper.cpp / llama.cpp aborts only the
//! loader thread instead of the whole app. The setup `catch_unwind`
//! does NOT cross the C ABI boundary.

use std::sync::Arc;

use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;

use crate::correction;
use crate::events;
use crate::models;
use crate::state::{lock_or_recover, AppState};
use crate::transcription;

pub type LoadedBackend = Arc<dyn transcription::backend::TranscriptionBackend>;
pub type SelfTestResult = Result<(), transcription::whisper_backend::SelfTestError>;
pub type LoadOutcome = Result<(LoadedBackend, SelfTestResult), String>;

/// Load a `WhisperBackend` and run its CoreML self-test on a dedicated
/// thread. Wrapping the FFI work in a thread isolates C++ exceptions and
/// the abort-the-thread-only behavior we already rely on for `load`.
///
/// Returns `Ok((backend, self_test_result))` on success — the caller decides
/// what to do with a failed self-test (typically: quarantine the encoder
/// and call this helper again). Returns `Err` only when the model itself
/// cannot be loaded.
pub fn load_with_self_test(path: &std::path::Path) -> LoadOutcome {
    let path_clone = path.to_path_buf();
    let handle = std::thread::Builder::new()
        .name("whisper-load".into())
        .spawn(move || -> LoadOutcome {
            let backend = transcription::whisper_backend::WhisperBackend::load(&path_clone)
                .map_err(|e| format!("{}", e))?;
            let st = backend.self_test();
            Ok((Arc::new(backend) as LoadedBackend, st))
        })
        .map_err(|e| format!("Failed to spawn whisper-load thread: {}", e))?;

    handle
        .join()
        .map_err(|_| "whisper-load thread crashed".to_string())?
}

/// Try to load the last used whisper model on startup.
///
/// On a clean Rust load failure, the selection is left alone so the user
/// can retry on the next launch. On a thread crash (typical signature of
/// a corrupt / version-mismatched GGML file), the selection is cleared
/// to prevent a crash-loop.
///
/// After a successful load, schedules a background CoreML encoder
/// backfill if the registry entry advertises an encoder URL but the
/// sibling `.mlmodelc` directory is missing — recovers gracefully from
/// upgrades that introduce ANE acceleration over models downloaded by
/// an older build.
pub fn try_load_last_model(state: &Arc<AppState>, app_handle: &tauri::AppHandle) {
    let (model_id, path, info) = match resolve_selected_model(state) {
        Some(t) => t,
        None => return,
    };

    let (backend, self_test) = match load_with_self_test(&path) {
        Ok(pair) => pair,
        Err(e) => {
            if e.contains("crashed") {
                log::error!(
                    "whisper-load thread crashed while loading model {}. \
                     Clearing selection to prevent crash loop.",
                    model_id
                );
                let mut settings = lock_or_recover(&state.settings);
                settings.selected_model = None;
                let _ = settings.save();
            } else {
                log::error!("Failed to auto-load model {}: {}", model_id, e);
            }
            return;
        }
    };

    if let Err(e) = self_test {
        log::warn!(
            "Self-test for model {} returned: {}. \
             Continuing — first real transcription will surface any issue.",
            model_id,
            e
        );
    }

    *lock_or_recover(&state.backend) = Some(backend);
    *lock_or_recover(&state.current_model_path) = Some(path.clone());
    log::info!("Auto-loaded model: {}", model_id);

    // CoreML encoder backfill — non-blocking. See function comment.
    maybe_backfill_coreml_encoder(app_handle, state.clone(), &info, model_id, path);
}

/// Resolve the user's `selected_model` to `(id, on-disk path, registry info)`,
/// or `None` if nothing is selected, the id is unknown, or the file is missing.
/// Reads only `settings` (rank 1) and drops the guard before returning.
fn resolve_selected_model(
    state: &AppState,
) -> Option<(String, std::path::PathBuf, models::registry::ModelInfo)> {
    let id = lock_or_recover(&state.settings).selected_model.clone()?;
    let info = models::registry::find_model(&id)?;
    let path = models::storage::model_path(&info.filename)
        .ok()
        .filter(|p| p.exists())?;
    Some((id, path, info))
}

/// Whether the app has a model the user can dictate with — either resident
/// now, or (under Memory Saver) selected and present on disk so the next
/// dictation will load it. UI routing keys off this so an idle-unloaded or
/// not-yet-loaded model doesn't look like "no model configured".
pub fn has_usable_model(state: &AppState) -> bool {
    if lock_or_recover(&state.backend).is_some() {
        return true;
    }
    // Backend not resident. Under Memory Saver a configured, on-disk model
    // still counts — we'll lazily load it on the next Fn press.
    let (memory_saver, selected) = {
        let s = lock_or_recover(&state.settings);
        (s.memory_saver, s.selected_model.clone())
    };
    if !memory_saver {
        return false;
    }
    selected
        .and_then(|id| models::registry::find_model(&id))
        .and_then(|info| models::storage::model_path(&info.filename).ok())
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Drop the resident transcription + correction models, returning their RAM
/// to the OS. Used by the Memory Saver idle-unload watchdog and the
/// settings toggle. Holds `model_load_lock` (rank 0) for the whole operation
/// so it can never interleave with a lazy load. `current_model_path` and
/// `current_correction_model_path` are intentionally kept as breadcrumbs (the
/// tray's acceleration label reads them) — the heavy `Arc`s are what free the
/// memory.
pub fn unload_models(state: &AppState) {
    let _load = lock_or_recover(&state.model_load_lock);

    let had_model = lock_or_recover(&state.backend).is_some();
    let had_correction = lock_or_recover(&state.correction_model).is_some();

    *lock_or_recover(&state.backend) = None;
    // Drop the LlamaModel (weights) BEFORE the LlamaBackend it was created
    // against, then the backend itself — dropping the backend resets
    // llama.cpp's global init flag so a later reload can re-init cleanly.
    *lock_or_recover(&state.correction_model) = None;
    *lock_or_recover(&state.llama_backend) = None;

    if had_model || had_correction {
        log::info!(
            "Memory Saver: unloaded idle models (whisper={}, correction={})",
            had_model,
            had_correction
        );
    }
}

/// Return the resident transcription backend, loading the user's selected
/// model first if it isn't resident (Memory Saver unloaded it, or it was
/// never eagerly loaded this launch). Returns `None` only when no model is
/// configured or the load fails.
///
/// Synchronous and intended for a blocking context (the `stop_recording`
/// task) or a background thread (the recording-start preload). Concurrent
/// callers are serialized by `model_load_lock`; the late caller sees the
/// backend already populated and returns it without reloading.
pub fn ensure_backend_loaded(state: &AppState) -> Option<LoadedBackend> {
    if let Some(b) = lock_or_recover(&state.backend).clone() {
        return Some(b);
    }

    let _load = lock_or_recover(&state.model_load_lock);
    // Re-check under the load lock: another caller may have just loaded it.
    if let Some(b) = lock_or_recover(&state.backend).clone() {
        return Some(b);
    }

    let (model_id, path, _info) = resolve_selected_model(state)?;
    let (backend, self_test) = match load_with_self_test(&path) {
        Ok(pair) => pair,
        Err(e) => {
            log::error!("Lazy model load failed for {}: {}", model_id, e);
            return None;
        }
    };
    if let Err(e) = self_test {
        log::warn!(
            "Self-test for {} on lazy load returned: {}. Continuing.",
            model_id,
            e
        );
    }

    *lock_or_recover(&state.backend) = Some(backend.clone());
    *lock_or_recover(&state.current_model_path) = Some(path);
    log::info!("Lazy-loaded model: {}", model_id);
    Some(backend)
}

/// Ensure the user's selected correction model is resident, loading it if
/// Memory Saver unloaded it. No-op when none is configured or one is already
/// loaded. The caller re-reads `llama_backend` / `correction_model` after this.
///
/// Mirrors [`try_load_last_correction_model`]'s crash-isolation: the llama.cpp
/// FFI work runs on a dedicated thread so a C++ exception aborts only that
/// thread. Serialized against unloads via `model_load_lock`.
pub fn ensure_correction_loaded(state: &AppState) {
    if lock_or_recover(&state.correction_model).is_some() {
        return;
    }

    let _load = lock_or_recover(&state.model_load_lock);
    if lock_or_recover(&state.correction_model).is_some() {
        return;
    }

    let (model_id, path) = {
        let id = match lock_or_recover(&state.settings)
            .selected_correction_model
            .clone()
        {
            Some(id) => id,
            None => return,
        };
        let info = match correction::registry::find_correction_model(&id) {
            Some(i) => i,
            None => return,
        };
        match models::storage::model_path(&info.filename) {
            Ok(p) if p.exists() => (id, p),
            _ => return,
        }
    };

    let path_clone = path.clone();
    let handle = std::thread::Builder::new().name("llm-reload".into()).spawn(
        move || -> Result<(LlamaBackend, LlamaModel), String> {
            let backend = LlamaBackend::init()
                .map_err(|e| format!("Failed to init llama backend: {:?}", e))?;
            let model = correction::engine::load_correction_model(&backend, &path_clone)
                .map_err(|e| format!("Failed to load correction model: {}", e))?;
            Ok((backend, model))
        },
    );

    let handle = match handle {
        Ok(h) => h,
        Err(e) => {
            log::error!("Failed to spawn llm-reload thread: {}", e);
            return;
        }
    };

    match handle.join() {
        Ok(Ok((backend, model))) => {
            *lock_or_recover(&state.llama_backend) = Some(backend);
            *lock_or_recover(&state.correction_model) = Some(model);
            *lock_or_recover(&state.current_correction_model_path) = Some(path);
            log::info!("Lazy-loaded correction model: {}", model_id);
        }
        Ok(Err(e)) => {
            log::error!("Lazy correction load failed for {}: {}", model_id, e);
        }
        Err(_) => {
            log::error!("llm-reload thread crashed while loading {}", model_id);
        }
    }
}

/// If the loaded model's registry entry advertises a CoreML encoder URL
/// and the sibling `.mlmodelc` directory is missing, kick off a background
/// download. Once the encoder lands we attempt to swap in a fresh
/// `WhisperContext` so the next transcription picks up ANE acceleration —
/// or, if the user is mid-recording, we defer the swap until idle (see
/// `pending_reload` in `AppState`).
fn maybe_backfill_coreml_encoder(
    app: &tauri::AppHandle,
    state: Arc<AppState>,
    info: &models::registry::ModelInfo,
    model_id: String,
    model_path: std::path::PathBuf,
) {
    let encoder_url = match info.encoder_url.clone() {
        Some(u) => u,
        None => return, // Distil models etc. — no CoreML encoder
    };

    let encoder_size = info.encoder_size_bytes.unwrap_or(0);
    let models_dir = match models::storage::models_dir() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Cannot resolve models dir for encoder backfill: {}", e);
            return;
        }
    };
    let encoder_dir = models_dir.join(models::downloader::encoder_dir_name_from_filename(
        &info.filename,
    ));
    if encoder_dir.exists() {
        return;
    }

    log::warn!(
        "CoreML enabled but encoder missing for model '{}' at {:?}. \
         whisper.cpp may fail with GenericError(-6) despite ALLOW_FALLBACK. \
         Triggering background backfill.",
        model_id,
        encoder_dir
    );

    let app2 = app.clone();
    let id2 = model_id.clone();
    let path2 = model_path.clone();
    let state2 = state.clone();
    tauri::async_runtime::spawn(async move {
        // Retry budget: 3 attempts total, ~30s then ~120s between, covering
        // most transient network blips without bothering the user. On final
        // failure we just log and exit — the next launch re-enters this
        // function (encoder_dir.exists() check at line 146 still false) so
        // recovery is automatic across restarts.
        const BACKOFF_SECS: &[u64] = &[30, 120];
        let mut last_err: Option<crate::models::ModelError> = None;
        for attempt in 0..=BACKOFF_SECS.len() {
            match models::downloader::download_encoder_only(
                &app2,
                &id2,
                &encoder_url,
                encoder_size,
                &encoder_dir,
                None,
            )
            .await
            {
                Ok(()) => {
                    log::info!(
                        "CoreML encoder backfill complete for {} on attempt {}; reloading backend",
                        id2,
                        attempt + 1
                    );
                    reload_backend_after_backfill(app2, state2, id2, path2).await;
                    return;
                }
                Err(e) => {
                    if attempt < BACKOFF_SECS.len() {
                        let wait = BACKOFF_SECS[attempt];
                        log::warn!(
                            "CoreML encoder backfill attempt {} for {} failed: {}. \
                             Retrying in {}s.",
                            attempt + 1,
                            id2,
                            e,
                            wait
                        );
                        last_err = Some(e);
                        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                    } else {
                        last_err = Some(e);
                    }
                }
            }
        }
        if let Some(e) = last_err {
            log::warn!(
                "CoreML encoder backfill for {} gave up after {} attempts: {}. \
                 Continuing in Metal mode; next app launch will retry.",
                id2,
                BACKOFF_SECS.len() + 1,
                e
            );
        }
    });
}

/// Rebuild the `WhisperContext` so the freshly downloaded `.mlmodelc`
/// takes effect. Defers to the next idle moment if a recording is in
/// flight — interrupting an active dictation is worse UX than waiting one
/// cycle. The deferred reload is kicked from `commands::recording::stop_recording`.
async fn reload_backend_after_backfill(
    app: tauri::AppHandle,
    state: Arc<AppState>,
    model_id: String,
    path: std::path::PathBuf,
) {
    if state.is_recording() || state.is_processing() {
        log::info!(
            "Encoder ready for {}; deferring backend reload until idle",
            model_id
        );
        *lock_or_recover(&state.pending_reload) = Some((model_id, path));
        return;
    }

    let (backend, self_test) = match load_with_self_test(&path) {
        Ok(pair) => pair,
        Err(e) => {
            log::error!(
                "Backend reload after encoder backfill failed for {}: {}",
                model_id,
                e
            );
            return;
        }
    };

    if let Err(e) = self_test {
        log::warn!("Self-test for {} after backfill returned: {}", model_id, e);
    }

    *lock_or_recover(&state.backend) = Some(backend);
    *lock_or_recover(&state.current_model_path) = Some(path);
    log::info!("Backend reloaded with CoreML encoder for {}", model_id);
    emit_app_state(&app, &state);
}

fn emit_app_state(app: &tauri::AppHandle, state: &Arc<AppState>) {
    let payload = events::AppStatePayload {
        recording: state.is_recording(),
        processing: state.is_processing(),
        has_model: true,
        last_transcription: lock_or_recover(&state.last_transcription).clone(),
    };
    events::emit_event(app, events::event_names::APP_STATE_CHANGED, payload);
}

/// Public entry point used by `commands::recording::stop_recording` to
/// drain a deferred backend reload once the recording-and-processing
/// cycle ends.
pub async fn flush_pending_reload(app: tauri::AppHandle, state: Arc<AppState>) {
    let pending = lock_or_recover(&state.pending_reload).take();
    if let Some((model_id, path)) = pending {
        reload_backend_after_backfill(app, state, model_id, path).await;
    }
}

/// Try to load the last used correction model on startup.
///
/// The actual llama.cpp FFI work runs in a dedicated thread so that a C++
/// exception escaping the FFI boundary (which `catch_unwind` cannot catch)
/// aborts only that thread instead of the entire application.
pub fn try_load_last_correction_model(state: &Arc<AppState>) {
    let selected = lock_or_recover(&state.settings)
        .selected_correction_model
        .clone();

    let (model_id, path) = match selected {
        Some(id) => {
            let info = match correction::registry::find_correction_model(&id) {
                Some(i) => i,
                None => return,
            };
            let p = match models::storage::model_path(&info.filename) {
                Ok(p) if p.exists() => p,
                _ => return,
            };
            (id, p)
        }
        None => return,
    };

    // Run the FFI-heavy work on a separate thread.  If llama.cpp throws a C++
    // exception that escapes through the extern-"C" boundary, it will abort
    // *this* thread rather than the main thread, keeping the app alive.
    let path_clone = path.clone();
    let load_handle = std::thread::Builder::new()
        .name("llm-load".into())
        .spawn(move || -> Result<(llama_cpp_2::llama_backend::LlamaBackend, llama_cpp_2::model::LlamaModel), String> {
            let backend = llama_cpp_2::llama_backend::LlamaBackend::init()
                .map_err(|e| format!("Failed to init llama backend: {:?}", e))?;
            let model = correction::engine::load_correction_model(&backend, &path_clone)
                .map_err(|e| format!("Failed to load correction model: {}", e))?;
            Ok((backend, model))
        });

    let handle = match load_handle {
        Ok(h) => h,
        Err(e) => {
            log::error!("Failed to spawn llm-load thread: {}", e);
            return;
        }
    };

    // Wait for the loader thread.  A panic or C++ abort in the thread
    // surfaces as an Err from join() – we handle it gracefully.
    match handle.join() {
        Ok(Ok((backend, model))) => {
            *lock_or_recover(&state.llama_backend) = Some(backend);
            *lock_or_recover(&state.correction_model) = Some(model);
            *lock_or_recover(&state.current_correction_model_path) = Some(path);
            log::info!("Auto-loaded correction model: {}", model_id);
        }
        Ok(Err(e)) => {
            // Clean Rust error – model file is likely valid, don't delete it.
            log::error!("Failed to auto-load correction model {}: {}", model_id, e);
            let mut settings = lock_or_recover(&state.settings);
            settings.selected_correction_model = None;
            let _ = settings.save();
        }
        Err(_) => {
            // The thread panicked or was aborted (e.g. foreign C++ exception).
            // Clear the selection so we don't crash-loop, but keep the file.
            log::error!(
                "llm-load thread crashed while loading correction model {}. \
                 Clearing selection to prevent crash loop.",
                model_id
            );
            let mut settings = lock_or_recover(&state.settings);
            settings.selected_correction_model = None;
            let _ = settings.save();
        }
    }
}
