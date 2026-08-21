# ADR-041: Proxy Security Policy สำหรับ TCP, WebSocket และ HTTP

- **สถานะ:** Accepted สำหรับ implementation boundary ของ Issue #42
- **วันที่:** 2026-08-21
- **ผู้ตัดสินใจ:** โครงการ blnk Rust
- **ขอบเขต platform:** Linux, Windows และ Android; macOS อยู่นอก scope
- **Dependency:** Issue #38 — authenticated session runtime
- **Blocks:** Issue #42 — TCP/WebSocket/HTTP proxy streams

## บริบทและปัญหา

Proxy stream รับ target จากฝั่ง remote session แล้วทำ network egress จากเครื่องที่ให้บริการ หากปล่อยให้ handler เชื่อมต่อ target โดยตรง ผู้โจมตีอาจใช้ session ที่ authenticate แล้วเป็นทางผ่านไปยัง loopback, private network, metadata service, multicast หรือปลายทางที่เปลี่ยนระหว่าง DNS retry ได้ ปัญหานี้เป็น **SSRF และ target-escalation boundary** ไม่ใช่เพียงการตรวจรูปแบบ URL

Issue นี้จึงกำหนด policy กลางก่อน implement concrete handlers โดยไม่เพิ่ม wire protocol และไม่อ้างว่า application policy แทน OS-level firewall ได้ การบังคับใช้จริงต้องเกิดก่อน connect และเกิดซ้ำก่อน retry หรือ redirect ทุกครั้ง

> **หลักการตัดสินใจ:** ไม่มี authorization ที่ตรวจสอบได้ ให้ปฏิเสธเสมอ; การเป็น public address เพียงอย่างเดียวไม่ใช่สิทธิ์ในการเชื่อมต่อ

## การตัดสินใจหลัก

| ประเด็น | การตัดสินใจ | ค่าเริ่มต้น/หลักฐานในโค้ด |
|---|---|---|
| Authorization | ใช้ exact allowlist แบบ scheme + canonical host + port หรือ user confirmation ที่เปิดใช้โดยผู้ดูแลอย่าง explicit | `ProxyAuthorization::None` ถูกปฏิเสธ; `ProxyPolicy::deny_by_default()` ปิด user confirmation |
| Scheme | รองรับเฉพาะ `http`, `https`, `tcp`, `ws`, `wss`; scheme อื่นถูกปฏิเสธ | `ProxyScheme` และ `UnsupportedScheme` |
| Target syntax | ต้องมี host; ห้าม userinfo/credentials และ fragment; TCP ต้องมี explicit port และไม่มี path/query ที่มีความหมาย | `ProxyPolicy::validate_target` |
| Port | ใช้ explicit port หรือ default ของ HTTP(S)/WS(S); port 0 และ port ที่อยู่นอก configured allowlist ถูกปฏิเสธ | `InvalidPort`, `PortNotAllowed` |
| DNS | resolve หนึ่งครั้งก่อน connect, ตรวจทุก answer, reject mixed public/private answers และ pin `SocketAddr` ที่ผ่านการตรวจ | `resolve_and_validate` คืน `ResolvedProxyTarget` |
| Local/special-use IP | deny-by-default สำหรับ private, loopback, link-local, IPv6 ULA, unspecified, multicast, broadcast, shared/reserved และ documentation ranges; local opt-in อนุญาตเฉพาะกลุ่ม local ที่ระบุ แต่ยังต้องมี authorization | `with_local_targets(true)` ไม่เปิด reserved/unspecified/multicast |
| IPv4-mapped IPv6 | แปลงความหมายกลับเป็น IPv4 แล้วตรวจซ้ำ จึงไม่สามารถหลบ policy ด้วย `::ffff:a.b.c.d` ได้ | ตรวจผ่าน `to_ipv4_mapped` แบบ recursive |
| Redirect | ทุก redirect ต้องถูก validate ใหม่; จำกัดจำนวน; same-origin exact key เป็นค่าเริ่มต้น; HTTPS/WSS downgrade และ cross-origin redirect ถูกปฏิเสธ | `max_redirects = 5`, `allow_cross_origin_redirects = false`, `allow_insecure_redirects = false` |
| DNS rebinding | ห้าม retry ไปยัง address ที่ไม่ได้อยู่ในชุด answer ที่ pin ไว้ และตรวจ IP ก่อน membership check ทุกครั้ง | `validate_retry` คืน `DnsRebindingDetected` หรือ `AddressNotAllowed` |
| Resource limits | จำกัด request/response bytes, active streams, buffered bytes, connect/idle timeout และ redirects; handler ต้องหยุดอ่าน/ยกเลิกเมื่อเกิน limit | ค่า default อยู่ใน `ProxyResourceLimits` |
| Backpressure/cancellation | ส่งต่อได้ไม่เกิน `max_buffered_bytes`; เมื่อ timeout, limit หรือ `CancellationToken` ถูกยกเลิก ให้หยุด I/O และปิด stream โดยไม่ retry ข้าม policy | เป็น contract สำหรับ Issue #42; handler ยังไม่อยู่ใน Issue #41 |
| Logging | log เฉพาะ scheme/host/port ที่ผ่านการ redact; ห้าม log path, query, fragment, userinfo, body และ credential headers | `redact_target_for_log`, `redact_header_value` |
| Error mapping | policy error ใช้ stable code และ map เป็น `BlnkError::Stream`; ห้ามส่งรายละเอียด DNS, URL credentials หรือ secret ใน error/log | `ProxyPolicyError::code` และ `From<ProxyPolicyError> for BlnkError` |

