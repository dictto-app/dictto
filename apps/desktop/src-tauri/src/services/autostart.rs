use winreg::enums::*;
use winreg::RegKey;

const APP_NAME: &str = "Dictto";
const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";

pub fn enable_autostart(exe_path: &str) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(RUN_KEY)
        .map_err(|e| format!("Failed to open registry key: {}", e))?;
    let value = format!("\"{}\" --autostart", exe_path);
    key.set_value(APP_NAME, &value)
        .map_err(|e| format!("Failed to set registry value: {}", e))
}

pub fn disable_autostart() -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_WRITE)
        .map_err(|e| format!("Failed to open registry key: {}", e))?;
    key.delete_value(APP_NAME)
        .map_err(|e| format!("Failed to delete registry value: {}", e))
}

#[allow(dead_code)]
pub fn is_autostart_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(RUN_KEY) {
        key.get_value::<String, _>(APP_NAME).is_ok()
    } else {
        false
    }
}
