README.md

# blnk Rust

>blnk Rust คือการพอร์ต `bitbang-cli` จากภาษา Go มาเป็น Rust โดยรักษาความสามารถหลักให้เทียบเท่าต้นฉบับ และออกแบบใหม่ให้เหมาะกับการพัฒนา, ทดสอบ, และดูแลรักษาในระยะยาว

## วัตถุประสงค์

- พอร์ตฟีเจอร์จาก `bitbang-cli` ให้ครบถ้วน
- ใช้ Rust เพื่อเพิ่ม memory safety และความคุมได้ของระบบ
- ออกแบบสถาปัตยกรรมใหม่ให้ modular และ testable
- รองรับ cross-platform: Linux, Windows, macOS
- เตรียมโครงสร้างสำหรับการพัฒนาต่อในอนาคตอย่างเป็นระบบ

## ภาพรวมระบบ

blnk เป็น CLI tool สำหรับ remote access แบบ peer-to-peer ผ่าน WebRTC โดยไม่ต้องพึ่ง account และไม่ต้องเปิด port forwarding เอง
ความสามารถหลัก:
- remote shell
- file transfer
- HTTP web proxy
- TCP forwarding
- WebSocket bridging
- pairing code
- PIN authentication
- mDNS discovery
- STUN/TURN NAT traversal
- QR code generation
> หมายเหตุ: service worker เป็นฝั่ง browser/frontend อยู่แล้ว ไม่ต้อง implement ใน Rust CLI

## ขอบเขตโปรเจค

โปรเจคนี้จะพัฒนา Rust implementation ให้ทำงานแทนต้นฉบับ Go โดยต้องรักษา protocol compatibility ให้เทียบเท่าเดิม โดยเฉพาะ:
- signaling protocol
- pairing flow
- SWSP protocol
- identity format
- control messages
- stream handling

## เป้าหมายคุณภาพ

- single static binary
- async-first architecture
- error handling ชัดเจน
- cross-platform support
- test coverage สูง
- release automation พร้อม

## สถานะการพัฒนา

[NOTE!] เอกสารนี้เป็นจุดเริ่มต้นของการพัฒนา โปรเจคยังอยู่ในระยะวางโครงสร้าง

## Quick Start

### สร้างโปรเจค

```bash
cargo new blnk-rust
cd blnk-rust
```

### build

```bash
cargo build
```

### run

```bash
cargo run -- serve
cargo run -- connect
cargo run -- cp
```

## CLI Commands

- `serve` - โหมดรับการเชื่อมต่อและให้บริการ
- `connect` - เชื่อมต่อไปยัง device ผ่าน signaling
- `cp` - คัดลอกไฟล์ระหว่างปลายทาง
- `devices` - จัดการรายการอุปกรณ์
- `version` - แสดงเวอร์ชัน

## Dependencies หลัก

- `tokio` - async runtime
- `webrtc` - WebRTC implementation
- `axum` - HTTP server
- `clap` - CLI parsing
- `tracing` - logging
- `thiserror` - error handling
- `serde` - serialization
- `prost` - protobuf support
- `qrcode` - QR code generation
- `mdns` - local discovery
- `nix` / `winapi` - PTY support

## การใช้งานโปรเจคนี้

โปรเจคนี้ถูกออกแบบมาเพื่อ:
- ใช้งานเป็น CLI tool
- เป็นฐานสำหรับพัฒนาแบบ modular
- รองรับการต่อยอดเป็น library ได้ด้วย

## เอกสารเพิ่มเติม

- `spec.md`
- `architecture.md`
- `api.md`
- `style.md`
- `todo.md`

## License
MIT

spec.md

# blnk Rust Specification

## 1. ภาพรวม

blnk Rust เป็น CLI tool สำหรับ remote access แบบ peer-to-peer ผ่าน WebRTC โดยออกแบบให้ทำงานได้โดยไม่ต้องมี account และไม่ต้องทำ port forwarding บนเครื่องปลายทาง
ระบบจะพึ่งพา signaling server เพื่อช่วยเชื่อม browser/client เข้ากับ device แล้วใช้ WebRTC data channel เป็นแกนหลักในการสื่อสาร

---

## 2. เป้าหมายของสเปค

