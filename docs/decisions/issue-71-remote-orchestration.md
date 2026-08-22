# Decision Record: Remote Signaling และ WebRTC Orchestration

**สถานะ:** Implemented in Issue #71 branch; external provider interoperability remains unverified

## Context

ก่อนงานนี้คำสั่ง `serve`, `connect --target` และ remote `cp` มีเพียง local fixture path ที่เรียก `TwoPeerHarness` จริง ส่วน remote path หยุดด้วยข้อความ `not implemented` แม้ signaling transport, `PeerHandle` และ authenticated `SessionRuntime` จะมีอยู่แล้ว ทำให้ CLI ยังไม่สามารถทำ flow ตาม architecture ได้

## Decisions

### 1. ใช้ signaling connection เดียวต่อหนึ่ง negotiation

`serve` เปิด WebSocket ผ่าน `SignalingClient`, ส่ง `RegisterRequest`, ตรวจ `RegisteredResponse`, รอ `Request` หรือ `PairRequest`, สร้าง data channel และ offer แล้วรอ `Answer` ที่มี `client_id` ตรงกัน การปิด socket signaling หลัง WebRTC/session พร้อมแล้วไม่ตัด session เพราะ data channel เป็น transport หลัก

`connect --target` ใช้ `DeviceRegistry` เป็นตัวเลือก endpoint ของ target และส่ง `ConnectionRequest` โดยใช้ UID ของ client เป็น `client_id` จากนั้นรับ offer, ตรวจ client ID และ metadata, สร้าง answer และส่งกลับบน connection เดิม วิธีนี้คง message types ใน `proto/signaling.proto` และไม่สร้าง wire message ใหม่สำหรับ target routing; endpoint ใน registry จึงต้องเป็น endpoint ที่ signaling server ใช้ route ไปยัง target device

### 2. ใช้ non-trickle SDP ตาม PeerHandle boundary

offer และ answer ถูก parse เป็น `RTCSessionDescription` แล้วส่งเข้า `PeerHandle::accept_offer` หรือ `set_remote_answer` หลัง ICE gathering เสร็จ การได้รับ `candidate` ใน flow นี้คืน error ชัดเจน เพราะ branch นี้ยังไม่มี public ICE-candidate adapter และไม่อ้าง trickle interoperability

### 3. ยืนยัน encrypted request ด้วย public key ที่ส่งใน offer metadata

`OfferMessage.streams["device_public_key"]` เก็บ DER public key ของ device ที่ถูก base64 encode โดย transport เมื่อ client ได้ offer จะ decode และ parse key ก่อนสร้าง `encrypted_request` ด้วย RSA-OAEP/SHA-256 การเข้ารหัสเป็น opaque protocol payload; PIN authentication ที่ให้สิทธิ์ session ยังคงทำใน `SessionRuntime` และไม่ log plaintext, key หรือ credential

metadata key นี้เป็น extension บน typed `streams` field ที่ schema ระบุไว้สำหรับ stream metadata จึงไม่เปลี่ยน discriminator หรือ message direction แต่ original signaling provider ต้องรองรับการส่งต่อ map นี้ก่อนจึงจะอ้าง interoperability ได้

### 4. ให้มี single-reader session dispatcher

หลัง authenticated `Ready` server ใช้ dispatcher เดียวอ่าน SWSP frames แล้วจำแนก `stream_id`/flags ก่อนเรียก service ที่มีอยู่จริง หากเป็น shell จะตรวจและรัน `ShellStreamHandler`; หากเป็น file จะรับ stream, เก็บ bounded chunks, เรียก `FileTransferService` และส่ง response frames กลับไป control stream ไม่เปิด reader หลายตัวบน data channel เดียวกัน

stream opener ที่ไม่ใช่ `SYN|DAT`, control frame ที่ไม่ใช่ terminal `FIN`, unknown stream ID และ frame ที่ decode ไม่ได้จะคืน typed error และปิด session ไม่ถูกตีความเป็น success

### 5. Timeout, reconnect และ cleanup

reconnect แบบจำกัดใช้เฉพาะตอนเปิด signaling socketหนึ่งครั้ง ส่วน negotiation ที่เริ่มแล้วไม่ replay อัตโนมัติ เพื่อป้องกันการส่ง offer/answer ซ้ำไปยัง state ที่ไม่ตรงกัน ทุกขั้น signaling และ peer readiness มี timeout 15 วินาที และ session handshake ใช้ timeout เดียวกัน การ timeout, signaling error, client ID mismatch, remote disconnect หรือ peer failure จะปิด peer/runtime แบบ best effort แล้วส่ง error กลับ caller

`serve` ใช้ cancellation token เพื่อให้ Ctrl-C ปิด dispatcher, runtime และ peer อย่างเป็นลำดับ `connect` และ `cp` ปิด stream และ runtime หลัง terminal response หรือ error

## Evidence

| Acceptance area | Evidence |
|---|---|
| Register/request/offer/answer | `accept_server_session`, `connect_target` และ relay fixture test ใน `src/signaling/orchestration.rs` |
| Authenticated session | fixture test สร้าง WebRTC peers ผ่าน signaling relay แล้วให้ทั้งสองฝั่งถึง `SessionState::Ready` |
| Shell dispatch | remote shell E2E ใช้ `ShellStreamHandler` และส่ง output/exit frame จริง |
| File dispatch | remote upload E2E ใช้ `FileTransferService` และตรวจไฟล์ที่ sandbox receiver root; client รองรับ download direction ผ่าน `remote:` |
| Failure handling | tests สำหรับไม่มี device, timeout, client ID mismatch และ malformed/typed signaling boundary ที่มีอยู่เดิม |
| Secret boundary | encrypted request เป็น ciphertext, public key/identity ไม่ถูกพิมพ์หรือใส่ registry |

## ข้อจำกัดที่ยังคงอยู่

หลักฐานในงานนี้เป็น local deterministic relay และ local UDP WebRTC fixture เท่านั้น ยังไม่ใช่หลักฐานของ original-Go/server provider, browser interoperability, TLS trust, STUN/TURN, NAT traversal, production deployment หรือ Windows/Android runtime. `EndpointPolicy::PublicOnly` ยังคงเป็นค่าเริ่มต้นสำหรับ remote path และ local fixture ต้องเลือก `AllowLocal` อย่าง explicit เท่านั้น
