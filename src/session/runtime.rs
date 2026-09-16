//! Session runtime integration for the WebRTC control stream.
//!
//! The runtime keeps the existing [`Session`] state machine as the source of
//! truth and only adapts it to control messages carried in SWSP stream 0. The
//! protobuf messages come from `proto/control.proto`; this module does not add
//! new wire message types or alter the existing PIN semantics.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use prost::Message;

use crate::peer::PeerHandle;
use crate::proto_generated::control as wire;
use crate::protocol::swsp::{Frame, FrameFlags};
use crate::signaling::PROTOCOL_VERSION;
use crate::stream::file::{FileTransferRequest, encode_data_frame, encode_request_frame};
use crate::stream::shell::{
    ShellCommand, encode_cancel_frame, encode_error_frame, encode_exit_frame, encode_input_frame,
    encode_open_frame, encode_output_frame, encode_resize_frame,
};
use crate::stream::{StreamEntry, StreamKind};
use crate::utils::error::BlnkError;

use super::{AuthOutcome, Session, SessionConfig, SessionResult, SessionState, SessionStats};

const CONTROL_STREAM_ID: u32 = 0;
const DEFAULT_CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_CONNECT_PATH: &str = "/";
const DEFAULT_ROUTING: &str = "direct";

/// The side of the authenticated control exchange represented by a runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    /// Accepts `connect`, validates PINs, and sends the ready response.
    Server,
    /// Sends `connect`, supplies the configured PINs, and waits for ready.
    Client,
}

/// A typed representation of the control messages on SWSP stream 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlMessage {
    Connect(wire::ConnectMessage),
    AuthRequired(wire::AuthRequiredMessage),
    Auth(wire::AuthMessage),
    AuthResult(wire::AuthResultMessage),
    Ready(wire::ReadyMessage),
    Error(wire::ErrorMessage),
}

impl ControlMessage {
    pub fn connect(path: impl Into<String>, version: Option<i32>) -> SessionResult<Self> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(BlnkError::Session("connect path must not be empty".into()));
        }
        Ok(Self::Connect(wire::ConnectMessage {
            r#type: "connect".into(),
            path,
            version: version.unwrap_or_default(),
        }))
    }

    pub fn auth_required() -> Self {
        Self::AuthRequired(wire::AuthRequiredMessage {
            r#type: "auth_required".into(),
        })
    }

    pub fn auth(pin: impl Into<String>) -> SessionResult<Self> {
        let pin = pin.into();
        if pin.trim().is_empty() {
            return Err(BlnkError::Session("PIN must not be empty".into()));
        }
        Ok(Self::Auth(wire::AuthMessage {
            r#type: "auth".into(),
            pin,
        }))
    }

    pub fn auth_result(success: bool) -> Self {
        Self::AuthResult(wire::AuthResultMessage {
            r#type: "auth_result".into(),
            success,
        })
    }

    pub fn ready(
        server_version: i32,
        capabilities: Vec<String>,
        routing: impl Into<String>,
    ) -> Self {
        Self::Ready(wire::ReadyMessage {
            r#type: "ready".into(),
            server_version,
            caps: capabilities,
            routing: routing.into(),
        })
    }

    pub fn error(message: impl Into<String>) -> SessionResult<Self> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(BlnkError::Session(
                "control error message must not be empty".into(),
            ));
        }
        Ok(Self::Error(wire::ErrorMessage {
            r#type: "error".into(),
            message,
        }))
    }

    /// Encodes the protobuf control payload without an SWSP header.
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Connect(message) => message.encode_to_vec(),
            Self::AuthRequired(message) => message.encode_to_vec(),
            Self::Auth(message) => message.encode_to_vec(),
            Self::AuthResult(message) => message.encode_to_vec(),
            Self::Ready(message) => message.encode_to_vec(),
            Self::Error(message) => message.encode_to_vec(),
        }
    }

    /// Decodes one protobuf control payload and validates its discriminator.
    pub fn decode(payload: &[u8]) -> SessionResult<Self> {
        if let Ok(message) = wire::ConnectMessage::decode(payload)
            && message.r#type == "connect"
        {
            return Ok(Self::Connect(message));
        }
        if let Ok(message) = wire::AuthRequiredMessage::decode(payload)
            && message.r#type == "auth_required"
        {
            return Ok(Self::AuthRequired(message));
        }
        if let Ok(message) = wire::AuthMessage::decode(payload)
            && message.r#type == "auth"
        {
            return Ok(Self::Auth(message));
        }
        if let Ok(message) = wire::AuthResultMessage::decode(payload)
            && message.r#type == "auth_result"
        {
            return Ok(Self::AuthResult(message));
        }
        if let Ok(message) = wire::ReadyMessage::decode(payload)
            && message.r#type == "ready"
        {
            return Ok(Self::Ready(message));
        }
        if let Ok(message) = wire::ErrorMessage::decode(payload)
            && message.r#type == "error"
        {
            return Ok(Self::Error(message));
        }
        Err(BlnkError::Protocol(
            "invalid or unknown control message discriminator".into(),
        ))
    }
}

