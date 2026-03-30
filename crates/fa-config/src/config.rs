use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadFailed(String),
    #[error("Failed to write config file: {0}")]
    WriteFailed(String),
    #[error("Failed to parse config: {0}")]
    ParseFailed(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub browser: BrowserConfig,
    #[serde(default = "default_search_engine")]
    pub default_search_engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    #[serde(default = "default_headless")]
    pub headless: bool,
    #[serde(default = "default_viewport_width")]
    pub viewport_width: u32,
    #[serde(default = "default_viewport_height")]
    pub viewport_height: u32,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub chrome_path: Option<String>,
    #[serde(default)]
    pub user_data_dir: Option<String>,
    #[serde(default = "default_profile_name")]
    pub profile_name: Option<String>,
    #[serde(default = "default_app_title")]
    pub app_title: Option<String>,
}

fn default_headless() -> bool { false }
fn default_viewport_width() -> u32 { 1280 }
fn default_viewport_height() -> u32 { 720 }
fn default_profile_name() -> Option<String> { Some("Fairy".to_string()) }
fn default_app_title() -> Option<String> { Some("FairyBrowser".to_string()) }
fn default_search_engine() -> String { "bing".to_string() }

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            headless: default_headless(),
            viewport_width: default_viewport_width(),
            viewport_height: default_viewport_height(),
            proxy: None,
            chrome_path: None,
            user_data_dir: None,
            profile_name: default_profile_name(),
            app_title: default_app_title(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            browser: BrowserConfig::default(),
            default_search_engine: default_search_engine(),
        }
    }
}

fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("fairy-action"))
}

fn config_file_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}

impl Config {
    pub fn config_dir() -> Option<PathBuf> {
        config_dir()
    }

    pub fn config_file_path() -> Option<PathBuf> {
        config_file_path()
    }

    pub fn load_from_file() -> Result<Self, ConfigError> {
        let path = config_file_path().ok_or_else(|| {
            ConfigError::ReadFailed("Cannot determine config directory".into())
        })?;
        if !path.exists() {
            tracing::debug!("Config file not found at {:?}, using defaults", path);
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            ConfigError::ReadFailed(format!("{}: {}", path.display(), e))
        })?;
        let config: Config = serde_json::from_str(&content)?;
        tracing::info!("Loaded config from {:?}", path);
        Ok(config)
    }

    pub fn save_to_file(&self) -> Result<(), ConfigError> {
        let dir = config_dir().ok_or_else(|| {
            ConfigError::WriteFailed("Cannot determine config directory".into())
        })?;
        std::fs::create_dir_all(&dir).map_err(|e| {
            ConfigError::WriteFailed(format!("Failed to create {:?}: {}", dir, e))
        })?;
        let path = dir.join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content).map_err(|e| {
            ConfigError::WriteFailed(format!("{}: {}", path.display(), e))
        })?;
        tracing::info!("Saved config to {:?}", path);
        Ok(())
    }

    pub fn merge_env(mut self) -> Self {
        if let Ok(v) = std::env::var("FA_BROWSER_HEADLESS") {
            self.browser.headless = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("FA_BROWSER_VIEWPORT_WIDTH") {
            if let Ok(n) = v.parse() { self.browser.viewport_width = n; }
        }
        if let Ok(v) = std::env::var("FA_BROWSER_VIEWPORT_HEIGHT") {
            if let Ok(n) = v.parse() { self.browser.viewport_height = n; }
        }
        if let Ok(v) = std::env::var("FA_BROWSER_PROXY") {
            self.browser.proxy = Some(v);
        }
        if let Ok(v) = std::env::var("FA_BROWSER_CHROME_PATH") {
            self.browser.chrome_path = Some(v);
        }
        if let Ok(v) = std::env::var("FA_BROWSER_PROFILE_NAME") {
            self.browser.profile_name = Some(v);
        }
        if let Ok(v) = std::env::var("FA_BROWSER_APP_TITLE") {
            self.browser.app_title = Some(v);
        }
        if let Ok(v) = std::env::var("FA_DEFAULT_SEARCH_ENGINE") {
            self.default_search_engine = v;
        }
        self
    }

    pub fn from_env() -> Self {
        Self::default().merge_env()
    }

    pub fn load() -> Self {
        let _ = dotenvy::dotenv();
        let config = Self::load_from_file().unwrap_or_default();
        config.merge_env()
    }

    pub fn load_from_path(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ConfigError::ReadFailed(format!("{}: {}", path, e))
        })?;
        let mut config: Config = serde_json::from_str(&content)?;
        config = config.merge_env();
        tracing::info!("Loaded config from {}", path);
        Ok(config)
    }

    pub fn save_to_path(&self, path: &str) -> Result<(), ConfigError> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content).map_err(|e| {
            ConfigError::WriteFailed(format!("{}: {}", path, e))
        })?;
        tracing::info!("Saved config to {}", path);
        Ok(())
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        let parts: Vec<&str> = key.split('.').collect();
        match parts.as_slice() {
            ["browser", sub_key] => match *sub_key {
                "headless" => self.browser.headless = value.parse::<bool>().map_err(|e| ConfigError::ParseFailed(e.to_string()))?,
                "viewport_width" => self.browser.viewport_width = value.parse::<u32>().map_err(|e| ConfigError::ParseFailed(e.to_string()))?,
                "viewport_height" => self.browser.viewport_height = value.parse::<u32>().map_err(|e| ConfigError::ParseFailed(e.to_string()))?,
                "chrome_path" => self.browser.chrome_path = Some(value.to_string()),
                "proxy" => self.browser.proxy = Some(value.to_string()),
                "user_data_dir" => self.browser.user_data_dir = Some(value.to_string()),
                "profile_name" => self.browser.profile_name = Some(value.to_string()),
                "app_title" => self.browser.app_title = Some(value.to_string()),
                _ => return Err(ConfigError::ParseFailed(format!("Unknown key: {}", key))),
            },
            ["default_search_engine"] => self.default_search_engine = value.to_string(),
            _ => return Err(ConfigError::ParseFailed(format!("Unknown key: {}", key))),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.browser.headless);
        assert_eq!(config.browser.viewport_width, 1280);
    }

    #[test]
    fn test_merge_env() {
        unsafe {
            std::env::set_var("FA_BROWSER_HEADLESS", "false");
            std::env::set_var("FA_BROWSER_VIEWPORT_WIDTH", "1920");
        }

        let config = Config::from_env();
        assert!(!config.browser.headless);
        assert_eq!(config.browser.viewport_width, 1920);

        unsafe {
            std::env::remove_var("FA_BROWSER_HEADLESS");
            std::env::remove_var("FA_BROWSER_VIEWPORT_WIDTH");
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.browser.viewport_width, config.browser.viewport_width);
    }

    #[test]
    fn test_partial_deserialize() {
        let json = r#"{"browser":{"headless":false,"viewport_width":1920}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(!config.browser.headless);
        assert_eq!(config.browser.viewport_width, 1920);
    }
}
