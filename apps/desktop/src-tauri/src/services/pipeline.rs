use std::time::Instant;
use tauri::Manager;

use super::db::keystore;
use super::injector;
use super::llm::openai_gpt::OpenAIGPT;
use super::llm::{LLMProcessor, ProcessingContext};
use super::transcription::openai_api::OpenAIWhisper;
use super::transcription::{TranscriptionConfig, TranscriptionEngine};

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("No API key configured")]
    NoApiKey,
    #[error("Transcription failed: {0}")]
    TranscriptionFailed(String),
    #[error("Injection failed: {0}")]
    InjectionFailed(String),
}

pub async fn run_pipeline(
    app: &tauri::AppHandle,
    audio_data: Vec<u8>,
) -> Result<(), PipelineError> {
    let pipeline_start = Instant::now();
    let audio_size_kb = audio_data.len() as f64 / 1024.0;
    log::info!(
        "[pipeline] Starting pipeline ({:.1} KB audio)",
        audio_size_kb
    );

    // 1. Get API key
    let t = Instant::now();
    let api_key = keystore::get_api_key().map_err(|e| {
        log::error!("API key retrieval failed: {}", e);
        PipelineError::NoApiKey
    })?;
    log::info!("[pipeline] API key retrieved in {:?}", t.elapsed());

    // 2. Get settings + shared HTTP client
    let t = Instant::now();
    let (languages, paste_delay_ms, llm_model, http_client) = {
        let state = app.state::<crate::AppState>();
        let db = state.db.lock().unwrap();

        // Read languages JSON array from DB (LANG-01)
        let languages_json = db
            .get_setting("languages")
            .unwrap_or_else(|| r#"["en"]"#.to_string());
        let languages: Vec<String> = serde_json::from_str(&languages_json).unwrap_or_else(|_| {
            log::warn!(
                "[pipeline] Failed to parse languages JSON '{}', falling back to [\"en\"]",
                languages_json
            );
            vec!["en".to_string()]
        });

        let paste_delay: u64 = db
            .get_setting("paste_delay_ms")
            .and_then(|v| v.parse().ok())
            .unwrap_or(150);
        let model = db
            .get_setting("llm_model")
            .unwrap_or_else(|| "gpt-4.1-nano".to_string());
        let client = state.http_client.clone();
        (languages, paste_delay, model, client)
    };
    log::info!(
        "[pipeline] Settings loaded in {:?} (languages={:?}, model={})",
        t.elapsed(),
        languages,
        llm_model
    );

    // 3. Transcribe audio
    let t = Instant::now();
    let whisper = OpenAIWhisper::with_client(api_key.clone(), http_client.clone());
    let config = TranscriptionConfig {
        // PIPE-01: 1 real language -> send code; PIPE-02: 2+ OR ["auto"] sentinel -> None (auto-detect)
        language: if languages.len() == 1 && languages[0] != "auto" {
            Some(languages[0].clone())
        } else {
            None
        },
        prompt: None,
    };

    let transcription = whisper
        .transcribe(&audio_data, &config)
        .await
        .map_err(|e| PipelineError::TranscriptionFailed(e.to_string()))?;

    let raw_text = transcription.text;
    log::info!(
        "[pipeline] Transcription done in {:?} ({} chars)",
        t.elapsed(),
        raw_text.len()
    );

    if raw_text.trim().is_empty() {
        log::info!(
            "[pipeline] Empty transcription, skipping. Total: {:?}",
            pipeline_start.elapsed()
        );
        return Ok(());
    }

    // 4. Clean with LLM (fallback to raw text on failure)
    let t = Instant::now();
    let gpt = OpenAIGPT::with_client(api_key, llm_model, http_client);
    let context = ProcessingContext {
        languages: languages.clone(),
    };

    let cleaned_text = match gpt.process(&raw_text, &context).await {
        Ok(cleaned) => {
            log::info!(
                "[pipeline] LLM cleanup done in {:?} ({} chars)",
                t.elapsed(),
                cleaned.len()
            );
            cleaned
        }
        Err(e) => {
            log::warn!(
                "[pipeline] LLM failed in {:?}: {}, using raw text",
                t.elapsed(),
                e
            );
            raw_text.clone()
        }
    };

    // 5. Inject text
    let t = Instant::now();
    injector::inject_text(&cleaned_text, paste_delay_ms)
        .await
        .map_err(|e| PipelineError::InjectionFailed(e.to_string()))?;
    log::info!("[pipeline] Text injection done in {:?}", t.elapsed());

    // 6. Save to history
    let t = Instant::now();
    {
        let state = app.state::<crate::AppState>();
        let db = state.db.lock().unwrap();
        if let Err(e) = db.save_history(&raw_text, &cleaned_text, &languages.join(","), None) {
            log::warn!("Failed to save history: {}", e);
        }
    }
    log::info!("[pipeline] History saved in {:?}", t.elapsed());

    log::info!(
        "[pipeline] TOTAL pipeline time: {:?}",
        pipeline_start.elapsed()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    // PIPE-01 + sentinel: ["auto"] must produce None, not Some("auto")
    #[test]
    fn test_auto_sentinel_produces_none() {
        let languages = vec!["auto".to_string()];
        let language_param: Option<String> = if languages.len() == 1 && languages[0] != "auto" {
            Some(languages[0].clone())
        } else {
            None
        };
        assert_eq!(
            language_param, None,
            "[\"auto\"] sentinel must produce None so Whisper uses auto-detect, not Some(\"auto\")"
        );
    }

    // PIPE-01: single real language still produces Some(code)
    #[test]
    fn test_single_real_language_produces_some_code() {
        let languages = vec!["es".to_string()];
        let language_param: Option<String> = if languages.len() == 1 && languages[0] != "auto" {
            Some(languages[0].clone())
        } else {
            None
        };
        assert_eq!(
            language_param,
            Some("es".to_string()),
            "single real language should still send Some(\"es\") to Whisper"
        );
    }

    // PIPE-02: two languages still produce None
    #[test]
    fn test_two_real_languages_produce_none() {
        let languages = vec!["es".to_string(), "en".to_string()];
        let language_param: Option<String> = if languages.len() == 1 && languages[0] != "auto" {
            Some(languages[0].clone())
        } else {
            None
        };
        assert_eq!(
            language_param, None,
            "two languages should still produce None so Whisper uses auto-detect"
        );
    }
}
