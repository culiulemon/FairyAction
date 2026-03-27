use thiserror::Error;

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("API request failed: {0}")]
    ApiError(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Request timeout after {0}s")]
    Timeout(u64),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Authentication error: missing or invalid API key")]
    AuthError,

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}
