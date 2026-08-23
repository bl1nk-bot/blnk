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

**สถานะปัจจุบัน: local MVP / ยังไม่พร้อมใช้งานจริง** โครงการมี runnable foundation, identity, signaling/WebRTC local harness, authenticated session, file-transfer/shell MVP และ CLI fixture workflow แล้ว แต่ remote signaling-to-WebRTC orchestration, external interoperability, production proxy integration และ web integration ยังอยู่ใน roadmap การมีคำสั่ง CLI หรือ protobuf schema เพียงอย่างเดียวไม่ถือว่า acceptance criteria ผ่านจนกว่าจะมี implementation และ integration tests รองรับ

ผลตรวจสอบล่าสุดและรายการงานที่ต้องทำอยู่ใน [`docs/implementation-status.md`](docs/implementation-status.md) ส่วนลำดับความสำคัญและกติกาแก้ความขัดแย้งของเอกสารอยู่ใน [`docs/plans/core-foundation.md`](docs/plans/core-foundation.md)

## Quick Start

> **หมายเหตุ:** คำสั่งด้านล่างแยกเป็น local-MVP workflow กับ remote workflow อย่างชัดเจน การใช้ `--local-fixture` ทำ authenticated in-process test โดยไม่ต้องใช้ external secret; remote signaling/WebRTC orchestration ยังไม่อ้างว่าใช้งานได้จริง


### สร้างโปรเจค

```bash
cargo init blnk
cd blnk
```

### build

```bash
cargo build
```

สำหรับการตรวจสอบก่อนเปิด Pull Request ให้รันคำสั่ง foundation verification ต่อไปนี้:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all
cargo clippy --all --all-targets -- -D warnings
```

บน Linux คำสั่งทั้งสี่ผ่านสำหรับ runnable foundation ในปัจจุบัน ส่วนการผ่าน acceptance ของ protocol และการใช้งานจริงยังต้องมี compatibility, integration และ cross-platform evidence ตาม [`docs/implementation-status.md`](docs/implementation-status.md)

### run

```bash
# สร้าง/โหลด identity และเริ่ม local fixture แบบ one-shot
cargo run -- serve --local-fixture --once

# authenticated shell transcript ผ่าน local WebRTC fixture
cargo run -- connect --local-fixture
cargo run -- connect --local-fixture --command printf "%s\\n" hello

# authenticated file-transfer transcript
cargo run -- cp --local-fixture ./local.txt remote.txt

# ดู metadata ของ devices ที่รู้จัก โดยไม่เปิดเผย credential
cargo run -- devices --list
```

## CLI Commands

- `serve` - โหลด/สร้าง identity และเริ่ม service; `--local-fixture` ใช้ local one-shot/running fixture
- `connect` - ใช้ `--local-fixture` เพื่อ authenticated shell transcript; remote target ตรวจ registry และรายงานข้อจำกัดอย่างชัดเจน
- `cp` - ใช้ `--local-fixture` เพื่อ authenticated sandboxed file transfer
- `devices` - อ่าน persistent metadata-only device registry โดยไม่แสดง private key หรือ access code
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
