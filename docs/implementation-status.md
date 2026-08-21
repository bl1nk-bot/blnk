# Implementation Status and Readiness

**ตรวจสอบฐาน:** branch งานนี้ต่อจาก foundation implementation ใน Issue #5, identity/pairing primitives ใน Issue #7 และ SWSP raw frame codec ใน Issue #9
**สถานะเอกสารฉบับนี้:** foundation runnable, identity/pairing primitives, SWSP raw frame codec, typed signaling/session/stream boundaries, deterministic protobuf generation boundary, local WebSocket signaling transport, WebRTC peer lifecycle สำหรับ local two-peer harness, authenticated session runtime, sandboxed file-transfer MVP, shell stream MVP และ proxy policy/concrete handler boundaries ถูก implement ใน dependency branches แล้ว; CLI local fixture workflow มีหลักฐานจริง แต่ remote signaling orchestration, external interoperability fixtures และ production readiness ยังไม่เสร็จ

## Executive Summary

blnk Rust **มี runnable local MVP ตามสถาปัตยกรรมแล้ว** และ Issue #7 เพิ่ม identity/pairing primitives, Issue #9 เพิ่ม SWSP raw frame codec, Issue #11 เพิ่ม typed signaling/session/stream boundaries, Issue #13 เพิ่ม deterministic protobuf generation boundary, Issue #33 เพิ่ม JSON-over-WebSocket transport กับ local loopback fixture ที่ทดสอบได้, Issue #35 เพิ่ม public-only signaling endpoint preflight พร้อม explicit local-fixture opt-in, Issue #37 เพิ่ม WebRTC offer/answer, non-trickle ICE, data-channel lifecycle และ deterministic local two-peer harness, Issue #38 เพิ่ม authenticated session runtime, Issue #39 เพิ่ม sandboxed file-transfer MVP, Issue #40 เพิ่ม shell stream MVP, Issue #41 เพิ่ม proxy security policy และ Issue #42 เพิ่ม concrete TCP/WebSocket/HTTP handler services บน policy boundary; Issue #43 เพิ่ม CLI local fixture orchestration และ metadata-only device registry แต่ยังไม่พร้อมใช้งานจริงหรือ deploy ระบบหลัก เพราะ remote signaling orchestration, external interoperability fixtures, TLS/provider coverage, web integration และ cross-platform release evidence ยังขาด [1] [2]

การมี CLI command, dependency หรือ protobuf schema ไม่ถือเป็นการผ่าน acceptance criterion จนกว่าจะมี business logic, protocol compatibility tests และ end-to-end evidence รองรับ

## Evidence Matrix

