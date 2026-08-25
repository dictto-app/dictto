use std::time::Instant;
use tauri::Manager;

use super::db::keystore;
use super::injector;
use super::llm::openai_gpt::OpenAIGPT;
use super::llm::{LLMProcessor, ProcessingContext};
use super::transcription::openai_api::OpenAIWhisper;
use super::transcription::{TranscriptionConfig, TranscriptionEngine};

#[cfg(feature = "local-transcription")]
use super::transcription::parakeet_local::ParakeetLocal;

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

    // 1. Get settings + shared HTTP client
    let t = Instant::now();
    let (languages, paste_delay_ms, llm_model, http_client, transcription_engine, parakeet_model_dir) = {
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
        // TRANS-ENGINE-01: "openai_api" (default, cloud, requires API key) or
        // "parakeet_local" (on-device, offline, requires a downloaded ONNX model —
        // see README "Local transcription" section).
        let engine = db
            .get_setting("transcription_engine")
            .unwrap_or_else(|| "openai_api".to_string());
        let model_dir = db
            .get_setting("parakeet_model_dir")
            .filter(|s| !s.trim().is_empty());
        let client = state.http_client.clone();
        (languages, paste_delay, model, client, engine, model_dir)
    };
    log::info!(
        "[pipeline] Settings loaded in {:?} (languages={:?}, model={}, transcription_engine={})",
        t.elapsed(),
        languages,
        llm_model,
        transcription_engine
    );

    let language_param = if languages.len() == 1 && languages[0] != "auto" {
        Some(languages[0].clone())
    } else {
        None
    };

    // 2. Transcribe audio — engine chosen by the "transcription_engine" setting.
    // Local (Parakeet) transcription needs no API key at all; the cloud engine
    // (OpenAI Whisper) does, so we only fetch the key on that path.
    let t = Instant::now();
    let raw_text = match transcription_engine.as_str() {
        #[cfg(feature = "local-transcription")]
        "parakeet_local" => {
            let model_dir = parakeet_model_dir.ok_or_else(|| {
                log::error!("[pipeline] transcription_engine=parakeet_local but no parakeet_model_dir configured");
                PipelineError::TranscriptionFailed(
                    "No local Parakeet model directory configured".into(),
                )
            })?;

            let parakeet = ParakeetLocal::from_model_dir(model_dir)
                .map_err(|e| PipelineError::TranscriptionFailed(e.to_string()))?;
            let config = TranscriptionConfig {
                language: language_param.clone(),
                prompt: None,
            };
            parakeet
                .transcribe(&audio_data, &config)
                .await
                .map_err(|e| PipelineError::TranscriptionFailed(e.to_string()))?
                .text
        }
        _ => {
            // Default / fallback: OpenAI Whisper API (cloud, requires API key).
            let api_key = keystore::get_api_key().map_err(|e| {
                log::error!("API key retrieval failed: {}", e);
                PipelineError::NoApiKey
            })?;
            let whisper = OpenAIWhisper::with_client(api_key, http_client.clone());
            let config = TranscriptionConfig {
                // PIPE-01: 1 real language -> send code; PIPE-02: 2+ OR ["auto"] sentinel -> None (auto-detect)
                language: language_param,
                prompt: None,
            };
            whisper
                .transcribe(&audio_data, &config)
                .await
                .map_err(|e| PipelineError::TranscriptionFailed(e.to_string()))?
                .text
        }
    };
    log::info!(
        "[pipeline] Transcription done in {:?} ({} chars, engine={})",
        t.elapsed(),
        raw_text.len(),
        transcription_engine
    );

    if raw_text.trim().is_empty() {
        log::info!(
            "[pipeline] Empty transcription, skipping. Total: {:?}",
            pipeline_start.elapsed()
        );
        return Ok(());
    }

    // 3. Clean with LLM (fallback to raw text on failure or missing key).
    // LLM cleanup still goes through OpenAI GPT regardless of transcription
    // engine — if no API key is configured (e.g. a fully local/offline setup),
    // we skip cleanup gracefully and paste the raw transcript instead of
    // hard-failing the whole pipeline.
    let t = Instant::now();
    let cleaned_text = match keystore::get_api_key() {
        Ok(api_key) => {
            let gpt = OpenAIGPT::with_client(api_key, llm_model, http_client);
            let context = ProcessingContext {
                languages: languages.clone(),
            };
            match gpt.process(&raw_text, &context).await {
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
            }
        }
        Err(_) => {
            log::info!(
                "[pipeline] No API key configured, skipping LLM cleanup, using raw text ({:?})",
                t.elapsed()
            );
            raw_text.clone()
        }
    };

    // 4. Inject text
    let t = Instant::now();
    injector::inject_text(&cleaned_text, paste_delay_ms)
        .await
        .map_err(|e| PipelineError::InjectionFailed(e.to_string()))?;
    log::info!("[pipeline] Text injection done in {:?}", t.elapsed());

    // 5. Save to history
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
