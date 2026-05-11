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

        // One-time conversion of the NSWindow to an NSPanel with the
        // non-activating style mask. Without this, the overlay cannot show
        // over fullscreen Spaces — see the doc comment on the function for
        // the full reasoning.
        #[cfg(target_os = "macos")]
        configure_overlay_as_nspanel(&window);

        // Make the pill visible over fullscreen Spaces (otherwise floating
        // windows are hidden when another app takes its own fullscreen Space).
        #[cfg(target_os = "macos")]
        configure_for_fullscreen_spaces(&window);
    }
}

/// One-time conversion of the overlay's underlying `NSWindow` to an `NSPanel`
/// with the non-activating style mask. This is the missing piece that
/// `setLevel` and `setCollectionBehavior` alone could not solve.
///
/// Magpie has `LSUIElement = true` (Info.plist) and uses
/// `NSApplicationActivationPolicyAccessory` (lib.rs). It is therefore never
/// the active app — and especially never active when the user is in another
/// app's fullscreen Space. From that state, a plain `NSWindow`:
///   1. Is clipped by the fullscreen content layer regardless of its level,
///      because regular windows from inactive apps don't compose into a
///      foreign fullscreen Space.
///   2. Is not brought forward by `-orderFront:` (which is what Tauri's
///      `WebviewWindow::show()` eventually calls), because that selector
///      respects the "app must be active" rule.
///
/// Reclassing to `NSPanel` and adding `NSWindowStyleMaskNonactivatingPanel`
/// puts the window in the panel layer, which composes above fullscreen
/// Spaces when combined with `canJoinAllSpaces | fullScreenAuxiliary`, and
/// makes the window eligible for `-orderFrontRegardless` (called from
/// `show_overlay`). This is the same pattern used by Loom, Bartender,
/// Granola, and distilled by the `tauri-nspanel` plugin.
#[cfg(target_os = "macos")]
fn configure_overlay_as_nspanel(window: &WebviewWindow) {
    use cocoa::base::{NO, YES};
    use objc::runtime::{Class, Object};
    use objc::{class, msg_send, sel, sel_impl};

    extern "C" {
        fn object_setClass(obj: *mut Object, cls: *const Class) -> *mut Class;
    }

    // NSWindowStyleMaskNonactivatingPanel = 1 << 7. Only meaningful once the
    // window is an `NSPanel` instance.
    const NS_WINDOW_STYLE_MASK_NON_ACTIVATING_PANEL: u64 = 1 << 7;

    let ns_window = match window.ns_window() {
        Ok(ptr) if !ptr.is_null() => ptr as *mut Object,
        _ => {
            log::warn!("overlay: could not obtain NSWindow for NSPanel conversion");
            return;
        }
    };

    unsafe {
        // Reclass NSWindow → NSPanel. NSPanel is a subclass of NSWindow so
        // all previously-set properties (frame, contentView, level, ...) are
        // preserved; only message dispatch changes.
        let panel_class = class!(NSPanel);
        object_setClass(ns_window, panel_class as *const _);

        // Add the non-activating panel style mask. This is what actually
        // unlocks "display without requiring the owning app to be active".
        let current_style: u64 = msg_send![ns_window, styleMask];
        let _: () = msg_send![
            ns_window,
            setStyleMask: current_style | NS_WINDOW_STYLE_MASK_NON_ACTIVATING_PANEL
        ];

        // Float above ordinary windows in the panel layer.
        let _: () = msg_send![ns_window, setFloatingPanel: YES];

        // Don't auto-hide when Magpie loses focus. Default panel behavior is
        // to hide on app deactivation — which would defeat the entire point
        // of a background-app overlay that has to stay visible while the
        // user works in other apps.
        let _: () = msg_send![ns_window, setHidesOnDeactivate: NO];
    }
    log::info!("overlay: reclassed NSWindow → NSPanel with non-activating style");
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
    const IGNORES_CYCLE: u64 = 1 << 6;
    const FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

    // NSScreenSaverWindowLevel — sits above the fullscreen app's own window,
    // not just the menu-bar reveal (level 24). Status level (25) is enough
    // to beat the menu-bar reveal but loses to the fullscreen content layer,
    // which is why the pill vanished on fullscreen Spaces. 1000 is what
    // AppKit's own overlay UIs (screen-saver, AirPlay HUD) use.
    const NS_SCREEN_SAVER_WINDOW_LEVEL: i64 = 1000;

    let ns_window = match window.ns_window() {
        Ok(ptr) if !ptr.is_null() => ptr as *mut objc::runtime::Object,
        _ => {
            log::warn!("overlay: could not obtain NSWindow for fullscreen configuration");
            return;
        }
    };

    let behavior: u64 =
        CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY | STATIONARY | IGNORES_CYCLE;
    unsafe {
        let _: () = msg_send![ns_window, setCollectionBehavior: behavior];
        let _: () = msg_send![ns_window, setLevel: NS_SCREEN_SAVER_WINDOW_LEVEL];
    }
    log::debug!(
        "overlay: applied fullscreen-space configuration (level=1000, behavior={:#x})",
        behavior
    );
}

/// Show the overlay window (called when recording starts).
pub fn show_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        // Re-center in case monitor setup changed
        center_overlay_horizontally(&window);
        let _ = window.show();

        // Re-apply collection behavior + level after show(). Tauri's internal
        // show path re-asserts `alwaysOnTop` (NSFloatingWindowLevel = 3) and
        // resets collectionBehavior, which would otherwise hide the pill the
        // moment another app owns a fullscreen Space.
        #[cfg(target_os = "macos")]
        {
            configure_for_fullscreen_spaces(&window);
            // Force display even though Magpie is not the active app. The
            // `-orderFront:` selector that Tauri's `show()` ends up calling
            // is gated on app activation — from another app's fullscreen
            // Space we are never active, so the panel would otherwise stay
            // queued and never come forward. `-orderFrontRegardless`
            // bypasses that gate. Only valid because the window is now an
            // `NSPanel` with the non-activating style mask (see
            // `configure_overlay_as_nspanel`).
            order_overlay_front_regardless(&window);
        }
    }
}

/// Send `-orderFrontRegardless` to the overlay's NSPanel. Required because
/// Magpie is an LSUIElement accessory app and the standard show path
/// refuses to display when the owning app is inactive, which is always our
/// case when the user is in another app's fullscreen Space.
#[cfg(target_os = "macos")]
fn order_overlay_front_regardless(window: &WebviewWindow) {
    use objc::{msg_send, sel, sel_impl};

    let ns_window = match window.ns_window() {
        Ok(ptr) if !ptr.is_null() => ptr as *mut objc::runtime::Object,
        _ => return,
    };
    unsafe {
        let _: () = msg_send![ns_window, orderFrontRegardless];
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
