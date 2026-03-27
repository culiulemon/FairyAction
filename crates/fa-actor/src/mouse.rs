use fa_browser::session::BrowserSession;

pub struct Mouse<'a> {
    session: &'a BrowserSession,
}

impl<'a> Mouse<'a> {
    pub fn new(session: &'a BrowserSession) -> Self {
        Self { session }
    }

    pub async fn move_to(&self, x: f64, y: f64) -> anyhow::Result<()> {
        self.session
            .execute_cdp(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": "mouseMoved",
                    "x": x,
                    "y": y
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn click(&self, x: f64, y: f64) -> anyhow::Result<()> {
        self.session.click_coordinate(x, y).await.map_err(Into::into)
    }

    pub async fn down(&self, x: f64, y: f64) -> anyhow::Result<()> {
        self.session
            .execute_cdp(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": "mousePressed",
                    "x": x,
                    "y": y,
                    "button": "left",
                    "clickCount": 1
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn up(&self, x: f64, y: f64) -> anyhow::Result<()> {
        self.session
            .execute_cdp(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": "mouseReleased",
                    "x": x,
                    "y": y,
                    "button": "left",
                    "clickCount": 1
                }),
            )
            .await?;
        Ok(())
    }
}
