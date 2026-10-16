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

เอกสารนี้เป็น **API contract ที่แยกสถานะ implementation กับ target integration** อย่างชัดเจน ฟังก์ชันหรือ type ใดจะถือว่าใช้งานได้ก็ต่อเมื่อมี Rust implementation, error handling และ tests รองรับ ส่วน interoperability หรือ remote deployment ต้องมี evidence เพิ่มเติม สถานะล่าสุดให้ดู [`docs/implementation-status.md`](implementation-status.md)

> หลักการอ่าน: ฟังก์ชันหรือ type ที่อธิบายในเอกสารนี้ ไม่ได้หมายความว่าผ่านการ implement เสมอไป ต้องตรวจสถานะจริงใน `docs/implementation-status.md` เพื่อยืนยันก่อนอ้างถึงใน PR หรือ issue

---

## 2. CLI API

## 2.1 `serve`

>เริ่มโหมด server/device เพื่อรอรับ connection

### Responsibilities
- load config
- load or generate the persisted identity
- with `--local-fixture`, start a deterministic local signaling fixture without external secrets
- with `--once`, initialize the service and process one authenticated remote session before exiting; without it, run the session dispatcher until Ctrl-C or disconnect
- remote mode registers the identity, accepts a signaling request, negotiates non-trickle SDP and dispatches authenticated shell/file streams

### Implemented CLI surface

```text
blnk serve [--signaling-url <URL>] [--local-fixture] [--once] [--pin <PIN>]
```

`serve` never prints private key material, pairing/access credentials, or raw registry contents. The local fixture reports only its endpoint and whether a PIN is configured.

### Example

```rust
run_serve(args).await?;
```

---

## 2.2 `connect`

เชื่อมต่อไปยัง device ผ่าน signaling หรือผ่าน local fixture

### Responsibilities
- `--local-fixture` creates a real in-process `TwoPeerHarness`, performs the authenticated `SessionRuntime` handshake, opens a shell stream, executes direct argv, and prints framed output/exit status
- `--target <id>` resolves metadata from `DeviceRegistry`; unknown devices are rejected without dialing
- remote mode sends a target-routed `ConnectionRequest`, accepts the device offer, sends a non-trickle SDP answer, performs the authenticated `SessionRuntime` handshake and dispatches the requested shell stream

```text
blnk connect [--target <DEVICE_ID>] [--local-fixture] [--command <PROGRAM> [ARGS...]] [--pin <PIN>]
```

---

## 2.3 `cp`
คัดลอกไฟล์ระหว่าง local/remote

### Responsibilities
- parse target, source/destination, `--pin` and `--overwrite`
- in `--local-fixture`, open an authenticated file stream, send `FileOp` plus bounded data chunks, execute the sandboxed receiver service, and consume the response transcript
- in remote mode, `--target` resolves the device endpoint and uses the same authenticated signaling/WebRTC session before dispatching the file stream
- `remote:<path>` selects the download direction; a normal local source selects upload
- report a concise completion line without exposing credential or full local path data beyond the requested source/destination

```text
blnk cp [--target <DEVICE_ID>] [--local-fixture] [--pin <PIN>] [--overwrite] <SOURCE> <DESTINATION>
```

---

## 2.4 `devices`
จัดการ metadata-only device registry

### Responsibilities
- `--list` (and the default when entries exist) prints ID, endpoint and last-seen timestamp
- persist only device metadata; private key, pairing code and access code are never printed or stored in the registry
- unknown devices are rejected by `connect --target`
- removal/status/identity resolution UI is outside this issue

```text
blnk devices [--list]
```

---

## 3. Signaling API

## 3.1 `SignalingClient`

```rust
pub struct SignalingClient { /* ... */ }
```

