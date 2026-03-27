use crate::serializer::serialize_dom;
use crate::views::{EnhancedDOMTreeNode, SerializedDOMState};
use fa_browser::session::BrowserSession;
use serde_json::Value;

pub struct DomService;

impl DomService {
    pub async fn get_dom_state(
        session: &BrowserSession,
    ) -> anyhow::Result<SerializedDOMState> {
        Self::inject_fa_indices(session).await?;
        let root = Self::fetch_dom_tree(session).await?;
        Ok(serialize_dom(&root))
    }

    async fn inject_fa_indices(session: &BrowserSession) -> anyhow::Result<()> {
        let js = r#"
            (function() {
                var idx = 0;
                var skipTags = new Set(['SCRIPT','STYLE','NOSCRIPT','META','LINK','HEAD','SVG','PATH','G','DEFS','USE','CLIPPATH']);
                var interactiveTags = new Set(['A','BUTTON','INPUT','TEXTAREA','SELECT','OPTION','DETAILS','SUMMARY','DIALOG','IFRAME','OBJECT']);

                function walk(node, depth) {
                    if (depth > 30) return;
                    if (node.nodeType !== 1) return;
                    if (skipTags.has(node.tagName)) return;

                    var style = window.getComputedStyle(node);
                    var visible = style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0';
                    var interactive = interactiveTags.has(node.tagName) || node.onclick || node.getAttribute('role') === 'button' || node.getAttribute('role') === 'link' || node.getAttribute('role') === 'textbox' || node.getAttribute('tabindex') !== null;

                    if (visible && (interactive || node.children.length > 0)) {
                        node.setAttribute('data-fa-index', idx);
                        idx++;
                    }

                    for (var i = 0; i < node.children.length; i++) {
                        walk(node.children[i], depth + 1);
                    }
                }

                document.querySelectorAll('[data-fa-index]').forEach(function(el) { el.removeAttribute('data-fa-index'); });
                walk(document.body, 0);
                return idx;
            })()
        "#;

        session.evaluate_js(js).await?;
        Ok(())
    }

    pub async fn fetch_dom_tree(
        session: &BrowserSession,
    ) -> anyhow::Result<EnhancedDOMTreeNode> {
        let doc_result = session
            .execute_cdp("DOM.getDocument", serde_json::json!({ "depth": -1, "pierce": true }))
            .await?;

        let root = Self::convert_node(&doc_result["root"]);
        Ok(root)
    }

    fn convert_node(node: &Value) -> EnhancedDOMTreeNode {
        let node_type = node["nodeType"].as_u64().unwrap_or(0) as u32;
        let node_name = node["nodeName"].as_str().unwrap_or("").to_string();
        let node_value = node["nodeValue"].as_str().unwrap_or("").to_string();
        let backend_node_id = node["backendNodeId"].as_i64().unwrap_or(0);
        let attributes = node["attributes"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_visible = Self::check_visibility_from_dom_node(node);
        let is_interactive = Self::is_tag_interactive(&node_name);

        let text_content = if node_type == 3 {
            let val = node_value.trim().to_string();
            if val.is_empty() { None } else { Some(val) }
        } else {
            None
        };

        let children = node["children"]
            .as_array()
            .map(|arr| arr.iter().map(Self::convert_node).collect())
            .unwrap_or_default();

        EnhancedDOMTreeNode {
            node_id: node["nodeId"].as_i64().unwrap_or(0),
            backend_node_id,
            node_type,
            node_name,
            node_value,
            attributes,
            text_content,
            children,
            is_visible,
            is_interactive,
            role: node.get("role").and_then(|v| v.as_str()).map(|s| s.to_string()),
            aria_label: node.get("ariaLabel").and_then(|v| v.as_str()).map(|s| s.to_string()),
            bounding_box: None,
        }
    }

    fn check_visibility_from_dom_node(node: &Value) -> bool {
        let node_type = node["nodeType"].as_u64().unwrap_or(0);
        if node_type == 3 {
            return true;
        }
        if node_type != 1 {
            return false;
        }
        true
    }

    fn is_tag_interactive(tag_name: &str) -> bool {
        matches!(
            tag_name.to_lowercase().as_str(),
            "a" | "button" | "input" | "textarea" | "select"
                | "option" | "details" | "summary" | "dialog"
        )
    }
}
