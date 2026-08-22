# Decision Record: Compatibility Baseline จาก local harness

- **Issue:** #44
- **สถานะ:** Accepted สำหรับ local compatibility baseline; external interoperability ยังไม่ proven
- **ฐาน branch:** `feat/cli-issue-43`
- **ขอบเขต:** signaling JSON, SWSP raw frame, control protobuf และ authenticated local session

## บริบท

Issue #44 ต้องสร้างหลักฐาน compatibility ที่ตรวจสอบซ้ำได้ ก่อนจะอ้างว่า Rust implementation ทำงานร่วมกับ original Go client/server ได้ การมี schema หรือการที่ local unit test ผ่านยังไม่เพียงพอ เพราะยังไม่ยืนยัน byte shape, directionality, ordering, close/error semantics หรือ version negotiation บน wire จริง

ดังนั้น baseline นี้แยก **fixture-format version** ออกจาก **protocol version** อย่างชัดเจน โฟลเดอร์ `tests/fixtures/compatibility/v1/` หมายถึงรูปแบบการจัดเก็บ fixture รุ่นแรก ส่วนค่า `protocol` และ `protocol_version` ในข้อมูลยังต้องตรงกับ `PROTOCOL_VERSION` ของ crate ปัจจุบัน ซึ่งขณะนี้คือ `3`

## การตัดสินใจ

### 1. ใช้ versioned raw fixtures เป็น source of truth

Fixture ที่ commit ใน repository ต้องเป็นข้อมูลที่ตรวจสอบได้แบบ deterministic และ test ต้องอ่าน fixture เหล่านั้นโดยตรง ไม่สร้าง expected bytes ใหม่ใน runtime ของ test โดยไม่บันทึกเป็น artifact แยก มี fixture รุ่นแรกดังนี้

| Fixture | รูปแบบ | สิ่งที่พิสูจน์ใน local baseline |
|---|---|---|
| `signaling_register.json` | UTF-8 JSON text | discriminator `type`, field names, protocol value และ `want_code` |
| `swsp_open_hello.hex` | hex ของ raw bytes | 8-byte SWSP header, stream ID, `SYN|DAT`, payload length และ payload |
| `control_connect_v1.hex` | hex ของ protobuf payload | field numbers/values ของ `ConnectMessage` และ round-trip decode |
| `session_handshake_v1.json` | JSON transcript metadata | ลำดับ `connect → auth_required → auth → auth_result → ready` และ routing mode |

Fixture ชื่อ `*_v1` หรืออยู่ใต้ `v1/` จึงไม่ควรถูกตีความว่าเป็น protocol version 1 หากค่าภายใน fixture ระบุ protocol version อื่น

### 2. ตรวจ fixture ทั้งในระดับ raw และ typed API

`tests/compatibility_baseline.rs` ต้องตรวจสองทิศทางเมื่อทำได้ กล่าวคือ encode typed message แล้วเทียบกับ raw fixture และ decode raw fixture กลับมาเทียบกับ typed value การตรวจแบบนี้จับทั้งการเปลี่ยน field name, field number, flag, endian หรือ payload โดยไม่ต้องพึ่ง external service

SWSP fixture ต้องตรวจ consumed length เท่ากับ input ทั้งหมด เพื่อป้องกัน decoder ที่ยอมรับ trailing bytes เงียบ ๆ ส่วน signaling fixture ต้องตรวจ JSON encode แบบ byte-for-byte เพราะ field order และ wire discriminator เป็นส่วนหนึ่งของ compatibility contract ใน transport ปัจจุบัน

### 3. ใช้ local authenticated session เป็น transcript evidence

ใช้ `TwoPeerHarness` กับ `SessionRuntime` สองฝั่งใน process เดียวกันและ PIN ที่สร้างใน test เพื่อให้ test ทำงานได้จาก clean environment โดยไม่ต้องมี external secret, signaling provider หรือ STUN/TURN การทดสอบต้องยืนยันว่า server และ client ไปถึง `SessionState::Ready` และปิด cleanly ได้หลัง handshake

หลักฐานนี้ยืนยัน local control lifecycle และ cleanup เท่านั้น ไม่ยืนยัน original-Go interoperability, remote signaling, NAT traversal หรือ production deployment

### 4. จัดระดับ provenance ของ compatibility evidence

| Feature | ระดับหลักฐาน | คำอธิบาย |
|---|---|---|
| Signaling register JSON | `fixture-only` และ `local-typed-roundtrip` | fixture สร้างจาก wire schema/encoder ใน repository; ยังไม่มี original-Go capture |
| SWSP open frame | `fixture-only` และ `local-raw-roundtrip` | ตรวจ bytes จริงด้วย decoder/encoder ใน repository; ยังไม่มี upstream capture |
| Control connect protobuf | `fixture-only` และ `local-decode` | ตรวจ field numbers จาก schema/generated API; ยังไม่มี original-Go capture |
| Authenticated session | `proven-local` | local two-peer E2E ถึง Ready และ close cleanly ได้ |
| Original Go client/server | `not-tested` | branch นี้ไม่มี executable หรือ fixture จาก original Go ที่ตรวจสอบสิทธิ์ได้ |
| Linux/Windows/Android interoperability | `not-tested` | validation รอบนี้รันใน Linux เท่านั้น |

### 5. ห้ามขยาย claim เกินหลักฐาน

เอกสาร, PR body และ release notes ต้องใช้คำว่า **local baseline**, **fixture-only**, **proven-local** หรือ **not-tested** ตามตารางข้างต้น ห้ามใช้คำว่า compatible, interoperable หรือ production-ready สำหรับ original client/server จนกว่าจะมี capture ที่ระบุ executable/version, input, output และผลตรวจซ้ำได้

## API boundary สำหรับงานถัดไป

Issue #45, #46, #47 และ #48 สามารถ reuse fixture layout และ test helper conventions นี้ได้ แต่ต้องเพิ่ม fixture ใหม่เมื่อเปลี่ยน feature หรือ protocol direction โดยห้ามแก้ fixture เดิมเพื่อทำให้ test ผ่านโดยไม่มี decision record

งานที่ต้องการ original-Go comparison ต้องแนบ provenance เพิ่มเติมอย่างน้อย executable identity/version, command line, sanitized environment, raw input/output และ expected result การไม่มี external secret ไม่ใช่เหตุผลให้ใช้ mock แล้วประกาศ interoperability

## Validation evidence

บน Linux branch นี้ต้องผ่านคำสั่งต่อไปนี้ก่อนเปิด PR:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all
cargo clippy --all --all-targets -- -D warnings
git diff --check
```

ชุด compatibility baseline มี test แยก 4 รายการสำหรับ signaling, SWSP, control และ authenticated session ส่วน test ทั้ง repository ต้องผ่านโดยไม่ใช้ external provider

## ผลกระทบและความเสี่ยง

การเก็บ raw fixtures ทำให้ protocol drift ตรวจพบเร็วและ review diff ได้ แต่ fixture ภายใน repository ยังไม่เท่ากับหลักฐานจาก original Go การเพิ่ม fixture ต้องรักษาความลับ โดยห้าม commit private key, PIN จริง, access credential, session token หรือ endpoint ภายใน

## References

[1]: ../../docs/api.md "blnk Rust API contract"
[2]: ../../docs/implementation-status.md "Canonical implementation status"
[3]: ../../src/protocol/swsp.rs "SWSP raw frame codec"
[4]: ../../src/session/runtime.rs "Authenticated SessionRuntime"
[5]: ../../src/signaling/transport.rs "Signaling transport codec"
[6]: ../../tests/compatibility_baseline.rs "Compatibility baseline tests"
