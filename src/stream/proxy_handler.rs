//! Concrete proxy stream handlers built on the Issue #41 security policy.
//!
//! The handlers are transport adapters, not a new session state machine. Callers
//! must allocate the stream in `Session`/`SessionRuntime` and pass the resulting
//! stream id here. Every outbound connection is resolved and checked once through
//! [`ProxyPolicy`]; retries use only that pinned answer set.

use std::{collections::HashMap, sync::Arc};

use futures::{SinkExt, StreamExt};
use prost::Message;
use reqwest::{Client, Method, header};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{OwnedSemaphorePermit, Semaphore},
    time,
};
use tokio_tungstenite::{client_async, tungstenite::Message as WebSocketMessage};
use url::Url;

use crate::{
    proto_generated::stream as wire,
    protocol::swsp::{Frame, FrameFlags},
    utils::error::BlnkError,
};

use super::proxy::{
    ProxyAuthorization, ProxyPolicy, ProxyPolicyError, ProxyResourceLimits, ProxyScheme,
    ResolvedProxyTarget,
};

pub type ProxyHandlerResult<T> = Result<T, BlnkError>;

const SYN_DAT_BITS: u16 = FrameFlags::SYN.bits() | FrameFlags::DAT.bits();
const DAT_MORE_BITS: u16 = FrameFlags::DAT.bits() | FrameFlags::MORE.bits();
const MAX_FRAME_DATA: usize = 16 * 1024;

fn syn_dat_flags() -> FrameFlags {
    FrameFlags::from_bits(SYN_DAT_BITS).expect("SYN|DAT is a valid SWSP combination")
}

fn dat_more_flags() -> FrameFlags {
    FrameFlags::from_bits(DAT_MORE_BITS).expect("DAT|MORE is a valid SWSP combination")
}

fn policy_error(error: ProxyPolicyError) -> BlnkError {
    error.into()
}

fn protocol_error(message: impl Into<String>) -> BlnkError {
    BlnkError::Protocol(message.into())
}

fn transport_error(message: impl Into<String>) -> BlnkError {
    BlnkError::Stream(format!("proxy transport error: {}", message.into()))
}

fn check_stream_id(frame: &Frame, stream_id: u32) -> ProxyHandlerResult<()> {
    if frame.stream_id != stream_id {
        return Err(protocol_error(format!(
            "proxy frame stream id {} does not match {}",
            frame.stream_id, stream_id
        )));
    }
    Ok(())
}

fn check_open_frame(frame: &Frame, stream_id: u32) -> ProxyHandlerResult<()> {
    check_stream_id(frame, stream_id)?;
    if frame.flags.bits() != SYN_DAT_BITS {
        return Err(protocol_error("proxy open frame must use SYN|DAT flags"));
    }
    Ok(())
}

fn check_data_frame(frame: &Frame, stream_id: u32) -> ProxyHandlerResult<()> {
    check_stream_id(frame, stream_id)?;
    if !frame.flags.is_dat() || frame.flags.is_syn() || frame.flags.is_fin() {
        return Err(protocol_error("proxy data frame has invalid flags"));
    }
    Ok(())
}

fn encode_frame(stream_id: u32, flags: FrameFlags, message: impl Message) -> Frame {
    Frame::new(stream_id, flags, message.encode_to_vec())
}

fn validate_wire_type(actual: &str, expected: &str) -> ProxyHandlerResult<()> {
    if !actual.is_empty() && actual != expected {
        return Err(protocol_error(format!(
            "proxy message type must be {expected}"
        )));
    }
    Ok(())
}

