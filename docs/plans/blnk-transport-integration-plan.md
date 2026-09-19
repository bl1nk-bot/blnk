# blnk Transport Integration Plan (Bl1nk ↔ blnk Rust)

**Version:** draft v1
**Date:** 14 กันยายน 2026
**Status:** ร่างแรกสำหรับ review — implementation ทุก phase ต้องมี PR ตาม `AGENTS.md` delivery lifecycle
**Source of truth precedence:** `AGENTS.md` (canonical list) > `specs/spec.md` + `docs/architecture.md` + `docs/api.md` + `docs/implementation-status.md` > เอกสารนี้ > roadmap mentions
**เอกสารนี้อธิบาย target behavior ไม่ใช่หลักฐานการ implement** — สถานะจริงของ module ใดตรวจจาก `docs/implementation-status.md`

> Upstream research: Track A = `deliverables/track-a-blnk-transport-surface.md` (28 KB, blnk transport surface profile) | Track B = `deliverables/track-b-bl1nk-transport-requirements.md` (57 KB, Bl1nk requirements + mapping)

---

## 0. Document Status & Scope

**เป้าหมาย:** detailed implementation plan สำหรับการ integrate **Bl1nk Universal Transfer & Secure Workspace** desktop application กับ **blnk Rust** ในฐานะ P2P transport layer ตามที่ roadmap `specs/blnk-product-roadmap.md` Phase 3 (P3-01 .. P3-06) และ Phase 5 (P5-02) ระบุไว้

**Scope ของเอกสารนี้:**

- กำหนด architectural position ของ transport layer ระหว่าง Bl1nk กับ blnk
- ตัดสิน integration pattern (library vs subprocess)
- Map ทุก Bl1nk transport operation ไปยัง blnk Rust API + identify gaps
- กำหนด envelope-to-SWSP bridging strategy + fragmentation + retry/cancel/receipt semantics
- แบ่ง work packages ตาม team topology + ระบุ verification gates
- แยก required changes ระหว่าง blnk repo vs Bl1nk repo ชัดเจน
- List open risks + decisions ที่ต้อง escalate

**ไม่อยู่ใน scope:**

- Bl1nk UI implementation (Phase 2 ของ roadmap)
- macOS/Android/iOS native (Phase 8+)
- Original-Go/browser interop (production evidence gap)
- OAuth provider, public cloud vault, multi-tenant infra
- Full proxy/failover runtime
- General workflow execution ที่มี arbitrary side effects

---

## 1. Executive Summary

- **blnk Rust พร้อมเป็น reusable library แล้ว** — `Cargo.toml` ประกาศทั้ง `[lib]` และ `[[bin]]` (Cargo.toml:9-15) + `src/lib.rs` expose 12 public modules รวม `signaling::SignalingClient`, `peer::PeerHandle`, `session::SessionRuntime`, `protocol::Frame` (SWSP), `stream::*` handlers, `identity::Identity`, `discovery` (mDNS), `vault` — Bl1nk `transport-blnk` crate สามารถ depend ตรงโดยไม่ต้อง shell-out
- **Integration pattern = library (Option B)** — depend on `blnk` ผ่าน `Cargo.toml` พร้อม `git` + tag; ได้ typed error propagation (`BlnkError` 8 variants), no IPC overhead, test reuse `TwoPeerHarness`/`RelayFixture` ทันที, single process lifecycle, version coupling จัดการด้วย Cargo
- **10-row API mapping ครอบคลุม** — device discovery, signaling connect, session auth (PIN+access code), open shell, open file read/write, send/receive share envelope, fragment large payload, ack/receipt, close session
- **ShareEnvelope bridging strategy = envelope-as-stream + envelope-level fragmentation** — 1 SWSP stream ต่อ 1 share envelope (ซึ่งมี N ObjectRecord + 1 encrypted_payload); fragment payload > 16 KB ผ่าน `MORE` frames; reassembly buffer + SHA-256 integrity check
- **5 implementation phases** — (P1) blnk lib API hardening + upstream contribution; (P2) transport-blnk skeleton + smoke; (P3) ShareEnvelope transport เต็ม + receipt; (P4) SyncBundle + access-code verifier; (P5) production hardening + cross-machine evidence
- **8 work packages แบ่งตาม team topology** — T4 (Transport/Sharing) lead + T2 (Storage/Vault), T5 (Desktop), T8 (QA/Security) coordinated
- **6 high-impact risks** — blnk lib API stability (no semver guarantee yet), original-Go/browser interop unproven, SWSP max payload 16 KB fragmentation ทุก envelope, sender-side receipt ต้องเพิ่ม application protocol, access-code verifier state machine ใหม่ทั้งหมด, cross-machine evidence ยังเป็น gap
- **Required changes split ชัด** — blnk repo: 7 upstream contributions (lib API hardening, share envelope codec, fragment/reassemble, progress hooks, access code verifier, sync bundle, test harness export) | Bl1nk repo: 1 new crate + 3 integrations (transport-blnk crate, device registry bridge, share service events)

---

## 2. Architectural Position

### 2.1 Layer Diagram