### Methods
- `new(url: &str) -> Result<Self>` — ใช้ `EndpointPolicy::PublicOnly` เป็นค่าเริ่มต้นและทำ preflight DNS/IP validation ก่อน dial
- `with_endpoint_policy(policy: EndpointPolicy) -> Self` — ใช้ `EndpointPolicy::AllowLocal` เฉพาะ local fixture/test ที่ควบคุมได้
- `connect(&self) -> Result<SignalingConnection>`
- `connect_with_retry(&self) -> Result<SignalingConnection>` — จำกัด retry เฉพาะ socket เปิดใหม่
- `transact(&self, request: &SignalingMessage) -> Result<SignalingMessage>`
- `transact_with_retry(&self, request: &SignalingMessage) -> Result<SignalingMessage>`
- `SignalingConnection::send`/`recv`/`close` — ส่ง JSON text frame ที่ validate และจำกัดขนาด

### Responsibilities
- maintain websocket connection
- preflight signaling endpoint policy before initial and retry connections
- encode/decode signaling messages
- route messages to the orchestration state machine; `orchestration` เชื่อม signaling กับ `PeerHandle` และ authenticated `SessionRuntime`

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
pub struct Session {
    state: SessionState,
    config: SessionConfig,
    expected_pin: Option<String>,
    auth_attempts: u32,
    registry: StreamRegistry,
    stats: SessionStats,
    next_auth_allowed_at: Option<Instant>,
}
```

### Methods ที่มีจริง
- `new(config: SessionConfig, expected_pin: Option<String>) -> Result<Self>` — ตรวจ `pin_required` และความยาว `expected_pin` (6 ตัวอักษร)
- `state() -> SessionState` — อ่านสถานะปัจจุบัน
- `auth_attempts() -> u32` — จำนวนครั้งที่ลอง auth
- `stats() -> SessionStats` — อ่าน `bytes_received`, `bytes_sent`, `frames_received`, `frames_sent`
- `mark_authenticated(&mut self) -> Result<()>` — เปลี่ยนสถานะเป็น `Authenticated`
- `mark_ready(&mut self) -> Result<()>` — เปลี่ยนเป็น `Ready`
- `register_stream(&mut self, kind: StreamKind, connect_path: ...) -> Result<StreamEntry>` — ลงทะเบียน stream ใหม่
- `close_stream(&mut self, stream_id: u32) -> Result<StreamEntry>` — ปิด stream
- `verify_pin(&mut self, pin: &str) -> SessionResult<()>` — ตรวจ PIN แบบ constant-time พร้อม retry limit และ delay
- `active_streams(&self) -> usize` — จำนวน stream ที่เปิดอยู่
- `closed_streams(&self) -> u64` — จำนวน stream ที่ปิดสะสม

### Responsibilities
- state machine ของ session (`Connecting` → `Authenticating` → `Ready` → `Closed`)
- auth flow พร้อม constant-time PIN check
- stream registry (เก็บ `StreamKind`, `connect_path`)
- stats tracking
- teardown/cleanup

### `SessionConfig` (จริงใน source)
- `pin_required: bool` (default `true`)
- `max_auth_fails: u32` (default `3`)
- `pin_fail_delay: Duration` (default `2_000ms`)
- `validate() -> SessionResult<()>` — ตรวจ `max_auth_fails > 0` เมื่อ `pin_required`

### `SessionState` (enum)
`Connecting`, `Authenticating`, `Ready`, `Closed`

### `AuthOutcome` (enum)
`Ready`, `Rejected { attempts_remaining: u32 }`, `Closed`

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
- `open_file_stream(connect_path) -> Result<StreamEntry>` — เปิด file stream ผ่าน registry หลัง session อยู่ใน `Ready`
- `accept_file_stream(stream_id, connect_path) -> Result<StreamEntry>` — รับ peer-assigned file stream ID พร้อมตรวจ duplicate/non-zero
- `close_stream(stream_id) -> Result<StreamEntry>` — ปิดและลด active stream count
- `send_file_request(stream_id, request) -> Result<()>` — ส่ง `FileOp` เป็น SWSP `SYN|DAT`
- `send_file_chunk(stream_id, data, final_chunk) -> Result<()>` — ส่ง bounded file data เป็น `DAT|MORE` หรือ `DAT|FIN`
- `recv_file_frame() -> Result<Frame>` — รับ raw non-control file frame สำหรับ dispatch/collection
- `close_file_stream(stream_id) -> Result<StreamEntry>` — ส่ง `FIN` และลบ file stream จาก registry
- `snapshot() -> SessionRuntimeSnapshot` — อ่าน state, stats และจำนวน active streams
- `send_control(message: ControlMessage) -> Result<()>` — ส่ง protobuf control message บน SWSP control stream 0
- `close(&mut self) -> Result<()>` — ส่ง SWSP `FIN` เมื่อยังทำได้, รอ send buffer แบบ bounded best-effort, ล้าง session และปิด peer; หาก remote ปิด data channel ก่อนจนการส่ง `FIN` คืน `send SWSP frame: data channel closed` ให้ถือเป็นการปิดแบบ idempotent และส่งต่อ error อื่นตามปกติ

### Runtime guarantees and limits

- ตรวจ duplicate `connect`, duplicate `auth`, duplicate `auth_required`/`auth_result` และ premature/duplicate `ready` ภายใน session scope
- timeout ระหว่าง handshake ปิด local session; wrong-PIN retry exhaustion ปิดทั้ง runtime ที่ตรวจพบ failure
- หลัง authenticated handshake การปิดฝั่ง remote ก่อนต้องไม่ทำให้ `close()` ของฝั่งที่สองล้มเหลวเพราะ data-channel close ระหว่างส่ง `FIN`; การปิดแบบนี้เป็น local lifecycle guarantee ไม่ใช่หลักฐาน interoperability กับ peer ภายนอก
- tests พิสูจน์ authenticated local two-peer E2E, wrong PIN/retry exhaustion, timeout, disconnect cleanup, duplicate control message, stream cleanup และ remote-first close handling
- local tests ไม่ใช่หลักฐาน original-client/server interoperability, browser compatibility, production NAT traversal หรือ external STUN/TURN availability

---

## 6. Stream API

## 6.1 Stream abstraction

> โปรดทราบ: ปัจจุบัน `src/stream/` **ไม่มี** public trait `StreamHandler` หรือ type `StreamContext`/`StreamResponse` ใน codebase โมดูลนี้เป็น concrete handler (`ShellStreamHandler`, `FileTransferService`, `ProxyStreamService`) พร้อม `StreamKind`, `StreamEntry` และ `StreamRegistry` เป็น boundary สำหรับลงทะเบียน stream

### `StreamKind` (จริงใน `src/stream/mod.rs`)
- variants: `Http`, `File`, `Tcp`, `WebSocket`, `Shell`
- `as_str() -> &'static str` คืน string label สำหรับ log/proto

