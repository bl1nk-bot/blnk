//! Remote signaling and WebRTC orchestration.
//!
//! This module owns the protocol state machine between the WebSocket signaling
//! boundary and [`PeerHandle`]. The signaling server remains an external
//! dependency in production; the relay fixture in the tests is deliberately
//! local and deterministic and is not presented as provider interoperability.

use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use base64::Engine;
use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::{Choice, ConstantTimeEq};
use tokio_util::sync::CancellationToken;
use webrtc::peer_connection::RTCSessionDescription;

use crate::{
    identity::Identity,
    peer::PeerHandle,
    session::{SessionRuntime, SessionRuntimeConfig},
    signaling::{
        AnswerMessage, ConnectionRequest, EndpointPolicy, OfferMessage, RegisterRequest,
        SignalingMessage, SignalingResult, transport::ReconnectPolicy,
    },
    stream::{
        file::{
            FileOperation, FileTransferCancellation, FileTransferConfig, FileTransferRequest,
            FileTransferResponse, FileTransferService, collect_data_frames, decode_request_frame,
            decode_response_metadata, encode_response_frames,
        },
        shell::{
            ShellCommand, ShellPolicy, ShellStreamHandler, decode_error_frame, decode_exit_frame,
            decode_open_frame, decode_output_frame,
        },
    },
    utils::error::BlnkError,
};

/// Timeout used by the CLI when it does not receive the next signaling event.
pub const DEFAULT_ORCHESTRATION_TIMEOUT: Duration = Duration::from_secs(15);
/// Limited reconnect is used only while opening the signaling socket. A
/// partially-negotiated WebRTC session is never silently replayed.
pub const INITIAL_RECONNECT_RETRIES: usize = 1;

const DEVICE_PUBLIC_KEY_STREAM: &str = "device_public_key";
const REQUEST_NONCE_STREAM: &str = "request_nonce";
const PIN_LEN: usize = 6;

pub struct ServerSession {
    pub runtime: SessionRuntime,
    pub client_id: String,
}

pub struct ClientSession {
    pub runtime: SessionRuntime,
    pub client_id: String,
    pub target: String,
}

/// Registers a device and accepts one connection request from the signaling
/// server, then completes non-trickle SDP and the authenticated session.
pub async fn accept_server_session(
    endpoint: &str,
    identity: &Identity,
    pin: String,
    endpoint_policy: EndpointPolicy,
    timeout: Duration,
) -> SignalingResult<ServerSession> {
    validate_timeout(timeout)?;
    let client = make_client(endpoint, endpoint_policy)?;
    let mut signaling = client
        .connect_with_retry()
        .await
        .map_err(|error| orchestration_error("connect signaling socket", error))?;
    let public_key = identity.public_key_b64()?;
    let register =
        SignalingMessage::Register(RegisterRequest::new(identity.uid(), public_key, true)?);
    signaling.send(&register).await?;

    match recv_with_timeout(&mut signaling, timeout).await? {
        SignalingMessage::Registered(response) => {
            response.validate()?;
        }
        SignalingMessage::Error(error) => {
            return Err(BlnkError::Signaling(format!(
                "signaling rejected registration: {}",
                error.message
            )));
        }
        other => {
            return Err(unexpected_message("registered response", &other));
        }
    }

    let client_id = match recv_with_timeout(&mut signaling, timeout).await? {
        SignalingMessage::Request(request) => request.client_id,
        SignalingMessage::PairRequest(_) => {
            return Err(BlnkError::Signaling(
                "pair_request requires the separate pairing flow and cannot start an authenticated session"
                    .into(),
            ));
        }
        SignalingMessage::Error(error) => {
            return Err(BlnkError::Signaling(format!(
                "signaling rejected connection request: {}",
                error.message
            )));
        }
        other => return Err(unexpected_message("connection request", &other)),
    };
    require_non_empty(&client_id, "client_id")?;

    let mut challenge = RequestChallenge::new();
    let peer = PeerHandle::new().await?;
    peer.create_data_channel("control").await?;
    let offer = match peer.create_offer().await {
        Ok(offer) => offer,
        Err(error) => {
            let _ = peer.close().await;
            return Err(error);
        }
    };
    let mut offer_message = OfferMessage::new(client_id.clone(), offer.sdp.clone())?;
    let public_key_der = base64::engine::general_purpose::STANDARD
        .decode(identity.public_key_b64()?)
        .map_err(|error| BlnkError::Identity(format!("decode device public key: {error}")))?;
    offer_message
        .streams
        .insert(DEVICE_PUBLIC_KEY_STREAM.to_owned(), public_key_der);
    offer_message.streams.insert(
        REQUEST_NONCE_STREAM.to_owned(),
        challenge.nonce().as_bytes().to_vec(),
    );
    let offer_message = SignalingMessage::Offer(offer_message);
    if let Err(error) = signaling.send(&offer_message).await {
        let _ = peer.close().await;
        return Err(error);
    }

    let answer = match recv_with_timeout(&mut signaling, timeout).await? {
        SignalingMessage::Answer(answer) => {
            validate_client_id(&client_id, &answer.client_id)?;
            answer
        }
        SignalingMessage::Error(error) => {
            let _ = peer.close().await;
            return Err(BlnkError::Signaling(format!(
                "signaling rejected SDP offer: {}",
                error.message
            )));
        }
        SignalingMessage::Candidate(_) => {
            let _ = peer.close().await;
            return Err(BlnkError::Signaling(
                "trickle ICE candidate arrived, but the current peer boundary only supports non-trickle SDP"
                    .into(),
            ));
        }
        other => {
            let _ = peer.close().await;
            return Err(unexpected_message("SDP answer", &other));
        }
    };
    let expected_fingerprint = fingerprint_for_sdp(&answer.sdp);
    if let Err(error) = challenge.validate_and_consume(
        identity,
        &answer.encrypted_request,
        &pin,
        &expected_fingerprint,
    ) {
        let _ = peer.close().await;
        return Err(error);
    }
    let answer = RTCSessionDescription::answer(answer.sdp)
        .map_err(|error| BlnkError::Peer(format!("parse remote SDP answer: {error}")))?;
    if let Err(error) = peer.set_remote_answer(answer).await {
        let _ = peer.close().await;
        return Err(error);
    }
    wait_peer_ready(&peer, timeout).await?;

    let mut runtime = match SessionRuntime::new(peer, SessionRuntimeConfig::server(pin)) {
        Ok(runtime) => runtime,
        Err(error) => return Err(error),
    };
    match tokio::time::timeout(timeout, runtime.handshake()).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let _ = runtime.close().await;
            return Err(error);
        }
        Err(_) => {
            let _ = runtime.close().await;
            return Err(BlnkError::Session(
                "timed out during authenticated session handshake".into(),
            ));
        }
    }
    let _ = signaling.close().await;
    Ok(ServerSession { runtime, client_id })
}

