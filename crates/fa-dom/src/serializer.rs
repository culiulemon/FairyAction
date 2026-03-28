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

pub struct DomTreeSerializer {
    show_empty_blocks: bool,
}

impl DomTreeSerializer {
    pub fn new() -> Self {
        Self { show_empty_blocks: false }
    }

    pub fn with_empty_blocks() -> Self {
        Self { show_empty_blocks: true }
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

        let is_interactive = node.is_interactive
            || INTERACTIVE_TAGS.contains(&tag.as_str())
            || node.get_attr("tabindex").is_some()
            || node.get_attr("role").map_or(false, |r| {
                matches!(r.as_str(), "button" | "link" | "textbox" | "checkbox" | "radio" | "tab" | "menuitem" | "switch" | "slider")
            });

        let index = node.get_attr("data-fa-index").and_then(|v| v.parse::<usize>().ok());
        let is_visible_only = node.get_attr("data-fa-visible").is_some() && index.is_none();

        let direct_text = Self::get_direct_text(node);

        if index.is_some() || is_visible_only {
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

            let mut text_parts = Vec::new();
            if !direct_text.is_empty() {
                text_parts.push(direct_text.clone());
            }
            let attr_text = self.get_attr_label(node);
            if !attr_text.is_empty() {
                text_parts.push(attr_text);
            }
            if text_parts.is_empty() && !class_hint.is_empty() {
                text_parts.push(class_hint);
            }
            if text_parts.is_empty() {
                let fallback = self.get_label(node, is_interactive);
                if !fallback.is_empty() {
                    text_parts.push(fallback.trim_matches('"').to_string());
                }
            }
            if text_parts.is_empty() {
                text_parts.push(Self::tag_display_name(&tag).to_string());
            }

            if let Some(idx) = index {
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
            }

            let type_name = Self::tag_display_name(&tag);
            let content = text_parts.join(" ");
            let is_only_tag_name = text_parts.len() == 1 && text_parts[0] == type_name;
            let skip_empty = is_only_tag_name && !self.show_empty_blocks && index.is_none();
            if !skip_empty {
                let indent = "  ".repeat(depth);
                if let Some(idx) = index {
                    output.push_str(&format!("{}[{}] {}：{}\n", indent, idx, type_name, content));
                } else {
                    output.push_str(&format!("{}{}：{}\n", indent, type_name, content));
                }
            }
        }

        let child_depth = if index.is_some() || is_visible_only { depth + 1 } else { depth };

        if !is_interactive && direct_text.is_empty() && LAYOUT_TAGS.contains(&tag.as_str()) {
            for child in &node.children {
                self.serialize_node(child, output, selector_map, child_depth);
            }
            return;
        }

        for child in &node.children {
            self.serialize_node(child, output, selector_map, child_depth);
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
        parts.join(" ").trim().to_string()
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

    fn tag_display_name(tag: &str) -> &'static str {
        match tag {
            "a" => "链接",
            "button" => "按钮",
            "input" => "输入框",
            "textarea" => "文本框",
            "select" => "下拉框",
            "option" => "选项",
            "img" => "图片",
            "video" => "视频",
            "audio" => "音频",
            "iframe" => "内嵌框",
            "details" => "折叠面板",
            "summary" => "折叠标题",
            "dialog" => "对话框",
            "label" => "标签",
            "h1" => "标题",
            "h2" => "标题",
            "h3" => "标题",
            "h4" => "标题",
            "h5" => "标题",
            "h6" => "标题",
            "p" => "段落",
            "span" => "文字",
            "div" => "区块",
            "table" => "表格",
            "form" => "表单",
            "nav" => "导航",
            "li" => "列表项",
            "th" => "表头",
            "td" => "单元格",
            _ => "元素",
        }
    }

    fn get_attr_label(&self, node: &EnhancedDOMTreeNode) -> String {
        let tag = node.node_name.to_lowercase();
        if tag == "input" {
            let input_type = node.get_attr("type").unwrap_or_default();
            let placeholder = node.get_attr("placeholder").unwrap_or_default();
            let value = node.get_attr("value").unwrap_or_default();
            let name = node.get_attr("name").unwrap_or_default();
            let mut parts = Vec::new();
            match input_type.as_str() {
                "text" | "email" | "password" | "tel" | "url" | "number" | "search" => {
                    if !placeholder.is_empty() {
                        parts.push(placeholder);
                    }
                    if !value.is_empty() {
                        parts.push(value);
                    }
                }
                "checkbox" => parts.push("复选框".to_string()),
                "radio" => parts.push("单选框".to_string()),
                "submit" => parts.push("提交".to_string()),
                "reset" => parts.push("重置".to_string()),
                "file" => parts.push("文件上传".to_string()),
                "hidden" => return String::new(),
                _ => {
                    if !placeholder.is_empty() {
                        parts.push(placeholder);
                    }
                }
            }
            if !name.is_empty() && parts.is_empty() {
                parts.push(name);
            }
            return parts.join(" ");
        }
        if tag == "textarea" {
            if let Some(ph) = node.get_attr("placeholder") {
                if !ph.trim().is_empty() {
                    return ph.trim().to_string();
                }
            }
        }
        if tag == "img" {
            if let Some(alt) = node.get_attr("alt") {
                if !alt.trim().is_empty() {
                    return alt.trim().to_string();
                }
            }
            if let Some(src) = node.get_attr("src") {
                return Self::shorten_url(&src, 60);
            }
            return "图片".to_string();
        }
        if tag == "a" {
            if let Some(href) = node.get_attr("href") {
                let shortened = Self::shorten_url(&href, 60);
                if !shortened.is_empty() {
                    return shortened;
                }
            }
        }
        String::new()
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

pub fn serialize_dom_full(root: &EnhancedDOMTreeNode) -> SerializedDOMState {
    let serializer = DomTreeSerializer::with_empty_blocks();
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
        assert!(result.llm_representation.contains("[5] 按钮：Click me"));
        let lines: Vec<&str> = result.llm_representation.lines().collect();
        let div_line = lines.iter().find(|l| l.contains("[0]"));
        let btn_line = lines.iter().find(|l| l.contains("[5]"));
        assert!(div_line.unwrap().starts_with("[0]"));
        assert!(btn_line.unwrap().starts_with("  [5]"));
    }

    #[test]
    fn test_hierarchy_depth() {
        let root = EnhancedDOMTreeNode {
            node_id: 1,
            backend_node_id: 1,
            node_type: 1,
            node_name: "NAV".to_string(),
            node_value: String::new(),
            attributes: vec!["data-fa-index".to_string(), "0".to_string()],
            text_content: None,
            children: vec![
                EnhancedDOMTreeNode {
                    node_id: 2,
                    backend_node_id: 2,
                    node_type: 1,
                    node_name: "A".to_string(),
                    node_value: String::new(),
                    attributes: vec!["data-fa-index".to_string(), "1".to_string(), "href".to_string(), "/home".to_string()],
                    text_content: Some("Home".to_string()),
                    children: vec![],
                    is_visible: true,
                    is_interactive: true,
                    role: None,
                    aria_label: None,
                    bounding_box: None,
                },
                EnhancedDOMTreeNode {
                    node_id: 3,
                    backend_node_id: 3,
                    node_type: 1,
                    node_name: "DIV".to_string(),
                    node_value: String::new(),
                    attributes: vec![],
                    text_content: None,
                    children: vec![
                        EnhancedDOMTreeNode {
                            node_id: 4,
                            backend_node_id: 4,
                            node_type: 1,
                            node_name: "BUTTON".to_string(),
                            node_value: String::new(),
                            attributes: vec!["data-fa-index".to_string(), "2".to_string()],
                            text_content: Some("Login".to_string()),
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
                },
            ],
            is_visible: true,
            is_interactive: false,
            role: None,
            aria_label: None,
            bounding_box: None,
        };

        let result = serialize_dom(&root);
        let lines: Vec<&str> = result.llm_representation.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("[0]"));
        assert!(lines[1].starts_with("  [1]"));
        assert!(lines[2].starts_with("  [2]"));
    }
}
