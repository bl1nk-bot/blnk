# ADR-046: Security Threat Model และ Runtime Hardening Audit

**สถานะ:** ใช้เป็น baseline ของ Issue #46 และต้องทบทวนเมื่อมีการเปลี่ยน protocol, stream capability, endpoint policy หรือ platform boundary
**ขอบเขต:** Linux, Windows และ Android ตาม support matrix; macOS อยู่นอกขอบเขต
**วันที่ตรวจสอบ:** 2026-08-21
**ผู้รับผิดชอบหลัก:** Maintainers ของ blnk Rust; finding ที่ระบุ `Release owner` ต้องปิดด้วยหลักฐานจาก CI หรือ interoperability fixture ก่อน release

## 1. วัตถุประสงค์และ non-claims

เอกสารนี้กำหนด threat model ที่ versioned สำหรับ local MVP ของ blnk Rust ซึ่งเปิด remote-access capability ผ่าน signaling, WebRTC data channel และ authenticated stream runtime โดย audit นี้ตรวจขอบเขตที่โค้ดควบคุมได้ ได้แก่ identity/pairing, signaling egress, session authentication, stream registry, file transfer, shell execution, proxy handlers, bounded response handling, resource limits, cleanup, log redaction และ platform-specific persistence

> เอกสารนี้ **ไม่ใช่การรับรองความปลอดภัยแบบสมบูรณ์** และไม่แทนที่ OS sandbox, filesystem ACL, host firewall, network egress policy, endpoint protection, TLS certificate policy หรือการ review ของ deployment environment

หลักฐาน local two-peer, local signaling fixture และ compatibility fixture ใช้ยืนยัน behavior ภายใน repository เท่านั้น ไม่ใช่หลักฐานว่าใช้งานร่วมกับ original-Go client, browser, signaling provider, STUN/TURN หรือ production NAT traversal ได้ [1] [2]

## 2. Assets, actors และ trust boundaries

| องค์ประกอบ | สิ่งที่ต้องปกป้อง | ขอบเขตความเชื่อถือ | ผลกระทบเมื่อถูกโจมตี |
|---|---|---|---|
| Identity | RSA private key, access credential, UID และ pairing code | local filesystem และ process owner | impersonation, credential disclosure และการเข้าถึง session |
| Signaling | endpoint URL, register/pairing messages และ WebSocket frame | signaling server และ DNS/network ภายนอกเป็น untrusted | SSRF, message injection, resource exhaustion และ metadata disclosure |
| WebRTC/session | DTLS data channel, control messages, PIN retry state และ stream registry | peer ที่ผ่าน transport แต่ยังไม่ authenticated ถือเป็น untrusted | session hijack, replay, unauthorized stream หรือ resource exhaustion |
| File stream | root directory, file bytes และ temporary upload | peer ไม่ได้รับสิทธิ์ filesystem โดยตรง; service เป็น policy boundary | path traversal, symlink escape, overwrite หรือข้อมูลรั่วไหล |
| Shell stream | executable, argv, cwd, environment และ output | peer ไม่ได้รับ shell interpreter โดยตรง; allowlist เป็น boundary | arbitrary process execution, environment leakage หรือ local DoS |
| Proxy stream | target URL, DNS answer set, redirects และ headers | remote target/network เป็น untrusted | SSRF, DNS rebinding, credential forwarding หรือ proxy abuse |
| Logs/errors | URL, headers, identifiers และ failure details | log sink อาจถูกอ่านโดยผู้ใช้อื่นหรือระบบรวม log | credential/token leakage และการเปิดเผย topology |

### Threat assumptions

ระบบถือว่า attacker สามารถส่ง signaling frames, pairing messages, control frames, stream requests, malformed paths, shell arguments, proxy URLs, redirect responses และ retry attempts ที่เลือกเองได้ ระบบไม่ถือว่า peer ที่เชื่อม WebRTC สำเร็จเป็น trusted จนกว่า control handshake และ PIN policy จะเปลี่ยน state เป็น `Ready` ผู้โจมตีที่มีสิทธิ์อ่านหรือแก้ไข process memory, parent directory ACL หรือ OS kernel อยู่นอกขอบเขตของ application-level controls นี้