/// Requests a target endpoint and completes the offer/answer exchange as the
/// client/answerer. The registry endpoint selects the target device; the
/// protocol request carries this client's identity as `client_id`, matching
/// the existing signaling schema without inventing a new wire message.
pub async fn connect_target(
    endpoint: &str,
    identity: &Identity,
    target: &str,
    pin: String,
    endpoint_policy: EndpointPolicy,
    timeout: Duration,
) -> SignalingResult<ClientSession> {
    validate_timeout(timeout)?;
    require_non_empty(target, "target")?;
    let client = make_client(endpoint, endpoint_policy)?;
    let mut signaling = client
        .connect_with_retry()
        .await
        .map_err(|error| orchestration_error("connect signaling socket", error))?;
    let client_id = identity.uid().to_owned();
    let request = SignalingMessage::Request(ConnectionRequest::new(client_id.clone())?);
    signaling.send(&request).await?;

    let offer = match recv_with_timeout(&mut signaling, timeout).await? {
        SignalingMessage::Offer(offer) => {
            validate_client_id(&client_id, &offer.client_id)?;
            offer
        }
        SignalingMessage::Error(error) => {
            return Err(BlnkError::Signaling(format!(
                "signaling rejected target '{target}': {}",
                error.message
            )));
        }
        SignalingMessage::Candidate(_) => {
            return Err(BlnkError::Signaling(
                "trickle ICE candidate arrived before the non-trickle offer".into(),
            ));
        }
        other => return Err(unexpected_message("SDP offer", &other)),
    };

    let target_public_key = offer.streams.get(DEVICE_PUBLIC_KEY_STREAM).ok_or_else(|| {
        BlnkError::Signaling(
            "signaling offer omitted device_public_key required for encrypted_request".into(),
        )
    })?;
    let target_public_key = RsaPublicKey::from_public_key_der(target_public_key)
        .map_err(|error| BlnkError::Identity(format!("parse target public key: {error}")))?;
    let request_nonce = offer.streams.get(REQUEST_NONCE_STREAM).ok_or_else(|| {
        BlnkError::Signaling(
            "signaling offer omitted request_nonce required for encrypted_request".into(),
        )
    })?;
    let request_nonce = std::str::from_utf8(request_nonce)
        .map_err(|_| BlnkError::Signaling("signaling request_nonce is not valid UTF-8".into()))?;
    require_non_empty(request_nonce, "request_nonce")?;
    let offer = RTCSessionDescription::offer(offer.sdp)
        .map_err(|error| BlnkError::Peer(format!("parse remote SDP offer: {error}")))?;
    let peer = PeerHandle::new().await?;
    let answer = match peer.accept_offer(offer).await {
        Ok(answer) => answer,
        Err(error) => {
            let _ = peer.close().await;
            return Err(error);
        }
    };
    let fingerprint = fingerprint_for_sdp(&answer.sdp);
    let encrypted_request =
        encrypted_session_request(&fingerprint, request_nonce, &pin, &target_public_key)?;

    let answer_message = SignalingMessage::Answer(AnswerMessage::new(
        client_id.clone(),
        answer.sdp.clone(),
        encrypted_request,
    )?);
    if let Err(error) = signaling.send(&answer_message).await {
        let _ = peer.close().await;
        return Err(error);
    }
    if let Err(error) = wait_peer_ready(&peer, timeout).await {
        let _ = peer.close().await;
        return Err(error);
    }

    let mut runtime = match SessionRuntime::new(peer, SessionRuntimeConfig::client(pin)) {
        Ok(runtime) => runtime,
        Err(error) => return Err(error),
    };
    match tokio::time::timeout(timeout, runtime.handshake()).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let _ = runtime.close().await;
            return Err(error);
        }
        Err(_) => {
            let _ = runtime.close().await;
            return Err(BlnkError::Session(
                "timed out during authenticated session handshake".into(),
            ));
        }
    }
    let _ = signaling.close().await;
    Ok(ClientSession {
        runtime,
        client_id,
        target: target.to_owned(),
    })
}

