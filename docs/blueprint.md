# blnk Project Blueprint

> Generated from `spec.md`, `architecture.md`, `api.md`, `TODO.md`, `README.md`

---

## 1. Product Voice Card

**Name:** blnk Rust  
**Type:** Remote access multitool (peer-to-peer via WebRTC)  
**Platform:** CLI binary (Linux / Windows / Android)  
**Auth:** Optional PIN + pairing code (6-digit)  
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
- **Transport:** WebSocket to signaling server
- **Messages:** register, request, offer, answer, candidate, error
- **Compatibility:** must match original `bitbang-cli` protocol version

### 2.4 Peer Layer
- **Core:** `webrtc` crate
- **Responsibilities:** ICE, SDP, DTLS/SCTP, data channel, state machine

### 2.5 Session Layer
- **Lifecycle:** auth → ready → stream registry → stats → teardown
- **Auth:** PIN verification (constant-time), retry limit, delay
- **State:** `Arc<RwLock<SessionState>>` + channels

### 2.6 Stream Layer
- **Protocol:** SWSP over data channel
- **Stream IDs:** 0 = control, 1+ = data
- **Frame flags:** SYN, FIN, DAT, MORE
- **Handlers:**
  - shell: PTY I/O, resize
  - file: list, download, upload, stat, delete
  - proxy: HTTP request/response, header rewrite, redirect
  - tcp: raw byte forwarding
  - websocket: text/binary frame bridge
  - http: metadata + streaming body

### 2.7 Protocol Layer
- **Signaling schema:** JSON/protobuf-compatible
- **SWSP frame:** length-prefixed, max SCTP-safe size
- **Pairing:** commit-reveal, nonce challenge, SAS computation
- **Control:** Connect, AuthRequired, Auth, AuthResult, Ready, Error

### 2.8 Identity Layer
- **Keys:** RSA 2048-bit
- **Persistence:** file-based identity store
- **Operations:** generate, load, save, sign, decrypt
- **Outputs:** UID, 6-digit pairing code

### 2.9 Web/HTTP Layer
- **Server:** `axum`
- **Routes:** frontend assets, WebSocket endpoint, header rewrite/redirect

### 2.10 Utility Layer
- **QR:** terminal/file/image output
- **Logging:** `tracing` with session/peer/stream spans
- **Errors:** `thiserror` typed + `anyhow` context

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

| Layer | Status | Notes |
|-------|--------|-------|
| CLI | stub | `main.rs` has serve/connect/cp/version stubs |
| Config | missing | not implemented |
| Signaling | missing | not implemented |
| Peer | missing | not implemented |
| Session | missing | not implemented |
| Stream | missing | not implemented |
| Protocol | missing | not implemented |
| Identity | missing | not implemented |
| Web | missing | not implemented |
| Utils | missing | not implemented |

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