```text
┌─────────────────────────────────────────────────────────────────────┐
│ Bl1nk Desktop (Tauri 2 + React/TypeScript)                          │
│ Home | Vault | Send | Receive | Devices | Workspaces | History     │
│                                                                     │
│ Application Services (T1 Domain, T2 Storage, T3 Adapters, T6 CLI) │
│  - Object Registry | Import Handler | Policy | Adapters | Sync    │
│  - Share Service ←── ShareEnvelope events                           │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ typed events
┌──────────────────────────────────▼──────────────────────────────────┐
│ transport-blnk crate (T4) — Bl1nk-side facade                       │
│  - Bl1nkDeviceRegistry  ↔  blnk::DeviceRegistry                    │
│  - ShareEnvelope codec  ↔  SWSP frames                             │
│  - Access code verifier state machine (Argon2id)                   │
│  - Progress / cancel / retry / receipt events                      │
│  - Typed errors: TransportError → Bl1nk ServiceError               │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ direct Rust API call (no IPC)
┌──────────────────────────────────▼──────────────────────────────────┐
│ blnk Rust crate (T8 dependency)                                     │
│  signaling::SignalingClient  peer::PeerHandle  session::SessionRuntime │
│  protocol::Frame (SWSP)      stream::* handlers identity::Identity │
│  discovery (mDNS)            vault                                 │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ wire
┌──────────────────────────────────▼──────────────────────────────────┐
│ WebRTC Data Channel over signaling server + mDNS LAN discovery      │
│  - Signaling: WebSocket JSON (protocol_version = 3)                │
│  - Data: SWSP 8-byte LE header + payload ≤16 KB                    │
│  - Auth: PIN (constant-time) + RSA-2048 identity + RSA-OAEP envelope │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 Layer Boundaries

| Boundary | Caller | Callee | Contract |
|---|---|---|---|
| UI ↔ Application Services | Tauri commands + typed IPC | T1/T2/T3/T6 services | structured errors, no SQL/WebRTC direct call |
| Application Services ↔ transport-blnk | typed Rust API | `transport_blnk::Transport` | `TransportError` enum, async |
| transport-blnk ↔ blnk Rust | typed Rust API | blnk public modules | `BlnkError` re-exported |
| blnk ↔ signaling server | WebSocket JSON | external signaling | protocol_version = 3 |
| blnk ↔ peer | WebRTC Data Channel | remote blnk peer | SWSP frames + SWSP frame format |

### 2.3 Trust Boundaries

- **Tauri ↔ Backend** — typed commands, structured error codes, no SQL in frontend
- **transport-blnk ↔ blnk** — single process, Rust type system, no separate trust boundary (in-process)
- **blnk ↔ signaling** — TLS via wss:// (production); plain ws:// only for local fixture
- **blnk ↔ peer** — DTLS via WebRTC crate; SWSP over data channel
- **Bl1nk Vault ↔ blnk private key** — Bl1nk owns key material; blnk uses Bl1nk-provided Identity via constructor injection (proposed upstream change)

---

## 3. Integration Pattern: Library vs Subprocess

### 3.1 Decision

**แนะนำ: Library (Option B)** — Bl1nk depend บน `blnk` เป็น Rust crate ผ่าน `Cargo.toml` + `git` + tag

### 3.2 Rationale (5 ข้อ)

1. **Cargo solves version coupling** — `blnk = { git = "https://github.com/bl1nk-bot/blnk", tag = "v0.x.y" }` ใน `Cargo.toml` ของ Bl1nk guarantees pinned version + reproducible builds + lockfile diff review
2. **Typed error propagation แบบ zero-cost** — `BlnkError` เป็น `thiserror` enum (8 variants) → propagate ผ่าน `?` operator → map เป็น `TransportError` ใน transport-blnk ได้โดยไม่ต้อง serialize/parse ข้าม process
3. **Test fixture reuse ทันที** — `TwoPeerHarness` และ `RelayFixture` เป็น in-process; ถ้า Bl1nk depend เป็น crate ก็ `use blnk::test_utils::*` ใน integration tests ได้ตรง ๆ
4. **Single process lifecycle** — ไม่ต้อง manage subprocess startup/shutdown/restart; ไม่ต้อง handle stdin/stdout framing; ไม่ต้อง deal with zombie processes
5. **PAN-OS-style isolation ไม่จำเป็น** — Bl1nk Vault เป็น process-internal; transport-blnk กับ blnk อยู่ process เดียวกัน trusted เท่ากัน

### 3.3 ผลกระทบที่ตามมา

| Dimension | Library (chosen) | Subprocess (rejected) |
|---|---|---|
| Bundle size | Bl1nk binary รวม blnk source (compile once, link statically) | Bl1nk binary + `blnk` binary แยก (2 artifacts, installer 2 files) |
| Deployment | 1 binary, 1 update channel | 2 binaries, version sync risk |
| IPC overhead | none | stdout parsing + JSON/structured framing |
| Error propagation | typed `Result<T, BlnkError>` | stdout/stderr + exit code + log parse |
| Version coupling | Cargo lockfile | runtime check + warning if version mismatch |
| Security (process boundary) | none (trusted) | มี process boundary แต่ Bl1nk Vault share memory anyway |
| Testing | `use blnk::test_utils::*` direct | subprocess harness + JSON assertions |
| Hot reload | not applicable (compile-time) | restart subprocess |

### 3.4 Upstream contribution ที่ต้องทำเพื่อให้ library path สะอาด

| Change | Purpose |
|---|---|
| `[package].publish = true` (หรือ private registry) + semver tag | downstream `cargo add blnk` ได้ |
| Doc comments (`///`) ครบทุก public type ใน transport modules | downstream DX |
| `pub use` facade ที่ crate root สำหรับ transport subset | ergonomics: `use blnk::SignalingClient` |
| `TwoPeerHarness` / `RelayFixture` gated `#[cfg(feature = "test-harness")]` แล้ว export | downstream integration test reuse |
| Crate version aligned กับ release tag (currently v0.2.6) | reproducibility |

---

## 4. API Mapping: Bl1nk transport-blnk → blnk Rust

mapping ต่อไปนี้แสดง Bl1nk use case (left) → blnk Rust API call (middle) → config params (right) → expected output (rightmost). ใช้เป็น contract สำหรับ `transport-blnk` crate

| # | Bl1nk Operation | blnk API Call | Config Params | Expected Output |
|---|---|---|---|---|
| 1 | **Discover device ใน LAN** | `discovery::MdnsResponder::announce()` + `discovery::query()` | service name, port | `Vec<DiscoveredPeer>` พร้อม id/ip/port |
| 2 | **Connect signaling (remote device)** | `SignalingClient::new(url)` → `connect()` → `connect_with_retry()` | signaling_url, identity_path, endpoint_policy | `SignalingConnection` |
| 3 | **Register device** | `SignalingClient::transact(RegisterRequest)` | uid, public_key | `RegisterResponse { pairing_code }` |
| 4 | **Send/receive pairing commit** | `SignalingClient::transact(PairRequest/PairAnswer)` | nonce, commit | `PairApproved/PairRejected` |
| 5 | **Open WebRTC connection** | `PeerHandle::create_offer()` → `accept_offer()` → `wait_connected()` | target device, ice_servers | `PeerHandle` (Connected) |
| 6 | **Open data channel** | `PeerHandle::create_data_channel("blnk-control")` → `wait_channel_open()` | label, reliability config | `DataChannel Open` |
| 7 | **Run session handshake** | `SessionRuntime::run_handshake()` | pin, role | `SessionState::Ready` |
| 8 | **Open shell stream** | `StreamRegistry::open()` + `ShellHandler` | command, env, cwd | `stream_id`, async shell I/O |
| 9 | **Open file stream (download)** | `StreamRegistry::open()` + `FileHandler` | `remote:path`, range, offset | `stream_id`, chunked file bytes |
| 10 | **Open file stream (upload)** | `StreamRegistry::open()` + `FileHandler` | local path, chunk_size | `stream_id`, chunked file bytes |
| 11 | **Send share envelope (N objects + encrypted_payload)** | `Frame::encode(stream_id, flags, payload)` → `PeerHandle::send_frame()` (loop per fragment with MORE flag) | envelope bytes, stream_id (≥1), MORE chain | SWSP frames transmitted; reassembled at receiver |
| 12 | **Receive share envelope** | `PeerHandle::recv_frame()` loop → reassembly buffer (until FIN) | stream_id | complete `ShareEnvelope` bytes |
| 13 | **Reply with ShareEvent (opened/verified/consumed)** | `Frame::encode(stream_id=0, DAT)` → `PeerHandle::send_frame()` | event message | event delivered to peer |
| 14 | **Close session** | `SessionRuntime::close()` → `PeerHandle::close()` → `SignalingConnection::close()` | (idempotent) | resources released |

