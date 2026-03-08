!macro NSIS_HOOK_PREUNINSTALL
  ; Delete Dictto API key from Windows Credential Manager
  ; Target: "openai_api_key.dictto" (keyring crate v3 format: {user}.{service})
  ; CRED_TYPE_GENERIC = 1, Flags = 0
  ; Returns TRUE (1) on success, FALSE (0) if not found — silent either way
  ; NOTE: Target name is coupled to keyring v3 windows-native backend format.
  ; If upgrading keyring to v4+, verify the target name hasn't changed.
  System::Call 'advapi32::CredDeleteW(w "openai_api_key.dictto", i 1, i 0)'
!macroend
