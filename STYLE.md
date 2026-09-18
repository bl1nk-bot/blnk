# Style

## Non-Negotiable

- **No panic in production**: `unwrap()` / `expect()` without context / `panic!()` as error flow — forbidden. Use `Result<T, E>` + `?` + `BlnkError`.
- **No secret in logs**: PIN, access_code, bootstrap_token, TURN credential, CSRF token — never in `tracing` output. Implement custom `Debug` to redact (see `Config`, `SessionRuntimeConfig`, `IceServer`).
- **No short-circuit on secrets**: PIN comparison via `subtle::ConstantTimeEq`. Never `==` on auth material.
- **No raw `canonicalize()`**: Windows `\\?\` path prefix breaks relative paths. Use `dunce::canonicalize()` or validate manually.
- **No raw child-process spawn**: Platform-specific process creation must go through the abstraction layer, not direct `Command::new()`.

## Code Organization

- `mod.rs` = public entry point. Internal details in sibling files.
- Re-export only what's used externally.
- Module doc: `//!` one-liner. Item doc: `///` on public items.
- Constants: `SCREAMING_SNAKE`. Types: `PascalCase`. Functions/vars: `snake_case`.

## Error Handling

- `BlnkError` (thiserror) for typed errors — 8 variants: `Config`, `Identity`, `Signaling`, `Peer`, `Session`, `Stream`, `Protocol`, `Io`.
- Application context: avoid `anyhow` in library code; use for binary error context only.
- `expect()` allowed only with descriptive context string explaining *why* this can't fail.

## Async

- `tokio` full features. `async_trait` for trait objects.
- I/O must be async. CPU-heavy → `spawn_blocking`.
- `tokio::select!` for multi-future wait. Never block the runtime.

## Logging

- `tracing` — never `println!`.
- Structured: `debug!(peer_id = %peer_id, "connected")`.
- Spans for session/peer/stream lifecycle.

## Security Boundaries

- Input validation at every untrusted boundary (UDP discovery, WebRTC data channel, proxy headers).
- Proxy: deny-by-default. Only explicitly allowed headers forwarded.
- File transfer: null byte rejection in paths.
- Crypto logic separated from business logic.

## Testing

- `TwoPeerHarness` for WebRTC loopback (two peers, in-process).
- `RelayFixture` for signaling integration (local WebSocket relay).
- Unit tests: behavior of single function. Integration tests: cross-module flow.
- Async tests: `#[tokio::test]`.

## Complexity

- Functions: ~50 lines max without strong reason.
- Nesting: ≤3 levels. Use early return + helper functions.
- Magic numbers: extract to named constants.

## Final Check

If you can't tell from reading the code:
- Who owns the state
- Where errors flow
- Where the async boundary is
- What a protocol message does

→ it's not done.

## TUI Style Guide

### Colors (enforced via clippy.toml)

| Context | Color | ANSI |
|---|---|---|
| Headers | bold | `Modifier::BOLD` |
| Secondary text | dim | `Modifier::DIM` |
| User input, selection, status | cyan | `Color::Cyan` |
| Success, additions | green | `Color::Green` |
| Errors, failures, deletions | red | `Color::Red` |
| "blnk" branding | magenta | `Color::Magenta` |
| Default foreground | reset | `Color::Reset` |

### Banned Colors (clippy disallowed_methods)

- `Color::Black`, `Color::White` as foreground → use `Color::Reset` or terminal default
- `Color::Blue`, `Color::Yellow` → not in style guide
- `Color::Rgb` → only allowed in shimmer (explicit `#[allow]`)
- `Color::Indexed` → use named ANSI colors

### Rules

- Most text: use default foreground (no explicit color)
- Custom RGB: only when blending terminal default colors at calculated levels
- Shimmer effect: `#[allow(clippy::disallowed_methods)]` with comment explaining why
- Contrast: let terminal theme handle it; don't force colors
