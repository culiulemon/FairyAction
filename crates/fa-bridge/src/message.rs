use crate::error::{BridgeError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeMessageType {
    Hello,
    Call,
    Ok,
    Error,
    Progress,
    Configure,
}

impl BridgeMessageType {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "hello" => Some(Self::Hello),
            "call" => Some(Self::Call),
            "ok" => Some(Self::Ok),
            "error" => Some(Self::Error),
            "progress" => Some(Self::Progress),
            "configure" => Some(Self::Configure),
            _ => None,
        }
    }
}

impl std::fmt::Display for BridgeMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Hello => "hello",
            Self::Call => "call",
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Progress => "progress",
            Self::Configure => "configure",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone)]
pub struct BridgeMessage {
    pub message_type: BridgeMessageType,
    pub module: Option<String>,
    pub channel: Option<String>,
    pub action: Option<String>,
    pub payload: serde_json::Value,
}

impl BridgeMessage {
    pub fn parse(input: &str) -> Result<Self> {
        let rest = input
            .strip_prefix("bridge://")
            .ok_or_else(|| BridgeError::InvalidProtocol("must start with bridge://".into()))?;

        let (fields_part, payload_str) = match rest.rsplit_once('#') {
            Some((before, after)) => (before, after),
            None => (rest, ""),
        };

        let payload = if payload_str.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(payload_str)
                .map_err(|e| BridgeError::InvalidPayload(e.to_string()))?
        };

        let fields: Vec<&str> = fields_part.split('\x1F').collect();

        let first = fields[0];

