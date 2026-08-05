# blnk Rust

>blnk Rust คือการพอร์ต `bitbang-cli` จากภาษา Go มาเป็น Rust โดยรักษาความสามารถหลักให้เทียบเท่าต้นฉบับ และออกแบบใหม่ให้เหมาะกับการพัฒนา, ทดสอบ, และดูแลรักษาในระยะยาว

## วัตถุประสงค์

- พอร์ตฟีเจอร์จาก `bitbang-cli` ให้ครบถ้วน
- ใช้ Rust เพื่อเพิ่ม memory safety และความคุมได้ของระบบ
- ออกแบบสถาปัตยกรรมใหม่ให้ modular และ testable
- รองรับ cross-platform: Linux, Windows, Android
- เตรียมโครงสร้างสำหรับการพัฒนาต่อในอนาคตอย่างเป็นระบบ

## ภาพรวมระบบ

blnk เป็น CLI tool สำหรับ remote access แบบ peer-to-peer ผ่าน WebRTC โดยไม่ต้องพึ่ง account และไม่ต้องเปิด port forwarding เอง
ความสามารถหลัก:
- remote shell
- file transfer
- HTTP web proxy
- TCP forwarding
- WebSocket bridging
- pairing code
- PIN authentication
- mDNS discovery
- STUN/TURN NAT traversal
- QR code generation
> หมายเหตุ: service worker เป็นฝั่ง browser/frontend อยู่แล้ว ไม่ต้อง implement ใน Rust CLI

## ขอบเขตโปรเจค

>โปรเจคนี้จะพัฒนา Rust implementation ให้ทำงานแทนต้นฉบับ Go โดยต้องรักษา protocol compatibility ให้เทียบเท่าเดิม โดยเฉพาะ:

- signaling protocol
- pairing flow
- SWSP protocol
- identity format
- control messages
- stream handling

## เป้าหมายคุณภาพ

- single static binary
- async-first architecture
- error handling ชัดเจน
- cross-platform support
- test coverage สูง
- release automation พร้อม

## สถานะการพัฒนา

[NOTE!] เอกสารนี้เป็นจุดเริ่มต้นของการพัฒนา โปรเจคยังอยู่ในระยะวางโครงสร้าง

## Quick Start

### สร้างโปรเจค

```bash
cargo init blnk
cd blnk
```

### build

```bash
cargo build
```

### run

```bash
cargo run -- serve
cargo run -- connect
cargo run -- cp
```

## CLI Commands

- `serve` - โหมดรับการเชื่อมต่อและให้บริการ
- `connect` - เชื่อมต่อไปยัง device ผ่าน signaling
- `cp` - คัดลอกไฟล์ระหว่างปลายทาง
- `devices` - จัดการรายการอุปกรณ์
- `version` - แสดงเวอร์ชัน

## Dependencies หลัก

- `tokio` - async runtime
- `webrtc` - WebRTC implementation
- `axum` - HTTP server
- `clap` - CLI parsing
- `tracing` - logging
- `thiserror` - error handling
- `serde` - serialization
- `prost` - protobuf support
- `qrcode` - QR code generation
- `mdns` - local discovery
- `nix` / `winapi` - PTY support

## การใช้งานโปรเจคนี้

โปรเจคนี้ถูกออกแบบมาเพื่อ:
- ใช้งานเป็น CLI tool
- เป็นฐานสำหรับพัฒนาแบบ modular
- รองรับการต่อยอดเป็น library ได้ด้วย

## เอกสารเพิ่มเติม

- `specs/spec.md`
- `docs/architecture.md`
- `docs/api.md`
- `docs/blueprint.md`
- `STYLE.md`
- `TODO.md`

## License

MIT
