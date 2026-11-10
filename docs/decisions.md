# Architectural Decision Records (ADRs)

เอกสารสรุปรวมการตัดสินใจทางสถาปัตยกรรม (Condensed Decision Ledger) เพื่อรักษาประวัติศาสตร์และเหตุผลเบื้องหลัง โดยไม่ต้องคงไฟล์ draft แยกย่อย

---

## ADR-0001: Non-Trickle ICE (Vanilla ICE)

- **สถานะ**: Accepted (Implement แล้วใน `src/peer/`)
- **บริบท**: WebRTC รองรับ Trickle ICE (ส่ง candidate ทันทีที่เจอ) และ Non-Trickle ICE (รอ gather candidate ครบทั้งหมดก่อนส่ง SDP Offer/Answer)
- **การตัดสินใจ**: ใช้ **Non-Trickle ICE** สำหรับ peer connection ทั้งหมด
- **เหตุผล & ข้อดี**:
  - State machine เรียบง่าย: การแลกเปลี่ยน Offer → Answer → Connect เป็นลำดับขั้นตอนแน่นอน ไม่มี race condition จาก candidate ที่มาถึงระหว่างทาง
  - ง่ายต่อการทดสอบ: `TwoPeerHarness` สร้าง fully-formed offer ได้แบบ deterministic โดยไม่ต้องรอ asynchronous candidate gathering
  - เข้ากันได้กับ Wire Protocol เดิม
- **Trade-off / ข้อจำกัด**:
  - Connection setup ช้ากว่าเล็กน้อย (เพิ่ม 500ms-2s ในกรณี WAN/TURN) เพราะต้องรอ STUN/TURN probe เสร็จสิ้นก่อนเริ่ม signaling (บน LAN ไม่ได้รับผลกระทบ)

---

## ADR-0047: Loopback Browser Control Surface

- **สถานะ**: Accepted (Implement แล้วใน `src/web/`)
- **บริบท**: คำสั่ง `blnk web` เพื่อเปิด control surface ผ่าน browser แต่ต้องป้องกันความเสี่ยงด้าน credential leak, CSRF, และ cross-site request
- **การตัดสินใจ**:
  1. **Loopback Only**: Bind เฉพาะ `127.0.0.1` (ห้าม bind external interface)
  2. **Auth & Tokens**: สร้าง session ผ่าน `POST /api/session` ด้วย Bearer bootstrap token และ exact origin
  3. **Strict CSRF & Cookies**: ออก `HttpOnly; SameSite=Strict` cookie พร้อม CSRF token และต้องส่ง `X-CSRF-Token` ตรวจสอบแบบ constant-time ในทุก state-changing request
  4. **Strict WebSocket**: `GET /api/session/{id}/ws` ต้องผ่าน exact origin และ session cookie โดยข้อความแรกต้องเป็น JSON hello พร้อม CSRF token
  5. **Bounded Limits**: กำหนด body limit, WebSocket frame limit, rate limit และ session TTL ชัดเจน
  6. **No Remote Execution Shortcut**: ไม่มี endpoint สำหรับ remote shell, file copy หรือ WebRTC direct offer/answer ผ่าน browser ผิวนี้มีหน้าที่ดูสถานะและควบคุม local session lifecycle เท่านั้น

---

## Core Invariants: Control vs Data Separation

- **Stream 0 = Control (Protobuf)**: จัดการ session lifecycle (connect → auth → ready → error)
- **Stream N > 0 = Data (SWSP Frames)**: shell, file transfer, TCP/WS/HTTP proxy
- **กฎเหล็ก**: Control ห้ามแบกข้อมูล payload ปกติ และ Data ห้ามแบก control messages
