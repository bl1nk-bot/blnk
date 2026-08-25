# blnk Rust Roadmap & Actionable Remaining Tasks

> **Repository Status:** Core MVP implemented at `v0.2.6` (134/134 tests passing).  
> **Source of Truth:** [`specs/spec.md`](specs/spec.md), [`docs/architecture.md`](docs/architecture.md), [`docs/releases/issue-48-release-gate.md`](docs/releases/issue-48-release-gate.md).

---

## 1. Actionable Next Steps (Work Remaining)

### Task 1: Resolve Security Advisories & Dependency Health (Issue #86 / Gate #48)
- **Problem:** `cargo audit` ตรวจพบ 1 advisory และ 2 unmaintained warnings:
  - `rsa 0.9.10` → `RUSTSEC-2023-0071` (Marvin timing side-channel; รอ upstream patch หรือเปลี่ยน crate ถ้าจำเป็น)
  - `net2 0.2.39` (transitive dependency จาก `mdns`) → unmaintained
  - `async-std 1.13.2` (transitive dependency จาก `mdns`) → unmaintained
- **Action Items:**
  - ติดตาม/อัปเดต `rsa` เมื่อมี release patch หรือพิจารณา isolation
  - ประเมินการแทนที่ `mdns 3.0.0` ด้วย crate ที่ maintained (เช่น `simple-dns` หรือ native multicast) ใน `src/discovery/mod.rs`
- **Validation Command:** `cargo audit` / `cargo check`

### Task 2: Release Packaging & Verification Gate (Issue #48)
- **Target Matrix & Levels:**
  - **Linux (`x86_64-unknown-linux-gnu`):** `proven` → ตรวจสอบผ่าน `scripts/release_gate.sh` ให้ได้ tarball + `SHA256SUMS`
  - **Windows (`x86_64-pc-windows-msvc`):** `runner-tested` → รัน CI สร้าง zip package
  - **Android (`aarch64-linux-android`):** `compile-only` → ปัจจุบันรัน `cargo check --target aarch64-linux-android --lib --locked` (ยังไม่รองรับ APK/NDK test)
  - **macOS:** `unsupported` (ตาม ADR-045)
- **Action Items:**
  - ตรวจสอบ `scripts/release_gate.sh` บน Linux environment จริง
  - ตรวจสอบ artifact provenance metadata (`PROVENANCE.json`)
  - ปิด GitHub Issue #48 เมื่อ release gate ผ่านครบ

### Task 3: External Live Interoperability (Optional / Future Milestone)
- **Current State:** มี baseline fixtures (`tests/compatibility_baseline.rs`) และ integrated mock tests (`tests/live_interop.rs`)
- **Action Items:**
  - ทดสอบต่อเชื่อมกับ external reference signaling server จริง (ไม่ใช่ mock loopback)
  - ทดสอบ WebRTC data-channel กับ reference bitbang client ข้ามเครื่อง

---

## 2. Completed Milestones Reference

| Milestone | Key Modules & Implementation Files | Verification |
|---|---|---|
| **M0: CI/CD & Hygiene** | `.github/workflows/{ci,release,release-gate}.yml`, `CONTRIBUTING.md` | CI Pass |
| **M1: Foundation** | `Cargo.toml`, `build.rs`, `proto/*.proto`, `src/lib.rs`, `src/proto_generated.rs` | `cargo build` |
| **M2: Error & Logging** | `src/utils/error.rs` (`BlnkError`), `src/utils/logging.rs` | `cargo test utils` |
| **M3: Config & Args** | `src/config/mod.rs`, `src/config/args.rs` | `cargo test config` |
| **M4: Identity & Pairing** | `src/identity/key.rs` (RSA-2048/OAEP/PEM/JSON), `src/protocol/pairing.rs` (SAS 6-digit) | `cargo test identity` |
| **M5: SWSP Codec** | `src/protocol/swsp.rs` (8-byte LE header: StreamID 4B, Flags 2B, Length 2B) | `cargo test protocol::swsp` |
| **M6: Signaling Client** | `src/signaling/client.rs`, `src/signaling/messages.rs`, `src/signaling/orchestration.rs` | `cargo test signaling` |
| **M7: WebRTC Peer** | `src/peer/mod.rs`, `src/peer/ice.rs` (STUN/TURN, DataChannel event handler) | `cargo test peer` |
| **M8: Session Runtime** | `src/session/runtime.rs`, `src/session/auth.rs` (Constant-time PIN validation) | `cargo test session` |
| **M9: Stream Handlers** | `src/stream/` (`shell.rs` PTY, `file.rs`, `proxy.rs`, `proxy_handler.rs`) | `cargo test stream` |
| **M10: Web Control** | `src/web/mod.rs` (Axum loopback, CSRF, Cookie, WebSocket broadcast) | `cargo test web` |
| **M11: Integration Tests** | `tests/compatibility_baseline.rs`, `tests/live_interop.rs` (134 total tests) | `cargo test --all` |
