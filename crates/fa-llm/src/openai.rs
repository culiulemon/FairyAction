use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use crate::base::{ChatCompletion, ChatModel, LlmError, Usage};
use crate::messages::Message;

pub struct ChatOpenAI {
    client: Client,
    model: String,
    api_key: String,
    base_url: String,
    max_tokens: u32,
    temperature: f32,
}

impl ChatOpenAI {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            model: model.into(),
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".to_string(),
            max_tokens: 4096,
            temperature: 0.0,
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    fn handle_http_error(status: u16, body: &str) -> LlmError {
        if status == 401 || status == 403 {
            LlmError::AuthError(body.to_string())
        } else if status == 429 {
            LlmError::RateLimited { retry_after: 1.0 }
        } else if status == 404 {
            LlmError::ModelNotFound(body.to_string())
        } else {
            LlmError::ApiError { status, message: body.to_string() }
        }
    }
}

#[async_trait]
impl ChatModel for ChatOpenAI {
    fn provider(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn invoke(
        &self,
        messages: Vec<Message>,
        output_format: Option<&str>,
    ) -> Result<ChatCompletion, LlmError> {
        let openai_messages: Vec<serde_json::Value> =
            messages.iter().map(|m| m.to_openai_value()).collect();

        let mut body = json!({
            "model": self.model,
            "messages": openai_messages,
            "max_tokens": self.max_tokens,
            "temperature": self.temperature,
        });

        if let Some(schema_str) = output_format {
            let schema: serde_json::Value = serde_json::from_str(schema_str)
                .map_err(|e| LlmError::InvalidResponse(format!("Invalid schema: {}", e)))?;
            body["response_format"] = json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "action",
                    "strict": true,
                    "schema": schema
                }
            });
        }

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LlmError::Timeout
                } else {
                    LlmError::RequestFailed(e.to_string())
                }
            })?;

        let status = resp.status().as_u16();
        let resp_text = resp.text().await.map_err(|e| {
            LlmError::RequestFailed(format!("Failed to read response: {}", e))
        })?;

        if !resp_text.is_empty() {
            let error_val: serde_json::Value = serde_json::from_str(&resp_text).unwrap_or_default();
            if let Some(err_msg) = error_val["error"]["message"].as_str() {
                if status >= 400 {
                    return Err(Self::handle_http_error(status, err_msg));
                }
            }
        }

        if status >= 400 {
            return Err(Self::handle_http_error(status, &resp_text));
        }

        let data: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| LlmError::InvalidResponse(format!("Failed to parse JSON: {}", e)))?;

        let choice = &data["choices"][0];
        let content = choice["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let returned_model = data["model"].as_str().unwrap_or(&self.model).to_string();

        let usage = data.get("usage").map(|u| Usage {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
        });

        let structured_output = if output_format.is_some() {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

        Ok(ChatCompletion {
            content: content.clone(),
            structured_output,
            usage,
            model: returned_model,
        })
    }
}
