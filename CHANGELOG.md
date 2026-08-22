# Changelog

การเปลี่ยนแปลงสำคัญของ blnk Rust จะบันทึกตามลำดับ version โดย release gate ต้องสร้าง artifact จาก source commit และ `Cargo.lock` ที่ระบุไว้ใน provenance ก่อนเผยแพร่ทุกครั้ง

## [0.1.0] — MVP baseline (ยังไม่เผยแพร่ production)

รุ่น baseline นี้รวม runnable Rust foundation, identity/pairing primitives, SWSP codec, typed signaling/session/stream boundaries, local WebRTC two-peer harness, authenticated session runtime, sandboxed file transfer, bounded shell stream, proxy policy/handlers, CLI local และ deterministic remote relay orchestration รวมถึง loopback-only browser control surface ที่สร้าง local session/status flow ผ่าน HTTP/WebSocket

Release gate evidence ของรุ่นนี้ต้องแยกเป็น Linux artifact ที่มี checksum/provenance และ smoke test, Windows build/test กับ binary smoke test และ Android `aarch64-linux-android` compile-only evidence ส่วน macOS, browser WebRTC interoperability, original-Go/provider interoperability, production TLS, STUN/TURN, NAT traversal, device/emulator runtime และ production deployment ยังไม่อยู่ในหลักฐานของรุ่นนี้

รายการ artifact นี้ยังไม่ใช่ production release และไม่มี tag หรือการเผยแพร่ asset อัตโนมัติจาก workflow `release-gate.yml`

[0.1.0]: https://github.com/bl1nk-bot/blnk/issues/48
