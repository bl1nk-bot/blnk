//! Typed signaling message boundaries.
//!
//! This module models the signaling contract without selecting a transport or
//! claiming protobuf interoperability. Transport adapters can map these types
//! to the wire schema once generated bindings and fixtures are available.

use std::collections::BTreeMap;

use crate::utils::error::BlnkError;

pub mod orchestration;
pub mod transport;

pub use transport::{
    DEFAULT_MAX_MESSAGE_SIZE, EndpointPolicy, FixtureConfig, FixtureSnapshot, LocalFixtureServer,
    ReconnectPolicy, SignalingClient, SignalingConnection, SignalingMessage, TransportResult,
    decode_message, encode_message,
};

pub const PROTOCOL_VERSION: i32 = 3;
pub type SignalingResult<T> = Result<T, BlnkError>;

fn require_message_type(actual: &str, expected: &str) -> SignalingResult<()> {
    if actual != expected {
        return Err(BlnkError::Signaling(format!(
            "message_type must be {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn require_non_empty(value: &str, field: &str) -> SignalingResult<()> {
    if value.trim().is_empty() {
        return Err(BlnkError::Signaling(format!("{field} must not be empty")));
    }
    Ok(())
}

fn validate_pairing_code(value: &str) -> SignalingResult<()> {
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BlnkError::Signaling(
            "pairing_code must contain exactly six decimal digits".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub version: i32,
}

impl ProtocolVersion {
    pub fn current() -> Self {
        Self {
            version: PROTOCOL_VERSION,
        }
    }

    pub fn validate(&self) -> SignalingResult<()> {
        if self.version != PROTOCOL_VERSION {
            return Err(BlnkError::Signaling(format!(
                "unsupported signaling protocol version: {}",
                self.version
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterRequest {
    pub message_type: String,
    pub uid: String,
    pub public_key: String,
    pub protocol: i32,
    pub want_code: bool,
}

impl RegisterRequest {
    pub fn new(
        uid: impl Into<String>,
        public_key: impl Into<String>,
        want_code: bool,
    ) -> SignalingResult<Self> {
        let request = Self {
            message_type: "register".into(),
            uid: uid.into(),
            public_key: public_key.into(),
            protocol: PROTOCOL_VERSION,
            want_code,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "register")?;
        require_non_empty(&self.uid, "uid")?;
        require_non_empty(&self.public_key, "public_key")?;
        ProtocolVersion {
            version: self.protocol,
        }
        .validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterResponse {
    pub message_type: String,
    pub pairing_code: String,
}

impl RegisterResponse {
    pub fn new(pairing_code: impl Into<String>) -> SignalingResult<Self> {
        let response = Self {
            message_type: "registered".into(),
            pairing_code: pairing_code.into(),
        };
        response.validate()?;
        Ok(response)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "registered")?;
        validate_pairing_code(&self.pairing_code)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceServer {
    pub urls: String,
    pub username: Option<String>,
    pub credential: Option<String>,
}

impl IceServer {
    pub fn new(urls: impl Into<String>) -> SignalingResult<Self> {
        let server = Self {
            urls: urls.into(),
            username: None,
            credential: None,
        };
        server.validate()?;
        Ok(server)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_non_empty(&self.urls, "ICE server urls")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRequest {
    pub message_type: String,
    pub client_id: String,
    pub browser_ip: Option<String>,
    pub ice_servers: Vec<IceServer>,
    pub force_relay: bool,
}

impl ConnectionRequest {
    pub fn new(client_id: impl Into<String>) -> SignalingResult<Self> {
        let request = Self {
            message_type: "request".into(),
            client_id: client_id.into(),
            browser_ip: None,
            ice_servers: Vec::new(),
            force_relay: false,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "request")?;
        require_non_empty(&self.client_id, "client_id")?;
        for server in &self.ice_servers {
            server.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferMessage {
    pub message_type: String,
    pub client_id: String,
    pub sdp: String,
    pub streams: BTreeMap<String, Vec<u8>>,
}

impl OfferMessage {
    pub fn new(client_id: impl Into<String>, sdp: impl Into<String>) -> SignalingResult<Self> {
        let message = Self {
            message_type: "offer".into(),
            client_id: client_id.into(),
            sdp: sdp.into(),
            streams: BTreeMap::new(),
        };
        message.validate()?;
        Ok(message)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "offer")?;
        require_non_empty(&self.client_id, "client_id")?;
        require_non_empty(&self.sdp, "sdp")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerMessage {
    pub message_type: String,
    pub client_id: String,
    pub sdp: String,
    pub encrypted_request: String,
}

impl AnswerMessage {
    pub fn new(
        client_id: impl Into<String>,
        sdp: impl Into<String>,
        encrypted_request: impl Into<String>,
    ) -> SignalingResult<Self> {
        let message = Self {
            message_type: "answer".into(),
            client_id: client_id.into(),
            sdp: sdp.into(),
            encrypted_request: encrypted_request.into(),
        };
        message.validate()?;
        Ok(message)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "answer")?;
        require_non_empty(&self.client_id, "client_id")?;
        require_non_empty(&self.sdp, "sdp")?;
        require_non_empty(&self.encrypted_request, "encrypted_request")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceCandidateMessage {
    pub message_type: String,
    pub client_id: String,
    pub candidate: BTreeMap<String, String>,
}

impl IceCandidateMessage {
    pub fn new(
        client_id: impl Into<String>,
        candidate: BTreeMap<String, String>,
    ) -> SignalingResult<Self> {
        let message = Self {
            message_type: "candidate".into(),
            client_id: client_id.into(),
            candidate,
        };
        message.validate()?;
        Ok(message)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "candidate")?;
        require_non_empty(&self.client_id, "client_id")?;
        if self.candidate.is_empty() {
            return Err(BlnkError::Signaling("candidate must not be empty".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairRequest {
    pub message_type: String,
    pub client_id: String,
    pub remote_ip: Option<String>,
    pub ice_servers: Vec<IceServer>,
}

impl PairRequest {
    pub fn new(client_id: impl Into<String>) -> SignalingResult<Self> {
        let request = Self {
            message_type: "pair_request".into(),
            client_id: client_id.into(),
            remote_ip: None,
            ice_servers: Vec::new(),
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "pair_request")?;
        require_non_empty(&self.client_id, "client_id")?;
        for server in &self.ice_servers {
            server.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairAnswer {
    pub message_type: String,
    pub client_id: String,
    pub sdp: String,
}

impl PairAnswer {
    pub fn new(client_id: impl Into<String>, sdp: impl Into<String>) -> SignalingResult<Self> {
        let message = Self {
            message_type: "pair_answer".into(),
            client_id: client_id.into(),
            sdp: sdp.into(),
        };
        message.validate()?;
        Ok(message)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "pair_answer")?;
        require_non_empty(&self.client_id, "client_id")?;
        require_non_empty(&self.sdp, "sdp")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairApproved {
    pub message_type: String,
    pub client_id: String,
}

impl PairApproved {
    pub fn new(client_id: impl Into<String>) -> SignalingResult<Self> {
        let message = Self {
            message_type: "pair_approved".into(),
            client_id: client_id.into(),
        };
        message.validate()?;
        Ok(message)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "pair_approved")?;
        require_non_empty(&self.client_id, "client_id")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairRejected {
    pub message_type: String,
    pub client_id: String,
    pub reason: String,
}

impl PairRejected {
    pub fn new(client_id: impl Into<String>, reason: impl Into<String>) -> SignalingResult<Self> {
        let message = Self {
            message_type: "pair_rejected".into(),
            client_id: client_id.into(),
            reason: reason.into(),
        };
        message.validate()?;
        Ok(message)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "pair_rejected")?;
        require_non_empty(&self.client_id, "client_id")?;
        require_non_empty(&self.reason, "reason")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalingError {
    pub message_type: String,
    pub message: String,
}

impl SignalingError {
    pub fn new(message: impl Into<String>) -> SignalingResult<Self> {
        let message = Self {
            message_type: "error".into(),
            message: message.into(),
        };
        message.validate()?;
        Ok(message)
    }

    pub fn validate(&self) -> SignalingResult<()> {
        require_message_type(&self.message_type, "error")?;
        require_non_empty(&self.message, "message")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_protocol_version_is_accepted() {
        assert!(ProtocolVersion::current().validate().is_ok());
        assert!(ProtocolVersion { version: 2 }.validate().is_err());
    }

    #[test]
    fn register_response_uses_pairing_code_and_validates_digits() {
        assert!(RegisterResponse::new("123456").is_ok());
        assert!(RegisterResponse::new("12345").is_err());
        assert!(RegisterResponse::new("12345x").is_err());
    }

    #[test]
    fn signaling_messages_reject_missing_required_fields() {
        assert!(RegisterRequest::new("", "key", true).is_err());
        assert!(ConnectionRequest::new("").is_err());
        assert!(OfferMessage::new("client", "").is_err());
        assert!(IceCandidateMessage::new("client", BTreeMap::new()).is_err());
    }

    #[test]
    fn signaling_messages_reject_wrong_discriminators() {
        let mut request = RegisterRequest::new("uid", "key", true).expect("valid request");
        request.message_type = "answer".into();
        assert!(request.validate().is_err());

        let mut response = RegisterResponse::new("123456").expect("valid response");
        response.message_type = "register".into();
        assert!(response.validate().is_err());
    }

    #[test]
    fn answer_requires_encrypted_request() {
        assert!(AnswerMessage::new("client", "sdp", "").is_err());
        assert!(AnswerMessage::new("client", "sdp", "ciphertext").is_ok());
    }

    #[test]
    fn pairing_request_validates_ice_servers() {
        let mut request = PairRequest::new("client").expect("valid client");
        request
            .ice_servers
            .push(IceServer::new("stun:example.test").expect("valid ICE server"));
        assert!(request.validate().is_ok());
    }
}