/// Configuration for one side of a session runtime.
#[derive(Clone)]
pub struct SessionRuntimeConfig {
    pub role: SessionRole,
    pub session: SessionConfig,
    pub expected_pin: Option<String>,
    pub auth_pins: Vec<String>,
    pub connect_path: String,
    pub server_version: i32,
    pub capabilities: Vec<String>,
    pub routing: String,
    pub control_timeout: Duration,
}

impl std::fmt::Debug for SessionRuntimeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let expected_pin_display = self.expected_pin.as_ref().map(|_| "<redacted>");
        let auth_pins_display = if self.auth_pins.is_empty() {
            "[]"
        } else {
            "[<redacted>]"
        };
        f.debug_struct("SessionRuntimeConfig")
            .field("role", &self.role)
            .field("session", &self.session)
            .field("expected_pin", &expected_pin_display)
            .field("auth_pins", &auth_pins_display)
            .field("connect_path", &self.connect_path)
            .field("server_version", &self.server_version)
            .field("capabilities", &self.capabilities)
            .field("routing", &self.routing)
            .field("control_timeout", &self.control_timeout)
            .finish()
    }
}

impl SessionRuntimeConfig {
    /// Creates a server configuration with the repository's current protocol version.
    pub fn server(expected_pin: impl Into<String>) -> Self {
        Self {
            role: SessionRole::Server,
            session: SessionConfig::default(),
            expected_pin: Some(expected_pin.into()),
            auth_pins: Vec::new(),
            connect_path: DEFAULT_CONNECT_PATH.into(),
            server_version: PROTOCOL_VERSION,
            capabilities: Vec::new(),
            routing: DEFAULT_ROUTING.into(),
            control_timeout: DEFAULT_CONTROL_TIMEOUT,
        }
    }

    /// Creates a client configuration. The client does not need to know the server PIN.
    pub fn client(auth_pin: impl Into<String>) -> Self {
        Self {
            role: SessionRole::Client,
            session: SessionConfig {
                pin_required: false,
                ..SessionConfig::default()
            },
            expected_pin: None,
            auth_pins: vec![auth_pin.into()],
            connect_path: DEFAULT_CONNECT_PATH.into(),
            server_version: PROTOCOL_VERSION,
            capabilities: Vec::new(),
            routing: DEFAULT_ROUTING.into(),
            control_timeout: DEFAULT_CONTROL_TIMEOUT,
        }
    }

    pub fn with_session_config(mut self, session: SessionConfig) -> Self {
        self.session = session;
        self
    }

    pub fn with_auth_pins(mut self, auth_pins: Vec<String>) -> Self {
        self.auth_pins = auth_pins;
        self
    }

    pub fn with_connect_path(mut self, path: impl Into<String>) -> Self {
        self.connect_path = path.into();
        self
    }

    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_routing(mut self, routing: impl Into<String>) -> Self {
        self.routing = routing.into();
        self
    }

    pub fn with_control_timeout(mut self, timeout: Duration) -> Self {
        self.control_timeout = timeout;
        self
    }

    fn validate(&self) -> SessionResult<()> {
        if self.control_timeout.is_zero() {
            return Err(BlnkError::Session(
                "control timeout must be greater than zero".into(),
            ));
        }
        if self.connect_path.trim().is_empty() {
            return Err(BlnkError::Session("connect path must not be empty".into()));
        }
        if self.role == SessionRole::Server
            && self.session.pin_required
            && self.expected_pin.is_none()
        {
            return Err(BlnkError::Session(
                "server expected PIN is required when PIN auth is enabled".into(),
            ));
        }
        if self.role == SessionRole::Client && self.auth_pins.is_empty() {
            return Err(BlnkError::Session(
                "client must have at least one authentication PIN".into(),
            ));
        }
        super::ReadyMessage::new(
            self.server_version,
            self.capabilities.clone(),
            self.routing.clone(),
        )?;
        Ok(())
    }
}

/// Runtime view returned by callers through the public accessors on [`SessionRuntime`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRuntimeSnapshot {
    pub state: SessionState,
    pub stats: SessionStats,
    pub active_streams: usize,
}

/// Connects a [`Session`] state machine to a [`PeerHandle`] control channel.
pub struct SessionRuntime {
    peer: PeerHandle,
    session: Session,
    config: SessionRuntimeConfig,
    auth_pins: VecDeque<String>,
    handshake_started: bool,
    connect_seen: bool,
    auth_pending: bool,
    last_auth_pin: Option<String>,
    ready_seen: bool,
}

impl SessionRuntime {
    pub fn new(peer: PeerHandle, config: SessionRuntimeConfig) -> SessionResult<Self> {
        config.validate()?;
        let session = Session::new(config.session.clone(), config.expected_pin.clone())?;
        Ok(Self {
            peer,
            session,
            auth_pins: config.auth_pins.clone().into(),
            config,
            handshake_started: false,
            connect_seen: false,
            auth_pending: false,
            last_auth_pin: None,
            ready_seen: false,
        })
    }

