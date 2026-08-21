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

---

## 2. CLI API

## 2.1 `serve`

>เริ่มโหมด server/device เพื่อรอรับ connection

### Responsibilities
- load config
- load or generate the persisted identity
- with `--local-fixture`, start a deterministic local signaling fixture without external secrets
- with `--once`, initialize/print the service state and exit; without it, wait for Ctrl-C
- remote signaling registration, peer/session creation and stream dispatch remain an explicit not-implemented boundary

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
- remote signaling, offer/answer, data-channel negotiation and stream dispatch are not claimed until a provider-compatible orchestration layer exists

```text
blnk connect [--target <DEVICE_ID>] [--local-fixture] [--command <PROGRAM> [ARGS...]] [--pin <PIN>]
```

---

## 2.3 `cp`
คัดลอกไฟล์ระหว่าง local/remote

### Responsibilities
- parse source/destination and `--overwrite`
- in `--local-fixture`, open an authenticated file stream, send `FileOp` plus bounded data chunks, execute the sandboxed receiver service, and consume the response transcript
- `remote:<path>` selects the download direction; a normal local source selects upload
- report a concise completion line without exposing credential or full local path data beyond the requested source/destination
- remote signaling/session orchestration is not claimed by this command yet

```text
blnk cp [--local-fixture] [--overwrite] <SOURCE> <DESTINATION>
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
- `open_file_stream(connect_path) -> Result<StreamEntry>` — เปิด file stream ผ่าน registry หลัง session อยู่ใน `Ready`
- `accept_file_stream(stream_id, connect_path) -> Result<StreamEntry>` — รับ peer-assigned file stream ID พร้อมตรวจ duplicate/non-zero
- `close_stream(stream_id) -> Result<StreamEntry>` — ปิดและลด active stream count
- `send_file_request(stream_id, request) -> Result<()>` — ส่ง `FileOp` เป็น SWSP `SYN|DAT`
- `send_file_chunk(stream_id, data, final_chunk) -> Result<()>` — ส่ง bounded file data เป็น `DAT|MORE` หรือ `DAT|FIN`
- `recv_file_frame() -> Result<Frame>` — รับ raw non-control file frame สำหรับ dispatch/collection
- `close_file_stream(stream_id) -> Result<StreamEntry>` — ส่ง `FIN` และลบ file stream จาก registry
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

### Related Types
- `FileTransferRequest`
- `FileTransferResponse`
- `FileTransferConfig`
- `FileTransferCancellation`
- `FileOp`
- `FileInfo`
- `FileList`

หลักฐานปัจจุบันเป็น local unit/integration tests เท่านั้น ยังไม่ใช่หลักฐาน interoperability กับ client ภายนอกหรือ production filesystem deployment
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
- read response chunks incrementally and reject `response_limit_exceeded` before appending any chunk that would exceed `ProxyResourceLimits::max_response_bytes`; this applies to responses with and without `Content-Length`

### Related Types
- `HTTPRequest`
- `HTTPResponse`
- `HTTPData`

---

## 6.7 Platform Support Matrix

Issue #45 กำหนด platform boundary ของ crate และวิธีอ่านหลักฐานโดยไม่แปลง compile success เป็น runtime หรือ release claim

| Platform | Contract | Evidence level |
|---|---|---|
| Linux | รัน full validation gate: format, all-target check, tests, clippy warnings-as-errors และ diff hygiene | runner-tested เมื่อ workflow job ผ่าน |
| Windows | ใช้ MSVC target สำหรับ `cargo check --all-targets` และ `cargo test --all`; shell fixtures ต้องเลือก executable/arguments ตาม OS | runner-tested เมื่อ Windows job ผ่าน; local Linux ไม่แทนหลักฐานนี้ |
| Android | ใช้ `aarch64-linux-android` เป็น compile-only gate ผ่าน `cargo check --lib --target ...`; ยังไม่อ้าง emulator/device, packaging หรือ runtime | compile-verified เท่านั้น |
| macOS | ไม่อยู่ใน product scope หรือ CI matrix | out-of-scope |

`rust-toolchain.toml` เป็น source of truth ของ channel และ target declarations ส่วน `.github/workflows/ci.yml` เป็น source ของ command/job boundary การเปลี่ยน platform-specific behavior ต้องเพิ่ม test หรือ fixture ที่รันบน platform นั้นได้จริง และต้องระบุข้อจำกัดใน `docs/implementation-status.md` กับ ADR #45

---

## 6.8 Security Boundaries

API ที่เปิด capability ให้ peer ต้องถูกเรียกหลัง authenticated session อยู่ใน state `Ready` และต้องผ่าน policy เฉพาะของ capability นั้นอีกชั้นหนึ่ง การมี `PeerHandle` หรือ data channel ที่เชื่อมสำเร็จไม่ถือเป็น authorization

| Boundary | ค่าเริ่มต้นและ enforcement | ข้อจำกัดของหลักฐาน |
|---|---|---|
| Identity/pairing | RSA 2048, fixed-size nonce, wire `type`, base64 nonce, constant-time commitment/PIN comparison, atomic persistence และ Unix mode `0700`/`0600` | PEM fixture ยืนยัน format-level compatibility; original-Go lifecycle ยังไม่ยืนยัน |
| Signaling egress | `EndpointPolicy::PublicOnly`, ตรวจ DNS answers และ special-use IP ทุกค่า, JSON text frames เท่านั้น, message-size limit และ reconnect policy แบบจำกัด | เป็น application preflight ไม่ใช่ OS firewall; hostname resolution/connect TOCTOU และ provider behavior ยังเป็น residual risk |
| Session/control | stream เปิดได้เมื่อ `Ready`; duplicate/premature control messages, retry exhaustion และ timeout ทำให้ session/peer ปิด | local two-peer/authenticated runtime evidence เท่านั้น |
| File stream | root-relative path, traversal/symlink rejection, size/overwrite policy, timeout และ temporary-file cleanup | ไม่แทน OS ACL และไม่ป้องกัน hostile filesystem race ได้สมบูรณ์ |
| Shell stream | direct argv, program/argument/cwd allowlist, `env_clear`, output/timeout/cancellation limits | ไม่แทน OS sandbox, container, SELinux หรือ AppContainer |
| Proxy stream | deny-by-default, allowlist/explicit confirmation, DNS pinning, retry/redirect guards, bounded incremental response-body reads, resource limits และ log redaction; `wss` handler ปฏิเสธจนกว่ามี tested TLS connector | ไม่ใช่ OS egress firewall; production TLS/provider/NAT evidence ยังขาด |

ความสามารถที่เกี่ยวกับ network, file และ process จึงไม่ควรถูกตีความเป็นสิทธิ์แบบไร้ขอบเขต การ deploy จริงต้องกำหนด OS account, filesystem ACL, firewall/egress policy, TLS trust policy และ resource quotas เพิ่มเติมตาม [ADR-046](decisions/issue-46-threat-model.md)

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