use serde::Serialize;

#[derive(Serialize)]
pub struct MicrophoneInfo {
    pub name: String,
    pub is_default: bool,
}

#[tauri::command]
pub fn list_microphones() -> Result<Vec<MicrophoneInfo>, String> {
    crate::services::audio::recorder::list_microphones().map_err(|e| e.to_string())
}

/// Start continuous recording from UI click (bar pill click)
#[tauri::command]
pub fn bar_start_recording() {
    crate::services::hotkey::send_bar_start();
}

/// Stop continuous recording and process the audio (bar Stop button)
#[tauri::command]
pub fn bar_stop_recording() {
    crate::services::hotkey::send_bar_stop();
}

/// Cancel continuous recording and discard audio (bar Cancel button)
#[tauri::command]
pub fn bar_cancel_recording() {
    crate::services::hotkey::send_bar_cancel();
}