### `StreamRegistry` (จริงใน `src/stream/mod.rs`)
- `new()`, `len()`, `is_empty()`, `contains(stream_id)`, `get(stream_id)`, `closed_count()`
- `open(kind, connect_path) -> Result<StreamEntry>` ลงทะเบียน stream ใหม่ (stream id เริ่มที่ 1)
- `close(stream_id) -> Result<StreamEntry>` ปิด stream และนับ closed_count

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

### `FileTransferService`
`FileTransferService` เป็น concrete sandboxed filesystem handler ของ Issue #39 โดยรับ `FileTransferRequest` และคืน `FileTransferResponse` ผ่าน schema ใน `proto/stream.proto` เดิม รองรับ `GET`, `PUT`, `LIST`, `STAT` และ `DELETE`

### Contract และ policy

- `FileTransferConfig::new(root)` canonicalize sandbox root และ reject root ที่ไม่ใช่ directory
- path ต้องเป็น relative path; absolute path, traversal และ symlink path ถูกปฏิเสธ
- `GET` รองรับ inclusive byte range `[start, end]` และจำกัดขนาดไฟล์ตาม config
- `PUT` เขียนผ่าน temporary file แล้ว rename เป็นปลายทาง พร้อมตรวจขนาดและ overwrite policy
- `LIST` จำกัดจำนวน entries และรองรับ `.` เพื่อแสดง sandbox root
- ทุก operation รองรับ bounded timeout และ cooperative cancellation
- file stream ใช้ non-zero stream ID ผ่าน `SessionRuntime`; request ใช้ `SYN|DAT`, data ใช้ `DAT|MORE`/`DAT|FIN`, และ close ใช้ `FIN`