- รักษาความสามารถเทียบเท่าต้นแบบ blnk-cli
- รักษา protocol compatibility กับระบบเดิม
- ออกแบบให้ implement ได้จริงใน Rust
- รองรับ Linux, Windows, macOS
- แยกส่วน core logic ออกจาก CLI และ web integration

---

## 3. Scope

### 3.1 อยู่ในขอบเขต
- CLI commands
- signaling client
- WebRTC peer connection
- identity generation
- pairing flow
- PIN auth
- stream protocol
- shell/file/proxy/tcp/websocket handlers
- mDNS discovery
- QR code generation
- HTTP server integration
- test and release support

### 3.2 อยู่นอกขอบเขต
- การเขียน service worker ใหม่
- การเปลี่ยน protocol หลักที่ต้นฉบับใช้อยู่
- การสร้าง backend ใหม่แทน signaling เดิมโดยสมบูรณ์
- ฟีเจอร์ที่ไม่เกี่ยวกับ operation หลักของ bitbang-cli

---

## 4. Functional Requirements

### 4.1 Interactive Shell
ระบบต้องรองรับ remote shell ผ่าน WebRTC data channel
- ต้องเปิด pseudo-terminal
- ต้องส่ง input/output แบบ streaming
- ต้องรองรับ terminal resize
- ต้องรองรับ Linux/macOS/Windows โดยใช้ implementation ที่เหมาะสมกับ platform

### 4.2 File Transfer
ระบบต้องรองรับ file transfer แบบ browse และ transfer
- list directory
- download file
- upload file
- stat file
- delete file
- handle metadata และ range ได้

### 4.3 Web Proxy
ระบบต้องรองรับ HTTP proxy สำหรับ access web app บนเครือข่ายปลายทาง
- map HTTP request/response ผ่าน stream
- rewrite headers ได้
- handle redirect ได้
- รองรับ streaming body

### 4.4 TCP Forwarding
ระบบต้องรองรับ raw TCP forwarding ผ่าน WebRTC
- open connection ไปยัง host/port ที่ระบุ
- ส่งข้อมูล bidirectionally
- handle close/error ได้ถูกต้อง

### 4.5 Pairing Code
ระบบต้องรองรับ pairing code 6 หลัก
- generate code
- exchange ผ่าน signaling
- ใช้เป็น part ของ authentication flow

### 4.6 PIN Authentication
ระบบต้องรองรับ PIN แบบ optional
- configure ได้
- verify แบบ constant-time
- จำกัดจำนวนครั้งที่ผิดได้
- มี delay ระหว่างการลองใหม่ได้

### 4.7 QR Code
ระบบต้องสามารถสร้าง QR code สำหรับ URL หรือ pairing link
- output เป็น terminal / file / image ได้ตาม implementation
- ใช้เพื่อช่วย pairing

### 4.8 mDNS Discovery
ระบบต้องรองรับ local discovery ผ่าน mDNS
- announce device ใน LAN
- discover service ใน local network

### 4.9 STUN/TURN
ระบบต้องรองรับ NAT traversal
- STUN สำหรับ discovery
- TURN สำหรับ relay fallback
- สามารถกำหนด ICE server ได้

### 4.10 WebSocket Bridging
ระบบต้องรองรับ WebSocket over WebRTC data channel
- connect ไปยัง target
- forward frames แบบ text/binary
- maintain connection lifecycle

---

## 5. Protocol Requirements

### 5.1 Signaling Protocol
- ใช้ WebSocket-based signaling
- message format เป็น JSON/protobuf-compatible schema ตาม design
- รองรับ register, request, offer, answer, candidate, error
- protocol version ต้องตรงกับต้นฉบับ

### 5.2 SWSP Protocol
- ใช้สำหรับ transport บน data channel
- stream id 0 เป็น control
- stream id 1+ เป็น data stream
- frame ต้องรองรับ SYN, FIN, DAT, MORE
- max frame size ต้องไม่เกินข้อจำกัดของ SCTP
- payload ที่ binary ต้องมีการ encode อย่างเหมาะสม

### 5.3 Pairing Protocol
- ใช้ commit-reveal flow
- มี nonce challenge
- มี SAS computation
- มี credential exchange
- ป้องกัน MITM ระดับพื้นฐานตามต้นฉบับ