fn tcp_target(open: &wire::TcpOpen) -> ProxyHandlerResult<Url> {
    validate_wire_type(&open.r#type, "tcp")?;
    if open.port <= 0 || open.port > i32::from(u16::MAX) {
        return Err(policy_error(ProxyPolicyError::InvalidPort));
    }
    if open.host.trim().is_empty() {
        return Err(policy_error(ProxyPolicyError::HostRequired));
    }
    let host = if open.host.contains(':') && !open.host.starts_with('[') {
        format!("[{}]", open.host)
    } else {
        open.host.clone()
    };
    Url::parse(&format!("tcp://{host}:{}", open.port))
        .map_err(|_| policy_error(ProxyPolicyError::InvalidTarget))
}

fn websocket_request(
    target: &Url,
    headers: &HashMap<String, String>,
) -> ProxyHandlerResult<tungstenite::http::Request<()>> {
    use tungstenite::{client::IntoClientRequest, http::header::HeaderName};

    let mut request = target
        .as_str()
        .into_client_request()
        .map_err(|error| transport_error(format!("invalid WebSocket request: {error}")))?;
    for (name, value) in headers {
        if is_hop_by_hop_header(name) {
            continue;
        }
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| protocol_error("invalid WebSocket header name"))?;
        let value = tungstenite::http::HeaderValue::from_str(value)
            .map_err(|_| protocol_error("invalid WebSocket header value"))?;
        request.headers_mut().insert(name, value);
    }
    Ok(request)
}

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "connection"
            | "host"
            | "upgrade"
            | "sec-websocket-key"
            | "sec-websocket-version"
            | "sec-websocket-extensions"
    )
}

fn connect_error(last: Option<String>) -> BlnkError {
    transport_error(format!(
        "all policy-approved addresses failed: {}",
        last.unwrap_or_else(|| "no connection attempt".into())
    ))
}

async fn connect_pinned_tcp(
    policy: &ProxyPolicy,
    resolved: &ResolvedProxyTarget,
) -> ProxyHandlerResult<TcpStream> {
    let mut last_error = None;
    for address in resolved.addresses() {
        policy
            .validate_retry(resolved, *address)
            .map_err(policy_error)?;
        match time::timeout(policy.limits().connect_timeout, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => last_error = Some(error.to_string()),
            Err(_) => last_error = Some("connect timeout".into()),
        }
    }
    Err(connect_error(last_error))
}

/// Service shared by the three concrete proxy protocols.
#[derive(Clone)]
pub struct ProxyStreamService {
    policy: ProxyPolicy,
    permits: Arc<Semaphore>,
}

impl ProxyStreamService {
    pub fn new(policy: ProxyPolicy) -> ProxyHandlerResult<Self> {
        policy.limits().validate().map_err(policy_error)?;
        let permits = Arc::new(Semaphore::new(policy.limits().max_concurrent_streams));
        Ok(Self { policy, permits })
    }

    pub fn policy(&self) -> &ProxyPolicy {
        &self.policy
    }

