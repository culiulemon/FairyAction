use std::io::Write;

pub struct ActionContext {
    pub domain: String,
    pub action: String,
    pub mode: RunMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Oneshot,
    Persistent,
}

impl ActionContext {
    pub fn progress(&self, percent: u32, status: &str) -> anyhow::Result<()> {
        if self.mode == RunMode::Persistent {
            let msg = format!(
                "progress\x1F{}\x1F{}\x1F{}\n",
                self.domain,
                self.action,
                serde_json::to_string(&serde_json::json!({"进度": percent, "状态": status}))?
            );
            std::io::stdout().write_all(msg.as_bytes())?;
            std::io::stdout().flush()?;
        }
        Ok(())
    }
}
