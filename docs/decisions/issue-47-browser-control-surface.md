# ADR-047: Browser control surface แบบ loopback และ session-only

## สถานะ

**Accepted — implement แบบจำกัดขอบเขต**

## บริบท

Issue #47 ต้องตัดสินใจก่อนว่าจะเพิ่ม browser/web control surface หรือ defer งานนี้ โดย dependency ที่เกี่ยวข้องกับ CLI E2E และ compatibility baseline (#43 และ #44) ปิดแล้ว แต่ repository ยังไม่มีหลักฐาน browser interoperability, external signaling/provider, production TLS, STUN/TURN, NAT traversal หรือ deployment จริง การ implement จึงต้องไม่สร้าง API ที่ดูเหมือนพิสูจน์ความสามารถเหล่านั้นแล้ว

ความเสี่ยงหลักของ HTTP ที่รับจาก browser คือ origin ที่ไม่ถูกต้อง, cross-site request, credential/session leakage, oversized body/frame, resource exhaustion และการเผลอเปิด capability ที่เดิมต้องอยู่หลัง authenticated `SessionRuntime`/capability policy การใช้ HTTP endpoint เป็นทางลัดไปยัง shell, file, proxy หรือ signaling จะข้าม security boundary และยังไม่มี browser-compatible E2E evidence รองรับ

## การตัดสินใจ

เลือก **implement minimal browser control flow** แทนการ defer โดยให้เป็น control surface สำหรับ lifecycle/status ของ local browser session เท่านั้น ไม่ใช่ browser WebRTC client และไม่ใช่ remote provider adapter

1. เพิ่มคำสั่ง `blnk web` ซึ่ง bind ได้เฉพาะ loopback address และใช้ TCP port ที่ caller กำหนดหรือ port `0` แบบ ephemeral สำหรับ fixture/การรันเฉพาะเครื่อง
2. การสร้าง session ใช้ `POST /api/session` พร้อม `Origin` ที่ตรงกับ exact configured origin และ `Authorization: Bearer <bootstrap token>` โดย token รับจาก `--bootstrap-token` หรือ `BLNK_WEB_TOKEN`; server ไม่พิมพ์ token หรือ credential ใดออกทาง log
3. เมื่อสร้าง session สำเร็จ server จะออก `HttpOnly; SameSite=Strict` cookie ที่ผูกกับ session และคืน CSRF token เฉพาะครั้งนั้นใน response body การอ่าน status ใช้ cookie และ exact origin ส่วน state-changing `POST /api/session/{id}` ต้องมี `X-CSRF-Token` ที่ตรงกันแบบ constant-time
4. WebSocket `GET /api/session/{id}/ws` ต้องผ่าน exact origin และ session cookie ก่อน upgrade และข้อความแรกต้องเป็น JSON `{"type":"hello","csrf_token":"..."}` การสื่อสารหลัง handshake รองรับเฉพาะ `status` และ `close`; binary frame, JSON field ที่ไม่รู้จัก, hello ซ้ำ และ malformed message ปิด connection
5. ตั้ง body limit, WebSocket frame/message limit, จำนวน active session, session creation rate และ session TTL เป็นค่า bounded ที่กำหนดใน config และตรวจ `Content-Length` ก่อน parse body เพื่อให้ oversized request ได้ผลลัพธ์ที่ deterministic
6. CORS อนุญาตเฉพาะ exact configured origin ไม่ใช้ wildcard และไม่เปิด credential ให้ origin อื่น response ใส่ `Cache-Control: no-store` และ `X-Content-Type-Options: nosniff`
7. error response ใช้รหัสทั่วไป (`invalid_request`, `unauthorized`, `forbidden`, `not_found`, `session_unavailable`, `rate_limited`, `payload_too_large`, `internal_error`) โดยไม่สะท้อน bearer token, CSRF token, cookie, path ลับ หรือรายละเอียดภายใน
8. ไม่มี endpoint สำหรับ remote shell, file copy, TCP/WebSocket/HTTP proxy, signaling หรือการสร้าง WebRTC offer/answer ในงานนี้ ความสามารถเหล่านั้นยังต้องถูกเรียกผ่าน business/session capability boundary เดิม และต้องมี evidence เฉพาะก่อนเพิ่มเข้าผิว browser

## ทางเลือกที่ไม่เลือก

### Defer ทั้งหมด
ปลอดภัยด้าน scope แต่ไม่ตอบ acceptance ที่ต้องการ minimal browser control flow หลัง dependency เสถียรแล้ว จึงไม่เลือกเมื่อสามารถทำ local fixture ที่ไม่พึ่ง external service ได้

### เปิด remote/browser WebRTC หรือ proxy ผ่าน HTTP ทันที
ไม่เลือก เพราะจะปะปน transport, authentication และ capability policy หลายชั้น รวมถึงทำให้เกิด claim เรื่อง browser/provider interoperability ที่ยังไม่มีหลักฐาน

### ใช้ wildcard CORS หรือ token ใน query string
ไม่เลือก เพราะ wildcard + credentials ทำให้ cross-origin boundary กว้างเกินจำเป็น และ query string มีโอกาสรั่วผ่าน history, proxy หรือ log

## Consequences

ผลดีคือมี flow ที่ browser สามารถสร้าง session, ตรวจ status, ยืนยัน CSRF ผ่าน WebSocket และปิด session ได้จริงใน local fixture โดยมี boundary ทดสอบได้ deterministic และไม่ต้องมี external secret/provider การ bind loopback จำกัด exposure ของ MVP และการใช้ session cookie/CSRF แยกจาก bootstrap token ลดการส่ง credential เดิมซ้ำในทุก request

ผลจำกัดคือ surface นี้ยังไม่ใช่ browser-compatible remote client ไม่ได้พิสูจน์ WebRTC data channel จาก browser ไม่รองรับ TLS termination, production reverse proxy, STUN/TURN, NAT traversal, external signaling หรือ original-Go interoperability และ session status เป็น local control state ไม่ใช่หลักฐานว่า remote peer เชื่อมต่อสำเร็จ

## หลักฐานและการทดสอบ

`src/web/mod.rs` มี fixture-backed tests สำหรับ exact origin/CORS, bearer authentication, session-cookie boundary, CSRF failure/success, body limit, creation rate limit, WebSocket CSRF hello, status flow, binary/oversized frame handling และ graceful shutdown การทดสอบใช้ loopback listener, local HTTP client และ local WebSocket client เท่านั้น ไม่มี external service หรือ production credential

การผ่าน `cargo fmt`, `cargo check`, `cargo test` หรือ CI บน platform ใด ๆ ยืนยันเฉพาะ code/test boundary ของ repository เท่านั้น ไม่เปลี่ยนข้อจำกัดด้าน browser interoperability, production deployment หรือ TLS ที่ยังไม่มี evidence