    pub fn snapshot(&self) -> SessionRuntimeSnapshot {
        SessionRuntimeSnapshot {
            state: self.session.state(),
            stats: self.session.stats(),
            active_streams: self.session.active_streams(),
        }
    }

    pub fn state(&self) -> SessionState {
        self.session.state()
    }

    pub fn stats(&self) -> SessionStats {
        self.session.stats()
    }

    pub fn active_streams(&self) -> usize {
        self.session.active_streams()
    }

    pub fn open_stream(
        &mut self,
        kind: StreamKind,
        connect_path: impl Into<String>,
    ) -> SessionResult<StreamEntry> {
        if matches!(
            kind,
            StreamKind::Tcp | StreamKind::WebSocket | StreamKind::Http
        ) {
            // FIXME: ProxyStreamService exists in proxy_handler.rs but is NOT wired
            // into the ConnectionSupervisor dispatch loop. This blocks spec reqs
            // 4.3 (Web Proxy), 4.4 (TCP Forwarding), 4.10 (WebSocket Bridging).
            // See TODO.md "Codex-Inspired Adaptation Backlog" for proxy dispatch task.
            // TODO: Wire proxy dispatch before enabling these stream kinds.
            return Err(BlnkError::Stream(
                "proxy stream dispatch is not implemented".into(),
            ));
        }
        self.session.open_stream(kind, connect_path)
    }

    pub fn close_stream(&mut self, stream_id: u32) -> SessionResult<StreamEntry> {
        self.session.close_stream(stream_id)
    }

    /// Opens a local file stream after the authenticated session is ready.
    pub fn open_file_stream(
        &mut self,
        connect_path: impl Into<String>,
    ) -> SessionResult<StreamEntry> {
        self.open_stream(StreamKind::File, connect_path)
    }

    /// Accepts a peer-assigned file stream ID after the authenticated session is ready.
    pub fn accept_file_stream(
        &mut self,
        stream_id: u32,
        connect_path: impl Into<String>,
    ) -> SessionResult<StreamEntry> {
        self.session
            .accept_stream(stream_id, StreamKind::File, connect_path)
    }

    /// Sends a file request as a SYN|DAT frame on an active file stream.
    pub async fn send_file_request(
        &self,
        stream_id: u32,
        request: &FileTransferRequest,
    ) -> SessionResult<()> {
        self.ensure_file_stream(stream_id)?;
        self.peer
            .send_frame(&encode_request_frame(stream_id, request)?)
            .await
    }

    /// Sends one file-data chunk as a DAT frame, optionally carrying FIN.
    pub async fn send_file_chunk(
        &self,
        stream_id: u32,
        data: Vec<u8>,
        final_chunk: bool,
    ) -> SessionResult<()> {
        self.ensure_file_stream(stream_id)?;
        self.peer
            .send_frame(&encode_data_frame(stream_id, data, final_chunk)?)
            .await
    }

    /// Receives the next raw file-stream SWSP frame.
    pub async fn recv_file_frame(&self) -> SessionResult<Frame> {
        let frame = self.peer.recv_frame().await?;
        if frame.stream_id != CONTROL_STREAM_ID || frame.flags.is_fin() {
            return Ok(frame);
        }
        let message = Self::decode_control_frame(frame)?;
        Err(BlnkError::Protocol(format!(
            "unexpected control message while waiting for file frame: {message:?}"
        )))
    }

    /// Sends FIN for an active file stream and removes it from the session registry.
    pub async fn close_file_stream(&mut self, stream_id: u32) -> SessionResult<StreamEntry> {
        self.ensure_file_stream(stream_id)?;
        self.peer
            .send_frame(&Frame::new(stream_id, FrameFlags::FIN, Vec::new()))
            .await?;
        self.session.close_stream(stream_id)
    }

    /// Opens a local shell stream after the authenticated session is ready.
    pub fn open_shell_stream(
        &mut self,
        connect_path: impl Into<String>,
    ) -> SessionResult<StreamEntry> {
        self.open_stream(StreamKind::Shell, connect_path)
    }

    /// Accepts a peer-assigned shell stream ID after the authenticated session is ready.
    pub fn accept_shell_stream(
        &mut self,
        stream_id: u32,
        connect_path: impl Into<String>,
    ) -> SessionResult<StreamEntry> {
        self.session
            .accept_stream(stream_id, StreamKind::Shell, connect_path)
    }

    /// Sends a shell open request using direct argv values and the existing SWSP SYN boundary.
    pub async fn send_shell_open(
        &self,
        stream_id: u32,
        command: &ShellCommand,
    ) -> SessionResult<()> {
        self.ensure_shell_stream(stream_id)?;
        self.peer
            .send_frame(&encode_open_frame(stream_id, command)?)
            .await
    }

