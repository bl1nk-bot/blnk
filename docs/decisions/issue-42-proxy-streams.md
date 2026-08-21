# ADR-042: Concrete TCP, WebSocket และ HTTP proxy streams

- **สถานะ:** Implemented locally; PR #56 เปิดอยู่ รอ review/merge
- **Issue:** #42
- **วันที่:** 2026-08-21
- **ขึ้นต่อ:** Issue #38 (SessionRuntime) และ Issue #41 (Proxy Security Policy, PR #55)

## Context

Issue #41 กำหนด security policy กลาง แต่ยังไม่เชื่อมกับ transport จริง งานนี้จึงเพิ่ม service สำหรับเปิด raw TCP, WebSocket และ HTTP proxy streams โดยบังคับให้ทุก outbound target ผ่าน `ProxyPolicy` ก่อน resolve, connect, retry หรือ redirect

การ implement ใน issue นี้เป็น **concrete handler/service boundary** ที่ทดสอบได้ด้วย local fixtures ยังไม่ใช่การผูกเข้ากับ `SessionRuntime` หรือการยืนยัน interoperability กับ client/browser ภายนอก การเปิดใช้งานผ่าน session จึงต้องทำในงานถัดไปโดยใช้ stream allocation และ authenticated control path ของ Issue #38

## Decisions

### 1. Service boundary และ concurrency

`ProxyStreamService::new(policy)` ตรวจ `ProxyResourceLimits` และสร้าง bounded semaphore ตาม `max_concurrent_streams` การเรียก `open_tcp`, `open_websocket` และ `request_http` ต้องถือ permit ก่อนเริ่ม outbound work หากเต็มจะคืน `concurrency_limit_exceeded` ก่อน dial

TCP และ WebSocket คืน per-stream adapter ที่ถือ permit ตลอดอายุ stream และปล่อย permit เมื่อ adapter ถูก drop ส่วน HTTP เป็น one-request transcript และปล่อย permit เมื่อ request จบ

### 2. TCP

`open_tcp(stream_id, TcpOpen, authorization)` ตรวจ wire discriminator, สร้าง `tcp://host:port`, เรียก `ProxyPolicy::resolve_and_validate`, ตรวจทุก DNS answer และเก็บ address set ที่ผ่านไว้สำหรับ retry จากนั้น connect ตาม address ที่ pin ไว้ภายใต้ `connect_timeout`

`TcpProxyStream::send` และ `recv` ใช้ raw bytes โดยตรวจ request/response ceilings และ idle timeout ตาม `ProxyResourceLimits` เมื่อเกินขนาดให้คืน stable policy error และไม่ส่งข้อมูลต่อ

### 3. WebSocket

`open_websocket(stream_id, WebSocketOpen, authorization)` ใช้ socket ที่ dial จาก address ที่ policy อนุมัติแล้ว และเรียก `client_async` บน socket นั้นโดยตรง เพื่อไม่ให้ WebSocket client ทำ DNS ซ้ำเอง `ws` มี local echo-fixture evidence รองรับ ส่วน `wss` ถูกปฏิเสธอย่างชัดเจนจนกว่าจะมี TLS connector ที่กำหนด DNS pinning, certificate verification และ cross-platform tests ครบ การไม่ fallback จาก `wss` เป็น plain TCP เป็นข้อกำหนด security

`WebSocketProxyStream::send` map payload เป็น binary message และ `recv` คืน binary/text payload เป็น bytes โดย ping/pong ถูกจัดการตาม tungstenite ส่วน close ส่ง close frame และปิด socketอย่างสุภาพ

### 4. HTTP

`request_http(base_target, HttpRequest, body, authorization)` ปิด automatic redirects ด้วย `reqwest::redirect::Policy::none()` และสร้าง client ใหม่ต่อ redirect hop โดยใช้ `resolve_to_addrs` กับ address set ที่ policy ตรวจแล้ว ทุก hop ต้องผ่าน `validate_redirect` ของ policy ซึ่งจำกัด same-origin, scheme downgrade และจำนวน redirect

