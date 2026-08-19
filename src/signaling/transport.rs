//! WebSocket signaling transport and deterministic local fixture server.
//!
//! The transport deliberately uses JSON text frames because the existing
//! signaling contract describes a JSON/WebSocket boundary. Internal Rust field
//! names are adapted to the wire schema here: `message_type` becomes `type`
//! and `pairing_code` becomes `code`. No external signaling provider or
//! original-client interoperability is implied by this module.

use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, accept_async, connect_async, tungstenite::Message,
};
use url::Url;

use super::{
    AnswerMessage, ConnectionRequest, IceCandidateMessage, IceServer, OfferMessage, PairAnswer,
    PairApproved, PairRejected, PairRequest, RegisterRequest, RegisterResponse, SignalingError,
};
use crate::utils::error::BlnkError;

pub type TransportResult<T> = Result<T, BlnkError>;

pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalingMessage {
    Register(RegisterRequest),
    Registered(RegisterResponse),
    Request(ConnectionRequest),
    Offer(OfferMessage),
    Answer(AnswerMessage),
    Candidate(IceCandidateMessage),
    PairRequest(PairRequest),
    PairAnswer(PairAnswer),
    PairApproved(PairApproved),
    PairRejected(PairRejected),
    Error(SignalingError),
}

impl SignalingMessage {
    pub fn validate(&self) -> TransportResult<()> {
        match self {
            Self::Register(message) => message.validate(),
            Self::Registered(message) => message.validate(),
            Self::Request(message) => message.validate(),
            Self::Offer(message) => message.validate(),
            Self::Answer(message) => message.validate(),
            Self::Candidate(message) => message.validate(),
            Self::PairRequest(message) => message.validate(),
            Self::PairAnswer(message) => message.validate(),
            Self::PairApproved(message) => message.validate(),
            Self::PairRejected(message) => message.validate(),
            Self::Error(message) => message.validate(),
        }
    }

