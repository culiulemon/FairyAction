use crate::serializer::{serialize_dom, serialize_dom_full};
use crate::views::{EnhancedDOMTreeNode, SerializedDOMState};
use fa_browser::session::BrowserSession;
use serde_json::Value;

pub struct DomService;

impl DomService {
    pub async fn get_dom_state(
        session: &BrowserSession,
    ) -> anyhow::Result<SerializedDOMState> {
        Self::get_dom_state_impl(session, false).await
    }

    pub async fn get_dom_state_full(
        session: &BrowserSession,
    ) -> anyhow::Result<SerializedDOMState> {
        Self::get_dom_state_impl(session, true).await
    }

    async fn get_dom_state_impl(
        session: &BrowserSession,
        show_empty_blocks: bool,
    ) -> anyhow::Result<SerializedDOMState> {
        Self::inject_fa_indices(session).await?;
        let root = Self::fetch_dom_tree(session).await?;
        Ok(if show_empty_blocks { serialize_dom_full(&root) } else { serialize_dom(&root) })
    }

    pub async fn show_annotations(session: &BrowserSession) -> anyhow::Result<bool> {
        let js = r#"
            (function() {
                if (document.getElementById('__fa_annotation_overlay__')) {
                    return 'already_exists';
                }

                var style = document.createElement('style');
                style.id = '__fa_annotation_style__';
                style.textContent = `
                    #__fa_annotation_overlay__ {
                        position: fixed;
                        top: 0;
                        left: 0;
                        width: 100%;
                        height: 100%;
                        pointer-events: none;
                        z-index: 2147483647;
                        font-family: 'SF Mono', 'Cascadia Code', 'Consolas', monospace;
                        font-size: 11px;
                        line-height: 1;
                    }
                    .__fa_ann_rect__ {
                        position: absolute;
                        border: 2px solid rgba(0, 120, 212, 0.7);
                        background: rgba(0, 120, 212, 0.08);
                        border-radius: 2px;
                        box-sizing: border-box;
                        transition: opacity 0.15s ease;
                    }
                    .__fa_ann_rect__.__fa_ann_interactive__ {
                        border-color: rgba(22, 163, 74, 0.8);
                        background: rgba(22, 163, 74, 0.10);
                    }
                    .__fa_ann_label__ {
                        position: absolute;
                        top: -1px;
                        left: -1px;
                        transform: translate(-100%, -100%);
                        background: rgba(0, 120, 212, 0.9);
                        color: #fff;
                        padding: 1px 4px;
                        border-radius: 3px 0 3px 0;
                        font-size: 10px;
                        font-weight: 600;
                        white-space: nowrap;
                        pointer-events: none;
                        letter-spacing: 0.3px;
                    }
                    .__fa_ann_rect__.__fa_ann_interactive__ .__fa_ann_label__ {
                        background: rgba(22, 163, 74, 0.9);
                    }
                    .__fa_ann_tag__ {
                        position: absolute;
                        bottom: -1px;
                        right: -1px;
                        transform: translate(100%, 100%);
                        background: rgba(100, 100, 100, 0.75);
                        color: #fff;
                        padding: 0px 3px;
                        border-radius: 0 3px 0 3px;
                        font-size: 9px;
                        white-space: nowrap;
                        pointer-events: none;
                    }
                `;
                document.head.appendChild(style);

                var overlay = document.createElement('div');
                overlay.id = '__fa_annotation_overlay__';
                document.body.appendChild(overlay);

                var interactiveTags = new Set(['A','BUTTON','INPUT','TEXTAREA','SELECT','OPTION','DETAILS','SUMMARY','DIALOG','IFRAME']);

                function updateAnnotations() {
                    overlay.innerHTML = '';
                    var elements = document.querySelectorAll('[data-fa-index]');
                    elements.forEach(function(el) {
                        var rect = el.getBoundingClientRect();
                        if (rect.width === 0 && rect.height === 0) return;

                        var idx = el.getAttribute('data-fa-index');
                        var tag = el.tagName.toLowerCase();
                        var isInteractive = interactiveTags.has(el.tagName)
                            || el.getAttribute('role') === 'button'
                            || el.getAttribute('role') === 'link'
                            || el.getAttribute('role') === 'textbox'
                            || el.getAttribute('role') === 'checkbox'
                            || el.getAttribute('role') === 'menuitem'
                            || el.getAttribute('tabindex') !== null;

                        var box = document.createElement('div');
                        box.className = '__fa_ann_rect__' + (isInteractive ? ' __fa_ann_interactive__' : '');
                        box.style.left = rect.left + 'px';
                        box.style.top = rect.top + 'px';
                        box.style.width = rect.width + 'px';
                        box.style.height = rect.height + 'px';

                        var label = document.createElement('span');
                        label.className = '__fa_ann_label__';
                        label.textContent = idx;
                        box.appendChild(label);

                        var tagLabel = document.createElement('span');
                        tagLabel.className = '__fa_ann_tag__';
                        tagLabel.textContent = tag;
                        box.appendChild(tagLabel);

                        overlay.appendChild(box);
                    });
                }

                updateAnnotations();

                var rafPending = false;
                var observer = new MutationObserver(function() {
                    if (!rafPending) {
                        rafPending = true;
                        requestAnimationFrame(function() {
                            rafPending = false;
                            updateAnnotations();
                        });
                    }
                });
                observer.observe(document.body, {
                    childList: true,
                    subtree: true,
                    attributes: true,
                    attributeFilter: ['data-fa-index', 'style', 'class', 'hidden']
                });

                window.addEventListener('scroll', function() {
                    if (!rafPending) {
                        rafPending = true;
                        requestAnimationFrame(function() {
                            rafPending = false;
                            updateAnnotations();
                        });
                    }
                }, { passive: true });

                window.addEventListener('resize', function() {
                    if (!rafPending) {
                        rafPending = true;
                        requestAnimationFrame(function() {
                            rafPending = false;
                            updateAnnotations();
                        });
                    }
                }, { passive: true });

                return 'created';
            })()
        "#;
        let result = session.evaluate_js(js).await?;
        let status = result["result"]["value"].as_str().unwrap_or("");
        Ok(status == "created")
    }

