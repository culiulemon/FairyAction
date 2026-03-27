use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub headless: bool,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub proxy: Option<String>,
    pub chrome_path: Option<String>,
    pub user_data_dir: Option<String>,
    pub extra_args: Vec<String>,
}

impl Default for BrowserProfile {
    fn default() -> Self {
        Self {
            headless: true,
            viewport_width: 1280,
            viewport_height: 720,
            proxy: None,
            chrome_path: None,
            user_data_dir: None,
            extra_args: Vec::new(),
        }
    }
}

impl BrowserProfile {
    pub fn from_config(config: &fa_config::config::BrowserConfig) -> Self {
        Self {
            headless: config.headless,
            viewport_width: config.viewport_width,
            viewport_height: config.viewport_height,
            proxy: config.proxy.clone(),
            chrome_path: config.chrome_path.clone(),
            user_data_dir: config.user_data_dir.clone(),
            extra_args: Vec::new(),
        }
    }

    pub fn find_chrome_path() -> Option<String> {
        let candidates: &[&str] = &[
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        ];

        for path in candidates {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        if let Ok(output) = Command::new("where").arg("chrome").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = path.lines().next() {
                    if !first_line.trim().is_empty() {
                        return Some(first_line.trim().to_string());
                    }
                }
            }
        }

        if let Ok(output) = Command::new("which").arg("google-chrome").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = path.lines().next() {
                    if !first_line.trim().is_empty() {
                        return Some(first_line.trim().to_string());
                    }
                }
            }
        }

        None
    }

    pub fn chrome_path(&self) -> String {
        self.chrome_path.clone()
            .or_else(Self::find_chrome_path)
            .unwrap_or_else(|| "chrome".to_string())
    }

    pub fn build_args(&self, remote_debugging_port: u16) -> Vec<String> {
        let mut args = vec![
            format!("--remote-debugging-port={}", remote_debugging_port),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--disable-background-networking".to_string(),
            "--disable-client-side-phishing-detection".to_string(),
            "--disable-default-apps".to_string(),
            "--disable-hang-monitor".to_string(),
            "--disable-popup-blocking".to_string(),
            "--disable-prompt-on-repost".to_string(),
            "--disable-sync".to_string(),
            "--metrics-recording-only".to_string(),
            "--safebrowsing-disable-auto-update".to_string(),
            format!("--window-size={},{}", self.viewport_width, self.viewport_height),
        ];

        if self.headless {
            args.push("--headless=new".to_string());
        }

        if let Some(udir) = &self.user_data_dir {
            args.push(format!("--user-data-dir={}", udir));
        } else {
            let temp_dir = std::env::temp_dir().join("fairy-action-chrome-profile");
            args.push(format!("--user-data-dir={}", temp_dir.display()));
        }

        if let Some(proxy) = &self.proxy {
            args.push(format!("--proxy-server={}", proxy));
        }

        args.extend(self.extra_args.clone());
        args
    }
}
