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

- TODO: Wire TCP/WebSocket/HTTP proxy dispatch into `SessionRuntime` after the
  proxy service contract and integration tests are ready.
- TODO: Publish the `v0.2.13` tag/release and bump `Cargo.toml` from `0.2.12`
  when the release gate is approved.
