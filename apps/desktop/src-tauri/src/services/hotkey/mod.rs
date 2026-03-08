use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Emitter;
use tauri::Manager;
use crate::services::sound::{play_sound, Sound};

#[cfg(windows)]
use windows::Win32::Foundation::LRESULT;
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LCONTROL, VK_LWIN, VK_RCONTROL, VK_RWIN};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

type AsyncJoinHandle = tauri::async_runtime::JoinHandle<()>;

// --- Hook → Tauri communication ---

enum HotkeyEvent {
    Pressed,
    Released,
}

/// Frontend → Rust commands for bar buttons
enum BarAction {
    Start,
    Stop,
    Cancel,
}

static HOTKEY_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<HotkeyEvent>> = OnceLock::new();

// Key state tracking (used by the hook proc — called sequentially, no races)
static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
static LWIN_DOWN: AtomicBool = AtomicBool::new(false);
static COMBO_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Low-level keyboard hook procedure.
/// Tracks Ctrl + Win state and sends Pressed/Released events via channel.
#[cfg(windows)]
unsafe extern "system" fn hook_proc(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> LRESULT {
    if code >= 0 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kb.vkCode as u16;

        let is_down = wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize;
        let is_up = wparam.0 == WM_KEYUP as usize || wparam.0 == WM_SYSKEYUP as usize;

        if is_down || is_up {
            let pressed = is_down;

            if vk == VK_LCONTROL.0 || vk == VK_RCONTROL.0 {
                CTRL_DOWN.store(pressed, Ordering::SeqCst);
            } else if vk == VK_LWIN.0 || vk == VK_RWIN.0 {
                LWIN_DOWN.store(pressed, Ordering::SeqCst);
            }

            // Detect combo transitions
            let both = CTRL_DOWN.load(Ordering::SeqCst) && LWIN_DOWN.load(Ordering::SeqCst);
            let was_active = COMBO_ACTIVE.swap(both, Ordering::SeqCst);

            if both && !was_active {
                if let Some(tx) = HOTKEY_TX.get() {
                    let _ = tx.send(HotkeyEvent::Pressed);
                }
            } else if !both && was_active {
                if let Some(tx) = HOTKEY_TX.get() {
                    let _ = tx.send(HotkeyEvent::Released);
                }
            }
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Spawn a dedicated OS thread with a message loop for the keyboard hook.
#[cfg(windows)]
fn spawn_hook_thread() {
    std::thread::Builder::new()
        .name("hotkey-hook".into())
        .spawn(|| unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0);

            if let Err(e) = &hook {
                log::error!("Failed to install keyboard hook: {}", e);
                return;
            }

            // Message loop — required for WH_KEYBOARD_LL to work
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                // Just pump messages; the hook proc handles everything
            }
        })
        .expect("Failed to spawn hotkey hook thread");
}

/// Emit structured recording state event to frontend.
/// Payload: { "state": "recording", "mode": "hold" } or { "state": "idle", "mode": null }
fn emit_state(app: &tauri::AppHandle, state: &str, mode: Option<&str>) {
    let _ = app.emit(
        "recording-state-changed",
        serde_json::json!({ "state": state, "mode": mode }),
    );
}

/// Synchronously stop the recorder and return WAV data.
/// Returns None for empty/short recordings or stop failures.
fn stop_recording_sync(app: &tauri::AppHandle) -> Option<Vec<u8>> {
    let t = std::time::Instant::now();
    let app_state = app.state::<crate::AppState>();
    let mut recorder = app_state.audio_recorder.lock().unwrap();
    let result = match recorder.stop() {
        Ok(data) => {
            log::info!(
                "[pipeline] Audio stop + WAV encode: {:?} ({:.1} KB)",
                t.elapsed(),
                data.len() as f64 / 1024.0
            );
            Some(data)
        }
        Err(crate::services::audio::recorder::AudioError::EmptyRecording) => None,
        Err(e) => {
            log::error!("[hotkey] Failed to stop recording: {}", e);
            None
        }
    };
    drop(recorder);
    play_sound(Sound::Stop, app);
    result
}