    /// Sends one shell stdin chunk, optionally carrying FIN.
    pub async fn send_shell_input(
        &self,
        stream_id: u32,
        data: Vec<u8>,
        final_chunk: bool,
    ) -> SessionResult<()> {
        self.ensure_shell_stream(stream_id)?;
        self.peer
            .send_frame(&encode_input_frame(stream_id, data, final_chunk)?)
            .await
    }

    /// Sends a validated terminal resize request. Pipe-based receivers may reject it as PTY-unsupported.
    pub async fn send_shell_resize(
        &self,
        stream_id: u32,
        cols: i32,
        rows: i32,
    ) -> SessionResult<()> {
        self.ensure_shell_stream(stream_id)?;
        self.peer
            .send_frame(&encode_resize_frame(stream_id, cols, rows)?)
            .await
    }

    /// Sends one shell stdout/stderr output chunk, optionally carrying FIN.
    pub async fn send_shell_output(
        &self,
        stream_id: u32,
        data: Vec<u8>,
        final_chunk: bool,
    ) -> SessionResult<()> {
        self.ensure_shell_stream(stream_id)?;
        self.peer
            .send_frame(&encode_output_frame(stream_id, data, final_chunk)?)
            .await
    }

    /// Sends a terminal process-exit message.
    pub async fn send_shell_exit(
        &self,
        stream_id: u32,
        code: i32,
        signaled: bool,
    ) -> SessionResult<()> {
        self.ensure_shell_stream(stream_id)?;
        self.peer
            .send_frame(&encode_exit_frame(stream_id, code, signaled)?)
            .await
    }

    /// Sends a policy/process error message.
    pub async fn send_shell_error(
        &self,
        stream_id: u32,
        message: impl Into<String>,
    ) -> SessionResult<()> {
        self.ensure_shell_stream(stream_id)?;
        self.peer
            .send_frame(&encode_error_frame(stream_id, message)?)
            .await
    }

    /// Sends a shell cancellation message.
    pub async fn send_shell_cancel(&self, stream_id: u32) -> SessionResult<()> {
        self.ensure_shell_stream(stream_id)?;
        self.peer.send_frame(&encode_cancel_frame(stream_id)?).await
    }

    /// Receives the next raw frame belonging to an active shell stream.
    pub async fn recv_shell_frame(&self) -> SessionResult<Frame> {
        let frame = self.peer.recv_frame().await?;
        if frame.stream_id == CONTROL_STREAM_ID {
            if frame.flags.is_fin() {
                return Ok(frame);
            }
            let message = Self::decode_control_frame(frame)?;
            return Err(BlnkError::Protocol(format!(
                "unexpected control message while waiting for shell frame: {message:?}"
            )));
        }
        if frame.flags.is_syn() {
            if !frame.flags.is_dat() {
                return Err(BlnkError::Protocol(
                    "shell SYN frame must carry DAT payload".into(),
                ));
            }
            return Ok(frame);
        }
        self.ensure_shell_stream(frame.stream_id)?;
        Ok(frame)
    }

    /// Sends FIN for an active shell stream and removes it from the session registry.
    pub async fn close_shell_stream(&mut self, stream_id: u32) -> SessionResult<StreamEntry> {
        self.ensure_shell_stream(stream_id)?;
        self.peer
            .send_frame(&Frame::new(stream_id, FrameFlags::FIN, Vec::new()))
            .await?;
        self.session.close_stream(stream_id)
    }

    fn ensure_shell_stream(&self, stream_id: u32) -> SessionResult<()> {
        if self.state() != SessionState::Ready {
            return Err(BlnkError::Session(
                "session must be ready before using a shell stream".into(),
            ));
        }
        match self.session.stream_entry(stream_id) {
            Some(entry) if entry.kind() == StreamKind::Shell => Ok(()),
            Some(entry) => Err(BlnkError::Stream(format!(
                "stream {stream_id} is not a shell stream ({:?})",
                entry.kind()
            ))),
            None => Err(BlnkError::Stream(format!(
                "unknown shell stream id: {stream_id}"
            ))),
        }
    }

    fn ensure_file_stream(&self, stream_id: u32) -> SessionResult<()> {
        if self.state() != SessionState::Ready {
            return Err(BlnkError::Session(
                "session must be ready before using a file stream".into(),
            ));
        }
        match self.session.stream_entry(stream_id) {
            Some(entry) if entry.kind() == StreamKind::File => Ok(()),
            Some(entry) => Err(BlnkError::Stream(format!(
                "stream {stream_id} is not a file stream ({:?})",
                entry.kind()
            ))),
            None => Err(BlnkError::Stream(format!(
                "unknown file stream id: {stream_id}"
            ))),
        }
    }

    /// Runs the role-specific control exchange until both sides reach Ready.
    pub async fn handshake(&mut self) -> SessionResult<()> {
        if self.handshake_started {
            return Err(BlnkError::Session(
                "session handshake already started".into(),
            ));
        }
        self.handshake_started = true;

        if self.config.role == SessionRole::Client {
            let connect =
                ControlMessage::connect(&self.config.connect_path, Some(PROTOCOL_VERSION))?;
            self.send_control(connect).await?;
        }

        loop {
            let message = match self.receive_control().await {
                Ok(message) => message,
                Err(error) => {
                    self.session.close();
                    return Err(error);
                }
            };
            if self.handle_control(message).await? {
                return Ok(());
            }
        }
    }

