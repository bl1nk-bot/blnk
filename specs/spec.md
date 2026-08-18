# blnk Rust Specification

## 1. ภาพรวม

blnk Rust เป็น CLI tool สำหรับ remote access แบบ peer-to-peer ผ่าน WebRTC โดยออกแบบให้ทำงานได้โดยไม่ต้องมี account และไม่ต้องทำ port forwarding บนเครื่องปลายทาง
ระบบจะพึ่งพา signaling server เพื่อช่วยเชื่อม browser/client เข้ากับ device แล้วใช้ WebRTC data channel เป็นแกนหลักในการสื่อสาร

---

## 2. เป้าหมายของสเปค

- รักษาความสามารถเทียบเท่าต้นแบบ blnk-cli
- รักษา protocol compatibility กับระบบเดิม
- ออกแบบให้ implement ได้จริงใน Rust
- รองรับ Linux, Windows, Android
- แยกส่วน core logic ออกจาก CLI และ web integration

---

## 3. Scope

### 3.1 อยู่ในขอบเขต
- CLI commands
- signaling client
- WebRTC peer connection
- identity generation
- pairing flow
- PIN auth
- stream protocol
- shell/file/proxy/tcp/websocket handlers
- mDNS discovery
- QR code generation
- HTTP server integration
- test and release support

### 3.2 อยู่นอกขอบเขต
- การเขียน service worker ใหม่
- การเปลี่ยน protocol หลักที่ต้นฉบับใช้อยู่
- การสร้าง backend ใหม่แทน signaling เดิมโดยสมบูรณ์
- ฟีเจอร์ที่ไม่เกี่ยวกับ operation หลักของ bitbang-cli

---

## 4. Functional Requirements

### 4.1 Interactive Shell
ระบบต้องรองรับ remote shell ผ่าน WebRTC data channel
- ต้องเปิด pseudo-terminal
- ต้องส่ง input/output แบบ streaming
- ต้องรองรับ terminal resize
- ต้องรองรับ Linux/Windows/Android โดยใช้ implementation ที่เหมาะสมกับ platform

### 4.2 File Transfer
ระบบต้องรองรับ file transfer แบบ browse และ transfer
- list directory
- download file
- upload file
- stat file
- delete file
- handle metadata และ range ได้

### 4.3 Web Proxy
ระบบต้องรองรับ HTTP proxy สำหรับ access web app บนเครือข่ายปลายทาง
- map HTTP request/response ผ่าน stream
- rewrite headers ได้
- handle redirect ได้
- รองรับ streaming body

### 4.4 TCP Forwarding
ระบบต้องรองรับ raw TCP forwarding ผ่าน WebRTC
- open connection ไปยัง host/port ที่ระบุ
- ส่งข้อมูล bidirectionally
- handle close/error ได้ถูกต้อง

### 4.5 Pairing Code
ระบบต้องรองรับ pairing code 6 หลัก
- generate code
- exchange ผ่าน signaling
- ใช้เป็น part ของ authentication flow

### 4.6 PIN Authentication
ระบบต้องรองรับ PIN แบบ optional
- configure ได้
- verify แบบ constant-time
- จำกัดจำนวนครั้งที่ผิดได้
- มี delay ระหว่างการลองใหม่ได้

### 4.7 QR Code
ระบบต้องสามารถสร้าง QR code สำหรับ URL หรือ pairing link
- output เป็น terminal / file / image ได้ตาม implementation
- ใช้เพื่อช่วย pairing

### 4.8 mDNS Discovery
ระบบต้องรองรับ local discovery ผ่าน mDNS
- announce device ใน LAN
- discover service ใน local network

### 4.9 STUN/TURN
ระบบต้องรองรับ NAT traversal
- STUN สำหรับ discovery
- TURN สำหรับ relay fallback
- สามารถกำหนด ICE server ได้

### 4.10 WebSocket Bridging
ระบบต้องรองรับ WebSocket over WebRTC data channel
- connect ไปยัง target
- forward frames แบบ text/binary
- maintain connection lifecycle

---

## 5. Protocol Requirements

### 5.1 Signaling Protocol
- ใช้ WebSocket-based signaling
- message format เป็น JSON/protobuf-compatible schema ตาม design
- รองรับ register, request, offer, answer, candidate, error
- protocol version ต้องตรงกับต้นฉบับ

### 5.2 SWSP Protocol
- ใช้สำหรับ transport บน data channel
- stream id 0 เป็น control
- stream id 1+ เป็น data stream
- frame ต้องรองรับ SYN, FIN, DAT, MORE
- max frame size ต้องไม่เกินข้อจำกัดของ SCTP
- payload ที่ binary ต้องมีการ encode อย่างเหมาะสม

