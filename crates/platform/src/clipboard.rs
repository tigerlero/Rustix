use arboard::Clipboard;

/// Get text from the system clipboard.
pub fn get_text() -> Option<String> {
    match Clipboard::new() {
        Ok(mut cb) => cb.get_text().ok(),
        Err(e) => {
            tracing::warn!("failed to open clipboard: {}", e);
            None
        }
    }
}

/// Set text on the system clipboard.
pub fn set_text(text: &str) -> Result<(), String> {
    let mut cb = Clipboard::new().map_err(|e| format!("failed to open clipboard: {}", e))?;
    cb.set_text(text).map_err(|e| format!("failed to set clipboard text: {}", e))
}

/// Clear the system clipboard.
pub fn clear() -> Result<(), String> {
    let mut cb = Clipboard::new().map_err(|e| format!("failed to open clipboard: {}", e))?;
    cb.clear().map_err(|e| format!("failed to clear clipboard: {}", e))
}
