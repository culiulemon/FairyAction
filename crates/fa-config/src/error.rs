use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(String),

    #[error("Failed to write config file: {0}")]
    WriteError(String),

    #[error("Failed to parse config: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Missing required configuration: {0}")]
    MissingConfig(String),
}
