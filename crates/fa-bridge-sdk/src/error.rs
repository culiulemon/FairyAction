#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    #[error("no action specified")]
    NoAction,
    #[error("unknown action: {0}")]
    UnknownAction(String),
    #[error("missing required parameter: {0}")]
    MissingParam(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