    pub async fn hide_annotations(session: &BrowserSession) -> anyhow::Result<bool> {
        let js = r#"
            (function() {
                var overlay = document.getElementById('__fa_annotation_overlay__');
                var style = document.getElementById('__fa_annotation_style__');
                if (overlay) overlay.remove();
                if (style) style.remove();
                return !!overlay;
            })()
        "#;
        let result = session.evaluate_js(js).await?;
        let removed = result["result"]["value"].as_bool().unwrap_or(false);
        Ok(removed)
    }

    pub async fn toggle_annotations(session: &BrowserSession) -> anyhow::Result<bool> {
        let js = r#"
            (function() {
                return !!document.getElementById('__fa_annotation_overlay__');
            })()
        "#;
        let result = session.evaluate_js(js).await?;
        let visible = result["result"]["value"].as_bool().unwrap_or(false);
        if visible {
            Self::hide_annotations(session).await
        } else {
            Self::show_annotations(session).await
        }
    }

    pub async fn inject_fa_indices(session: &BrowserSession) -> anyhow::Result<()> {
        let js = r#"
            (function() {
                if (!document.body) return -1;
                var idx = 0;
                var skipTags = new Set(['SCRIPT','STYLE','NOSCRIPT','META','LINK','HEAD','SVG','PATH','G','DEFS','USE','CLIPPATH']);
                var interactiveTags = new Set(['A','BUTTON','INPUT','TEXTAREA','SELECT','OPTION','DETAILS','SUMMARY','DIALOG','IFRAME','OBJECT']);
                var semanticTags = new Set(['H1','H2','H3','H4','H5','H6','P','LABEL','IMG','VIDEO','AUDIO','FIGCAPTION','BLOCKQUOTE','PRE','CODE','STRONG','EM','B','I','MARK','SMALL','SPAN']);

                function walk(node, depth) {
                    if (depth > 30) return;
                    if (node.nodeType !== 1) return;
                    if (skipTags.has(node.tagName)) return;

                    var style = window.getComputedStyle(node);
                    var visible = style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0';
                    if (!visible) return;

                    var interactive = interactiveTags.has(node.tagName)
                        || node.onclick
                        || node.getAttribute('role') === 'button'
                        || node.getAttribute('role') === 'link'
                        || node.getAttribute('role') === 'textbox'
                        || node.getAttribute('role') === 'checkbox'
                        || node.getAttribute('role') === 'radio'
                        || node.getAttribute('role') === 'tab'
                        || node.getAttribute('role') === 'menuitem'
                        || node.getAttribute('role') === 'switch'
                        || node.getAttribute('tabindex') !== null
                        || style.cursor === 'pointer';

                    var hasDirectText = false;
                    for (var i = 0; i < node.childNodes.length; i++) {
                        if (node.childNodes[i].nodeType === 3 && node.childNodes[i].textContent.trim().length > 0) {
                            hasDirectText = true;
                            break;
                        }
                    }

                    var meaningful = interactive
                        || semanticTags.has(node.tagName)
                        || hasDirectText
                        || node.tagName === 'IMG'
                        || node.tagName === 'INPUT'
                        || node.tagName === 'TEXTAREA'
                        || node.tagName === 'SELECT';

                    if (meaningful) {
                        if (interactive) {
                            node.setAttribute('data-fa-index', idx);
                            idx++;
                        } else {
                            node.setAttribute('data-fa-visible', '1');
                        }
                    }

                    for (var i = 0; i < node.children.length; i++) {
                        walk(node.children[i], depth + 1);
                    }
                }

                document.querySelectorAll('[data-fa-index]').forEach(function(el) { el.removeAttribute('data-fa-index'); });
                document.querySelectorAll('[data-fa-visible]').forEach(function(el) { el.removeAttribute('data-fa-visible'); });
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
