# Implementation Status and Architecture Readiness (v0.2.13)

**Verified Base:** Codebase ณ ปัจจุบันมี implementation ครบทั้ง Core Protocol, Handlers และ CLI (PRs #1 – #101 รวมอยู่ใน `main`) โดยชุดทดสอบ `cargo test --all` ผ่านทั้งหมด 134 tests

---

## 1. Subsystem Implementation Map

| Subsystem | Source Path | State & Behavior | Evidence / Test File |
|---|---|---|---|
| **CLI & Commands** | `src/main.rs`, `src/config/args.rs` | รองรับ subcommands: `serve`, `connect`, `cp`, `devices`, `web`, `version` | `src/main.rs:tests` |
| **Protobuf Codegen** | `build.rs`, `proto/`, `src/proto_generated.rs` | คอมไพล์ schema สำหรับ signaling, control, identity, stream; fall back ไปใช้ system protoc เมื่อ vendored ไม่รองรับ platform | `proto_generated::tests` |
| **Identity Management** | `src/identity/key.rs` | รองรับ RSA-2048 คีย์คู่, OAEP encryption, SHA256 signing, โหลด/เซฟทั้ง JSON และ PEM | `src/identity/key.rs:tests` |
| **Pairing & SAS** | `src/protocol/pairing.rs` | 6-digit numeric SAS, commit-reveal verification, unpadded base64 nonces | `protocol::pairing::tests` |
| **SWSP Frame Codec** | `src/protocol/swsp.rs` | Raw 8-byte LE header (`stream_id` 4B, `flags` 2B, `length` 2B), multi-frame buffer parsing | `protocol::swsp::tests` |
| **Stream Message Envelope** | `src/protocol/stream.rs`, `proto/stream.proto` | Unified envelope สำหรับ dispatch metadata (stream_type, kind, sequence, payload); opt-in over raw SWSP | `protocol::stream::tests` |
| **Build System** | `build.rs` | Protoc resolution: vendored → PROTOC env → which protoc → panic; Android cross-compile support | `cargo check --target aarch64-linux-android` |
| **Signaling Transport** | `src/signaling/` | WebSocket transport, message validation, discriminator checks, replay prevention | `signaling::tests` |
| **WebRTC & DataChannel** | `src/peer/` | DataChannel event handling, ICE gathering, STUN/TURN configuration | `peer::tests` |
| **Session State Machine** | `src/session/` | PIN authentication (constant-time check), session timeouts, active stream lifecycle | `session::tests` |
| **Stream Handlers** | `src/stream/` | `shell.rs` (PTY/cancellation), `file.rs` (sandboxed ops), `proxy.rs` (SSRF-protected HTTP/TCP/WS) | `stream::*::tests` |
| **Web Control Surface** | `src/web/` | Axum loopback surface, CSRF/Cookie origin protection, WebSocket status streaming | `web::tests` |

---

## 2. Integration & Compatibility Evidence

- **`tests/compatibility_baseline.rs`**: ตรวจสอบ SWSP raw bytes, Protobuf control payloads, Signaling register format และ Full auth session lifecycle.
- **`tests/live_interop.rs`**: ตรวจสอบ 2-peer loopback interop, stream multiplexing และ negative failure cases (auth error, malformed frame).

---

## 3. Pending Release Gaps (Issue #48 & Security)

1. **Platform & Artifact Matrix:**
   - Linux: `proven` (tarball + sha256 checksums ผ่าน `scripts/release_gate.sh`)
   - Windows: `runner-tested` (CI zip artifacts)
   - Android: `compile-only` (`cargo check --target aarch64-linux-android --lib`)
   - macOS: `unsupported` (ADR-045)
2. **Reproducible Release Gate:**
   - รัน `.github/workflows/release-gate.yml` เพื่อยืนยัน build provenance และ hash ของ `Cargo.lock` ก่อน tag release
