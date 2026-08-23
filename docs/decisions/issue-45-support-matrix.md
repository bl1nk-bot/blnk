# Decision Record: Issue #45 — Support Matrix และ Cross-Platform CI

## สถานะ

**Accepted for the current local-MVP repository.** เอกสารนี้กำหนดความหมายของคำว่า “รองรับ” สำหรับ Linux, Windows และ Android ให้แยกจากการ compile ผ่าน, การทดสอบบน runner และความพร้อมสำหรับ release production

## Context

โครงการประกาศขอบเขตแพลตฟอร์มไว้สามกลุ่ม ได้แก่ Linux, Windows และ Android ขณะที่ macOS อยู่นอก scope การมี source ที่ไม่มี `cfg` ผูกกับแพลตฟอร์มไม่เพียงพอที่จะยืนยันว่า runtime, process, filesystem, networking และ packaging ทำงานเหมือนกันทุกระบบ ก่อน Issue #45 CI มีเฉพาะ Linux jobs จึงไม่สามารถแยกได้ว่าข้อความใน README หมายถึง supported, buildable หรือเพียงออกแบบไว้

Issue นี้จึงเพิ่ม matrix ที่ตรวจสอบซ้ำได้และปรับ test fixtures ที่พึ่งพาคำสั่ง Unix ให้เลือก executable ตาม OS โดยไม่เปลี่ยน production shell policy หรืออ้างว่า remote signaling, external WebRTC, Android packaging หรือ original-Go interoperability เสร็จแล้ว

## Decision

### 1. ความหมายของระดับหลักฐาน

| ระดับ | ความหมาย | สิ่งที่ห้ามสรุปเกินหลักฐาน |
|---|---|---|
| `supported-scope` | อยู่ในขอบเขตผลิตภัณฑ์ตาม architecture/specification | ไม่ได้แปลว่ามี release artifact หรือ interoperability แล้ว |
| `compile-verified` | `cargo check` ผ่านบน target ที่ระบุ | ไม่ได้ยืนยัน runtime, linker/device หรือ system integration |
| `runner-tested` | unit/integration tests ผ่านบน OS runner ที่ระบุ | ไม่ได้ยืนยัน hardware, network provider, packaging หรือทุก shell environment |
| `local-smoke` | local fixture/harness ทำงานซ้ำได้ใน environment ควบคุม | ไม่ได้ยืนยัน external signaling, NAT traversal หรือ client เดิม |
| `release-verified` | มี build artifact, packaging, checksum และ smoke evidence ที่ตรวจย้อนหลังได้ | ยังต้องมี security/release sign-off ก่อนใช้งานจริง |

### 2. Matrix ที่ใช้ใน CI

| แพลตฟอร์ม/target | CI gate ใน Issue #45 | ระดับหลักฐานที่ได้ | สถานะปัจจุบัน |
|---|---|---|---|
| Linux / `x86_64-unknown-linux-gnu` | `fmt`, `clippy -D warnings`, `cargo check --all-targets`, `cargo test --all` | `runner-tested` และ `local-smoke` สำหรับ fixture ที่มีอยู่ | ผ่านใน sandbox และเป็น full gate หลัก |
| Windows / `x86_64-pc-windows-msvc` | `cargo check --all-targets`, `cargo test --all` บน `windows-latest` | `compile-verified` และ `runner-tested` เมื่อ GitHub job ผ่าน | มี job แล้ว; ยังไม่มี release artifact/interop claim |
| Android / `aarch64-linux-android` | `cargo check --target aarch64-linux-android --all-targets` | `compile-verified` เท่านั้น | compile-only; ยังไม่มี device/emulator smoke หรือ APK/AAB |
| macOS | ไม่มี job และอยู่นอก product scope | `not-tested` | ไม่รองรับตาม decision ปัจจุบัน |

CI ใช้ Rust toolchain `1.97.0` จาก `rust-toolchain.toml` และ workflow ทุก job ใช้ toolchain action ที่ pin เวอร์ชันเดียวกันเท่าที่ runner รองรับ การเพิ่ม target ใน toolchain เป็น configuration baseline ไม่ใช่หลักฐานว่า target นั้นมี linker, SDK หรือ packaging tool ครบในทุกเครื่อง

### 3. Cross-platform test fixtures

Shell tests และ shell integration tests ใช้ helper ที่เลือก command ตาม `cfg!(windows)` หรือ `cfg!(unix)` โดยส่ง executable และ arguments แบบ direct argv เช่นเดิม ห้ามกลับไปใช้ shell interpolation เพื่อทำให้ fixture สั้นลง เพราะจะทำลาย security contract ของ Issue #40

การปรับ fixture มีเป้าหมายเฉพาะให้ทดสอบ policy, output capture, timeout, cancellation และ exit handling บน runner ที่มีอยู่จริง ไม่ได้เปลี่ยน allowlist, environment clearing, path policy หรือ SWSP framing production behavior

### 4. CI trigger และ stacked PR

