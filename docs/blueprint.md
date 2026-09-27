# blnk Project Blueprint

> Generated from `spec.md`, `architecture.md`, `api.md`, `TODO.md`, `README.md` และตรวจสถานะจาก `implementation-status.md`

---

## 0. Documentation and Implementation Status

This blueprint describes the target product and module behavior. It does not assert that the corresponding Rust modules already exist. For readiness, build results, known risks and acceptance gates, see [`implementation-status.md`](implementation-status.md).

## 1. Product Voice Card

**Name:** blnk Rust  
**Type:** Remote access multitool (peer-to-peer via WebRTC)  
**Platform:** CLI binary (Linux / Windows / Android)  
**Auth:** Optional PIN + user-visible pairing code (6-digit); persistent `access_code` is separate if required by the original protocol
**Transport:** WebRTC data channel over signaling server  
**Discovery:** mDNS (LAN) + signaling server (WAN)  
**Output:** Single static binary, async-first, memory-safe  

---

## 2. Module Voice Cards

### 2.1 CLI Layer
- **Commands:** `serve`, `connect`, `cp`, `devices`, `version`
- **Parsing:** `clap` derive
- **Responsibility:** args → config load → business logic dispatch

### 2.2 Config Layer
- **Sources:** config file, env vars, CLI flags
- **Keys:** signaling URL, identity path, PIN, STUN/TURN, debug level
- **Format:** TOML via `config` crate

### 2.3 Signaling Layer
- **Transport:** WebSocket ผ่าน `tokio-tungstenite` พร้อม local relay fixture สำหรับทดสอบ
- **Adapter shape:** `src/signaling/transport.rs` เก็บ `SignalingClient` + `LocalFixtureServer`; `src/signaling/orchestration.rs` ผูก signaling เข้ากับ `PeerHandle` และ `SessionRuntime`
- **Messages:** register, request, offer, answer, candidate, pair_request, pair_answer, pair_approved, pair_rejected, error
- **Compatibility:** `PROTOCOL_VERSION = 3` ตรวจบน register/answer/connect; protocol version ต้องตรงกับต้นฉบับเพื่อ wire-compat

### 2.4 Peer Layer
- **Core:** `webrtc` crate (v0.20) บน `tokio` runtime
- **Responsibilities:** ICE (non-trickle), SDP, DTLS/SCTP, data channel, state machine
- **Test harness:** `TwoPeerHarness` แลกเปลี่ยน SDP ภายใน process โดยไม่พึ่ง signaling/STUN/TURN ภายนอก

### 2.5 Session Layer
- **Lifecycle:** auth → ready → stream registry → stats → teardown
- **Auth:** PIN verification (constant-time), retry limit, delay
- **State:** `Arc<RwLock<SessionState>>` + channels

