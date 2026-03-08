use super::InjectionError;
use arboard::Clipboard;

pub enum ClipboardContent {
    Text(String),
    // Image support can be added later
}

pub fn save_clipboard() -> Option<ClipboardContent> {
    let mut clipboard = Clipboard::new().ok()?;
    clipboard
        .get_text()
        .ok()
        .map(ClipboardContent::Text)
}

pub fn set_clipboard_text(text: &str) -> Result<(), InjectionError> {
    let mut clipboard =
        Clipboard::new().map_err(|e| InjectionError::ClipboardError(e.to_string()))?;
    clipboard
        .set_text(text)
        .map_err(|e| InjectionError::ClipboardError(e.to_string()))
}

pub fn restore_clipboard(content: ClipboardContent) -> Result<(), InjectionError> {
    let mut clipboard =
        Clipboard::new().map_err(|e| InjectionError::ClipboardError(e.to_string()))?;
    match content {
        ClipboardContent::Text(text) => clipboard
            .set_text(text)
            .map_err(|e| InjectionError::ClipboardError(e.to_string())),
    }
}
