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
}
