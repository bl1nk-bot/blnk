//! Loopback-only browser control surface.
//!
//! This module deliberately exposes session lifecycle/status only. It does not
//! claim browser/WebRTC interoperability and does not forward arbitrary shell,
//! file, proxy, or signaling operations from an unauthenticated HTTP request.

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Json, Path, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CACHE_CONTROL, CONTENT_LENGTH, COOKIE, ORIGIN,
    SET_COOKIE, VARY, WWW_AUTHENTICATE, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Router, serve};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::utils::error::BlnkError;

const SESSION_COOKIE_NAME: &str = "blnk_session";
const MAX_LABEL_BYTES: usize = 64;
const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(900);
const DEFAULT_RATE_WINDOW: Duration = Duration::from_secs(60);
const DEFAULT_MAX_SESSION_CREATIONS: usize = 8;
const DEFAULT_MAX_BODY_BYTES: usize = 16 * 1024;
const DEFAULT_MAX_FRAME_BYTES: usize = 64 * 1024;
const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuration for the local browser surface.
#[derive(Clone, Debug)]
pub struct BrowserControlConfig {
    pub bind_addr: SocketAddr,
    pub allowed_origin: String,
    pub bootstrap_token: String,
    pub session_ttl: Duration,
    pub max_sessions: usize,
    pub max_session_creations: usize,
    pub rate_window: Duration,
    pub max_body_bytes: usize,
    pub max_frame_bytes: usize,
    pub handshake_timeout: Duration,
}

impl BrowserControlConfig {
    /// Creates a conservative loopback configuration for a local browser client.
    pub fn loopback(allowed_origin: impl Into<String>, bootstrap_token: impl Into<String>) -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            allowed_origin: allowed_origin.into(),
            bootstrap_token: bootstrap_token.into(),
            session_ttl: DEFAULT_SESSION_TTL,
            max_sessions: 8,
            max_session_creations: DEFAULT_MAX_SESSION_CREATIONS,
            rate_window: DEFAULT_RATE_WINDOW,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
        }
    }

    pub fn with_bind_addr(mut self, bind_addr: SocketAddr) -> Self {
        self.bind_addr = bind_addr;
        self
    }

    fn validate(&self) -> Result<(), BlnkError> {
        if !self.bind_addr.ip().is_loopback() {
            return Err(BlnkError::Config(
                "browser control server must bind to a loopback address".into(),
            ));
        }
        if self.bootstrap_token.trim().is_empty() {
            return Err(BlnkError::Config(
                "browser control server requires a non-empty bootstrap token".into(),
            ));
        }
        if self.allowed_origin == "*" {
            return Err(BlnkError::Config(
                "browser control server does not allow wildcard origins".into(),
            ));
        }
        let origin = Url::parse(&self.allowed_origin).map_err(|error| {
            BlnkError::Config(format!("invalid browser allowed origin: {error}"))
        })?;
        if !matches!(origin.scheme(), "http" | "https")
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.path() != "/"
        {
            return Err(BlnkError::Config(
                "browser allowed origin must be an exact http(s) origin without credentials, path, query, or fragment".into(),
            ));
        }
        if self.session_ttl.is_zero()
            || self.max_sessions == 0
            || self.max_session_creations == 0
            || self.rate_window.is_zero()
            || self.max_body_bytes == 0
            || self.max_frame_bytes == 0
            || self.handshake_timeout.is_zero()
        {
            return Err(BlnkError::Config(
                "browser control limits must be non-zero".into(),
            ));
        }
        HeaderValue::from_str(&self.allowed_origin).map_err(|_| {
            BlnkError::Config("browser allowed origin is not a valid HTTP header value".into())
        })?;
        Ok(())
    }
}

/// A running browser control server. Dropping it aborts the listener task;
/// callers should prefer [`BrowserControlServer::shutdown`] for graceful cleanup.
pub struct BrowserControlServer {
    address: SocketAddr,
    shutdown: CancellationToken,
    task: Option<JoinHandle<Result<(), std::io::Error>>>,
}