### 4.1 Pin/identity lifecycle ที่ Bl1nk ต้อง enforce

- **PIN** — ใช้แค่ครั้งเดียวต่อ session; blnk `SessionRuntime` handle attempt/lockout ให้แล้ว
- **access_code** — สร้างใน blnk `Identity` ตอน bootstrap; persist ใน Bl1nk Vault (encrypted); pass เข้า `SessionRuntime` เมื่อ reconnect
- **pairing_code** — 6-digit decimal; แสดง QR ผ่าน `utils::qr`; ใช้ครั้งเดียวต่อ pair

### 4.2 Bl1nk operations ที่ map ไม่ตรง (gaps)

| Bl1nk Need | blnk Capability | Gap | Workaround |
|---|---|---|---|
| Send envelope > 16 KB | SWSP max payload 16 KB | envelope fragmentation | transport-blnk implements fragment/reassemble |
| Send N objects batched | 1 stream = 1 capability | multi-object envelope | transport-blnk uses envelope-as-stream (1 stream per envelope containing N objects) |
| Send envelope receipt callback | no app-level ACK | no sender-side notification when receiver opens | transport-blnk implements request/ack application protocol on top of SWSP |
| Per-share access code (Argon2id) | 6-digit PIN + access_code in Identity | no Argon2id verifier | transport-blnk implements Argon2id verifier as new component |
| Per-share attempts/lockout | session-level PIN attempts | per-share state machine | transport-blnk maintains `ShareStatus` state per envelope id |
| Progress reporting | shell cancel only | byte-count progress | transport-blnk counts bytes reassembled, emits progress events |
| Cancel mid-transfer | shell cancel only | stream cancel for file/envelope | transport-blnk sends RST frame + drop reassembly buffer |

---

## 5. ShareEnvelope Wire Bridging

### 5.1 Mapping: `blnk.share.ShareEnvelope` ↔ SWSP frames

```text
ShareEnvelope (protobuf bytes)
  ├─ version            (uint32, field 1)
  ├─ share_id           (string, field 2)
  ├─ sender_device_id   (string, field 3)
  ├─ recipient_device_id (string, field 4)
  ├─ objects[]          (repeated ObjectRecord, field 5)
  ├─ encrypted_payload  (bytes, field 6)   ← opaque ciphertext
  ├─ signature          (bytes, field 7)
  ├─ signature_algorithm (string, field 8)
  ├─ payload_hash       (string, field 9)
  ├─ expires_at         (int64, field 10)
  ├─ one_time           (bool, field 11)
  ├─ trust_policy       (TrustPolicy enum, field 12)
  └─ headers{}          (map, field 13)

         │  protobuf bytes (size variable, MB-scale)
         ▼
┌──────────────────────────────────────────────────────┐
│ transport-blnk envelope codec                        │
│  1. serialize ShareEnvelope → bytes (prost)         │
│  2. prepend envelope header (12 bytes):             │
│     [magic 2B = 0xBE 0xEF] [version u16 LE]         │
│     [flags u16 LE] [total_len u32 LE]                │
│  3. fragment into chunks ≤14 KB (envelope payload)  │
│  4. emit SWSP frames: stream_id ≥1                  │
│     - chunk 0: flags = SYN | DAT                    │
│     - chunk k: flags = DAT | MORE (k<n-1)           │
│     - chunk n-1: flags = DAT | FIN                  │
└──────────────────────────────────────────────────────┘
         │
         ▼
SWSP frame(s) → PeerHandle::send_frame()
```

### 5.2 Fragmentation Strategy

**เลือก: envelope-as-stream + envelope-level fragmentation**

**เหตุผล:**

1. Match `ShareEnvelope` semantics (1 envelope = N ObjectRecord + 1 encrypted_payload)
2. Reuse SWSP `MORE` flag pattern ที่ `stream/file.rs` ใช้อยู่แล้ว
3. Simplify ordering — 1 envelope = 1 in-order stream; receiver ไม่ต้อง interleave หลาย objects
4. Stream cleanup ง่าย — เมื่อ FIN หรือ error, drop 1 stream ไม่กระทบ streams อื่น

**ขนาด:**

- SWSP max payload = 16 KB conservative (blnk `DEFAULT_MAX_PAYLOAD_LEN`)
- envelope header 12 bytes → chunk payload max = 16 KB − 12 = 16372 bytes
- envelope 1 MB → ~64 chunks; envelope 100 KB → ~7 chunks; envelope < 16 KB → 1 chunk (no MORE flag)

### 5.3 Reassembly & Integrity

**Receiver-side:**

```text
PeerHandle::recv_frame() loop:
  for stream_id in registry:
    if frame.flags.SYN:
      start new envelope buffer
      reserve stream_id → envelope_session_id
    if frame.flags.MORE | frame.flags.DAT:
      append frame.payload to envelope_session_id buffer
    if frame.flags.FIN:
      finalize buffer:
        1. verify total_len from envelope header matches
        2. verify SHA-256(payload) == envelope.header.payload_hash
        3. verify signature using sender_device_id's public key
        4. verify expires_at > now (or mark expired)
        5. deserialize → ShareEnvelope
        6. emit ShareEvent::Received
        7. drop envelope buffer
    if frame.flags.RST or protocol error:
      drop envelope buffer
      emit ShareEvent::Failed
```

### 5.4 Retry / Cancel / Receipt Semantics

**Sender-side retry policy:**

- transport-blnk maintains `ShareSession` state per `share_id`
- Retry on transport error up to N=3 with exponential backoff (1s, 4s, 16s)
- After exhaustion → emit `ShareEvent::Failed` with reason
- Receiver-side: `one_time` envelopes after successful verify → drop + emit `ShareEvent::Consumed`

**Cancel:**

- Sender: send SWSP `RST` frame on the envelope stream + drop state
- Receiver: drop reassembly buffer immediately
- Emits `ShareEvent::Cancelled` ทั้งสองฝั่ง

**Receipt flow:**

```
Sender                                   Receiver
  │                                          │
  │ ─── send share envelope (stream ≥1) ───►│
  │             [fragmented frames]          │
  │                                          │ verify + apply
  │ ◄── ShareEvent::Opened (stream 0) ──────│
  │ ◄── ShareEvent::Verified (stream 0) ────│
  │ ◄── ShareEvent::Consumed (stream 0) ────│  (one-time only)
  │                                          │
  sender updates ShareRecord.status:
    OFFERED → OPENED → VERIFIED → CONSUMED
```

ShareEvent messages go through SWSP stream 0 (control) encoded as protobuf, distinct from `SessionRuntime` control messages — need a sub-protocol namespace เพื่อไม่ชน (proposed: ใช้ SWSP control opcode 0x10–0x1F range)

### 5.5 Access Code Verifier (P3-02)

Bl1nk-side component (not in blnk core):