### Related Types (จริงใน source)
- `FileOperation` (enum: `Get`, `Put`, `List`, `Stat`, `Delete`) — ไม่ใช่ `FileOp` ที่เป็น proto type ใน `proto_generated::stream::FileOpType`
- `FileTransferRequest` — มี `operation: FileOperation`, `path`, `size`, `overwrite`, `range: Option<(u64, u64)>`
- `FileTransferResponse` — concrete response
- `FileTransferConfig` — config (root path, max file size, max list entries, operation timeout)
- `FileTransferCancellation` — `CancellationToken` wrapper
- `DEFAULT_MAX_FILE_SIZE = 64 * 1024 * 1024` (64 MiB)
- `DEFAULT_MAX_LIST_ENTRIES = 10_000`
- `DEFAULT_OPERATION_TIMEOUT = 10s`

หลักฐานปัจจุบันเป็น local unit/integration tests เท่านั้น ยังไม่ใช่หลักฐาน interoperability กับ client ภายนอกหรือ production filesystem deployment
---

## 6.4 TCP Stream

Issue #42 เพิ่ม concrete service ใน `src/stream/proxy_handler.rs`; ยังไม่ผูก dispatch เข้า `SessionRuntime` โดยตรง

```rust
pub struct ProxyStreamService { /* policy + bounded concurrency */ }

impl ProxyStreamService {
    pub fn new(policy: ProxyPolicy) -> Result<Self>;
    pub async fn open_tcp(
        &self,
        stream_id: u32,
        open: proto::stream::TcpOpen,
        authorization: ProxyAuthorization,
    ) -> Result<TcpProxyStream>;
}

impl TcpProxyStream {
    pub async fn send(&mut self, data: &[u8]) -> Result<()>;
    pub async fn recv(&mut self, max_bytes: usize) -> Result<Vec<u8>>;
    pub async fn close(self) -> Result<()>;
}
```

> หมายเหตุ: ชื่อ proto type ตาม convention ของ `prost-build` คือ `TcpOpen` และ `TcpData` (proto ใช้ `TCPOpen`/`TCPData`)

`open_tcp` ตรวจ target ด้วย `ProxyPolicy`, pin DNS answer set ก่อน dial/retry, ใช้ connect/idle/resource limits และคืน stable policy/transport errors เมื่อปฏิเสธหรือ timeout

### Related Types
- `TcpOpen`, `TcpData` (proto-generated)
- `TcpProxyStream`
- `ProxyStreamService`

---

## 6.5 WebSocket Stream

```rust
impl ProxyStreamService {
    pub async fn open_websocket(
        &self,
        stream_id: u32,
        open: proto::stream::WebSocketOpen,
        authorization: ProxyAuthorization,
    ) -> Result<WebSocketProxyStream>;
}

impl WebSocketProxyStream {
    pub async fn send(&mut self, data: &[u8]) -> Result<()>;
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>>;
    pub async fn close(&mut self) -> Result<()>;
}

`ws` ใช้ policy-approved pinned TCP socket และ bridge text/binary payload เป็น bytes; `wss` ถูกปฏิเสธอย่าง explicit จนกว่าจะมี TLS connector ที่มี DNS pinning และ cross-platform certificate evidence ไม่ใช่การ fallback เป็น plain TCP

> หมายเหตุ: proto type คือ `WebSocketOpen` และ `WebSocketMessage` (`prost-build` CamelCase)

### Related Types
- `WebSocketOpen`, `WebSocketMessage` (proto-generated)
- `WebSocketProxyStream`
- `ProxyStreamService`

---

## 6.6 HTTP Stream

```rust
impl ProxyStreamService {
    pub async fn request_http(
        &self,
        base_target: url::Url,
        request: proto::stream::HttpRequest,
        body: Vec<u8>,
        authorization: ProxyAuthorization,
    ) -> Result<HttpProxyResponse>;
}