การส่ง request จะตัด hop-by-hop headers และ `content-length` ที่ caller ส่งมา แล้วตรวจ content length จาก body จริง response headers ถูกแปลงเป็น transcript ที่ไม่รวม body โดย body ถูกอ่านภายใต้ `max_response_bytes` การตอบกลับจึงไม่อ้างว่าเป็น streaming HTTP body เต็มรูปแบบใน issue นี้

### 5. Scheme, DNS และ authorization

ทุก handler ใช้ `ProxyPolicy::deny_by_default()` หรือ policy ที่ caller สร้างขึ้นอย่าง explicit การ authorize เป็น `Allowlisted` หรือ `UserConfirmed` ตาม policy configuration; `None` ถูกปฏิเสธเสมอ private/loopback/link-local/multicast/reserved/documentation และ IPv4-mapped special-use addresses ถูกปฏิเสธตาม ADR-041

การ retry ใช้เฉพาะ address set ที่ resolve และ validate แล้ว ห้าม resolve ใหม่ระหว่าง retry แบบเงียบ ๆ การ redirect ทำให้เกิดการ validate target ใหม่และห้าม cross-origin หรือ downgrade ตาม policy

### 6. SWSP/protobuf boundary

โมดูลนี้มี codec helpers สำหรับ `TcpOpen`, `WebSocketOpen`, `HttpRequest` และ `HttpData` โดยใช้ `SYN|DAT` สำหรับ open/request และ `DAT|MORE` สำหรับ data ตาม SWSP raw frame contract การ decode ตรวจ stream id, flags และ protobuf payload ก่อนคืน typed message

โมดูลนี้ยังไม่ได้เพิ่ม message dispatch เข้า `SessionRuntime`; Issue ถัดไปต้อง map `StreamKind`/stream id ที่ authenticated runtime จัดสรรให้เข้ากับ service boundary นี้ และต้องส่ง `DAT|FIN` เมื่อ close/error/cancel ตาม lifecycle ของ runtime

## Error mapping

Policy failures ถูก map เป็น `BlnkError::Stream("proxy policy rejected: <stable_code>")` ส่วน malformed protobuf/header, invalid method/header และ transport/connect failures ใช้ `BlnkError::Protocol` หรือ `BlnkError::Stream` ตามชั้นที่เกิด error โดยไม่ใส่ credential, authorization header หรือ query token ลงข้อความ error/log

## Evidence

| พฤติกรรม | หลักฐาน local test |
|---|---|
| TCP echo และ request/response limits | `tcp_fixture_echoes_and_applies_limits` |
| WebSocket text/binary bridge และ close path | `websocket_fixture_bridges_text_and_binary_messages` |
| HTTP response transcript และ manual same-origin redirect | `http_fixture_returns_response_transcript_and_manual_redirects` |
| Deny ก่อน connect | `denied_target_is_rejected_before_local_connect` |
| Concurrency ceiling | `concurrency_limit_is_enforced_before_connect` |
| Canonical SWSP flags/protobuf | `raw_transcript_codecs_use_canonical_flags_and_protobuf` |

Validation บน Linux ผ่าน `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo test --all` (77 tests), `cargo clippy --all --all-targets -- -D warnings` และ `git diff --check`

## Non-goals และ known gaps

งานนี้ยังไม่อ้าง external interoperability, browser compatibility, TLS `wss`, authenticated SessionRuntime dispatch, production NAT traversal, Windows/Android build evidence หรือ OS-level egress firewall การใช้งานจริงต้องเพิ่ม integration path, cross-platform CI และ external fixtures ก่อน release

## References

[1]: ./issue-41-proxy-policy.md "ADR-041: Proxy Security Policy"
[2]: ../api.md "API contract"
[3]: ../../proto/stream.proto "Stream protobuf schema"
[4]: ../implementation-status.md "Canonical implementation status"

- [ADR-041: Proxy Security Policy][1]
- [API contract][2]
- [Stream protobuf schema][3]
- [Canonical implementation status][4]
