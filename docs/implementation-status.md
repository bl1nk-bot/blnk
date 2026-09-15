# Implementation Status and Architecture Readiness (v0.2.13)

**Verified Base:** Codebase ณ ปัจจุบันมี implementation ครบทั้ง Core Protocol, Handlers, CLI และ discovery/storage/vault modules (PRs #1 – #127 รวมอยู่ใน `main`) โดยชุดทดสอบ `cargo test --all` ผ่านทั้งหมด

---

## 1. Subsystem Implementation Map

| Subsystem | Source Path | State & Behavior | Evidence / Test File |
|---|---|---|---|
| **CLI & Commands** | `src/main.rs`, `src/config/args.rs` | รองรับ subcommands: `serve`, `connect`, `cp`, `devices`, `web`, `version` (clap derive) | `src/main.rs:tests` |
| **Config & Device Registry** | `src/config/mod.rs` | โหลด TOML config, metadata-only device registry พร้อม persistence (`Config::load`, `DeviceRegistry::load`/`save`/`upsert`) | `config::tests` |
| **Protobuf Codegen** | `build.rs`, `proto/`, `src/proto_generated.rs` | คอมไพล์ schema สำหรับ signaling, control, identity, stream, vault, share, sync, storage, adapter, workspace, common, object_model, pairing, swsp | `proto_generated::tests` |
| **Identity Management** | `src/identity/mod.rs`, `src/identity/key.rs` | รองรับ RSA-2048 คีย์คู่, RSA-OAEP encryption, SHA256 signing, โหลด/เซฟทั้ง JSON และ PEM, derived values (`uid`, `pairing_code`, `access_code`) | `identity::tests` |
| **Pairing & SAS** | `src/protocol/pairing.rs` | 6-digit numeric SAS, commit-reveal verification, unpadded base64 nonces | `protocol::pairing::tests` |
| **SWSP Frame Codec** | `src/protocol/swsp.rs` | Raw 8-byte LE header (`stream_id` 4B, `flags` 2B, `length` 2B), multi-frame buffer parsing, consumed-length check | `protocol::swsp::tests` |
| **Signaling Transport** | `src/signaling/transport.rs` | WebSocket transport (`SignalingClient`, `LocalFixtureServer`, `EndpointPolicy::PublicOnly`/`AllowLocal`), message validation, preflight DNS/IP check, replay prevention, `DEFAULT_MAX_MESSAGE_SIZE` | `signaling::transport::tests` |
| **Signaling Orchestration** | `src/signaling/orchestration.rs` | ผูก signaling ↔ `PeerHandle` ↔ `SessionRuntime`; remote `connect_target`, `accept_server_session`, `serve_session`, `run_shell_client`, `run_file_client` | `signaling::orchestration::tests` |
| **WebRTC & DataChannel** | `src/peer/mod.rs` | `PeerHandle` (offer/answer, ICE gathering, channel open/close/error), `TwoPeerHarness` (in-process test), `webrtc` v0.20 + tokio runtime | `peer::tests` |
| **Session State Machine** | `src/session/mod.rs`, `src/session/runtime.rs` | `SessionRuntime` (handshake, ready, stream registry, close), PIN auth (constant-time check + retry limit + delay), session timeouts | `session::tests` |
| **Stream Handlers** | `src/stream/` | `shell.rs` (PTY/cancellation), `file.rs` (sandboxed ops, GET/PUT/LIST/STAT/DELETE), `proxy.rs` (`ProxyPolicy` deny-by-default), `proxy_handler.rs` (`ProxyStreamService` สำหรับ TCP/WS/HTTP) | `stream::*::tests` |
| **Discovery (mDNS)** | `src/discovery/mod.rs` | `MdnsResponder` (announce), `discover_local_peers` (LAN discovery), `DEFAULT_MDNS_DISCOVERY_TIMEOUT`, lifecycle ผ่าน `CancellationToken` | `discovery::tests` |
| **Storage** | `src/storage/mod.rs`, `src/storage/schema.rs`, `src/storage/store.rs` | `SqliteStore`, `SCHEMA_VERSION = 1` (rusqlite bundled); ใช้สำหรับ device registry และ metadata persistence | `storage::tests` |
| **Vault (Encrypted)** | `src/vault.rs` | `EncryptedVault` ใช้ XChaCha20-Poly1305 (chacha20poly1305 crate) สำหรับ secret at-rest; แยก boundary จาก plain `Storage` | `vault::tests` |
| **Web Control Surface** | `src/web/mod.rs` | Axum loopback surface (`BrowserControlServer`, `BrowserControlConfig`), CSRF/Cookie/Origin protection, WebSocket status streaming, bounded body/session/rate/frame/TTL | `web::tests` |
| **Utilities** | `src/utils/qr.rs`, `src/utils/error.rs` | `render_qr_terminal`/`render_qr_ascii`, `PairingQrPayload`, `BlnkError` (8 variants: Config, Identity, Signaling, Peer, Session, Stream, Protocol, Io) | `utils::tests` |

---

## 2. Integration & Compatibility Evidence

- **`tests/compatibility_baseline.rs`**: ตรวจสอบ SWSP raw bytes, Protobuf control payloads, Signaling register format และ Full auth session lifecycle (fixtures อยู่ใน `tests/fixtures/compatibility/v1/`)
- **`tests/live_interop.rs`**: ตรวจสอบ 2-peer loopback interop, stream multiplexing และ negative failure cases (auth error, malformed frame)

---

## 3. Pending Release Gaps (Issue #48 & Security)

1. **Security Vulnerability Blockers:**
   - `rsa 0.9.10` มี advisory `RUSTSEC-2023-0071` (Marvin timing side-channel)
   - Transitive unmaintained dependencies: `net2`, `async-std` (ผ่าน `mdns 3.0.0`)
2. **Platform & Artifact Matrix:**
   - Linux: `proven` (tarball + sha256 checksums ผ่าน `scripts/release_gate.sh`)
   - Windows: `runner-tested` (CI zip artifacts)
   - Android: `compile-only` (`cargo check --target aarch64-linux-android --lib`)
   - macOS: `unsupported` (ADR-045)
3. **Reproducible Release Gate:**
   - รัน `.github/workflows/release-gate.yml` เพื่อยืนยัน build provenance และ hash ของ `Cargo.lock` ก่อน tag release
4. **Proxy Dispatch Gap (Issue #42):**
   - `ProxyStreamService` มี bounded TCP/WebSocket/HTTP implementation แต่ยังไม่ wire เข้า `SessionRuntime` stream dispatcher ผ่าน authenticated session