// TODO: Add a latched operation scope and per-stream audit record here before
// exposing additional RustDesk-inspired operations beyond shell and file.
/// Owns one authenticated server session and guarantees that its dispatcher
/// receives frames through one reader until cancellation, peer close, or error.
pub struct ConnectionSupervisor {
    session: ServerSession,
    root: PathBuf,
    shutdown: CancellationToken,
}

impl ConnectionSupervisor {
    pub fn new(
        session: ServerSession,
        root: impl AsRef<Path>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            session,
            root: root.as_ref().to_path_buf(),
            shutdown,
        }
    }

    pub async fn run(self) -> SignalingResult<()> {
        serve_session_with_shutdown(self.session, self.root, self.shutdown).await
    }
}

/// Runs a server-side authenticated session with a single-reader stream
/// dispatcher. Shell and file streams are handled using the existing bounded
/// services; unrecognized or out-of-order frames are errors, not success.
pub async fn serve_session(session: ServerSession, root: impl AsRef<Path>) -> SignalingResult<()> {
    ConnectionSupervisor::new(session, root, CancellationToken::new())
        .run()
        .await
}

/// Runs the dispatcher until a terminal frame, disconnect, or cancellation.
pub async fn serve_session_with_shutdown(
    mut session: ServerSession,
    root: impl AsRef<Path>,
    shutdown: CancellationToken,
) -> SignalingResult<()> {
    let root = root.as_ref().to_path_buf();
    let result = async {
        loop {
        let frame = tokio::select! {
            _ = shutdown.cancelled() => break Ok(()),
            result = session.runtime.recv_frame() => match result {
                Ok(frame) => frame,
                Err(BlnkError::Peer(message)) if message.contains("receive loop closed") => break Ok(()),
                Err(error) => break Err(error),
            },
        };
        if frame.stream_id == 0 {
            if frame.flags.is_fin() {
                break Ok(());
            }
            break Err(BlnkError::Protocol(
                "unexpected control frame after session handshake".into(),
            ));
        }
        if frame.flags.is_fin() {
            if session.runtime.stream_kind(frame.stream_id).is_some() {
                session.runtime.close_stream(frame.stream_id)?;
                continue;
            }
            break Err(BlnkError::Stream(format!(
                "FIN for unknown stream id: {}",
                frame.stream_id
            )));
        }
        if !frame.flags.is_syn() || !frame.flags.is_dat() {
            break Err(BlnkError::Protocol(format!(
                "stream {} must start with SYN|DAT",
                frame.stream_id
            )));
        }

        if let Ok(command) = decode_open_frame(&frame) {
            dispatch_shell(&mut session.runtime, frame.stream_id, command, &root).await?;
            continue;
        }
        if let Ok(request) = decode_request_frame(&frame) {
            dispatch_file(&mut session.runtime, frame.stream_id, request, &root).await?;
            continue;
        }
            break Err(BlnkError::Protocol(format!(
                "unknown stream opener for stream {}",
                frame.stream_id
            )));
        }
    }
    .await;
    let close_result = session.runtime.close().await;
    match (result, close_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

/// Executes one remote shell command and returns its process status.
pub async fn run_shell_client(
    runtime: &mut SessionRuntime,
    command: &ShellCommand,
) -> SignalingResult<i32> {
    let stream = runtime.open_shell_stream("/")?;
    let stream_id = stream.id();
    runtime.send_shell_open(stream_id, command).await?;
    loop {
        let frame = runtime.recv_shell_frame().await?;
        if frame.flags.is_fin() {
            if let Ok(exit) = decode_exit_frame(&frame) {
                let _ = runtime.close_shell_stream(stream_id).await;
                return Ok(exit.code);
            }
            let message = decode_error_frame(&frame)
                .map(|error| error.message)
                .unwrap_or_else(|_| "remote shell failed".to_owned());
            let _ = runtime.close_shell_stream(stream_id).await;
            return Err(BlnkError::Stream(format!("remote shell error: {message}")));
        }
        let output = decode_output_frame(&frame)?;
        write_stdout(&output.data)?;
    }
}

/// Performs upload/download over an authenticated remote file stream.
pub async fn run_file_client(
    runtime: &mut SessionRuntime,
    source: &str,
    destination: &str,
    overwrite: bool,
) -> SignalingResult<()> {
    let download_path = source.strip_prefix("remote:");
    let (request, upload, local_destination) = if let Some(remote_path) = download_path {
        (
            FileTransferRequest::get(remote_path, None)?,
            Vec::new(),
            Some(PathBuf::from(destination)),
        )
    } else {
        let data = std::fs::read(source).map_err(BlnkError::Io)?;
        (
            FileTransferRequest::put(destination, data.len() as u64, overwrite),
            data,
            None,
        )
    };

    let stream = runtime.open_file_stream("/")?;
    let stream_id = stream.id();
    runtime.send_file_request(stream_id, &request).await?;
    if request.operation == FileOperation::Put {
        if upload.is_empty() {
            runtime.send_file_chunk(stream_id, Vec::new(), true).await?;
        } else {
            for (index, chunk) in upload
                .chunks(crate::protocol::swsp::DEFAULT_MAX_PAYLOAD_LEN)
                .enumerate()
            {
                runtime
                    .send_file_chunk(
                        stream_id,
                        chunk.to_vec(),
                        (index + 1) * crate::protocol::swsp::DEFAULT_MAX_PAYLOAD_LEN
                            >= upload.len(),
                    )
                    .await?;
            }
        }
    }

    let mut received_data = Vec::new();
    let mut metadata_seen = false;
    loop {
        let frame = runtime.recv_file_frame().await?;
        if !metadata_seen {
            let metadata = decode_response_metadata(&request, &frame)?;
            metadata_seen = true;
            if let FileTransferResponse::Download { info, .. } = metadata {
                println!("received={} bytes={}", info.name, info.size);
            }
        } else if request.operation == FileOperation::Get {
            received_data.extend_from_slice(&frame.payload);
        }
        if frame.flags.is_fin() {
            break;
        }
    }

    if let Some(destination) = local_destination {
        if let Some(parent) = destination.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(BlnkError::Io)?;
        }
        std::fs::write(&destination, received_data).map_err(BlnkError::Io)?;
        println!("copied {} -> {}", source, destination.display());
    } else {
        println!("copied {} -> remote:{}", source, destination);
    }
    runtime.close_file_stream(stream_id).await.map(|_| ())
}

async fn dispatch_shell(
    runtime: &mut SessionRuntime,
    stream_id: u32,
    command: ShellCommand,
    root: &Path,
) -> SignalingResult<()> {
    runtime.accept_shell_stream(stream_id, "/")?;
    let policy = ShellPolicy::new(root.to_path_buf())?;
    let handler = ShellStreamHandler::new(policy);
    match handler.run(command, CancellationToken::new()).await {
        Ok(result) => {
            if !result.stdout.is_empty() {
                runtime
                    .send_shell_output(stream_id, result.stdout, false)
                    .await?;
            }
            if !result.stderr.is_empty() {
                runtime
                    .send_shell_output(stream_id, result.stderr, false)
                    .await?;
            }
            runtime
                .send_shell_exit(stream_id, result.exit_code.unwrap_or(-1), result.signaled)
                .await
        }
        Err(error) => runtime.send_shell_error(stream_id, error.to_string()).await,
    }
}

async fn dispatch_file(
    runtime: &mut SessionRuntime,
    stream_id: u32,
    request: FileTransferRequest,
    root: &Path,
) -> SignalingResult<()> {
    runtime.accept_file_stream(stream_id, "/")?;
    let mut body_frames = Vec::new();
    if request.operation == FileOperation::Put {
        loop {
            let frame = runtime.recv_file_frame().await?;
            if frame.stream_id != stream_id {
                return Err(BlnkError::Protocol(format!(
                    "file stream {} received frame for stream {}",
                    stream_id, frame.stream_id
                )));
            }
            let final_chunk = frame.flags.is_fin();
            body_frames.push(frame);
            if final_chunk {
                break;
            }
        }
    }
    let service = FileTransferService::new(FileTransferConfig::new(root.to_path_buf())?);
    let body = if request.operation == FileOperation::Put {
        collect_data_frames(
            body_frames,
            request.size,
            service.config().max_file_size,
            &FileTransferCancellation::default(),
        )?
    } else {
        Vec::new()
    };
    let response = service
        .execute(request, &body, &FileTransferCancellation::default())
        .await?;
    for frame in encode_response_frames(stream_id, &response)? {
        runtime.send_frame(&frame).await?;
    }
    Ok(())
}

async fn wait_peer_ready(peer: &PeerHandle, timeout: Duration) -> SignalingResult<()> {
    tokio::time::timeout(timeout, async {
        peer.wait_connected().await?;
        peer.wait_channel_open().await
    })
    .await
    .map_err(|_| BlnkError::Peer("timed out waiting for connected data channel".into()))??;
    Ok(())
}

async fn recv_with_timeout(
    connection: &mut crate::signaling::SignalingConnection,
    timeout: Duration,
) -> SignalingResult<SignalingMessage> {
    match tokio::time::timeout(timeout, connection.recv()).await {
        Ok(Ok(Some(message))) => Ok(message),
        Ok(Ok(None)) => Err(BlnkError::Signaling(
            "signaling connection closed before the expected message".into(),
        )),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(BlnkError::Signaling(
            "timed out waiting for the expected signaling message".into(),
        )),
    }
}

fn make_client(
    endpoint: &str,
    endpoint_policy: EndpointPolicy,
) -> SignalingResult<crate::signaling::SignalingClient> {
    Ok(crate::signaling::SignalingClient::new(endpoint)?
        .with_endpoint_policy(endpoint_policy)
        .with_reconnect_policy(ReconnectPolicy::limited(
            INITIAL_RECONNECT_RETRIES,
            Duration::from_millis(50),
        )))
}

#[derive(Debug)]
struct RequestChallenge {
    nonce: String,
    consumed: bool,
}

impl RequestChallenge {
    fn new() -> Self {
        Self {
            nonce: uuid::Uuid::new_v4().to_string(),
            consumed: false,
        }
    }

    fn nonce(&self) -> &str {
        &self.nonce
    }

    fn validate_and_consume(
        &mut self,
        identity: &Identity,
        encoded: &str,
        expected_pin: &str,
        expected_fingerprint: &str,
    ) -> SignalingResult<()> {
        if self.consumed {
            return Err(BlnkError::Signaling(
                "encrypted request nonce has already been consumed".into(),
            ));
        }
        // Consume before validation so a malformed or wrong-PIN answer cannot
        // reuse the same challenge on a retrying signaling connection.
        self.consumed = true;
        validate_encrypted_request(
            identity,
            encoded,
            self.nonce(),
            expected_pin,
            expected_fingerprint,
        )
    }
}

fn encrypted_session_request(
    fingerprint: &str,
    nonce: &str,
    code: &str,
    target_public_key: &RsaPublicKey,
) -> SignalingResult<String> {
    #[derive(Serialize)]
    struct RequestEnvelope<'a> {
        fingerprint: &'a str,
        nonce: &'a str,
        code: &'a str,
    }
    let envelope = RequestEnvelope {
        fingerprint,
        nonce,
        code,
    };
    let plaintext = serde_json::to_vec(&envelope)
        .map_err(|error| BlnkError::Protocol(format!("encode encrypted request: {error}")))?;
    let ciphertext = Identity::encrypt_for_peer(target_public_key, &plaintext)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(ciphertext))
}

