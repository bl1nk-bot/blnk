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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_blnk_error_display_formatting() {
        let config_err = BlnkError::Config("invalid setting".into());
        assert_eq!(format!("{config_err}"), "config error: invalid setting");

        let identity_err = BlnkError::Identity("missing key".into());
        assert_eq!(format!("{identity_err}"), "identity error: missing key");

        let signaling_err = BlnkError::Signaling("connection dropped".into());
        assert_eq!(
            format!("{signaling_err}"),
            "signaling error: connection dropped"
        );

        let peer_err = BlnkError::Peer("ICE failed".into());
        assert_eq!(format!("{peer_err}"), "peer error: ICE failed");

        let session_err = BlnkError::Session("auth timeout".into());
        assert_eq!(format!("{session_err}"), "session error: auth timeout");

        let stream_err = BlnkError::Stream("stream closed".into());
        assert_eq!(format!("{stream_err}"), "stream error: stream closed");

        let protocol_err = BlnkError::Protocol("bad header".into());
        assert_eq!(format!("{protocol_err}"), "protocol error: bad header");

        let io_err: BlnkError = io::Error::new(io::ErrorKind::NotFound, "file not found").into();
        assert_eq!(format!("{io_err}"), "I/O error: file not found");
    }
}