/// Spawn the async pipeline for the given audio data.
/// If audio_data is None (empty recording), emits idle immediately.
fn spawn_pipeline(
    app: &tauri::AppHandle,
    audio_data: Option<Vec<u8>>,
    pipeline_running: &Arc<AtomicBool>,
) {
    let Some(data) = audio_data else {
        emit_state(app, "idle", None);
        return;
    };

    if pipeline_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        log::warn!("[hotkey] Pipeline already running, discarding audio");
        emit_state(app, "idle", None);
        return;
    }

    emit_state(app, "processing", None);
    let app_clone = app.clone();
    let pr = pipeline_running.clone();
    tauri::async_runtime::spawn(async move {
        match crate::services::pipeline::run_pipeline(&app_clone, data).await {
            Ok(_) => {}
            Err(e) => log::error!("[hotkey] Pipeline error: {}", e),
        }
        emit_state(&app_clone, "idle", None);
        pr.store(false, Ordering::SeqCst);
    });
}

/// Cancel recording: stop the audio stream and discard everything.
fn cancel_recording(app: &tauri::AppHandle) {
    let app_state = app.state::<crate::AppState>();
    let mut recorder = app_state.audio_recorder.lock().unwrap();
    if recorder.is_recording() {
        // stop() returns audio data, but we discard it
        let _ = recorder.stop();
    }
    drop(recorder); // release mutex before play_sound acquires db mutex
    play_sound(Sound::Stop, app); // same stop sound for cancel per user decision
    emit_state(app, "idle", None);
}

/// Start audio recording with the configured device.
/// Returns the recording limit in seconds, or None on failure.
fn start_recording(app: &tauri::AppHandle) -> Option<u64> {
    let app_state = app.state::<crate::AppState>();

    let (device_name, limit_secs) = {
        let db = app_state.db.lock().unwrap();
        let device = db.get_setting("microphone_device");
        let limit: u64 = db
            .get_setting("recording_limit_seconds")
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);
        (device, limit)
    };

    let mut recorder = app_state.audio_recorder.lock().unwrap();
    if let Err(e) = recorder.start(device_name) {
        log::error!("Failed to start recording: {}", e);
        return None;
    }
    drop(recorder); // explicit drop to release mutex before play_sound acquires db mutex

    play_sound(Sound::Start, app);
    Some(limit_secs)
}

/// How long a press can last and still count as a "tap" for double-tap detection
const TAP_MAX_DURATION: std::time::Duration = std::time::Duration::from_millis(250);
/// Window after first tap release to detect second tap
const DOUBLE_TAP_WINDOW: std::time::Duration = std::time::Duration::from_millis(300);

