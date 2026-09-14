# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Quick Start

Run `just --list` for all recipes. Key ones:

```bash
just ci               # fmt + clippy + check + test — the full gate
cargo test <name>     # single test
just build-release    # release build (lto, strip, abort)
just bump <pr> "msg"  # version bump via scripts/bump_version.py
```

**Toolchain**: Rust 1.97.0 (pinned). **Primary target**: `x86_64-unknown-linux-gnu`. Cross-compile: `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-linux-android` (compile-only).

**Pre-push hook** runs `just ci` — can timeout on slow machines. Use `git push --no-verify` to bypass when CI already passed.

## Architecture

```
Browser (loopback) ←HTTP/WS→ Web Control (Axum)
     ↕
WebRTC Data Channel ← SWSP Frames → Remote Peer
     ↕
Stream Multiplexer (shell | file | proxy | tcp | websocket)
```

### The Split

**Stream 0 = control** (protobuf): session lifecycle (connect → auth → ready → error). **Stream N > 0 = data** (SWSP frames): shell, file, proxy, tcp, websocket. This split is the core invariant — control never carries data, data never carries control.

**SWSP frame**: 8-byte LE header — `stream_id (4B) | flags (2B) | length (2B)`.

### Session Lifecycle

`Connecting → Authenticating → Ready → Closed`

Commit-reveal pairing with 6-digit SAS. PIN auth via `subtle::ConstantTimeEq` — never short-circuit.

### Modules

| Module | Role |
|---|---|
| `config` | `blnk.toml` + `BLNK_*` env, device registry, CLI args (clap) |
| `identity` | RSA-2048 key pair gen/load/save (PEM/JSON) |
| `discovery` | mDNS LAN peer discovery (UDP multicast) |
| `signaling` | WebSocket signaling + orchestration + session lifecycle |
| `session` | State machine, PIN auth, stream registry |
| `peer` | WebRTC peer lifecycle, data channel I/O, `TwoPeerHarness` |
| `protocol` | Wire protocol: `pairing` (commitment, SAS), `swsp` (Frame) |
| `stream` | Multiplexer + handlers: `shell`, `file`, `proxy`, `proxy_handler` |
| `web` | Loopback-only Axum server (browser control surface) |
| `utils` | `BlnkError` (thiserror), QR rendering |
| `proto_generated` | Auto-generated protobuf bindings from `proto/*.proto` |

### Conventions

- **Debug redaction**: `Config`, `SessionRuntimeConfig`, `IceServer` implement custom `Debug` — never log secrets
- **Config layering**: `blnk.toml` → `BLNK_*` env vars (via `config` crate)
- **Constant-time security**: PIN comparison, bearer tokens — use `subtle` crate, never `==`

### Tests

`TwoPeerHarness` = in-process WebRTC loopback. `RelayFixture` = local WebSocket relay for signaling tests. Both in-process, no network.

```bash
cargo test <test_name> -- --nocapture  # single test with output
```

## CI Gotchas

- **Zero clippy warnings** enforced (`-D warnings`)
- **fmt check** must pass
- `cargo audit` ignores `RUSTSEC-2023-0071` — OAEP usage not vulnerable (see `.cargo/audit.toml`)
- GitHub Actions billing failures → `gh pr merge --admin --merge` to bypass

## Contributing

Target `main`. Bump patch via `just bump`. Add `CHANGELOG.md` entry under `## [0.2.x]`.

## Security

- **RUSTSEC-2023-0071**: See `.cargo/audit.toml` for risk acceptance rationale
- Proxy: deny-by-default, only explicitly allowed headers forwarded
- File transfer: null byte rejection in paths