    /// Processes control frames after the handshake until the peer disconnects.
    pub async fn run_until_disconnect(&mut self) -> SessionResult<()> {
        if self.state() != SessionState::Ready {
            return Err(BlnkError::Session(
                "session must be ready before waiting for disconnect".into(),
            ));
        }

        loop {
            tokio::select! {
                disconnected = self.peer.wait_disconnected() => {
                    self.session.close();
                    return disconnected;
                }
                result = self.peer.recv_frame() => {
                    match result {
                        Ok(frame) if frame.flags.is_fin() => {
                            self.session.close();
                            return Ok(());
                        }
                        Ok(frame) => {
                            let message = Self::decode_control_frame(frame)?;
                            self.handle_control(message).await?;
                        }
                        Err(BlnkError::Peer(message)) if message.contains("receive loop closed") => {
                            self.session.close();
                            return Ok(());
                        }
                        Err(error) => {
                            self.session.close();
                            return Err(error);
                        }
                    }
                }
            }
        }
    }

    /// Sends one raw SWSP frame after the authenticated session is ready.
    ///
    /// The remote dispatcher uses this method to preserve the stream codec's
    /// exact flags and payload (for example file response metadata frames).
    pub async fn send_frame(&self, frame: &Frame) -> SessionResult<()> {
        if self.state() != SessionState::Ready {
            return Err(BlnkError::Session(
                "session must be ready before sending a data frame".into(),
            ));
        }
        self.peer.send_frame(frame).await
    }

    /// Receives one raw SWSP frame for the single reader owned by a dispatcher.
    pub async fn recv_frame(&self) -> SessionResult<Frame> {
        self.peer.recv_frame().await
    }

    /// Returns the registered kind of a stream, if it exists.
    pub fn stream_kind(&self, stream_id: u32) -> Option<StreamKind> {
        self.session
            .stream_entry(stream_id)
            .map(|entry| entry.kind())
    }

    /// Sends a typed control message. Public for transport-level integration tests.
    pub async fn send_control(&self, message: ControlMessage) -> SessionResult<()> {
        let frame = Frame::new(CONTROL_STREAM_ID, FrameFlags::DAT, message.encode());
        self.peer.send_frame(&frame).await
    }

    /// Closes the session, clears active streams, and tears down the peer.
    pub async fn close(&mut self) -> SessionResult<()> {
        let result = if self.state() != SessionState::Closed {
            match self
                .peer
                .send_frame(&Frame::new(CONTROL_STREAM_ID, FrameFlags::FIN, Vec::new()))
                .await
            {
                Err(error) if is_data_channel_closed_send_error(&error) => Ok(()),
                result => result,
            }
        } else {
            Ok(())
        };
        if result.is_ok() {
            let _ = tokio::time::timeout(
                Duration::from_millis(250),
                self.peer.wait_send_buffer_empty(),
            )
            .await;
        }
        self.session.close();
        let close_result = self.peer.close().await;
        result.and(close_result)
    }

    async fn receive_control(&self) -> SessionResult<ControlMessage> {
        match tokio::time::timeout(self.config.control_timeout, self.peer.recv_frame()).await {
            Ok(Ok(frame)) => Self::decode_control_frame(frame),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(BlnkError::Session(
                "timed out waiting for session control message".into(),
            )),
        }
    }

    fn decode_control_frame(frame: Frame) -> SessionResult<ControlMessage> {
        if frame.stream_id != CONTROL_STREAM_ID {
            return Err(BlnkError::Protocol(format!(
                "session runtime received non-control stream {}",
                frame.stream_id
            )));
        }
        ControlMessage::decode(&frame.payload)
    }

    async fn handle_control(&mut self, message: ControlMessage) -> SessionResult<bool> {
        match self.config.role {
            SessionRole::Server => self.handle_server_message(message).await,
            SessionRole::Client => self.handle_client_message(message).await,
        }
    }