        if let Some(msg_type) = BridgeMessageType::from_str(first) {
            let (module, channel, action) = match msg_type {
                BridgeMessageType::Hello | BridgeMessageType::Configure => {
                    let module = fields.get(1).map(|s| (*s).to_string());
                    (module, None, None)
                }
                _ => {
                    let module = fields.get(1).map(|s| (*s).to_string());
                    let channel = fields.get(2).map(|s| (*s).to_string());
                    let action = fields.get(3).map(|s| (*s).to_string());
                    (module, channel, action)
                }
            };

            Ok(Self {
                message_type: msg_type,
                module,
                channel,
                action,
                payload,
            })
        } else {
            let module = Some(first.to_string());
            let channel = fields.get(1).map(|s| (*s).to_string());
            let action = fields.get(2).map(|s| (*s).to_string());

            Ok(Self {
                message_type: BridgeMessageType::Call,
                module,
                channel,
                action,
                payload,
            })
        }
    }

    pub fn serialize(&self) -> String {
        let mut result = format!("bridge://{}", self.message_type);

        if let Some(module) = &self.module {
            result.push('\x1F');
            result.push_str(module);
        }

        if let Some(channel) = &self.channel {
            result.push('\x1F');
            result.push_str(channel);
        }

        if let Some(action) = &self.action {
            result.push('\x1F');
            result.push_str(action);
        }

        result.push('#');

        match &self.payload {
            serde_json::Value::Null => {}
            other => result.push_str(&other.to_string()),
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_call_message_full() {
        let input = format!(
            "bridge://call\u{001F}com.ffmpeg.fap\u{001F}图片转换\u{001F}png2jpg#{{\"质量\":30}}"
        );
        let msg = BridgeMessage::parse(&input).unwrap();
        assert_eq!(msg.message_type, BridgeMessageType::Call);
        assert_eq!(msg.module.as_deref(), Some("com.ffmpeg.fap"));
        assert_eq!(msg.channel.as_deref(), Some("图片转换"));
        assert_eq!(msg.action.as_deref(), Some("png2jpg"));
        assert_eq!(msg.payload["质量"], 30);
    }

    #[test]
    fn parse_hello_with_module() {
        let input = format!("bridge://hello\u{001F}com.ffmpeg.fap#");
        let msg = BridgeMessage::parse(&input).unwrap();
        assert_eq!(msg.message_type, BridgeMessageType::Hello);
        assert_eq!(msg.module.as_deref(), Some("com.ffmpeg.fap"));
        assert!(msg.channel.is_none());
        assert!(msg.action.is_none());
        assert!(msg.payload.is_null());
    }

    #[test]
    fn parse_configure_payload_only() {
        let input = r#"bridge://configure#{"fap.install_dir":"D:\\MyApp"}"#;
        let msg = BridgeMessage::parse(input).unwrap();
        assert_eq!(msg.message_type, BridgeMessageType::Configure);
        assert!(msg.module.is_none());
        assert_eq!(msg.payload["fap.install_dir"], "D:\\MyApp");
    }

    #[test]
    fn parse_ok_response() {
        let input = format!(
            "bridge://ok\u{001F}com.ffmpeg.fap\u{001F}图片转换\u{001F}png2jpg#{{\"结果\":\"成功\"}}"
        );
        let msg = BridgeMessage::parse(&input).unwrap();
        assert_eq!(msg.message_type, BridgeMessageType::Ok);
        assert_eq!(msg.module.as_deref(), Some("com.ffmpeg.fap"));
        assert_eq!(msg.channel.as_deref(), Some("图片转换"));
        assert_eq!(msg.action.as_deref(), Some("png2jpg"));
        assert_eq!(msg.payload["结果"], "成功");
    }

    #[test]
    fn serialize_call_message() {
        let msg = BridgeMessage {
            message_type: BridgeMessageType::Call,
            module: Some("com.ffmpeg.fap".into()),
            channel: Some("图片转换".into()),
            action: Some("png2jpg".into()),
            payload: serde_json::json!({"质量": 30}),
        };
        let s = msg.serialize();
        let expected = format!(
            "bridge://call\u{001F}com.ffmpeg.fap\u{001F}图片转换\u{001F}png2jpg#{{\"质量\":30}}"
        );
        assert_eq!(s, expected);
    }

    #[test]
    fn serialize_ok_response() {
        let msg = BridgeMessage {
            message_type: BridgeMessageType::Ok,
            module: Some("com.ffmpeg.fap".into()),
            channel: Some("图片转换".into()),
            action: Some("png2jpg".into()),
            payload: serde_json::json!({"结果": "成功"}),
        };
        let s = msg.serialize();
        let expected = format!(
            "bridge://ok\u{001F}com.ffmpeg.fap\u{001F}图片转换\u{001F}png2jpg#{{\"结果\":\"成功\"}}"
        );
        assert_eq!(s, expected);
    }

    #[test]
    fn parse_invalid_no_bridge_prefix() {
        let input = "hello\x1Fcom.ffmpeg.fap#";
        let result = BridgeMessage::parse(input);
        assert!(result.is_err());
        match result.unwrap_err() {
            BridgeError::InvalidProtocol(_) => {}
            other => panic!("expected InvalidProtocol, got: {other}"),
        }
    }

    #[test]
    fn parse_unknown_type_treated_as_call() {
        let input = format!(
            "bridge://com.ffmpeg.fap\u{001F}图片转换\u{001F}png2jpg#{{\"质量\":30}}"
        );
        let msg = BridgeMessage::parse(&input).unwrap();
        assert_eq!(msg.message_type, BridgeMessageType::Call);
        assert_eq!(msg.module.as_deref(), Some("com.ffmpeg.fap"));
        assert_eq!(msg.channel.as_deref(), Some("图片转换"));
        assert_eq!(msg.action.as_deref(), Some("png2jpg"));
        assert_eq!(msg.payload["质量"], 30);
    }

    #[test]
    fn parse_empty_payload() {
        let input = format!("bridge://call\u{001F}mod\u{001F}chan\u{001F}act#");
        let msg = BridgeMessage::parse(&input).unwrap();
        assert_eq!(msg.message_type, BridgeMessageType::Call);
        assert!(msg.payload.is_null());
    }
}