```text
ShareRecord.id → ShareAccessCode:
  - verifier: Argon2id(pin_code, share_id, salt)
  - attempts: u32 (current)
  - max_attempts: u32 = 5
  - locked_until: Option<Instant>
  - expires_at: Instant
  - status: ShareStatus (Created → Offered → Opened → Verified → Consumed/Expired/Revoked)
```

State transitions verified by transport-blnk ก่อน open session boundary:

1. Receiver receives offer → emit `ShareEvent::Created` (sender side) / `ShareEvent::Offered` (receiver side)
2. Receiver prompts user for access code → verify with Argon2id (constant-time)
3. On success → run session handshake → emit `ShareEvent::Opened`
4. On failure → increment attempts; if ≥ max_attempts → emit `ShareEvent::Failed` + lockout
5. On expiry check → emit `ShareEvent::Expired`

---

## 6. Work Packages (cross-team)

แบ่งตาม team topology ใน `specs/blnk-product-roadmap.md:158-216`

| WP ID | Owner Team | Goal | Deliverable | Dependency | Exit Criteria |
|---|---|---|---|---|---|
| **WP-T4-01** | T4 | Define `transport-blnk` crate skeleton | crate skeleton + Cargo.toml + module structure + minimal `TransportError` | none | `cargo check` ผ่าน + empty module compiles |
| **WP-T4-02** | T4 | Implement device registry bridge | `Bl1nkDeviceRegistry` ↔ `blnk::DeviceRegistry` mapping + discovery adapter | WP-T4-01 | round-trip test: Bl1nk `DeviceRecord` ↔ blnk `DeviceRecord` |
| **WP-T4-03** | T4 | Implement signaling connect + register + pair | thin wrapper over `SignalingClient` | WP-T4-01 | unit test: register → pair → connect ด้วย `RelayFixture` |
| **WP-T4-04** | T4 | Implement session open + close + PIN + access_code | thin wrapper over `SessionRuntime` | WP-T4-03 | integration test: full handshake ใน `TwoPeerHarness` |
| **WP-T4-05** | T4 | Implement share envelope codec + fragmentation | `EnvelopeCodec` module + split/combine + integrity verify | WP-T4-04 | unit test: 100 B / 16 KB / 100 KB / 1 MB envelope round-trip + SHA-256 verify |
| **WP-T4-06** | T4 | Implement access code verifier (Argon2id) | `AccessCodeVerifier` + state machine + attempts + lockout + TTL | none (Bl1nk-only) | unit test: 5-attempt lockout + constant-time verify + TTL expiry |
| **WP-T4-07** | T4 | Implement ShareEvent callback channel | SWSP control opcode 0x10–0x1F + sender/receiver state machine | WP-T4-04, WP-T4-05 | integration test: opened → verified → consumed events delivered |
| **WP-T2-08** | T2 | Implement Vault integration for envelope payload | store `encrypted_payload` as opaque Vault blob + hash ref | WP-T4-05 | round-trip: write blob → read blob → hash match |
| **WP-T5-09** | T5 | Implement Send wizard UI + receive wizard UI | Tauri page + form + preview + apply | WP-T4-07, WP-T2-08 | E2E test: send envelope → receive → apply workspace pack |
| **WP-T8-10** | T8 | Define cross-machine evidence + threat review | threat model + cross-machine test plan + security checklist | WP-T4-07, WP-T5-09 | threat review report + cross-machine test scenario ready |

**Team handoff rule** (จาก roadmap 5.3):

- ทีมที่สร้าง contract ต้องส่ง schema, examples, negative cases, version policy, test fixture พร้อมกัน
- ทีมที่ใช้ contract ห้ามเดา behavior จาก implementation ภายใน
- ทุก WP ต้อง open PR ที่ target `main` (Bl1nk repo) หรือ upstream blnk repo ตามที่ระบุ พร้อม `Closes #<issue>` per AGENTS.md

---

## 7. Phased Implementation Roadmap

### 7.1 Phase 1 — blnk lib API hardening (blnk repo, ~4 weeks)

**Goal:** ทำให้ blnk Rust เป็น reusable library ที่ downstream (Bl1nk) depend ได้สะอาด

**Work packages (blnk repo upstream):**

- BX-01 [package].publish + semver tag policy
- BX-02 `pub use` facade at crate root for transport subset
- BX-03 Doc comments `///` ครบทุก public type ใน signaling/peer/session/protocol/stream/identity
- BX-04 `TwoPeerHarness` + `RelayFixture` gate ด้วย `#[cfg(feature = "test-harness")]` แล้ว export
- BX-05 Add `share` module: envelope codec + fragment/reassemble helper (เตรียมสำหรับ Phase 3)
- BX-06 Add typed `Progress`/`Cancel` events ใน `StreamHandler` trait
- BX-07 Add typed `ShareEvent` codec (sender/receiver) on top of SWSP control stream

**Verification gate:**

- `cargo build` + `cargo test --all` ผ่าน (134+ tests, ยังคงเดิม)
- `cargo doc --no-deps` สร้าง doc สำหรับทุก public type
- `cargo publish --dry-run` ผ่าน (ถ้า public) หรือ internal registry dry-run
- Integration test: external crate depend on `blnk` ผ่าน `cargo add` + import `SignalingClient` + call API

**Exit criteria:**

- Downstream Bl1nk crate `transport-blnk` สามารถ depend บน `blnk` ผ่าน git + tag ได้
- All public types มี doc comments
- Test fixtures export ได้

**Risks:**

- Semver compatibility ยังไม่ guarantee — Phase 1 ต้องตกลง semver policy ก่อน
- Doc coverage เพิ่มเวลา ~1 week เหนือ estimate

### 7.2 Phase 2 — transport-blnk crate skeleton + smoke (Bl1nk repo, ~3 weeks)

**Goal:** มี transport-blnk crate ที่ depend บน blnk + รัน smoke test ของ shell + file ได้

**Work packages (Bl1nk repo):**

- WP-T4-01, WP-T4-02, WP-T4-03, WP-T4-04

**Verification gate:**

- `cargo test -p transport-blnk` ผ่าน (smoke tests)
- Integration test: 2 Bl1nk processes ในเครื่องเดียวกัน run shell + file ผ่าน blnk
- `TwoPeerHarness` reuse ได้ใน transport-blnk tests

**Exit criteria:**

- transport-blnk ส่ง shell command ผ่าน blnk → ได้ output กลับ
- transport-blnk ส่ง file ผ่าน blnk → ได้ไฟล์ครบ + hash match

**Risks:**

- blnk lib API breaking change ระหว่าง Phase 1 → Phase 2 — pin tag ต้องแน่น

### 7.3 Phase 3 — ShareEnvelope transport เต็ม + receipt (Bl1nk repo, ~4 weeks)

**Goal:** Bl1nk desktop ส่ง share envelope ระหว่าง 2 Windows/Linux devices + รับ + apply + receipt callback ได้

**Work packages (Bl1nk repo + upstream contribution):**

- WP-T4-05, WP-T4-06, WP-T4-07, WP-T2-08, WP-T5-09