impl BrowserControlServer {
    pub async fn start(config: BrowserControlConfig) -> Result<Self, BlnkError> {
        config.validate()?;
        let listener = TcpListener::bind(config.bind_addr).await?;
        let address = listener.local_addr()?;
        let shutdown = CancellationToken::new();
        let state = Arc::new(AppState::new(config.clone()));
        let router = build_router(state);
        let task_shutdown = shutdown.clone();
        let task = tokio::spawn(async move {
            serve(listener, router)
                .with_graceful_shutdown(task_shutdown.cancelled_owned())
                .await
        });
        Ok(Self {
            address,
            shutdown,
            task: Some(task),
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    pub async fn shutdown(&mut self) -> Result<(), BlnkError> {
        self.shutdown.cancel();
        let Some(task) = self.task.take() else {
            return Ok(());
        };
        match task.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(BlnkError::Io(error)),
            Err(error) => Err(BlnkError::Io(std::io::Error::other(format!(
                "browser control task join: {error}"
            )))),
        }
    }
}

impl Drop for BrowserControlServer {
    fn drop(&mut self) {
        self.shutdown.cancel();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

#[derive(Clone)]
struct AppState {
    config: BrowserControlConfig,
    sessions: Arc<Mutex<SessionStore>>,
}

impl AppState {
    fn new(config: BrowserControlConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(Mutex::new(SessionStore::default())),
        }
    }
}

#[derive(Default)]
struct SessionStore {
    sessions: HashMap<String, SessionRecord>,
    creation_times: VecDeque<Instant>,
}

#[derive(Clone)]
struct SessionRecord {
    id: String,
    csrf_token: String,
    label: Option<String>,
    expires_at: Instant,
    state: SessionState,
}

#[derive(Clone, Copy)]
enum SessionState {
    Active,
    Closed,
}

impl SessionState {
    fn as_str(self, expires_at: Instant, now: Instant) -> &'static str {
        match self {
            Self::Active if expires_at <= now => "expired",
            Self::Active => "active",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSessionRequest {
    label: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateSessionResponse {
    session_id: String,
    csrf_token: String,
    state: &'static str,
    websocket_path: String,
    expires_in_secs: u64,
}

#[derive(Debug, Serialize)]
struct SessionResponse {
    session_id: String,
    state: &'static str,
    websocket_path: String,
    expires_in_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

#[derive(Debug, Serialize)]
struct CloseResponse {
    session_id: String,
    state: &'static str,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    browser_control: &'static str,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str) -> Self {
        Self { status, code }
    }

    fn bad_request() -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request")
    }

    fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }

    fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden")
    }

    fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found")
    }

    fn gone() -> Self {
        Self::new(StatusCode::GONE, "session_unavailable")
    }

    fn too_many_requests() -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited")
    }

    fn payload_too_large() -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large")
    }

    fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(ErrorResponse { error: self.code })).into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer realm=blnk-browser"),
            );
        }
        response
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum WsClientMessage {
    #[serde(rename = "hello")]
    Hello { csrf_token: String },
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "close")]
    Close,
}

#[derive(Debug, Serialize)]
struct WsServerMessage {
    #[serde(rename = "type")]
    message_type: &'static str,
    session_id: String,
    state: &'static str,
}

fn build_router(state: Arc<AppState>) -> Router {
    let max_body_bytes = state.config.max_body_bytes;
    Router::new()
        .route("/healthz", get(health))
        .route(
            "/api/session",
            post(create_session).on(axum::routing::MethodFilter::OPTIONS, preflight),
        )
        .route(
            "/api/session/{session_id}",
            get(get_session)
                .post(close_session)
                .on(axum::routing::MethodFilter::OPTIONS, preflight),
        )
        .route(
            "/api/session/{session_id}/ws",
            get(websocket).on(axum::routing::MethodFilter::OPTIONS, preflight),
        )
        .with_state(state.clone())
        .layer(axum::extract::DefaultBodyLimit::max(max_body_bytes))
        .layer(from_fn_with_state(state, security_headers))
}