pub fn register_hotkey(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<HotkeyEvent>();

    HOTKEY_TX
        .set(tx)
        .map_err(|_| "Hotkey channel already initialized")?;

    // Spawn OS thread with WH_KEYBOARD_LL hook + message loop
    #[cfg(windows)]
    spawn_hook_thread();

    // Channel for bar button actions (stop/cancel from frontend)
    let (bar_tx, mut bar_rx) = tokio::sync::mpsc::unbounded_channel::<BarAction>();

    // Store bar_tx in a global so the Tauri command can reach it
    BAR_ACTION_TX
        .set(bar_tx)
        .map_err(|_| "Bar action channel already initialized")?;

    // Spawn async task to process hotkey events with state machine
    let app_handle = app.handle().clone();
    let timer_handle: Arc<Mutex<Option<AsyncJoinHandle>>> = Arc::new(Mutex::new(None));

    tauri::async_runtime::spawn(async move {
        // State machine
        enum State {
            Idle,
            PendingHold,       // Pressed, waiting to see if it's a tap or hold
            WaitingSecondTap,  // First tap done, waiting for second tap
            HoldRecording,     // Recording in hold mode (release to stop)
            ContinuousRecording, // Recording in continuous mode (press again to stop)
        }

        let mut state = State::Idle;
        // Timer for tap/hold discrimination and double-tap window
        let mut tap_timer: Option<AsyncJoinHandle> = None;
        // Channel for tap timer expiry notifications
        let (timer_tx, mut timer_rx) = tokio::sync::mpsc::unbounded_channel::<&'static str>();
        // Guard to prevent double-spawning pipelines
        let pipeline_running = Arc::new(AtomicBool::new(false));

        loop {
            tokio::select! {
                hotkey_event = rx.recv() => {
                    let Some(event) = hotkey_event else { break };

                    match (&state, event) {
                        // --- IDLE ---
                        (State::Idle, HotkeyEvent::Pressed) => {
                            // EAGER START: begin recording immediately on key-down
                            if let Some(limit_secs) = start_recording(&app_handle) {
                                emit_state(&app_handle, "recording", Some("hold"));

                                // Spawn recording limit timer
                                let timer_handle_clone = timer_handle.clone();
                                let limit_tx = timer_tx.clone();
                                let handle = tauri::async_runtime::spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_secs(limit_secs)).await;
                                    log::info!("Recording limit reached ({}s), auto-stopping", limit_secs);
                                    {
                                        let mut th = timer_handle_clone.lock().unwrap();
                                        *th = None;
                                    }
                                    let _ = limit_tx.send("limit_expired");
                                });
                                {
                                    let mut th = timer_handle.lock().unwrap();
                                    *th = Some(handle);
                                }

                                // Start tap/hold discrimination timer
                                if let Some(t) = tap_timer.take() { t.abort(); }
                                let tx = timer_tx.clone();
                                tap_timer = Some(tauri::async_runtime::spawn(async move {
                                    tokio::time::sleep(TAP_MAX_DURATION).await;
                                    let _ = tx.send("hold_confirmed");
                                }));

                                state = State::PendingHold;
                            } else {
                                // start_recording() failed — stay idle
                                emit_state(&app_handle, "idle", None);
                            }
                        }
                        (State::Idle, HotkeyEvent::Released) => {
                            // Spurious release, ignore
                        }

                        // --- PENDING HOLD ---
                        (State::PendingHold, HotkeyEvent::Released) => {
                            // Released quickly = this was a tap. Wait for second tap.
                            // Recording KEEPS RUNNING — do NOT stop it.
                            if let Some(t) = tap_timer.take() { t.abort(); }
                            let tx = timer_tx.clone();
                            tap_timer = Some(tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(DOUBLE_TAP_WINDOW).await;
                                let _ = tx.send("single_tap_expired");
                            }));
                            state = State::WaitingSecondTap;
                        }
                        (State::PendingHold, HotkeyEvent::Pressed) => {
                            // Shouldn't happen (already pressed), ignore
                        }

                        // --- WAITING SECOND TAP ---
                        (State::WaitingSecondTap, HotkeyEvent::Pressed) => {
                            // Second tap detected → continuous mode!
                            // Recording already running from Idle→PendingHold.
                            // Recording limit timer already running too.
                            if let Some(t) = tap_timer.take() { t.abort(); }

                            // Switch UI mode from "hold" to "continuous"
                            emit_state(&app_handle, "recording", Some("continuous"));

                            state = State::ContinuousRecording;
                        }
                        (State::WaitingSecondTap, HotkeyEvent::Released) => {
                            // Release of second tap in continuous mode, or spurious — ignore
                            // (we already started recording on Pressed)
                        }

                        // --- HOLD RECORDING ---
                        (State::HoldRecording, HotkeyEvent::Released) => {
                            // Cancel recording limit timer
                            {
                                let mut th = timer_handle.lock().unwrap();
                                if let Some(handle) = th.take() {
                                    handle.abort();
                                }
                            }

                            let audio_data = stop_recording_sync(&app_handle);
                            state = State::Idle;
                            spawn_pipeline(&app_handle, audio_data, &pipeline_running);
                        }
                        (State::HoldRecording, HotkeyEvent::Pressed) => {
                            // Shouldn't happen (already holding), ignore
                        }

                        // --- CONTINUOUS RECORDING ---
                        (State::ContinuousRecording, HotkeyEvent::Pressed) => {
                            // Hotkey pressed again → stop and process
                            {
                                let mut th = timer_handle.lock().unwrap();
                                if let Some(handle) = th.take() {
                                    handle.abort();
                                }
                            }

                            let audio_data = stop_recording_sync(&app_handle);
                            state = State::Idle;
                            spawn_pipeline(&app_handle, audio_data, &pipeline_running);
                        }
                        (State::ContinuousRecording, HotkeyEvent::Released) => {
                            // Release after pressing to stop — already handled on Pressed, ignore
                        }
                    }
                }

                timer_event = timer_rx.recv() => {
                    let Some(event) = timer_event else { break };

                    match (&state, event) {
                        (State::PendingHold, "hold_confirmed") => {
                            // User held long enough → confirmed hold mode.
                            // Recording already running from Idle→PendingHold.
                            // Recording limit timer already running too.
                            // UI already showing "recording"/"hold" from key-down.
                            tap_timer = None;
                            state = State::HoldRecording;
                        }
                        (State::WaitingSecondTap, "single_tap_expired") => {
                            // No second tap came → cancel the eagerly-started recording
                            tap_timer = None;

                            // Abort the recording limit timer
                            {
                                let mut th = timer_handle.lock().unwrap();
                                if let Some(handle) = th.take() {
                                    handle.abort();
                                }
                            }

                            // Stop WASAPI stream and discard audio buffer
                            cancel_recording(&app_handle);
                            // cancel_recording already emits "idle" state
                            state = State::Idle;
                        }
                        (State::PendingHold, "limit_expired")
                        | (State::WaitingSecondTap, "limit_expired")
                        | (State::HoldRecording, "limit_expired")
                        | (State::ContinuousRecording, "limit_expired") => {
                            // Recording limit reached — stop and process
                            tap_timer = None;
                            let audio_data = stop_recording_sync(&app_handle);
                            state = State::Idle;
                            spawn_pipeline(&app_handle, audio_data, &pipeline_running);
                        }
                        _ => {
                            // Stale timer event from a previous state, ignore
                        }
                    }
                }

                bar_event = bar_rx.recv() => {
                    let Some(action) = bar_event else { break };

                    match (&state, action) {
                        (State::Idle, BarAction::Start) => {
                            // UI click → start continuous recording directly
                            if let Some(limit_secs) = start_recording(&app_handle) {
                                emit_state(&app_handle, "recording", Some("continuous"));

                                // Spawn recording limit timer
                                let timer_handle_clone = timer_handle.clone();
                                let limit_tx = timer_tx.clone();
                                let handle = tauri::async_runtime::spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_secs(limit_secs)).await;
                                    log::info!("Recording limit reached ({}s), auto-stopping", limit_secs);
                                    {
                                        let mut th = timer_handle_clone.lock().unwrap();
                                        *th = None;
                                    }
                                    let _ = limit_tx.send("limit_expired");
                                });
                                {
                                    let mut th = timer_handle.lock().unwrap();
                                    *th = Some(handle);
                                }

                                state = State::ContinuousRecording;
                            }
                        }
                        (State::ContinuousRecording, BarAction::Stop) => {
                            // Stop button → process
                            {
                                let mut th = timer_handle.lock().unwrap();
                                if let Some(handle) = th.take() {
                                    handle.abort();
                                }
                            }
                            let audio_data = stop_recording_sync(&app_handle);
                            state = State::Idle;
                            spawn_pipeline(&app_handle, audio_data, &pipeline_running);
                        }
                        (State::ContinuousRecording, BarAction::Cancel) => {
                            // Cancel button → discard
                            {
                                let mut th = timer_handle.lock().unwrap();
                                if let Some(handle) = th.take() {
                                    handle.abort();
                                }
                            }
                            cancel_recording(&app_handle);
                            state = State::Idle;
                        }
                        _ => {
                            // Bar action in wrong state, ignore
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

// Global channel for bar button actions
static BAR_ACTION_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<BarAction>> = OnceLock::new();

/// Called by the frontend pill click to start continuous recording
pub fn send_bar_start() {
    if let Some(tx) = BAR_ACTION_TX.get() {
        let _ = tx.send(BarAction::Start);
    }
}

/// Called by the frontend "Stop" button in continuous mode
pub fn send_bar_stop() {
    if let Some(tx) = BAR_ACTION_TX.get() {
        let _ = tx.send(BarAction::Stop);
    }
}

/// Called by the frontend "Cancel" button in continuous mode
pub fn send_bar_cancel() {
    if let Some(tx) = BAR_ACTION_TX.get() {
        let _ = tx.send(BarAction::Cancel);
    }
}
