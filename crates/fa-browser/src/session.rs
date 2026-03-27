use crate::cdp::CdpClient;
use crate::events::{BrowserEvent, EventBus};
use crate::profile::BrowserProfile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("Failed to launch browser: {0}")]
    LaunchFailed(String),
    #[error("Failed to connect to browser: {0}")]
    ConnectionFailed(String),
    #[error("CDP command failed: {0}")]
    CdpError(String),
    #[error("Page not found")]
    PageNotFound,
    #[error("Navigation timeout")]
    NavigationTimeout,
    #[error("Screenshot failed: {0}")]
    ScreenshotFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: String,
    pub url: String,
    pub title: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    pub width: u32,
    pub height: u32,
    pub scroll_x: f64,
    pub scroll_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserStateSummary {
    pub url: String,
    pub title: String,
    pub tabs: Vec<TabInfo>,
    pub dom_state: Option<String>,
    pub screenshot: Option<String>,
    pub selector_map: HashMap<usize, String>,
    pub page_info: Option<PageInfo>,
    pub recent_events: Vec<BrowserEvent>,
}

pub struct BrowserSession {
    profile: BrowserProfile,
    cdp_client: CdpClient,
    event_bus: EventBus,
    process_id: Option<u32>,
    port: u16,
}

impl BrowserSession {
    pub async fn new(profile: BrowserProfile) -> Result<Self, BrowserError> {
        let port = Self::find_free_port().map_err(|e| {
            BrowserError::LaunchFailed(format!("Failed to find free port: {}", e))
        })?;

        let chrome_path = profile.chrome_path();
        let args = profile.build_args(port);

        tracing::info!("Launching browser: {} {:?}", chrome_path, args);

        let child = std::process::Command::new(&chrome_path)
            .args(&args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| BrowserError::LaunchFailed(format!("{}: {}", chrome_path, e)))?;

        let process_id = Some(child.id());

        let version_url = format!("http://localhost:{}/json/version", port);
        Self::wait_for_browser(&version_url, 30)
            .await
            .map_err(|e| BrowserError::ConnectionFailed(e.to_string()))?;

        let page_ws_url = Self::get_page_ws_url(port)
            .await
            .map_err(|e| BrowserError::ConnectionFailed(e.to_string()))?;

        tracing::info!("Connecting to page target: {}", page_ws_url);

        let cdp_client = CdpClient::connect(&page_ws_url)
            .await
            .map_err(|e| BrowserError::ConnectionFailed(e.to_string()))?;

        cdp_client
            .execute_unit("Page.enable", serde_json::json!({}))
            .await
            .map_err(|e| BrowserError::CdpError(format!("Failed to enable Page domain: {}", e)))?;

        cdp_client
            .execute_unit("Runtime.enable", serde_json::json!({}))
            .await
            .map_err(|e| BrowserError::CdpError(format!("Failed to enable Runtime domain: {}", e)))?;

        let event_bus = EventBus::new(256);

        Ok(Self {
            profile,
            cdp_client,
            event_bus,
            process_id,
            port,
        })
    }

    async fn get_page_ws_url(port: u16) -> Result<String, anyhow::Error> {
        let list_url = format!("http://localhost:{}/json/list", port);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        for _ in 0..30 {
            let resp = client.get(&list_url).send().await?;
            let targets: Vec<serde_json::Value> = resp.json().await?;

            if let Some(page) = targets.iter().find(|t| {
                t.get("type").and_then(|v| v.as_str()) == Some("page")
            }) {
                if let Some(ws_url) = page["webSocketDebuggerUrl"].as_str() {
                    return Ok(ws_url.to_string());
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        anyhow::bail!("No page target found after waiting")
    }

    fn find_free_port() -> Result<u16, std::io::Error> {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        Ok(port)
    }

    async fn wait_for_browser(
        cdp_url: &str,
        max_wait_secs: u64,
    ) -> Result<String, anyhow::Error> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()?;

        let start = std::time::Instant::now();
        let mut last_error = String::new();
        loop {
            if start.elapsed().as_secs() > max_wait_secs {
                anyhow::bail!(
                    "Browser did not start within {} seconds. Last error: {}. \
                     Make sure no other Chrome instance is using the same user-data-dir, \
                     or try closing all Chrome windows first.",
                    max_wait_secs, last_error
                );
            }
            match client.get(cdp_url).send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        last_error = format!("HTTP {}", resp.status());
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }
                    let data: serde_json::Value = resp.json().await?;
                    if let Some(ws_url) = data["webSocketDebuggerUrl"].as_str() {
                        return Ok(ws_url.to_string());
                    }
                    last_error = format!("No webSocketDebuggerUrl in response: {}", data);
                }
                Err(e) => {
                    last_error = e.to_string();
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub fn profile(&self) -> &BrowserProfile {
        &self.profile
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn navigate(&self, url: &str) -> Result<(), BrowserError> {
        self.event_bus.publish(BrowserEvent::Navigate {
            url: url.to_string(),
        });

        self.cdp_client
            .execute("Page.navigate", serde_json::json!({ "url": url }))
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))?;

        self.cdp_client
            .wait_for_event("Page.loadEventFired", std::time::Duration::from_secs(30))
            .await
            .map_err(|_| BrowserError::NavigationTimeout)?;

        self.event_bus.publish(BrowserEvent::PageLoaded {
            url: url.to_string(),
        });

        tracing::info!("Navigated to {}", url);
        Ok(())
    }

    pub async fn go_back(&self) -> Result<(), BrowserError> {
        self.cdp_client
            .execute_unit("Page.goBack", serde_json::json!({}))
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))
    }

