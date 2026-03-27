use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentPart {
    Text { text: String },
    Image { image_url: ImageUrl },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "system")]
    System { content: String },
    #[serde(rename = "user")]
    User { content: UserContent },
    #[serde(rename = "assistant")]
    Assistant { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Message::System { content: content.into() }
    }

    pub fn user_text(text: impl Into<String>) -> Self {
        Message::User { content: UserContent::Text(text.into()) }
    }

    pub fn user_multimodal(parts: Vec<ContentPart>) -> Self {
        Message::User { content: UserContent::Parts(parts) }
    }

    pub fn user_with_image(text: impl Into<String>, image_base64: &str) -> Self {
        let data_url = format!("data:image/png;base64,{}", image_base64);
        Message::User {
            content: UserContent::Parts(vec![
                ContentPart::Text { text: text.into() },
                ContentPart::Image {
                    image_url: ImageUrl {
                        url: data_url,
                        detail: Some("auto".into()),
                    },
                },
            ]),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Message::Assistant { content: content.into() }
    }

    pub fn to_openai_value(&self) -> serde_json::Value {
        match self {
            Message::System { content } => json!({ "role": "system", "content": content }),
            Message::User { content } => json!({ "role": "user", "content": content }),
            Message::Assistant { content } => json!({ "role": "assistant", "content": content }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msgs = vec![
            Message::system("You are a helpful assistant."),
            Message::user_text("Hello"),
            Message::assistant("Hi there!"),
        ];
        let json = serde_json::to_string(&msgs).unwrap();
        assert!(json.contains("system"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_multimodal_message() {
        let msg = Message::user_with_image("What is this?", "iVBORw0KGgo=");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("image_url"));
        assert!(json.contains("data:image/png;base64"));
    }
}
