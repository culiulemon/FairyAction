use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionParamDef {
    pub name: String,
    pub param_type: ParamType,
    pub description: String,
    pub required: bool,
    pub default: Option<Value>,
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamType {
    String,
    Integer,
    Number,
    Boolean,
    Array,
    Object,
}

impl ParamType {
    pub fn to_json_schema_type(&self) -> &str {
        match self {
            ParamType::String => "string",
            ParamType::Integer => "integer",
            ParamType::Number => "number",
            ParamType::Boolean => "boolean",
            ParamType::Array => "array",
            ParamType::Object => "object",
        }
    }

    pub fn to_value_type(&self) -> &str {
        self.to_json_schema_type()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    pub name: String,
    pub description: String,
    pub params: Vec<ActionParamDef>,
    pub terminates_sequence: bool,
}

impl ActionDef {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            params: Vec::new(),
            terminates_sequence: false,
        }
    }

    pub fn param(
        mut self,
        name: impl Into<String>,
        param_type: ParamType,
        description: impl Into<String>,
    ) -> Self {
        self.params.push(ActionParamDef {
            name: name.into(),
            param_type,
            description: description.into(),
            required: true,
            default: None,
            enum_values: None,
        });
        self
    }

    pub fn optional_param(
        mut self,
        name: impl Into<String>,
        param_type: ParamType,
        description: impl Into<String>,
        default: Value,
    ) -> Self {
        self.params.push(ActionParamDef {
            name: name.into(),
            param_type,
            description: description.into(),
            required: false,
            default: Some(default),
            enum_values: None,
        });
        self
    }

    pub fn enum_param(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        values: Vec<String>,
        default: Option<&str>,
    ) -> Self {
        self.params.push(ActionParamDef {
            name: name.into(),
            param_type: ParamType::String,
            description: description.into(),
            required: default.is_none(),
            default: default.map(|v| Value::String(v.to_string())),
            enum_values: Some(values),
        });
        self
    }

    pub fn terminates_sequence(mut self) -> Self {
        self.terminates_sequence = true;
        self
    }

    pub fn to_json_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();

        for param in &self.params {
            let mut prop = serde_json::Map::new();
            prop.insert("type".to_string(), Value::String(param.param_type.to_json_schema_type().to_string()));
            prop.insert("description".to_string(), Value::String(param.description.clone()));

            if let Some(ref enum_vals) = param.enum_values {
                let vals: Vec<Value> = enum_vals.iter().map(|v| Value::String(v.clone())).collect();
                prop.insert("enum".to_string(), Value::Array(vals));
            }

            if let Some(ref default) = param.default {
                prop.insert("default".to_string(), default.clone());
            }

            properties.insert(param.name.clone(), Value::Object(prop));
        }

        let required: Vec<Value> = self.params
            .iter()
            .filter(|p| p.required)
            .map(|p| Value::String(p.name.clone()))
            .collect();

        let mut schema = serde_json::Map::new();
        schema.insert("type".to_string(), Value::String("object".to_string()));
        schema.insert("properties".to_string(), Value::Object(properties));
        schema.insert("additionalProperties".to_string(), Value::Bool(false));
        if !required.is_empty() {
            schema.insert("required".to_string(), Value::Array(required));
        }

        Value::Object(schema)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub include_in_memory: bool,
    pub is_done: bool,
    pub extracted_content: Option<String>,
    pub extracted_links: Option<Vec<String>>,
    pub extracted_images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_after: Option<ActionStateAfter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStateAfter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_tab_opened: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigation_occurred: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}

impl ActionResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            include_in_memory: false,
            is_done: false,
            extracted_content: None,
            extracted_links: None,
            extracted_images: None,
            state_after: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(msg.into()),
            include_in_memory: false,
            is_done: false,
            extracted_content: None,
            extracted_links: None,
            extracted_images: None,
            state_after: None,
        }
    }

    pub fn done(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            include_in_memory: false,
            is_done: true,
            extracted_content: None,
            extracted_links: None,
            extracted_images: None,
            state_after: None,
        }
    }

    pub fn extracted(content: impl Into<String>) -> Self {
        Self {
            success: true,
            output: None,
            error: None,
            include_in_memory: true,
            is_done: false,
            extracted_content: Some(content.into()),
            extracted_links: None,
            extracted_images: None,
            state_after: None,
        }
    }

    pub fn with_memory(mut self, output: impl Into<String>) -> Self {
        self.include_in_memory = true;
        self.output = Some(output.into());
        self
    }

    pub fn with_extracted_links(mut self, links: Vec<String>) -> Self {
        self.extracted_links = Some(links);
        self
    }

    pub fn extracted_with_links(content: impl Into<String>, links: Vec<String>) -> Self {
        Self {
            success: true,
            output: None,
            error: None,
            include_in_memory: true,
            is_done: false,
            extracted_content: Some(content.into()),
            extracted_links: Some(links),
            extracted_images: None,
            state_after: None,
        }
    }

    pub fn with_state_after(mut self, state: ActionStateAfter) -> Self {
        self.state_after = Some(state);
        self
    }
}

pub fn parse_action_params(params: &Value) -> HashMap<String, Value> {
    if let Some(map) = params.as_object() {
        map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    } else {
        HashMap::new()
    }
}

pub fn get_string(params: &HashMap<String, Value>, key: &str) -> Option<String> {
    params.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

pub fn get_i64(params: &HashMap<String, Value>, key: &str) -> Option<i64> {
    params.get(key).and_then(|v| v.as_i64())
}

pub fn get_f64(params: &HashMap<String, Value>, key: &str) -> Option<f64> {
    params.get(key).and_then(|v| v.as_f64())
}

pub fn get_bool(params: &HashMap<String, Value>, key: &str) -> bool {
    params.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

pub fn get_bool_raw(params: &HashMap<String, Value>, key: &str) -> Option<bool> {
    params.get(key).and_then(|v| v.as_bool())
}