Workflow ทำงานเมื่อมี `pull_request` ทุก base branch เพื่อรองรับ stacked PRs ที่ชี้ไปยัง dependency branch และทำงานบน `push` ไป `main` สำหรับ baseline หลัง merge การแก้ workflow ไม่ได้เปิดสิทธิ์ merge อัตโนมัติและไม่ถือว่า PR ใดผ่านการ review หรือ release approval

### 5. Definition of done ของ Issue #45

Issue นี้ถือว่าผ่านเมื่อ repository มี pinned matrix สำหรับ Linux/Windows/Android, มี Windows-safe test fixtures ใน shell/runtime tests, Android มี compile-only gate ที่ระบุชัด, README/API/status/ADR แยกระดับ evidence ถูกต้อง และ Linux validation gate ผ่านโดยไม่ลด security หรือ protocol tests

Issue นี้ **ไม่** ถือว่าทำให้เสร็จในส่วนต่อไปนี้:

- binary release หรือ installer สำหรับ Linux, Windows หรือ Android;
- Android emulator/device smoke, APK/AAB packaging หรือ signing;
- external signaling/WebRTC, STUN/TURN, NAT traversal หรือ original-Go interoperability;
- PTY parity, filesystem permission parity และ network behavior ครบทุก OS;
- macOS support

## Consequences

ผลดีคือ reviewer และผู้ใช้เห็นได้ว่าแต่ละแพลตฟอร์มมีหลักฐานระดับใด และ CI จะจับ regression ที่เกิดจาก Unix-only test assumptions ได้เร็วขึ้น Android ไม่ถูกกล่าวอ้างเกินจริงเพราะ gate เป็น compile-only ส่วน Windows มี job ที่ตรวจ build/test บน runner จริง

ข้อแลกเปลี่ยนคือ workflow ใช้เวลาและทรัพยากรมากขึ้น และบางข้อผิดพลาดจะพบเฉพาะเมื่อ runner มี SDK/toolchain ที่เหมาะสม การผ่าน CI ยังต้องอ่านควบคู่กับ readiness document และไม่แทนที่ interoperability หรือ release verification

## Evidence

หลักฐานใน repository ประกอบด้วย `.github/workflows/ci.yml`, `rust-toolchain.toml`, shell/runtime platform-aware tests, README support wording และผล `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo test --all`, `cargo clippy --all --all-targets -- -D warnings` บน Linux ผล CI ของ Windows/Android ต้องอ้างจาก job run จริงเมื่อมีผลบน GitHub; local Linux pass ไม่ถูกนำไปแทนหลักฐานของสอง target นั้น

การลอง `cargo check --target aarch64-linux-android --all-targets` ใน sandbox ยังไม่ใช่ Android evidence เพราะ environment ไม่มี Android NDK/clang compiler ที่ `ring` ต้องใช้ จึงบันทึกเป็น local tooling blocker ไม่ใช่ผลผ่านหรือผลล้มของ target; workflow ติดตั้ง NDK รุ่นที่ pin ไว้ก่อนเรียก gate เพื่อให้ runner สะอาดทำซ้ำได้

## Related decisions

- [`docs/architecture.md`](../architecture.md) — cross-platform และ release architecture
- [`docs/implementation-status.md`](../implementation-status.md) — canonical evidence matrix และ readiness
- [`docs/api.md`](../api.md) — API contract และ compatibility evidence levels
- [`Issue #45`](https://github.com/bl1nk-bot/blnk/issues/45)

## Verification matrix

| กรณี adversarial/regression | หลักฐาน |
|---|---|
| shell command path แตกต่างระหว่าง Unix/Windows | platform-aware test helper และ Windows CI test job |
| test fixture ใช้ `/bin/sh` โดยไม่ตั้งใจ | source review + `cfg`-scoped command selection |
| Android ถูกกล่าวอ้างว่า runtime ใช้งานได้จาก compile ผ่าน | เอกสารระบุ `compile-verified` เท่านั้น และไม่มี device smoke gate |
| PR stacked ไม่ได้รับ CI เพราะ base ไม่ใช่ `main` | `pull_request` trigger ไม่จำกัด base branch |
| local Linux pass ถูกนำไปกล่าวอ้างเป็น external interoperability | README/status/ADR แยก `local-smoke` จาก `release-verified` และ `original-Go interoperability` |
| macOS ถูกตีความว่าเป็น supported platform | matrix ระบุ `not-tested` และ architecture scope ระบุ out-of-scope |

## Open follow-up

Release artifact/packaging, Android device evidence, PTY parity, external interoperability และ production network validation ต้องมี issue/decision record แยกต่างหาก ไม่รวมอยู่ใน acceptance ของ Issue #45 ส่วน Android local compile ยังติดตั้ง NDK/clang ไม่ครบใน sandbox และต้องรอผลจาก workflow ที่ provisioning toolchain แล้ว

Closes #45 เมื่อ PR ที่นำ decision นี้ไปใช้ถูก merge ตาม workflow ของ repository

---

**หมายเหตุ:** โฟลเดอร์ `v1` ใน compatibility fixtures ของ Issue #44 เป็น fixture-format version ไม่ใช่ protocol version หรือ platform version
[1]: ../architecture.md
[2]: ../implementation-status.md
[3]: ../api.md
