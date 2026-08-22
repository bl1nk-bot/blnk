# blnk Rust API

## 1. Overview

เอกสารนี้อธิบาย API ภายในของ blnk Rust สำหรับการพอร์ตจาก bitbang-cli ต้นฉบับ โดยอิงจาก protocol และ module structure ที่ออกแบบไว้
API ในเอกสารนี้แบ่งเป็น:
- CLI API
- Signaling API
- Peer API
- Session API
- Stream API
- Identity API
- Protocol API
- Utility API

### 1.1 Contract Status

เอกสารนี้เป็น **target API contract** สำหรับการ implement ไม่ใช่รายการของ public symbols ที่มีอยู่แล้วใน source tree ฟังก์ชันหรือ type ใดจะถือว่าใช้งานได้ก็ต่อเมื่อมี Rust implementation, error handling, tests และ protocol compatibility evidence รองรับ สถานะล่าสุดให้ดู [`docs/implementation-status.md`](implementation-status.md)

---

## 2. CLI API

## 2.1 `serve`

>เริ่มโหมด server/device เพื่อรอรับ connection

### Responsibilities
- load config
- load/generate identity
- connect signaling server
- register device
- wait for incoming request
- create peer/session
- dispatch streams

### Example

```rust
run_serve(args).await?;
```

---

## 2.2 `connect`

เชื่อมต่อไปยัง device ผ่าน signaling

### Responsibilities
- connect to signaling server
- submit connect request
- negotiate offer/answer
- open data channel
- establish session

---

## 2.3 `cp`
คัดลอกไฟล์ระหว่าง local/remote

### Responsibilities
- parse source/destination
- open file stream
- transfer file content
- report progress

---

## 2.4 `devices`
จัดการ device registry

### Responsibilities
- list devices
- show status
- remove saved device
- resolve identity info

---

## 3. Signaling API

## 3.1 `SignalingClient`

```rust
pub struct SignalingClient { /* ... */ }
```

### Methods
- `new(url: &str) -> Result<Self>` — ใช้ `EndpointPolicy::PublicOnly` เป็นค่าเริ่มต้นและทำ preflight DNS/IP validation ก่อน dial
- `with_endpoint_policy(policy: EndpointPolicy) -> Self` — ใช้ `EndpointPolicy::AllowLocal` เฉพาะ local fixture/test ที่ควบคุมได้
- `connect(&mut self) -> Result<()>`
- `register(&self, req: RegisterRequest) -> Result<RegisterResponse>`
- `send_offer(&self, msg: OfferMessage) -> Result<()>`
- `send_answer(&self, msg: AnswerMessage) -> Result<()>`
- `send_candidate(&self, msg: ICECandidateMessage) -> Result<()>`
- `listen(&self) -> Result<()>`

### Responsibilities
- maintain websocket connection
- preflight signaling endpoint policy before initial and retry connections
- encode/decode signaling messages
- route messages to session manager

---

## 4. Peer API

## 4.1 `PeerHandle`

```rust
pub struct PeerHandle { /* WebRTC connection, lifecycle state and SWSP receive queue */ }
```

`PeerHandle::new()` สร้าง peer ด้วย loopback UDP และ `RTCConfiguration` ที่ไม่มี ICE server ภายนอก การกำหนด STUN/TURN สำหรับ production ต้องมาจาก caller/configuration layer และอยู่นอก local harness ของ Issue #37

### Methods

- `new() -> Result<PeerHandle>` สร้าง peer และติดตั้ง event handler
- `create_data_channel(label: &str) -> Result<()>` สร้าง application data channel baseline ซึ่งใช้ค่าเริ่มต้นของ WebRTC crate สำหรับ reliable/ordered delivery
- `create_offer() -> Result<RTCSessionDescription>` สร้าง local offer และรอ non-trickle ICE gathering ให้เสร็จ
- `accept_offer(offer: RTCSessionDescription) -> Result<RTCSessionDescription>` รับ offer สร้าง answer และรอ ICE gathering
- `set_remote_answer(answer: RTCSessionDescription) -> Result<()>` ตั้งค่า remote answer
- `wait_connected() -> Result<()>` รอ connected state หรือคืน failure/timeout แบบ typed error
- `wait_channel_open() -> Result<()>` รอ channel open หรือคืน channel error/close/timeout แบบ typed error
- `send_frame(frame: &Frame) -> Result<()>` encode และส่ง SWSP frame เป็น binary data-channel message
- `recv_frame() -> Result<Frame>` รับ binary message และตรวจสอบ SWSP frame ต้อง consume payload ครบพอดี
- `close() -> Result<()>` ปิด data channel และ peer connection โดยเรียกซ้ำได้อย่างปลอดภัย

