use blnk::peer::TwoPeerHarness;
use blnk::protocol::swsp::{Frame, FrameFlags};
use blnk::session::SessionState;
use blnk::session::runtime::{ControlMessage, SessionRuntime, SessionRuntimeConfig};
use blnk::signaling::transport::{SignalingMessage, decode_message, encode_message};
use blnk::signaling::{PROTOCOL_VERSION, RegisterRequest};

fn hex_fixture(input: &str) -> Vec<u8> {
    input
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("fixture must contain hexadecimal bytes"))
        .collect()
}

#[test]
fn signaling_register_fixture_preserves_wire_shape_and_semantics() {
    let expected = include_str!("fixtures/compatibility/v1/signaling_register.json").trim();
    let request = RegisterRequest::new("fixture-client", "fixture-public-key", true)
        .expect("fixture request should validate");
    let encoded = encode_message(&SignalingMessage::Register(request.clone()))
        .expect("register message should encode");

    assert_eq!(encoded, expected);
    let decoded = decode_message(expected).expect("fixture should decode");
    assert_eq!(decoded, SignalingMessage::Register(request));
}

#[test]
fn swsp_open_fixture_is_byte_exact_and_decodes() {
    let bytes = hex_fixture(include_str!(
        "fixtures/compatibility/v1/swsp_open_hello.hex"
    ));
    let (frame, consumed) = Frame::decode(&bytes).expect("SWSP fixture should decode");

    assert_eq!(consumed, bytes.len());
    assert_eq!(frame.stream_id, 7);
    assert_eq!(frame.flags, FrameFlags::SYN | FrameFlags::DAT);
    assert_eq!(frame.payload, b"hello");
    assert_eq!(frame.encode().expect("frame should re-encode"), bytes);
}

#[test]
fn control_connect_fixture_preserves_protobuf_payload() {
    let bytes = hex_fixture(include_str!(
        "fixtures/compatibility/v1/control_connect_v1.hex"
    ));
    let message = ControlMessage::decode(&bytes).expect("control fixture should decode");

    assert_eq!(
        message,
        ControlMessage::connect("/", Some(PROTOCOL_VERSION)).expect("connect should validate")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authenticated_session_fixture_reaches_ready_and_closes_cleanly() {
    let transcript: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/compatibility/v1/session_handshake_v1.json"
    ))
    .expect("transcript fixture must be valid JSON");
    assert_eq!(transcript["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(
        transcript["events"].as_array().expect("events array").len(),
        5
    );

    let harness = TwoPeerHarness::new("compatibility-baseline")
        .await
        .expect("local peer harness should connect");
    let mut server = SessionRuntime::new(
        harness.offerer,
        SessionRuntimeConfig::server("123456").with_capabilities(vec!["session".into()]),
    )
    .expect("server runtime should build");
    let mut client = SessionRuntime::new(harness.answerer, SessionRuntimeConfig::client("123456"))
        .expect("client runtime should build");

    let (server_result, client_result) = tokio::join!(server.handshake(), client.handshake());
    server_result.expect("server handshake should succeed");
    client_result.expect("client handshake should succeed");

    assert_eq!(server.state(), SessionState::Ready);
    assert_eq!(client.state(), SessionState::Ready);
    server.close().await.expect("server close should succeed");
    client.close().await.expect("client close should succeed");
    assert_eq!(server.state(), SessionState::Closed);
    assert_eq!(client.state(), SessionState::Closed);
}
