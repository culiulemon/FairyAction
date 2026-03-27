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

const LAYOUT_TAGS: &[&str] = &[
    "div", "span", "section", "main", "header", "footer", "nav",
    "article", "aside", "ul", "ol", "li", "table", "tbody", "thead",
    "tr", "td", "th", "form", "fieldset", "figure", "figcaption",
    "template", "slot", "br", "hr", "wbr",
];

pub struct DomTreeSerializer;

impl DomTreeSerializer {
    pub fn new() -> Self {
        Self
    }

    pub fn serialize(&self, root: &EnhancedDOMTreeNode) -> (String, HashMap<usize, DomNodeInfo>) {
        let mut output = String::new();
        let mut selector_map = HashMap::new();
        self.serialize_node(root, &mut output, &mut selector_map);
        (output, selector_map)
    }

    fn serialize_node(
        &self,
        node: &EnhancedDOMTreeNode,
        output: &mut String,
        selector_map: &mut HashMap<usize, DomNodeInfo>,
    ) {
        if node.node_type != 1 {
            for child in &node.children {
                self.serialize_node(child, output, selector_map);
            }
            return;
        }

        let tag = node.node_name.to_lowercase();

        if SKIP_TAGS.contains(&tag.as_str()) {
            return;
        }

        let is_interactive = node.is_interactive
            || INTERACTIVE_TAGS.contains(&tag.as_str())
            || node.get_attr("tabindex").is_some()
            || node.get_attr("role").map_or(false, |r| {
                matches!(r.as_str(), "button" | "link" | "textbox" | "checkbox" | "radio" | "tab" | "menuitem" | "switch" | "slider")
            });

        let index = node.get_attr("data-fa-index").and_then(|v| v.parse::<usize>().ok());

        let direct_text = Self::get_direct_text(node);

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

            let class_str = node.get_attr("class").unwrap_or_default();
            let class_hint = Self::get_class_hint(&class_str);
            let label = if is_interactive {
                self.get_label(node, true)
            } else if !direct_text.is_empty() {
                format!("\"{}\"", direct_text)
            } else if !class_hint.is_empty() {
                class_hint
            } else {
                self.get_label(node, false)
            };

            let xpath = format!("//*[@data-fa-index='{}']", idx);
            selector_map.insert(idx, DomNodeInfo {
                tag_name: node.node_name.clone(),
                attributes: attr_pairs,
                text_content: if direct_text.is_empty() { None } else { Some(direct_text.clone()) },
                is_interactive,
                is_visible: node.is_visible,
                backend_node_id: Some(node.backend_node_id),
                xpath,
            });

            let attrs_str = self.format_attributes(node);

            if is_interactive {
                output.push_str(&format!("[{}] <{}{}>", idx, tag, attrs_str));
            } else {
                output.push_str(&format!("[{}] {}{}", idx, tag, attrs_str));
            }

            if !label.is_empty() {
                output.push_str(&format!(" {}", label));
            }

            output.push('\n');
        }

        if !is_interactive && direct_text.is_empty() && LAYOUT_TAGS.contains(&tag.as_str()) {
            for child in &node.children {
                self.serialize_node(child, output, selector_map);
            }
            return;
        }

        for child in &node.children {
            self.serialize_node(child, output, selector_map);
        }
    }

    fn get_direct_text(node: &EnhancedDOMTreeNode) -> String {
        let mut parts = Vec::new();
        for child in &node.children {
            if child.node_type == 3 {
                let t = child.node_value.trim();
                if !t.is_empty() {
                    parts.push(t.to_string());
                }
            }
        }
        parts.join(" ").split_whitespace().collect()
    }

    fn get_label(&self, node: &EnhancedDOMTreeNode, is_interactive: bool) -> String {
        for attr_name in &["aria-label", "title", "alt", "placeholder"] {
            if let Some(val) = node.get_attr(attr_name) {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    return format!("\"{}\"", trimmed);
                }
            }
        }

        let tag = node.node_name.to_lowercase();
        if tag == "input" {
            if let Some(val) = node.get_attr("value") {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    return format!("\"{}\"", trimmed);
                }
            }
        }

        if is_interactive {
            let children_text = node.get_all_children_text(5);
            let trimmed = children_text.trim();
            if !trimmed.is_empty() && trimmed.len() < 200 {
                return format!("\"{}\"", trimmed);
            }
        } else {
            let direct = Self::get_direct_text(node);
            if !direct.is_empty() && direct.len() < 200 {
                return format!("\"{}\"", direct);
            }
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
            let display_val = if k == "href" || k == "src" {
                Self::shorten_url(v, 60)
            } else if v.len() > 80 {
                let mut end = 80;
                while end > 0 && !v.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &v[..end])
            } else {
                v.clone()
            };
            if !display_val.is_empty() {
                parts.push(format!(" {}=\"{}\"", k, display_val));
            }
        }
        parts.join("")
    }

    fn shorten_url(url: &str, max_len: usize) -> String {
        if url.starts_with("javascript:") {
            return String::new();
        }
        let trimmed = url.split('?').next().unwrap_or(url);
        let trimmed = trimmed.split('#').next().unwrap_or(trimmed);
        let trimmed = trimmed.trim_end_matches('/');
        if trimmed.is_empty() || trimmed == "/" {
            return String::new();
        }
        if trimmed.len() > max_len {
            let mut end = max_len;
            while end > 0 && !trimmed.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &trimmed[..end])
        } else {
            trimmed.to_string()
        }
    }

    fn get_class_hint(class_str: &str) -> String {
        let action_keywords = [
            ("close", "close"),
            ("back", "back"),
            ("delete", "delete"),
            ("remove", "remove"),
            ("cancel", "cancel"),
            ("confirm", "confirm"),
            ("submit", "submit"),
            ("search", "search"),
            ("menu", "menu"),
            ("dropdown", "dropdown"),
            ("toggle", "toggle"),
            ("expand", "expand"),
            ("collapse", "collapse"),
            ("play", "play"),
            ("pause", "pause"),
            ("next", "next"),
            ("prev", "previous"),
            ("arrow", "arrow"),
            ("chevron", "chevron"),
            ("icon-btn", "icon button"),
            ("btn", "button"),
            ("button", "button"),
            ("click", "clickable"),
        ];
        let lower = class_str.to_lowercase();
        for (keyword, hint) in &action_keywords {
            if lower.contains(keyword) {
                return format!("[{}]", hint);
            }
        }
        String::new()
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
                    children: vec![
                        EnhancedDOMTreeNode {
                            node_id: 3,
                            backend_node_id: 3,
                            node_type: 3,
                            node_name: "#text".to_string(),
                            node_value: "Click me".to_string(),
                            attributes: vec![],
                            text_content: Some("Click me".to_string()),
                            children: vec![],
                            is_visible: true,
                            is_interactive: false,
                            role: None,
                            aria_label: None,
                            bounding_box: None,
                        },
                    ],
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
        assert!(result.selector_map.contains_key(&5));
        assert!(result.llm_representation.contains("[5] <button>"));
        assert!(result.llm_representation.contains("Clickme") || result.llm_representation.contains("Click me"));
    }
}