    async fn handle_server_message(&mut self, message: ControlMessage) -> SessionResult<bool> {
        match message {
            ControlMessage::Connect(message) => {
                if self.connect_seen {
                    return self.fail_session("duplicate connect control message").await;
                }
                self.connect_seen = true;
                if message.path.trim().is_empty() {
                    return self.fail_session("connect path must not be empty").await;
                }
                if message.version != 0 && message.version != PROTOCOL_VERSION {
                    return self
                        .fail_session("unsupported session protocol version")
                        .await;
                }

                match self.session.begin_authentication()? {
                    Some(_) => {
                        self.send_control(ControlMessage::auth_required()).await?;
                        Ok(false)
                    }
                    None => {
                        self.send_ready().await?;
                        Ok(true)
                    }
                }
            }
            ControlMessage::Auth(message) => {
                if self.session.state() != SessionState::Authenticating {
                    return self.fail_session("unexpected auth control message").await;
                }
                if self.last_auth_pin.as_deref() == Some(message.pin.as_str()) {
                    return self.fail_session("duplicate auth control message").await;
                }
                self.last_auth_pin = Some(message.pin.clone());

                match self.session.authenticate(&message.pin, Instant::now())? {
                    AuthOutcome::Ready => {
                        self.send_control(ControlMessage::auth_result(true)).await?;
                        self.send_ready().await?;
                        Ok(true)
                    }
                    AuthOutcome::Rejected { .. } => {
                        self.send_control(ControlMessage::auth_result(false))
                            .await?;
                        self.send_control(ControlMessage::auth_required()).await?;
                        Ok(false)
                    }
                    AuthOutcome::Closed => {
                        self.send_control(ControlMessage::auth_result(false))
                            .await?;
                        self.fail_session("authentication retry limit exhausted")
                            .await
                    }
                }
            }
            ControlMessage::Error(message) => {
                self.fail_session(&format!("peer reported session error: {}", message.message))
                    .await
            }
            _ => {
                self.fail_session("unexpected control message for session server")
                    .await
            }
        }
    }

    async fn handle_client_message(&mut self, message: ControlMessage) -> SessionResult<bool> {
        match message {
            ControlMessage::AuthRequired(_) => {
                if self.auth_pending || self.ready_seen {
                    return self
                        .fail_session("duplicate auth_required control message")
                        .await;
                }
                self.session.accept_auth_required()?;
                let pin = self.auth_pins.pop_front().ok_or_else(|| {
                    BlnkError::Session("no authentication PIN remains for retry".into())
                })?;
                self.auth_pending = true;
                self.send_control(ControlMessage::auth(pin)?).await?;
                Ok(false)
            }
            ControlMessage::AuthResult(message) => {
                if !self.auth_pending {
                    return self
                        .fail_session("duplicate auth_result control message")
                        .await;
                }
                self.auth_pending = false;
                self.session.accept_auth_result(message.success)?;
                Ok(false)
            }
            ControlMessage::Ready(message) => {
                if self.ready_seen || self.auth_pending {
                    return self
                        .fail_session("duplicate or premature ready control message")
                        .await;
                }
                if message.r#type != "ready" || message.routing.trim().is_empty() {
                    return self.fail_session("invalid ready control message").await;
                }
                self.session.mark_ready()?;
                self.ready_seen = true;
                Ok(true)
            }
            ControlMessage::Error(message) => {
                self.fail_session(&format!("peer reported session error: {}", message.message))
                    .await
            }
            _ => {
                self.fail_session("unexpected control message for session client")
                    .await
            }
        }
    }

    async fn send_ready(&mut self) -> SessionResult<()> {
        if self.ready_seen {
            return self
                .fail_session("duplicate ready control message")
                .await
                .map(|_| ());
        }
        let ready = ControlMessage::ready(
            self.config.server_version,
            self.config.capabilities.clone(),
            self.config.routing.clone(),
        );
        self.send_control(ready).await?;
        self.ready_seen = true;
        Ok(())
    }

    async fn fail_session(&mut self, message: &str) -> SessionResult<bool> {
        let _ = self
            .send_control(ControlMessage::error(message.to_owned())?)
            .await;
        self.session.close();
        let _ = self.peer.close().await;
        Err(BlnkError::Session(message.to_owned()))
    }
}

