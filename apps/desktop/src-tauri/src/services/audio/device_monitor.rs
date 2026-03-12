use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

// Required for the #[implement] proc macro to resolve ::windows_core crate name
#[cfg(windows)]
extern crate windows_core;

/// Shared state between the device monitor thread and the rest of the app.
/// Fields are overwritten on each callback — never accumulated.
pub struct DeviceMonitorState {
    /// The endpoint ID of the last activated/default capture device.
    pub last_device_id: Mutex<Option<String>>,
    /// The friendly name of that device (for UI display).
    pub last_device_name: Mutex<String>,
    /// Flag set to true when any device change occurs. Frontend checks + clears.
    pub devices_changed: AtomicBool,
}

impl DeviceMonitorState {
    pub fn new() -> Self {
        Self {
            last_device_id: Mutex::new(None),
            last_device_name: Mutex::new(String::new()),
            devices_changed: AtomicBool::new(false),
        }
    }

    /// Read the current auto-detect device ID.
    pub fn get_device_id(&self) -> Option<String> {
        self.last_device_id.lock().unwrap().clone()
    }

    /// Read the current auto-detect device friendly name.
    pub fn get_device_name(&self) -> String {
        self.last_device_name.lock().unwrap().clone()
    }

    /// Check and clear the devices_changed flag.
    pub fn take_devices_changed(&self) -> bool {
        self.devices_changed.swap(false, Ordering::AcqRel)
    }
}

/// How the capture thread should resolve which device to use.
#[derive(Clone, Debug)]
pub enum DeviceSelection {
    /// Use the device tracked by the monitor (or fallback to system default).
    AutoDetect(Option<String>), // Option<endpoint_id>
    /// Use a specific device matched by friendly name.
    ByName(String),
}

// ─── Windows-specific implementation ─────────────────────────────────────────

#[cfg(windows)]
use windows::Win32::Foundation::PROPERTYKEY;
#[cfg(windows)]
use windows::Win32::Media::Audio::{
    eCapture, eConsole, EDataFlow, ERole, IMMDeviceEnumerator,
    IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, DEVICE_STATE,
};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED,
};
#[cfg(windows)]
use windows::Win32::System::Com::StructuredStorage;
#[cfg(windows)]
use windows::Win32::System::Com::STGM_READ;
#[cfg(windows)]
use windows::core::{implement, GUID, PCWSTR, Result};

#[cfg(windows)]
const PKEY_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

/// COM callback implementation for device change notifications.
#[cfg(windows)]
#[implement(IMMNotificationClient)]
struct DeviceChangeNotifier {
    state: Arc<DeviceMonitorState>,
    app_handle: tauri::AppHandle,
}

#[cfg(windows)]
impl IMMNotificationClient_Impl for DeviceChangeNotifier_Impl {
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        pwstrdefaultdeviceid: &PCWSTR,
    ) -> Result<()> {
        // Follow eConsole only — the "Dispositivo predeterminado" in Windows Sound Settings.
        // Same approach as Whispr Flow: auto-detect = Windows default input device.
        if flow != eCapture || role != eConsole {
            return Ok(());
        }

        let id = if pwstrdefaultdeviceid.is_null() {
            None
        } else {
            unsafe { pwstrdefaultdeviceid.to_string().ok() }
        };

        log::debug!(
            "[DeviceMonitor] OnDefaultDeviceChanged: eConsole → {:?}",
            id
        );

        if let Some(ref id_str) = id {
            *self.state.last_device_id.lock().unwrap() = Some(id_str.clone());
            if let Ok(name) = resolve_friendly_name_by_id(id_str) {
                *self.state.last_device_name.lock().unwrap() = name;
            }
        }
        self.state.devices_changed.store(true, Ordering::Release);
        let _ = self.app_handle.emit("audio-devices-changed", ());
        Ok(())
    }

    fn OnDeviceStateChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _dwnewstate: DEVICE_STATE,
    ) -> Result<()> {
        // Intentionally no log — Windows fires 10-20 of these per physical device change.
        // OnDefaultDeviceChanged (logged above) is the actionable event.
        self.state.devices_changed.store(true, Ordering::Release);
        let _ = self.app_handle.emit("audio-devices-changed", ());
        Ok(())
    }

    fn OnDeviceAdded(&self, pwstrdeviceid: &PCWSTR) -> Result<()> {
        let id = unsafe { pwstrdeviceid.to_string().unwrap_or_default() };
        log::debug!("[DeviceMonitor] OnDeviceAdded: {}", id);
        self.state.devices_changed.store(true, Ordering::Release);
        let _ = self.app_handle.emit("audio-devices-changed", ());
        Ok(())
    }

    fn OnDeviceRemoved(&self, pwstrdeviceid: &PCWSTR) -> Result<()> {
        let id = unsafe { pwstrdeviceid.to_string().unwrap_or_default() };
        log::debug!("[DeviceMonitor] OnDeviceRemoved: {}", id);
        self.state.devices_changed.store(true, Ordering::Release);
        let _ = self.app_handle.emit("audio-devices-changed", ());
        Ok(())
    }

    fn OnPropertyValueChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _key: &PROPERTYKEY,
    ) -> Result<()> {
        // Ignore property changes (volume, etc.)
        Ok(())
    }
}

