use std::collections::HashMap;
use crate::views::{DomNodeInfo, EnhancedDOMTreeNode, SerializedDOMState};

const INTERACTIVE_TAGS: &[&str] = &[
    "a", "button", "input", "textarea", "select", "option",
    "details", "summary", "dialog", "iframe", "[object]",
];

const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "meta", "link", "head",
    "svg", "path", "g", "defs", "use", "clippath",
];

pub struct DomTreeSerializer;

impl DomTreeSerializer {
    pub fn new() -> Self {
        Self
    }

    pub fn serialize(&self, root: &EnhancedDOMTreeNode) -> (String, HashMap<usize, DomNodeInfo>) {
        let mut output = String::new();
        let mut selector_map = HashMap::new();
        self.serialize_node(root, &mut output, &mut selector_map, 0);
        (output, selector_map)
    }

    fn serialize_node(
        &self,
        node: &EnhancedDOMTreeNode,
        output: &mut String,
        selector_map: &mut HashMap<usize, DomNodeInfo>,
        depth: usize,
    ) {
        if depth > 30 {
            return;
        }

        if node.node_type != 1 {
            for child in &node.children {
                self.serialize_node(child, output, selector_map, depth);
            }
            return;
        }

        let tag = node.node_name.to_lowercase();

        if SKIP_TAGS.contains(&tag.as_str()) {
            return;
        }

        let is_interactive = node.is_interactive || INTERACTIVE_TAGS.contains(&tag.as_str());

        if !node.is_visible && !is_interactive {
            for child in &node.children {
                self.serialize_node(child, output, selector_map, depth);
            }
            return;
        }

        let index = node.get_attr("data-fa-index").and_then(|v| v.parse::<usize>().ok());

        if let Some(idx) = index {
            let attr_pairs: HashMap<String, String> = node.attributes
                .chunks(2)
                .filter_map(|chunk| {
                    if chunk.len() == 2 {
                        Some((chunk[0].clone(), chunk[1].clone()))
                    } else {
                        None
                    }
                })
                .collect();

            let xpath = format!("//*[@data-fa-index='{}']", idx);
            selector_map.insert(idx, DomNodeInfo {
                tag_name: node.node_name.clone(),
                attributes: attr_pairs,
                text_content: node.text_content.clone(),
                is_interactive,
                is_visible: node.is_visible,
                backend_node_id: Some(node.backend_node_id),
                xpath,
            });

            let indent_str = "  ".repeat(depth);
            let attrs_str = self.format_attributes(node);

            if is_interactive {
                output.push_str(&format!("{}[{}] <{}{}>\n", indent_str, idx, tag, attrs_str));
            } else {
                output.push_str(&format!("{}[{}] {}{}\n", indent_str, idx, tag, attrs_str));
            }

            let label = self.get_semantic_label(node);
            if !label.is_empty() {
                output.push_str(&format!("{}  {}\n", indent_str, label));
            }
        }

        for child in &node.children {
            self.serialize_node(child, output, selector_map, depth + 1);
        }
    }

    fn get_semantic_label(&self, node: &EnhancedDOMTreeNode) -> String {
        let tag = node.node_name.to_lowercase();

        for attr_name in &["aria-label", "title", "alt", "placeholder"] {
            if let Some(val) = node.get_attr(attr_name) {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    return format!("\"{}\"", trimmed);
                }
            }
        }

        if tag == "input" {
            if let Some(val) = node.get_attr("value") {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    return format!("\"{}\"", trimmed);
                }
            }
        }

        let children_text = node.get_all_children_text(5);
        let trimmed = children_text.trim();
        if !trimmed.is_empty() && trimmed.len() < 200 {
            return format!("\"{}\"", trimmed);
        }

        String::new()
    }

    fn format_attributes(&self, node: &EnhancedDOMTreeNode) -> String {
        let attrs = node.get_interactive_attributes();
        if attrs.is_empty() {
            return String::new();
        }
        let mut parts = Vec::new();
        for (k, v) in &attrs {
            if k == "data-fa-index" {
                continue;
            }
            let display_val = if v.len() > 80 {
                let mut end = 80;
                while end > 0 && !v.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &v[..end])
            } else {
                v.clone()
            };
            parts.push(format!(" {}=\"{}\"", k, display_val));
        }
        parts.join("")
    }
}

pub fn serialize_dom(root: &EnhancedDOMTreeNode) -> SerializedDOMState {
    let serializer = DomTreeSerializer::new();
    let (llm_representation, selector_map) = serializer.serialize(root);

    SerializedDOMState {
        selector_map,
        llm_representation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_with_fa_index() {
        let root = EnhancedDOMTreeNode {
            node_id: 1,
            backend_node_id: 1,
            node_type: 1,
            node_name: "DIV".to_string(),
            node_value: String::new(),
            attributes: vec!["data-fa-index".to_string(), "0".to_string()],
            text_content: None,
            children: vec![
                EnhancedDOMTreeNode {
                    node_id: 2,
                    backend_node_id: 2,
                    node_type: 1,
                    node_name: "BUTTON".to_string(),
                    node_value: String::new(),
                    attributes: vec!["data-fa-index".to_string(), "5".to_string()],
                    text_content: Some("Click me".to_string()),
                    children: vec![],
                    is_visible: true,
                    is_interactive: true,
                    role: None,
                    aria_label: None,
                    bounding_box: None,
                },
            ],
            is_visible: true,
            is_interactive: false,
            role: None,
            aria_label: None,
            bounding_box: None,
        };

        let result = serialize_dom(&root);
        assert!(result.llm_representation.contains("[0] div"));
        assert!(result.llm_representation.contains("[5] <button>"));
        assert!(result.llm_representation.contains("Click me"));
        assert!(result.selector_map.contains_key(&0));
        assert!(result.selector_map.contains_key(&5));
    }
}