#[derive(Debug, Deserialize)]
struct SessionRequestEnvelope {
    fingerprint: String,
    nonce: String,
    code: String,
}

fn validate_encrypted_request(
    identity: &Identity,
    encoded: &str,
    expected_nonce: &str,
    expected_pin: &str,
    expected_fingerprint: &str,
) -> SignalingResult<()> {
    require_non_empty(expected_nonce, "expected request nonce")?;
    require_non_empty(expected_pin, "expected PIN")?;
    require_non_empty(expected_fingerprint, "expected request fingerprint")?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| BlnkError::Signaling(format!("decode encrypted request: {error}")))?;
    let plaintext = identity.decrypt(&ciphertext)?;
    let request: SessionRequestEnvelope = serde_json::from_slice(&plaintext).map_err(|error| {
        BlnkError::Signaling(format!("decode encrypted request payload: {error}"))
    })?;
    require_non_empty(&request.fingerprint, "encrypted request fingerprint")?;
    require_non_empty(&request.nonce, "encrypted request nonce")?;
    require_non_empty(&request.code, "encrypted request code")?;
    if !constant_time_string_eq(&request.nonce, expected_nonce) {
        return Err(BlnkError::Signaling(
            "encrypted request nonce does not match the pending challenge".into(),
        ));
    }
    if !constant_time_string_eq(&request.fingerprint, expected_fingerprint) {
        return Err(BlnkError::Signaling(
            "encrypted request fingerprint does not match the negotiated answer".into(),
        ));
    }
    if !constant_time_pin_eq(expected_pin, &request.code) {
        return Err(BlnkError::Signaling(
            "encrypted request PIN rejected".into(),
        ));
    }
    Ok(())
}

