pub fn default_settings() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hotkey", "Ctrl+Win"),
        ("transcription_engine", "openai_api"),
        ("llm_provider", "openai_gpt"),
        ("llm_model", "gpt-4.1-nano"),
        ("auto_start", "true"),
        ("bar_visible_idle", "true"),
        ("bar_opacity", "0.9"),
        ("paste_delay_ms", "150"),
        ("microphone_device", "default"),
        ("recording_limit_seconds", "300"),
        ("sound_effects_enabled", "true"),
    ]
}
