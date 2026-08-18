use thiserror::Error;

#[derive(Debug, Error)]
pub enum BlnkError {
    #[error("config error: {0}")]
    Config(String),

    #[error("identity error: {0}")]
    Identity(String),

    #[error("signaling error: {0}")]
    Signaling(String),

    #[error("peer error: {0}")]
    Peer(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("stream error: {0}")]
    Stream(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
