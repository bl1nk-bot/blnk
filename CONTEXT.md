# Domain Glossary — blnk

## Identity

**Identity** — RSA-2048 keypair + derived metadata. Contains `private_key`, `uid`, `pairing_code`, `access_code`. Never stores public key separately.

**uid** — 22-char URL-safe-base64 string derived from SHA-256 of public key DER (first 16 bytes). Immutable after generation.

**pairing_code** — 6-digit decimal string (000000-999999). Generated fresh per PEM load — not persisted in PEM format.

**access_code** — 11-char URL-safe-no-pad base64 of 8 random bytes. Stored in both JSON and PEM formats.

**IdentityFormat** — `Json` or `Pem`. PEM uses two-block format for BitBang compatibility.

## Session

**SessionState** — `Connecting → Authenticating → Ready → Closed`. Strict state machine; invalid transitions return errors.

**SessionRole** — `Server` (validates PIN, sends ready) or `Client` (supplies PIN, waits for ready).

**PIN** — Exactly 6 bytes. Constant-time comparison via `subtle::ConstantTimeEq`. Never short-circuit.

**AuthOutcome** — `Ready`, `Rejected { attempts_remaining }`, or `Closed` (exhausted).

**SessionRuntime** — Connects `Session` state machine to `PeerHandle` control channel (SWSP stream 0).

**CONTROL_STREAM_ID** — `0`. Reserved SWSP stream for session control messages.

## Signaling

**PROTOCOL_VERSION** — `3` (i32). Wire version checked on register/connect.

**RegisterRequest/Response** — Registration flow: uid + public_key → pairing_code.

**ConnectionRequest** — Client requests connection: client_id, ice_servers, force_relay.

**OfferMessage/AnswerMessage** — SDP exchange with encrypted request envelope.

**PairRequest/PairAnswer/PairApproved/PairRejected** — Out-of-band pairing flow.

**encrypted_request** — RSA-OAEP encrypted JSON: `{fingerprint, nonce, code}`.

**RequestChallenge** — Single-use UUID v4 nonce for encrypted session request validation.

**IceServer** — STUN/TURN server: urls + optional username/credential.

## Pairing

**PairCommit** — `commit = base64(SHA-256(nonce))`. Commitment phase.

**PairChallenge** — Server sends 32-byte random nonce_d.

**PairReveal** — Client reveals 32-byte nonce_c.

**SAS (Short Auth String)** — 6-digit: `SHA-256(nonce_c + nonce_d + sorted fingerprints) mod 1,000,000`.

**commitment_for()** — `base64(SHA-256(nonce))`. Zero-knowledge proof of nonce knowledge.

## SWSP (Stream Wire Session Protocol)

**SWSP** — Binary framing over WebRTC data channels.

**Frame** — `{stream_id: u32, flags: FrameFlags, payload: Vec<u8>}`.

**HEADER_LEN** — 8 bytes fixed: `stream_id (u32 LE) | flags (u16 LE) | payload_len (u16 LE)`.

**FrameFlags** — Bitflags: `SYN` (stream start), `MORE` (fragment continuation), `FIN` (stream end), `DAT` (data present). Unknown bits rejected.

**DEFAULT_MAX_PAYLOAD_LEN** — 16 KB. Conservative limit below u16 max.

## Stream

**StreamKind** — `Http`, `File`, `Tcp`, `WebSocket`, `Shell`.

**StreamRegistry** — Monotonic ID allocator + `BTreeMap<u32, StreamEntry>`. Stream 0 never allocated.

**stream_id** — Non-zero u32. Sequential, wrapping. Reuse prevented.

## Peer

**PeerHandle** — WebRTC peer wrapping `PeerConnection` + data channel + event state.

**TwoPeerHarness** — Deterministic in-process test pair: offerer + answerer with loopback UDP.

## Config

**Config** — `signaling_url`, `identity_path`, `devices_path`, `pin`, `ice_servers`. Loaded from `blnk.toml` + `BLNK_*` env vars.

**DeviceRecord** — `{id, endpoint, last_seen_unix}`. Metadata only, no secrets.

**DeviceRegistry** — Vec<DeviceRecord> with upsert/find/save. Atomic write via temp+rename.

## Discovery

**DiscoveredPeer** — `{id, ip, port, host_name}` from LAN mDNS.

**MdnsResponder** — UDP multicast announcer with CancellationToken lifecycle.

**BLNK_MDNS_SERVICE_NAME** — `_blnk._tcp.local`.

## Object Storage

**Object** — Versioned transferable unit identified by `id` and `kind`. Its searchable metadata is stored in SQLite and its sensitive payload is referenced through an opaque `payload_ref`.

**ObjectKind** — Versioned discriminator such as `mcp.server`, `provider`, `profile`, `prompt`, `skill`, `workspace.pack`, `note`, or `secret.password`.

**Sensitivity** — Storage policy: `public`, `internal`, `sensitive`, or `secret`. `secret` payloads must not enter SQLite metadata, FTS, audit events, URLs, QR content, or logs.

**ObjectRevision** — Immutable payload revision associated with an object. Updating an object creates a new revision and changes `current_revision` transactionally.

**payload_ref** — Opaque vault reference. It must not be derived from a title, path, credential, or raw payload.

**StorageState** — Cross-store write state: `staged`, `ready`, `deleting`, or `corrupt`. It coordinates SQLite metadata with the encrypted vault during crash recovery.

**Encrypted Vault** — Separate encrypted payload store. It uses a vault root key obtained from an OS keychain or passphrase provider and records key versions, authenticated encryption metadata, and payload hashes.

**AppAdapter** — Client-specific projection boundary that detects live configuration, imports it into normalized objects, previews activation, applies changes atomically, and rolls back from a backup.

**ImportHandler** — Shared pipeline for every input channel: parse → normalize → validate → policy check → redacted preview → explicit confirmation → stage → commit → receipt.

**Share** — Time-bounded delivery record that references an object or pack, records channel and recipient policy, and tracks opened, verified, consumed, revoked, and expired states.
