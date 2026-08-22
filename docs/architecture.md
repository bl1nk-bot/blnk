# blnk Rust Architecture

## 1. Architecture Overview

blnk Rust ใช้สถาปัตยกรรมแบบ modular async CLI + WebRTC peer-to-peer communication
แนวคิดหลัก:
- CLI เป็น entry point
- signaling เป็นตัวเริ่ม session
- WebRTC เป็น transport layer หลัก
- stream protocol เป็น abstraction สำหรับแต่ละ service
- handler แต่ละประเภทแยกเป็น module ชัดเจน

### 1.1 Architecture Status and Source of Truth

เอกสารนี้เป็น **canonical architecture** สำหรับ module boundary, runtime model และ data flow ของ Rust implementation ส่วนสถานะว่า module ใด implement แล้วให้ยึด [`docs/implementation-status.md`](implementation-status.md) ไม่ใช่จากการที่ชื่อ module ปรากฏอยู่ในเอกสารหรือ schema

เมื่อเอกสารขัดแย้งกัน ให้ใช้ลำดับ `docs/architecture.md` > `specs/spec.md` > `docs/api.md` > `STYLE.md` > `README.md` > `TODO.md` ตามที่ระบุใน foundation plan โดยการเปลี่ยน protocol semantics ต้องมี decision record และ compatibility test รองรับ

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

### 2.3.1 mDNS Discovery
ทำ local discovery แบบ LAN โดยไม่พึ่ง signaling server เสมอไป
- announce service ผ่าน mDNS
- discover peer/device ในเครือข่ายเดียวกัน
- fallback ไป signaling server เมื่อ mDNS ไม่พอใช้

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
blnk-rust/
├── Cargo.toml                 # Main manifest
├── Cargo.lock
├── README.md
├── TODO.md
├── STYLE.md
├── src/
│   ├── main.rs                # Entry point
│   ├── lib.rs                 # Library root
│   ├── config/                # Configuration
│   │   ├── mod.rs
│   │   └── args.rs            # CLI argument parsing
│   ├── signaling/             # Signaling client
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── messages.rs        # Signaling messages
│   ├── peer/                  # WebRTC peer connection
│   │   ├── mod.rs
│   │   ├── connection.rs
│   │   └── ice.rs
│   ├── session/               # Session management
│   │   ├── mod.rs
│   │   ├── session.rs
│   │   └── auth.rs            # PIN authentication
│   ├── stream/                # Stream handlers
│   │   ├── mod.rs
│   │   ├── handler.rs         # Base handler trait
│   │   ├── shell.rs           # Shell stream handler
│   │   ├── file.rs            # File transfer handler
│   │   ├── proxy.rs           # Shared proxy security policy
│   │   └── proxy_handler.rs   # TCP/WebSocket/HTTP concrete service
│   ├── protocol/              # Protocol definitions
│   │   ├── mod.rs
│   │   ├── swsp.rs            # SWSP protocol
│   │   └── pairing.rs          # Pairing protocol
│   ├── identity/              # Identity management
│   │   ├── mod.rs
│   │   └── key.rs
│   ├── utils/                 # Utilities
│   │   ├── mod.rs
│   │   ├── qr.rs              # QR code generation
│   │   ├── logging.rs         # Logging setup
│   │   └── error.rs           # Error types
│   └── web/                   # Web frontend assets (optional)
│       └── static/            # Static files
├── tests/                     # Integration tests
├── benchmarks/                # Performance benchmarks
├── proto/                     
└── docs/                      # Documentation
    ├── architecture.md
	└── api.md
```

---

## 4. Data Flow

### 4.0 mDNS Discovery Flow

1. announce service บน LAN ผ่าน mDNS
2. discover peer ที่มี service ที่คล้ายกัน
3. กรอง/validate candidate
4. ถ้าไม่เจอ => fallback ไป signaling server

### 4.1 Device Start Flow

1. CLI start `serve`
2. load config
3. load/generate identity
4. connect signaling server
5. register device
6. wait for connection request
7. establish WebRTC peer
8. create data channel
9. start session dispatcher

### 4.1.1 Remote CLI Connect/Copy Flow

1. CLI resolve `--target` through metadata-only `DeviceRegistry` and reject unknown target before dial
2. connect to the configured signaling endpoint using `EndpointPolicy::PublicOnly`
3. send `ConnectionRequest` with the client identity UID as `client_id`
4. receive the target device `OfferMessage`; validate matching client ID and device public-key metadata
5. create the non-trickle WebRTC answer and send `AnswerMessage` with an RSA-OAEP encrypted `{fingerprint, nonce, code}` request
6. device decrypts and validates the opaque request, then both peers wait for the data channel
7. run the authenticated PIN `SessionRuntime` handshake until `Ready`
8. client opens shell/file stream; device owns one raw-frame reader and dispatches to the capability-specific handler
9. close stream and runtime on completion, timeout, disconnect, protocol error or cancellation

The production remote path uses the same module boundaries as the deterministic relay fixture. The fixture proves local behavior only; it does not prove external signaling-provider, original-Go, browser, TLS, STUN/TURN or NAT-traversal interoperability.

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

For the current CLI remote path, the server dispatcher is single-reader: it first validates stream flags and ownership, then routes shell and file openers to their existing bounded services. A malformed opener, unknown stream or invalid terminal frame is a protocol error and triggers session cleanup rather than a success response.


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
- publish artifacts สำหรับ Linux, Windows