fn is_data_channel_closed_send_error(error: &BlnkError) -> bool {
    matches!(
        error,
        BlnkError::Peer(message) if message == "send SWSP frame: data channel closed"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::TwoPeerHarness;
    use crate::stream::shell::{
        decode_cancel_frame, decode_exit_frame, decode_input_frame, decode_open_frame,
        decode_output_frame,
    };

    fn fixture_shell_program() -> String {
        #[cfg(windows)]
        {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned())
        }
        #[cfg(not(windows))]
        {
            "/bin/sh".to_owned()
        }
    }

    fn fixture_shell_command(script: &str) -> ShellCommand {
        #[cfg(windows)]
        {
            ShellCommand::new(fixture_shell_program()).args(["/C", script])
        }
        #[cfg(not(windows))]
        {
            ShellCommand::new(fixture_shell_program()).args(["-c", script])
        }
    }

    async fn connected_runtime_pair(
        server_config: SessionRuntimeConfig,
        client_config: SessionRuntimeConfig,
    ) -> (SessionRuntime, SessionRuntime) {
        let harness = TwoPeerHarness::new("session-control")
            .await
            .expect("local peer harness should connect");
        let mut server = SessionRuntime::new(harness.offerer, server_config)
            .expect("server runtime should build");
        let mut client = SessionRuntime::new(harness.answerer, client_config)
            .expect("client runtime should build");
        let (server_result, client_result) = tokio::join!(server.handshake(), client.handshake());
        server_result.expect("server handshake should succeed");
        client_result.expect("client handshake should succeed");
        (server, client)
    }

    #[test]
    fn control_messages_round_trip_with_wire_discriminators() {
        let messages = [
            ControlMessage::connect("/", Some(PROTOCOL_VERSION)).expect("connect"),
            ControlMessage::auth_required(),
            ControlMessage::auth("123456").expect("auth"),
            ControlMessage::auth_result(true),
            ControlMessage::ready(PROTOCOL_VERSION, vec!["session".into()], "direct"),
            ControlMessage::error("failure").expect("error"),
        ];

        for message in messages {
            let decoded = ControlMessage::decode(&message.encode()).expect("control decode");
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn only_data_channel_close_send_error_is_tolerated_during_shutdown() {
        assert!(is_data_channel_closed_send_error(&BlnkError::Peer(
            "send SWSP frame: data channel closed".to_owned(),
        )));
        assert!(!is_data_channel_closed_send_error(&BlnkError::Peer(
            "send SWSP frame: permission denied".to_owned(),
        )));
        assert!(!is_data_channel_closed_send_error(&BlnkError::Protocol(
            "data channel closed".to_owned(),
        )));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn authenticated_local_session_reaches_ready_and_cleans_streams() {
        let (mut server, mut client) = connected_runtime_pair(
            SessionRuntimeConfig::server("123456"),
            SessionRuntimeConfig::client("123456"),
        )
        .await;

        assert_eq!(server.state(), SessionState::Ready);
        assert_eq!(client.state(), SessionState::Ready);
        assert_eq!(server.active_streams(), 0);

        for kind in [StreamKind::Tcp, StreamKind::WebSocket, StreamKind::Http] {
            let error = server
                .open_stream(kind, "/proxy")
                .expect_err("unserviced proxy stream must be rejected");
            assert!(
                error
                    .to_string()
                    .contains("proxy stream dispatch is not implemented")
            );
        }

        let stream = server
            .open_stream(StreamKind::Shell, "/shell")
            .expect("ready runtime should open stream");
        assert_eq!(stream.id(), 1);
        assert_eq!(server.active_streams(), 1);
        server
            .close_stream(stream.id())
            .expect("runtime should close stream");
        assert_eq!(server.active_streams(), 0);

        server.close().await.expect("server close");
        client.close().await.expect("client close");
        assert_eq!(server.state(), SessionState::Closed);
        assert_eq!(client.state(), SessionState::Closed);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wrong_pin_retry_exhaustion_closes_both_runtimes() {
        let harness = TwoPeerHarness::new("retry")
            .await
            .expect("local peer harness should connect");
        let server_config =
            SessionRuntimeConfig::server("123456").with_session_config(SessionConfig {
                pin_fail_delay: Duration::ZERO,
                ..SessionConfig::default()
            });
        let client_config = SessionRuntimeConfig::client("000001").with_auth_pins(vec![
            "000001".into(),
            "000002".into(),
            "000003".into(),
        ]);
        let mut server = SessionRuntime::new(harness.offerer, server_config).expect("server");
        let mut client = SessionRuntime::new(harness.answerer, client_config).expect("client");

        let (server_result, client_result) = tokio::join!(server.handshake(), client.handshake());
        assert!(server_result.is_err());
        assert!(client_result.is_err());
        assert_eq!(server.state(), SessionState::Closed);
        assert_eq!(client.state(), SessionState::Closed);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn duplicate_connect_is_rejected_after_ready() {
        let (mut server, mut client) = connected_runtime_pair(
            SessionRuntimeConfig::server("123456"),
            SessionRuntimeConfig::client("123456"),
        )
        .await;

        client
            .send_control(
                ControlMessage::connect("/replay", Some(PROTOCOL_VERSION))
                    .expect("duplicate connect"),
            )
            .await
            .expect("duplicate frame should be sent");
        let error = server
            .run_until_disconnect()
            .await
            .expect_err("duplicate connect must be rejected");
        assert!(error.to_string().contains("duplicate connect"));
        let _ = client.close().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn control_timeout_closes_session_without_peer_message() {
        let harness = TwoPeerHarness::new("timeout")
            .await
            .expect("local peer harness should connect");
        let config =
            SessionRuntimeConfig::server("123456").with_control_timeout(Duration::from_millis(10));
        let mut server = SessionRuntime::new(harness.offerer, config).expect("server runtime");
        let idle_peer = harness.answerer;

        let error = server
            .handshake()
            .await
            .expect_err("missing connect must time out");
        assert!(error.to_string().contains("timed out"));
        assert_eq!(server.state(), SessionState::Closed);
        server.close().await.expect("server close");
        idle_peer.close().await.expect("idle peer close");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn disconnect_clears_active_streams_in_runtime() {
        let (mut server, mut client) = connected_runtime_pair(
            SessionRuntimeConfig::server("123456"),
            SessionRuntimeConfig::client("123456"),
        )
        .await;
        server
            .open_stream(StreamKind::Tcp, "127.0.0.1:9")
            .expect("stream should open");
        assert_eq!(server.active_streams(), 1);

        let server_task = tokio::spawn(async move {
            let result = server.run_until_disconnect().await;
            (server, result)
        });
        client.close().await.expect("client close");
        let (server, result) = server_task.await.expect("server task should join");
        result.expect("disconnect should be handled");
        assert_eq!(server.state(), SessionState::Closed);
        assert_eq!(server.active_streams(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shell_stream_round_trips_control_and_output_frames() {
        let (mut server, mut client) = connected_runtime_pair(
            SessionRuntimeConfig::server("123456"),
            SessionRuntimeConfig::client("123456"),
        )
        .await;

        let client_stream = client
            .open_shell_stream("/shell")
            .expect("client should open shell stream");
        let stream_id = client_stream.id();
        let command = fixture_shell_command("echo hello");
        client
            .send_shell_open(stream_id, &command)
            .await
            .expect("client should send shell open");

        let open_frame = server
            .recv_shell_frame()
            .await
            .expect("server should receive peer SYN before accept");
        assert_eq!(
            decode_open_frame(&open_frame).expect("open decode"),
            command
        );
        server
            .accept_shell_stream(stream_id, "/shell")
            .expect("server should accept peer shell stream");

        client
            .send_shell_input(stream_id, b"stdin".to_vec(), false)
            .await
            .expect("client should send stdin");
        let input_frame = server
            .recv_shell_frame()
            .await
            .expect("server should receive stdin");
        assert_eq!(
            decode_input_frame(&input_frame).expect("input decode").data,
            b"stdin"
        );

        server
            .send_shell_output(stream_id, b"stdout".to_vec(), false)
            .await
            .expect("server should send stdout");
        let output_frame = client
            .recv_shell_frame()
            .await
            .expect("client should receive stdout");
        assert_eq!(
            decode_output_frame(&output_frame)
                .expect("output decode")
                .data,
            b"stdout"
        );

        server
            .send_shell_exit(stream_id, 0, false)
            .await
            .expect("server should send process exit");
        let exit_frame = client
            .recv_shell_frame()
            .await
            .expect("client should receive process exit");
        assert_eq!(decode_exit_frame(&exit_frame).expect("exit decode").code, 0);

        client
            .close_shell_stream(stream_id)
            .await
            .expect("client should close shell stream");
        let client_fin = server
            .recv_shell_frame()
            .await
            .expect("server should receive client FIN");
        assert!(client_fin.flags.is_fin());
        server
            .close_shell_stream(stream_id)
            .await
            .expect("server should close shell stream");
        assert_eq!(client.active_streams(), 0);
        assert_eq!(server.active_streams(), 0);

        let cancel_stream = client
            .open_shell_stream("/cancel")
            .expect("client should open cancellation stream");
        let cancel_id = cancel_stream.id();
        client
            .send_shell_open(cancel_id, &ShellCommand::new(fixture_shell_program()))
            .await
            .expect("client should send cancellation open");
        let cancel_open = server
            .recv_shell_frame()
            .await
            .expect("server should receive cancellation SYN");
        assert_eq!(
            decode_open_frame(&cancel_open)
                .expect("cancellation open decode")
                .program(),
            fixture_shell_program().as_str()
        );
        server
            .accept_shell_stream(cancel_id, "/cancel")
            .expect("server should accept cancellation stream");
        client
            .send_shell_cancel(cancel_id)
            .await
            .expect("client should send cancellation");
        let cancel_frame = server
            .recv_shell_frame()
            .await
            .expect("server should receive cancellation");
        assert!(
            decode_cancel_frame(&cancel_frame)
                .expect("cancellation decode")
                .requested
        );
        client
            .close_shell_stream(cancel_id)
            .await
            .expect("client should close cancellation stream");
        server
            .close_shell_stream(cancel_id)
            .await
            .expect("server should close cancellation stream");
        assert_eq!(client.active_streams(), 0);
        assert_eq!(server.active_streams(), 0);

        server.close().await.expect("server close");
        client.close().await.expect("client close");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn runtime_rejects_empty_client_pin_configuration() {
        let harness = TwoPeerHarness::new("config")
            .await
            .expect("local peer harness should connect");
        let config = SessionRuntimeConfig::client("123456").with_auth_pins(Vec::new());
        let result = SessionRuntime::new(harness.offerer, config);
        let error = match result {
            Ok(_) => panic!("empty client PIN list should be rejected"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("at least one authentication PIN")
        );
        let peer = harness.answerer;
        peer.close().await.expect("peer close");
    }

    #[test]
    fn session_runtime_config_debug_redacts_pins() {
        let server_config = SessionRuntimeConfig::server("secret-server-pin-123456");
        let server_debug = format!("{server_config:?}");
        assert!(!server_debug.contains("secret-server-pin-123456"));
        assert!(server_debug.contains("<redacted>"));

        let client_config = SessionRuntimeConfig::client("secret-client-pin-654321");
        let client_debug = format!("{client_config:?}");
        assert!(!client_debug.contains("secret-client-pin-654321"));
        assert!(client_debug.contains("<redacted>"));
    }
}