### 5.4 Identity Protocol
- ใช้ RSA 2048-bit key pair
- identity ต้อง persist ได้
- code และ uid ต้อง generate ได้ตรงตาม format
- ต้องรองรับ sign/decrypt ตามที่ protocol ระบุ

---

## 6. Non-Functional Requirements

### 6.1 Performance
- latency ต่ำ
- throughput เพียงพอสำหรับ shell/file/tcp stream
- memory usage คุมได้
- release binary ควร lean

### 6.2 Security
- ใช้ cryptographic primitives ที่ปลอดภัย
- หลีกเลี่ยง unwrap/expect ใน production
- PIN verification ต้องใช้ constant-time comparison
- key material ต้องไม่ log ออกมา

### 6.3 Reliability
- reconnect/error handling ต้องชัดเจน
- stream failure ต้องไม่ล่มทั้ง session ถ้าไม่จำเป็น
- invalid message ต้องถูก reject อย่างปลอดภัย

### 6.4 Portability
- Linux: PTY ใช้ `nix`
- Windows: PTY ใช้ `winapi`
- macOS: ใช้ implementation ฝั่ง Unix ที่เหมาะสม

---

## 7. Acceptance Criteria

โปรเจคจะถือว่า pass specification เมื่อ:
- `serve`, `connect`, `cp` ทำงานได้
- เชื่อมต่อ signaling ได้จริง
- สร้าง WebRTC peer connection ได้
- ส่งข้อมูลผ่าน data channel ได้
- shell, file, proxy, tcp, websocket ใช้งานได้
- pairing และ PIN auth ใช้ได้
- build บน Linux/Windows/macOS ได้
- มี test ครอบคลุมส่วนสำคัญ

---

## 8. Constraints

- ต้องคง protocol compatibility กับต้นฉบับ
- ต้องใช้ Rust ecosystem ให้มากที่สุด
- service worker ไม่อยู่ใน scope ของ Rust backend
- ต้องเตรียมความพร้อมสำหรับ integration กับ frontend เดิม

architecture.md

# blnk Rust Architecture

## 1. Architecture Overview

blnk Rust ใช้สถาปัตยกรรมแบบ modular async CLI + WebRTC peer-to-peer communication
แนวคิดหลัก:
- CLI เป็น entry point
- signaling เป็นตัวเริ่ม session
- WebRTC เป็น transport layer หลัก
- stream protocol เป็น abstraction สำหรับแต่ละ service
- handler แต่ละประเภทแยกเป็น module ชัดเจน

---

## 2. High-Level Components

### 2.1 CLI Layer
รับคำสั่งจาก user และแปลงเป็น operation
- `serve`
- `connect`
- `cp`
- `devices`
- `version`
หน้าที่:
- parse args
- load config
- call business logic
- handle errors/logging

### 2.2 Config Layer
จัดการ configuration จากไฟล์, env, และ CLI flags
- signaling server
- identity path
- pin config
- relay/STUN/TURN settings
- debug/log settings

### 2.3 Signaling Layer
เชื่อมต่อกับ signaling server ผ่าน WebSocket
- register device
- request connection
- exchange offer/answer/candidate
- handle pairing and error messages

### 2.4 Peer Layer
สร้างและจัดการ WebRTC peer connection
- ICE
- SDP
- DTLS/SCTP
- data channel
- connection state

### 2.5 Session Layer
ดูแล lifecycle ของ session
- auth
- ready state
- stream registry
- session statistics
- cleanup

### 2.6 Stream Layer
จัดการ stream protocol และ dispatch ไปยัง handler
- control stream
- shell stream
- file stream
- tcp stream
- websocket stream
- http stream

### 2.7 Protocol Layer
เก็บ definition ของ message และ frame format
- signaling schema
- SWSP frame
- pairing messages
- identity messages
- control messages

### 2.8 Identity Layer
ดูแล key pair และ identity persistence
- generate key
- load/save identity
- sign/decrypt
- uid/code management

### 2.9 Web/HTTP Layer
ให้บริการ HTTP server และ static assets
- route request
- serve frontend assets
- websocket endpoint
- header rewrite / redirect handling

