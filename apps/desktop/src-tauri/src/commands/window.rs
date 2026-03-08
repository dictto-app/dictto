use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn update_pill_hitbox(left: i32, top: i32, right: i32, bottom: i32) {
    crate::services::cursor_passthrough::update_hitbox(left, top, right, bottom);
}

#[tauri::command]
pub fn clear_pill_hitbox(app: AppHandle) {
    crate::services::cursor_passthrough::clear_hitbox();
    if let Some(win) = app.get_webview_window("recording-bar") {
        let _ = win.set_ignore_cursor_events(true);
    }
}