## Target validation contract

Future handlers ต้องเรียก policy ตามลำดับต่อไปนี้และต้องถือว่าการ validate ไม่ใช่การ connect:

1. แปลง incoming target เป็น absolute `Url` และเรียก `validate_target(target, authorization)` ก่อนสร้าง socket หรือ HTTP client
2. ถ้า target ใช้ hostname ให้เรียก `resolve_and_validate`; ถ้า handler มี resolver ของตัวเอง ต้องส่ง **ทุก** `SocketAddr` เข้า `validate_resolved_target`
3. connect ได้เฉพาะ address ใน `ResolvedProxyTarget::addresses()` และใช้ port ที่ policy คืนเท่านั้น
4. เมื่อจะ retry ให้เรียก `validate_retry` กับ address เดิม; ห้าม resolve ใหม่แล้วใช้ answer ใหม่โดยอัตโนมัติ
5. เมื่อจะ follow redirect ให้ join relative location ใน HTTP handler ก่อน แล้วเรียก `validate_redirect` กับ absolute URL ทุกครั้ง โดยเพิ่ม redirect counter ก่อนการติดตามลำดับถัดไป
6. ตรวจ request/response/buffer/concurrency limits ตลอดอายุ stream และผูก cancellation เข้ากับ `SessionRuntime` lifecycle

Allowlist เป็น **exact match** หลัง canonicalize hostname เป็น lowercase และตัด trailing dot; ไม่ใช่ suffix match ดังนั้น `example.com` ไม่อนุญาต `evil-example.com` หรือ `example.com.attacker.test` การตั้ง user confirmation ต้องเป็น decision ของ local operator ไม่ใช่ค่า implicit จากข้อความที่ remote peer ส่งมา

## DNS, IP และ rebinding policy

การตรวจ IP แบ่งเป็นสองกลุ่ม กลุ่มแรกคือ address ที่ห้ามเสมอ ได้แก่ unspecified, multicast, broadcast, IPv4-mapped ที่เมื่อแปลงแล้วอยู่ในกลุ่มห้าม, IPv4 zero/shared/reserved/documentation/benchmark ranges และ IPv6 documentation range `2001:db8::/32` กลุ่มที่สองคือ private, loopback, link-local และ IPv6 unique-local/link-local ซึ่ง deny โดย default และเปิดได้เฉพาะเมื่อ policy ตั้ง `allow_local_targets` พร้อม authorization ที่ถูกต้องแล้ว