    pub fn message_type(&self) -> &'static str {
        match self {
            Self::Register(_) => "register",
            Self::Registered(_) => "registered",
            Self::Request(_) => "request",
            Self::Offer(_) => "offer",
            Self::Answer(_) => "answer",
            Self::Candidate(_) => "candidate",
            Self::PairRequest(_) => "pair_request",
            Self::PairAnswer(_) => "pair_answer",
            Self::PairApproved(_) => "pair_approved",
            Self::PairRejected(_) => "pair_rejected",
            Self::Error(_) => "error",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum WireMessage {
    #[serde(rename = "register")]
    Register {
        uid: String,
        public_key: String,
        protocol: i32,
        want_code: bool,
    },
    #[serde(rename = "registered")]
    Registered { code: String },
    #[serde(rename = "request")]
    Request {
        client_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        browser_ip: Option<String>,
        #[serde(default)]
        ice_servers: Vec<WireIceServer>,
        #[serde(default)]
        force_relay: bool,
    },
    #[serde(rename = "offer")]
    Offer {
        client_id: String,
        sdp: String,
        #[serde(default)]
        streams: BTreeMap<String, String>,
    },
    #[serde(rename = "answer")]
    Answer {
        client_id: String,
        sdp: String,
        encrypted_request: String,
    },
    #[serde(rename = "candidate")]
    Candidate {
        client_id: String,
        candidate: BTreeMap<String, String>,
    },
    #[serde(rename = "pair_request")]
    PairRequest {
        client_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remote_ip: Option<String>,
        #[serde(default)]
        ice_servers: Vec<WireIceServer>,
    },
    #[serde(rename = "pair_answer")]
    PairAnswer { client_id: String, sdp: String },
    #[serde(rename = "pair_approved")]
    PairApproved { client_id: String },
    #[serde(rename = "pair_rejected")]
    PairRejected { client_id: String, reason: String },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct WireIceServer {
    urls: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credential: Option<String>,
}

impl From<&IceServer> for WireIceServer {
    fn from(server: &IceServer) -> Self {
        Self {
            urls: server.urls.clone(),
            username: server.username.clone(),
            credential: server.credential.clone(),
        }
    }
}

impl TryFrom<WireIceServer> for IceServer {
    type Error = BlnkError;

    fn try_from(server: WireIceServer) -> Result<Self, Self::Error> {
        let server = Self {
            urls: server.urls,
            username: server.username,
            credential: server.credential,
        };
        server.validate()?;
        Ok(server)
    }
}

impl SignalingMessage {
    fn to_wire(&self) -> TransportResult<WireMessage> {
        self.validate()?;
        Ok(match self {
            Self::Register(message) => WireMessage::Register {
                uid: message.uid.clone(),
                public_key: message.public_key.clone(),
                protocol: message.protocol,
                want_code: message.want_code,
            },
            Self::Registered(message) => WireMessage::Registered {
                code: message.pairing_code.clone(),
            },
            Self::Request(message) => WireMessage::Request {
                client_id: message.client_id.clone(),
                browser_ip: message.browser_ip.clone(),
                ice_servers: message
                    .ice_servers
                    .iter()
                    .map(WireIceServer::from)
                    .collect(),
                force_relay: message.force_relay,
            },
            Self::Offer(message) => WireMessage::Offer {
                client_id: message.client_id.clone(),
                sdp: message.sdp.clone(),
                streams: message
                    .streams
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                value,
                            ),
                        )
                    })
                    .collect(),
            },
            Self::Answer(message) => WireMessage::Answer {
                client_id: message.client_id.clone(),
                sdp: message.sdp.clone(),
                encrypted_request: message.encrypted_request.clone(),
            },
            Self::Candidate(message) => WireMessage::Candidate {
                client_id: message.client_id.clone(),
                candidate: message.candidate.clone(),
            },
            Self::PairRequest(message) => WireMessage::PairRequest {
                client_id: message.client_id.clone(),
                remote_ip: message.remote_ip.clone(),
                ice_servers: message
                    .ice_servers
                    .iter()
                    .map(WireIceServer::from)
                    .collect(),
            },
            Self::PairAnswer(message) => WireMessage::PairAnswer {
                client_id: message.client_id.clone(),
                sdp: message.sdp.clone(),
            },
            Self::PairApproved(message) => WireMessage::PairApproved {
                client_id: message.client_id.clone(),
            },
            Self::PairRejected(message) => WireMessage::PairRejected {
                client_id: message.client_id.clone(),
                reason: message.reason.clone(),
            },
            Self::Error(message) => WireMessage::Error {
                message: message.message.clone(),
            },
        })
    }
}

impl TryFrom<WireMessage> for SignalingMessage {
    type Error = BlnkError;

