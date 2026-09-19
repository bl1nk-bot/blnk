# TODO

> **Source of Truth**: `specs/spec.md`, `docs/architecture.md`, `CONTEXT.md`

## Active

### Release Gate (Issue #48)
- Linux (`x86_64-unknown-linux-gnu`): `runner-tested` ✅ — CI passes, `scripts/release_gate.sh` produces tarball + SHA256SUMS
- Windows (`x86_64-pc-windows-msvc`): `compile-verified` ✅ — CI build/test passes
- Android (Termux `aarch64-linux-android`): `native-tested` ✅ — runs on device, CI covers Linux
- macOS: `not-tested` / out of scope per ADR-045
- **Remaining**: Run `scripts/release_gate.sh` on real Linux environment, verify PROVENANCE.json, close Issue #48

### External Interoperability (Optional)
- Live test against external reference signaling server (not mock loopback)
- WebRTC data-channel test with reference bitbang client cross-machine
- Baseline fixtures: `tests/compatibility_baseline.rs`, `tests/live_interop.rs`

- TODO: Publish the `v0.2.13` tag/release and bump `Cargo.toml` from `0.2.12`
  when the release gate is approved.

---

## RustDesk Reuse and Adaptation Backlog

- [x] Extract RustDesk `src/rendezvous_mediator.rs` direct-connect, relay-fallback, punch retry, timeout, and duplicate-route handling into `src/signaling/transport.rs` without changing blnk's non-trickle ICE contract.
- [x] Add a bounded `ConnectionSupervisor` around `serve_session_with_shutdown` using RustDesk's single-reader plus cancellation/select lifecycle for every authenticated blnk session.
- [x] Map RustDesk `src/server/connection.rs` login-scope latching and permission checks into a versioned blnk operation-scope policy for shell, file, proxy, share, and adapter streams.
- [x] Add per-session and per-operation audit receipts at the orchestration boundary using RustDesk's connection/file audit separation and the existing `audit_events` schema.
- [x] Reuse RustDesk's file-transfer request validation pattern to add explicit size, range, path, overwrite, cancellation, timeout, and list-entry profiles to `src/stream/file.rs`.
- [x] Add atomic temporary-file write, checksum verification, and rename-on-commit to blnk file PUT using the RustDesk file-transfer lifecycle as the implementation reference.
- [x] Add stale job/session rejection for file and stream responses by tracking request IDs like RustDesk's cancelled/unknown read-job filtering.
- [ ] Add a reconnecting local IPC/session channel abstraction based on RustDesk's `next_timeout` plus reconnect-and-replace behavior for future desktop UI, adapter, and vault workers.
- [ ] Define a `TransportRoute` state model for direct, relay, and forced-relay connections by adapting RustDesk's route selection without allowing silent downgrade or policy bypass.
- [ ] Add bounded relay/punch retry metrics and route deduplication tests based on RustDesk's resend queue and punch retry tests.
- [ ] Add a permission-scoped proxy dispatcher by combining RustDesk's connection permission matrix with blnk `src/stream/proxy.rs` private-address, redirect, and resource-limit policies.
- [ ] Add operation cancellation propagation from session shutdown into shell, file, proxy, adapter, and vault tasks using the existing `CancellationToken` boundary.
- [ ] Add clipboard-as-object/share channel mapping using RustDesk text/binary/file clipboard separation and blnk `share.proto` channel/trust policies.
- [ ] Add a platform `KeyProvider` trait for Windows Credential Manager and Linux Secret Service, replacing direct root-key injection in `EncryptedVault` while preserving `vault.proto` key versions.
- [ ] Add persistent connection capability negotiation so RustDesk-style reusable connections can serve multiple approved blnk streams without widening the authenticated operation scope.
- [ ] Add cross-platform transport and file-transfer fixtures for Windows/Linux direct, relay, cancellation, timeout, and recovery cases before reusing RustDesk code patterns in production.

---

## TUI Implementation (blnk-tui)