async fn security_headers(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let request_origin = request
        .headers()
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let is_allowed_origin = request_origin.as_deref() == Some(state.config.allowed_origin.as_str());
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
        .headers_mut()
        .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    // Mitigate clickjacking by enforcing framing restrictions on control responses
    response
        .headers_mut()
        .insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
        .headers_mut()
        .insert(VARY, HeaderValue::from_static("Origin"));
    if is_allowed_origin {
        if let Ok(origin) = HeaderValue::from_str(&state.config.allowed_origin) {
            response
                .headers_mut()
                .insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        }
        response.headers_mut().insert(
            ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
        response.headers_mut().insert(
            ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("authorization, content-type, x-csrf-token"),
        );
        response.headers_mut().insert(
            ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, OPTIONS"),
        );
    }
    response
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        browser_control: "loopback-session-only",
    })
}

async fn preflight(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if !origin_allowed(&headers, &state.config) {
        return ApiError::forbidden().into_response();
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    add_cors_headers(response.headers_mut(), &state.config);
    response
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    payload: Result<Json<CreateSessionRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    if let Err(error) = require_origin(&headers, &state.config) {
        return error.into_response();
    }
    if !valid_bearer(&headers, &state.config.bootstrap_token) {
        return ApiError::unauthorized().into_response();
    }
    if headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > state.config.max_body_bytes)
    {
        return ApiError::payload_too_large().into_response();
    }
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return ApiError::payload_too_large().into_response();
        }
        Err(_) => return ApiError::bad_request().into_response(),
    };
    if payload
        .label
        .as_deref()
        .is_some_and(|label| label.len() > MAX_LABEL_BYTES)
    {
        return ApiError::payload_too_large().into_response();
    }

    let now = Instant::now();
    let mut store = state.sessions.lock().await;
    store.sessions.retain(|_, session| {
        session.expires_at > now || matches!(session.state, SessionState::Closed)
    });
    store
        .creation_times
        .retain(|created_at| now.duration_since(*created_at) < state.config.rate_window);
    if store.creation_times.len() >= state.config.max_session_creations {
        return ApiError::too_many_requests().into_response();
    }
    if store
        .sessions
        .values()
        .filter(|session| matches!(session.state, SessionState::Active) && session.expires_at > now)
        .count()
        >= state.config.max_sessions
    {
        return ApiError::too_many_requests().into_response();
    }

    let id = match random_token() {
        Ok(id) => id,
        Err(_) => return ApiError::internal().into_response(),
    };
    let csrf_token = match random_token() {
        Ok(token) => token,
        Err(_) => return ApiError::internal().into_response(),
    };
    let record = SessionRecord {
        id: id.clone(),
        csrf_token: csrf_token.clone(),
        label: payload.label,
        expires_at: now + state.config.session_ttl,
        state: SessionState::Active,
    };
    store.creation_times.push_back(now);
    store.sessions.insert(id.clone(), record);
    drop(store);

    let websocket_path = format!("/api/session/{id}/ws");
    let mut response = (
        StatusCode::CREATED,
        Json(CreateSessionResponse {
            session_id: id.clone(),
            csrf_token,
            state: "active",
            websocket_path,
            expires_in_secs: state.config.session_ttl.as_secs(),
        }),
    )
        .into_response();
    let cookie =
        format!("{SESSION_COOKIE_NAME}={id}; Path=/api/session; HttpOnly; SameSite=Strict");
    if let Ok(cookie) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(SET_COOKIE, cookie);
    }
    response
}

async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = require_origin(&headers, &state.config) {
        return error.into_response();
    }
    let session = match authenticated_session(&state, &session_id, &headers).await {
        Ok(session) => session,
        Err(error) => return error.into_response(),
    };
    (
        StatusCode::OK,
        Json(session_response(&session, Instant::now())),
    )
        .into_response()
}