หาก hostname คืนหลาย answer ต้องตรวจทุก answer และปฏิเสธทั้งชุดเมื่อมี answer ใดไม่ผ่าน วิธีนี้ป้องกันการเลือก answer private ใน retry ภายหลัง การ retry ใช้เฉพาะ `SocketAddr` ที่อยู่ใน answer set เดิม; address ใหม่ถือเป็น potential DNS rebinding และถูกปฏิเสธ แม้ hostname เดิมจะเหมือนเดิมก็ตาม

การตรวจนี้เป็น application-level egress preflight ไม่ใช่ guarantee ว่า route, proxy chain, kernel, NAT หรือ DNS infrastructure จะไม่เปลี่ยนหลังจากตรวจแล้ว ดังนั้น production deployment ควรใช้ OS/network egress controls เพิ่มเติม

## Redirect และ scheme policy

Redirect นับเฉพาะการ follow response ที่ handler ตัดสินใจทำ ไม่รวม request แรก ค่า default อนุญาตไม่เกิน 5 ครั้ง, ห้าม cross-origin และห้าม downgrade จาก HTTPS ไป HTTP หรือจาก WSS ไป WS การเปลี่ยน port ถือเป็นคนละ target key และถูกปฏิเสธด้วย same-origin rule เว้นแต่ explicit redirect policy และ authorization อนุญาต

Redirect ไม่ได้รับสิทธิ์สืบทอดโดยอัตโนมัติ การเรียก `validate_redirect` ต้องส่ง authorization ที่เหมาะสมสำหรับ URL ใหม่ และ URL ใหม่นั้นต้องผ่าน scheme, credentials, fragment, port และ IP/DNS validation ต่อไป การตัดสินใจว่าจะถามผู้ใช้ซ้ำหรือใช้ allowlist เป็นหน้าที่ของ handler/UI แต่ policy จะไม่ยอมรับ `UserConfirmed` หากไม่ได้เปิด capability ไว้

## Resource, backpressure และ cancellation contract

ค่าเริ่มต้นที่ใช้เป็น baseline สำหรับ handlers มีดังนี้ ค่าเหล่านี้เป็น safety ceiling ของ policy object ไม่ใช่ benchmark หรือหลักฐาน production capacity:

| Resource | Default | Absolute ceiling ที่ config ยอมรับ |
|---|---:|---:|
| Request body | 1 MiB | 64 MiB |
| Response body | 8 MiB | 64 MiB |
| Concurrent proxy streams | 16 | 1,024 |
| Buffered bytes ต่อ stream | 256 KiB | 16 MiB |
| Connect timeout | 10 วินาที | 300 วินาที |
| Idle timeout | 60 วินาที | 3,600 วินาที |
| Redirects | 5 | 20 |

`check_concurrent_streams` ต้องถูกเรียกก่อนเปิด stream ใหม่, `check_buffered_bytes` ต้องถูกเรียกก่อนเพิ่ม buffer, และ byte checks ต้องหยุดอ่านทันทีที่เกิน limit การหมด timeout หรือ cancellation ต้อง cancel pending I/O, close child/task/connection และปล่อย stream registry entry; ห้ามเปลี่ยนเป็น unbounded queue และห้าม retry ไป target ที่ policy ไม่ได้ pin ไว้

## Log และ credential redaction

`redact_target_for_log` คืนเพียง `scheme://host:port` และตัด path, query, fragment และ userinfo ออกทั้งหมด แม้ URL จะ malformed ก็คืน `<invalid-target>` แทน raw input Header ที่ถือเป็น secret ได้แก่ `Authorization`, `Proxy-Authorization`, `Cookie`, `Set-Cookie`, `WWW-Authenticate`, `X-Api-Key` และ `X-Auth-Token` ต้องแสดงเป็น `<redacted>` ส่วน header อื่นคืนค่าได้เมื่อจำเป็นต่อ debugging แต่ห้าม log request/response body โดย policy นี้

Stable error code มีไว้สำหรับ metrics และ remote-safe error mapping เท่านั้น ไม่ควรนำ `Display` ของ low-level resolver หรือ URL parser ที่อาจมี hostname/input กลับไปส่ง remote โดยตรง

## API boundary สำหรับ Issue #42

Issue #42 ต้องใช้ API ที่มีอยู่ใน `src/stream/proxy.rs` ดังนี้:

| API | หน้าที่ของ handler |
|---|---|
| `ProxyPolicy::deny_by_default()` | สร้าง baseline policy ที่ไม่มี implicit authorization |
| `validate_target` | ตรวจ URL และ authorization ก่อน connect |
| `resolve_and_validate` / `validate_resolved_target` | ตรวจ DNS/IP ทุก answer และสร้าง pinned address set |
| `validate_retry` | ป้องกัน DNS rebinding และ port drift ใน retry |
| `validate_redirect` | ตรวจ redirect count, origin, scheme downgrade และ authorization ใหม่ |
| `ProxyResourceLimits` และ `check_*` | บังคับ size, concurrency และ backpressure limits |
| `redact_target_for_log`, `redact_header_value` | ใช้ก่อนสร้าง structured logs |
| `ProxyPolicyError::code` | map metrics/remote-safe errors แบบ deterministic |

โมดูลนี้ไม่สร้าง `HTTPRequest`, `TCPOpen`, `WebSocketOpen` หรือ message ใหม่ และไม่เปลี่ยน semantics ของ protobuf/SWSP ใน Issue #41 Handler ใน Issue #42 ต้องทำ policy check บน fields จาก `proto/stream.proto` ก่อนแปลงไปเป็น transport-specific connect request

## Adversarial test matrix

| กรณีโจมตี/ผิดพลาด | Expected result | Test evidence |
|---|---|---|
| ไม่มี authorization | reject ก่อน DNS/connect | `deny_by_default_requires_explicit_authorization` |
| public target ใน allowlist | allow หลังทุก DNS answer ผ่าน | `allowlisted_public_target_and_dns_answers_are_accepted` |
| private, loopback, link-local, IPv6 ULA | reject by default; local opt-in เท่านั้น | `private_loopback_and_mapped_addresses_are_denied_by_default`, `explicit_local_opt_in_still_requires_authorization_and_keeps_reserved_denied` |
| IPv4-mapped IPv6 ของ special-use IPv4 | reject หลังแปลงความหมาย | `private_loopback_and_mapped_addresses_are_denied_by_default` |
| mixed public/private DNS answers | reject ทั้งชุด | `dns_answer_sets_are_all_checked_and_retries_are_pinned` |
| retry ไป address ใหม่ | reject as rebinding | `dns_answer_sets_are_all_checked_and_retries_are_pinned` |
| cross-origin redirect | reject by default | `redirect_policy_reports_specific_cross_origin_and_downgrade_errors` |
| HTTPS/WSS downgrade | reject by default | `redirect_policy_reports_specific_cross_origin_and_downgrade_errors` |
| redirect count เกิน limit | deterministic reject | `redirect_policy_reports_specific_cross_origin_and_downgrade_errors` |
| credentials, fragment, missing TCP port | reject before connect | `malformed_targets_and_ports_are_rejected` |
| request/response/concurrency/buffer overflow | deterministic typed error | `resource_limits_and_error_mapping_are_deterministic` |
| URL query token หรือ credential header ใน log | redacted | `log_and_header_redaction_never_exposes_credentials_or_query_tokens` |

## Non-goals และหลักฐานที่ยังไม่มี

ADR นี้ไม่ implement TCP, WebSocket หรือ HTTP stream handler, ไม่ทำ DNS resolver แบบ custom, ไม่รับประกัน browser/original-client interoperability, ไม่พิสูจน์ NAT/TURN behavior และไม่แทน OS-level firewall, egress ACL, sandbox หรือ container isolation การทดสอบใน Issue #41 เป็น deterministic unit tests ด้วย synthetic `SocketAddr`; ยังต้องมี integration tests กับ real handlers และ platform-specific validation ในงานถัดไป

## References

[1]: ../../docs/architecture.md "Architecture and module layout"
[2]: ../../specs/spec.md "Functional and security requirements"
[3]: ../../docs/api.md "Public API contract"
[4]: ../../src/stream/proxy.rs "Proxy policy implementation and tests"
[5]: ../../proto/stream.proto "Existing stream protobuf boundary"
[6]: ../../src/signaling/transport.rs "Existing signaling endpoint preflight precedent"