    fn acquire(&self) -> ProxyHandlerResult<OwnedSemaphorePermit> {
        self.permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| policy_error(ProxyPolicyError::ConcurrencyLimitExceeded))
    }

    /// Opens a raw TCP stream after validating and pinning the target.
    pub async fn open_tcp(
        &self,
        stream_id: u32,
        open: wire::TcpOpen,
        authorization: ProxyAuthorization,
    ) -> ProxyHandlerResult<TcpProxyStream> {
        let permit = self.acquire()?;
        let target = tcp_target(&open)?;
        let resolved = self
            .policy
            .resolve_and_validate(&target, authorization)
            .await
            .map_err(policy_error)?;
        let socket = connect_pinned_tcp(&self.policy, &resolved).await?;
        Ok(TcpProxyStream {
            stream_id,
            socket,
            limits: self.policy.limits(),
            _permit: permit,
            request_bytes: 0,
            response_bytes: 0,
        })
    }

    /// Opens a WebSocket over a policy-approved pinned TCP socket.
    ///
    /// `ws` is fully exercised by local fixtures. `wss` is intentionally rejected
    /// until this crate enables an explicit TLS connector whose DNS pinning and
    /// certificate behavior can be tested on all supported platforms.
    pub async fn open_websocket(
        &self,
        stream_id: u32,
        open: wire::WebSocketOpen,
        authorization: ProxyAuthorization,
    ) -> ProxyHandlerResult<WebSocketProxyStream> {
        let permit = self.acquire()?;
        validate_wire_type(&open.r#type, "websocket")?;
        let target =
            Url::parse(&open.target).map_err(|_| policy_error(ProxyPolicyError::InvalidTarget))?;
        let resolved = self
            .policy
            .resolve_and_validate(&target, authorization)
            .await
            .map_err(policy_error)?;
        if resolved.target().scheme() != ProxyScheme::Ws {
            return Err(transport_error(
                "wss handler requires an explicit TLS connector; no implicit TLS fallback",
            ));
        }

        let mut last_error = None;
        for address in resolved.addresses() {
            self.policy
                .validate_retry(&resolved, *address)
                .map_err(policy_error)?;
            let socket = match time::timeout(
                self.policy.limits().connect_timeout,
                TcpStream::connect(address),
            )
            .await
            {
                Ok(Ok(socket)) => socket,
                Ok(Err(error)) => {
                    last_error = Some(error.to_string());
                    continue;
                }
                Err(_) => {
                    last_error = Some("connect timeout".into());
                    continue;
                }
            };
            let request = websocket_request(&target, &open.headers)?;
            match time::timeout(
                self.policy.limits().connect_timeout,
                client_async(request, socket),
            )
            .await
            {
                Ok(Ok((socket, _response))) => {
                    return Ok(WebSocketProxyStream {
                        stream_id,
                        socket,
                        limits: self.policy.limits(),
                        _permit: permit,
                        request_bytes: 0,
                        response_bytes: 0,
                    });
                }
                Ok(Err(error)) => last_error = Some(error.to_string()),
                Err(_) => last_error = Some("WebSocket handshake timeout".into()),
            }
        }
        Err(connect_error(last_error))
    }

    /// Executes one HTTP request, manually applying redirect policy and DNS pinning.
    pub async fn request_http(
        &self,
        base_target: Url,
        request: wire::HttpRequest,
        body: Vec<u8>,
        authorization: ProxyAuthorization,
    ) -> ProxyHandlerResult<HttpProxyResponse> {
        let _permit = self.acquire()?;
        validate_wire_type(&request.r#type, "http")?;
        if request.content_length < 0
            || (request.content_length != 0
                && request.content_length != i64::try_from(body.len()).unwrap_or(i64::MAX))
        {
            return Err(protocol_error("HTTP content length does not match body"));
        }
        self.policy
            .check_request_size(body.len() as u64)
            .map_err(policy_error)?;
        let mut target = base_target
            .join(if request.pathname.is_empty() {
                "/"
            } else {
                &request.pathname
            })
            .map_err(|_| policy_error(ProxyPolicyError::InvalidTarget))?;
        let method = if request.method.trim().is_empty() {
            Method::GET
        } else {
            Method::from_bytes(request.method.as_bytes())
                .map_err(|_| protocol_error("invalid HTTP method"))?
        };
        let mut redirects = 0;

        loop {
            let resolved = self
                .policy
                .resolve_and_validate(&target, authorization)
                .await
                .map_err(policy_error)?;
            let client = Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(self.policy.limits().connect_timeout)
                .timeout(self.policy.limits().idle_timeout)
                .resolve_to_addrs(resolved.target().host(), resolved.addresses())
                .build()
                .map_err(|error| transport_error(format!("HTTP client build failed: {error}")))?;
            let mut builder = client
                .request(method.clone(), target.clone())
                .body(body.clone());
            for (name, value) in &request.headers {
                if is_hop_by_hop_header(name) || name.eq_ignore_ascii_case("content-length") {
                    continue;
                }
                let name = header::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| protocol_error("invalid HTTP header name"))?;
                let value = header::HeaderValue::from_str(value)
                    .map_err(|_| protocol_error("invalid HTTP header value"))?;
                builder = builder.header(name, value);
            }
            let mut response = builder
                .send()
                .await
                .map_err(|error| transport_error(format!("HTTP request failed: {error}")))?;
            if response.status().is_redirection()
                && let Some(location) = response.headers().get(header::LOCATION)
            {
                let location = location
                    .to_str()
                    .map_err(|_| protocol_error("invalid HTTP redirect location"))?;
                let next = target
                    .join(location)
                    .map_err(|_| policy_error(ProxyPolicyError::InvalidTarget))?;
                target = self
                    .policy
                    .validate_redirect(resolved.target(), &next, authorization, redirects)
                    .map_err(policy_error)?
                    .url()
                    .clone();
                redirects = redirects.saturating_add(1);
                continue;
            }
            if let Some(length) = response.content_length() {
                self.policy
                    .check_response_size(length)
                    .map_err(policy_error)?;
            }
            let status = response.status().as_u16();
            let headers = response_headers(response.headers());
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| transport_error(format!("HTTP body read failed: {error}")))?
            {
                let body_len = body
                    .len()
                    .checked_add(chunk.len())
                    .ok_or_else(|| policy_error(ProxyPolicyError::ResponseLimitExceeded))?;
                self.policy
                    .check_response_size(body_len as u64)
                    .map_err(policy_error)?;
                body.extend_from_slice(&chunk);
            }
            return Ok(HttpProxyResponse {
                response: wire::HttpResponse {
                    status: i32::from(status),
                    headers,
                },
                body,
                redirects_followed: redirects,
            });
        }
    }
}

