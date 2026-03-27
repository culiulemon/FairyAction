use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::messages::Message;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },
    #[error("Rate limited, retry after {retry_after}s")]
    RateLimited { retry_after: f64 },
    #[error("Request timeout")]
    Timeout,
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Authentication failed: {0}")]
    AuthError(String),
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletion {
    pub content: String,
    pub structured_output: Option<serde_json::Value>,
    pub usage: Option<Usage>,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[async_trait]
pub trait ChatModel: Send + Sync {
    fn provider(&self) -> &str;
    fn model(&self) -> &str;
    async fn invoke(
        &self,
        messages: Vec<Message>,
        output_format: Option<&str>,
    ) -> Result<ChatCompletion, LlmError>;
}
