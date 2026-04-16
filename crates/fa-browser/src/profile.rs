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
    pub profile_name: Option<String>,
    pub app_title: Option<String>,
    pub extra_args: Vec<String>,
}

impl Default for BrowserProfile {
    fn default() -> Self {
        Self {
            headless: false,
            viewport_width: 1280,
            viewport_height: 720,
            proxy: None,
            chrome_path: None,
            user_data_dir: None,
            profile_name: Some("Fairy".to_string()),
            app_title: Some("FairyBrowser".to_string()),
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
            profile_name: config.profile_name.clone(),
            app_title: config.app_title.clone(),
            extra_args: Vec::new(),
        }
    }

    pub fn find_chrome_path() -> Option<String> {
        #[cfg(target_os = "windows")]
        {
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
        }

        #[cfg(target_os = "linux")]
        {
            let candidates: &[&str] = &[
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/google-chrome-beta",
                "/usr/bin/chromium-browser",
                "/usr/bin/chromium",
                "/snap/bin/chromium",
                "/snap/bin/google-chrome",
                "/usr/bin/brave-browser",
                "/usr/bin/brave",
                "/usr/bin/microsoft-edge",
                "/usr/bin/microsoft-edge-stable",
            ];

            for path in candidates {
                if std::path::Path::new(path).exists() {
                    return Some(path.to_string());
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

            if let Ok(output) = Command::new("which").arg("chromium-browser").output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout);
                    if let Some(first_line) = path.lines().next() {
                        if !first_line.trim().is_empty() {
                            return Some(first_line.trim().to_string());
                        }
                    }
                }
            }

            if let Ok(output) = Command::new("which").arg("chromium").output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout);
                    if let Some(first_line) = path.lines().next() {
                        if !first_line.trim().is_empty() {
                            return Some(first_line.trim().to_string());
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let candidates: &[&str] = &[
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
                "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            ];

            for path in candidates {
                if std::path::Path::new(path).exists() {
                    return Some(path.to_string());
                }
            }

            if let Ok(output) = Command::new("mdfind")
                .args(&["kMDKind", "==", "com.google.chrome"])
                .output()
            {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout);
                    if let Some(first_line) = path.lines().next() {
                        if !first_line.trim().is_empty() {
                            let chrome_path = format!(
                                "{}/Contents/MacOS/Google Chrome",
                                first_line.trim()
                            );
                            if std::path::Path::new(&chrome_path).exists() {
                                return Some(chrome_path);
                            }
                        }
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

    pub fn user_data_dir_resolved(&self) -> std::path::PathBuf {
        if let Some(udir) = &self.user_data_dir {
            std::path::PathBuf::from(udir)
        } else {
            std::env::temp_dir().join("FairyBrowser")
        }
    }

    pub fn prepare_profile_dir(&self) {
        let profile_name = match &self.profile_name {
            Some(name) if !name.is_empty() => name.clone(),
            _ => return,
        };

        let base_dir = self.user_data_dir_resolved();
        let profile_dir = base_dir.join(&profile_name);

        let _ = std::fs::create_dir_all(&profile_dir);

        self.update_local_state(&base_dir, &profile_name);
        self.update_preferences(&profile_dir, &profile_name);
    }

    fn update_local_state(&self, base_dir: &std::path::Path, profile_name: &str) {
        let local_state_path = base_dir.join("Local State");
        let mut local_state: serde_json::Value = if local_state_path.exists() {
            std::fs::read_to_string(&local_state_path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        let needs_update = local_state
            .get("profile")
            .and_then(|p| p.get("info_cache"))
            .and_then(|ic| ic.get(profile_name))
            .and_then(|entry| entry.get("name"))
            .and_then(|n| n.as_str())
            .map(|n| n == profile_name)
            .unwrap_or(false);

        if !needs_update {
            if let Some(obj) = local_state.as_object_mut() {
                if !obj.contains_key("profile") {
                    obj.insert("profile".to_string(), serde_json::json!({}));
                }
                if let Some(profile) = obj.get_mut("profile").and_then(|p| p.as_object_mut()) {
                    if !profile.contains_key("info_cache") {
                        profile.insert("info_cache".to_string(), serde_json::json!({}));
                    }
                    if let Some(cache) = profile.get_mut("info_cache").and_then(|ic| ic.as_object_mut()) {
                        if !cache.contains_key(profile_name) {
                            cache.insert(profile_name.to_string(), serde_json::json!({}));
                        }
                        if let Some(entry) = cache.get_mut(profile_name).and_then(|e| e.as_object_mut()) {
                            entry.insert("name".to_string(), serde_json::Value::String(profile_name.to_string()));
                            entry.insert("is_using_default_name".to_string(), serde_json::Value::Bool(false));
                            if !entry.contains_key("avatar_index") {
                                entry.insert("avatar_index".to_string(), serde_json::Value::Number(0.into()));
                            }
                        }
                    }
                }
                let _ = std::fs::write(&local_state_path, serde_json::to_string_pretty(&local_state).unwrap_or_default());
            }
        }
    }

    fn update_preferences(&self, profile_dir: &std::path::Path, profile_name: &str) {
        let prefs_path = profile_dir.join("Preferences");
        if prefs_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&prefs_path) {
                if let Ok(mut prefs) = serde_json::from_str::<serde_json::Value>(&content) {
                    let already_correct = prefs.get("profile")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|n| n == profile_name)
                        .unwrap_or(false);
                    if already_correct {
                        return;
                    }
                    if let Some(obj) = prefs.as_object_mut() {
                        if !obj.contains_key("profile") {
                            obj.insert("profile".to_string(), serde_json::json!({}));
                        }
                        if let Some(profile_obj) = obj.get_mut("profile").and_then(|p| p.as_object_mut()) {
                            profile_obj.insert("name".to_string(), serde_json::Value::String(profile_name.to_string()));
                            profile_obj.insert("is_using_default_name".to_string(), serde_json::Value::Bool(false));
                        }
                        let _ = std::fs::write(&prefs_path, serde_json::to_string_pretty(&prefs).unwrap_or_default());
                    }
                    return;
                }
            }
        }

        let default_prefs = serde_json::json!({
            "profile": {
                "name": profile_name,
                "is_using_default_name": false,
                "avatar_index": 0,
            }
        });
        let _ = std::fs::write(&prefs_path, serde_json::to_string_pretty(&default_prefs).unwrap_or_default());
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
            let temp_dir = std::env::temp_dir().join("FairyBrowser");
            args.push(format!("--user-data-dir={}", temp_dir.display()));
        }

        if let Some(profile) = &self.profile_name {
            args.push(format!("--profile-directory={}", profile));
        }

        if let Some(proxy) = &self.proxy {
            args.push(format!("--proxy-server={}", proxy));
        }

        args.extend(self.extra_args.clone());
        args
    }
}
