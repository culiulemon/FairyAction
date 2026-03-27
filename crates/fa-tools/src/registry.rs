use crate::actions::ActionContext;
use crate::params::{ActionDef, ActionResult, get_bool, get_f64, get_i64, get_string, parse_action_params};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

pub type ActionHandler = Arc<dyn Fn(ActionContext, Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = ActionResult> + Send>> + Send + Sync>;

pub struct RegisteredAction {
    pub definition: ActionDef,
    pub handler: ActionHandler,
}

pub struct Registry {
    actions: RwLock<HashMap<String, RegisteredAction>>,
    excluded: RwLock<Vec<String>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            actions: RwLock::new(HashMap::new()),
            excluded: RwLock::new(Vec::new()),
        }
    }

    pub async fn register<F, Fut>(&self, definition: ActionDef, handler: F)
    where
        F: Fn(ActionContext, Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ActionResult> + Send + 'static,
    {
        let handler: ActionHandler = Arc::new(move |ctx, params| {
            Box::pin(handler(ctx, params))
        });
        let name = definition.name.clone();
        let mut actions = self.actions.write().await;
        actions.insert(name.clone(), RegisteredAction { definition, handler });
        debug!(action = %name, "Action registered");
    }

    pub async fn exclude(&self, action_name: &str) {
        let mut excluded = self.excluded.write().await;
        if !excluded.contains(&action_name.to_string()) {
            excluded.push(action_name.to_string());
        }
    }

    pub async fn is_excluded(&self, action_name: &str) -> bool {
        let excluded = self.excluded.read().await;
        excluded.contains(&action_name.to_string())
    }

    pub async fn execute(&self, action_name: &str, params: Value, context: ActionContext) -> anyhow::Result<ActionResult> {
        if self.is_excluded(action_name).await {
            return Err(anyhow::anyhow!("Action '{}' is excluded", action_name));
        }

        let actions = self.actions.read().await;
        let action = actions.get(action_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown action: '{}'", action_name))?;

        debug!(action = %action_name, "Executing action");
        let result = (action.handler)(context, params).await;

        if !result.success {
            if let Some(ref err) = result.error {
                warn!(action = %action_name, error = %err, "Action failed");
            }
        }

        Ok(result)
    }

    pub async fn get_action(&self, action_name: &str) -> Option<ActionDef> {
        let actions = self.actions.read().await;
        actions.get(action_name).map(|a| a.definition.clone())
    }

    pub async fn action_names(&self) -> Vec<String> {
        let actions = self.actions.read().await;
        let excluded = self.excluded.read().await;
        actions.keys()
            .filter(|k| !excluded.contains(*k))
            .cloned()
            .collect()
    }

    pub async fn action_definitions(&self) -> Vec<ActionDef> {
        let actions = self.actions.read().await;
        let excluded = self.excluded.read().await;
        actions.values()
            .filter(|a| !excluded.contains(&a.definition.name))
            .map(|a| a.definition.clone())
            .collect()
    }

    pub fn build_json_schema(definitions: &[ActionDef]) -> Value {
        let mut one_of = Vec::new();

        for def in definitions {
            let mut properties = serde_json::Map::new();
            properties.insert(def.name.clone(), def.to_json_schema());

            let required: Vec<Value> = vec![Value::String(def.name.clone())];

            let mut schema = serde_json::Map::new();
            schema.insert("type".to_string(), Value::String("object".to_string()));
            schema.insert("properties".to_string(), Value::Object(properties));
            schema.insert("required".to_string(), Value::Array(required));
            schema.insert("additionalProperties".to_string(), Value::Bool(false));

            one_of.push(Value::Object(schema));
        }

        let mut schema = serde_json::Map::new();
        schema.insert("type".to_string(), Value::String("object".to_string()));
        schema.insert("oneOf".to_string(), Value::Array(one_of));
        schema.insert("additionalProperties".to_string(), Value::Bool(false));

        Value::Object(schema)
    }

    pub async fn get_action_schema(&self) -> Value {
        let defs = self.action_definitions().await;
        Self::build_json_schema(&defs)
    }

    pub async fn build_action_description(&self) -> String {
        let defs = self.action_definitions().await;
        let mut desc = String::from("Available actions:\n");
        for def in &defs {
            desc.push_str(&format!("- {}: {}\n", def.name, def.description));
            for param in &def.params {
                let req = if param.required { "required" } else { "optional" };
                let type_str = param.param_type.to_value_type();
                desc.push_str(&format!("    - {} ({}: {})", param.name, type_str, req));
                if let Some(ref default) = param.default {
                    desc.push_str(&format!(", default={}", default));
                }
                if let Some(ref enums) = param.enum_values {
                    desc.push_str(&format!(", values=[{}]", enums.join(", ")));
                }
                desc.push('\n');
            }
        }
        desc
    }

    pub async fn register_default_actions(&self) {
        self.register_default_nav_actions().await;
        self.register_default_interact_actions().await;
        self.register_default_page_actions().await;
        self.register_default_file_actions().await;
        self.register_default_meta_actions().await;
    }

    async fn register_default_nav_actions(&self) {
        self.register(
            ActionDef::new("navigate", "Navigate to a URL. Use absolute URL including protocol.")
                .param("url", crate::params::ParamType::String, "Target URL to navigate to (include protocol, e.g. https://)")
                .optional_param("new_tab", crate::params::ParamType::Boolean, "Open in new tab", Value::Bool(false))
                .terminates_sequence(),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let url = match get_string(&p, "url") {
                    Some(u) => u,
                    None => return ActionResult::error("Missing required parameter: url"),
                };
                let new_tab = get_bool(&p, "new_tab");

                let result = if new_tab {
                    ctx.session.new_tab(Some(&url)).await
                } else {
                    ctx.session.navigate(&url).await
                };

                match result {
                    Ok(_) => ActionResult::success(format!("Navigated to {}", url)),
                    Err(e) => ActionResult::error(format!("Navigation failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("go_back", "Go back to the previous page in browser history.")
                .terminates_sequence(),
            |ctx, _params| async move {
                match ctx.session.go_back().await {
                    Ok(_) => ActionResult::success("Navigated back"),
                    Err(e) => ActionResult::error(format!("Go back failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("go_forward", "Go forward to the next page in browser history.")
                .terminates_sequence(),
            |ctx, _params| async move {
                match ctx.session.go_forward().await {
                    Ok(_) => ActionResult::success("Navigated forward"),
                    Err(e) => ActionResult::error(format!("Go forward failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("reload", "Reload the current page.")
                .terminates_sequence(),
            |ctx, _params| async move {
                match ctx.session.reload().await {
                    Ok(_) => ActionResult::success("Page reloaded"),
                    Err(e) => ActionResult::error(format!("Reload failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("search", "Search using a search engine. Navigates to search results page.")
                .param("query", crate::params::ParamType::String, "Search query string")
                .enum_param("engine", "Search engine to use", vec!["duckduckgo".to_string(), "google".to_string(), "bing".to_string()], Some("duckduckgo"))
                .terminates_sequence(),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let query = match get_string(&p, "query") {
                    Some(q) => q,
                    None => return ActionResult::error("Missing required parameter: query"),
                };
                let engine = get_string(&p, "engine").unwrap_or_else(|| "duckduckgo".to_string());
                let url = match engine.as_str() {
                    "google" => format!("https://www.google.com/search?q={}", urlencoding(&query)),
                    "bing" => format!("https://www.bing.com/search?q={}", urlencoding(&query)),
                    _ => format!("https://duckduckgo.com/?q={}", urlencoding(&query)),
                };
                match ctx.session.navigate(&url).await {
                    Ok(_) => ActionResult::success(format!("Searched '{}' via {}", query, engine)),
                    Err(e) => ActionResult::error(format!("Search failed: {}", e)),
                }
            },
        ).await;
    }

    async fn register_default_interact_actions(&self) {
        self.register(
            ActionDef::new("click", "Click on an element identified by its index from the DOM state.")
                .param("index", crate::params::ParamType::Integer, "Element index (from DOM state, 0-based)"),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let index = match get_i64(&p, "index") {
                    Some(i) => i as usize,
                    None => return ActionResult::error("Missing required parameter: index"),
                };
                match ctx.session.click_element(index).await {
                    Ok(_) => ActionResult::success(format!("Clicked element [{}]", index)),
                    Err(e) => ActionResult::error(format!("Click failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("input", "Type text into an input element identified by its index.")
                .param("index", crate::params::ParamType::Integer, "Element index (from DOM state, 0-based)")
                .param("text", crate::params::ParamType::String, "Text to type into the element")
                .optional_param("clear", crate::params::ParamType::Boolean, "Clear existing text before typing", Value::Bool(true)),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let index = match get_i64(&p, "index") {
                    Some(i) => i as usize,
                    None => return ActionResult::error("Missing required parameter: index"),
                };
                let text = match get_string(&p, "text") {
                    Some(t) => t,
                    None => return ActionResult::error("Missing required parameter: text"),
                };
                let _clear = get_bool(&p, "clear");
                match ctx.session.type_text(index, &text).await {
                    Ok(_) => ActionResult::success(format!("Typed '{}' into element [{}]", text, index)),
                    Err(e) => ActionResult::error(format!("Input failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("scroll", "Scroll the page or a specific element.")
                .optional_param("direction", crate::params::ParamType::String, "Scroll direction: 'up' or 'down'", Value::String("down".to_string()))
                .optional_param("amount", crate::params::ParamType::Number, "Scroll amount in pixels (default: viewport height)", Value::Number(serde_json::Number::from(0)))
                .optional_param("index", crate::params::ParamType::Integer, "Optional element index to scroll within", Value::Number(serde_json::Number::from(-1))),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let direction = get_string(&p, "direction").unwrap_or_else(|| "down".to_string());
                let amount = get_f64(&p, "amount").unwrap_or(0.0) as u32;
                let pixels = if amount > 0 { amount } else { 800 };
                match ctx.session.scroll(&direction, pixels).await {
                    Ok(_) => ActionResult::success(format!("Scrolled {} by {}px", direction, pixels)),
                    Err(e) => ActionResult::error(format!("Scroll failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("send_keys", "Send keyboard keys or key combinations to the page.")
                .param("keys", crate::params::ParamType::String, "Key name (e.g. 'Enter', 'Escape', 'Tab', 'PageDown') or combo (e.g. 'Control+a', 'Control+c')"),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let keys = match get_string(&p, "keys") {
                    Some(k) => k,
                    None => return ActionResult::error("Missing required parameter: keys"),
                };
                match ctx.session.send_keys(&keys).await {
                    Ok(_) => ActionResult::success(format!("Sent keys: {}", keys)),
                    Err(e) => ActionResult::error(format!("Send keys failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("select_option", "Select an option in a dropdown element.")
                .param("index", crate::params::ParamType::Integer, "Dropdown element index (from DOM state)")
                .param("value", crate::params::ParamType::String, "Value or text of the option to select"),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let index = match get_i64(&p, "index") {
                    Some(i) => i as usize,
                    None => return ActionResult::error("Missing required parameter: index"),
                };
                let value = match get_string(&p, "value") {
                    Some(v) => v,
                    None => return ActionResult::error("Missing required parameter: value"),
                };
                let js = format!(
                    "(function() {{ var el = document.querySelector('[data-fa-index=\"{}\"]'); \
                     if (!el) return 'not found'; \
                     for (var i = 0; i < el.options.length; i++) {{ \
                     if (el.options[i].value === '{}' || el.options[i].text === '{}') {{ \
                     el.selectedIndex = i; el.dispatchEvent(new Event('change', {{bubbles: true}})); return 'selected'; }} }} \
                     return 'not found'; }})()",
                    index, value, value
                );
                match ctx.session.evaluate_js(&js).await {
                    Ok(result) => {
                        let val = result["result"]["value"].as_str().unwrap_or("");
                        if val == "selected" {
                            ActionResult::success(format!("Selected '{}' in dropdown [{}]", value, index))
                        } else {
                            ActionResult::error(format!("Element [{}] not found or option '{}' not available", index, value))
                        }
                    }
                    Err(e) => ActionResult::error(format!("Select option failed: {}", e)),
                }
            },
        ).await;
    }

    async fn register_default_page_actions(&self) {
        self.register(
            ActionDef::new("screenshot", "Take a screenshot of the current page or element.")
                .optional_param("index", crate::params::ParamType::Integer, "Optional element index to screenshot (full page if omitted)", Value::Null),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let _index = get_i64(&p, "index");
                match ctx.session.screenshot().await {
                    Ok(_) => ActionResult::success("Screenshot taken"),
                    Err(e) => ActionResult::error(format!("Screenshot failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("extract", "Extract text content from the current page.")
                .optional_param("query", crate::params::ParamType::String, "Optional query describing what to extract", Value::Null),
            |ctx, _params| async move {
                let js = "document.body.innerText.substring(0, 20000)";
                match ctx.session.evaluate_js(js).await {
                    Ok(result) => {
                        let text = result["result"]["value"].as_str().unwrap_or("").to_string();
                        ActionResult::extracted(text)
                    }
                    Err(e) => ActionResult::error(format!("Extract failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("switch_tab", "Switch to a different browser tab.")
                .param("index", crate::params::ParamType::Integer, "Tab index to switch to (0-based)")
                .terminates_sequence(),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let index = match get_i64(&p, "index") {
                    Some(i) => i as usize,
                    None => return ActionResult::error("Missing required parameter: index"),
                };
                match ctx.session.switch_tab(index).await {
                    Ok(_) => ActionResult::success(format!("Switched to tab {}", index)),
                    Err(e) => ActionResult::error(format!("Switch tab failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("close_tab", "Close a browser tab.")
                .optional_param("index", crate::params::ParamType::Integer, "Tab index to close (current tab if omitted)", Value::Number(serde_json::Number::from(-1))),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let index = get_i64(&p, "index").unwrap_or(-1) as i64;
                if index < 0 {
                    match ctx.session.close_tab(0).await {
                        Ok(_) => ActionResult::success("Current tab closed"),
                        Err(e) => ActionResult::error(format!("Close tab failed: {}", e)),
                    }
                } else {
                    match ctx.session.close_tab(index as usize).await {
                        Ok(_) => ActionResult::success(format!("Tab {} closed", index)),
                        Err(e) => ActionResult::error(format!("Close tab failed: {}", e)),
                    }
                }
            },
        ).await;

        self.register(
            ActionDef::new("new_tab", "Open a new browser tab.")
                .optional_param("url", crate::params::ParamType::String, "URL to open in new tab", Value::Null),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let url = get_string(&p, "url");
                let result = if let Some(u) = url {
                    ctx.session.new_tab(Some(&u)).await
                } else {
                    ctx.session.new_tab(None).await
                };
                match result {
                    Ok(_) => ActionResult::success("Opened new tab"),
                    Err(e) => ActionResult::error(format!("New tab failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("evaluate", "Execute JavaScript code on the current page.")
                .param("code", crate::params::ParamType::String, "JavaScript code to execute")
                .terminates_sequence(),
            |ctx, params| async move {
                let p = parse_action_params(&params);
                let code = match get_string(&p, "code") {
                    Some(c) => c,
                    None => return ActionResult::error("Missing required parameter: code"),
                };
                match ctx.session.evaluate_js(&code).await {
                    Ok(result) => ActionResult::success(format!("Result: {}", result["result"]["value"])),
                    Err(e) => ActionResult::error(format!("Evaluate failed: {}", e)),
                }
            },
        ).await;
    }

    async fn register_default_file_actions(&self) {
        self.register(
            ActionDef::new("save_to_file", "Save text content to a file on disk.")
                .param("file_name", crate::params::ParamType::String, "Name/path of the file to save")
                .param("content", crate::params::ParamType::String, "Content to write to the file"),
            |_ctx, params| async move {
                let p = parse_action_params(&params);
                let file_name = match get_string(&p, "file_name") {
                    Some(f) => f,
                    None => return ActionResult::error("Missing required parameter: file_name"),
                };
                let content = match get_string(&p, "content") {
                    Some(c) => c,
                    None => return ActionResult::error("Missing required parameter: content"),
                };
                match tokio::fs::write(&file_name, &content).await {
                    Ok(_) => ActionResult::success(format!("Saved to {}", file_name)),
                    Err(e) => ActionResult::error(format!("Save failed: {}", e)),
                }
            },
        ).await;

        self.register(
            ActionDef::new("read_file", "Read content from a file on disk.")
                .param("file_name", crate::params::ParamType::String, "Name/path of the file to read"),
            |_ctx, params| async move {
                let p = parse_action_params(&params);
                let file_name = match get_string(&p, "file_name") {
                    Some(f) => f,
                    None => return ActionResult::error("Missing required parameter: file_name"),
                };
                match tokio::fs::read_to_string(&file_name).await {
                    Ok(content) => ActionResult::extracted(content),
                    Err(e) => ActionResult::error(format!("Read failed: {}", e)),
                }
            },
        ).await;
    }

    async fn register_default_meta_actions(&self) {
        self.register(
            ActionDef::new("wait", "Wait for a specified duration before the next action.")
                .optional_param("seconds", crate::params::ParamType::Integer, "Number of seconds to wait (max 30)", Value::Number(serde_json::Number::from(3))),
            |_ctx, _params| async move {
                let p = parse_action_params(&_params);
                let seconds = get_i64(&p, "seconds").unwrap_or(3).min(30).max(1) as u64;
                tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
                ActionResult::success(format!("Waited {} seconds", seconds))
            },
        ).await;

        self.register(
            ActionDef::new("done", "Signal that the task is complete. Include final result text.")
                .param("text", crate::params::ParamType::String, "Final result or summary message")
                .optional_param("success", crate::params::ParamType::Boolean, "Whether the task completed successfully", Value::Bool(true)),
            |_ctx, params| async move {
                let p = parse_action_params(&params);
                let text = get_string(&p, "text").unwrap_or_else(|| "Task complete".to_string());
                ActionResult::done(text)
            },
        ).await;
    }
}

fn urlencoding(s: &str) -> String {
    s.chars().map(|c| {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
            c.to_string()
        } else {
            format!("%{:02X}", c as u8)
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_def_json_schema() {
        let def = ActionDef::new("test_action", "A test action")
            .param("name", crate::params::ParamType::String, "The name parameter")
            .optional_param("count", crate::params::ParamType::Integer, "Count", Value::Number(serde_json::Number::from(1)));

        let schema = def.to_json_schema();
        assert_eq!(schema["type"], Value::String("object".to_string()));
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["name"].is_object());
        assert_eq!(schema["properties"]["name"]["type"], Value::String("string".to_string()));
        assert_eq!(schema["properties"]["count"]["default"], Value::Number(serde_json::Number::from(1)));
    }

    #[test]
    fn test_build_json_schema() {
        let defs = vec![
            ActionDef::new("navigate", "Navigate to URL")
                .param("url", crate::params::ParamType::String, "Target URL"),
            ActionDef::new("click", "Click element")
                .param("index", crate::params::ParamType::Integer, "Element index"),
        ];

        let schema = Registry::build_json_schema(&defs);
        assert_eq!(schema["type"], Value::String("object".to_string()));
        assert!(schema["oneOf"].is_array());
        assert_eq!(schema["oneOf"].as_array().unwrap().len(), 2);
    }
}