async fn close_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = require_origin(&headers, &state.config) {
        return error.into_response();
    }
    let session = match authenticated_session(&state, &session_id, &headers).await {
        Ok(session) => session,
        Err(error) => return error.into_response(),
    };
    if !csrf_allowed(&headers, &session.csrf_token) {
        return ApiError::forbidden().into_response();
    }
    let mut store = state.sessions.lock().await;
    let Some(session) = store.sessions.get_mut(&session_id) else {
        return ApiError::not_found().into_response();
    };
    session.state = SessionState::Closed;
    (
        StatusCode::OK,
        Json(CloseResponse {
            session_id,
            state: "closed",
        }),
    )
        .into_response()
}

async fn websocket(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if let Err(error) = require_origin(&headers, &state.config) {
        return error.into_response();
    }
    let session = match authenticated_session(&state, &session_id, &headers).await {
        Ok(session) => session,
        Err(error) => return error.into_response(),
    };
    if !matches!(session.state, SessionState::Active) || session.expires_at <= Instant::now() {
        return ApiError::gone().into_response();
    }
    let frame_limit = state.config.max_frame_bytes;
    let write_buffer = frame_limit.clamp(1, 1024);
    let write_limit = frame_limit
        .saturating_mul(4)
        .max(write_buffer.saturating_add(1));
    let handshake_timeout = state.config.handshake_timeout;
    ws.write_buffer_size(write_buffer)
        .max_message_size(frame_limit)
        .max_frame_size(frame_limit)
        .max_write_buffer_size(write_limit)
        .on_upgrade(move |socket| {
            run_websocket(
                socket,
                state,
                session_id,
                session.csrf_token,
                handshake_timeout,
            )
        })
}

