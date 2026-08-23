# blnk Porting & Feature Parity Plan (Phase 1 & Phase 2)

**Goal:** Complete 100% protocol and feature parity with the reference `bitbang-cli` implementation in Rust.

**Architecture Reference:** `docs/architecture.md`, `specs/spec.md`, `docs/blueprint.md`

---

## Phase 1: MVP Release Closure (v0.2.0)
- **Target Version:** `v0.2.0`
- **Issue #48:** release: กำหนด reproducible MVP gate และ release artifacts
  - Run verification gate (`cargo fmt`, `clippy`, `test`, `check --all-targets`)
  - Ensure Linux / Windows / Android compilation reproducibility
  - Bump package version to `0.2.0` in `Cargo.toml`, `Cargo.lock`
  - Update `CHANGELOG.md` with all v0.2.0 merged features

---

## Phase 2: Feature Parity & Complete Porting (Post-0.2.0)

### Issue 1: feat(discovery): mDNS/DNS-SD Local Discovery
- **Objective:** Enable direct LAN peer discovery without relying on WAN signaling servers.
- **Specification:** `specs/spec.md` Section 4.8, `docs/architecture.md` Section 2.3.1
- **Scope:**
  - Announce blnk service on local network via mDNS.
  - Query and browse local peers via `blnk devices --local`.
  - Automatic fallback to signaling server if LAN discovery fails.
- **Acceptance:** Two nodes on the same subnet discover each other and initiate connection directly.

### Issue 2: feat(cli): QR Code Rendering for CLI Pairing Flow
- **Objective:** Render ASCII/ANSI QR code on terminal for device pairing 1:1 with bitbang-cli.
- **Specification:** `specs/spec.md` Section 4.7, `docs/blueprint.md` Section 2.10
- **Scope:**
  - Encode pairing metadata (`uid`, `pairing_code`, public key URL) using `qrcode` crate.
  - Render ANSI block / ASCII matrix to stdout on `blnk serve --qr`.
  - Fallback to raw text string if terminal output is redirected or non-interactive.
- **Acceptance:** Scanning terminal QR with mobile/web client yields exact pairing parameters.

### Issue 3: feat(nat): Configurable STUN/TURN ICE Servers
- **Objective:** Full NAT traversal support for non-direct and symmetric NAT networks.
- **Specification:** `specs/spec.md` Section 4.9, `docs/architecture.md` Section 2.2
- **Scope:**
  - Add `stun_servers` and `turn_servers` fields to `Config` struct.
  - CLI flags `--stun-server` and `--turn-server` with credentials support.
  - Pass custom ICE servers to WebRTC `RTCConfiguration`.
- **Acceptance:** Successfully connects across restricted NATs using STUN/TURN relay candidates.

### Issue 4: test(compat): Live Interoperability Test Suite vs Reference Bitbang-CLI
- **Objective:** Verify byte-for-byte protocol compatibility with original Go/Browser client.
- **Specification:** `specs/spec.md` Section 2, `docs/implementation-status.md` Section 3
- **Scope:**
  - Integration test suite comparing wire frames, signaling handshakes, and SWSP streaming.
  - Test cross-implementation file transfer and remote shell.
  - Negative tests for malformed frames, protocol version mismatches, and signature validation.
- **Acceptance:** Automated test suite verifies two-way communication between Rust and reference bitbang-cli without error.
