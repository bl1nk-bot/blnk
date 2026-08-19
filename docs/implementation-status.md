# Implementation Status and Readiness

**Verified base:** `main` includes the merged foundation, identity/pairing, SWSP, signaling/session/stream boundary, protobuf-generation, review-remediation, and CI-recovery work from PRs #6, #8, #10, #12, #14, #22, #24, #25, #26, #28, and #29. PR #30 is an open compatibility slice for Issue #23.

**Status of this document:** The Rust foundation and protocol boundaries are implemented and locally verifiable. The user-facing remote-access path is not complete: signaling transport, WebRTC lifecycle, runtime session wiring, stream handlers, CLI dispatch, cross-platform builds, and original-client interoperability remain in progress.

## Executive Summary

blnk Rust has a runnable foundation, generated protobuf bindings, identity/pairing primitives, the SWSP raw-frame codec, and typed signaling/session/stream boundaries. These components are tested at unit and boundary level, but they do not yet constitute a usable remote-access product. The next work is to connect the boundaries to a local signaling transport and WebRTC data channel, then wire session control, stream handlers, and CLI operations.

A compiled schema, dependency, or CLI command is not an acceptance result by itself. The implementation must provide business logic, negative tests, local end-to-end evidence, and—before claiming interoperability—fixtures or a live compatibility test against the original client/server.

## Evidence Matrix

| Area | Present in repository | Status |
|---|---|---|
| CLI surface | `serve`, `connect`, `cp`, `devices`, and `version` exist in `src/main.rs`, but the remote-operation handlers still contain placeholder behavior | Boundary only; not user-ready |
| Architecture | Module layout, data flow, runtime model, security principles, and testing principles are documented | Design ready |
| Protocol schemas | `.proto` files cover signaling, identity, pairing, control, stream, and SWSP | Schema present |
| Protobuf build | `build.rs` uses `prost-build` with vendored `protoc`; generated bindings are included through `src/proto_generated.rs` | Implemented; wire compatibility still needs fixtures |
| Library foundation | `src/lib.rs`, typed errors, config loader, module boundaries, and CLI wiring are present | Implemented |
| Identity and pairing | RSA-2048 identity, load/save, signing, OAEP encryption, six-digit `pairing_code`, `access_code`, nonce, commit-reveal, and provisional SAS are implemented. JSON remains the existing default path; PR #30 adds an explicit upstream-compatible PEM boundary and fixtures without silently changing the default | Primitives implemented; compatibility slice in progress |
| SWSP codec | Typed flags, canonical eight-byte little-endian header, max-payload enforcement, incomplete-frame handling, round-trip tests, and negative tests are implemented | Codec implemented; data-channel integration pending |
| Signaling/session/stream boundaries | Typed message boundaries, discriminator validation, PIN policy, retry handling, state machine, and stream registry are implemented | Boundary implemented; runtime transport integration pending |
| Build | `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo test --all`, and `cargo clippy --all --all-targets -- -D warnings` pass on Linux after the dependency and CI fixes | Passing on Linux |
| CI | Format, test, and clippy jobs run on the main workflow; a cross-platform matrix and release workflow are not yet present | Partial |
| Tests | Unit and boundary tests cover configuration, identity, pairing, SWSP, signaling, session, and generated-protobuf compilation; there is no complete two-peer remote-access test | Boundary coverage only |
| Release | No verified Linux/Windows/Android binary artifacts, checksums, or installation flow are published | Not started |

## Acceptance Gates