| พื้นที่ | สิ่งที่มีอยู่ใน repository | สถานะ |
|---|---|---|
| CLI surface | มี `serve`, `connect`, `cp`, `devices`, `version` ใน `src/main.rs`; `serve --local-fixture`, `connect --local-fixture` และ `cp --local-fixture` เรียก local authenticated fixture จริง ส่วน `connect --target` และ remote `cp` รายงานข้อจำกัดอย่าง explicit | Local MVP implemented; remote orchestration pending |
| Architecture | มี module layout, data flow, runtime model, security และ testing principles [4] | Design ready |
| Protocol design | มี `.proto` สำหรับ signaling, identity, pairing, control, stream และ SWSP [5] และ `build.rs` สร้าง bindings ใต้ `crate::proto_generated`; มี raw SWSP codec แยกใน `src/protocol/swsp.rs` | Schema plus generated/raw codec boundaries |
| Library foundation | มี `src/lib.rs`, module boundaries, typed errors, config loader และ CLI routing ที่แยก local fixture กับ remote capability boundary [4] [6] | Implemented with local CLI workflow |
| Identity and pairing | มี RSA 2048 identity generate/load/save, sign/verify, OAEP encrypt/decrypt, 6-digit `pairing_code`, `access_code`, nonce, commit-reveal และ provisional SAS ใน Issue #7 branch | Implemented; integration/fixture pending |
| SWSP codec | มี typed flags, canonical 8-byte little-endian header, max-payload enforcement, incomplete-frame handling และ round-trip/negative tests ใน Issue #9; Issue #37 เพิ่มการ encode/decode และส่งผ่าน WebRTC data channel ใน local harness; ยังไม่มี upstream interoperability fixture | Implemented with local WebRTC integration; interoperability pending |
| Signaling boundary | Issue #11 มี typed register/request/offer/answer/candidate/pairing/error messages และ protocol-version validation; Issue #13 เพิ่ม generated protobuf bindings ใต้ namespace แยก; Issue #33 เพิ่ม JSON-over-WebSocket codec/client, message-size guard, close/error propagation, reconnect policy และ local loopback fixture tests; Issue #35 เพิ่ม default public-only endpoint preflight, DNS/IP special-use rejection และ explicit local-fixture opt-in; Issue #37 ใช้ SDP offer/answer แบบ non-trickle ใน process เป็น local harness boundary | Local transport, endpoint boundary and local peer negotiation implemented; external interoperability pending |
| Peer lifecycle | Issue #37 มี `PeerHandle` สำหรับสร้าง peer, offer/answer, non-trickle ICE gathering, connected/failed/closed state, data-channel open/error/close, SWSP send/receive และ idempotent teardown; `TwoPeerHarness` พิสูจน์ local loopback path โดยไม่ใช้ external network; Issue #38 ใช้ `PeerHandle` เป็น control/data boundary ของ authenticated session runtime และ bounded graceful close | Local peer/session lifecycle implemented; signaling and external interoperability pending |
| Session/auth | Issue #11 มี explicit connecting/authenticating/ready/closed state machine, PIN retry/delay policy, typed control messages และ cleanup on terminal failure; Issue #38 เพิ่ม `SessionRuntime` เชื่อม protobuf control messages กับ `PeerHandle`; Issue #39 ใช้ runtime boundary เดิมสำหรับ file request/data/FIN; Issue #43 ใช้ handshake จริงใน CLI local fixture | Local runtime/file/CLI integration implemented; original-client interoperability pending |
| Stream registry | Issue #11 มี non-zero stream ID allocation, lifecycle validation, counters และ cleanup; Issue #38 ผูก `open_stream`/`close_stream`/disconnect กับ runtime snapshot; Issue #39 เพิ่ม file streams; Issue #40 เพิ่ม shell streams; Issue #43 ใช้ทั้งสองผ่าน local CLI transcript | Registry/runtime/file/shell/CLI local lifecycle implemented; remote dispatch pending |
| Protobuf build | Issue #13 มี `build.rs`, vendored `protoc`, explicit schema input list และ generated modules ใต้ `src/proto_generated.rs` พร้อม encode/decode compile test [6] [7] [9] [10] | Implemented; interoperability pending |
| Build | `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo test --all` และ `cargo clippy --all --all-targets -- -D warnings` ผ่านบน Linux หลังแก้ dependency table scope [7] | Passing on Linux |
| CI | มี jobs สำหรับ fmt, clippy และ test แต่ไม่มี cross-platform matrix หรือ release workflow ใน repository ปัจจุบัน [8] | Partial |
| Tests | มี unit tests สำหรับ config, identity, pairing, SWSP, signaling validation, session/auth lifecycle และ stream registry; Issues #33/#37/#38 เพิ่ม local transport, WebRTC, authenticated runtime และ failure-path coverage; Issue #39 เพิ่ม sandboxed file-transfer; Issue #40 เพิ่ม shell lifecycle/cancellation/limits; Issue #43 เพิ่ม CLI arg parsing, direct-argv safety, local authenticated shell/file transcripts และ metadata registry path; ล่าสุด `cargo test --all` ผ่าน 75 tests | Foundation plus local transport/peer/session-runtime/file/shell/CLI coverage; external interoperability pending |
| Release | ยังไม่มี binary artifact, checksum หรือ verified Linux/Windows/Android build | Not started |

