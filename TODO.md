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

## Resolved

- [x] RUSTSEC-2023-0071: OAEP usage not vulnerable, risk accepted in `.cargo/audit.toml`
- [x] `net2` / `async-std` unmaintained: removed from lockfile
- [x] Security advisories: all resolved or accepted
- [x] PR merge backlog: #111, #112, #121-#127 merged, #115-#120 closed (superseded)

## Milestones (Completed)

| # | Module | Verification |
|---|---|---|
| M0 | CI/CD & Hygiene | CI Pass |
| M1 | Foundation (Cargo.toml, build.rs, proto) | `cargo build` |
| M2 | Error & Logging (BlnkError) | `cargo test utils` |
| M3 | Config & Args | `cargo test config` |
| M4 | Identity & Pairing (RSA-2048, SAS) | `cargo test identity` |
| M5 | SWSP Codec (8-byte LE header) | `cargo test protocol::swsp` |
| M6 | Signaling Client | `cargo test signaling` |
| M7 | WebRTC Peer | `cargo test peer` |
| M8 | Session Runtime (constant-time PIN) | `cargo test session` |
| M9 | Stream Handlers (shell, file, proxy) | `cargo test stream` |
| M10 | Web Control (Axum loopback) | `cargo test web` |
| M11 | Integration Tests (134+ tests) | `cargo test --all` |
