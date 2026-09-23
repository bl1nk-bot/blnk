# Implementation Status and Architecture Readiness (v0.2.6)

**Verified Base:** Codebase ณ ปัจจุบันมี implementation ครบทั้ง Core Protocol, Handlers และ CLI (PRs #1 – #101 รวมอยู่ใน `main`) โดยชุดทดสอบ `cargo test --all` ผ่านทั้งหมด 134 tests

---

## 1. Subsystem Implementation Map

| Subsystem | Source Path | State & Behavior | Evidence / Test File |
|---|---|---|---|
| **CLI & Commands** | `src/main.rs`, `src/config/args.rs` | รองรับ subcommands: `serve`, `connect`, `cp`, `devices`, `web`, `version` | `src/main.rs:tests` |
| **Protobuf Codegen** | `build.rs`, `proto/`, `src/proto_generated.rs` | คอมไพล์ schema สำหรับ signaling, control, identity, stream | `proto_generated::tests` |
| **Identity Management** | `src/identity/key.rs` | รองรับ RSA-2048 คีย์คู่, OAEP encryption, SHA256 signing, โหลด/เซฟทั้ง JSON และ PEM | `src/identity/key.rs:tests` |
| **Pairing & SAS** | `src/protocol/pairing.rs` | 6-digit numeric SAS, commit-reveal verification, unpadded base64 nonces | `protocol::pairing::tests` |
| **SWSP Frame Codec** | `src/protocol/swsp.rs` | Raw 8-byte LE header (`stream_id` 4B, `flags` 2B, `length` 2B), multi-frame buffer parsing | `protocol::swsp::tests` |
| **Signaling Transport** | `src/signaling/` | WebSocket transport, message validation, discriminator checks, replay prevention | `signaling::tests` |
| **WebRTC & DataChannel** | `src/peer/` | DataChannel event handling, ICE gathering, STUN/TURN configuration | `peer::tests` |
| **Session State Machine** | `src/session/` | PIN authentication (constant-time check), session timeouts, active stream lifecycle | `session::tests` |
| **Stream Handlers** | `src/stream/` | `shell.rs` (PTY/cancellation), `file.rs` (sandboxed ops), `proxy.rs` (SSRF-protected HTTP/TCP/WS) | `stream::*::tests` |
| **Web Control Surface** | `src/web/` | Axum loopback surface, CSRF/Cookie origin protection, WebSocket status streaming | `web::tests` |
| **Terminal Detection** | `src/utils/terminal_detection.rs` | Terminal emulator detection (name, multiplexer, hyperlink support) | `terminal_detection::tests` |
| **Hyperlink Policy** | `src/utils/hyperlinks.rs` | OSC 8 display policy (label-only vs full URL) | `hyperlinks::tests` |

---

## 1.5 TUI Subsystem (blnk-tui)

**Crate:** `crates/blnk-tui/` — ratatui-based terminal user interface

| Module | Source Path | State & Behavior | Evidence / Test File |
|---|---|---|---|
| **Terminal Lifecycle** | `tui.rs` | Raw mode, alternate screen, panic hook, synchronized draw | `tui.rs` (manual) |
| **URL-Aware Wrapping** | `wrapping.rs` | Word wrap with URL preservation, sound mark projection, mixed URL/prose | `wrapping::tests` |
| **Streaming Markdown** | `render/markdown.rs` | Single-pass parse with block boundary tracking (stub) | — |
| **Vertical Tables** | `render/records.rs` | Grid→key/value fallback with threshold detection | — |
| **Terminal Hyperlinks** | `terminal_hyperlinks.rs` | OSC 8 hyperlink annotation for ratatui Lines | — |
| **Color Detection** | `terminal_palette.rs` | Default fg/bg from COLORFGBG + ANSI→RGB | `terminal_palette::tests` |
| **Shimmer** | `shimmer.rs` | Time-based sweep animation with cosine band | — |
| **Color Math** | `color.rs` | Blend, luma, CIE76 perceptual distance | `color::tests` |
| **Display Width** | `width.rs` | Unicode width + sound mark handling | `width::tests` |
| **Keyboard Modes** | `keyboard_modes.rs` | Kitty protocol detection (stub) | — |
| **Windows Console** | `windows_console.rs` | Win32 console state (stub) | — |
| **Notifications** | `notifications.rs` | Desktop notification backend (stub) | `workspace_messages::tests` |
| **Pets** | `pets.rs` | Sixel/Kitty image rendering (stub) | — |
| **Workspace Messages** | `workspace_messages.rs` | Headline extraction from protobuf | `workspace_messages::tests` |
| **Proto Stubs** | `proto.rs` | Temporary type definitions (replace with prost) | — |

**Status:** Phase 1 complete (core rendering). Phase 2 (platform integration) pending.

---

## 2. Integration & Compatibility Evidence

- **`tests/compatibility_baseline.rs`**: ตรวจสอบ SWSP raw bytes, Protobuf control payloads, Signaling register format และ Full auth session lifecycle.
- **`tests/live_interop.rs`**: ตรวจสอบ 2-peer loopback interop, stream multiplexing และ negative failure cases (auth error, malformed frame).

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
