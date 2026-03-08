use super::prompts::build_cleanup_prompt;
use super::{LLMError, LLMProcessor, ProcessingContext};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub struct OpenAIGPT {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_completion_tokens: u32,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
    json_schema: JsonSchemaSpec,
}

#[derive(Serialize)]
struct JsonSchemaSpec {
    name: String,
    strict: bool,
    schema: serde_json::Value,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct CleanedTextResponse {
    text: String,
}

impl OpenAIGPT {
    pub fn with_client(api_key: String, model: String, client: reqwest::Client) -> Self {
        Self {
            api_key,
            model,
            client,
        }
    }
}

#[async_trait]
impl LLMProcessor for OpenAIGPT {
    async fn process(
        &self,
        raw_text: &str,
        context: &ProcessingContext,
    ) -> Result<String, LLMError> {
        let system_prompt = build_cleanup_prompt(&context.languages);
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                Message {
                    role: "user".to_string(),
                    content: format!("[TRANSCRIPT_START]{raw_text}[TRANSCRIPT_END]"),
                },
            ],
            temperature: 0.0,
            max_completion_tokens: 4096,
            response_format: ResponseFormat {
                format_type: "json_schema".to_string(),
                json_schema: JsonSchemaSpec {
                    name: "cleaned_text".to_string(),
                    strict: true,
                    schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "text": { "type": "string" }
                        },
                        "required": ["text"],
                        "additionalProperties": false
                    }),
                },
            },
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LLMError::Timeout
                } else {
                    LLMError::HttpError(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(LLMError::ApiError(format!("Status {}: {}", status, body)));
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| LLMError::ApiError(e.to_string()))?;

        let raw_content = chat_response
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .ok_or_else(|| LLMError::ApiError("No response from model".to_string()))?;

        match serde_json::from_str::<CleanedTextResponse>(&raw_content) {
            Ok(parsed) => Ok(parsed.text),
            Err(_) => Ok(raw_content),
        }
    }
}
