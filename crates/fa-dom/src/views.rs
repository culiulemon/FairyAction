use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SerializedDOMState {
    pub selector_map: HashMap<usize, DomNodeInfo>,
    pub llm_representation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomNodeInfo {
    pub tag_name: String,
    pub attributes: HashMap<String, String>,
    pub text_content: Option<String>,
    pub is_interactive: bool,
    pub is_visible: bool,
    pub backend_node_id: Option<i64>,
    pub xpath: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedDOMTreeNode {
    pub node_id: i64,
    pub backend_node_id: i64,
    pub node_type: u32,
    pub node_name: String,
    pub node_value: String,
    pub attributes: Vec<String>,
    pub text_content: Option<String>,
    pub children: Vec<EnhancedDOMTreeNode>,
    pub is_visible: bool,
    pub is_interactive: bool,
    pub role: Option<String>,
    pub aria_label: Option<String>,
    pub bounding_box: Option<BoundingBox>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl EnhancedDOMTreeNode {
    pub fn get_interactive_attributes(&self) -> Vec<(String, String)> {
        let mut attrs = Vec::new();
        let interesting = [
            "href", "src", "alt", "title", "aria-label", "aria-description",
            "placeholder", "value", "name", "type", "role", "id",
            "aria-expanded", "aria-checked", "aria-disabled", "aria-haspopup",
            "aria-required", "aria-selected", "aria-valuemin", "aria-valuemax",
            "aria-valuenow", "disabled", "checked", "selected", "readonly",
            "data-fa-index",
        ];
        let attr_pairs: Vec<(String, String)> = self.attributes
            .chunks(2)
            .filter_map(|chunk| {
                if chunk.len() == 2 {
                    Some((chunk[0].clone(), chunk[1].clone()))
                } else {
                    None
                }
            })
            .collect();

        for (key, value) in &attr_pairs {
            if interesting.contains(&key.as_str()) {
                attrs.push((key.clone(), value.clone()));
            }
        }

        if let Some(role) = &self.role {
            attrs.push(("role".to_string(), role.clone()));
        }
        if let Some(label) = &self.aria_label {
            attrs.push(("aria-label".to_string(), label.clone()));
        }

        attrs
    }

    pub fn get_attr(&self, name: &str) -> Option<String> {
        let pairs: Vec<(&str, &str)> = self.attributes
            .chunks(2)
            .filter_map(|chunk| {
                if chunk.len() == 2 {
                    Some((chunk[0].as_str(), chunk[1].as_str()))
                } else {
                    None
                }
            })
            .collect();
        pairs.iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.to_string())
    }

    pub fn get_all_children_text(&self, max_depth: usize) -> String {
        let mut parts = Vec::new();
        self._collect_text(&mut parts, 0, max_depth);
        let result: String = parts.join(" ").split_whitespace().collect();
        result
    }

    fn _collect_text(&self, parts: &mut Vec<String>, depth: usize, max_depth: usize) {
        if depth > max_depth {
            return;
        }
        if self.node_type == 3 && !self.node_value.trim().is_empty() {
            parts.push(self.node_value.trim().to_string());
        }
        for child in &self.children {
            child._collect_text(parts, depth + 1, max_depth);
        }
    }
}
