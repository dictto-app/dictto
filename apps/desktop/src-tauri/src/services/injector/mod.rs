pub mod clipboard;

use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum InjectionError {
    #[error("Clipboard error: {0}")]
    ClipboardError(String),
    #[error("Keyboard simulation error: {0}")]
    KeyboardError(String),
}

pub async fn inject_text(text: &str, paste_delay_ms: u64) -> Result<(), InjectionError> {
    // 1. Save current clipboard content
    let saved = clipboard::save_clipboard();

    // 2. Write text to clipboard
    clipboard::set_clipboard_text(text)?;

    // 3. Small delay before paste
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 4. Simulate Ctrl+V
    simulate_paste()?;

    // 5. Wait for paste to complete
    tokio::time::sleep(Duration::from_millis(paste_delay_ms)).await;

    // 6. Restore original clipboard
    if let Some(content) = saved {
        if let Err(e) = clipboard::restore_clipboard(content) {
            log::warn!("Warning: Failed to restore clipboard: {}", e);
        }
    }

    Ok(())
}

fn simulate_paste() -> Result<(), InjectionError> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| InjectionError::KeyboardError(e.to_string()))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| InjectionError::KeyboardError(e.to_string()))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| InjectionError::KeyboardError(e.to_string()))?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| InjectionError::KeyboardError(e.to_string()))?;

    Ok(())
}