pub struct HttpProxyResponse {
    pub response: proto::stream::HttpResponse,
    pub body: Vec<u8>,
    pub redirects_followed: u8,
}
```

HTTP ใช้ one-request transcript ใน Issue #42 ไม่ใช่ full-duplex body stream ปิด automatic redirects และ validate ทุก redirect ด้วย `ProxyPolicy`; hop-by-hop headers และ caller-supplied `content-length` ถูกตัดออก และ response body/headers อยู่ภายใต้ limits/redaction boundary

> หมายเหตุ: proto type คือ `HttpRequest`, `HttpResponse`, `HttpData` (proto ใช้ `HTTPRequest`/`HTTPResponse`/`HTTPData`)

### Related Types
- `HttpRequest`, `HttpResponse`, `HttpData` (proto-generated)
- `HttpProxyResponse`
- `ProxyStreamService`

---

## 6.7 Proxy Security Policy

`src/stream/proxy.rs` เป็น policy boundary กลางที่ `ProxyStreamService` ใน `src/stream/proxy_handler.rs` เรียกใช้สำหรับ TCP, WebSocket และ HTTP โมดูล policy **ไม่เปิด socket เอง** แต่ถูกบังคับใช้ก่อน connect, retry และ redirect ทุกครั้ง ส่วนการ dispatch จาก authenticated `SessionRuntime` ยังเป็นงานถัดไป

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

- `ConnectMessage` (path, version)
- `AuthRequiredMessage` (ส่งโดย server เพื่อขอ PIN)
- `AuthMessage` (pin)
- `AuthResultMessage` (success: bool)
- `ReadyMessage` (server_version, capabilities, routing: `"target-prefix"|"direct"`)
- `ControlErrorMessage` (message) — message_type เป็น `"error"`
- `SessionConfig`, `SessionStats`, `SessionState`, `AuthOutcome`

## 8.5 Stream Types
- `StreamKind` (enum) — variants: `Http`, `File`, `Tcp`, `WebSocket`, `Shell` พร้อม `as_str() -> &'static str`
- `StreamEntry` — `stream_id`, `kind: StreamKind`, `connect_path` พร้อม getter `id()`, `kind()`, `connect_path()`
- `StreamRegistry` — `new()`, `len()`, `is_empty()`, `contains(stream_id)`, `get(stream_id)`, `closed_count()`, `open(kind, path)`, `close(stream_id)`

> หมายเหตุ: ไม่มี public type ชื่อ `StreamType`, `StreamContext`, `StreamHandlerInfo` หรือ `StreamResponse` ใน source code ปัจจุบัน

## 8.6 Compatibility Baseline

Issue #44 กำหนด compatibility evidence boundary ผ่านไฟล์ versioned ใต้ `tests/fixtures/compatibility/v1/` และ integration test `tests/compatibility_baseline.rs` โดยไม่สร้าง wire schema ใหม่

| Boundary | Contract ที่ test ตรวจ | สถานะหลักฐาน |
|---|---|---|
| Signaling register | JSON `type`, field names, protocol value และ typed encode/decode | fixture-only + local round-trip |
| SWSP open frame | 8-byte header, little-endian fields, flags, payload และ consumed length | fixture-only + local round-trip |
| Control connect | protobuf field numbers/values และ decode เป็น typed `ControlMessage` | fixture-only + local decode |
| Authenticated session | ลำดับ connect/auth/ready, state `Ready` และ clean close ผ่าน `TwoPeerHarness` | proven-local |

ชื่อโฟลเดอร์ `v1` คือ fixture-format version ไม่ใช่ protocol version ค่า protocol ใน fixture ต้องตรงกับ `PROTOCOL_VERSION` ปัจจุบัน การไม่มี original-Go executable/capture ทำให้สถานะ original-client interoperability เป็น `not-tested` และห้ามเรียก local fixture ว่า production-compatible

