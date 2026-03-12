mod commands;
mod services;
mod tray;

use log::LevelFilter;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Manager;
use tauri::WebviewUrl;
use tauri::WebviewWindowBuilder;

pub struct AppState {
    pub audio_recorder: Mutex<services::audio::recorder::AudioRecorder>,
    pub db: Mutex<services::db::Database>,
    pub http_client: reqwest::Client,
    pub device_monitor: Arc<services::audio::device_monitor::DeviceMonitorState>,
    pub device_monitor_shutdown: Mutex<Option<std::sync::mpsc::Sender<()>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let audio_recorder = services::audio::recorder::AudioRecorder::new();
    let http_client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");

    let is_autostart = std::env::args().any(|arg| arg == "--autostart");
    log::info!("Launch mode: {}", if is_autostart { "auto-start" } else { "manual" });

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    LevelFilter::Debug
                } else {
                    LevelFilter::Warn
                })
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            commands::audio::list_microphones,
            commands::audio::get_current_microphone,
            commands::audio::bar_start_recording,
            commands::audio::bar_stop_recording,
            commands::audio::bar_cancel_recording,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::settings::get_all_settings,
            commands::settings::set_api_key,
            commands::settings::has_api_key,
            commands::settings::remove_api_key,
            commands::settings::get_api_key_hint,
            commands::window::update_pill_hitbox,
            commands::window::clear_pill_hitbox,
        ])
        .setup(move |app| {
            // Initialize database in Tauri's local data directory
            let data_dir = app.path().app_local_data_dir()?;
            let db = services::db::Database::new(data_dir)
                .expect("Failed to initialize database");

            // Spawn device monitor thread (Windows only — cfg guard inside spawn_monitor)
            let device_monitor_state = Arc::new(
                services::audio::device_monitor::DeviceMonitorState::new()
            );
            #[cfg(windows)]
            let shutdown_tx = services::audio::device_monitor::spawn_monitor(
                device_monitor_state.clone(),
                app.handle().clone(),
            );
            #[cfg(not(windows))]
            let shutdown_tx: Option<std::sync::mpsc::Sender<()>> = None;
            #[cfg(windows)]
            let shutdown_tx = Some(shutdown_tx);

            app.manage(AppState {
                audio_recorder: Mutex::new(audio_recorder),
                db: Mutex::new(db),
                http_client,
                device_monitor: device_monitor_state,
                device_monitor_shutdown: Mutex::new(shutdown_tx),
            });

            // Migration: ensure existing auto-start registry entries include --autostart flag
            {
                let state = app.state::<AppState>();
                let db = state.db.lock().unwrap();
                if db.get_setting("auto_start").as_deref() == Some("true") {
                    if let Ok(exe_path) = std::env::current_exe() {
                        let _ = services::autostart::enable_autostart(&exe_path.to_string_lossy());
                    }
                }
            }

            // Inject AppHandle into AudioRecorder for waveform event emission
            {
                let state = app.state::<AppState>();
                let mut recorder = state.audio_recorder.lock().unwrap();
                recorder.set_app_handle(app.handle().clone());
            }

            tray::setup_tray(app)?;
            services::hotkey::register_hotkey(app)?;

            // Create recording bar overlay window
            let monitor = app
                .primary_monitor()
                .ok()
                .flatten()
                .or_else(|| app.available_monitors().ok().and_then(|m| m.into_iter().next()));

            let (x, y, bar_width, bar_height) = if let Some(mon) = monitor {
                let size = mon.size();
                let pos = mon.position();
                let scale = mon.scale_factor();
                let w = 360.0;
                let h = 120.0;
                let x = pos.x as f64 + (size.width as f64 / scale - w) / 2.0;
                let y = pos.y as f64 + size.height as f64 / scale - h - 60.0;
                (x, y, w, h)
            } else {
                (560.0, 740.0, 360.0, 120.0)
            };

            WebviewWindowBuilder::new(
                app,
                "recording-bar",
                WebviewUrl::App("/".into()),
            )
            .title("Dictto Recording Bar")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .background_color(tauri::window::Color(0, 0, 0, 0))
            .always_on_top(true)
            .resizable(false)
            .skip_taskbar(true)
            .inner_size(bar_width, bar_height)
            .position(x, y)
            .build()?;

            // Recording bar is ALWAYS click-through. The WH_MOUSE_LL hook
            // intercepts clicks inside the pill hitbox and re-emits them as
            // Tauri events, so WebView2 never receives mouse clicks and never
            // steals focus from the user's active application.
            if let Some(bar_window) = app.get_webview_window("recording-bar") {
                let _ = bar_window.set_ignore_cursor_events(true);
            }
            services::cursor_passthrough::start_mouse_hook(app.handle().clone());

            // Set high-resolution window icon for sharp taskbar display
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = main_window.set_icon(tauri::include_image!("./icons/icon.png"));
            }

            // Auto-update: check for updates silently after startup
            #[cfg(desktop)]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // Skip update check in dev mode — no valid release endpoint exists
                    if cfg!(dev) {
                        log::debug!("[updater] Skipping update check (dev mode)");
                        return;
                    }

                    // Delay to avoid blocking startup
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    let update_result =
                        handle.plugin(tauri_plugin_updater::Builder::new().build());

                    if let Err(e) = update_result {
                        log::error!("[updater] Failed to initialize updater plugin: {}", e);
                        return;
                    }

                    let updater = match tauri_plugin_updater::UpdaterExt::updater(&handle) {
                        Ok(u) => u,
                        Err(e) => {
                            log::error!("[updater] Failed to create updater: {}", e);
                            return;
                        }
                    };

                    match updater.check().await {
                        Ok(Some(update)) => {
                            log::info!(
                                "[updater] Update available: v{} (current: v{})",
                                update.version, update.current_version
                            );
                            match update.download_and_install(|_, _| {}, || {}).await {
                                Ok(_) => log::info!(
                                    "[updater] Update downloaded and staged for next restart"
                                ),
                                Err(e) => {
                                    log::error!("[updater] Download/install failed: {}", e)
                                }
                            }
                        }
                        Ok(None) => log::debug!("[updater] No update available"),
                        Err(e) => log::error!("[updater] Update check failed: {}", e),
                    }
                });
            }

            // Show Settings window on manual launch (not auto-start)
            if !is_autostart {
                if let Some(main_window) = app.get_webview_window("main") {
                    let _ = main_window.show();
                    let _ = main_window.set_focus();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide the Settings window instead of destroying it (tray app pattern)
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
