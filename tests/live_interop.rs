use blnk::peer::TwoPeerHarness;
use blnk::protocol::swsp::{DEFAULT_MAX_PAYLOAD_LEN, Frame, FrameFlags};
use blnk::session::SessionState;
use blnk::session::runtime::{SessionRuntime, SessionRuntimeConfig};
use blnk::stream::{StreamKind, StreamRegistry};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_full_interop_lifecycle_swsp_streaming_and_streams() {
    let harness = TwoPeerHarness::new("live-interop-test")
        .await
        .expect("harness should connect");

    let mut server = SessionRuntime::new(
        harness.offerer,
        SessionRuntimeConfig::server("654321").with_capabilities(vec![
            "shell".into(),
            "file".into(),
            "proxy".into(),
        ]),
    )
    .expect("server runtime should build");

    let mut client = SessionRuntime::new(harness.answerer, SessionRuntimeConfig::client("654321"))
        .expect("client runtime should build");

    let (server_hs, client_hs) = tokio::join!(server.handshake(), client.handshake());
    server_hs.expect("server handshake must succeed");
    client_hs.expect("client handshake must succeed");

    assert_eq!(server.state(), SessionState::Ready);
    assert_eq!(client.state(), SessionState::Ready);

    // Test stream registry creation and lookup for all stream kinds
    let mut registry = StreamRegistry::new();
    let shell_stream = registry
        .open(StreamKind::Shell, "bash")
        .expect("allocate shell stream");
    assert_ne!(shell_stream.id(), 0);
    assert_eq!(shell_stream.kind(), StreamKind::Shell);

    let file_stream = registry
        .open(StreamKind::File, "file_transfer")
        .expect("allocate file stream");
    assert_ne!(file_stream.id(), 0);
    assert_eq!(file_stream.kind(), StreamKind::File);

    let proxy_stream = registry
        .open(StreamKind::Tcp, "tcp_proxy")
        .expect("allocate proxy stream");
    assert_ne!(proxy_stream.id(), 0);
    assert_eq!(proxy_stream.kind(), StreamKind::Tcp);

    // Verify SWSP framing over data channel stream simulation
    let chunk_data = b"STREAMING_BINARY_DATA_CHUNK_FOR_FILE_TRANSFER";
    let frame = Frame::new(2, FrameFlags::DAT, chunk_data.to_vec());
    let encoded = frame.encode().expect("encode frame");
    let (decoded, consumed) = Frame::decode(&encoded).expect("decode frame");
    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded.stream_id, 2);
    assert_eq!(decoded.payload, chunk_data);

    // Clean close
    tokio::join!(server.close(), client.close())
        .0
        .expect("server close");
    assert_eq!(server.state(), SessionState::Closed);
    assert_eq!(client.state(), SessionState::Closed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_interop_negative_auth_failure() {
    let harness = TwoPeerHarness::new("live-interop-negative-auth")
        .await
        .expect("harness should connect");

    let mut server = SessionRuntime::new(harness.offerer, SessionRuntimeConfig::server("111111"))
        .expect("server runtime should build");

    let mut client = SessionRuntime::new(
        harness.answerer,
        SessionRuntimeConfig::client("999999"), // Wrong PIN
    )
    .expect("client runtime should build");

    let (server_res, client_res) = tokio::join!(server.handshake(), client.handshake());
    assert!(server_res.is_err() || client_res.is_err(), "Mismatched PINs must fail authentication");
}

#[test]
fn test_interop_negative_malformed_swsp_frames() {
    // 1. Frame too short (< 8 bytes header)
    let truncated = vec![0x01, 0x02, 0x03];
    assert!(Frame::decode(&truncated).is_err());

    // 2. Length header mismatch (claims 100 bytes, only 4 provided)
    let mut bad_len = vec![1, 0, 0, 0, 0, 0, 100, 0];
    bad_len.extend_from_slice(b"test");
    assert!(Frame::decode(&bad_len).is_err());

    // 3. Oversized payload > DEFAULT_MAX_PAYLOAD_LEN
    let huge_payload = vec![0u8; DEFAULT_MAX_PAYLOAD_LEN + 1];
    let frame = Frame::new(1, FrameFlags::DAT, huge_payload);
    assert!(frame.encode().is_err());
}
