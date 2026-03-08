use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

#[cfg(windows)]
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, MSLLHOOKSTRUCT, MSG, WH_MOUSE_LL,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
};

// --- Static atomics: lock-free state accessible from the hook thread ---

static HITBOX_LEFT: AtomicI32 = AtomicI32::new(0);
static HITBOX_TOP: AtomicI32 = AtomicI32::new(0);
static HITBOX_RIGHT: AtomicI32 = AtomicI32::new(0);
static HITBOX_BOTTOM: AtomicI32 = AtomicI32::new(0);
static HITBOX_ACTIVE: AtomicBool = AtomicBool::new(false);
static IS_INTERACTIVE: AtomicBool = AtomicBool::new(false);

enum CursorEvent {
    EnteredHitbox,
    LeftHitbox,
    /// Click intercepted inside hitbox — physical screen coordinates
    Clicked(i32, i32),
}

static MOUSE_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<CursorEvent>> = OnceLock::new();

/// Check if a point is inside the current hitbox bounds.
#[cfg(windows)]
fn is_inside_hitbox(sx: i32, sy: i32) -> bool {
    HITBOX_ACTIVE.load(Ordering::Relaxed)
        && sx >= HITBOX_LEFT.load(Ordering::Relaxed)
        && sx <= HITBOX_RIGHT.load(Ordering::Relaxed)
        && sy >= HITBOX_TOP.load(Ordering::Relaxed)
        && sy <= HITBOX_BOTTOM.load(Ordering::Relaxed)
}

// --- Mouse hook procedure (WH_MOUSE_LL) ---

/// Low-level mouse hook. Handles two concerns:
///
/// 1. **Hover tracking** (WM_MOUSEMOVE): toggles IS_INTERACTIVE on enter/exit.
/// 2. **Click interception** (WM_LBUTTONDOWN/UP): when cursor is inside the
///    hitbox, CONSUMES the click (returns non-zero) so it never reaches WebView2.
///    This prevents focus stealing entirely — the click is re-emitted as a Tauri
///    event for the frontend to handle synthetically.
#[cfg(windows)]
unsafe extern "system" fn mouse_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let sx = ms.pt.x;
        let sy = ms.pt.y;

        match msg {
            WM_MOUSEMOVE => {
                let was = IS_INTERACTIVE.load(Ordering::Relaxed);

                if HITBOX_ACTIVE.load(Ordering::Relaxed) {
                    let inside = is_inside_hitbox(sx, sy);

                    if inside && !was {
                        IS_INTERACTIVE.store(true, Ordering::Relaxed);
                        if let Some(tx) = MOUSE_TX.get() {
                            let _ = tx.send(CursorEvent::EnteredHitbox);
                        }
                    } else if !inside && was {
                        IS_INTERACTIVE.store(false, Ordering::Relaxed);
                        if let Some(tx) = MOUSE_TX.get() {
                            let _ = tx.send(CursorEvent::LeftHitbox);
                        }
                    }
                } else if was {
                    IS_INTERACTIVE.store(false, Ordering::Relaxed);
                    if let Some(tx) = MOUSE_TX.get() {
                        let _ = tx.send(CursorEvent::LeftHitbox);
                    }
                }
            }

            WM_LBUTTONDOWN => {
                if IS_INTERACTIVE.load(Ordering::Relaxed) && is_inside_hitbox(sx, sy) {
                    // CONSUME the click — never reaches WebView2, never steals focus
                    if let Some(tx) = MOUSE_TX.get() {
                        let _ = tx.send(CursorEvent::Clicked(sx, sy));
                    }
                    return LRESULT(1);
                }
            }

            WM_LBUTTONUP => {
                // Also consume button-up inside hitbox to prevent partial click delivery
                if IS_INTERACTIVE.load(Ordering::Relaxed) && is_inside_hitbox(sx, sy) {
                    return LRESULT(1);
                }
            }

            _ => {}
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

// --- Hook lifecycle ---

/// Spawn the WH_MOUSE_LL hook thread and async event processor.
pub fn start_mouse_hook(app_handle: AppHandle) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<CursorEvent>();

    MOUSE_TX.set(tx).expect("Mouse hook channel already set");

    // Spawn dedicated OS thread with WH_MOUSE_LL hook + message loop
    #[cfg(windows)]
    std::thread::Builder::new()
        .name("mouse-hook".into())
        .spawn(|| unsafe {
            let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0);
            if let Err(e) = &hook {
                log::error!("cursor_passthrough: failed to install mouse hook: {e}");
                return;
            }
            // Message loop — required for WH_MOUSE_LL to work
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
        })
        .expect("Failed to spawn mouse hook thread");

    // Spawn async task to process cursor events (set_ignore_cursor_events + emit)
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CursorEvent::EnteredHitbox => {
                    // Window stays always click-through — hover is driven
                    // by this event, not by DOM mouse events.
                    let _ = app_handle.emit("pill-cursor-entered", ());
                }
                CursorEvent::LeftHitbox => {
                    let _ = app_handle.emit("pill-cursor-left", ());
                }
                CursorEvent::Clicked(sx, sy) => {
                    let _ = app_handle.emit(
                        "hitbox-click",
                        serde_json::json!({ "screenX": sx, "screenY": sy }),
                    );
                }
            }
        }
    });
}

// --- Public API for Tauri commands ---

/// Update the pill hitbox in physical screen pixels.
pub fn update_hitbox(left: i32, top: i32, right: i32, bottom: i32) {
    HITBOX_LEFT.store(left, Ordering::Relaxed);
    HITBOX_TOP.store(top, Ordering::Relaxed);
    HITBOX_RIGHT.store(right, Ordering::Relaxed);
    HITBOX_BOTTOM.store(bottom, Ordering::Relaxed);
    HITBOX_ACTIVE.store(true, Ordering::Relaxed);
}

/// Clear the hitbox (pill hidden) and reset interactive state.
pub fn clear_hitbox() {
    HITBOX_ACTIVE.store(false, Ordering::Relaxed);
    IS_INTERACTIVE.store(false, Ordering::Relaxed);
}