การเพิ่ม feature ที่เปลี่ยน directionality, ordering, close/error semantics หรือ version negotiation ต้องเพิ่ม fixture และ test ที่มี provenance ใหม่ โดยห้ามแก้ expected bytes เดิมเพียงเพื่อให้ test ผ่าน

---

## 9. Utility API

## 9.1 QR

```rust
pub fn render_qr_terminal(content: &str) -> Result<String, qrcode::types::QrError>;
pub fn render_qr_ascii(content: &str) -> Result<String, qrcode::types::QrError>;
```
### Purpose
สร้าง QR code สำหรับ URL หรือ pairing link ใช้ใน `blnk serve --qr` (พิมพ์ QR สำหรับ `PairingQrPayload { uid, pairing_code, endpoint }`)

### `PairingQrPayload`
- `uid: String`
- `pairing_code: String`
- `endpoint: String`
- `to_payload_string(&self) -> String` — serialize เป็นข้อความที่นำไป render เป็น QR ได้

> หมายเหตุ: ไม่มี public function ชื่อ `generate_qr` ใน source ใช้ `render_qr_terminal` หรือ `render_qr_ascii` แทน
---
## 9.2 Logging Setup

ในปัจจุบัน binary ใช้ `tracing_subscriber::fmt()` กับ `EnvFilter::from_default_env()` โดยตรงใน `src/main.rs` ไม่มีฟังก์ชัน `init_logging` แยก

```rust
tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .try_init();
```

ผู้ใช้ปรับ log level ผ่าน env var `RUST_LOG` (เช่น `RUST_LOG=blnk=debug`)

---
## 9.3 Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum BlnkError {
    Config(String),
    Identity(String),
    Signaling(String),
    Peer(String),
    Session(String),
    Stream(String),
    Protocol(String),
    Io(std::io::Error),
}
```

มี 8 variants ตามที่ระบุใน `src/utils/error.rs`
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

ตัวอย่างต่อไปนี้แสดงการใช้ API หลักระดับ public ในสถานการณ์ local fixture (ใช้ `TwoPeerHarness`) ส่วน remote flow แบบ signaling/WebRTC เต็มรูปแบบให้ดูที่ `signaling/orchestration.rs` และ `main.rs`

```rust
use blnk::peer::TwoPeerHarness;
use blnk::session::{SessionRuntime, SessionRuntimeConfig};

// สร้าง identity (หรือโหลดจากไฟล์)
let identity = blnk::identity::Identity::generate()?;

// สร้าง local harness แบบ in-process
let harness = TwoPeerHarness::new("control").await?;
let mut client = SessionRuntime::new(
    harness.offerer,
    SessionRuntimeConfig::client(pin),
)?;
let mut server = SessionRuntime::new(
    harness.answerer,
    SessionRuntimeConfig::server(pin),
)?;

// รัน authenticated handshake เป็น Ready
let (c, s) = tokio::join!(client.handshake(), server.handshake());
c?; s?;

// ส่ง SWSP frame ระหว่าง peer
let frame = blnk::protocol::swsp::Frame::new(
    1,
    blnk::protocol::swsp::FrameFlags::SYN | blnk::protocol::swsp::FrameFlags::DAT,
    b"hello".to_vec(),
);
harness.offerer.send_frame(&frame).await?;
let received = harness.answerer.recv_frame().await?;
```

### หมายเหตุสำคัญ

- ไม่มี public type ชื่อ `PeerConnection` ใน `blnk::peer` โมดูลนี้ export เพียง `PeerHandle` และ `TwoPeerHarness`
- ตัวอย่าง remote signaling flow (เช่น `connect_target`, `accept_server_session`, `run_shell_client`) อยู่ใน `blnk::signaling::orchestration` และถูกเรียกจาก `main.rs` ไม่ใช่ public API ที่ผู้ใช้เรียกเอง
- ฟังก์ชันที่อธิบายในเอกสารนี้บางส่วนอยู่ในชั้น internal/test harness เท่านั้น โปรดตรวจ `pub` visibility ใน source ก่อนเรียกใช้จาก crate ภายนอก