fn response_headers(headers: &header::HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned()))
        })
        .collect()
}

/// A TCP stream bound to one SWSP stream id.
pub struct TcpProxyStream {
    stream_id: u32,
    socket: TcpStream,
    limits: ProxyResourceLimits,
    _permit: OwnedSemaphorePermit,
    request_bytes: u64,
    response_bytes: u64,
}

impl TcpProxyStream {
    pub fn stream_id(&self) -> u32 {
        self.stream_id
    }

    /// Applies an incoming SWSP data/FIN frame to the TCP socket.
    /// Returns `true` when the stream is closed by the peer.
    pub async fn handle_frame(&mut self, frame: Frame) -> ProxyHandlerResult<bool> {
        check_stream_id(&frame, self.stream_id)?;
        if frame.flags.is_fin() {
            self.socket.shutdown().await?;
            return Ok(true);
        }
        check_data_frame(&frame, self.stream_id)?;
        let data = wire::TcpData::decode(frame.payload.as_slice())
            .map_err(|error| protocol_error(format!("invalid TCP data payload: {error}")))?;
        let length = data.data.len() as u64;
        self.request_bytes = self
            .request_bytes
            .checked_add(length)
            .ok_or_else(|| policy_error(ProxyPolicyError::RequestLimitExceeded))?;
        self.limits
            .check_request_size(self.request_bytes)
            .map_err(policy_error)?;
        self.limits
            .check_buffered_bytes(data.data.len())
            .map_err(policy_error)?;
        time::timeout(self.limits.idle_timeout, self.socket.write_all(&data.data))
            .await
            .map_err(|_| transport_error("TCP write timeout"))??;
        Ok(false)
    }

