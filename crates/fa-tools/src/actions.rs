use crate::params::ActionStateAfter;
use fa_browser::session::BrowserSession;
use serde_json::Value;
use std::sync::Arc;

pub struct ActionContext {
    pub session: Arc<BrowserSession>,
    pub page_url: String,
    pub page_title: String,
    pub extra: Option<Value>,
}

impl ActionContext {
    pub fn new(session: Arc<BrowserSession>, page_url: String, page_title: String) -> Self {
        Self {
            session,
            page_url,
            page_title,
            extra: None,
        }
    }

    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = Some(extra);
        self
    }

    pub async fn capture_state_after(&self, url_before: &str, tab_count_before: usize) -> ActionStateAfter {
        let url_after = self.session.get_url().await.unwrap_or_default();
        let title_after = self.session.get_title().await.unwrap_or_default();
        let tabs_after = self.session.get_tabs().await.unwrap_or_default();
        let tab_count_after = tabs_after.len();
        let new_tab_opened = tab_count_after > tab_count_before;
        let navigation_occurred = url_after != url_before && !url_after.is_empty();

        ActionStateAfter {
            url: Some(url_after),
            title: Some(title_after),
            tab_count: Some(tab_count_after),
            new_tab_opened: if new_tab_opened { Some(true) } else { None },
            navigation_occurred: if navigation_occurred { Some(true) } else { None },
            screenshot: None,
        }
    }
}
