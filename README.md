# blnk

P2P remote access over WebRTC — no accounts, no port forwarding. RSA-2048 identity, 6-digit SAS pairing, constant-time PIN auth.

![blnk Demo](docs/assets/blnk-demo.gif)

## Features

- **Shell & File Transfer** — remote shell + high-speed file transfer via SWSP over WebRTC Data Channel
- **mDNS Discovery** — find LAN peers automatically (`blnk devices --local`)
- **QR Pairing** — terminal QR code for instant device pairing (`blnk serve --qr`)
- **NAT Traversal** — STUN/TURN ICE servers for cross-firewall connections
- **Local Fixture** — full in-process test mode (`--local-fixture`) — no external secrets needed
- **HTTP Proxy, TCP Forwarding, WebSocket Bridging** — via SWSP stream multiplexer

## Installation

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/bl1nk-bot/blnk/main/scripts/install.ps1 | iex
```

**Linux (Bash):**
```bash
curl -fsSL https://raw.githubusercontent.com/bl1nk-bot/blnk/main/scripts/install.sh | bash
```

Both install to `~/.blnk/bin` and add to PATH. Auto-update available via `--auto-update` flag.

## Quick Start

```bash
# Local fixture — in-process authenticated test
cargo run -- serve --local-fixture --once
cargo run -- connect --local-fixture
cargo run -- cp --local-fixture ./local.txt remote.txt

# Device discovery
cargo run -- devices --list

# Browser control surface (loopback only)
cargo run -- web --host 127.0.0.1 --port 0 --origin http://127.0.0.1:3000
```

## Platform Support

| Platform | Evidence | Scope |
|---|---|---|
| Linux (`x86_64-unknown-linux-gnu`) | CI: format, check, test, clippy | `runner-tested` |
| Windows (`x86_64-pc-windows-msvc`) | CI build/test | `compile-verified` |
| Android (`aarch64-linux-android`) | CI compile-only | `compile-verified` |
| macOS | No CI job | `not-tested` |

## Architecture

```
Browser (loopback) ←HTTP/WS→ Web Control (Axum)
     ↕
WebRTC Data Channel ← SWSP Frames → Remote Peer
     ↕
Stream Multiplexer (shell | file | proxy | tcp | websocket)
```

See `CONTEXT.md` for domain glossary, `docs/architecture.md` for full design, `STYLE.md` for coding conventions.

## Development

```bash
just ci               # fmt + clippy + check + test
cargo test <name>     # single test
just build-release    # release build (lto, strip, abort)
```

See `CLAUDE.md` for agent guidance, `CONTRIBUTING.md` for contribution workflow.

## License

MIT
