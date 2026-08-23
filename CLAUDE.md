# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Workflow & State Verification
- Always read `docs/implementation-status.md` to verify codebase state before answering questions or planning tasks.
- Keep `docs/implementation-status.md` updated as features and PRs are merged.
- Do not assume implementation state from memory or past conversation summaries.

## Core Commands

### Build & Check
```bash
cargo check --all-targets
cargo build --release
```

### Linting & Formatting
```bash
cargo fmt --all -- --check
cargo clippy --all --all-targets -- -D warnings
```

### Testing
```bash
# Run all tests
cargo test --all

# Run a single test by name
cargo test <test_name>

# Run tests in a specific module
cargo test session::tests
```

### Release Verification Gate
```bash
scripts/release_gate.sh
```

## Architecture & Code Structure

`blnk` is an async-first Rust port of `bitbang-cli` providing peer-to-peer remote access over WebRTC without requiring accounts or port forwarding.

### Module Hierarchy & Responsibilities
- **`src/main.rs` & `src/config/`**: CLI parsing (`clap` derive) and configuration loading (`config` TOML/env). Dispatches commands (`serve`, `connect`, `cp`, `devices`, `web`, `version`).
- **`src/identity/`**: RSA-2048 key management, PEM/JSON persistence, signing, and 6-digit pairing code generation.
- **`src/signaling/`**: WebSocket transport connecting to signaling servers, message schemas (offer/answer/candidate), and endpoint validation.
- **`src/peer/`**: WebRTC connection lifecycle (`webrtc` crate), ICE/DTLS handshake, and data channel orchestration.
- **`src/session/`**: Session state machine, constant-time PIN authentication, retry policies, and stream multiplexing registry.
- **`src/stream/`**: SWSP (Stream Wire Session Protocol) handlers over WebRTC data channels:
  - `shell`: Interactive PTY command execution and terminal resizing.
  - `file`: Sandboxed directory listing, file upload, download, and deletion.
  - `proxy`: HTTP header rewriting, streaming proxy, and TCP/WebSocket bridges.
- **`src/web/`**: Loopback-only Axum browser control surface for session inspection.
- **`src/protocol/` & `src/proto_generated.rs`**: Protobuf schemas and raw 8-byte LE SWSP frame codec (`stream_id`, `flags`, `length`).

### Conflict Resolution Order
`docs/architecture.md` > `specs/spec.md` > `docs/api.md` > `STYLE.md` > `README.md` > `TODO.md`

### Platform Constraints
- Target platforms: Linux (`x86_64`, `aarch64`), Windows (`x86_64`), Android (`aarch64`, `armv7`).
- macOS is explicitly out of scope.