**Verification gate:**

- 2 Windows/Linux Bl1nk devices ส่ง share envelope (5 objects, 100 KB encrypted payload) → อีกเครื่อง verify + apply + receipt delivered ภายใน 30s
- Negative cases: expired share, wrong access code, corrupt signature, one-time replay — ทุก case rejected + ShareEvent emitted
- Vault integration: encrypted_payload blob round-trip + hash match
- UI: send wizard + receive wizard E2E (Phase 3 ของ roadmap)

**Exit criteria:**

- ShareEnvelope transport ทำงาน E2E บน Windows/Linux devices 2 เครื่อง
- Receipt (sender รู้ว่า receiver opened/consumed) ทำงาน
- Access code verifier + lockout + TTL ทำงาน
- one-time replay rejected

**Risks:**

- Cross-device signaling provider dependency — ต้องตั้ง local relay หรือ external signaling service
- mDNS discovery ข้าม network จำกัด — ต้อง fallback signaling
- Vault lock/unlock timing กับ share handshake

### 7.4 Phase 4 — SyncBundle + cross-device evidence (Bl1nk repo, ~4 weeks)

**Goal:** Sync metadata + selected encrypted payload ข้าม Windows/Linux devices ได้ พร้อม conflict engine

**Work packages:**

- BX-08 (upstream) Add `sync` module: SyncChange codec + VectorClock + SyncConflict
- WP-T7-11 Implement conflict engine (T7 Sync)
- WP-T7-12 Implement sync UI (T7 + T5)
- WP-T8-13 Cross-device two-device matrix test (T8 QA)

**Verification gate:**

- 2 devices sync metadata + selected encrypted payloads + conflict resolution ทำงาน
- credential sync เป็น opt-in (default = false per proto/sync.proto)
- offline + reconnect resume ทำงาน
- concurrent edits → conflict inbox ไม่ silent overwrite

**Exit criteria:**

- Conflict engine ไม่ overwrite เงียง
- Sync metadata + encrypted payload ทำงาน
- Credential sync opt-in แยก

**Risks:**

- Vector clock skew ข้าม NTP-unreliable networks
- SyncBundle encryption overhead — bundle size growth

### 7.5 Phase 5 — Production hardening + cross-machine evidence (~3 weeks)

**Goal:** ทุกอย่างพร้อมสำหรับ release

**Work packages:**

- WP-T8-14 Cross-machine evidence tests (live signaling + STUN/TURN)
- WP-T8-15 Threat review + security checklist
- WP-T8-16 Crash tests + recovery scenarios
- WP-T4-17 Performance benchmarks
- WP-T4-18 Documentation + onboarding

**Verification gate:**

- Cross-machine (2 different machines, different networks) share envelope ทำงาน
- Threat review report พร้อม
- Performance: 100 KB envelope ≤ 5s end-to-end (LAN); ≤ 30s (WAN with relay)
- Crash recovery: disconnect mid-transfer → reconnect resume ไม่ corrupt

**Exit criteria:**

- Production-ready + cross-machine evidence published
- Release notes + changelog + tag
- Run `scripts/release_gate.sh` (blnk repo) + equivalent for Bl1nk

**Risks:**

- External signaling provider reliability
- NAT traversal failures — TURN relay cost
- Cross-platform PTY/file path bugs (Windows vs Linux)

### 7.6 Phase Gates Summary

| Phase | Goal | Verification Gate | Release | Exit if |
|---|---|---|---|---|
| P1 | blnk lib API hardening | `cargo test` + `cargo doc` + downstream integration test | blnk v0.3.0 | downstream depend ได้ |
| P2 | transport-blnk skeleton | smoke shell + file ในเครื่องเดียว | Bl1nk pre-release | smoke ผ่าน |
| P3 | ShareEnvelope E2E | 2 devices send/receive/apply | Bl1nk Phase 3 release | receipt + access code + negative cases ผ่าน |
| P4 | SyncBundle | 2 devices sync + conflict | Bl1nk Phase 5 release | conflict engine ไม่ silent overwrite |
| P5 | Production hardening | cross-machine + threat review + benchmarks | Bl1nk Phase 5 GA | evidence published |

---

## 8. Required Changes to blnk Rust Repo (upstream contributions)

> รายการ PR ที่ต้อง contribute กลับ `bl1nk-bot/blnk@main` เพื่อให้ Phase 1 ของ roadmap เสร็จ — ไม่ใช่ fork

| ID | Change | Purpose | Affects |
|---|---|---|---|
| **BX-01** | `[package].publish` policy + semver tag discipline | downstream `cargo add blnk` ทำงาน + version coupling | `Cargo.toml` |
| **BX-02** | `pub use` facade at crate root for transport subset | ergonomics: `use blnk::SignalingClient` | `src/lib.rs` |
| **BX-03** | Doc comments `///` ครบทุก public type ใน signaling/peer/session/protocol/stream/identity/vault | downstream DX + IDE help | `src/signaling/`, `src/peer/`, `src/session/`, `src/protocol/`, `src/stream/`, `src/identity/`, `src/vault/` |
| **BX-04** | `TwoPeerHarness` + `RelayFixture` gate `#[cfg(feature = "test-harness")]` แล้ว export | downstream integration test reuse | `tests/common/` (move to `src/test_utils/`?) |
| **BX-05** | New `share` module: envelope codec + fragment/reassemble helper | Bl1nk Phase 3 envelope transport | `src/share/` (new), `Cargo.toml`, `proto/share.proto` (already exists) |
| **BX-06** | Typed `Progress`/`Cancel` events ใน `StreamHandler` trait | Bl1nk progress UI | `src/stream/handler.rs` |
| **BX-07** | Typed `ShareEvent` codec on top of SWSP control stream | Bl1nk receipt flow | `src/session/runtime.rs` + new control opcode |
| **BX-08** | `sync` module: SyncChange codec + VectorClock + SyncConflict + cursor protocol | Bl1nk Phase 5 sync | `src/sync/` (new), `proto/sync.proto` (already exists) |
| **BX-09** | Add `access_code_verifier` trait + Argon2id adapter (Bl1nk-contributed) | P3-02 share access code | `src/session/auth.rs` |
| **BX-10** | `Identity` constructor variant ที่รับ externally-managed key material (Bl1nk Vault owns it) | trust boundary clean | `src/identity/key.rs` |
| **BX-11** | `SWSP MAX_PAYLOAD_LEN` config exposed + larger chunk size option (ถ้าจำเป็น) | larger envelopes | `src/protocol/swsp.rs` |
| **BX-12** | `SenderReceipt` trait + delivery confirmation callback | Bl1nk receipt | `src/session/runtime.rs` |

### 8.1 Implementation note

- แต่ละ BX ต้องเป็น PR แยกที่ target `main` ของ blnk repo
- PR gate per `AGENTS.md`: semantic version + Cargo.lock + CHANGELOG entry + `Closes #<issue>`
- ถ้า blnk upstream ไม่ accept บาง BX — Bl1nk ต้อง fork + maintain patch หรือ implement ฝั่ง Bl1nk เอง (ตาม decision ใน Section 12)