### Responsibilities

- manage WebRTC lifecycle และ state transitions
- handle non-trickle ICE/SDP offer-answer exchange
- open, monitor และ teardown data channel
- bridge binary data-channel messages กับ SWSP frame codec
- report failure, close และ timeout โดยไม่อ้าง external interoperability

## 4.2 `TwoPeerHarness`

```rust
pub struct TwoPeerHarness {
    pub offerer: PeerHandle,
    pub answerer: PeerHandle,
}
```

`TwoPeerHarness::new(label)` แลกเปลี่ยน offer/answer ภายใน process ผ่าน loopback peers สองฝั่ง โดยไม่ใช้ signaling server หรือ external STUN/TURN เพื่อให้ integration test deterministic และตรวจสอบ SWSP round-trip กับ lifecycle failure paths ได้

---

## 5. Session API

## 5.1 `Session`

```rust
pub struct Session { /* ... */ }
```

### Methods
- `new(config: SessionConfig) -> Result<Self>`
- `start(&mut self) -> Result<()>`
- `authenticate(&mut self, pin: Option<String>) -> Result<()>`
- `mark_ready(&mut self) -> Result<()>`
- `register_stream(&mut self, stream: StreamDescriptor) -> Result<()>`
- `close(&mut self) -> Result<()>`

### Responsibilities
- auth flow
- session state
- stream registry
- stats tracking
- teardown/cleanup

## 5.2 `SessionRuntime`

```rust
pub struct SessionRuntime { /* Session state machine + PeerHandle control channel */ }
```

`SessionRuntime` เป็น adapter ระหว่าง `Session` state machine กับ SWSP stream 0 บน `PeerHandle` โดยใช้ protobuf control messages จาก `proto/control.proto` และไม่สร้าง wire schema ใหม่

### Methods

- `new(peer: PeerHandle, config: SessionRuntimeConfig) -> Result<Self>` — ตรวจ configuration และสร้าง runtime state
- `handshake(&mut self) -> Result<()>` — ฝั่ง client เริ่ม `connect`; ฝั่ง server ตรวจ version/path, ทำ PIN auth และทั้งสองฝั่งเปลี่ยนเป็น `Ready` หลังได้รับ control sequence ครบ
- `run_until_disconnect(&mut self) -> Result<()>` — ประมวลผล control frames ต่อหลัง Ready และล้าง session เมื่อได้รับ SWSP `FIN`, receive loop close หรือ peer disconnect
- `open_stream(kind, connect_path) -> Result<StreamEntry>` — ลงทะเบียน stream ผ่าน `StreamRegistry`
- `close_stream(stream_id) -> Result<StreamEntry>` — ปิดและลด active stream count
- `snapshot() -> SessionRuntimeSnapshot` — อ่าน state, stats และจำนวน active streams
- `send_control(message: ControlMessage) -> Result<()>` — ส่ง protobuf control message บน SWSP control stream 0
- `close(&mut self) -> Result<()>` — ส่ง SWSP `FIN`, รอ send buffer แบบ bounded best-effort, ล้าง session และปิด peer

### Runtime guarantees and limits

- ตรวจ duplicate `connect`, duplicate `auth`, duplicate `auth_required`/`auth_result` และ premature/duplicate `ready` ภายใน session scope
- timeout ระหว่าง handshake ปิด local session; wrong-PIN retry exhaustion ปิดทั้ง runtime ที่ตรวจพบ failure
- tests พิสูจน์ authenticated local two-peer E2E, wrong PIN/retry exhaustion, timeout, disconnect cleanup, duplicate control message และ stream cleanup
- local tests ไม่ใช่หลักฐาน original-client/server interoperability, browser compatibility, production NAT traversal หรือ external STUN/TURN availability

