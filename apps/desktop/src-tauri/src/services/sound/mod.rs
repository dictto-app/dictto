use tauri::Manager;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
use windows::core::PCWSTR;

#[derive(Debug)]
pub enum Sound {
    Start,
    Stop,
}

static START_WAV: &[u8] = include_bytes!("../../../assets/sounds/start.wav");
static STOP_WAV: &[u8] = include_bytes!("../../../assets/sounds/stop.wav");

/// Play a sound effect if enabled in settings. Non-blocking fire-and-forget.
///
/// Uses Win32 PlaySound with SND_MEMORY|SND_ASYNC for native OS audio pipeline —
/// identical quality to the system media player, zero resampling.
/// Static include_bytes! data never gets freed, so SND_MEMORY|SND_ASYNC is safe.
pub fn play_sound(sound: Sound, app: &tauri::AppHandle) {
    let enabled = {
        let state = app.state::<crate::AppState>();
        let db = state.db.lock().unwrap();
        db.get_setting("sound_effects_enabled")
            .map(|v| v == "true")
            .unwrap_or(true)
    };

    if !enabled {
        return;
    }

    let bytes: &[u8] = match &sound {
        Sound::Start => START_WAV,
        Sound::Stop => STOP_WAV,
    };

    let _ = unsafe {
        let ptr = PCWSTR(bytes.as_ptr() as *const u16);
        PlaySoundW(ptr, None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT)
    };
}
