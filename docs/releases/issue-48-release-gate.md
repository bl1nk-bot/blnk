# Release gate และ artifact evidence ของ Issue #48

## สถานะ

**Implemented as a pre-release gate; ยังไม่ใช่ production release**

เอกสารนี้กำหนดวิธีสร้างและตรวจ artifact ของ blnk Rust ให้ทำซ้ำได้จาก source commit เดียวกัน โดย workflow จะตรวจสอบและ upload artifact เป็น evidence เท่านั้น ไม่สร้าง Git tag และไม่ publish GitHub Release อัตโนมัติ

## Source of truth

| รายการ | แหล่งข้อมูล |
|---|---|
| package name/version | `Cargo.toml` |
| dependency resolution | `Cargo.lock` และ `--locked` ทุกคำสั่ง build/test |
| Rust version/targets | `rust-toolchain.toml` (`1.97.0`) |
| source provenance | `GITHUB_SHA` ใน CI หรือ `git rev-parse HEAD` ใน local script |
| functional/security evidence | Issues #44, #45, #46, #47 และเอกสารที่ลิงก์จาก issue เหล่านั้น |
| automated gate | `.github/workflows/release-gate.yml` |
| local reproducible command | `scripts/release_gate.sh` |
| version notes | `CHANGELOG.md` |

## Reproducible Linux artifact

รันจาก repository checkout ที่ต้องการตรวจด้วยคำสั่งต่อไปนี้:

```bash
scripts/release_gate.sh
```

สคริปต์อ่าน version จาก `Cargo.toml` และ toolchain จาก `rust-toolchain.toml`, ปฏิเสธ host ที่ไม่ใช่ `x86_64-unknown-linux-gnu`, รัน `fmt`, `check`, `test` และ `clippy` ด้วย `--locked`, สร้าง `cargo build --locked --release`, ตรวจ executable version smoke test แล้วสร้าง:

```text
dist/blnk-v<version>-x86_64-unknown-linux-gnu.tar.gz
dist/blnk-v<version>-x86_64-unknown-linux-gnu.tar.gz.sha256
```

ภายใน tarball มี binary, README, VERSION และ `PROVENANCE.json` ซึ่งบันทึก package version, target, source commit, Rust toolchain, SHA-256 ของ `Cargo.lock`, build command และ smoke command ตัว archive ใช้ชื่อเรียงตามลำดับ, mtime epoch, owner/group เป็นศูนย์ และ gzip ไม่บันทึก timestamp เพื่อให้ metadata ที่ script ควบคุมได้ไม่เปลี่ยนตามเวลาสร้าง

การตรวจ checksum และ provenance ทำได้ด้วย:

```bash
sha256sum --check dist/*.tar.gz.sha256
tar -xOf dist/*.tar.gz '*/PROVENANCE.json'
```

การทำซ้ำบน commit และ environment เดิมควรได้ source/provenance เดิมและ archive ที่ตรงกันภายใต้ข้อจำกัดของ compiler/toolchain และ dependency cache ที่ถูก pin ด้วย `Cargo.lock`; workflow จึงเก็บ checksum/provenance ไว้ให้ตรวจย้อนหลัง แทนการอ้างว่า artifact ใด ๆ สามารถทำซ้ำได้โดยไม่ระบุ environment

## Platform evidence matrix

| Platform/target | Gate และ artifact | ระดับหลักฐาน | สิ่งที่ยังไม่สรุป |
|---|---|---|---|
| Linux `x86_64-unknown-linux-gnu` | format, check all targets, test all, clippy warnings-as-errors, locked release build, tarball/checksum, provenance และ `blnk --version` smoke | `proven` เมื่อ workflow job ผ่าน; local script ให้หลักฐานซ้ำในเครื่อง Linux | ไม่ใช่ external interoperability หรือ production deployment |
| Windows `x86_64-pc-windows-msvc` | format, locked check/test, locked release build, `blnk.exe --version` smoke, zip/checksum/provenance upload เป็น CI evidence | `runner-tested` เมื่อ job ผ่าน | ไม่อ้าง installer/signing, service installation หรือ production Windows deployment |
| Android `aarch64-linux-android` | setup NDK/clang ที่ pin แล้ว `cargo check --target ... --lib --locked` | `compile-only` เมื่อ job ผ่าน | ไม่อ้าง APK/AAB, emulator/device runtime, UI, signing หรือ store distribution |
| macOS | ไม่มี job และไม่มี artifact | `unsupported`/out of scope ตาม ADR-045 | ไม่อ้าง build, test หรือ runtime |