    pub async fn go_forward(&self) -> Result<(), BrowserError> {
        self.cdp_client
            .execute_unit("Page.goForward", serde_json::json!({}))
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))
    }

    pub async fn reload(&self) -> Result<(), BrowserError> {
        self.cdp_client
            .execute_unit("Page.reload", serde_json::json!({}))
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))
    }

    pub async fn screenshot(&self) -> Result<String, BrowserError> {
        self.event_bus.publish(BrowserEvent::Screenshot);

        let result = self
            .cdp_client
            .execute(
                "Page.captureScreenshot",
                serde_json::json!({
                    "format": "png",
                    "quality": 80
                }),
            )
            .await
            .map_err(|e| BrowserError::ScreenshotFailed(e.to_string()))?;

        let data = result["data"]
            .as_str()
            .ok_or_else(|| BrowserError::ScreenshotFailed("No screenshot data".into()))?;

        Ok(data.to_string())
    }

    pub async fn get_url(&self) -> Result<String, BrowserError> {
        let result = self
            .cdp_client
            .execute(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": "window.location.href",
                    "returnByValue": true
                }),
            )
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))?;

        Ok(result["result"]["value"].as_str().unwrap_or("").to_string())
    }

    pub async fn get_title(&self) -> Result<String, BrowserError> {
        let result = self
            .cdp_client
            .execute(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": "document.title",
                    "returnByValue": true
                }),
            )
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))?;

        Ok(result["result"]["value"].as_str().unwrap_or("").to_string())
    }

    pub async fn evaluate_js(
        &self,
        expression: &str,
    ) -> Result<serde_json::Value, BrowserError> {
        let result = self
            .cdp_client
            .execute(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true
                }),
            )
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))?;

        Ok(result)
    }

    pub async fn click_element(&self, index: usize) -> Result<(), BrowserError> {
        self.event_bus.publish(BrowserEvent::Click { index });
        let js = format!(
            r#"(function() {{ var el = document.querySelector('[data-fa-index="{}"]'); if (el) {{ el.click(); return true; }} return false; }})()"#,
            index
        );
        let result = self.evaluate_js(&js).await?;
        let success = result["result"]["value"].as_bool().unwrap_or(false);
        if !success {
            return Err(BrowserError::CdpError(format!(
                "Element with index {} not found",
                index
            )));
        }
        Ok(())
    }

    pub async fn click_coordinate(&self, x: f64, y: f64) -> Result<(), BrowserError> {
        self.event_bus.publish(BrowserEvent::ClickCoordinate { x, y });

        self.cdp_client
            .execute(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": "mousePressed",
                    "x": x,
                    "y": y,
                    "button": "left",
                    "clickCount": 1
                }),
            )
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))?;

        self.cdp_client
            .execute(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": "mouseReleased",
                    "x": x,
                    "y": y,
                    "button": "left",
                    "clickCount": 1
                }),
            )
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))?;

        Ok(())
    }

    pub async fn type_text(&self, index: usize, text: &str) -> Result<(), BrowserError> {
        self.event_bus.publish(BrowserEvent::TypeText {
            index,
            text: text.to_string(),
        });

        let js = format!(
            r#"(function() {{ var el = document.querySelector('[data-fa-index="{}"]'); if (el) {{ el.focus(); el.value = ''; return true; }} return false; }})()"#,
            index
        );
        let result = self.evaluate_js(&js).await?;
        let found = result["result"]["value"].as_bool().unwrap_or(false);
        if !found {
            return Err(BrowserError::CdpError(format!(
                "Element with index {} not found",
                index
            )));
        }

        self.cdp_client
            .execute(
                "Input.insertText",
                serde_json::json!({ "text": text }),
            )
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))?;

        Ok(())
    }

    pub async fn scroll(&self, direction: &str, amount: u32) -> Result<(), BrowserError> {
        self.event_bus.publish(BrowserEvent::Scroll {
            direction: direction.to_string(),
            amount,
        });

        let pixels = match direction {
            "up" => -(amount as i64),
            _ => amount as i64,
        };

        let js = format!("window.scrollBy(0, {})", pixels);
        self.evaluate_js(&js).await?;
        Ok(())
    }

    pub async fn send_keys(&self, keys: &str) -> Result<(), BrowserError> {
        self.event_bus.publish(BrowserEvent::SendKeys {
            keys: keys.to_string(),
        });

        self.cdp_client
            .execute_unit("Input.insertText", serde_json::json!({ "text": keys }))
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))
    }

    pub async fn get_tabs(&self) -> Result<Vec<TabInfo>, BrowserError> {
        let http_url = format!("http://localhost:{}/json/list", self.port);
        let client = reqwest::Client::new();
        let resp = client
            .get(&http_url)
            .send()
            .await
            .map_err(|e| BrowserError::ConnectionFailed(e.to_string()))?;

        let tabs: Vec<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))?;

        let mut result = Vec::new();
        for tab in tabs {
            if tab.get("type").and_then(|t| t.as_str()) == Some("page") {
                result.push(TabInfo {
                    id: tab["id"].as_str().unwrap_or("").to_string(),
                    url: tab["url"].as_str().unwrap_or("").to_string(),
                    title: tab["title"].as_str().unwrap_or("").to_string(),
                    is_active: false,
                });
            }
        }

        if let Some(first) = result.first_mut() {
            first.is_active = true;
        }

        Ok(result)
    }

    pub async fn switch_tab(&self, index: usize) -> Result<(), BrowserError> {
        self.event_bus.publish(BrowserEvent::TabSwitch { index });

        let tabs = self.get_tabs().await?;
        let target_tab = tabs.get(index).ok_or(BrowserError::PageNotFound)?;
        let target_id = target_tab.id.clone();

        self.cdp_client
            .execute_unit(
                "Target.activateTarget",
                serde_json::json!({ "targetId": target_id }),
            )
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))
    }

    pub async fn close_tab(&self, index: usize) -> Result<(), BrowserError> {
        self.event_bus.publish(BrowserEvent::TabClose { index });

        let tabs = self.get_tabs().await?;
        let target_tab = tabs.get(index).ok_or(BrowserError::PageNotFound)?;
        let target_id = target_tab.id.clone();

        self.cdp_client
            .execute_unit(
                "Target.closeTarget",
                serde_json::json!({ "targetId": target_id }),
            )
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))
    }

    pub async fn new_tab(&self, url: Option<&str>) -> Result<(), BrowserError> {
        self.event_bus.publish(BrowserEvent::TabNew);

        let navigate_url = url.unwrap_or("about:blank");

        self.cdp_client
            .execute_unit(
                "Target.createTarget",
                serde_json::json!({ "url": navigate_url }),
            )
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))
    }

    pub async fn execute_cdp(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, BrowserError> {
        self.cdp_client
            .execute(method, params)
            .await
            .map_err(|e| BrowserError::CdpError(e.to_string()))
    }

    pub async fn get_browser_state_summary(&self) -> Result<BrowserStateSummary, BrowserError> {
        let url = self.get_url().await.unwrap_or_default();
        let title = self.get_title().await.unwrap_or_default();
        let tabs = self.get_tabs().await.unwrap_or_default();

        let page_info = self
            .evaluate_js(
                r#"(function() { return { width: window.innerWidth, height: window.innerHeight, scrollX: window.scrollX, scrollY: window.scrollY }; })()"#,
            )
            .await
            .ok()
            .and_then(|v| serde_json::from_value(v["result"]["value"].clone()).ok());

        Ok(BrowserStateSummary {
            url,
            title,
            tabs,
            dom_state: None,
            screenshot: None,
            selector_map: HashMap::new(),
            page_info,
            recent_events: Vec::new(),
        })
    }

    pub async fn close(mut self) {
        if let Some(pid) = self.process_id.take() {
            tracing::info!("Closing browser process (PID: {})", pid);
            Self::kill_process(pid);
        }
    }

    fn kill_process(pid: u32) {
        let _ = std::process::Command::new("taskkill")
            .args(&["/F", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        if let Some(pid) = self.process_id.take() {
            tracing::info!("Dropping browser session, killing PID: {}", pid);
            Self::kill_process(pid);
        }
    }
}
