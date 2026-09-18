//! Transport-independent session lifecycle and authentication policy.

use std::time::{Duration, Instant};

use subtle::{Choice, ConstantTimeEq};

use crate::stream::{StreamEntry, StreamKind, StreamRegistry};
use crate::utils::error::BlnkError;

pub type SessionResult<T> = Result<T, BlnkError>;

pub mod runtime;

pub use runtime::{
    ControlMessage, SessionRole, SessionRuntime, SessionRuntimeConfig, SessionRuntimeSnapshot,
};

const PIN_LEN: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectMessage {
    pub message_type: String,
    pub path: String,
    pub version: Option<i32>,
}

impl ConnectMessage {
    pub fn new(path: impl Into<String>, version: Option<i32>) -> SessionResult<Self> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(BlnkError::Session("connect path must not be empty".into()));
        }
        Ok(Self {
            message_type: "connect".into(),
            path,
            version,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRequiredMessage {
    pub message_type: String,
}

impl Default for AuthRequiredMessage {
    fn default() -> Self {
        Self {
            message_type: "auth_required".into(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthMessage {
    pub message_type: String,
    pub pin: String,
}

impl std::fmt::Debug for AuthMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthMessage")
            .field("message_type", &self.message_type)
            .field("pin", &"<redacted>")
            .finish()
    }
}

impl AuthMessage {
    pub fn new(pin: impl Into<String>) -> SessionResult<Self> {
        let pin = pin.into();
        if pin.trim().is_empty() {
            return Err(BlnkError::Session("PIN must not be empty".into()));
        }
        Ok(Self {
            message_type: "auth".into(),
            pin,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthResultMessage {
    pub message_type: String,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyMessage {
    pub message_type: String,
    pub server_version: i32,
    pub capabilities: Vec<String>,
    pub routing: String,
}

impl ReadyMessage {
    pub fn new(
        server_version: i32,
        capabilities: Vec<String>,
        routing: impl Into<String>,
    ) -> SessionResult<Self> {
        let routing = routing.into();
        if routing != "target-prefix" && routing != "direct" {
            return Err(BlnkError::Session(format!("unsupported routing mode: {routing}")));
        }
        Ok(Self {
            message_type: "ready".into(),
            server_version,
            capabilities,
            routing,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlErrorMessage {
    pub message_type: String,
    pub message: String,
}

impl ControlErrorMessage {
    pub fn new(message: impl Into<String>) -> SessionResult<Self> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(BlnkError::Session("control error message must not be empty".into()));
        }
        Ok(Self {
            message_type: "error".into(),
            message,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    pub pin_required: bool,
    pub max_auth_fails: u32,
    pub pin_fail_delay: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            pin_required: true,
            max_auth_fails: 3,
            pin_fail_delay: Duration::from_millis(2_000),
        }
    }
}

impl SessionConfig {
    pub fn validate(&self) -> SessionResult<()> {
        if self.pin_required && self.max_auth_fails == 0 {
            return Err(BlnkError::Session(
                "max_auth_fails must be greater than zero when PIN auth is enabled".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Connecting,
    Authenticating,
    Ready,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthOutcome {
    Ready,
    Rejected { attempts_remaining: u32 },
    Closed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionStats {
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub frames_received: u64,
    pub frames_sent: u64,
}

impl SessionStats {
    pub fn active_streams(self, registry: &StreamRegistry) -> u64 {
        registry.len() as u64
    }
}

pub struct Session {
    state: SessionState,
    config: SessionConfig,
    expected_pin: Option<String>,
    auth_attempts: u32,
    registry: StreamRegistry,
    stats: SessionStats,
    next_auth_allowed_at: Option<Instant>,
}

impl Session {
    pub fn new(config: SessionConfig, expected_pin: Option<String>) -> SessionResult<Self> {
        config.validate()?;
        if config.pin_required {
            let expected_pin = expected_pin.as_deref().unwrap_or_default();
            if expected_pin.trim().is_empty() {
                return Err(BlnkError::Session(
                    "expected PIN is required when PIN auth is enabled".into(),
                ));
            }
            if expected_pin.len() != PIN_LEN {
                return Err(BlnkError::Session(format!(
                    "expected PIN must be exactly {PIN_LEN} bytes"
                )));
            }
        }
        Ok(Self {
            state: SessionState::Connecting,
            config,
            expected_pin,
            auth_attempts: 0,
            registry: StreamRegistry::new(),
            stats: SessionStats::default(),
            next_auth_allowed_at: None,
        })
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn auth_attempts(&self) -> u32 {
        self.auth_attempts
    }

    pub fn begin_authentication(&mut self) -> SessionResult<Option<AuthRequiredMessage>> {
        if self.state != SessionState::Connecting {
            return Err(BlnkError::Session(format!(
                "cannot begin authentication from {:?}",
                self.state
            )));
        }
        if self.config.pin_required {
            self.state = SessionState::Authenticating;
            Ok(Some(AuthRequiredMessage::default()))
        } else {
            self.state = SessionState::Ready;
            Ok(None)
        }
    }

    /// Records that a client received the server's request for a PIN.
    pub fn accept_auth_required(&mut self) -> SessionResult<()> {
        match self.state {
            SessionState::Connecting | SessionState::Authenticating => {
                self.state = SessionState::Authenticating;
                Ok(())
            }
            state => Err(BlnkError::Session(format!("cannot accept auth_required from {state:?}"))),
        }
    }

    /// Records an authentication result received by a client.
    ///
    /// The final transition to `Ready` is intentionally driven by the wire
    /// `ready` message so the runtime cannot expose streams before the server
    /// completes the control handshake.
    pub fn accept_auth_result(&mut self, success: bool) -> SessionResult<()> {
        if self.state != SessionState::Authenticating {
            return Err(BlnkError::Session(format!(
                "cannot accept auth_result from {:?}",
                self.state
            )));
        }
        if success {
            self.next_auth_allowed_at = None;
        }
        Ok(())
    }

    /// Marks the session ready after a valid wire `ready` message.
    pub fn mark_ready(&mut self) -> SessionResult<()> {
        match self.state {
            SessionState::Connecting | SessionState::Authenticating => {
                self.state = SessionState::Ready;
                self.next_auth_allowed_at = None;
                Ok(())
            }
            state => Err(BlnkError::Session(format!("cannot mark session ready from {state:?}"))),
        }
    }

    pub fn authenticate(&mut self, pin: &str, now: Instant) -> SessionResult<AuthOutcome> {
        if self.state != SessionState::Authenticating {
            return Err(BlnkError::Session(format!("cannot authenticate from {:?}", self.state)));
        }
        if let Some(next_allowed) = self.next_auth_allowed_at
            && now < next_allowed
        {
            return Err(BlnkError::Session("PIN retry delay has not elapsed".into()));
        }

        if self.expected_pin.as_deref().is_some_and(|expected_pin| {
            constant_time_pin_eq(expected_pin.as_bytes(), pin.as_bytes())
        }) {
            self.state = SessionState::Ready;
            self.next_auth_allowed_at = None;
            Ok(AuthOutcome::Ready)
        } else {
            let next_attempts = self.auth_attempts.saturating_add(1);
            if next_attempts >= self.config.max_auth_fails {
                self.auth_attempts = next_attempts;
                self.close();
                Ok(AuthOutcome::Closed)
            } else {
                let next_auth_allowed_at = now
                    .checked_add(self.config.pin_fail_delay)
                    .ok_or_else(|| BlnkError::Session("PIN retry delay is too large".into()))?;
                self.auth_attempts = next_attempts;
                self.next_auth_allowed_at = Some(next_auth_allowed_at);
                Ok(AuthOutcome::Rejected {
                    attempts_remaining: self.config.max_auth_fails - self.auth_attempts,
                })
            }
        }
    }

    pub fn open_stream(
        &mut self,
        kind: StreamKind,
        connect_path: impl Into<String>,
    ) -> SessionResult<StreamEntry> {
        if self.state != SessionState::Ready {
            return Err(BlnkError::Session(format!("cannot open stream from {:?}", self.state)));
        }
        self.registry.open(kind, connect_path)
    }

    pub fn accept_stream(
        &mut self,
        stream_id: u32,
        kind: StreamKind,
        connect_path: impl Into<String>,
    ) -> SessionResult<StreamEntry> {
        if self.state != SessionState::Ready {
            return Err(BlnkError::Session(format!("cannot accept stream from {:?}", self.state)));
        }
        self.registry.accept(stream_id, kind, connect_path)
    }

    pub fn close_stream(&mut self, stream_id: u32) -> SessionResult<StreamEntry> {
        self.registry.close(stream_id)
    }

    pub fn record_received(&mut self, bytes: u64) {
        self.stats.bytes_received = self.stats.bytes_received.saturating_add(bytes);
        self.stats.frames_received = self.stats.frames_received.saturating_add(1);
    }

    pub fn record_sent(&mut self, bytes: u64) {
        self.stats.bytes_sent = self.stats.bytes_sent.saturating_add(bytes);
        self.stats.frames_sent = self.stats.frames_sent.saturating_add(1);
    }

    pub fn stats(&self) -> SessionStats {
        self.stats
    }

    pub fn stream_entry(&self, stream_id: u32) -> Option<&StreamEntry> {
        self.registry.get(stream_id)
    }

    pub fn active_streams(&self) -> usize {
        self.registry.len()
    }

    pub fn close(&mut self) {
        self.registry.clear();
        self.state = SessionState::Closed;
        self.next_auth_allowed_at = None;
    }
}

pub(super) fn constant_time_pin_eq(expected: &[u8], provided: &[u8]) -> bool {
    let mut expected_fixed = [0_u8; PIN_LEN];
    let mut provided_fixed = [0_u8; PIN_LEN];
    let expected_copy_len = expected.len().min(PIN_LEN);
    let provided_copy_len = provided.len().min(PIN_LEN);
    expected_fixed[..expected_copy_len].copy_from_slice(&expected[..expected_copy_len]);
    provided_fixed[..provided_copy_len].copy_from_slice(&provided[..provided_copy_len]);

    let contents_match = expected_fixed.ct_eq(&provided_fixed);
    let expected_len_match = Choice::from((expected.len() == PIN_LEN) as u8);
    let provided_len_match = Choice::from((provided.len() == PIN_LEN) as u8);

    (contents_match & expected_len_match & provided_len_match).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_message_debug_redacts_pin() {
        let msg = AuthMessage::new("123456").expect("valid pin");
        let debug_output = format!("{msg:?}");
        assert!(!debug_output.contains("123456"));
        assert!(debug_output.contains("<redacted>"));
    }

    #[test]
    fn whitespace_expected_pin_is_rejected() {
        let result = Session::new(SessionConfig::default(), Some("   \t".into()));
        assert!(result.is_err());
    }

    #[test]
    fn no_auth_transitions_to_ready_without_error() {
        let config = SessionConfig {
            pin_required: false,
            ..SessionConfig::default()
        };
        let mut session = Session::new(config, None).expect("valid no-auth session");
        assert_eq!(session.begin_authentication().expect("no-auth transition"), None);
        assert_eq!(session.state(), SessionState::Ready);
    }

    #[test]
    fn variable_length_pin_is_compared_against_fixed_size_buffers() {
        let mut session =
            Session::new(SessionConfig::default(), Some("123456".into())).expect("valid session");
        session
            .begin_authentication()
            .expect("auth required")
            .expect("PIN auth should be enabled");

        assert_eq!(
            session
                .authenticate("123", Instant::now())
                .expect("wrong-length PIN should be rejected"),
            AuthOutcome::Rejected { attempts_remaining: 2 }
        );
        assert_eq!(session.auth_attempts(), 1);
    }

    #[test]
    fn oversized_retry_delay_returns_error_instead_of_panicking() {
        let config = SessionConfig {
            pin_fail_delay: Duration::MAX,
            ..SessionConfig::default()
        };
        let mut session = Session::new(config, Some("123456".into())).expect("valid session");
        session
            .begin_authentication()
            .expect("auth required")
            .expect("PIN auth should be enabled");
        let error = session
            .authenticate("wrong", Instant::now())
            .expect_err("unrepresentable retry delay should be rejected");
        assert!(error.to_string().contains("PIN retry delay is too large"));
        assert_eq!(session.auth_attempts(), 0);
    }

    #[test]
    fn pin_auth_reaches_ready_and_tracks_stats() {
        let mut session =
            Session::new(SessionConfig::default(), Some("123456".into())).expect("valid session");
        assert_eq!(session.state(), SessionState::Connecting);
        assert!(
            session
                .begin_authentication()
                .expect("auth required")
                .is_some()
        );
        assert_eq!(
            session
                .authenticate("123456", Instant::now())
                .expect("auth result"),
            AuthOutcome::Ready
        );
        let stream = session
            .open_stream(StreamKind::Shell, "/shell")
            .expect("stream opens");
        assert_eq!(stream.id(), 1);
        session.record_received(4);
        session.record_sent(8);
        assert_eq!(session.stats().bytes_received, 4);
        assert_eq!(session.stats().bytes_sent, 8);
        assert_eq!(session.active_streams(), 1);
    }

    #[test]
    fn failed_auth_closes_and_clears_streams() {
        let config = SessionConfig {
            pin_fail_delay: Duration::ZERO,
            ..SessionConfig::default()
        };
        let mut session = Session::new(config, Some("123456".into())).expect("valid session");
        assert!(
            session
                .begin_authentication()
                .expect("auth required")
                .is_some()
        );
        let now = Instant::now();
        assert_eq!(
            session.authenticate("bad", now).expect("rejection"),
            AuthOutcome::Rejected { attempts_remaining: 2 }
        );
        assert_eq!(
            session.authenticate("bad", now).expect("rejection"),
            AuthOutcome::Rejected { attempts_remaining: 1 }
        );
        assert_eq!(session.authenticate("bad", now).expect("closed"), AuthOutcome::Closed);
        assert_eq!(session.state(), SessionState::Closed);
        assert_eq!(session.active_streams(), 0);
    }

    #[test]
    fn retry_delay_blocks_immediate_retry() {
        let mut session =
            Session::new(SessionConfig::default(), Some("123456".into())).expect("valid session");
        assert!(
            session
                .begin_authentication()
                .expect("auth required")
                .is_some()
        );
        let now = Instant::now();
        session.authenticate("bad", now).expect("rejection");
        assert!(session.authenticate("123456", now).is_err());
    }

    #[test]
    fn invalid_state_transitions_are_rejected() {
        let mut session =
            Session::new(SessionConfig::default(), Some("123456".into())).expect("valid session");
        assert!(session.open_stream(StreamKind::Shell, "/shell").is_err());
        assert!(
            session
                .begin_authentication()
                .expect("auth required")
                .is_some()
        );
        assert!(session.begin_authentication().is_err());
    }
}