### 2.10 Utility Layer
- QR code
- logging
- error types
- helpers

---

## 3. Proposed Module Structure

```
src/
├── main.rs
├── lib.rs
├── config/
├── signaling/
├── peer/
├── session/
├── stream/
├── protocol/
├── identity/
├── fileshare/
├── shell/
├── proxy/
├── tcpforward/
├── utils/
└── web/
```

---

## 4. Data Flow

## 4.1 Device Start Flow

1. CLI start `serve`
2. load config
3. load/generate identity
4. connect signaling server
5. register device
6. wait for connection request
7. establish WebRTC peer
8. create data channel
9. start session dispatcher

## 4.2 Browser Connect Flow

1. browser connects signaling server
2. server sends request to device
3. device creates offer
4. browser receives offer
5. browser sends answer
6. ICE candidates exchanged
7. data channel opens
8. control stream starts
9. session becomes ready

## 4.3 Stream Dispatch Flow

1. receive SWSP frame
2. parse stream id and flags
3. route to handler ตาม stream type
4. handler process input/output
5. response frame encode กลับไป

---

## 5. Architectural Principles

### 5.1 Separation of Concerns
แต่ละ module รับผิดชอบเรื่องเดียวให้ชัด

### 5.2 Async First
ทุก network I/O และ stream operation ใช้ async/await

### 5.3 Protocol-Driven Design
protocol เป็น source of truth สำหรับ message shape และ framing

### 5.4 Trait-Based Extensibility
handler แต่ละประเภทควร implement trait ร่วมกัน

### 5.5 Minimal Global State
หลีกเลี่ยง global mutable state ใช้ `Arc`, `Mutex`, `RwLock` อย่างระวัง

---

## 6. Core Runtime Model

### 6.1 Runtime
ใช้ `tokio` เป็น runtime หลัก

### 6.2 Task Model
- network listeners รันเป็น async task
- signaling loop รันแยก task
- data channel processing รันแยก task
- stream handlers รันตาม connection/session

### 6.3 Communication
ใช้ channels เช่น:
- `tokio::sync::mpsc`
- `tokio::sync::oneshot`
- `tokio::sync::broadcast`

---

## 7. Error Handling Architecture

- ใช้ `thiserror` สำหรับ typed error
- ใช้ `anyhow` ใน application layer ที่ต้องการ context
- ทุก boundary ต้อง map error ให้เหมาะสม
- ไม่ใช้ panic เป็น flow ปกติ

---

## 8. Logging Architecture

- ใช้ `tracing`
- ใช้ span ต่อ session/peer/stream
- log ต้องมี context เช่น:
  - peer_id
  - stream_id
  - client_id
  - session_id

---

## 9. Cross-Platform Strategy

### 9.1 Unix
- PTY ใช้ `nix`
- file and socket operations ใช้ standard library + Unix extensions

### 9.2 Windows
- PTY ใช้ `winapi`
- ต้องแยก platform-specific code อย่างชัดเจนด้วย `cfg`

### 9.3 Shared Core
logic หลักของ protocol, signaling, session, stream ควรเป็น shared code

---

## 10. Security Considerations
- key material ต้องเก็บอย่างปลอดภัย
- auth flow ต้องป้องกัน replay พื้นฐาน
- sensitive payload ต้องไม่ log
- input validation ต้องเข้ม
- protocol decoding ต้อง fail safe

---

## 11. Testing Architecture

### 11.1 Unit Tests
- protocol parsing
- frame encoding/decoding
- identity utilities
- auth logic

### 11.2 Integration Tests
- signaling flow
- peer connection flow
- session negotiation
- file/tcp/shell stream behavior

### 11.3 Cross-Platform Tests
- PTY behavior
- path handling
- binary release compatibility

---

## 12. Release Architecture

- build release binary ด้วย `cargo build --release`
- strip symbols สำหรับ production
- ใช้ CI สำหรับทุก platform
- publish artifacts สำหรับ Linux, Windows, macOS


style.md

# blnk Rust Code Style

## 1. หลักการเขียนโค้ด

โค้ดต้อง:
- อ่านง่าย
- แบ่งหน้าที่ชัดเจน
- testable
- cross-platform friendly
- async-safe
- free จาก panic ใน production เท่าที่เป็นไปได้