## Required checks before a release decision

ก่อนตัดสินใจ tag หรือ publish ต้องตรวจว่า release-gate workflow ผ่านทุก jobที่เกี่ยวข้อง ได้แก่ Linux artifact/smoke, Windows validation/package evidence, Android compile-only และ Rust dependency advisory audit นอกจากนั้นต้องตรวจ compatibility baseline ของ #44, platform matrix ของ #45, threat model/hardening ของ #46 และ browser control boundary ของ #47 โดยใช้ link ไปยัง issue/PR จริง ไม่ใช้ local fixture เพียงอย่างเดียวแทน interoperability evidence

ต้องตรวจ `cargo fmt --all -- --check`, `cargo check --all-targets --locked`, `cargo test --all --locked`, `cargo clippy --all --all-targets --locked -- -D warnings` และ `git diff --check` ให้ผ่านบน Linux การเปลี่ยน dependency, Rust toolchain, target, packaging layout, protocol, authentication boundary หรือ release script ต้องทำให้ gate ทำงานใหม่ก่อนใช้ evidence เดิม

## Dependency, logging, timeout และ cleanup checks

`Cargo.lock` ต้องถูกใช้แบบ locked ใน release build/test/compile commands และ workflow มี dependency advisory job แยกต่างหาก การ audit นี้เป็น advisory database check ไม่ใช่การรับประกันว่าไม่มี vulnerability ที่ยังไม่ถูกเผยแพร่

การตรวจ logging ใช้ source review และ tests ของ subsystem เดิมร่วมกับ browser surface ซึ่งไม่พิมพ์ bearer, CSRF, cookie, private key, pairing/access code หรือ registry raw contents ออกมา Error ของ browser surface ใช้รหัสทั่วไปและไม่สะท้อน credential

การตรวจ timeout/cleanup ต้องอาศัย tests ของ peer/session/stream/proxy และ fixture tests ของ browser surface สคริปต์ release gate ไม่เพิ่ม network operation หรือ process daemon ใหม่นอกเหนือจาก build/test/smoke binary และล้าง temporary staging directory เมื่อจบ; `dist` เป็น evidence output ที่ถูก ignore จาก source tree

## Rollback procedure

หาก artifact หรือ release ที่เผยแพร่ภายหลังพบปัญหา ให้หยุดการเผยแพร่และประกาศ version/target ที่ได้รับผลกระทบ จากนั้นเก็บ artifact/checksum/provenance ไว้เพื่อ audit, ถอนหรือซ่อน asset ที่ผิดตาม policy ของ GitHub repository, ชี้ผู้ใช้ไปยัง version ก่อนหน้าที่ผ่าน gate และเปิด issue ใหม่ที่อ้าง source commit และ job URL ของ artifact ที่มีปัญหา การ rollback นี้ไม่เปลี่ยน protocol หรือแก้ไข artifact เดิมแบบ in-place; ต้องสร้าง source commit ใหม่และรัน gate ใหม่เสมอ

ในสถานะปัจจุบัน workflow นี้ยังไม่ publish production asset จึงไม่มี release tag ที่ต้อง rollback คำสั่ง rollback เป็น procedure ที่เตรียมไว้สำหรับขั้นตอนหลังผู้ดูแลอนุมัติ release เท่านั้น

## Known limitations

หลักฐานของงานนี้ยังไม่ใช่ original-Go/provider interoperability, browser WebRTC, external signaling, production TLS, STUN/TURN, NAT traversal, reverse-proxy deployment, signed installer, package manager, APK/AAB, emulator/device runtime หรือ macOS support `blnk web` เป็น loopback session/status control surface ไม่ได้ยืนยันว่า browser เชื่อม remote peer ได้ ส่วน Android เป็น compile-only และ Windows artifact เป็น runner evidence จนกว่าจะมี signing/deployment evidence เพิ่มเติม

## References

- [Issue #44 — compatibility baseline](https://github.com/bl1nk-bot/blnk/issues/44)
- [Issue #45 — platform support matrix](https://github.com/bl1nk-bot/blnk/issues/45)
- [Issue #46 — threat model and hardening](https://github.com/bl1nk-bot/blnk/issues/46)
- [Issue #47 — browser control surface](https://github.com/bl1nk-bot/blnk/issues/47)
- [`docs/implementation-status.md`](../implementation-status.md)
- [`docs/decisions/issue-45-support-matrix.md`](../decisions/issue-45-support-matrix.md)
- [`docs/decisions/issue-46-threat-model.md`](../decisions/issue-46-threat-model.md)
