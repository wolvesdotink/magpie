use std::time::Duration;

use tauri::AppHandle;

use crate::constants::CLIPBOARD_RESTORE_DELAY_MS;
use crate::output::{OutputError, Result};

/// Paste text into the active application by:
/// 1. Saving the current clipboard
/// 2. Setting clipboard to the transcription text
/// 3. Simulating Cmd+V (dispatched to main thread for macOS safety)
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
    // On macOS, enigo's Key::Unicode handling calls TISCopyCurrentKeyboardInputSource()
    // and UCKeyTranslate() which are NOT thread-safe and must run on the main thread.
    // We use a channel to wait for the result synchronously.
    let (tx, rx) = std::sync::mpsc::channel::<Result<()>>();

    app.run_on_main_thread(move || {
        let result = (|| -> Result<()> {
            use enigo::{Direction, Enigo, Key, Keyboard, Settings};

            let mut enigo = Enigo::new(&Settings::default())
                .map_err(|e| OutputError::InputSimInit(format!("{e:?}")))?;

            enigo
                .key(Key::Meta, Direction::Press)
                .map_err(|e| OutputError::Keystroke(format!("press Meta: {e:?}")))?;
            enigo
                .key(Key::Unicode('v'), Direction::Click)
                .map_err(|e| OutputError::Keystroke(format!("click v: {e:?}")))?;
            enigo
                .key(Key::Meta, Direction::Release)
                .map_err(|e| OutputError::Keystroke(format!("release Meta: {e:?}")))?;

            Ok(())
        })();
        let _ = tx.send(result);
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