---

## 2. Naming Conventions

### 2.1 Functions / Variables / Modules
ใช้ `snake_case`
ตัวอย่าง:
- `connect_to_signaling`
- `load_identity`
- `stream_handler`
- `pin_required`

### 2.2 Types / Traits / Structs / Enums
ใช้ `PascalCase`
ตัวอย่าง:
- `PeerConnection`
- `SignalingClient`
- `SessionState`
- `StreamType`

### 2.3 Constants
ใช้ `SCREAMING_SNAKE_CASE`
ตัวอย่าง:
- `MAX_FRAME_SIZE`
- `DEFAULT_PIN_TIMEOUT_MS`
- `SWSP_VERSION`

---

## 3. Documentation Style

### 3.1 Module Documentation
ใช้ `//!` สำหรับอธิบาย module
ตัวอย่าง:

```rust
//! Signaling client for blnk Rust.
```

### 3.2 Item Documentation
ใช้ `///` สำหรับ function, struct, enum, trait
ตัวอย่าง:

```rust
/// Connects to the signaling server and registers the current device.
pub async fn connect() -> Result<()> {
    Ok(())
}
```

---

## 4. Type Derives

ควรใช้ `#[derive(...)]` เมื่อเหมาะสม
แนะนำ:
- `Debug`
- `Clone`
- `PartialEq`
- `Eq`
- `Serialize`
- `Deserialize`
ตัวอย่าง:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeviceId {
    pub uid: String,
}
```

---

## 5. Error Handling Style

### 5.1 ห้ามใช้ใน production
- `unwrap()`
- `expect()` โดยไม่มี context
- `panic!()` เป็น error flow
- silent failure

### 5.2 แนะนำ
- ใช้ `Result<T, E>`
- ใช้ `?`
- ใช้ `thiserror` สำหรับ typed error
- ใช้ `anyhow` สำหรับ application-level context
ตัวอย่าง:

```rust
pub async fn connect() -> Result<SignalingClient, blnkError> {
    let client = SignalingClient::new().await?;
    Ok(client)
}
```

---

## 6. Async Style

### 6.1 ใช้ async/await เป็นหลัก
- ใช้ `async fn`
- ใช้ `tokio`
- แยก task เมื่อจำเป็น
- ใช้ `tokio::select!` เมื่อรอหลาย future

### 6.2 หลักการ
- อย่า block runtime โดยไม่จำเป็น
- งาน I/O ต้อง async
- งาน CPU-heavy ให้พิจารณา `spawn_blocking`

---

## 7. Logging Style

ใช้ `tracing` แทน `println!`
ตัวอย่าง:
```rust
use tracing::{info, debug, error};
info!("session started");
debug!(peer_id = %peer_id, "peer connected");
error!(error = %err, "connection failed");
```

แนวทาง:
- log ต้องมี context
- หลีกเลี่ยงการ log ข้อมูลลับ
- ใช้ span สำหรับ session/peer/stream

---

## 8. Module Organization

### 8.1 Layout
- `mod.rs` ใช้เป็น public entry ของ module
- แยก file ตาม responsibility
- re-export เฉพาะสิ่งที่ใช้ภายนอกจริง

### 8.2 Example

```rust
pub mod connection;
pub mod ice;
pub use connection::PeerConnection;
```

---

## 9. Code Size and Complexity Rules

### 9.1 Function Size
- ฟังก์ชันไม่ควรยาวเกินประมาณ 50 บรรทัดโดยไม่มีเหตุผล

### 9.2 Nesting
- หลีกเลี่ยง nested `if/else` มากเกิน 3 ชั้น
- ใช้ early return
- แยก helper function

### 9.3 Magic Numbers
- ห้าม hardcode ตัวเลขสำคัญโดยไม่มี constant
ตัวอย่าง:

```rust
const MAX_AUTH_FAILS: usize = 3;
```

---

## 10. Concurrency Style

- ใช้ `Arc<T>` เมื่อ share ownership
- ใช้ `Arc<Mutex<T>>` หรือ `Arc<RwLock<T>>` เฉพาะเมื่อจำเป็น
- ใช้ channel เป็นตัวกลางมากกว่าการแชร์ mutable state โดยตรง
- avoid lock ระยะยาว

---

## 11. Platform-Specific Style

- ใช้ `cfg(target_os = "...")` สำหรับ code ที่ต่างกัน
- แยก Unix/Windows ให้ชัด
- อย่าปน platform code กับ shared protocol code

---

## 12. Testing Style

### 12.1 Unit Test
- ทดสอบ behavior ของ function เดี่ยว
- ทดสอบ encoding/decoding
- ทดสอบ auth logic
- ทดสอบ edge cases

### 12.2 Integration Test
- ทดสอบ flow จริงระหว่าง modules
- ใช้ async test
- ตั้งชื่อ test ให้ชัดเจน
ตัวอย่าง:

```rust
#[tokio::test]
async fn test_signaling_connection_success() {
    // ...
}
```

---

## 13. Dependency Style
- ใช้ dependency เท่าที่จำเป็น
- prefer crates ที่ mature และมี maintenance ดี
- ถ้ามี feature ใน std ใช้ std ก่อน
- ถ้ามีหลาย crate ทำหน้าที่คล้ายกัน ให้เลือกตัวที่เหมาะกับ architecture มากที่สุด

---

## 14. Security Style

- validate input ทุก boundary
- อย่า log secret
- ใช้ constant-time comparison สำหรับ PIN และ secret comparison
- แยก crypto logic ออกจาก business logic
- code path สำหรับ sensitive operation ต้อง review ง่าย

---

## 15. Recommended Rust Practices
- ใช้ `Option<T>` สำหรับค่าที่อาจไม่มี
- ใช้ `Result<T, E>` สำหรับ error path
- ใช้ iterator แทน loop ที่ซับซ้อนเมื่อเหมาะสม
- ใช้ `match` เมื่อ logic เป็นแบบหลายกรณี
- ใช้ trait เพื่อ abstract handler ต่าง ๆ

---

## 16. Example Pattern

```rust
/// Represents a stream handler.
pub trait StreamHandler {
    /// Returns the stream type supported by this handler.
    fn stream_type(&self) -> &'static str;
    /// Handles an incoming frame.
    async fn handle_frame(&self, frame: Frame) -> Result<(), blnkError>;
}
```

---

## 17. Final Style Rule

ถ้าโค้ดอ่านแล้วไม่ชัดว่า:
- ใครเป็นเจ้าของ state
- error ไหลไปทางไหน
- async boundary อยู่ตรงไหน
- protocol message นี้ใช้ทำอะไร
แปลว่ายังไม่ผ่าน style ของโปรเจคนี้


api.md

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
- uid/code generation
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
- `ParseFrameResponse
`
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

pub enum blnkError { /* ... */ }

```
---
## 10. API Behavior Rules
- ทุก API ที่ล้มเหลวต้อง return `Result`
- message ที่รับจาก network ต้อง validate ก่อนใช้งาน
- protocol decode error ต้องไม่ทำให้ process crash
- public API ควรมี docs ครบ
- function ที่เกี่ยวกับ crypto ต้องระวัง side effects
---
## 11. Example Usage
```rust

let identity = Identity::generate()?;

let mut signaling = SignalingClient::new("wss://bitba.ng").await?;

signaling.connect().await?;

let mut peer = PeerConnection::new(identity, signaling).await?;

peer.connect().await?;

```