### 5.3 Pairing Protocol
- ใช้ commit-reveal flow
- มี nonce challenge
- มี SAS computation
- มี credential exchange
- ป้องกัน MITM ระดับพื้นฐานตามต้นฉบับ

### 5.4 Identity Protocol
- ใช้ RSA 2048-bit key pair
- identity ต้อง persist ได้
- uid และ user-visible pairing code (6 หลัก) ต้อง generate ได้ตรงตาม format; persistent access credential ถ้ามีต้องใช้ชื่อ `access_code` แยกต่างหาก
- ต้องรองรับ sign/decrypt ตามที่ protocol ระบุ

---

## 6. Non-Functional Requirements

### 6.1 Performance
- latency ต่ำ
- throughput เพียงพอสำหรับ shell/file/tcp stream
- memory usage คุมได้
- release binary ควร lean

### 6.2 Security
- ใช้ cryptographic primitives ที่ปลอดภัย
- หลีกเลี่ยง unwrap/expect ใน production
- PIN verification ต้องใช้ constant-time comparison
- key material ต้องไม่ log ออกมา

### 6.3 Reliability
- reconnect/error handling ต้องชัดเจน
- stream failure ต้องไม่ล่มทั้ง session ถ้าไม่จำเป็น
- invalid message ต้องถูก reject อย่างปลอดภัย

### 6.4 Portability
- Linux: PTY ใช้ `nix`
- Windows: PTY ใช้ `winapi`
- Android: ใช้ Unix-like PTY implementation

---

## 7. Acceptance Criteria

โปรเจคจะถือว่า pass specification เมื่อ:
- `serve`, `connect`, `cp` ทำงานได้
- เชื่อมต่อ signaling ได้จริง
- สร้าง WebRTC peer connection ได้
- ส่งข้อมูลผ่าน data channel ได้
- shell, file, proxy, tcp, websocket ใช้งานได้
- pairing และ PIN auth ใช้ได้
- build บน Linux/Windows/Android ได้
- มี test ครอบคลุมส่วนสำคัญ

---

## 8. Constraints

- ต้องคง protocol compatibility กับต้นฉบับ
- ต้องใช้ Rust ecosystem ให้มากที่สุด
- service worker ไม่อยู่ใน scope ของ Rust backend
- ต้องเตรียมความพร้อมสำหรับ integration กับ frontend เดิม

---

## 9. Protocol Decisions and Open Risks

ส่วนนี้เป็น decision record ที่ใช้ปิดความกำกวมก่อนเริ่มเขียน codec และ interoperability tests โดยไม่เปลี่ยน protocol semantics ของต้นฉบับ

### 9.1 Pairing Code Naming

**Pairing code ที่แสดงต่อผู้ใช้ต้องเป็นตัวเลข 6 หลัก** ตาม functional requirement ในเอกสารนี้ หาก implementation ต้องมี credential ภายใน 64-bit หรือ 11 ตัวอักษร base64url ให้ใช้ชื่อ `access_code` และแยก lifecycle จาก pairing code อย่างชัดเจน ห้ามใช้ field ชื่อ `code` แทนค่าทั้งสองประเภท

### 9.2 SWSP Wire Format

SWSP raw frame ใช้ header ขนาด 8 bytes ตามลำดับ little-endian ดังนี้: `stream_id` 4 bytes, `flags` 2 bytes และ `length` 2 bytes ตามด้วย payload ความยาว `length` ส่วน protobuf messages ใน `proto/swsp.proto` ใช้เป็น schema/control representation และยังไม่ถือว่าเป็น raw wire encoder จนกว่าจะมี compatibility fixture ยืนยัน

การ implement ต้องกำหนด max frame size, fragmentation/incomplete-frame behavior, invalid flag handling และ round-trip fixtures ก่อนผูกเข้ากับ WebRTC data channel

### 9.3 Rust Toolchain and Versioning

ใช้ Rust 2024 ตาม foundation plan และให้ `Cargo.toml`, `rust-toolchain.toml` หรือ container configuration เป็น source of truth เดียวกัน ข้อความ edition 2021 หรือ version ที่อยู่ใน roadmap เก่าให้ถือเป็น historical planning note จนกว่าจะมีการแก้ให้ตรงกับ manifest จริง

### 9.4 Implementation Status

ข้อกำหนดในเอกสารนี้อธิบาย target behavior ไม่ใช่หลักฐานว่า feature ถูก implement แล้ว ให้ตรวจสถานะจาก [`docs/implementation-status.md`](../docs/implementation-status.md) และใช้ acceptance criteria ในหมวด 7 เป็น release gate
