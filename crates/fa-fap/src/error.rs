#[derive(Debug, thiserror::Error)]
pub enum FapError {
    #[error("manifest error: {0}")]
    Manifest(String),
    #[error("package not found: {0}")]
    PackageNotFound(String),
    #[error("invalid fap file: {0}")]
    InvalidFapFile(String),
    #[error("install error: {0}")]
    Install(String),
    #[error("platform not supported: {0}")]
    PlatformNotSupported(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