---

## 9. Required Changes to Bl1nk Repo

### 9.1 New crate: `transport-blnk`

```text
crates/transport-blnk/
├── Cargo.toml                  # depends on blnk via git + tag
├── src/
│   ├── lib.rs
│   ├── error.rs                # TransportError enum
│   ├── device.rs               # Bl1nkDeviceRegistry ↔ blnk::DeviceRegistry
│   ├── signaling.rs            # thin wrapper over SignalingClient
│   ├── session.rs              # thin wrapper over SessionRuntime + access code
│   ├── stream.rs               # shell + file + proxy
│   ├── envelope.rs             # ShareEnvelope codec + fragment/reassemble
│   ├── verifier.rs             # Argon2id access code verifier + state machine
│   ├── event.rs                # ShareEvent channel → blnk share service
│   └── progress.rs             # Progress/Cancel/Receipt callbacks
├── tests/
│   ├── two_peer_smoke.rs       # use blnk::test_utils::TwoPeerHarness
│   ├── relay_pair.rs           # use blnk::test_utils::RelayFixture
│   └── envelope_round_trip.rs  # 100 B / 16 KB / 100 KB / 1 MB
└── fixtures/
```

### 9.2 IPC contract: Tauri commands

```rust
// Tauri commands exposed to UI
#[tauri::command]
async fn send_share_envelope(
    state: tauri::State<AppState>,
    request: SendShareRequest,
) -> Result<ShareReceipt, TransportError>;

#[tauri::command]
async fn receive_share_envelope(
    state: tauri::State<AppState>,
    request: ReceiveShareRequest,
) -> Result<ShareEnvelope, TransportError>;

#[tauri::command]
async fn list_devices(state: tauri::State<AppState>) -> Result<Vec<DeviceRecord>, TransportError>;

#[tauri::command]
async fn open_session(
    state: tauri::State<AppState>,
    request: OpenSessionRequest,
) -> Result<SessionHandle, TransportError>;
```

### 9.3 Integration points กับ Application Services ที่มีอยู่

| Service | Integration | Note |
|---|---|---|
| `share` (T4) | consume `ShareEvent` channel + drive UI | map state machine |
| `vault` (T2) | `encrypted_payload` เก็บเป็น opaque Vault blob + `payload_hash` index | reuse `VaultRecord` schema |
| `device_registry` (T1/T4) | sync with `Bl1nkDeviceRegistry` ↔ `blnk::DeviceRegistry` | bidirectional |
| `sync` (T7) | consume `SyncChange` events | Phase 4 |
| `desktop` UI (T5) | consume `Progress`/`Cancel`/`ShareEvent` events | render real-time |

### 9.4 New modules in Bl1nk (Bl1nk-only, ไม่ต้องการ blnk change)

| Module | Purpose |
|---|---|
| `transport-blnk::verifier` | Argon2id verifier + state machine (per-share) |
| `transport-blnk::envelope` | envelope codec (ถ้า BX-05 ยังไม่ผ่าน upstream review) |
| `transport-blnk::event` | ShareEvent channel (ถ้า BX-07 ยังไม่ผ่าน) |
| `transport-blnk::progress` | Progress tracking (ถ้า BX-06 ยังไม่ผ่าน) |

### 9.5 Data flow

```text
Bl1nk Desktop UI
  ↓ (Tauri command, typed)
Application Service (share, sync, vault)
  ↓ (typed Rust API call)
transport-blnk crate
  ↓ (typed Rust API call, no IPC)
blnk Rust crate
  ↓ (WebSocket JSON / WebRTC data channel)
External signaling server / remote peer
```

---

## 10. Security & Privacy Boundary

### 10.1 Material Classification (per `CONTEXT.md` + `STYLE.md`)

| Material | Storage | Logging | Transport |
|---|---|---|---|
| PIN (6 bytes) | ไม่ persist; in-memory only | **never** log | constant-time compare |
| `pairing_code` (6-digit decimal) | regenerate per session load | never log in plaintext | in signaling offer/answer |
| `access_code` (11-char base64url) | persist ใน Bl1nk Vault (encrypted) | never log in plaintext | pass เข้้า `SessionRuntime` |
| RSA private key (2048-bit) | persist ใน Bl1nk Vault | never log | used for sign/decrypt in-process |
| `bootstrap_token` (web control) | process-local only | never log | HTTP `Authorization: Bearer` |
| `encrypted_payload` (opaque) | Vault (T2) | never log plaintext | over SWSP as opaque bytes |
| Session token | process-local only | never log | SWSP control stream 0 |

### 10.2 Trust Boundaries

| Boundary | Enforcement |
|---|---|
| Tauri ↔ Backend | typed commands + structured errors (Section 9.2) |
| transport-blnk ↔ blnk | single-process, Rust type system (no separate boundary) |
| Bl1nk Vault ↔ blnk private key | BX-10 proposed: blnk รับ Identity จาก Bl1nk constructor injection |
| blnk ↔ signaling | TLS via wss:// (production); ws:// only for local fixture |
| blnk ↔ peer | DTLS via WebRTC crate; SWSP over data channel |
| Sender ↔ Receiver envelope | signature + payload_hash + expires_at + trust_policy + access_code |

### 10.3 Audit / Event Log Strategy

| Event | Sink | Format |
|---|---|---|
| `ShareEvent::Created/Offered/Opened/Verified/Consumed/Cancelled/Expired/Failed` | Bl1nk audit log (T2) | structured JSON |
| blnk `tracing` events | tracing subscriber → structured log | span-scoped (peer_id, session_id, stream_id) |
| Sign in/out | Bl1nk audit log | structured |
| PIN failed attempts | blnk `tracing` (counter, never value) + Bl1nk audit log | counter only |
| Device added/removed | Bl1nk audit log | structured |
| Vault unlock/lock | Bl1nk audit log | structured |

### 10.4 No-go list (mandatory)

- ❌ Plaintext PIN / access_code / nonce / ciphertext ใน log
- ❌ Private key ใน log หรือ panic message
- ❌ Session token ใน URL หรือ query parameter
- ❌ Secret ใน screenshot / test artifact
- ❌ Panic as error flow
- ❌ Short-circuit comparison on auth material

---

## 11. Testing Strategy

### 11.1 Unit tests (transport-blnk)

- `envelope.rs` — encode/decode round-trip + integrity verify (SHA-256, signature, expires_at)
- `envelope.rs` — fragment/reassemble 100 B / 16 KB / 100 KB / 1 MB / 10 MB
- `envelope.rs` — corruption mid-fragment → rejected
- `envelope.rs` — out-of-order frames → rejected
- `envelope.rs` — duplicate frames → rejected
- `verifier.rs` — Argon2id verify + constant-time + 5-attempt lockout + TTL expiry + one-time replay guard
- `progress.rs` — bytes-counted progress + cancel mid-transfer
- `error.rs` — typed error mapping (BlnkError → TransportError → ServiceError)

