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
- `web`
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

โมดูลจริงใน source แบ่งเป็น `src/signaling/transport.rs` (WebSocket client + local fixture) และ `src/signaling/orchestration.rs` (ผูก signaling เข้ากับ `PeerHandle` และ `SessionRuntime`)

### 2.3.1 Discovery Layer (mDNS)
ทำ local discovery แบบ LAN โดยไม่พึ่ง signaling server เสมอไป
- ใช้ UDP multicast บน service type `_blnk._tcp.local` ผ่าน `src/discovery/mod.rs`
- `MdnsResponder` ประกาศ service ของตัวเอง และ `discover_local_peers` ค้นหา peer ภายใน timeout ที่กำหนด
- ใช้ `CancellationToken` ควบคุม lifecycle ของ background task
- ใช้ร่วมกับ `blnk devices --local` ใน CLI

### 2.4 Peer Layer
- ใช้ `webrtc` crate (v0.20) บน `tokio` runtime
- lifecycle: ICE, SDP, DTLS/SCTP, data channel
- โมดูล `src/peer/mod.rs` มีเพียง `PeerHandle` (public type) และ `TwoPeerHarness` (test/integration helper) ไม่มี `connection.rs` หรือ `ice.rs` แยก

### 2.5 Session Layer
- โมดูล `src/session/` แบ่งเป็น `mod.rs` (typed messages และ state) และ `runtime.rs` (`SessionRuntime`, `SessionRuntimeConfig`, `SessionRuntimeSnapshot`)
- ไม่มี `session.rs` หรือ `auth.rs` แยกเป็นไฟล์

### 2.6 Stream Layer
จัดการ stream protocol และ dispatch ไปยัง handler
- control stream (stream id 0)
- shell stream (`src/stream/shell.rs`)
- file stream (`src/stream/file.rs`)
- proxy security policy (`src/stream/proxy.rs`)
- concrete proxy service (`src/stream/proxy_handler.rs`) — TCP/WebSocket/HTTP

