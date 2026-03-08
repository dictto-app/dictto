const SERVICE_NAME: &str = "dictto";
const OPENAI_KEY_NAME: &str = "openai_api_key";

pub fn set_api_key(api_key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, OPENAI_KEY_NAME)
        .map_err(|e| format!("Keyring error: {}", e))?;
    entry
        .set_password(api_key)
        .map_err(|e| format!("Failed to save API key: {}", e))?;

    // Verify the key was actually stored
    match entry.get_password() {
        Ok(stored) if stored == api_key => Ok(()),
        Ok(_) => Err("API key verification failed: stored value doesn't match".to_string()),
        Err(e) => Err(format!(
            "API key was not persisted in Windows Credential Manager: {}",
            e
        )),
    }
}

pub fn get_api_key() -> Result<String, String> {
    let entry = keyring::Entry::new(SERVICE_NAME, OPENAI_KEY_NAME)
        .map_err(|e| format!("Keyring error: {}", e))?;
    entry
        .get_password()
        .map_err(|e| format!("API key not found: {}", e))
}

pub fn delete_api_key() -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, OPENAI_KEY_NAME)
        .map_err(|e| format!("Keyring error: {}", e))?;
    entry
        .delete_credential()
        .map_err(|e| format!("Failed to delete API key: {}", e))
}