---

## 6. Stream API

## 6.1 `StreamHandler`

```rust
#[async_trait::async_trait]
pub trait StreamHandler {
    fn stream_type(&self) -> StreamType;
    async fn handle(&mut self, ctx: StreamContext, frame: Frame) -> Result<StreamResponse>;
}
```

### Purpose
เป็น abstraction กลางสำหรับ stream type ต่าง ๆ

---

## 6.2 Shell Stream

### `ShellStreamHandler`
- รับ input terminal
- ส่ง output terminal
- handle resize event

### Related Types
- `ShellInput`
- `ShellOutput`
- `TerminalResize`

---

## 6.3 File Stream

### `FileStreamHandler`
- list directory
- download
- upload
- stat
- delete

### Related Types
- `FileOp`
- `FileInfo`
- `FileList`
---

## 6.4 TCP Stream

### `TcpStreamHandler`
- open TCP connection
- forward raw bytes bidirectionally

### Related Types
- `TCPOpen`
- `TCPData`

---

## 6.5 WebSocket Stream

### `WebSocketStreamHandler`
- open WebSocket target
- bridge text/binary frames

### Related Types
- `WebSocketOpen`
- `WebSocketMessage`

---

## 6.6 HTTP Stream

### `HttpStreamHandler`
- process HTTP request metadata
- stream request/response body
- rewrite headers
- follow redirect policy

### Related Types
- `HTTPRequest`
- `HTTPResponse`
- `HTTPData`

---

## 6.7 Proxy Security Policy

`src/stream/proxy.rs` เป็น policy boundary กลางสำหรับ TCP, WebSocket และ HTTP handlers ที่จะพัฒนาใน Issue #42 เป็นต้นไป โมดูลนี้ **ไม่เปิด socket และไม่ implement stream handler** แต่ต้องถูกเรียกก่อน connect, retry และ redirect ทุกครั้ง

### Core types and methods

```rust
pub enum ProxyAuthorization {
    None,
    Allowlisted,
    UserConfirmed,
}

pub struct ProxyPolicy { /* deny-by-default configuration */ }

impl ProxyPolicy {
    pub fn deny_by_default() -> Self;
    pub fn validate_target(
        &self,
        target: &url::Url,
        authorization: ProxyAuthorization,
    ) -> Result<ValidatedProxyTarget, ProxyPolicyError>;
    pub async fn resolve_and_validate(
        &self,
        target: &url::Url,
        authorization: ProxyAuthorization,
    ) -> Result<ResolvedProxyTarget, ProxyPolicyError>;
    pub fn validate_retry(
        &self,
        resolved: &ResolvedProxyTarget,
        retry_address: std::net::SocketAddr,
    ) -> Result<(), ProxyPolicyError>;
    pub fn validate_redirect(
        &self,
        original: &ValidatedProxyTarget,
        next: &url::Url,
        authorization: ProxyAuthorization,
        redirect_count: u8,
    ) -> Result<ValidatedProxyTarget, ProxyPolicyError>;
}
```

`ProxyPolicy::deny_by_default()` ไม่ยอมรับ target ที่ไม่มี authorization, ปฏิเสธ credentials/fragment/unsupported scheme และตรวจ private, loopback, link-local, multicast, reserved, documentation และ IPv4-mapped IPv6 ตาม ADR-041 การเปิด local target หรือ user confirmation ต้องตั้งค่าอย่าง explicit และยังต้องผ่าน authorization ทุกครั้ง

`resolve_and_validate` ตรวจ DNS answers ทั้งหมดแล้วคืน address set ที่ pin ไว้ใน `ResolvedProxyTarget`; handler ห้าม resolve ใหม่ระหว่าง retry โดยไม่เรียก `validate_retry` การ follow redirect ต้องส่ง URL ใหม่กลับเข้า `validate_redirect` เพื่อบังคับ same-origin, scheme downgrade และ redirect-count policy อีกครั้ง