## 3. Severity และ disposition

| ระดับ | ความหมายในเอกสารนี้ |
|---|---|
| Critical | ยึด private key หรือได้ arbitrary remote/local capability โดยไม่ต้องผ่าน policy |
| High | ข้าม authentication หรือ boundary สำคัญได้ หรือทำ SSRF/path/process escape ที่มีผลจริง |
| Medium | ทำให้เกิด disclosure, replay, resource exhaustion หรือ platform failure ภายใต้เงื่อนไขเฉพาะ |
| Low | ความเสี่ยงจำกัด, evidence gap หรือ behavior ที่ยังไม่พร้อม production แต่ไม่ข้าม MVP boundary |

`Mitigated` หมายถึงมี code path และ negative test ใน branch นี้แล้ว; `Open before release` หมายถึงยังต้องมี evidence หรือ control เพิ่มก่อนอ้าง production readiness; `Accepted for local MVP` ใช้เฉพาะ residual risk ที่ถูกจำกัดและประกาศไว้ ไม่ใช่ security waiver สำหรับ deployment จริง

## 4. Findings และ disposition

| ID | พื้นที่ / finding | Severity | Evidence ที่ตรวจพบ | Owner | Disposition |
|---|---|---:|---|---|---|
| TM-001 | Identity รับเฉพาะ RSA modulus 2048 บิต, ตรวจ access code/UID, สร้าง parent directory ด้วย mode `0700`, file ด้วย `0600` บน Unix และเขียนผ่าน temporary file ก่อน replace | High | `Identity::load`, `validate_rsa_key_size`, `ensure_private_parent`, `write_atomic` และ tests สำหรับ non-2048 key, permissions และ concurrent parent creation [3] | Identity maintainers | **Mitigated ใน branch นี้**; Windows ใช้ `MoveFileExW` แบบ replace/write-through แต่ ACL ของ parent ต้องมาจาก user-private location |
| TM-002 | Persistence ต้องไม่ถูกตีความว่า JSON เป็น upstream wire format; มี explicit JSON/PEM boundary และ loader ของ two-block PEM (`PRIVATE KEY`, `BITBANG ACCESS CODE`) พร้อม fixture | High | `load_auto`, `load_pem`, `save_pem`, `save_as` และ `tests/fixtures/bitbang_identity.pem`; ADR identity persistence ระบุว่า format-level fixture ยังไม่ใช่ end-to-end interoperability [3] [4] | Identity maintainers / Release owner | **Mitigated เป็น compatibility boundary**; default JSON ยังไม่ใช่ข้ออ้างว่า interoperable กับ original-Go และต้องตัดสินใจ migration ก่อน release |
| TM-003 | Pairing discriminator ใช้ wire key `type`, nonce ใช้ unpadded base64 และ commit/reveal บังคับ nonce 32 bytes; commitment comparison ใช้ constant-time equality | High | `src/protocol/pairing.rs` และ JSON round-trip/invalid-length tests [5] | Protocol maintainers | **Mitigated ใน branch นี้**; exact upstream SAS encoding ยังเป็น provisional และต้องมี cross-implementation fixture |
| TM-004 | Pairing code มี 6 หลักและ access credential แยกชื่อ/format; online PIN retry ถูกจำกัดและ session ปิดเมื่อ exhausted | High | `Identity::generate`, `Session::authenticate`, `constant_time_pin_eq` และ retry tests [3] [6] | Session maintainers | **Mitigated สำหรับ local runtime**; ต้องใช้ transport/session rate limit เพิ่มเมื่อมี remote signaling จริง |
| TM-005 | Signaling default เป็น `EndpointPolicy::PublicOnly`, ตรวจ DNS answers ทุกค่า, ปฏิเสธ loopback/private/link-local/reserved/documentation/IPv4-mapped special-use และรับเฉพาะ `ws`/`wss` | High | `EndpointPolicy::validate_endpoint`, `is_non_public_ip`, `SignalingClient::connect` และ endpoint negative tests [7] | Signaling maintainers | **Mitigated เป็น preflight boundary**; `connect_async` อาจ resolve hostname ซ้ำหลัง preflight จึงยังมี DNS validation/connect TOCTOU risk และไม่ใช่ OS firewall |
| TM-006 | Signaling ใช้ JSON text frames เท่านั้น, จำกัดขนาด send/receive, ตอบ ping/pong, มี reconnect policy แบบจำกัดและ re-applies endpoint policy ทุก attempt | Medium | `WebSocketConnection::send/recv`, `connect_with_retry`, `transact_with_retry` และ transport tests [7] | Signaling maintainers | **Mitigated สำหรับ bounded local transport**; retry count/delay ต้องถูกตั้งจาก deployment policy และไม่มี claim ว่าทนต่อ provider abuse |
| TM-007 | WebRTC peer lifecycle จำกัดไว้ที่ local two-peer harness; no hardcoded STUN/TURN และ cleanup เป็น idempotent | Medium | `PeerHandle`, `TwoPeerHarness`, lifecycle tests และ implementation status [8] | WebRTC maintainers | **Accepted for local MVP; open before release** สำหรับ external NAT traversal, browser interop, TURN credentials และ production ICE policy |
| TM-008 | Control/session ปฏิเสธ duplicate หรือ premature `connect`, `auth_required`, `auth_result`, `ready`; timeout และ terminal failure ปิด runtime/peer พร้อม cleanup stream | High | `SessionRuntime::handle_*`, `fail_session`, `connect_seen`, `last_auth_pin` และ tests สำหรับ duplicate connect, wrong-PIN exhaustion และ control timeout [9] | Session maintainers | **Mitigated ใน local authenticated runtime**; original-client ordering/replay behavior ยังต้องมี interoperability capture |
| TM-009 | File transfer บังคับ relative path, ปฏิเสธ absolute/`..`, canonical root escape และ symlink; มี per-operation timeout, size limit, overwrite policy และ temp upload cleanup | High | `safe_relative_path`, `resolve_existing`, `resolve_for_write`, `upload`, cancellation/path traversal/symlink/size tests [10] | File-stream maintainers | **Mitigated สำหรับ application boundary**; ไม่แทน OS ACL และยังต้องพิจารณา filesystem race ระหว่าง check กับ use บน deployment ที่ hostile |
| TM-010 | Shell stream ไม่ใช้ shell interpolation; สร้าง process ด้วย direct argv, จำกัด program/args/cwd, เรียก `env_clear`, จำกัด output และ kill เมื่อ timeout/cancel | High | `ShellPolicy`, `ShellStreamHandler::run`, `read_limited`/output accounting และ rejection/timeout/cancellation tests [11] | Shell-stream maintainers | **Mitigated สำหรับ allowlisted local MVP**; OS sandbox, seccomp/AppContainer/SELinux และ user account isolation ยังจำเป็นก่อนเปิดใช้กับ untrusted peer |
| TM-011 | Proxy default deny-by-default; target ต้องผ่าน allowlist หรือ explicit user confirmation; credentials/fragment ถูกปฏิเสธ; DNS answers ถูก pin; retry/redirect ตรวจ address, origin, downgrade และ count | High | `ProxyPolicy::validate_target`, `resolve_and_validate`, `validate_retry`, `validate_redirect` และ adversarial tests [12] | Proxy maintainers | **Mitigated สำหรับ `tcp`, `http`, `https`, `ws` ที่ผ่าน policy**; `wss` ถูกปฏิเสธ explicit จนกว่าจะมี TLS connector ที่ทดสอบ DNS pinning/certificate behavior ทุก platform |
| TM-012 | Proxy resource limits ครอบคลุม request/response bytes, buffered bytes, active concurrency, connect/idle timeout; handler ใช้ `resolve_to_addrs`, ปิด auto-redirect และอ่าน HTTP response แบบ incremental ด้วย `Response::chunk` ก่อน append เข้า body buffer | Medium | `ProxyResourceLimits`, `ProxyStreamService`, TCP/WebSocket/HTTP handlers, chunked-response regression test และ local handler tests [12] [13] | Proxy maintainers | **Mitigated สำหรับ bounded handler path**; body buffer ไม่เกิน `max_response_bytes` แล้ว แต่ network peer ยังอาจทำให้เกิด CPU/socket pressure ที่ OS/runtime ต้องคุมเพิ่ม |
| TM-013 | Log redaction ซ่อน authorization-bearing headers และลด target เหลือ scheme/host/port โดยไม่ใส่ path/query/fragment/userinfo; private key ไม่ถูกส่งเข้า logging API | Medium | `redact_target_for_log`, `redact_header_value` และ error mapping ใน proxy policy [12] | All maintainers / Release owner | **Partially mitigated**; ต้องมี repository-wide logging review และ structured-log test ก่อน release เพราะ helper ไม่บังคับทุก call site |
| TM-014 | Dependency surface มี `ring 0.17.14`, `rsa 0.9.10`, `webrtc 0.20.0`, `reqwest 0.13.4`, `tokio-tungstenite 0.30.0`; sandbox นี้ไม่มี `cargo-audit` และไม่มี advisory database evidence | Medium | `Cargo.lock` และ dependency tree ที่บันทึกใน audit run [14] | Release owner | **Open before release**; ต้องรัน advisory scanner ใน CI/release environment และ pin/update ตาม advisory ที่ตรวจพบ ห้ามตีความ compile/test ผ่านว่าไม่มี CVE |
| TM-015 | Platform boundaries ไม่เท่ากัน: Unix มี mode test, Windows มี atomic replacement code, Android มี compile-only target และไม่มี device/runtime evidence | Medium | `cfg(unix)`, `cfg(windows)`, `rust-toolchain.toml`, `.github/workflows/ci.yml` และ ADR #45 [15] | Release owner | **Documented; open before release** สำหรับ Windows runner proof, Android compile job proof, packaging, emulator/device และ ACL behavior |
| TM-016 | Interoperability กับ original-Go, signaling provider, browser, external STUN/TURN และ production NAT traversal ยังไม่มีหลักฐาน | High | Compatibility fixtures เป็น repository/local fixture; `tests/compatibility_baseline.rs` ระบุ boundary และ not-tested status [2] [8] | Protocol/WebRTC maintainers | **Open before release**; ห้ามประกาศ remote production compatibility จาก local tests เพียงอย่างเดียว |