fn fingerprint_for_sdp(sdp: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha256::digest(sdp.as_bytes()))
}

fn constant_time_string_eq(expected: &str, provided: &str) -> bool {
    let expected_digest = Sha256::digest(expected.as_bytes());
    let provided_digest = Sha256::digest(provided.as_bytes());
    expected_digest.ct_eq(&provided_digest).into()
}

fn constant_time_pin_eq(expected: &str, provided: &str) -> bool {
    let mut expected_fixed = [0_u8; PIN_LEN];
    let mut provided_fixed = [0_u8; PIN_LEN];
    let expected_copy_len = expected.len().min(PIN_LEN);
    let provided_copy_len = provided.len().min(PIN_LEN);
    expected_fixed[..expected_copy_len].copy_from_slice(&expected.as_bytes()[..expected_copy_len]);
    provided_fixed[..provided_copy_len].copy_from_slice(&provided.as_bytes()[..provided_copy_len]);
    let contents_match = expected_fixed.ct_eq(&provided_fixed);
    let expected_len_match = Choice::from((expected.len() == PIN_LEN) as u8);
    let provided_len_match = Choice::from((provided.len() == PIN_LEN) as u8);
    (contents_match & expected_len_match & provided_len_match).into()
}

fn validate_timeout(timeout: Duration) -> SignalingResult<()> {
    if timeout.is_zero() {
        return Err(BlnkError::Signaling(
            "orchestration timeout must be greater than zero".into(),
        ));
    }
    Ok(())
}