### Phase 1: Core Rendering (current)
- [x] Workspace setup with shared dependencies
- [x] URL-aware word wrapping (`wrapping.rs`)
- [x] Streaming markdown rendering (`render/markdown.rs`)
- [x] Vertical table rendering (`render/records.rs`)
- [x] Terminal hyperlink support (OSC 8)
- [x] Terminal color detection
- [x] Shimmer animation
- [x] TUI lifecycle (init/restore/draw)
- [x] Color math utilities
- [x] Display width with sound marks
- [x] Workspace message headlines

### Phase 2: Platform Integration
- [ ] Implement kitty keyboard protocol detection
- [ ] Implement Windows console state management
- [ ] Implement desktop notifications (peer connect/disconnect)
- [ ] Implement Sixel/Kitty image rendering (ambient display)
- [ ] Replace proto stubs with prost-generated types
- [ ] Wire blnk-tui as dependency on blnk core

### Phase 3: blnk Remote Access TUI (must be complete — incomplete = broken)

**Core principle: blnk is a security/remote-access app. An incomplete feature is a broken feature. Ship nothing that doesn't work end-to-end.**

Minimum viable completeness:
- [ ] `blnk serve` → TUI mode
  - [ ] Display QR code in TUI (inline)
  - [ ] Serve QR + PIN บน browser ผ่าน web control surface
  - [ ] Show pairing code + PIN status
  - [ ] Accept incoming connections
  - [ ] Complete pairing flow (commit → reveal → SAS verify)
  - [ ] Session lifecycle (connect → auth → ready → closed)
- [ ] `blnk connect` → TUI mode
  - [ ] Device discovery (mDNS LAN scan)
  - [ ] Select peer → connect
  - [ ] PIN entry → authenticate
  - [ ] Shell stream (PTY) with resize
  - [ ] File transfer (`blnk cp`)
  - [ ] Disconnect / reconnect
- [ ] Session management
  - [ ] Active session display (peer info, latency, streams)
  - [ ] Session history
  - [ ] Graceful shutdown (Ctrl+C)
- [ ] Connection status bar
- [ ] Keyboard shortcuts (Ctrl+C disconnect, Ctrl+Z suspend)
- [ ] Split pane: shell output + status sidebar
- [ ] Remote editing via `$EDITOR` (vim/nano over shell stream)
- [ ] Error handling: every failure path has user-visible message
- [ ] Security: no secrets in logs, constant-time PIN comparison
- [ ] Add `app_event.rs` — TuiEvent enum with blnk-specific events (PeerConnected, StreamOpened, etc.)
- [ ] Add `event_stream.rs` — TuiEventStream combining crossterm events + broadcast channel
- [ ] Add `status/` — status bar (peer info, latency, streams, session state)
- [ ] Add `history_cell/` — shell session history with expand/collapse
- [ ] Add `bottom_pane/` — composable bottom panel (shell input, file transfer progress)
- [ ] Add `streaming.rs` — streaming output rendering for shell/file output

### Phase 4: Desktop App (Tauri 2 + Vite)
- [ ] Rich markdown editing (syntax highlighting, preview)
- [ ] JSON/YAML editing (schema-aware, validation)
- [ ] Object management UI
- [ ] Workspace management

---

## Codex-Inspired Adaptation Backlog

- [ ] Add layered config loader (defaults → global → project → env → CLI overrides) modeled after Codex's `ConfigLayerStack` pattern in `src/config/`.
- [ ] Add feature-flag system with runtime toggles (`Feature::enabled(name)`) for gradual rollout of proxy, adapter, and share streams.
- [ ] Add permission-profile abstraction (read-only, workspace, custom) that maps to `OperationScope` versions for fine-grained stream access control.
- [ ] Add plugin/hook boundary at `SessionRuntime` for pre/post dispatch callbacks without changing core session logic.
- [ ] Add outbound network-proxy policy layer in `ProxyPolicy` for DNS pinning, redirect validation, and port-allowlist enforcement (extends existing SSRF protection).
- [ ] Add session-resume capability by persisting `SessionRuntimeSnapshot` + active stream registry to local state DB for reconnection after disconnect.
- [ ] Add context-manager pattern for embedding project-level instructions (AGENTS.md equivalent) into session metadata for future desktop UI integration.
- [ ] Add multi-agent role abstraction (`SessionRole::Orchestrator` / `SessionRole::Worker`) for future subagent dispatch over local IPC channels.
