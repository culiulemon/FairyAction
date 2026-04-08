#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("invalid bridge protocol: {0}")]
    InvalidProtocol(String),
    #[error("invalid message type: {0}")]
    InvalidMessageType(String),
    #[error("invalid json payload: {0}")]
    InvalidPayload(String),
    #[error("frame length mismatch: expected {expected}, got {actual}")]
    FrameLengthMismatch { expected: usize, actual: usize },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, BridgeError>;