todo.md

# blnk Rust TODO
## Phase 1: Setup & Foundation
- [ ] สร้างโปรเจค Rust ใหม่
- [ ] ตั้งค่า `Cargo.toml`
- [ ] เลือก dependency หลัก
- [ ] สร้างโครงสร้าง `src/`
- [ ] วาง `main.rs` และ `lib.rs`
- [ ] implement CLI skeleton ด้วย `clap`
- [ ] ตั้งค่า `tracing`
- [ ] สร้าง error type กลางด้วย `thiserror`
- [ ] ตั้งค่า config management
### Deliverable
- โปรเจค compile ได้
- มี CLI command base structure
- logging และ error handling ใช้งานได้
---
## Phase 2: Core Networking
- [ ] implement signaling client
- [ ] implement signaling message schema
- [ ] implement websocket connection ไปยัง signaling server
- [ ] implement WebRTC peer connection
- [ ] implement ICE candidate handling
- [ ] implement SDP offer/answer exchange
- [ ] implement data channel
- [ ] integrate STUN/TURN
- [ ] implement mDNS discovery
### Deliverable
- เชื่อมต่อ signaling server ได้
- WebRTC connection ทำงานได้
- data channel ส่งข้อมูลได้
---
## Phase 3: Identity & Authentication
- [ ] implement identity generation
- [ ] implement identity persistence
- [ ] implement uid/code generation
- [ ] implement pairing code flow
- [ ] implement PIN authentication
- [ ] implement session management
- [ ] implement crypto helpers สำหรับ sign/decrypt
### Deliverable
- identity system ใช้งานได้
- pairing code flow ใช้งานได้
- PIN auth ทำงานได้
---
## Phase 4: Stream Handlers
- [ ] define `StreamHandler` trait
- [ ] implement SWSP frame parser
- [ ] implement SWSP frame builder
- [ ] implement shell stream handler
- [ ] implement PTY layer
- [ ] implement file transfer handler
- [ ] implement HTTP proxy handler
- [ ] implement TCP forwarding handler
- [ ] implement WebSocket bridging
- [ ] implement stream dispatcher
### Deliverable
- shell ใช้งานได้
- file transfer ใช้งานได้
- proxy/tcp/websocket ทำงานได้
---
## Phase 5: HTTP & Web Integration
- [ ] implement HTTP server ด้วย `axum`
- [ ] implement static file serving
- [ ] implement WebSocket server
- [ ] implement request routing
- [ ] implement header rewriting
- [ ] implement redirect handling
- [ ] integrate frontend assets
### Deliverable
- HTTP server พร้อมใช้งาน
- web integration พร้อม
---
## Phase 6: File Sharing
- [ ] implement file share config
- [ ] implement file listing
- [ ] implement file download
- [ ] implement file upload
- [ ] implement file metadata
- [ ] implement range handling
### Deliverable
- file sharing ทำงานครบ
---
## Phase 7: CLI Commands
- [ ] implement `serve`
- [ ] implement `connect`
- [ ] implement `cp`
- [ ] implement `devices`
- [ ] add all CLI flags
- [ ] implement help
- [ ] implement version output
### Deliverable
- CLI commands ครบ
- flags ครบ
- UX พร้อมใช้งาน
---
## Phase 8: Testing & Optimization
- [ ] write unit tests
- [ ] write integration tests
- [ ] write protocol tests
- [ ] optimize performance
- [ ] optimize memory usage
- [ ] test cross-platform build
- [ ] perform security audit
- [ ] run load/latency checks
### Deliverable
- test coverage สูง
- performance acceptable
- security reviewed
---
## Phase 9: Documentation & Release
- [ ] write README.md
- [ ] write spec.md
- [ ] write architecture.md
- [ ] write api.md
- [ ] write style.md
- [ ] write usage docs
- [ ] create release binaries
- [ ] setup CI/CD
- [ ] publish repository
- [ ] publish versioned releases
### Deliverable
- documentation ครบ
- release pipeline พร้อม
- binary พร้อมแจกจ่าย
---
## Priority Order
1. signaling client
2. WebRTC peer connection
3. identity generation
4. session management
5. SWSP parser/builder
6. shell handler
7. file transfer
8. HTTP proxy
9. TCP forwarding
10. WebSocket bridging
11. CLI polish
12. tests and release

---

## Definition of Done

ฟีเจอร์หนึ่งจะถือว่าเสร็จเมื่อ:
- มี implementation จริง
- มี unit test ครอบคลุม
- มี integration test ถ้าเกี่ยวข้อง
- ไม่มี panic ใน path ปกติ
- มี docs ประกอบ
- รองรับ platform ที่เกี่ยวข้อง

---

## Immediate Next Steps

- [ ] อ่าน implementation_notes.md ให้ละเอียด
- [ ] ตรวจสอบ protocol compatibility ของต้นฉบับ
- [ ] สร้าง repo Rust ใหม่
- [ ] ตั้งค่า CI
- [ ] ลงมือทำ signaling client ก่อน
- [ ] ทดสอบ webrtc-rs เบื้องต้น
- [ ] ต่อด้วย identity และ session

# blnk