async fn run_websocket(
    mut socket: WebSocket,
    state: Arc<AppState>,
    session_id: String,
    expected_csrf: String,
    handshake_timeout: Duration,
) {
    let first_message = timeout(handshake_timeout, socket.next()).await;
    let authenticated = match first_message {
        Ok(Some(Ok(Message::Text(text)))) => {
            match serde_json::from_str::<WsClientMessage>(text.as_ref()) {
                Ok(WsClientMessage::Hello { csrf_token }) => secure_eq(&csrf_token, &expected_csrf),
                _ => false,
            }
        }
        _ => false,
    };
    if !authenticated {
        let _ = socket.send(Message::Close(None)).await;
        close_session_record(&state, &session_id).await;
        return;
    }
    if !send_ws_json(
        &mut socket,
        WsServerMessage {
            message_type: "ready",
            session_id: session_id.clone(),
            state: "active",
        },
    )
    .await
    {
        close_session_record(&state, &session_id).await;
        return;
    }

    while let Some(result) = socket.next().await {
        let Ok(message) = result else { break };
        match message {
            Message::Text(text) => {
                let Ok(command) = serde_json::from_str::<WsClientMessage>(text.as_ref()) else {
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                };
                match command {
                    WsClientMessage::Status => {
                        let status = session_status(&state, &session_id).await;
                        if !send_ws_json(
                            &mut socket,
                            WsServerMessage {
                                message_type: "status",
                                session_id: session_id.clone(),
                                state: status,
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                    WsClientMessage::Close => {
                        close_session_record(&state, &session_id).await;
                        let _ = send_ws_json(
                            &mut socket,
                            WsServerMessage {
                                message_type: "closed",
                                session_id: session_id.clone(),
                                state: "closed",
                            },
                        )
                        .await;
                        let _ = socket.send(Message::Close(None)).await;
                        return;
                    }
                    WsClientMessage::Hello { .. } => {
                        let _ = socket.send(Message::Close(None)).await;
                        break;
                    }
                }
            }
            Message::Close(_) => break,
            Message::Ping(payload) => {
                if socket.send(Message::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Message::Pong(_) => {}
            Message::Binary(_) => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
        }
    }
    close_session_record(&state, &session_id).await;
}

async fn send_ws_json(socket: &mut WebSocket, message: WsServerMessage) -> bool {
    match serde_json::to_string(&message) {
        Ok(payload) => socket.send(Message::Text(payload.into())).await.is_ok(),
        Err(_) => false,
    }
}

async fn authenticated_session(
    state: &AppState,
    session_id: &str,
    headers: &HeaderMap,
) -> Result<SessionRecord, ApiError> {
    let cookie = session_cookie(headers).ok_or_else(ApiError::unauthorized)?;
    if !secure_eq(cookie, session_id) {
        return Err(ApiError::unauthorized());
    }
    let store = state.sessions.lock().await;
    let session = store
        .sessions
        .get(session_id)
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    if !secure_eq(&session.id, session_id) {
        return Err(ApiError::unauthorized());
    }
    Ok(session)
}

async fn close_session_record(state: &AppState, session_id: &str) {
    let mut store = state.sessions.lock().await;
    if let Some(session) = store.sessions.get_mut(session_id) {
        session.state = SessionState::Closed;
    }
}

async fn session_status(state: &AppState, session_id: &str) -> &'static str {
    let store = state.sessions.lock().await;
    store
        .sessions
        .get(session_id)
        .map(|session| session.state.as_str(session.expires_at, Instant::now()))
        .unwrap_or("expired")
}

fn session_response(session: &SessionRecord, now: Instant) -> SessionResponse {
    SessionResponse {
        session_id: session.id.clone(),
        state: session.state.as_str(session.expires_at, now),
        websocket_path: format!("/api/session/{}/ws", session.id),
        expires_in_secs: session.expires_at.saturating_duration_since(now).as_secs(),
        label: session.label.clone(),
    }
}

fn require_origin(headers: &HeaderMap, config: &BrowserControlConfig) -> Result<(), ApiError> {
    if origin_allowed(headers, config) {
        Ok(())
    } else {
        Err(ApiError::forbidden())
    }
}

fn origin_allowed(headers: &HeaderMap, config: &BrowserControlConfig) -> bool {
    headers
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|origin| origin == config.allowed_origin)
}

fn valid_bearer(headers: &HeaderMap, expected: &str) -> bool {
    let Some(value) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return false;
    };
    secure_eq(token, expected)
}

fn csrf_allowed(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|actual| secure_eq(actual, expected))
}

fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookie_header| {
            cookie_header.split(';').find_map(|part| {
                let (name, value) = part.trim().split_once('=')?;
                (name == SESSION_COOKIE_NAME).then_some(value)
            })
        })
}