## 5. Audit checklist ก่อน release

| Control area | Checklist ที่ต้องผ่าน | หลักฐานปัจจุบัน | ผล |
|---|---|---|---|
| Identity/pairing | RSA 2048, private persistence permissions, atomic write, PEM/JSON explicitness, nonce length/base64, constant-time commitment | Unit tests, PEM fixture และ ADR #23/#46 [3] [4] [5] | ผ่าน local code/test; interoperability pending |
| Signaling egress | Public-only default, all DNS answers checked, JSON-only, frame size, bounded retry, no local opt-in by default | Transport tests และ `EndpointPolicy` [7] | ผ่าน local code/test; TOCTOU/provider evidence pending |
| WebRTC/session | No unbounded ICE claim, PIN retry exhaustion, fixed-size comparison, duplicate rejection, timeout, cleanup | Peer/session runtime tests [8] [9] | ผ่าน local harness; external evidence pending |
| File capability | Root containment, traversal/symlink rejection, size/overwrite policy, temp cleanup, cancellation | File-stream negative tests [10] | ผ่าน local code/test; OS ACL/race residual |
| Process capability | Direct argv, allowlist, clean env, bounded args/output, timeout/cancel kill | Shell tests [11] | ผ่าน local code/test; OS sandbox residual |
| Proxy capability | Deny-by-default, authorization, DNS pinning, mapped-IP handling, redirect/rebinding guards, resource limits, explicit wss rejection | Proxy policy/handler tests and ADRs [12] [13] | ผ่าน local code/test; TLS/provider evidence pending |
| Dependencies | Advisory database scan and review of transitive crypto/WebRTC crates | `Cargo.lock` only; scanner unavailable in audit sandbox [14] | ยังไม่ผ่าน release gate |
| Logging/errors | Redact credential headers/query tokens, avoid private key/body logging, stable remote-safe errors | Proxy helpers and typed policy errors [12] | บางส่วน; repository-wide call-site review pending |
| Platforms | Linux full gate, Windows build/test runner, Android compile-only runner, no macOS claim | Workflow/toolchain and ADR #45 [15] | Linux local; runner evidence pending |