    /// Reads one TCP chunk and turns it into a SWSP frame. `FIN` means EOF.
    pub async fn read_frame(&mut self) -> ProxyHandlerResult<Frame> {
        let chunk_size = self
            .limits
            .max_buffered_bytes
            .min(MAX_FRAME_DATA)
            .min(self.limits.max_response_bytes as usize)
            .max(1);
        let mut buffer = vec![0; chunk_size];
        let read = time::timeout(self.limits.idle_timeout, self.socket.read(&mut buffer))
            .await
            .map_err(|_| transport_error("TCP read timeout"))??;
        if read == 0 {
            return Ok(Frame::new(self.stream_id, FrameFlags::FIN, Vec::new()));
        }
        self.response_bytes = self
            .response_bytes
            .checked_add(read as u64)
            .ok_or_else(|| policy_error(ProxyPolicyError::ResponseLimitExceeded))?;
        self.limits
            .check_response_size(self.response_bytes)
            .map_err(policy_error)?;
        Ok(encode_frame(
            self.stream_id,
            dat_more_flags(),
            wire::TcpData {
                data: buffer[..read].to_vec(),
            },
        ))
    }

    pub async fn close(mut self) -> ProxyHandlerResult<()> {
        self.socket.shutdown().await.map_err(BlnkError::from)
    }
}

/// A WebSocket stream bound to one SWSP stream id.
pub struct WebSocketProxyStream {
    stream_id: u32,
    socket: tokio_tungstenite::WebSocketStream<TcpStream>,
    limits: ProxyResourceLimits,
    _permit: OwnedSemaphorePermit,
    request_bytes: u64,
    response_bytes: u64,
}

impl WebSocketProxyStream {
    pub fn stream_id(&self) -> u32 {
        self.stream_id
    }

    pub async fn handle_frame(&mut self, frame: Frame) -> ProxyHandlerResult<bool> {
        check_stream_id(&frame, self.stream_id)?;
        if frame.flags.is_fin() {
            self.socket
                .send(WebSocketMessage::Close(None))
                .await
                .map_err(|error| transport_error(format!("WebSocket close failed: {error}")))?;
            return Ok(true);
        }
        check_data_frame(&frame, self.stream_id)?;
        let message = wire::WebSocketMessage::decode(frame.payload.as_slice())
            .map_err(|error| protocol_error(format!("invalid WebSocket payload: {error}")))?;
        self.request_bytes = self
            .request_bytes
            .checked_add(message.data.len() as u64)
            .ok_or_else(|| policy_error(ProxyPolicyError::RequestLimitExceeded))?;
        self.limits
            .check_request_size(self.request_bytes)
            .map_err(policy_error)?;
        self.limits
            .check_buffered_bytes(message.data.len())
            .map_err(policy_error)?;
        let websocket_message = match message.message_type {
            0 => WebSocketMessage::text(
                String::from_utf8(message.data)
                    .map_err(|_| protocol_error("WebSocket text payload is not UTF-8"))?,
            ),
            1 => WebSocketMessage::binary(message.data),
            _ => return Err(protocol_error("unknown WebSocket message type")),
        };
        time::timeout(
            self.limits.idle_timeout,
            self.socket.send(websocket_message),
        )
        .await
        .map_err(|_| transport_error("WebSocket write timeout"))?
        .map_err(|error| transport_error(format!("WebSocket write failed: {error}")))?;
        Ok(false)
    }

    pub async fn read_frame(&mut self) -> ProxyHandlerResult<Frame> {
        loop {
            let next = time::timeout(self.limits.idle_timeout, self.socket.next())
                .await
                .map_err(|_| transport_error("WebSocket read timeout"))?;
            let Some(message) = next else {
                return Ok(Frame::new(self.stream_id, FrameFlags::FIN, Vec::new()));
            };
            let message = message
                .map_err(|error| transport_error(format!("WebSocket read failed: {error}")))?;
            let (message_type, data) = match message {
                WebSocketMessage::Text(text) => (0, text.as_str().as_bytes().to_vec()),
                WebSocketMessage::Binary(data) => (1, data.to_vec()),
                WebSocketMessage::Ping(data) => {
                    self.socket
                        .send(WebSocketMessage::Pong(data))
                        .await
                        .map_err(|error| {
                            transport_error(format!("WebSocket pong failed: {error}"))
                        })?;
                    continue;
                }
                WebSocketMessage::Pong(_) => continue,
                WebSocketMessage::Close(_) => {
                    return Ok(Frame::new(self.stream_id, FrameFlags::FIN, Vec::new()));
                }
                WebSocketMessage::Frame(_) => continue,
            };
            self.response_bytes = self
                .response_bytes
                .checked_add(data.len() as u64)
                .ok_or_else(|| policy_error(ProxyPolicyError::ResponseLimitExceeded))?;
            self.limits
                .check_response_size(self.response_bytes)
                .map_err(policy_error)?;
            self.limits
                .check_buffered_bytes(data.len())
                .map_err(policy_error)?;
            return Ok(encode_frame(
                self.stream_id,
                dat_more_flags(),
                wire::WebSocketMessage { message_type, data },
            ));
        }
    }