### 2.6 Stream Layer
- **Protocol:** SWSP บน WebRTC data channel (header 8 byte little-endian)
- **Stream IDs:** `0` = control, `1+` = data
- **Frame flags:** `SYN`, `FIN`, `DAT`, `MORE`
- **Handler location:**
  - `src/stream/shell.rs` — ShellStreamHandler (PTY I/O, resize, cancellation)
  - `src/stream/file.rs` — sandboxed file service (GET, PUT, LIST, STAT, DELETE)
  - `src/stream/proxy.rs` — `ProxyPolicy` deny-by-default, DNS pinning, redaction
  - `src/stream/proxy_handler.rs` — `ProxyStreamService` สำหรับ TCP, WebSocket, HTTP (one-request transcript)
  - หมายเหตุ: ปัจจุบัน TCP/WebSocket/HTTP ยังเป็น bounded service ที่ผูกผ่าน `ProxyPolicy`; การ dispatch จาก `SessionRuntime` สำหรับ stream เหล่านี้ยังเป็นงานถัดไป (ดู issue #42)

### 2.7 Protocol Layer
- **Signaling schema:** typed JSON/protobuf structure ใน `src/signaling/mod.rs` พร้อม `proto/*.proto`
- **SWSP frame:** 8-byte little-endian header (`stream_id` 4 bytes, `flags` 2 bytes, `length` 2 bytes) ตามด้วย payload ความยาว `length` (payload สูงสุด 65,535 bytes)
- **Pairing:** commit-reveal, nonce challenge, SAS computation; user-visible pairing code เป็นเลข 6 หลัก
- **Control:** connect, auth_required, auth, auth_result, ready, error (encode เป็น JSON บน SWSP stream 0)

### 2.8 Identity Layer
- **Keys:** RSA-2048-bit
- **Persistence:** `Identity::save`/`Identity::load` รองรับทั้ง JSON และ PEM (PEM ใช้รูปแบบสอง-block เพื่อเข้ากับ upstream)
- **Operations:** generate, load, save, sign, decrypt (RSA-OAEP)
- **Derived values:** `uid` (22-char URL-safe-base64 จาก SHA-256 ของ public key DER), `pairing_code` (เลข 6 หลัก), และ persistent `access_code` (11 ตัวอักษร URL-safe-no-pad base64)

### 2.9 Web/HTTP Layer
- **Server:** `axum` + `tokio`
- **Routes:** เฉพาะ loopback — `/api/session` (POST), `/api/session/{id}` (GET/POST), `/api/session/{id}/ws`
- **Scope:** lifecycle/status ของ local control session เท่านั้น ไม่ expose remote shell/file/proxy/signaling/WebRTC capability จาก browser
- **Boundary:** exact Origin check, HttpOnly/SameSite=Strict cookie, CSRF token, bounded body/session/rate/frame/TTL

### 2.10 Utility Layer
- **QR:** terminal QR (`render_qr_terminal` / `render_qr_ascii`) สำหรับ pairing
- **Logging:** `tracing` พร้อม session/peer/stream spans; redact secret เสมอ
- **Errors:** `BlnkError` (thiserror) มี 8 variants: Config, Identity, Signaling, Peer, Session, Stream, Protocol, Io

### 2.11 Discovery Layer
- **mDNS:** UDP multicast บน `_blnk._tcp.local` สำหรับ LAN peer discovery
- **Cancellable:** `MdnsResponder` ใช้ `CancellationToken` ควบคุม lifecycle

---

## 3. Data Flow Blueprint

```
User CLI
  │
  ▼
CLI Layer (clap)
  │
  ▼
Config Layer
  │
  ├─▶ Identity Layer (key load/generate)
  │
  ├─▶ Signaling Layer (WebSocket)
  │     │
  │     ▼
  │   Signaling Server
  │     │
  │     ▼
  │   Peer Layer (WebRTC)
  │     │
  │     ▼
  │   Session Layer (auth + registry)
  │     │
  │     ▼
  │   Stream Layer (SWSP dispatch)
  │     │
  │     ├─▶ Shell Handler
  │     ├─▶ File Handler
  │     ├─▶ Proxy Handler
  │     ├─▶ TCP Handler
  │     ├─▶ WebSocket Handler
  │     └─▶ HTTP Handler
  │
  └─▶ Direct commands (cp, version)
```

### 3.1 Device Start Flow
1. `serve` → load config
2. load/generate identity
3. connect signaling server
4. register device
5. wait for connection request
6. establish WebRTC peer
7. create data channel
8. start session dispatcher

### 3.2 Browser Connect Flow
1. browser → signaling server
2. server → device request
3. device creates offer
4. browser receives offer → sends answer
5. ICE candidates exchanged
6. data channel opens
7. control stream starts
8. session ready

### 3.3 Stream Dispatch Flow
1. receive SWSP frame
2. parse stream id + flags
3. route to handler by stream type
4. handler processes I/O
5. response frame encoded → back

---

## 4. File Structure Target

```
blnk/
├── Cargo.toml                 # workspace / package manifest
├── src/
│   ├── main.rs                # CLI entry
│   ├── lib.rs                 # library root
│   ├── config/
│   │   ├── mod.rs
│   │   └── args.rs
│   ├── signaling/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── messages.rs
│   ├── peer/
│   │   ├── mod.rs
│   │   ├── connection.rs
│   │   └── ice.rs
│   ├── session/
│   │   ├── mod.rs
│   │   ├── session.rs
│   │   └── auth.rs
│   ├── stream/
│   │   ├── mod.rs
│   │   ├── handler.rs
│   │   ├── shell.rs
│   │   ├── file.rs
│   │   ├── proxy.rs
│   │   ├── tcp.rs
│   │   ├── websocket.rs
│   │   └── http.rs
│   ├── protocol/
│   │   ├── mod.rs
│   │   ├── swsp.rs
│   │   └── pairing.rs
│   ├── identity/
│   │   ├── mod.rs
│   │   └── key.rs
│   ├── fileshare/
│   │   ├── mod.rs
│   │   └── share.rs
│   ├── shell/
│   │   ├── mod.rs
│   │   ├── pty.rs
│   │   └── terminal.rs
│   ├── proxy/
│   │   ├── mod.rs
│   │   └── http.rs
│   ├── tcpforward/
│   │   ├── mod.rs
│   │   └── forward.rs
│   ├── utils/
│   │   ├── mod.rs
│   │   ├── qr.rs
│   │   ├── logging.rs
│   │   └── error.rs
│   └── web/
│       └── static/
├── tests/
├── benchmarks/
├── proto/
└── docs/
    ├── architecture.md
    └── api.md
```

---

## 5. Implementation Status

> ตารางนี้เดิมเป็นสถานะเริ่มต้นของ blueprint ไม่ใช่หลักฐานสถานะปัจจุบัน หลังจากนั้นให้ยึด [`docs/implementation-status.md`](implementation-status.md) เป็น source of truth โดยเฉพาะข้อจำกัด interoperability และ release evidence

| Layer | Status | Notes |
|-------|--------|-------|
| CLI | implemented | `serve`, `connect`, `cp`, `devices` และ `version`; local fixture และ deterministic remote relay paths มี business logic จริง |
| Config | implemented | โหลด config และ metadata-only device registry พร้อม validation/persistence |
| Signaling | implemented locally | typed JSON/WebSocket transport และ remote register/request/offer/answer orchestration ผ่าน deterministic relay fixture; external provider ยังไม่ยืนยัน |
| Peer | implemented locally | non-trickle offer/answer, data channel และ lifecycle ผ่าน local UDP fixture |
| Session | implemented locally | authenticated control handshake, PIN policy และ stream lifecycle |
| Stream | implemented partially | shell/file และ proxy boundaries มี implementation; TCP/WebSocket dispatcher และบาง service integration ยังเหลือ |
| Protocol | implemented | signaling/SWSP/control schemas, raw codec และ compatibility fixtures |
| Identity | implemented | RSA-2048 persistence, pairing/access credentials และ cryptographic helpers |
| Web | planned | browser control surface เป็นงานถัดไปหลัง CLI E2E; ยังไม่มีหลักฐาน runtime |
| Utils | implemented partially | typed errors และ supporting helpers มีอยู่; release/operational evidence ยังไม่ครบ |

---

## 6. Critical Constraints

- **Protocol compatibility:** must match original `bitbang-cli` exactly
- **Rust-first:** maximize Rust ecosystem, minimize FFI
- **No service worker in Rust:** frontend responsibility only
- **Single binary:** release build must be lean, static where possible
- **No panic flow:** avoid `unwrap/expect` in production paths
- **Crypto safety:** PIN must use constant-time compare, keys must not log

---

## 7. Acceptance Criteria

- [ ] `serve` accepts connections
- [ ] `connect` establishes session via signaling
- [ ] `cp` transfers files bidirectionally
- [ ] WebRTC peer connection + data channel works
- [ ] Shell stream with PTY + resize
- [ ] File stream: list, download, upload, stat, delete
- [ ] HTTP proxy with header rewrite + redirect
- [ ] TCP forwarding
- [ ] WebSocket bridging
- [ ] Pairing code flow
- [ ] PIN auth with retry limit
- [ ] mDNS discovery
- [ ] QR code generation
- [ ] Builds on Linux / Windows / Android
- [ ] Tests cover core paths

---

## 8. Quick Reference

**Build:**
```bash
cargo build
cargo build --release
```

**Run:**
```bash
cargo run -- serve
cargo run -- connect
cargo run -- cp
```

**Test:**
```bash
cargo fmt --all -- --check
cargo clippy --all --all-targets -- -D warnings
cargo test --all
cargo audit
```

**Dependencies:**
- tokio, webrtc, axum, clap, tracing, thiserror, serde, prost, qrcode, mdns, nix/winapi
