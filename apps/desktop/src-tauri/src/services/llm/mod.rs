pub mod openai_gpt;
pub mod prompts;

use async_trait::async_trait;

pub struct ProcessingContext {
    pub languages: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LLMError {
    #[error("HTTP error: {0}")]
    HttpError(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Timeout")]
    Timeout,
}

#[async_trait]
pub trait LLMProcessor: Send + Sync {
    async fn process(
        &self,
        raw_text: &str,
        context: &ProcessingContext,
    ) -> Result<String, LLMError>;
}