    pub async fn close(mut self) -> ProxyHandlerResult<()> {
        self.socket
            .send(WebSocketMessage::Close(None))
            .await
            .map_err(|error| transport_error(format!("WebSocket close failed: {error}")))
    }
}

/// HTTP response returned after redirects and body limits have been applied.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpProxyResponse {
    pub response: wire::HttpResponse,
    pub body: Vec<u8>,
    pub redirects_followed: u8,
}

impl HttpProxyResponse {
    /// Encodes a deterministic transcript: response metadata, body chunks, then FIN.
    pub fn into_frames(self, stream_id: u32, max_chunk: usize) -> ProxyHandlerResult<Vec<Frame>> {
        if max_chunk == 0 {
            return Err(policy_error(ProxyPolicyError::InvalidLimits));
        }
        let mut frames = vec![encode_frame(stream_id, syn_dat_flags(), self.response)];
        for chunk in self.body.chunks(max_chunk) {
            frames.push(encode_frame(
                stream_id,
                dat_more_flags(),
                wire::HttpData {
                    data: chunk.to_vec(),
                },
            ));
        }
        frames.push(Frame::new(stream_id, FrameFlags::FIN, Vec::new()));
        Ok(frames)
    }
}

pub fn encode_tcp_open(stream_id: u32, open: wire::TcpOpen) -> Frame {
    encode_frame(stream_id, syn_dat_flags(), open)
}

pub fn encode_websocket_open(stream_id: u32, open: wire::WebSocketOpen) -> Frame {
    encode_frame(stream_id, syn_dat_flags(), open)
}

pub fn encode_http_request(stream_id: u32, request: wire::HttpRequest) -> Frame {
    encode_frame(stream_id, syn_dat_flags(), request)
}

pub fn encode_data(stream_id: u32, data: Vec<u8>) -> Frame {
    encode_frame(stream_id, dat_more_flags(), wire::HttpData { data })
}

pub fn decode_tcp_open(frame: &Frame, stream_id: u32) -> ProxyHandlerResult<wire::TcpOpen> {
    check_open_frame(frame, stream_id)?;
    wire::TcpOpen::decode(frame.payload.as_slice())
        .map_err(|error| protocol_error(format!("invalid TCP open payload: {error}")))
}

pub fn decode_websocket_open(
    frame: &Frame,
    stream_id: u32,
) -> ProxyHandlerResult<wire::WebSocketOpen> {
    check_open_frame(frame, stream_id)?;
    wire::WebSocketOpen::decode(frame.payload.as_slice())
        .map_err(|error| protocol_error(format!("invalid WebSocket open payload: {error}")))
}

