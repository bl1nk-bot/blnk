# TODO

> **Source of Truth**: `specs/spec.md`, `docs/architecture.md`, `CONTEXT.md`

## Active

### Release Gate (Issue #48)
- Linux (`x86_64-unknown-linux-gnu`): `runner-tested` ✅ — CI passes, `scripts/release_gate.sh` produces tarball + SHA256SUMS
- Windows (`x86_64-pc-windows-msvc`): `compile-verified` ✅ — CI build/test passes
- Android (`aarch64-linux-android`): `compile-only` ✅ — `cargo check --target aarch64-linux-android --lib --locked`
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
- [ ] Reuse RustDesk's file-transfer request validation pattern to add explicit size, range, path, overwrite, cancellation, timeout, and list-entry profiles to `src/stream/file.rs`.
- [ ] Add atomic temporary-file write, checksum verification, and rename-on-commit to blnk file PUT using the RustDesk file-transfer lifecycle as the implementation reference.
- [ ] Add stale job/session rejection for file and stream responses by tracking request IDs like RustDesk's cancelled/unknown read-job filtering.
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

## Codex-Inspired Adaptation Backlog

- [ ] Add layered config loader (defaults → global → project → env → CLI overrides) modeled after Codex's `ConfigLayerStack` pattern in `src/config/`.
- [ ] Add feature-flag system with runtime toggles (`Feature::enabled(name)`) for gradual rollout of proxy, adapter, and share streams.
- [ ] Add permission-profile abstraction (read-only, workspace, custom) that maps to `OperationScope` versions for fine-grained stream access control.
- [ ] Add plugin/hook boundary at `SessionRuntime` for pre/post dispatch callbacks without changing core session logic.
- [ ] Add outbound network-proxy policy layer in `ProxyPolicy` for DNS pinning, redirect validation, and port-allowlist enforcement (extends existing SSRF protection).
- [ ] Add session-resume capability by persisting `SessionRuntimeSnapshot` + active stream registry to local state DB for reconnection after disconnect.
- [ ] Add context-manager pattern for embedding project-level instructions (AGENTS.md equivalent) into session metadata for future desktop UI integration.
- [ ] Add multi-agent role abstraction (`SessionRole::Orchestrator` / `SessionRole::Worker`) for future subagent dispatch over local IPC channels.
