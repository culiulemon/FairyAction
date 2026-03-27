use fa_browser::session::BrowserSession;

pub struct Element<'a> {
    session: &'a BrowserSession,
    index: usize,
}

impl<'a> Element<'a> {
    pub fn new(session: &'a BrowserSession, index: usize) -> Self {
        Self { session, index }
    }

    pub async fn click(&self) -> anyhow::Result<()> {
        self.session.click_element(self.index).await.map_err(Into::into)
    }

    pub async fn fill(&self, text: &str) -> anyhow::Result<()> {
        self.session.type_text(self.index, text).await.map_err(Into::into)
    }

    pub async fn get_text(&self) -> anyhow::Result<String> {
        let js = format!(
            r#"(function() {{ var el = document.querySelector('[data-fa-index="{}"]'); return el ? el.textContent.trim() : ''; }})()"#,
            self.index
        );
        let result = self.session.evaluate_js(&js).await?;
        Ok(result["result"]["value"].as_str().unwrap_or("").to_string())
    }

    pub async fn get_attribute(&self, name: &str) -> anyhow::Result<Option<String>> {
        let js = format!(
            r#"(function() {{ var el = document.querySelector('[data-fa-index="{}"]'); return el ? el.getAttribute("{}") : null; }})()"#,
            self.index, name
        );
        let result = self.session.evaluate_js(&js).await?;
        let val = result["result"]["value"].as_str();
        if val == Some("null") || val == Some("") {
            Ok(None)
        } else {
            Ok(val.map(|s| s.to_string()))
        }
    }

    pub async fn is_visible(&self) -> anyhow::Result<bool> {
        let js = format!(
            r#"(function() {{ var el = document.querySelector('[data-fa-index="{}"]'); if (!el) return false; var rect = el.getBoundingClientRect(); return rect.width > 0 && rect.height > 0 && getComputedStyle(el).display !== 'none' && getComputedStyle(el).visibility !== 'hidden'; }})()"#,
            self.index
        );
        let result = self.session.evaluate_js(&js).await?;
        Ok(result["result"]["value"].as_bool().unwrap_or(false))
    }
}