ปัจจุบัน `ProxyStreamService` ยังไม่ผูก dispatch เข้า `SessionRuntime` โดยตรง การ wire TCP/WebSocket/HTTP เข้ากับ stream registry เป็นงานถัดไป (Issue #42)

### 2.7 Protocol Layer
เก็บ definition ของ message และ frame format
- `src/protocol/mod.rs` — module root
- `src/protocol/swsp.rs` — SWSP frame codec
- `src/protocol/pairing.rs` — pairing messages
- `src/proto_generated.rs` — generated protobuf จาก `proto/*.proto` (ใช้สำหรับ control/stream messages เท่านั้น signaling ใช้ typed JSON)

### 2.8 Identity Layer
- `src/identity/mod.rs` — `Identity` struct, derived values (`uid`, `pairing_code`, `access_code`)
- `src/identity/key.rs` — RSA-2048 key pair, sign/decrypt (RSA-OAEP)
- รองรับทั้ง JSON และ PEM persistence

### 2.9 Storage Layer
- `src/storage/mod.rs` — `SqliteStore`, `SCHEMA_VERSION = 1`
- `src/storage/schema.rs` — schema constants
- `src/storage/store.rs` — concrete implementation
- ใช้สำหรับ device registry และ metadata persistence

### 2.10 Web/HTTP Layer
ให้บริการ **loopback-only browser control surface** สำหรับ lifecycle/status ของ local browser session
- bind เฉพาะ loopback address
- ตรวจ exact configured Origin และ CORS แบบไม่ใช้ wildcard
- bootstrap ด้วย bearer token แล้วออก HttpOnly/SameSite session cookie
- ตรวจ CSRF token สำหรับ state-changing HTTP request และ WebSocket hello
- bounded body/session/rate/frame/TTL limits และ sanitized error/log policy
- HTTP `GET /healthz`, `POST /api/session`, `GET/POST /api/session/{id}` และ WebSocket status/close flow

งานนี้ยังไม่ serve frontend assets และไม่ expose remote shell/file/proxy/signaling/WebRTC capability จาก browser; สิ่งเหล่านั้นต้องผ่าน authenticated session/capability boundary และมี evidence เฉพาะก่อนเพิ่ม surface

### 2.11 Vault (Encrypted Storage)
- `src/vault.rs` — `EncryptedVault` ใช้ XChaCha20-Poly1305 สำหรับ secret persistence
- เป็น boundary แยกจาก `Storage` (ที่เก็บ metadata แบบ plain)

### 2.12 Utility Layer
- QR code (`src/utils/qr.rs`)
- error types (`src/utils/error.rs` — `BlnkError`)
- helpers (`src/utils/mod.rs`)

---

## 3. Proposed Module Structure

> โครงสร้างนี้สะท้อน source จริงใน `/workspace/src/` ณ ปัจจุบัน ส่วนที่เป็น `proto/`, `tests/`, `benchmarks/` ถูกตัดออกจากตัวอย่างนี้เพื่อให้กระชับ

```
blnk/
├── Cargo.toml                 # package manifest (edition = "2024")
├── src/
│   ├── main.rs                # CLI entry
│   ├── lib.rs                 # library root ประกาศ module ทั้งหมด
│   ├── config/                # Configuration
│   │   ├── mod.rs
│   │   └── args.rs            # CLI argument parsing (clap derive)
│   ├── discovery/             # mDNS discovery (LAN)
│   │   └── mod.rs             # MdnsResponder, discover_local_peers
│   ├── identity/              # Identity & key persistence
│   │   ├── mod.rs             # Identity + derived values (uid, pairing_code, access_code)
│   │   └── key.rs             # RSA-2048, RSA-OAEP sign/decrypt
│   ├── peer/                  # WebRTC peer connection
│   │   └── mod.rs             # PeerHandle, TwoPeerHarness
│   ├── protocol/              # Protocol
│   │   ├── mod.rs
│   │   ├── swsp.rs            # SWSP frame codec
│   │   └── pairing.rs         # Pairing messages
│   ├── session/               # Session lifecycle & auth policy
│   │   ├── mod.rs             # Typed messages
│   │   └── runtime.rs         # SessionRuntime, SessionRuntimeConfig
│   ├── signaling/             # Signaling client + orchestration
│   │   ├── mod.rs             # SignalingMessage, ProtocolVersion, Sign
│   │   ├── transport.rs       # SignalingClient, LocalFixtureServer, EndpointPolicy
│   │   └── orchestration.rs   # ผูก signaling ↔ PeerHandle ↔ SessionRuntime
│   ├── storage/               # Persistent metadata store (sqlite)
│   │   ├── mod.rs             # SqliteStore, SCHEMA_VERSION
│   │   ├── schema.rs
│   │   └── store.rs
│   ├── stream/                # Stream handlers
│   │   ├── mod.rs             # StreamRegistry, FrameFlags codec
│   │   ├── shell.rs           # ShellStreamHandler
│   │   ├── file.rs            # FileTransferService
│   │   ├── proxy.rs           # ProxyPolicy (deny-by-default)
│   │   └── proxy_handler.rs   # ProxyStreamService (TCP/WS/HTTP)
│   ├── utils/                 # Utilities
│   │   ├── mod.rs
│   │   ├── qr.rs              # QR code generation + PairingQrPayload
│   │   └── error.rs           # BlnkError (thiserror)
│   ├── vault.rs               # EncryptedVault (XChaCha20-Poly1305)
│   ├── web/                   # Loopback browser control surface
│   │   └── mod.rs             # BrowserControlServer, BrowserControlConfig
│   └── proto_generated.rs     # Generated protobuf types
├── tests/                     # Integration tests
└── docs/                      # Documentation
    ├── architecture.md        # (เอกสารนี้)
    ├── api.md
    ├── blueprint.md
    └── implementation-status.md
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

## 4.2 Browser Control Flow (Issue #47)

1. user starts `blnk web` with a loopback bind address, exact allowed Origin และ bootstrap token
2. browser sends `POST /api/session` with the exact Origin and bearer token
3. server creates a bounded local control session, returns a short-lived CSRF token and sets an HttpOnly/SameSite session cookie
4. browser reads `GET /api/session/{id}` with the session cookie to inspect local control state
5. browser may upgrade `GET /api/session/{id}/ws` only with the exact Origin and cookie, then sends a first `hello` message containing the CSRF token
6. after the WebSocket handshake, only `status` and `close` messages are accepted; malformed, repeated hello, binary or oversized frames close the connection
7. HTTP `POST /api/session/{id}` requires the session cookie and `X-CSRF-Token` and closes only the local control session

This is a local lifecycle/status flow. It does not establish browser WebRTC, connect an external signaling provider, invoke remote shell/file/proxy services, terminate TLS, or prove original-Go/browser interoperability.

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
- key material ต้องเก็บอย่างปลอดภัย — `EncryptedVault` ใช้ XChaCha20-Poly1305 สำหรับ secret ที่ต้องการ confidentiality-at-rest
- auth flow ต้องป้องกัน replay พื้นฐาน — PIN verified แบบ constant-time (`subtle::ConstantTimeEq`) พร้อม retry limit และ delay
- sensitive payload ต้องไม่ log — `tracing` redact secret เสมอ; `ProxyPolicy::redact_target_for_log` ซ่อน credential-bearing header
- input validation ต้องเข้ม — `ProxyPolicy::deny_by_default` ปฏิเสธ credentials/fragment/unsupported scheme และตรวจ private/loopback/link-local/multicast/reserved/documentation/IPv4-mapped IPv6
- protocol decoding ต้อง fail safe — `Frame::decode` ตรวจ consumed length ตรง payload; SWSP error map ไปยัง `BlnkError::Protocol` โดยไม่ทำให้ process crash
- web layer บังคับใช้ exact Origin check, HttpOnly/SameSite=Strict cookie, CSRF token และ bounded body/session/rate/frame/TTL

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
- strip symbols สำหรับ production (ใช้ `strip = "symbols"` ใน `[profile.release]`)
- ใช้ CI สำหรับทุก platform (Linux เป็น primary target ปัจจุบัน; Windows/Android อยู่ใน roadmap)
- release evidence และ gate state ปัจจุบันอยู่ใน `docs/implementation-status.md` และ `docs/releases/`
- versioning ตาม semver ใน `Cargo.toml` (`version = "0.2.x"`); ดู release note ใน `CHANGELOG.md`
- compatibility boundary สำหรับ original-client interoperability อยู่ใน `tests/fixtures/compatibility/v1/` และ `tests/compatibility_baseline.rs` (Issue #44)

---

## 10. TUI Architecture (blnk-tui)

### 10.1 Overview

blnk-tui เป็น ratatui-based terminal user interface สำหรับ blnk. แยกเป็น crate ต่างหาก (`crates/blnk-tui/`) เพื่อความยืดหยุ่นในการ develop และ test

### 10.2 Layer Model

| Layer | Modules | Role |
|---|---|---|
| **Foundation** | `width`, `color`, `terminal_palette` | Pure math, no I/O |
| **Terminal** | `terminal_hyperlinks`, `wrapping`, `shimmer` | Text rendering primitives |
| **Render** | `render/line_utils`, `render/markdown`, `render/records` | Layout and content rendering |
| **Platform** | `tui`, `keyboard_modes`, `windows_console` | OS interaction, lifecycle |
| **Feature** | `notifications`, `pets`, `workspace_messages` | Domain-specific features |
| **Protocol** | `proto` | Type definitions |

### 10.3 Key Design Decisions

1. **URL-aware wrapping** — standard `textwrap` splits URLs at `/` and `-`. `wrapping.rs` detects URL-like tokens and keeps them intact. Mixed URL/prose lines wrap prose at word boundaries while preserving URLs.

2. **Sound mark projection** — halfwidth katakana sound marks (FF9E, FF9F) are projected to equal-width placeholders before wrapping, then mapped back to source byte offsets.

3. **Terminal detection** — `utils/terminal_detection.rs` in blnk core detects terminal emulator + multiplexer via `TERM_PROGRAM`, `VTE_VERSION`, `WT_SESSION`, `TMUX`, `STY` env vars.

4. **Hyperlink display policy** — `utils/hyperlinks.rs` decides whether to show URL as label-only (terminal supports OSC 8) or full text (terminal doesn't).

5. **Style guide enforcement** — `clippy.toml` bans black/white/blue/yellow as foreground colors. Shimmer effect gets explicit `#[allow]` for RGB blending.

### 10.4 Integration with Core

blnk-tui ปัจจุบันเป็น standalone crate. เมื่อ Phase 2 เสร็จ:
- blnk-tui จะ depend on blnk core สำหรับ proto types
- blnk core จะ re-export `utils::terminal_detection` และ `utils::hyperlinks`
- Event loop จะเชื่อมต่อกับ signaling + peer modules ของ blnk core
