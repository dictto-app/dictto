use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct MicrophoneInfo {
    pub name: String,
    pub id: String,
    pub is_default: bool,
    pub form_factor: String,
}

#[tauri::command]
pub fn list_microphones() -> Result<Vec<MicrophoneInfo>, String> {
    crate::services::audio::recorder::list_microphones().map_err(|e| e.to_string())
}

/// Returns the friendly name of the device the monitor currently tracks.
/// Used by the frontend to display "Auto-detect (Device Name)".
#[tauri::command]
pub fn get_current_microphone(state: tauri::State<'_, crate::AppState>) -> String {
    state.device_monitor.get_device_name()
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
