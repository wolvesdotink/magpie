use tauri::{AppHandle, Manager, WebviewWindow, window::Color};

/// Configure the overlay window after creation.
/// Sets click-through and positions it centered at top of screen.
pub fn setup_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        // Make click-through so it never steals focus or intercepts clicks
        let _ = window.set_ignore_cursor_events(true);

        // Force the native window + webview background to fully transparent
        let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));

        // Center horizontally on the primary monitor
        center_overlay_horizontally(&window);

        // Make the pill visible over fullscreen Spaces (otherwise floating
        // windows are hidden when another app takes its own fullscreen Space).
        #[cfg(target_os = "macos")]
        configure_for_fullscreen_spaces(&window);
    }
}

/// Pin the overlay NSWindow to every Space (including fullscreen Spaces) and
/// raise its level above the fullscreen menu-bar reveal. Tauri's `alwaysOnTop`
/// only sets `NSFloatingWindowLevel` with default collectionBehavior, which
/// hides the window the moment another app enters its own fullscreen Space.
#[cfg(target_os = "macos")]
fn configure_for_fullscreen_spaces(window: &WebviewWindow) {
    use objc::{msg_send, sel, sel_impl};

    // NSWindowCollectionBehavior bitmask values from AppKit/NSWindow.h
    const CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
    const STATIONARY: u64 = 1 << 4;
    const FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

    // NSStatusWindowLevel — above NSFloatingWindowLevel(3), stays over the
    // fullscreen menu-bar reveal.
    const NS_STATUS_WINDOW_LEVEL: i64 = 25;

    let ns_window = match window.ns_window() {
        Ok(ptr) if !ptr.is_null() => ptr as *mut objc::runtime::Object,
        _ => {
            log::warn!("overlay: could not obtain NSWindow for fullscreen configuration");
            return;
        }
    };

    unsafe {
        let behavior: u64 = CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY | STATIONARY;
        let _: () = msg_send![ns_window, setCollectionBehavior: behavior];
        let _: () = msg_send![ns_window, setLevel: NS_STATUS_WINDOW_LEVEL];
    }
}

/// Show the overlay window (called when recording starts).
pub fn show_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        // Re-center in case monitor setup changed
        center_overlay_horizontally(&window);
        let _ = window.show();
    }
}

/// Hide the overlay window (called when back to idle).
pub fn hide_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
}

fn center_overlay_horizontally(window: &WebviewWindow) {
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let screen_width = monitor.size().width;
        let screen_height = monitor.size().height;
        let scale = monitor.scale_factor();
        let window_width = 360.0; // logical pixels, matches tauri.conf.json
        let window_height = 84.0; // logical pixels, matches tauri.conf.json
        let x = ((screen_width as f64 / scale) - window_width) / 2.0;
        let y = (screen_height as f64 / scale) - window_height - 38.0;
        let _ = window.set_position(tauri::Position::Logical(
            tauri::LogicalPosition::new(x, y),
        ));
    }
}
