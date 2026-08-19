# Identity persistence compatibility decision

**Status:** Provisional compatibility boundary

**Date:** 2026-08-19

**Tracking issue:** [Issue #23](https://github.com/bl1nk-bot/blnk/issues/23)

## Evidence

The upstream `richlegrand/bitbang-cli` implementation stores a persistent identity in `identity.pem` as two PEM blocks:

1. `PRIVATE KEY`, containing an RSA PKCS#8 private-key DER payload.
2. `BITBANG ACCESS CODE`, containing the raw 8-byte access credential. The URL-facing value is the same bytes encoded with unpadded base64url.

The upstream implementation derives the UID from the first 16 bytes of SHA-256 over the public-key DER. The user-visible six-digit pairing code is generated for the pairing/session lifecycle and is not persisted in the upstream PEM file.

The evidence comes from the upstream implementation and tests:

- [`internal/identity/identity.go`](https://github.com/richlegrand/bitbang-cli/blob/main/internal/identity/identity.go)
- [`internal/identity/identity_test.go`](https://github.com/richlegrand/bitbang-cli/blob/main/internal/identity/identity_test.go)

## Decision for this implementation slice

Blnk exposes explicit `Identity::load_pem` and `Identity::save_pem` boundaries and checks in a deterministic fixture under `tests/fixtures/bitbang_identity.pem`. The PEM loader derives UID from the loaded public key and generates a fresh six-digit pairing code, while preserving the access code from the PEM block. The loader validates the RSA modulus is exactly 2048 bits and the access-code block is exactly eight bytes.

The existing `Identity::load` and `Identity::save` JSON methods remain unchanged in this slice. This avoids silently changing the default on-disk format before migration behavior, legacy JSON handling, and the public configuration default have a final compatibility decision.

## Non-claims

This fixture proves format-level parsing and round-trip behavior for the observed upstream PEM representation. It does **not** prove end-to-end interoperability with the original signaling server or browser client. That requires signaling/WebRTC fixtures and remains outside this issue slice.

## Remaining work before closing Issue #23

The issue remains open until the project decides whether the default persistence path will migrate from JSON to PEM, defines legacy JSON migration or rejection behavior, updates configuration/docs consistently, and adds a cross-implementation fixture run against the upstream client.

The default format must not be changed without those compatibility tests and a recorded final decision.

## Platform scope

The implementation is intended for Linux, Windows, and Android. macOS is outside the supported platform scope. The current explicit PEM boundary is pure Rust and does not use platform-specific replacement behavior; the existing JSON atomic save path retains its previously reviewed platform handling.

## References

- [`proto/identity.proto`](../../proto/identity.proto)
- [`specs/spec.md`](../../specs/spec.md)
- [`docs/blueprint.md`](../blueprint.md)
- [`docs/implementation-status.md`](../implementation-status.md)