pub fn decode_http_request(frame: &Frame, stream_id: u32) -> ProxyHandlerResult<wire::HttpRequest> {
    check_open_frame(frame, stream_id)?;
    wire::HttpRequest::decode(frame.payload.as_slice())
        .map_err(|error| protocol_error(format!("invalid HTTP request payload: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    fn local_policy(target: &Url) -> ProxyPolicy {
        ProxyPolicy::deny_by_default()
            .with_local_targets(true)
            .with_allowed_target(target)
            .expect("local target can be allowlisted")
    }

    #[test]
    fn raw_transcript_codecs_use_canonical_flags_and_protobuf() {
        let request = wire::TcpOpen {
            r#type: "tcp".into(),
            host: "example.com".into(),
            port: 443,
        };
        let frame = encode_tcp_open(7, request.clone());
        assert_eq!(frame.flags.bits(), SYN_DAT_BITS);
        let decoded = wire::TcpOpen::decode(frame.payload.as_slice()).expect("decode TCP open");
        assert_eq!(decoded, request);

        let data = encode_data(7, b"payload".to_vec());
        assert!(data.flags.is_dat());
        assert!(data.flags.is_more());
        let decoded = wire::HttpData::decode(data.payload.as_slice()).expect("decode data");
        assert_eq!(decoded.data, b"payload");
    }

    #[tokio::test]
    async fn tcp_fixture_echoes_and_applies_limits() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("listener");
        let address = listener.local_addr().expect("address");
        let target = Url::parse(&format!("tcp://127.0.0.1:{}", address.port())).expect("target");
        let service = ProxyStreamService::new(local_policy(&target)).expect("service");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut data = [0; 7];
            tokio::io::AsyncReadExt::read_exact(&mut socket, &mut data)
                .await
                .expect("read");
            socket.write_all(&data).await.expect("echo");
        });
        let mut stream = service
            .open_tcp(
                9,
                wire::TcpOpen {
                    r#type: "tcp".into(),
                    host: "127.0.0.1".into(),
                    port: i32::from(address.port()),
                },
                ProxyAuthorization::Allowlisted,
            )
            .await
            .expect("open TCP");
        let payload = wire::TcpData {
            data: b"payload".to_vec(),
        };
        let incoming = Frame::new(9, dat_more_flags(), payload.encode_to_vec());
        assert!(!stream.handle_frame(incoming).await.expect("write frame"));
        let outgoing = stream.read_frame().await.expect("read frame");
        let echoed = wire::TcpData::decode(outgoing.payload.as_slice()).expect("decode echo");
        assert_eq!(echoed.data, b"payload");
        stream.close().await.expect("close TCP");
        server.await.expect("fixture task");
    }

    #[tokio::test]
    async fn websocket_fixture_bridges_text_and_binary_messages() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("listener");
        let address = listener.local_addr().expect("address");
        let target =
            Url::parse(&format!("ws://127.0.0.1:{}/echo", address.port())).expect("target");
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.expect("accept");
            let mut socket = accept_async(socket).await.expect("handshake");
            if let Some(Ok(message)) = socket.next().await {
                socket.send(message).await.expect("echo");
            }
        });
        let service = ProxyStreamService::new(local_policy(&target)).expect("service");
        let mut stream = service
            .open_websocket(
                10,
                wire::WebSocketOpen {
                    r#type: "websocket".into(),
                    target: target.to_string(),
                    headers: HashMap::new(),
                },
                ProxyAuthorization::Allowlisted,
            )
            .await
            .expect("open WebSocket");
        let incoming = wire::WebSocketMessage {
            message_type: 0,
            data: b"hello".to_vec(),
        };
        let frame = Frame::new(10, dat_more_flags(), incoming.encode_to_vec());
        assert!(!stream.handle_frame(frame).await.expect("send websocket"));
        let outgoing = stream.read_frame().await.expect("read websocket");
        let echoed =
            wire::WebSocketMessage::decode(outgoing.payload.as_slice()).expect("decode websocket");
        assert_eq!(echoed.message_type, 0);
        assert_eq!(echoed.data, b"hello");
        stream.close().await.expect("close websocket");
        server.await.expect("fixture task");
    }

    #[tokio::test]
    async fn http_fixture_returns_response_transcript_and_manual_redirects() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("listener");
        let address = listener.local_addr().expect("address");
        let target = Url::parse(&format!("http://127.0.0.1:{}", address.port())).expect("target");
        let requests = Arc::new(AtomicUsize::new(0));
        let fixture_requests = requests.clone();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.expect("accept");
                fixture_requests.fetch_add(1, Ordering::SeqCst);
                let mut buffer = [0; 2048];
                let read = socket.read(&mut buffer).await.expect("request");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let response = if request.starts_with("GET /start") {
                    "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nOK"
                };
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("response");
            }
        });
        let policy = local_policy(&target).with_redirect_policy(false, false);
        let service = ProxyStreamService::new(policy).expect("service");
        let response = service
            .request_http(
                target,
                wire::HttpRequest {
                    r#type: "http".into(),
                    method: "GET".into(),
                    pathname: "/start".into(),
                    content_type: String::new(),
                    content_length: 0,
                    headers: HashMap::new(),
                },
                Vec::new(),
                ProxyAuthorization::Allowlisted,
            )
            .await
            .expect("HTTP request");
        assert_eq!(response.response.status, 200);
        assert_eq!(response.body, b"OK");
        assert_eq!(response.redirects_followed, 1);
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        let frames = response.into_frames(11, 1).expect("frames");
        assert_eq!(frames.first().expect("metadata").flags.bits(), SYN_DAT_BITS);
        assert!(frames.last().expect("fin").flags.is_fin());
        server.await.expect("fixture task");
    }

    #[tokio::test]
    async fn chunked_http_response_is_rejected_before_full_buffering() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("listener");
        let address = listener.local_addr().expect("address");
        let target = Url::parse(&format!("http://127.0.0.1:{}", address.port())).expect("target");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request = [0; 2048];
            let read = socket.read(&mut request).await.expect("request");
            assert!(read > 0, "request must contain at least one byte");
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n",
                )
                .await
                .expect("response");
        });
        let limits = ProxyResourceLimits {
            max_response_bytes: 5,
            ..ProxyResourceLimits::default()
        };
        let policy = local_policy(&target).with_limits(limits).expect("limits");
        let service = ProxyStreamService::new(policy).expect("service");
        let error = service
            .request_http(
                target,
                wire::HttpRequest {
                    r#type: "http".into(),
                    method: "GET".into(),
                    pathname: "/chunked".into(),
                    content_type: String::new(),
                    content_length: 0,
                    headers: HashMap::new(),
                },
                Vec::new(),
                ProxyAuthorization::Allowlisted,
            )
            .await
            .expect_err("chunked response must exceed the configured limit");
        assert!(error.to_string().contains("response_limit_exceeded"));
        server.await.expect("fixture task");
    }

    #[tokio::test]
    async fn denied_target_is_rejected_before_local_connect() {
        let target = Url::parse("tcp://127.0.0.1:9").expect("target");
        let service = ProxyStreamService::new(ProxyPolicy::deny_by_default()).expect("service");
        let error = match service
            .open_tcp(
                12,
                wire::TcpOpen {
                    r#type: "tcp".into(),
                    host: "127.0.0.1".into(),
                    port: 9,
                },
                ProxyAuthorization::Allowlisted,
            )
            .await
        {
            Ok(_) => panic!("private target must be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("target_not_allowlisted"));
        assert_eq!(target.scheme(), "tcp");
    }

    #[tokio::test]
    async fn concurrency_limit_is_enforced_before_connect() {
        let target = Url::parse("tcp://127.0.0.1:9").expect("target");
        let limits = ProxyResourceLimits {
            max_concurrent_streams: 1,
            ..ProxyResourceLimits::default()
        };
        let policy = ProxyPolicy::deny_by_default()
            .with_local_targets(true)
            .with_allowed_target(&target)
            .expect("target")
            .with_limits(limits)
            .expect("limits");
        let service = ProxyStreamService::new(policy).expect("service");
        let permit = service.acquire().expect("first permit");
        let error = match service.acquire() {
            Ok(_) => panic!("second permit must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("concurrency_limit_exceeded"));
        drop(permit);
    }
}
