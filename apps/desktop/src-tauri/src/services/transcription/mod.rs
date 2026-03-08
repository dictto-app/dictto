pub mod openai_api;

use async_trait::async_trait;

pub struct TranscriptionConfig {
    pub language: Option<String>,
    pub prompt: Option<String>,
}

pub struct TranscriptionResult {
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    #[error("HTTP error: {0}")]
    HttpError(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Timeout")]
    Timeout,
}

#[async_trait]
pub trait TranscriptionEngine: Send + Sync {
    async fn transcribe(
        &self,
        audio_data: &[u8],
        config: &TranscriptionConfig,
    ) -> Result<TranscriptionResult, TranscriptionError>;
}
