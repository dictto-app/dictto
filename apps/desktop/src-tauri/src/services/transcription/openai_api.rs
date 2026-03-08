use async_trait::async_trait;
use reqwest::multipart;
use serde::Deserialize;
use std::time::Duration;
use super::{TranscriptionConfig, TranscriptionEngine, TranscriptionError, TranscriptionResult};

pub struct OpenAIWhisper {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct WhisperResponse {
    text: String,
}

impl OpenAIWhisper {
    pub fn with_client(api_key: String, client: reqwest::Client) -> Self {
        Self { api_key, client }
    }
}

const TRANSCRIPTION_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_ATTEMPTS: u32 = 2;
const RETRY_DELAY: Duration = Duration::from_secs(2);

#[async_trait]
impl TranscriptionEngine for OpenAIWhisper {
    async fn transcribe(
        &self,
        audio_data: &[u8],
        config: &TranscriptionConfig,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        let mut last_error = TranscriptionError::Timeout;

        for attempt in 1..=MAX_ATTEMPTS {
            let file_part = multipart::Part::bytes(audio_data.to_vec())
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| TranscriptionError::HttpError(e.to_string()))?;

            let mut form = multipart::Form::new()
                .text("model", "gpt-4o-mini-transcribe")
                .part("file", file_part);

            if let Some(ref lang) = config.language {
                form = form.text("language", lang.clone());
            }

            if let Some(ref prompt) = config.prompt {
                form = form.text("prompt", prompt.clone());
            }

            let response = self
                .client
                .post("https://api.openai.com/v1/audio/transcriptions")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .multipart(form)
                .timeout(TRANSCRIPTION_TIMEOUT)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        return Err(TranscriptionError::ApiError(format!(
                            "Status {}: {}",
                            status, body
                        )));
                    }

                    let whisper_response: WhisperResponse = resp
                        .json()
                        .await
                        .map_err(|e| TranscriptionError::ApiError(e.to_string()))?;

                    return Ok(TranscriptionResult {
                        text: whisper_response.text,
                    });
                }
                Err(e) if e.is_timeout() && attempt < MAX_ATTEMPTS => {
                    log::debug!(
                        "[transcription] Attempt {}/{} timed out, retrying in {}s...",
                        attempt,
                        MAX_ATTEMPTS,
                        RETRY_DELAY.as_secs()
                    );
                    last_error = TranscriptionError::Timeout;
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Err(e) if e.is_timeout() => {
                    log::debug!(
                        "[transcription] Attempt {}/{} timed out, no retries left",
                        attempt, MAX_ATTEMPTS
                    );
                    return Err(TranscriptionError::Timeout);
                }
                Err(e) => {
                    return Err(TranscriptionError::HttpError(e.to_string()));
                }
            }
        }

        Err(last_error)
    }
}