/// RAII guard: unregisters the callback before the COM object is dropped.
/// Prevents use-after-free crash (windows-rs #2487).
#[cfg(windows)]
struct NotificationGuard {
    enumerator: IMMDeviceEnumerator,
    callback: IMMNotificationClient,
}

#[cfg(windows)]
impl Drop for NotificationGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .enumerator
                .UnregisterEndpointNotificationCallback(&self.callback);
        }
        log::debug!("[DeviceMonitor] Unregistered notification callback");
    }
}

/// RAII guard for CoUninitialize.
#[cfg(windows)]
struct ComUninitGuard;

#[cfg(windows)]
impl Drop for ComUninitGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

/// Resolve a device's friendly name from its endpoint ID.
#[cfg(windows)]
fn resolve_friendly_name_by_id(device_id: &str) -> std::result::Result<String, String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("CoCreateInstance: {}", e))?;

        let wide_id: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let pcwstr = PCWSTR(wide_id.as_ptr());

        let device = enumerator
            .GetDevice(pcwstr)
            .map_err(|e| format!("GetDevice: {}", e))?;

        let store = device
            .OpenPropertyStore(STGM_READ)
            .map_err(|e| format!("OpenPropertyStore: {}", e))?;

        let prop = store
            .GetValue(&PKEY_FRIENDLY_NAME)
            .map_err(|e| format!("GetValue: {}", e))?;

        let pwstr = StructuredStorage::PropVariantToStringAlloc(&prop)
            .map_err(|e| format!("PropVariantToStringAlloc: {}", e))?;

        let name = pwstr.to_string().unwrap_or_default();
        CoTaskMemFree(Some(pwstr.as_ptr() as *const std::ffi::c_void));
        Ok(name)
    }
}

/// Read the current default capture device's ID and friendly name.
/// Uses eConsole — the "Dispositivo predeterminado" in Windows Sound Settings.
#[cfg(windows)]
fn read_initial_default(enumerator: &IMMDeviceEnumerator) -> (Option<String>, String) {
    unsafe {
        let device = enumerator
            .GetDefaultAudioEndpoint(eCapture, eConsole)
            .ok();

        if let Some(dev) = device {
            let id = dev.GetId().ok().and_then(|pwstr| {
                let s = pwstr.to_string().unwrap_or_default();
                CoTaskMemFree(Some(pwstr.as_ptr() as *const std::ffi::c_void));
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            });

            let name = dev
                .OpenPropertyStore(STGM_READ)
                .ok()
                .and_then(|store| store.GetValue(&PKEY_FRIENDLY_NAME).ok())
                .and_then(|prop| StructuredStorage::PropVariantToStringAlloc(&prop).ok())
                .map(|pwstr| {
                    let n = pwstr.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(pwstr.as_ptr() as *const std::ffi::c_void));
                    n
                })
                .unwrap_or_default();

            (id, name)
        } else {
            (None, String::new())
        }
    }
}

/// Spawn the device monitor thread. Returns an mpsc::Sender<()> for shutdown signaling.
/// Caller stores this in AppState. Sending `()` or dropping the sender signals shutdown.
#[cfg(windows)]
pub fn spawn_monitor(
    state: Arc<DeviceMonitorState>,
    app_handle: tauri::AppHandle,
) -> std::sync::mpsc::Sender<()> {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<()>();

    std::thread::Builder::new()
        .name("device-monitor".into())
        .spawn(move || {
            if let Err(e) = monitor_thread_main(state, app_handle, cmd_rx) {
                log::error!("[DeviceMonitor] Thread fatal error: {}", e);
            }
        })
        .expect("Failed to spawn device-monitor thread");

    cmd_tx
}

#[cfg(windows)]
fn monitor_thread_main(
    state: Arc<DeviceMonitorState>,
    app_handle: tauri::AppHandle,
    shutdown_rx: std::sync::mpsc::Receiver<()>,
) -> std::result::Result<(), String> {
    unsafe {
        // MTA — callbacks arrive freely on RPC threads (no message pump needed)
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|e| format!("CoInitializeEx: {}", e))?;
        let _com_guard = ComUninitGuard;

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("CoCreateInstance: {}", e))?;

        // Read initial default device
        let (initial_id, initial_name) = read_initial_default(&enumerator);
        log::info!(
            "[DeviceMonitor] Initial device: {:?} ({})",
            initial_id,
            initial_name
        );
        *state.last_device_id.lock().unwrap() = initial_id;
        *state.last_device_name.lock().unwrap() = initial_name;

        // Create and register the notification callback
        let notifier = DeviceChangeNotifier {
            state: state.clone(),
            app_handle,
        };
        let callback: IMMNotificationClient = notifier.into();

        enumerator
            .RegisterEndpointNotificationCallback(&callback)
            .map_err(|e| format!("RegisterEndpointNotificationCallback: {}", e))?;

        // RAII guard ensures Unregister happens before callback is dropped
        let _guard = NotificationGuard {
            enumerator,
            callback,
        };

        log::info!("[DeviceMonitor] Registered and waiting for events");

        // Block until shutdown signal or channel disconnect (0% CPU)
        let _ = shutdown_rx.recv();
        log::info!("[DeviceMonitor] Shutting down");

        // _guard drops here → UnregisterEndpointNotificationCallback
        // _com_guard drops here → CoUninitialize
    }
    Ok(())
}
