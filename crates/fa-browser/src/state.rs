use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserStateSummary {
    pub url: String,
    pub title: String,
    pub dom_state: String,
    pub screenshot: Option<String>,
    pub tabs: Vec<TabInfo>,
    pub page_info: Option<PageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: String,
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    pub width: u32,
    pub height: u32,
    pub scroll_x: f64,
    pub scroll_y: f64,
}

impl Default for BrowserStateSummary {
    fn default() -> Self {
        Self {
            url: String::new(),
            title: String::new(),
            dom_state: String::new(),
            screenshot: None,
            tabs: Vec::new(),
            page_info: None,
        }
    }
}
