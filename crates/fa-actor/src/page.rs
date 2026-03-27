use crate::element::Element;
use crate::mouse::Mouse;
use fa_browser::session::BrowserSession;

pub struct Page<'a> {
    session: &'a BrowserSession,
    mouse: Mouse<'a>,
}

impl<'a> Page<'a> {
    pub fn new(session: &'a BrowserSession) -> Self {
        Self {
            session,
            mouse: Mouse::new(session),
        }
    }

    pub async fn evaluate(&self, expression: &str) -> anyhow::Result<serde_json::Value> {
        self.session.evaluate_js(expression).await.map_err(Into::into)
    }

    pub async fn screenshot(&self) -> anyhow::Result<String> {
        self.session.screenshot().await.map_err(Into::into)
    }

    pub async fn navigate(&self, url: &str) -> anyhow::Result<()> {
        self.session.navigate(url).await.map_err(Into::into)
    }

    pub async fn go_back(&self) -> anyhow::Result<()> {
        self.session.go_back().await.map_err(Into::into)
    }

    pub async fn reload(&self) -> anyhow::Result<()> {
        self.session.reload().await.map_err(Into::into)
    }

    pub async fn get_element(&self, index: usize) -> anyhow::Result<Element<'a>> {
        Ok(Element::new(self.session, index))
    }

    pub fn mouse(&self) -> &Mouse<'a> {
        &self.mouse
    }

    pub async fn url(&self) -> anyhow::Result<String> {
        self.session.get_url().await.map_err(Into::into)
    }

    pub async fn title(&self) -> anyhow::Result<String> {
        self.session.get_title().await.map_err(Into::into)
    }
}