### 11.2 Integration tests (reuse blnk fixtures)

- `tests/two_peer_smoke.rs` — `use blnk::test_utils::TwoPeerHarness` + transport-blnk wrappers + open shell + open file + verify bytes
- `tests/relay_pair.rs` — `use blnk::test_utils::RelayFixture` + signaling connect + register + pair + open session
- `tests/envelope_round_trip.rs` — send envelope (N objects) → reassemble → verify signature → receipt callback

### 11.3 End-to-end tests

- `tests/e2e_send_share.rs` — Bl1nk desktop (programmatic) ↔ transport-blnk ↔ blnk ↔ `TwoPeerHarness` ↔ Bl1nk desktop → ShareEvent delivered
- `tests/e2e_negative.rs` — expired share, wrong access code, corrupt signature, one-time replay, signature mismatch, payload_hash mismatch
- `tests/e2e_cancel.rs` — sender cancels mid-transfer + receiver drops reassembly
- `tests/e2e_large_envelope.rs` — 1 MB envelope (file reference inside) → fragment + reassemble + integrity

### 11.4 Cross-machine evidence (production gate)

- 2 physical machines, different networks, same Bl1nk release
- External signaling server (production provider)
- STUN/TURN configured per machine network
- Steps: announce → discover → pair → send envelope → receive → apply → receipt → revoke
- Expected: envelope delivered + verified + applied + receipt delivered ภายใน 30s (LAN), 60s (WAN with relay)
- Failure modes: NAT failure → TURN fallback; signaling down → reconnect; network partition → resume
- **Status:** Phase 5 deliverable, ยังไม่พิสูจน์ในปัจจุบัน (gap)

### 11.5 Adversarial tests

- Replay attack (one-time envelope sent twice)
- Wrong access code (5 attempts → lockout)
- Corrupt signature (random byte flip)
- Corrupt payload_hash
- Expired envelope
- Revoked envelope (sender revoke หลั� send)
- Man-in-the-middle (intercept signaling + replay)
- Timing attack on PIN (constant-time must hold)
- Path traversal in file stream (`../../etc/passwd`)
- Null byte in file path (`/etc/passwd\0`)
- Proxy SSRF (private IP, localhost, link-local)

### 11.6 Performance benchmarks

- Envelope encode/decode: < 100 ms for 1 MB
- Fragment: < 10 ms per chunk
- Reassemble: < 50 ms per chunk
- Hash + verify: < 200 ms for 1 MB payload
- End-to-end envelope (100 KB): < 5 s LAN, < 30 s WAN with relay
- Memory: < 100 MB working set during 1 MB envelope transfer

---

## 12. Open Risks & Decisions Needed

### 12.1 Risks (8 items with mitigation)

| # | Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|---|
| **R1** | blnk lib API breaking change (no semver guarantee yet) | downstream integration break | high in v0.x | pin tag + Cargo.lock; contribute semver policy ใน Phase 1 (BX-01); Bl1nk fork fallback ถ้า reject |
| **R2** | Cross-machine original-Go/browser interop unproven | production deployment uncertainty | high | Phase 5 cross-machine evidence tests (WP-T8-14); explicit "NOT supported" claim จนกว่าจะพิสูจน์ |
| **R3** | SWSP max payload 16 KB ต้อง fragment ทุก envelope | complexity + memory pressure for large payloads | certain | fragment + reassemble + integrity verify in transport-blnk |
| **R4** | Sender-side receipt ต้องเพิ่ม application protocol | extra SWSP control opcode + state machine | medium | use SWSP control stream 0 + dedicated opcode range (0x10–0x1F); BX-07 upstream contribution |
| **R5** | Access-code verifier state machine ใหม่ทั้งหมด | ใช้เวลา + bug-prone | medium | pure Bl1nk-side implementation; reuse blnk session attempt/lockout pattern as reference; comprehensive unit tests |
| **R6** | Multi-device concurrent session pool (Phase 5+) | resource exhaustion | medium | bounded session pool + queue + per-device rate limit |
| **R7** | Vault lock/unlock timing กับ share handshake | user experience + security | medium | design lock state machine ให้ explicit; reject share handshake ถ้า Vault locked |
| **R8** | Long-tail compatibility bugs (Windows + Linux + Android PTY, path, line ending) | Phase 5 release delay | high | matrix testing from Phase 2; defer Android to Phase 8+ per roadmap |

### 12.2 Decisions ที่ต้อง escalate

| # | Decision | Owner | Consulted | Recommendation |
|---|---|---|---|---|
| **D1** | BX-01: ตกลง semver policy ใน blnk repo (currently pre-1.0, breaking changes allowed) | T8 blnk maintainer + T1 Bl1nk | all Bl1nk teams | freeze API ใน 0.3.0, guarantee semver from 1.0.0 |
| **D2** | BX-04: ย้าย `TwoPeerHarness` + `RelayFixture` เข้า `src/test_utils/` หรือเก็บใน `tests/common/` | T8 blnk maintainer | T4 Bl1nk | move to `src/test_utils.rs` gated `#[cfg(feature = "test-harness")]` for downstream reuse |
| **D3** | BX-05: ShareEnvelope codec อยู่ blnk หรือ Bl1nk | T4 Bl1nk + T8 blnk | T1, T2 | upstream — reuse Bl1nk's envelope codec module as starting point |
| **D4** | BX-07: ShareEvent opcode namespace conflict กับ SessionRuntime control messages | T4 Bl1nk + T8 blnk | T8 | dedicated range 0x10–0x1F; document in PROTOCOL.md |
| **D5** | BX-09: Argon2id verifier ใน blnk หรือ Bl1nk | T4 + T2 + T8 | T1 | Bl1nk-only — blnk core ไม่ควรรู้จัก Argon2id (transport concern, not crypto primitive concern) |
| **D6** | BX-10: Identity injection vs Bl1nk-managed key | T8 + T1 + T2 | T4 | upstream contribution เพิ่ม Identity::from_external_keypair() constructor |
| **D7** | Phase 3 signaling provider: self-hosted local relay vs external production | T5 + T8 | T4 | local relay สำหรับ dev/test, external production สำหรับ release |
| **D8** | When to start Android/iOS/macOS native | T0 Product | T1, T4, T5 | defer to Phase 8+ per roadmap; maintain abstraction boundaries |

---

## 13. Evidence Index