fn require_non_empty(value: &str, field: &str) -> SignalingResult<()> {
    if value.trim().is_empty() {
        return Err(BlnkError::Signaling(format!("{field} must not be empty")));
    }
    Ok(())
}

fn validate_client_id(expected: &str, actual: &str) -> SignalingResult<()> {
    if expected != actual {
        return Err(BlnkError::Signaling(format!(
            "signaling client_id mismatch: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn unexpected_message(expected: &str, actual: &SignalingMessage) -> BlnkError {
    BlnkError::Signaling(format!(
        "expected {expected}, received {}",
        actual.message_type()
    ))
}

fn orchestration_error(operation: &str, error: BlnkError) -> BlnkError {
    BlnkError::Signaling(format!("{operation}: {error}"))
}

fn write_stdout(data: &[u8]) -> SignalingResult<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(data).map_err(BlnkError::Io)?;
    stdout.flush().map_err(BlnkError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{session::SessionState, signaling::RegisterResponse};
    use futures::{SinkExt, StreamExt};
    use std::{collections::HashMap, sync::Arc};
    use tokio::sync::{Mutex, mpsc, oneshot};
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn relay_completes_register_offer_answer_and_authenticated_session() {
        let fixture = RelayFixture::start().await;
        let server_identity = Identity::generate().expect("server identity");
        let client_identity = Identity::generate().expect("client identity");
        let endpoint = fixture.url();
        let (server_result, client_result) = tokio::join!(
            accept_server_session(
                &endpoint,
                &server_identity,
                "123456".into(),
                EndpointPolicy::AllowLocal,
                Duration::from_secs(10),
            ),
            connect_target(
                &endpoint,
                &client_identity,
                "device-1",
                "123456".into(),
                EndpointPolicy::AllowLocal,
                Duration::from_secs(10),
            )
        );
        let mut server = server_result.expect("server session should be ready");
        let mut client = client_result.expect("client session should be ready");
        assert_eq!(server.runtime.state(), SessionState::Ready);
        assert_eq!(client.runtime.state(), SessionState::Ready);
        assert_eq!(server.client_id, client.client_id);
        server.runtime.close().await.expect("server closes");
        client.runtime.close().await.expect("client closes");
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connection_supervisor_closes_cleanly_when_cancelled() {
        let fixture = RelayFixture::start().await;
        let server_identity = Identity::generate().expect("server identity");
        let client_identity = Identity::generate().expect("client identity");
        let endpoint = fixture.url();
        let (server_result, client_result) = tokio::join!(
            accept_server_session(
                &endpoint,
                &server_identity,
                "123456".into(),
                EndpointPolicy::AllowLocal,
                Duration::from_secs(10),
            ),
            connect_target(
                &endpoint,
                &client_identity,
                "device-1",
                "123456".into(),
                EndpointPolicy::AllowLocal,
                Duration::from_secs(10),
            )
        );
        let server = server_result.expect("server session should be ready");
        let mut client = client_result.expect("client session should be ready");
        let shutdown = CancellationToken::new();
        shutdown.cancel();
        ConnectionSupervisor::new(server, ".", shutdown)
            .run()
            .await
            .expect("cancelled supervisor should close cleanly");
        client.runtime.close().await.expect("client closes");
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn missing_registered_device_returns_typed_signaling_error() {
        let fixture = RelayFixture::start().await;
        let identity = Identity::generate().expect("identity");
        let error = connect_target(
            &fixture.url(),
            &identity,
            "missing",
            "123456".into(),
            EndpointPolicy::AllowLocal,
            Duration::from_secs(2),
        )
        .await
        .err()
        .expect("relay without a device must fail");
        assert!(error.to_string().contains("no registered device"));
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_wait_timeout_is_not_reported_as_success() {
        let fixture = RelayFixture::start().await;
        let identity = Identity::generate().expect("identity");
        let error = accept_server_session(
            &fixture.url(),
            &identity,
            "123456".into(),
            EndpointPolicy::AllowLocal,
            Duration::from_millis(20),
        )
        .await
        .err()
        .expect("server must time out without a request");
        assert!(error.to_string().contains("timed out"));
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn relay_dispatches_remote_shell_and_file_streams() {
        let fixture = RelayFixture::start().await;
        let server_identity = Identity::generate().expect("server identity");
        let client_identity = Identity::generate().expect("client identity");
        let endpoint = fixture.url();
        let (server_result, client_result) = tokio::join!(
            accept_server_session(
                &endpoint,
                &server_identity,
                "123456".into(),
                EndpointPolicy::AllowLocal,
                Duration::from_secs(10),
            ),
            connect_target(
                &endpoint,
                &client_identity,
                "device-1",
                "123456".into(),
                EndpointPolicy::AllowLocal,
                Duration::from_secs(10),
            )
        );
        let server = server_result.expect("server session should be ready");
        let mut client = client_result.expect("client session should be ready");
        let root = std::env::temp_dir().join(format!("blnk-remote-root-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create remote root");
        let source =
            std::env::temp_dir().join(format!("blnk-client-source-{}.txt", uuid::Uuid::new_v4()));
        let download =
            std::env::temp_dir().join(format!("blnk-client-download-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(&source, b"remote file body").expect("write client source");
        let server_root = root.clone();
        let server_task = tokio::spawn(serve_session(server, server_root));

        #[cfg(windows)]
        let shell_command = ShellCommand::new("cmd.exe").args(["/C", "echo remote shell"]);
        #[cfg(not(windows))]
        let shell_command = ShellCommand::new("printf").arg("remote shell\n");
        let exit = run_shell_client(&mut client.runtime, &shell_command)
            .await
            .expect("remote shell should complete");
        assert_eq!(exit, 0);
        run_file_client(
            &mut client.runtime,
            source.to_str().expect("source path"),
            "uploaded.txt",
            false,
        )
        .await
        .expect("remote upload should complete");
        run_file_client(
            &mut client.runtime,
            "remote:uploaded.txt",
            download.to_str().expect("download path"),
            false,
        )
        .await
        .expect("remote download should complete");
        client.runtime.close().await.expect("client closes");
        server_task
            .await
            .expect("server task joins")
            .expect("server dispatcher closes cleanly");
        assert_eq!(
            std::fs::read(root.join("uploaded.txt")).expect("uploaded file"),
            b"remote file body"
        );
        assert_eq!(
            std::fs::read(&download).expect("downloaded file"),
            b"remote file body"
        );
        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_file(download);
        let _ = std::fs::remove_dir_all(root);
        fixture.shutdown().await.expect("fixture shuts down");
    }

    #[test]
    fn mismatched_client_id_is_rejected_before_peer_mutation() {
        let error = validate_client_id("expected", "other").expect_err("ids differ");
        assert!(error.to_string().contains("client_id mismatch"));
    }

    #[test]
    fn encrypted_request_is_non_empty_and_does_not_expose_identity_key() {
        let identity = Identity::generate().expect("identity");
        let fingerprint = fingerprint_for_sdp("answer");
        let nonce = "nonce-1";
        let encoded =
            encrypted_session_request(&fingerprint, nonce, "123456", &identity.public_key())
                .expect("request");
        assert!(!encoded.is_empty());
        validate_encrypted_request(&identity, &encoded, nonce, "123456", &fingerprint)
            .expect("request decrypts and validates");
        assert!(!encoded.contains(identity.uid()));
        assert!(!encoded.contains("PRIVATE KEY"));
    }

    #[test]
    fn encrypted_request_rejects_wrong_pin_nonce_and_fingerprint() {
        let identity = Identity::generate().expect("identity");
        let public_key = identity.public_key();
        let fingerprint = fingerprint_for_sdp("answer");
        let nonce = "nonce-1";

        let wrong_pin = encrypted_session_request(&fingerprint, nonce, "654321", &public_key)
            .expect("wrong-pin request");
        let error =
            validate_encrypted_request(&identity, &wrong_pin, nonce, "123456", &fingerprint)
                .expect_err("wrong PIN must be rejected");
        assert!(error.to_string().contains("PIN rejected"));
        assert!(!error.to_string().contains("654321"));

        let wrong_nonce = encrypted_session_request(&fingerprint, "nonce-2", "123456", &public_key)
            .expect("wrong-nonce request");
        let error =
            validate_encrypted_request(&identity, &wrong_nonce, nonce, "123456", &fingerprint)
                .expect_err("wrong nonce must be rejected");
        assert!(error.to_string().contains("pending challenge"));

        let wrong_fingerprint = encrypted_session_request(
            &fingerprint_for_sdp("other-answer"),
            nonce,
            "123456",
            &public_key,
        )
        .expect("wrong-fingerprint request");
        let error = validate_encrypted_request(
            &identity,
            &wrong_fingerprint,
            nonce,
            "123456",
            &fingerprint,
        )
        .expect_err("wrong fingerprint must be rejected");
        assert!(error.to_string().contains("negotiated answer"));
    }

    #[test]
    fn request_challenge_is_single_use_even_after_validation_failure() {
        let identity = Identity::generate().expect("identity");
        let fingerprint = fingerprint_for_sdp("answer");
        let encoded =
            encrypted_session_request(&fingerprint, "nonce-1", "654321", &identity.public_key())
                .expect("request");
        let mut challenge = RequestChallenge {
            nonce: "nonce-1".into(),
            consumed: false,
        };
        assert!(
            challenge
                .validate_and_consume(&identity, &encoded, "123456", &fingerprint)
                .is_err()
        );
        let error = challenge
            .validate_and_consume(&identity, &encoded, "123456", &fingerprint)
            .expect_err("challenge must not be reusable");
        assert!(error.to_string().contains("already been consumed"));
    }

    #[test]
    fn pin_comparison_requires_six_digits_without_secret_dependent_errors() {
        assert!(constant_time_string_eq("nonce-1", "nonce-1"));
        assert!(!constant_time_string_eq("nonce-1", "nonce-2"));
        assert!(!constant_time_string_eq("nonce-1", "nonce-10"));
        assert!(constant_time_pin_eq("123456", "123456"));
        assert!(!constant_time_pin_eq("123456", "12345"));
        assert!(!constant_time_pin_eq("123456", "1234567"));
        assert!(!constant_time_pin_eq("123456", "654321"));
    }

    struct RelayFixture {
        address: std::net::SocketAddr,
        shutdown: Option<oneshot::Sender<()>>,
        task: Option<tokio::task::JoinHandle<()>>,
    }

    #[derive(Default)]
    struct RelayState {
        device: Option<mpsc::Sender<SignalingMessage>>,
        clients: HashMap<String, mpsc::Sender<SignalingMessage>>,
    }

    impl RelayFixture {
        async fn start() -> Self {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .expect("bind relay fixture");
            let address = listener.local_addr().expect("relay address");
            let state = Arc::new(Mutex::new(RelayState::default()));
            let (shutdown, mut shutdown_receiver) = oneshot::channel();
            let task_state = Arc::clone(&state);
            let task = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = &mut shutdown_receiver => break,
                        accepted = listener.accept() => {
                            let Ok((stream, _)) = accepted else { break };
                            let state = Arc::clone(&task_state);
                            tokio::spawn(async move {
                                let Ok(socket) = accept_async(stream).await else { return };
                                relay_connection(socket, state).await;
                            });
                        }
                    }
                }
            });
            Self {
                address,
                shutdown: Some(shutdown),
                task: Some(task),
            }
        }

        fn url(&self) -> String {
            format!("ws://{}", self.address)
        }

        async fn shutdown(mut self) -> Result<(), BlnkError> {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
            if let Some(task) = self.task.take() {
                task.await
                    .map_err(|error| BlnkError::Signaling(error.to_string()))?;
            }
            Ok(())
        }
    }

    async fn relay_connection<S>(
        socket: tokio_tungstenite::WebSocketStream<S>,
        state: Arc<Mutex<RelayState>>,
    ) where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (mut sink, mut source) = socket.split();
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<SignalingMessage>(16);
        loop {
            tokio::select! {
                outbound = outbound_rx.recv() => {
                    let Some(message) = outbound else { return };
                    let Ok(payload) = crate::signaling::encode_message(&message) else { return };
                    if sink.send(Message::Text(payload.into())).await.is_err() { return; }
                }
                inbound = source.next() => {
                    let Some(Ok(frame)) = inbound else { return };
                    let Message::Text(payload) = frame else { continue };
                    let Ok(message) = crate::signaling::decode_message(payload.as_ref()) else { return };
                    route_relay_message(message, outbound_tx.clone(), Arc::clone(&state)).await;
                }
            }
        }
    }

    async fn route_relay_message(
        message: SignalingMessage,
        sender: mpsc::Sender<SignalingMessage>,
        state: Arc<Mutex<RelayState>>,
    ) {
        match message {
            SignalingMessage::Register(_) => {
                let mut relay = state.lock().await;
                relay.device = Some(sender.clone());
                if let Ok(response) = RegisterResponse::new("123456") {
                    let _ = sender.send(SignalingMessage::Registered(response)).await;
                }
            }
            SignalingMessage::Request(request) => {
                {
                    let mut relay = state.lock().await;
                    relay
                        .clients
                        .insert(request.client_id.clone(), sender.clone());
                }
                let device = tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if let Some(device) = state.lock().await.device.clone() {
                            break device;
                        }
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                })
                .await
                .ok();
                if let Some(device) = device {
                    let _ = device.send(SignalingMessage::Request(request)).await;
                } else if let Ok(error) =
                    crate::signaling::SignalingError::new("no registered device")
                {
                    let _ = sender.send(SignalingMessage::Error(error)).await;
                }
            }

            SignalingMessage::Offer(offer) => {
                let relay = state.lock().await;
                if let Some(client) = relay.clients.get(&offer.client_id).cloned() {
                    let _ = client.send(SignalingMessage::Offer(offer)).await;
                }
            }
            SignalingMessage::Answer(answer) => {
                let relay = state.lock().await;
                if let Some(device) = relay.device.clone() {
                    let _ = device.send(SignalingMessage::Answer(answer)).await;
                }
            }
            other => {
                let relay = state.lock().await;
                if let Some(device) = relay.device.clone() {
                    let _ = device.send(other).await;
                }
            }
        }
    }
}
