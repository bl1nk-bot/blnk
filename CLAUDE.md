# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when writing code in this repository.

## Session Lifecycle (MANDATORY)

### Session Start — SELF-STARTING
Plugin hook injects FULL PREPARATION on first turn: git state, TODO, build, blocking.
You receive this context. Do NOT wait for user instructions.

1. Read injected context (branch, dirty, TODO, build, blocking)
2. Decide what to work on based on TODO state
3. Present plan: what, why, expected outcome
4. If build FAIL or BLOCKING → fix first
5. If clean → propose next TODO item
6. Begin work after user confirms

### Session End — EXECUTE CLOSEOUT
Do NOT wait. Execute immediately when session ending.

1. `cargo check` — verify build
2. Update `TODO.md` — mark done [-], note in-progress
3. Git commit with conventional message
4. Memory — 200 chars: done, next, blockers

### You are self-starting.
Think, decide, propose — then work. Don't wait to be told.

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

## Workspace Structure

```
blnk/                     # Root workspace
├── Cargo.toml            # Workspace definition + blnk core crate
├── src/                  # blnk core (CLI + P2P engine)
├── crates/
│   └── blnk-tui/         # TUI crate (ratatui-based terminal UI)
│       ├── Cargo.toml
│       └── src/
├── proto/                # Protobuf schemas
├── tools/
│   └── argument-comment-lint/  # Dylint lint (excluded from workspace)
├── specs/                # Specifications
├── docs/                 # Documentation
└── scripts/              # Build/release scripts
```

### Workspace Members

- `blnk` — core crate (lib + bin): P2P remote access, CLI, signaling, WebRTC, streams
- `blnk-tui` — TUI crate: ratatui-based terminal UI with markdown rendering, URL-aware wrapping

### Adding New Crates

1. Create `crates/<name>/` with `Cargo.toml` + `src/lib.rs`
2. Add to `members` in root `Cargo.toml`
3. Use `version.workspace = true`, `edition.workspace = true`, `license.workspace = true`
4. Dependencies: use `{ workspace = true }` for shared deps

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

### Core Modules

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
| `utils` | `BlnkError` (thiserror), QR rendering, terminal detection, hyperlinks |
| `proto_generated` | Auto-generated protobuf bindings from `proto/*.proto` |

### TUI Modules (blnk-tui)

| Module | Role |
|---|---|
| `tui` | Terminal lifecycle: init, restore, draw, alt-screen |
| `wrapping` | URL-aware word wrapping with sound mark projection |
| `render/markdown` | Streaming markdown rendering with block tracking |
| `render/records` | Vertical table rendering (grid→key/value fallback) |
| `terminal_hyperlinks` | OSC 8 hyperlink annotation for ratatui Lines |
| `terminal_palette` | Terminal default fg/bg color detection |
| `shimmer` | Time-based sweep animation for branding |
| `color` | RGB math: blend, luma, perceptual distance |
| `width` | Display width calculation with sound mark support |
| `keyboard_modes` | Kitty keyboard protocol detection |
| `windows_console` | Win32 console state management |
| `notifications` | Desktop notifications (peer connect/disconnect) |
| `pets` | Sixel/Kitty image rendering (ambient pet) |
| `workspace_messages` | Workspace headline extraction from protobuf |
| `proto` | Protobuf type stubs (replace with prost-generated) |

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