| Claim | Source |
|---|---|
| blnk Cargo.toml has [lib] + [[bin]] | `Cargo.toml:9-15` |
| src/lib.rs exposes 12 public modules | `src/lib.rs:1-13` |
| `BlnkError` 8 variants | `utils/error.rs` |
| `SignalingClient` API surface | `signaling/transport.rs:512-624` |
| `PeerHandle` API + non-trickle SDP | `peer/mod.rs:33-396` |
| Session PIN + state machine | `session/runtime.rs:298-356` |
| SWSP 8-byte LE header | `protocol/swsp.rs:21` |
| `DEFAULT_MAX_PAYLOAD_LEN = 16 * 1024` | `protocol/swsp.rs:21` |
| Frame flags SYN/FIN/DAT/MORE | `protocol/swsp.rs:24` |
| PIN 6 bytes + constant-time + 3 attempts | `session/mod.rs:131-144` + `subtle::ConstantTimeEq` |
| Identity RSA-2048 + uid + pairing_code + access_code | `identity/key.rs:34`, `CONTEXT.md:5-9` |
| Web control loopback-only + CSRF + cookie | `web/mod.rs` + Issue #47 |
| Proxy deny-by-default + null-byte rejection | `stream/proxy.rs` |
| 134 tests v0.2.6 | `docs/implementation-status.md` |
| TwoPeerHarness + RelayFixture scope | `tests/compatibility_baseline.rs` + `tests/live_interop.rs` |
| Bl1nk roadmap Phase 3 work packages P3-01 .. P3-06 | `specs/blnk-product-roadmap.md:158-171` |
| Bl1nk roadmap Phase 5 P5-02 sync | `specs/blnk-product-roadmap.md:225-239` |
| Bl1nk team topology T0-T8 | `specs/blnk-product-roadmap.md:114-216` |
| Bl1nk repository target structure 4.3 | `specs/blnk-product-roadmap.md:75-91` |
| Contract C-01 Object Manifest | `specs/blnk-product-roadmap.md:151-167` |
| ShareEnvelope schema | `proto/share.proto:24-39` |
| ShareChannel.P2P = 6 | `proto/share.proto:6-15` |
| TrustPolicy enum | `proto/share.proto:17-25` |
| Object schema | `proto/object_model.proto` |
| SyncChange schema | `proto/sync.proto` |
| AGENTS.md canonical context list | `AGENTS.md` |
| Project delivery lifecycle | `AGENTS.md` + `docs/agent-operations.md` |
| STYLE.md non-negotiable rules | `STYLE.md` |
| Implementation status v0.2.6 | `docs/implementation-status.md` |

### URL อ้างอิงทั้งหมด

- blnk repo root: https://github.com/bl1nk-bot/blnk
- spec.md: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/specs/spec.md
- implementation-status.md: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/docs/implementation-status.md
- architecture.md: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/docs/architecture.md
- api.md: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/docs/api.md
- blueprint.md: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/docs/blueprint.md
- protocol.md: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/PROTOCOL.md
- style.md: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/STYLE.md
- context.md: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/CONTEXT.md
- proto/share.proto: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/proto/share.proto
- proto/swsp.proto: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/proto/swsp.proto
- proto/object_model.proto: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/proto/object_model.proto
- proto/sync.proto: https://raw.githubusercontent.com/bl1nk-bot/blnk/main/proto/sync.proto

### Upstream research deliverables

- Track A (this spec): `.mavis/plans/blnk-transport-profile/deliverables/track-a-blnk-transport-surface.md`
- Track B (this spec): `.mavis/plans/blnk-transport-profile/deliverables/track-b-bl1nk-transport-requirements.md`

---

## 14. Glossary

| Term | Definition |
|---|---|
| **SWSP** | Stream Wire Session Protocol — binary framing over WebRTC data channels; 8-byte LE header (`stream_id` 4B + `flags` 2B + `length` 2B) + payload max 16 KB conservative |
| **SignalingClient** | blnk public type — WebSocket JSON transport ไปยัง signaling server; preflight `EndpointPolicy::PublicOnly` |
| **PeerHandle** | blnk public type — wraps WebRTC `RTCPeerConnection` + data channel + SWSP frame queue |
| **SessionRuntime** | blnk public type — finite state machine wrapper เชื่อม `Session` ↔ `PeerHandle` (SWSP stream 0); PIN auth with constant-time compare + attempt limit |
| **ShareEnvelope** | `blnk.share.ShareEnvelope` protobuf message — versioned manifest of N ObjectRecord + opaque encrypted_payload + signature + policy |
| **transport-blnk** | Bl1nk-side crate — Rust facade wrapping blnk + envelope codec + access code verifier + ShareEvent channel |
| **TwoPeerHarness** | blnk test fixture — deterministic in-process WebRTC pair (offerer + answerer) with loopback UDP |
| **RelayFixture** | blnk test fixture — local WebSocket signaling relay for integration tests |
| **identity** | blnk `Identity` struct — RSA-2048 keypair + derived uid + pairing_code (6-digit) + access_code (11-char base64url) |
| **access_code** | 11-char URL-safe-no-pad base64 of 8 random bytes; persistent credential แยกจาก pairing_code |
| **pairing_code** | 6-digit decimal SAS; user-visible; fresh per PEM load; not persisted in PEM format |
| **Frame** | SWSP frame — `{stream_id, flags, payload}` |
| **FrameFlags** | bitflags: SYN (0x01), FIN (0x04), DAT, MORE (0x02); unknown bits rejected |
| **CONTROL_STREAM_ID** | SWSP stream 0 — reserved for `SessionRuntime` control messages |
| **ShareEvent** | `blnk.share.ShareEvent` message — status transitions (Created/Offered/Opened/Verified/Consumed/Cancelled/Expired/Failed) |
| **ShareStatus** | `blnk.share.ShareStatus` enum — Created → Offered → Opened → Verified → Consumed/Revoked/Expired/Failed |
| **TrustPolicy** | `blnk.share.TrustPolicy` enum — Local / PrivateDevice / SelectedRecipient / LinkBearer / Delegated |
| **ShareChannel** | `blnk.share.ShareChannel` enum — LocalFile / Clipboard / QR / DeepLink / HTTPS / **P2P = 6** / Sync / API |
| **Sensitivity** | `blnk.common.Sensitivity` — Public / Internal / Sensitive / Secret (Secret payloads ห้าม enter FTS/audit/URL/QR) |
| **TwoPeerHarness** | deterministic in-process WebRTC pair; proves local behavior only — not external signaling, NAT, or cross-machine |
| **endpoint_policy** | `EndpointPolicy` enum — `PublicOnly` (default) / `AllowLocal` (fixture/test) |
| **Bl1nk** | "Bl1nk Universal Transfer & Secure Workspace" — Tauri desktop application; main consumer ของ blnk transport |
| **Bl1nkDeviceRegistry** | Bl1nk-side device registry abstraction; bridges to blnk `DeviceRegistry` via transport-blnk |
| **persistent nonce** | single-use UUID v4 nonce for encrypted session request validation; cleared on timeout/disconnect/failure |
| **TURN** | Traversal Using Relays around NAT — ICE server type for fallback relay |
| **STUN** | Session Traversal Utilities for NAT — ICE server type for NAT discovery |

---

> สิ้นสุดแผนงาน — implementation ทุก phase ต้องทำ PR per `AGENTS.md` lifecycle และ update canonical docs เมื่อข้อเท็จจริงเปลี่ยน