The runnable-foundation gate is satisfied when the library boundaries and test harness exist and the following commands pass on Linux:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all
cargo clippy --all --all-targets -- -D warnings
git diff --check
```

The next gate is the local remote-access MVP. It requires a deterministic local signaling fixture, two Rust peers, a completed WebRTC data-channel handshake, authenticated session transitions, SWSP data transfer, and at least one file and one shell operation with negative-path tests. This gate must not require production credentials or an external secret.

The specification gate is stricter: signaling, WebRTC data channel, pairing/PIN, shell, file, proxy, TCP, and WebSocket flows must work with the declared protocol, have meaningful test coverage, and build on Linux, Windows, and Android within the stated scope. A local Rust-to-Rust harness is evidence for implementation correctness, not proof of interoperability with the original Go/browser system.

## Canonical Decisions

When documents conflict, use `docs/architecture.md` > `specs/spec.md` > `docs/api.md` > `STYLE.md` > `README.md` > `TODO.md`.

| Topic | Decision used for implementation |
|---|---|
| Rust toolchain | Use Rust 2024 and the toolchain pinned by repository/container configuration. |
| User-visible pairing code | Use the six-digit `pairing_code` defined by the functional specification. |
| Persistent credential naming | Keep `access_code` distinct from `pairing_code`; do not use a generic `code` field. |
| Identity persistence compatibility | Keep the existing JSON load/save behavior as the non-breaking default. Add PEM read/write as an explicit compatibility path, with format detection, fixtures, and migration tests. Do not claim that the default on-disk format has changed until Issue #23 is completed. |
| SWSP | Keep the raw eight-byte little-endian frame header (`stream_id`, `flags`, `length`) and enforce the existing size/error semantics. Protobuf remains the schema/control representation until a compatibility fixture proves another wire contract. |
| Pairing SAS | Treat the current deterministic construction as provisional until exact upstream encoding is demonstrated by an interoperability fixture. |
| Security | Implement deny-by-default boundaries, bounded resources, constant-time credential comparisons, path/target validation, and platform-specific permission handling as part of each feature—not as a substitute for completing the feature. |
| Credentials and secrets | Local fixtures and two-peer tests must use generated test keys and explicit test data. Production secrets are not required for the local MVP; secret-backed integration belongs to a later evidence gate. |
| Platform scope | Support Linux, Windows, and Android. macOS is out of scope. |

## Known Risks and Non-Claims

The largest remaining risk is interoperability with the original client/browser and signaling server. The repository contains schemas and typed boundaries, but the end-to-end mapping to the original runtime has not yet been demonstrated. Compilation and Rust-to-Rust tests must not be reported as original-client interoperability.

Other risks include PTY and filesystem differences, networking behavior, Android packaging, identity migration, PIN retry policy, TCP target validation, resource limits, and lifecycle cleanup. Each feature must add negative tests and platform notes before it is considered ready.

## Recommended Implementation Order

| Order | Work item | Definition of done |
|---:|---|---|
| 1 | Documentation/status synchronization (Issue #31) | Canonical status matches merged code, open PRs, known gaps, and evidence limits. |
| 2 | Identity compatibility slice (Issue #23 / PR #30) | JSON remains compatible; PEM boundary has deterministic fixtures; migration/read-both/write policy is tested; issue remains open until the compatibility contract is complete. |
| 3 | Signaling transport and local fixture | Real WebSocket transport is connected to typed signaling messages; deterministic local server/client fixture covers offer, answer, candidate, errors, reconnect, and shutdown without production secrets. |
| 4 | WebRTC peer/data-channel integration | Two local Rust peers complete signaling, ICE/DTLS setup, data-channel open/close, backpressure, and cleanup with observable tests. |
| 5 | Session runtime integration | Pairing/auth/PIN state machine is driven by real control messages, retry/deadline behavior is exercised end to end, and unauthorized transitions are rejected. |
| 6 | Stream MVP | File and shell handlers are connected through the stream registry and SWSP framing with size, timeout, path, process, cancellation, and cleanup tests. |
| 7 | CLI workflow | `serve`, `connect`, `cp`, and `devices` invoke real business logic; placeholders are removed; local two-process workflow is documented and tested. |
| 8 | Additional capabilities | HTTP, TCP, WebSocket, mDNS, QR, and Android adapters are implemented one capability at a time with separate acceptance tests and platform notes. |
| 9 | Interoperability and release | Original Go/browser fixtures or live compatibility tests, Linux/Windows/Android CI evidence, security audit, performance/resource checks, artifacts, and release documentation are complete. |

## References

[1]: ../README.md "Project scope and current README"
[2]: ../specs/spec.md "Functional and acceptance requirements"
[3]: ../src/main.rs "Current CLI entry point"
[4]: architecture.md "Architecture and module layout"
[5]: ../proto/ "Protocol schemas"
[6]: ../Cargo.toml "Cargo manifest"
[7]: ../build.rs "Deterministic protobuf generation"
[8]: ../src/proto_generated.rs "Generated protobuf bindings"
[9]: ../.github/workflows/ci.yml "Current CI workflow"
[10]: https://github.com/bl1nk-bot/blnk/issues/23 "Issue #23: identity persistence compatibility"
[11]: https://github.com/bl1nk-bot/blnk/pull/30 "PR #30: identity compatibility slice"
[12]: https://github.com/bl1nk-bot/blnk/issues/31 "Issue #31: documentation/status synchronization"
[13]: decisions/identity-persistence.md "Identity persistence decision record"