## 6. Release boundary และ residual risks

ก่อน release ต้องปิดอย่างน้อย TM-005 TOCTOU ของ signaling endpoint, TM-014 dependency advisory scan, TM-015 runner/platform evidence และ TM-016 original-Go/browser/provider interoperability หรือประกาศ release เป็น local MVP ที่ไม่รองรับ production remote access การประกาศว่า `wss` รองรับต้องไม่ทำก่อนมี explicit TLS connector ที่ pin DNS, ตรวจ certificate hostname และมี evidence บน Linux/Windows/Android ตาม support matrix

ความเสี่ยงที่ยอมรับได้เฉพาะ local MVP คือการใช้ local fixture, provisional SAS, JSON persistence ที่เลือก explicit ได้, และการไม่มี external NAT traversal ทั้งหมดต้องไม่ถูกซ่อนด้วยคำว่า secure หรือ production-ready การป้องกันของ application layer ไม่สามารถชดเชย process compromise, malicious local user, permissive parent ACL, kernel compromise, firewall misconfiguration หรือ resource exhaustion ที่อยู่นอก crate ได้

## 7. Validation record

คำสั่งที่ต้องบันทึกผลทุกครั้งก่อนเปิด PR คือ:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all
cargo clippy --all --all-targets -- -D warnings
git diff --check
```

Linux local pass เป็นหลักฐานของ Linux เท่านั้น ส่วน Windows และ Android ต้องอ้างสถานะจาก job ที่รันจริงของ workflow ไม่ใช้การ cross-compile หรือ local absence ของ NDK แทน runner evidence [15]

## References

[1]: ../../README.md "Project scope และ local MVP boundary"
[2]: ../../tests/compatibility_baseline.rs "Versioned compatibility baseline tests"
[3]: ../../src/identity/key.rs "Identity persistence, RSA validation และ platform replacement"
[4]: ./identity-persistence.md "Identity PEM/JSON compatibility decision"
[5]: ../../src/protocol/pairing.rs "Pairing wire encoding, nonce และ commitment"
[6]: ../../src/session/mod.rs "Session PIN authentication และ retry policy"
[7]: ../../src/signaling/transport.rs "Endpoint policy และ JSON WebSocket transport"
[8]: ../../src/peer/mod.rs "Peer lifecycle และ local two-peer harness"
[9]: ../../src/session/runtime.rs "Authenticated runtime sequencing, replay และ timeout"
[10]: ../../src/stream/file.rs "Sandboxed file-transfer policy"
[11]: ../../src/stream/shell.rs "Allowlisted shell process policy"
[12]: ../../src/stream/proxy.rs "Deny-by-default proxy policy และ redaction"
[13]: ../../src/stream/proxy_handler.rs "Concrete TCP/WebSocket/HTTP handlers"
[14]: ../../Cargo.lock "Pinned dependency versions"
[15]: ../../docs/decisions/issue-45-support-matrix.md "Linux/Windows/Android support matrix"
