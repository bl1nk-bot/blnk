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
- `new(url: &str) -> Result<Self>`
- `connect(&mut self) -> Result<()>`
- `register(&self, req: RegisterRequest) -> Result<RegisterResponse>`
- `send_offer(&self, msg: OfferMessage) -> Result<()>`
- `send_answer(&self, msg: AnswerMessage) -> Result<()>`
- `send_candidate(&self, msg: ICECandidateMessage) -> Result<()>`
- `listen(&self) -> Result<()>`

### Responsibilities
- maintain websocket connection
- encode/decode signaling messages
- route messages to session manager

---

## 4. Peer API

## 4.1 `PeerConnection`

```rust
pub struct PeerConnection { /* ... */ }
```

### Methods
- `new(identity: Identity, signaling: SignalingClient) -> Result<Self>`
- `connect(&mut self) -> Result<()>`
- `create_data_channel(&self) -> Result<DataChannel>`
- `add_ice_candidate(&self, candidate: IceCandidate) -> Result<()>`
- `set_remote_description(&self, sdp: String) -> Result<()>`
- `set_local_description(&self, sdp: String) -> Result<()>`

### Responsibilities

- manage WebRTC lifecycle
- handle ICE/SDP exchange
- open data channel
- report state transitions

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