    fn try_from(message: WireMessage) -> Result<Self, BlnkError> {
        let message = match message {
            WireMessage::Register {
                uid,
                public_key,
                protocol,
                want_code,
            } => Self::Register(RegisterRequest {
                message_type: "register".into(),
                uid,
                public_key,
                protocol,
                want_code,
            }),
            WireMessage::Registered { code } => Self::Registered(RegisterResponse {
                message_type: "registered".into(),
                pairing_code: code,
            }),
            WireMessage::Request {
                client_id,
                browser_ip,
                ice_servers,
                force_relay,
            } => Self::Request(ConnectionRequest {
                message_type: "request".into(),
                client_id,
                browser_ip,
                ice_servers: ice_servers
                    .into_iter()
                    .map(IceServer::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
                force_relay,
            }),
            WireMessage::Offer {
                client_id,
                sdp,
                streams,
            } => Self::Offer(OfferMessage {
                message_type: "offer".into(),
                client_id,
                sdp,
                streams: streams
                    .into_iter()
                    .map(|(key, value)| {
                        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, value)
                            .map(|decoded| (key, decoded))
                            .map_err(|error| {
                                BlnkError::Signaling(format!(
                                    "invalid base64 streams entry: {error}"
                                ))
                            })
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?,
            }),
            WireMessage::Answer {
                client_id,
                sdp,
                encrypted_request,
            } => Self::Answer(AnswerMessage {
                message_type: "answer".into(),
                client_id,
                sdp,
                encrypted_request,
            }),
            WireMessage::Candidate {
                client_id,
                candidate,
            } => Self::Candidate(IceCandidateMessage {
                message_type: "candidate".into(),
                client_id,
                candidate,
            }),
            WireMessage::PairRequest {
                client_id,
                remote_ip,
                ice_servers,
            } => Self::PairRequest(PairRequest {
                message_type: "pair_request".into(),
                client_id,
                remote_ip,
                ice_servers: ice_servers
                    .into_iter()
                    .map(IceServer::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            WireMessage::PairAnswer { client_id, sdp } => Self::PairAnswer(PairAnswer {
                message_type: "pair_answer".into(),
                client_id,
                sdp,
            }),
            WireMessage::PairApproved { client_id } => Self::PairApproved(PairApproved {
                message_type: "pair_approved".into(),
                client_id,
            }),
            WireMessage::PairRejected { client_id, reason } => Self::PairRejected(PairRejected {
                message_type: "pair_rejected".into(),
                client_id,
                reason,
            }),
            WireMessage::Error { message } => Self::Error(SignalingError {
                message_type: "error".into(),
                message,
            }),
        };
        message.validate()?;
        Ok(message)
    }
}

pub fn encode_message(message: &SignalingMessage) -> TransportResult<String> {
    let wire = message.to_wire()?;
    serde_json::to_string(&wire).map_err(|error| {
        BlnkError::Signaling(format!("failed to encode signaling message: {error}"))
    })
}

pub fn decode_message(payload: &str) -> TransportResult<SignalingMessage> {
    let wire = serde_json::from_str::<WireMessage>(payload).map_err(|error| {
        BlnkError::Signaling(format!("failed to decode signaling message: {error}"))
    })?;
    wire.try_into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub max_retries: usize,
    pub retry_delay: Duration,
}

impl ReconnectPolicy {
    pub const fn none() -> Self {
        Self {
            max_retries: 0,
            retry_delay: Duration::ZERO,
        }
    }

    pub const fn limited(max_retries: usize, retry_delay: Duration) -> Self {
        Self {
            max_retries,
            retry_delay,
        }
    }
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone)]
pub struct SignalingClient {
    endpoint: Url,
    max_message_size: usize,
    reconnect: ReconnectPolicy,
}

impl SignalingClient {
    pub fn new(endpoint: impl AsRef<str>) -> TransportResult<Self> {
        let endpoint = Url::parse(endpoint.as_ref())
            .map_err(|error| BlnkError::Signaling(format!("invalid signaling URL: {error}")))?;
        if !matches!(endpoint.scheme(), "ws" | "wss") || endpoint.host_str().is_none() {
            return Err(BlnkError::Signaling(
                "signaling URL must use ws or wss and include a host".into(),
            ));
        }
        Ok(Self {
            endpoint,
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
            reconnect: ReconnectPolicy::default(),
        })
    }

    pub fn with_max_message_size(mut self, max_message_size: usize) -> TransportResult<Self> {
        if max_message_size == 0 {
            return Err(BlnkError::Signaling(
                "maximum signaling message size must be greater than zero".into(),
            ));
        }
        self.max_message_size = max_message_size;
        Ok(self)
    }

    pub fn with_reconnect_policy(mut self, reconnect: ReconnectPolicy) -> Self {
        self.reconnect = reconnect;
        self
    }

    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    pub async fn connect(&self) -> TransportResult<SignalingConnection> {
        let (socket, _) = connect_async(self.endpoint.as_str())
            .await
            .map_err(|error| {
                BlnkError::Signaling(format!("failed to connect signaling socket: {error}"))
            })?;
        WebSocketConnection::new(socket, self.max_message_size)
    }

    pub async fn connect_with_retry(&self) -> TransportResult<SignalingConnection> {
        let mut attempt = 0;
        loop {
            match self.connect().await {
                Ok(connection) => return Ok(connection),
                Err(error) if attempt < self.reconnect.max_retries => {
                    attempt += 1;
                    if !self.reconnect.retry_delay.is_zero() {
                        tokio::time::sleep(self.reconnect.retry_delay).await;
                    }
                    let _ = error;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub async fn transact(&self, request: &SignalingMessage) -> TransportResult<SignalingMessage> {
        let mut connection = self.connect().await?;
        connection.send(request).await?;
        connection
            .recv()
            .await?
            .ok_or_else(|| BlnkError::Signaling("signaling peer closed before response".into()))
    }

    pub async fn transact_with_retry(
        &self,
        request: &SignalingMessage,
    ) -> TransportResult<SignalingMessage> {
        let mut attempt = 0;
        loop {
            match self.transact(request).await {
                Ok(response) => return Ok(response),
                Err(error) if attempt < self.reconnect.max_retries => {
                    attempt += 1;
                    if !self.reconnect.retry_delay.is_zero() {
                        tokio::time::sleep(self.reconnect.retry_delay).await;
                    }
                    let _ = error;
                }
                Err(error) => return Err(error),
            }
        }
    }
}

pub type SignalingConnection = WebSocketConnection<MaybeTlsStream<TcpStream>>;

pub struct WebSocketConnection<S> {
    socket: WebSocketStream<S>,
    max_message_size: usize,
}

impl<S> WebSocketConnection<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    fn new(socket: WebSocketStream<S>, max_message_size: usize) -> TransportResult<Self> {
        if max_message_size == 0 {
            return Err(BlnkError::Signaling(
                "maximum signaling message size must be greater than zero".into(),
            ));
        }
        Ok(Self {
            socket,
            max_message_size,
        })
    }

    pub async fn send(&mut self, message: &SignalingMessage) -> TransportResult<()> {
        let payload = encode_message(message)?;
        self.send_text(payload).await
    }

    async fn send_text(&mut self, payload: String) -> TransportResult<()> {
        if payload.len() > self.max_message_size {
            return Err(BlnkError::Signaling(format!(
                "signaling message exceeds {} byte limit",
                self.max_message_size
            )));
        }
        self.socket
            .send(Message::Text(payload.into()))
            .await
            .map_err(|error| {
                BlnkError::Signaling(format!("failed to send signaling message: {error}"))
            })
    }

    pub async fn recv(&mut self) -> TransportResult<Option<SignalingMessage>> {
        while let Some(frame) = self.socket.next().await {
            let frame = frame.map_err(|error| {
                BlnkError::Signaling(format!("signaling socket receive error: {error}"))
            })?;
            match frame {
                Message::Text(payload) => {
                    if payload.len() > self.max_message_size {
                        return Err(BlnkError::Signaling(format!(
                            "received signaling message exceeds {} byte limit",
                            self.max_message_size
                        )));
                    }
                    return decode_message(payload.as_ref()).map(Some);
                }
                Message::Binary(_) => {
                    return Err(BlnkError::Signaling(
                        "signaling transport accepts JSON text frames only".into(),
                    ));
                }
                Message::Ping(payload) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|error| {
                            BlnkError::Signaling(format!(
                                "failed to respond to signaling ping: {error}"
                            ))
                        })?;
                }
                Message::Pong(_) => {}
                Message::Close(_) => return Ok(None),
                _ => {}
            }
        }
        Err(BlnkError::Signaling("signaling connection dropped".into()))
    }

    pub async fn close(&mut self) -> TransportResult<()> {
        self.socket
            .send(Message::Close(None))
            .await
            .map_err(|error| {
                BlnkError::Signaling(format!("failed to close signaling socket: {error}"))
            })
    }
}

#[derive(Debug, Clone)]
pub struct FixtureConfig {
    pub pairing_code: String,
    pub close_first_connection_before_response: bool,
    pub close_first_connection_after_response: bool,
    pub send_malformed_response_on_first_register: bool,
}

impl Default for FixtureConfig {
    fn default() -> Self {
        Self {
            pairing_code: "123456".into(),
            close_first_connection_before_response: false,
            close_first_connection_after_response: false,
            send_malformed_response_on_first_register: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FixtureSnapshot {
    pub connection_count: usize,
    pub received_messages: Vec<SignalingMessage>,
}

pub struct LocalFixtureServer {
    address: SocketAddr,
    state: Arc<Mutex<FixtureSnapshot>>,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl LocalFixtureServer {
    pub async fn start() -> TransportResult<Self> {
        Self::start_with_config(FixtureConfig::default()).await
    }

    pub async fn start_with_config(config: FixtureConfig) -> TransportResult<Self> {
        RegisterResponse::new(config.pairing_code.clone())?;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let state = Arc::new(Mutex::new(FixtureSnapshot::default()));
        let task_state = Arc::clone(&state);
        let (shutdown, mut shutdown_receiver) = oneshot::channel();

        let task = tokio::spawn(async move {
            let mut connection_index = 0usize;
            loop {
                tokio::select! {
                    _ = &mut shutdown_receiver => break,
                    accepted = listener.accept() => {
                        let Ok((stream, _peer)) = accepted else { break };
                        let current_index = connection_index;
                        connection_index = connection_index.saturating_add(1);
                        let connection_state = Arc::clone(&task_state);
                        let connection_config = config.clone();
                        tokio::spawn(async move {
                            let Ok(socket) = accept_async(stream).await else { return };
                            {
                                let mut snapshot = connection_state.lock().await;
                                snapshot.connection_count = snapshot.connection_count.saturating_add(1);
                            }
                            let Ok(mut connection) = WebSocketConnection::new(socket, DEFAULT_MAX_MESSAGE_SIZE) else {
                                return;
                            };
                            fixture_connection_loop(
                                &mut connection,
                                current_index,
                                connection_config,
                                connection_state,
                            )
                            .await;
                        });
                    }
                }
            }
        });

        Ok(Self {
            address,
            state,
            shutdown: Some(shutdown),
            task: Some(task),
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn url(&self) -> String {
        format!("ws://{}", self.address)
    }

    pub async fn snapshot(&self) -> FixtureSnapshot {
        self.state.lock().await.clone()
    }

    pub async fn shutdown(mut self) -> TransportResult<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.await.map_err(|error| {
                BlnkError::Signaling(format!("fixture server task failed: {error}"))
            })?;
        }
        Ok(())
    }
}

impl Drop for LocalFixtureServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn fixture_connection_loop(
    connection: &mut WebSocketConnection<TcpStream>,
    connection_index: usize,
    config: FixtureConfig,
    state: Arc<Mutex<FixtureSnapshot>>,
) {
    loop {
        let message = match connection.recv().await {
            Ok(Some(message)) => message,
            Ok(None) | Err(_) => return,
        };
        {
            let mut snapshot = state.lock().await;
            snapshot.received_messages.push(message.clone());
        }

        if let SignalingMessage::Register(_) = message {
            if connection_index == 0 && config.close_first_connection_before_response {
                let _ = connection.close().await;
                return;
            }
            if connection_index == 0 && config.send_malformed_response_on_first_register {
                let _ = connection.send_text("{not valid json".into()).await;
                return;
            }
            let Ok(response) = RegisterResponse::new(config.pairing_code.clone()) else {
                return;
            };
            if connection
                .send(&SignalingMessage::Registered(response))
                .await
                .is_err()
            {
                return;
            }
            if connection_index == 0 && config.close_first_connection_after_response {
                let _ = connection.close().await;
                return;
            }
        } else if let Ok(error) =
            SignalingError::new("fixture does not implement this message flow")
        {
            let _ = connection.send(&SignalingMessage::Error(error)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn codec_adapts_internal_names_to_wire_schema() {
        let response = SignalingMessage::Registered(
            RegisterResponse::new("123456").expect("fixture code is valid"),
        );
        let encoded = encode_message(&response).expect("message encodes");
        let json: serde_json::Value = serde_json::from_str(&encoded).expect("valid JSON");
        assert_eq!(json["type"], "registered");
        assert_eq!(json["code"], "123456");
        assert!(json.get("message_type").is_none());
        assert!(json.get("pairing_code").is_none());
        assert_eq!(decode_message(&encoded).expect("message decodes"), response);
    }

    #[test]
    fn codec_rejects_wrong_discriminator_and_invalid_json() {
        let wrong_type = r#"{"type":"not-a-signaling-message"}"#;
        assert!(decode_message(wrong_type).is_err());
        assert!(decode_message("{not valid json").is_err());
        assert!(decode_message(r#"{"message_type":"register"}"#).is_err());
    }

    #[tokio::test]
    async fn fixture_supports_request_response_flow() {
        let fixture = LocalFixtureServer::start().await.expect("fixture starts");
        let client = SignalingClient::new(fixture.url()).expect("client URL is valid");
        let request = SignalingMessage::Register(
            RegisterRequest::new("uid", "public-key", true).expect("request is valid"),
        );
        let response = client.transact(&request).await.expect("response arrives");
        assert_eq!(
            response,
            SignalingMessage::Registered(
                RegisterResponse::new("123456").expect("response is valid")
            )
        );
        let snapshot = fixture.snapshot().await;
        assert_eq!(snapshot.connection_count, 1);
        assert_eq!(snapshot.received_messages, vec![request]);
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[tokio::test]
    async fn malformed_response_is_propagated_without_claiming_compatibility() {
        let fixture = LocalFixtureServer::start_with_config(FixtureConfig {
            send_malformed_response_on_first_register: true,
            ..FixtureConfig::default()
        })
        .await
        .expect("fixture starts");
        let client = SignalingClient::new(fixture.url()).expect("client URL is valid");
        let request = SignalingMessage::Register(
            RegisterRequest::new("uid", "public-key", true).expect("request is valid"),
        );
        let error = client
            .transact(&request)
            .await
            .expect_err("malformed frame fails");
        assert!(
            error
                .to_string()
                .contains("failed to decode signaling message")
        );
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[tokio::test]
    async fn graceful_disconnect_is_distinguished_from_a_dropped_socket() {
        let fixture = LocalFixtureServer::start_with_config(FixtureConfig {
            close_first_connection_after_response: true,
            ..FixtureConfig::default()
        })
        .await
        .expect("fixture starts");
        let client = SignalingClient::new(fixture.url()).expect("client URL is valid");
        let mut connection = client.connect().await.expect("connection succeeds");
        let request = SignalingMessage::Register(
            RegisterRequest::new("uid", "public-key", true).expect("request is valid"),
        );
        connection.send(&request).await.expect("request sends");
        assert!(
            connection
                .recv()
                .await
                .expect("response succeeds")
                .is_some()
        );
        assert!(connection.recv().await.expect("close is clean").is_none());
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[tokio::test]
    async fn reconnect_policy_retries_after_first_fixture_disconnect() {
        let fixture = LocalFixtureServer::start_with_config(FixtureConfig {
            close_first_connection_before_response: true,
            ..FixtureConfig::default()
        })
        .await
        .expect("fixture starts");
        let client = SignalingClient::new(fixture.url())
            .expect("client URL is valid")
            .with_reconnect_policy(ReconnectPolicy::limited(1, Duration::from_millis(1)));
        let request = SignalingMessage::Register(
            RegisterRequest::new("uid", "public-key", true).expect("request is valid"),
        );
        let response = client
            .transact_with_retry(&request)
            .await
            .expect("second connection succeeds");
        assert!(matches!(response, SignalingMessage::Registered(_)));
        let snapshot = fixture.snapshot().await;
        assert_eq!(snapshot.connection_count, 2);
        assert_eq!(snapshot.received_messages, vec![request.clone(), request]);
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[tokio::test]
    async fn fixture_error_message_is_returned_as_a_typed_response() {
        let fixture = LocalFixtureServer::start().await.expect("fixture starts");
        let client = SignalingClient::new(fixture.url()).expect("client URL is valid");
        let request =
            SignalingMessage::PairApproved(PairApproved::new("client").expect("request is valid"));
        let response = client
            .transact(&request)
            .await
            .expect("error response arrives");
        match response {
            SignalingMessage::Error(error) => {
                assert_eq!(
                    error.message,
                    "fixture does not implement this message flow"
                );
            }
            other => panic!("expected typed fixture error, got {other:?}"),
        }
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[tokio::test]
    async fn oversized_outbound_messages_are_rejected_before_send() {
        let fixture = LocalFixtureServer::start().await.expect("fixture starts");
        let client = SignalingClient::new(fixture.url())
            .expect("client URL is valid")
            .with_max_message_size(32)
            .expect("positive size");
        let request = SignalingMessage::Register(
            RegisterRequest::new("uid", "public-key", true).expect("request is valid"),
        );
        let error = client
            .transact(&request)
            .await
            .expect_err("size limit applies");
        assert!(error.to_string().contains("exceeds 32 byte limit"));
        fixture.shutdown().await.expect("fixture shuts down");
    }
}