### Resource and redaction boundary

`ProxyResourceLimits` กำหนด request/response size, active concurrency, buffered bytes, connect/idle timeout และ redirect ceiling ส่วน `check_request_size`, `check_response_size`, `check_concurrent_streams` และ `check_buffered_bytes` คืน `ProxyPolicyError` แบบ deterministic เมื่อเกิน limit เมื่อ timeout หรือ cancellation เกิดขึ้น handler ต้องยกเลิก pending I/O และปิด stream โดยไม่ bypass policy

`redact_target_for_log` แสดงเพียง `scheme://host:port` และ `redact_header_value` ซ่อน credential-bearing headers เป็น `<redacted>` ห้าม log raw URL ที่มี query token, userinfo, fragment หรือ request/response body `ProxyPolicyError::code()` เป็น stable code สำหรับ metrics และ mapping ไปยัง `BlnkError::Stream`; ไม่ควรส่งข้อความ low-level resolver กลับ remote

### Related types

- `ProxyScheme`
- `ProxyTargetKey`
- `ValidatedProxyTarget`
- `ResolvedProxyTarget`
- `ProxyAuthorization`
- `ProxyResourceLimits`
- `ProxyPolicyError`

---

## 7. Identity API

## 7.1 `Identity`

```rust
pub struct Identity { /* ... */ }
```

### Methods
- `generate() -> Result<Self>`
- `save(&self, path: &Path) -> Result<()>`
- `load(path: &Path) -> Result<Self>`
- `sign(&self, data: &[u8]) -> Result<Vec<u8>>`
- `decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>`

### Responsibilities
- keypair management
- uid, user-visible pairing code และ persistent access_code generation
- persistence
- cryptographic ops

---

## 8. Protocol API

## 8.1 Signaling Messages

- `RegisterRequest`
- `RegisterResponse`
- `ConnectionRequest`
- `OfferMessage`
- `AnswerMessage`
- `ICECandidateMessage`
- `PairRequest`
- `PairAnswer`
- `PairApproved`
- `PairRejected`

## 8.2 SWSP

- `Frame`
- `FrameFlag`
- `ParsedFrame`
- `BuildFrameRequest`
- `ParseFrameResponse`
## 8.3 Pairing

- `PairCommit`
- `PairChallenge`
- `PairReveal`
- `PairCredentials`
- `SASInput`
- `SASResult`

## 8.4 Control

- `ConnectMessage`
- `AuthRequiredMessage`
- `AuthMessage`
- `AuthResultMessage`
- `ReadyMessage`
- `ErrorMessage`
- `SessionConfig`
- `SessionStats`

## 8.5 Stream Types
- `StreamType`
- `StreamContext`
- `StreamHandlerInfo`

---

## 9. Utility API

## 9.1 QR

```rust

pub fn generate_qr(data: &str) -> Result<String>;

```
### Purpose
สร้าง QR code สำหรับ URL หรือ pairing link
---

## 9.2 Logging Setup

```rust
pub fn init_logging(level: &str) -> Result<()>;
```
### Purpose
ตั้งค่า tracing subscriber
---
## 9.3 Error Types
```rust
#[derive(thiserror::Error, Debug)]
pub enum BlnkError { /* ... */ }
```
---

## 10. API Behavior Rules
- ทุก API ที่ล้มเหลวต้อง return `Result`
- message ที่รับจาก network ต้อง validate ก่อนใช้งาน
- protocol decode error ต้องไม่ทำให้ process crash
- raw SWSP framing ต้องยึด 8-byte header contract ใน `specs/spec.md`
- public API ควรมี docs ครบ
- function ที่เกี่ยวกับ crypto ต้องระวัง side effects

---

## 11. Example Usage

```rust
let identity = Identity::generate()?;
let mut signaling = SignalingClient::new("wss://bitba.ng")?;
signaling.connect()?;
let mut peer = PeerConnection::new(identity, signaling)?;
peer.connect()?;
```