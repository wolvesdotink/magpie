use arboard::Clipboard;

use crate::output::Result;

/// Get the current clipboard text content (if any)
pub fn get_clipboard_text() -> Option<String> {
    Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok())
}

/// Set clipboard text content
pub fn set_clipboard_text(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text.to_string())?;
    Ok(())
}