fn add_cors_headers(headers: &mut HeaderMap, config: &BrowserControlConfig) {
    if let Ok(origin) = HeaderValue::from_str(&config.allowed_origin) {
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    headers.insert(
        ACCESS_CONTROL_ALLOW_CREDENTIALS,
        HeaderValue::from_static("true"),
    );
    headers.insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("authorization, content-type, x-csrf-token"),
    );
    headers.insert(
        ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
}

fn secure_eq(left: &str, right: &str) -> bool {
    let left_digest = Sha256::digest(left.as_bytes());
    let right_digest = Sha256::digest(right.as_bytes());
    left_digest.ct_eq(&right_digest).into()
}

fn random_token() -> Result<String, BlnkError> {
    let mut bytes = [0_u8; 24];
    getrandom::fill(&mut bytes)
        .map_err(|error| BlnkError::Io(std::io::Error::other(format!("random token: {error}"))))?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use reqwest::header::{HeaderValue as ReqwestHeaderValue, ORIGIN as REQWEST_ORIGIN};
    use reqwest::{Client, StatusCode as ReqwestStatusCode};
    use serde_json::Value;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    fn fixture_config() -> BrowserControlConfig {
        BrowserControlConfig {
            max_session_creations: 2,
            max_sessions: 4,
            rate_window: Duration::from_secs(60),
            max_body_bytes: 128,
            max_frame_bytes: 256,
            handshake_timeout: Duration::from_secs(2),
            ..BrowserControlConfig::loopback("http://127.0.0.1:3000", "fixture-bootstrap-token")
        }
    }

    async fn start_fixture() -> BrowserControlServer {
        BrowserControlServer::start(fixture_config())
            .await
            .expect("fixture server should start")
    }

    fn origin() -> ReqwestHeaderValue {
        ReqwestHeaderValue::from_static("http://127.0.0.1:3000")
    }

    fn cookie_from(response: &reqwest::Response) -> String {
        response
            .headers()
            .get(SET_COOKIE)
            .expect("session cookie")
            .to_str()
            .expect("cookie header")
            .split(';')
            .next()
            .expect("cookie pair")
            .to_owned()
    }

    async fn create(
        client: &Client,
        server: &BrowserControlServer,
        label: &str,
    ) -> (String, String, String) {
        let response = client
            .post(format!("{}/api/session", server.url()))
            .header(REQWEST_ORIGIN, origin())
            .bearer_auth("fixture-bootstrap-token")
            .json(&serde_json::json!({"label": label}))
            .send()
            .await
            .expect("create request");
        assert_eq!(response.status(), ReqwestStatusCode::CREATED);
        let cookie = cookie_from(&response);
        let body: Value = response.json().await.expect("create response json");
        (
            body["session_id"].as_str().expect("session id").to_owned(),
            body["csrf_token"].as_str().expect("csrf token").to_owned(),
            cookie,
        )
    }

    #[tokio::test]
    async fn browser_session_enforces_origin_auth_csrf_and_cookie_boundary() {
        let mut server = start_fixture().await;
        let client = Client::new();
        let no_origin = client
            .post(format!("{}/api/session", server.url()))
            .bearer_auth("fixture-bootstrap-token")
            .json(&serde_json::json!({"label": "no-origin"}))
            .send()
            .await
            .expect("no-origin request");
        assert_eq!(no_origin.status(), ReqwestStatusCode::FORBIDDEN);

        let bad_auth = client
            .post(format!("{}/api/session", server.url()))
            .header(REQWEST_ORIGIN, origin())
            .bearer_auth("wrong-token")
            .json(&serde_json::json!({"label": "bad-auth"}))
            .send()
            .await
            .expect("bad-auth request");
        assert_eq!(bad_auth.status(), ReqwestStatusCode::UNAUTHORIZED);
        assert!(
            !bad_auth
                .text()
                .await
                .expect("bad-auth body")
                .contains("wrong-token")
        );

        let (session_id, csrf, cookie) = create(&client, &server, "first").await;
        let without_cookie = client
            .get(format!("{}/api/session/{session_id}", server.url()))
            .header(REQWEST_ORIGIN, origin())
            .send()
            .await
            .expect("missing-cookie request");
        assert_eq!(without_cookie.status(), ReqwestStatusCode::UNAUTHORIZED);

        let status = client
            .get(format!("{}/api/session/{session_id}", server.url()))
            .header(REQWEST_ORIGIN, origin())
            .header(COOKIE, cookie.clone())
            .send()
            .await
            .expect("status request");
        assert_eq!(status.status(), ReqwestStatusCode::OK);
        let status_body: Value = status.json().await.expect("status json");
        assert_eq!(status_body["state"], "active");
        assert!(status_body.get("csrf_token").is_none());

        let bad_csrf = client
            .post(format!("{}/api/session/{session_id}", server.url()))
            .header(REQWEST_ORIGIN, origin())
            .header(COOKIE, cookie.clone())
            .header("x-csrf-token", "wrong-csrf")
            .send()
            .await
            .expect("bad-csrf request");
        assert_eq!(bad_csrf.status(), ReqwestStatusCode::FORBIDDEN);

        let closed = client
            .post(format!("{}/api/session/{session_id}", server.url()))
            .header(REQWEST_ORIGIN, origin())
            .header(COOKIE, cookie.clone())
            .header("x-csrf-token", csrf)
            .send()
            .await
            .expect("close request");
        assert_eq!(closed.status(), ReqwestStatusCode::OK);
        let closed_body: Value = closed.json().await.expect("close json");
        assert_eq!(closed_body["state"], "closed");

        let after_close = client
            .get(format!("{}/api/session/{session_id}", server.url()))
            .header(REQWEST_ORIGIN, origin())
            .header(COOKIE, cookie)
            .send()
            .await
            .expect("closed status request");
        assert_eq!(after_close.status(), ReqwestStatusCode::OK);
        assert_eq!(
            after_close.json::<Value>().await.expect("closed json")["state"],
            "closed"
        );
        server.shutdown().await.expect("fixture shutdown");
    }

    #[tokio::test]
    async fn browser_surface_enforces_cors_body_and_creation_rate_limits() {
        let mut server = start_fixture().await;
        let client = Client::new();
        let disallowed_origin = client
            .get(format!("{}/healthz", server.url()))
            .header(REQWEST_ORIGIN, "http://evil.example")
            .send()
            .await
            .expect("health request");
        assert_eq!(disallowed_origin.status(), ReqwestStatusCode::OK);
        assert_eq!(
            disallowed_origin
                .headers()
                .get("x-frame-options")
                .and_then(|val| val.to_str().ok()),
            Some("DENY")
        );
        assert!(
            disallowed_origin
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );

        let too_large = client
            .post(format!("{}/api/session", server.url()))
            .header(REQWEST_ORIGIN, origin())
            .bearer_auth("fixture-bootstrap-token")
            .body("x".repeat(512))
            .send()
            .await
            .expect("body-limit request");
        assert_eq!(too_large.status(), ReqwestStatusCode::PAYLOAD_TOO_LARGE);

        let _ = create(&client, &server, "one").await;
        let _ = create(&client, &server, "two").await;
        let rate_limited = client
            .post(format!("{}/api/session", server.url()))
            .header(REQWEST_ORIGIN, origin())
            .bearer_auth("fixture-bootstrap-token")
            .json(&serde_json::json!({"label": "three"}))
            .send()
            .await
            .expect("rate-limit request");
        assert_eq!(rate_limited.status(), ReqwestStatusCode::TOO_MANY_REQUESTS);
        server.shutdown().await.expect("fixture shutdown");
    }

    #[tokio::test]
    async fn websocket_requires_csrf_hello_and_supports_bounded_status_flow() {
        let mut server = start_fixture().await;
        let client = Client::new();
        let (session_id, csrf, cookie) = create(&client, &server, "websocket").await;
        let ws_url = format!("ws://{}/api/session/{session_id}/ws", server.address());
        let mut request = ws_url.into_client_request().expect("websocket request");
        request.headers_mut().insert(ORIGIN, origin());
        request.headers_mut().insert(
            COOKIE,
            HeaderValue::from_str(&cookie).expect("websocket cookie"),
        );
        let (mut socket, _) = tokio_tungstenite::connect_async(request)
            .await
            .expect("websocket connect");
        socket
            .send(tungstenite::Message::Text(
                serde_json::json!({"type": "hello", "csrf_token": csrf})
                    .to_string()
                    .into(),
            ))
            .await
            .expect("hello send");
        let ready = socket
            .next()
            .await
            .expect("ready frame")
            .expect("ready message");
        let ready_text = ready.into_text().expect("ready text");
        assert!(ready_text.contains("\"type\":\"ready\""));

        socket
            .send(tungstenite::Message::Text(
                serde_json::json!({"type": "status"}).to_string().into(),
            ))
            .await
            .expect("status send");
        let status = socket
            .next()
            .await
            .expect("status frame")
            .expect("status message");
        assert!(
            status
                .into_text()
                .expect("status text")
                .contains("\"state\":\"active\"")
        );

        socket
            .send(tungstenite::Message::Text("x".repeat(512).into()))
            .await
            .expect("oversized frame send");
        let _ = socket.next().await;
        server.shutdown().await.expect("fixture shutdown");
    }
}
