use std::time::Duration;

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use tauri::AppHandle;

use crate::constants::CLIPBOARD_RESTORE_DELAY_MS;
use crate::output::{OutputError, Result};

/// Virtual keycode for the ANSI 'V' key (kVK_ANSI_V). Pairing this keycode with
/// the Command flag is the system Paste shortcut on QWERTY / QWERTZ / AZERTY
/// layouts. Exotic remaps (Dvorak/Colemak) relocate 'v' and are unsupported —
/// a pre-existing limitation.
const KVK_ANSI_V: u16 = 9;

/// Paste text into the active application by:
/// 1. Saving the current clipboard
/// 2. Setting clipboard to the transcription text
/// 3. Simulating Cmd+V (see `press_cmd_v`)
/// 4. Restoring the original clipboard after a delay
pub fn paste_text(app: &AppHandle, text: &str) -> Result<()> {
    // Save current clipboard
    let original_clipboard = super::clipboard::get_clipboard_text();

    // Set our text
    super::clipboard::set_clipboard_text(text)?;

    // Small delay to ensure clipboard is set
    std::thread::sleep(Duration::from_millis(50));

    // Simulate Cmd+V on the main thread.
    //
    // The keystroke itself (`press_cmd_v`) no longer touches the thread-unsafe
    // Cocoa APIs (TISCopyCurrentKeyboardInputSource / UCKeyTranslate) that the
    // old enigo `Key::Unicode` path used — it posts raw CGEvents with the
    // Command flag set directly on the 'v' key events. We keep the main-thread
    // dispatch as a low-risk safety margin and to preserve the synchronous wait
    // via the channel; it is no longer strictly required.
    let (tx, rx) = std::sync::mpsc::channel::<Result<()>>();

    app.run_on_main_thread(move || {
        let _ = tx.send(press_cmd_v());
    })
    .map_err(|e| OutputError::Keystroke(format!("dispatch to main thread: {e:?}")))?;

    rx.recv()
        .map_err(|e| OutputError::Keystroke(format!("main thread channel closed: {e:?}")))??;

    // Restore original clipboard after a delay
    if let Some(original) = original_clipboard {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(CLIPBOARD_RESTORE_DELAY_MS));
            let _ = super::clipboard::set_clipboard_text(&original);
        });
    }

    Ok(())
}

/// Synthesize a Cmd+V paste by posting `v` key-down/up events with the Command
/// flag set *directly on each event*.
///
/// We deliberately do NOT press a separate Command key and rely on the system's
/// global modifier state propagating before the 'v' event arrives (which is what
/// enigo did). That approach is racy under load: right after a long transcription
/// the 'v' event could be processed before Command registered, so the focused app
/// received a literal "v" instead of pasting. Setting the flag on the event makes
/// Cmd+V atomic. Because we never toggle a real modifier key, there is also no
/// risk of a "stuck" Command modifier, and we emit no `FlagsChanged` event that
/// could disturb the Fn-key CGEventTap in `hotkey.rs`.
fn press_cmd_v() -> Result<()> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|()| OutputError::InputSimInit("CGEventSource::new failed".into()))?;

    let down = CGEvent::new_keyboard_event(source.clone(), KVK_ANSI_V, true)
        .map_err(|()| OutputError::Keystroke("create V key-down event".into()))?;
    down.set_flags(CGEventFlags::CGEventFlagCommand);
    down.post(CGEventTapLocation::HID);

    let up = CGEvent::new_keyboard_event(source, KVK_ANSI_V, false)
        .map_err(|()| OutputError::Keystroke("create V key-up event".into()))?;
    up.set_flags(CGEventFlags::CGEventFlagCommand);
    up.post(CGEventTapLocation::HID);

    Ok(())
}