## Acceptance Gates

โครงการผ่าน gate ของ runnable foundation แล้วเมื่อ `src/lib.rs`, error/config/CLI wiring, identity boundary และ test harness ถูกสร้างขึ้น และคำสั่งต่อไปนี้ผ่านบน Linux:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all
cargo clippy --all --all-targets -- -D warnings
```

โครงการจะถือว่าผ่าน specification ก็ต่อเมื่อ signaling, WebRTC data channel, pairing/PIN, shell, file, proxy, TCP และ WebSocket flow ทำงานได้จริง พร้อม test ครอบคลุมส่วนสำคัญ และ build ได้บน Linux, Windows และ Android ตาม scope ที่ประกาศไว้ [2]

## Canonical Decisions

ลำดับความสำคัญเมื่อเอกสารขัดแย้งกันคือ `docs/architecture.md` > `specs/spec.md` > `docs/api.md` > `STYLE.md` > `README.md` > `TODO.md` ตาม foundation plan [6]

| หัวข้อ | การตัดสินใจที่ใช้ต่อจากนี้ |
|---|---|
| Rust toolchain | ใช้ Rust 2024 และ toolchain ตาม `rust-toolchain.toml`/container configuration; ไม่ใช้ข้อความ edition 2021 จาก roadmap เก่า |
| User-visible pairing code | ใช้ pairing code 6 หลักตาม functional specification |
| Persistent access credential | หากยังต้องมี credential ภายใน 64-bit ให้ใช้ชื่อ `access_code` แยกจาก pairing code และห้ามเรียกปนกันว่า `code` |
| SWSP | `Frame` raw wire format เป็น header 8 bytes ตาม specification ของ SWSP: `stream_id` 4 bytes, `flags` 2 bytes, `length` 2 bytes, ตามด้วย payload; protobuf ใช้เป็น schema/control representation เท่านั้นจนกว่าจะมี compatibility fixture ยืนยันอย่างอื่น |
| Protocol compatibility | ห้ามเปลี่ยน signaling, pairing, identity หรือ SWSP semantics เพื่อให้ implement ง่ายขึ้น; หากจำเป็นต้องเปลี่ยนต้องมี decision record และ fixture จากต้นฉบับ |
| Signaling transport scope | Issue #33 ใช้ JSON text frames บน WebSocket; adapter แปลงชื่อภายใน `message_type`/`pairing_code` เป็น wire schema `type`/`code`; Issue #35 ใช้ `EndpointPolicy::PublicOnly` เป็นค่าเริ่มต้นและให้ local fixture ใช้ `EndpointPolicy::AllowLocal` อย่าง explicit; policy นี้เป็น application-level preflight ไม่ใช่ OS/network egress firewall; local fixture พิสูจน์เฉพาะ local request/response และ failure handling ไม่ใช่หลักฐาน original-client หรือ provider interoperability |
| WebRTC peer scope | Issue #37 ใช้ local loopback UDP และ `RTCConfiguration` ที่ไม่มี hardcoded STUN/TURN; production caller ต้องเป็นผู้แปลงค่า ICE server จาก config/signaling เอง; implementation นี้ไม่อ้าง external NAT traversal, browser compatibility หรือ TURN availability |
| Pairing SAS | Issue #7 ใช้ deterministic provisional construction จาก nonce และ DTLS fingerprints; ต้องยืนยัน exact upstream encoding ด้วย interoperability fixture ก่อนผูกเข้ากับ client/server จริง |
| Service worker | ไม่สร้าง service worker ใหม่ใน Rust; frontend/service worker เดิมอยู่นอก scope ตาม specification |
| Platform scope | รองรับ Linux, Windows และ Android; macOS อยู่นอก scope |

## Known Risks

ความเสี่ยงสูงสุดคือ interoperability กับ client/browser และ signaling server เดิม เพราะ schema ที่มีอยู่ยังไม่ได้ถูกเชื่อมเข้ากับ Rust codec หรือ end-to-end tests การ compile ผ่านเพียงอย่างเดียวจึงไม่เพียงพอที่จะยืนยันว่า protocol ใช้งานร่วมกับต้นฉบับได้

ความเสี่ยงรองลงมาคือ cross-platform behavior ของ PTY, filesystem, networking และ Android packaging รวมถึง security boundary ของ file path, TCP target, PIN retry, key persistence, signaling endpoint egress และ resource limits ก่อนเปิดใช้งานจริงต้องมี negative tests และ audit evidence สำหรับขอบเขตเหล่านี้ โดย `EndpointPolicy` ไม่ได้ทดแทน OS/network egress controls

## Recommended Implementation Order

| ลำดับ | งาน | Definition of done |
|---:|---|---|
| 1 | แก้ dependency scope และสร้าง library/build foundation | **ผ่านใน Issue #5:** คำสั่ง verification ทั้งสี่ผ่านบน Linux |
| 2 | เพิ่ม `build.rs`/protobuf generation หรือบันทึกเหตุผลที่เลือก hand-written codec | **ผ่านใน Issue #13:** generation deterministic และมี compile test; ยังเหลือ interoperability fixture |
| 3 | เพิ่ม typed errors, logging, config และ CLI routing | **ผ่านบางส่วนใน Issue #43:** local fixture commands เรียก business logic จริงและไม่มี stub output; remote signaling orchestration ยังเหลือ |
| 4 | Implement identity และ pairing | **อยู่ใน Issue #7:** primitives และ unit tests ผ่าน; เหลือ interoperability fixture และ integration กับ session/signaling |
| 5 | Implement SWSP/control codec | **อยู่ใน Issue #9:** round-trip, invalid frame, incomplete-frame และ max-size tests ผ่าน; เหลือ upstream interoperability fixture และการเชื่อมกับ data channel |
| 6 | Implement signaling และ WebRTC peer/data channel | **ผ่านบางส่วนใน Issue #37 และ #38:** local two-peer offer/answer, non-trickle ICE, SWSP data channel harness และ authenticated session runtime ผ่าน; Issue #39 ใช้ boundary เดิมส่ง file frames; ยังเหลือ external interoperability และ production NAT traversal |
| 7 | Implement session/auth และ stream registry | **ผ่านบางส่วนใน Issues #11, #38, #39, #40, #42 และ #43:** typed lifecycle, PIN retry/delay, protobuf control handshake, runtime stream cleanup, file/shell/proxy boundaries และ local CLI transcripts ผ่าน; เหลือ remote dispatch/interoperability tests |
| 8 | เพิ่ม shell/HTTP/TCP/WebSocket, mDNS และ QR | **ผ่านบางส่วนใน Issues #40 และ #42:** shell และ concrete proxy handlers มี local coverage; ยังเหลือ SessionRuntime dispatch, TLS-enabled interoperability, mDNS/QR และ platform notes |
| 9 | ทำ cross-platform, security, performance และ release validation | CI matrix ผ่าน, audit ไม่มี critical issue, artifacts ใช้งานได้ |

## References

[1]: ../README.md "Project scope and current README"
[2]: ../specs/spec.md "Functional and acceptance requirements"
[3]: ../src/main.rs "Current CLI entry point"
[4]: architecture.md "Architecture and module layout"
[5]: ../proto/ "Protocol schemas"
[6]: plans/core-foundation.md "Foundation plan and conflict resolution order"
[7]: ../Cargo.toml "Cargo manifest"
[8]: ../.github/workflows/ci.yml "Current CI workflow"
[9]: ../build.rs "Deterministic protobuf generation boundary"
[10]: ../src/proto_generated.rs "Namespaced generated protobuf bindings and compile smoke test"
