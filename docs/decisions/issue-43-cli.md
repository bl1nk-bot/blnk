# Decision Record: CLI MVP และ Local Fixture Workflow

**สถานะ:** Accepted for Issue #43 MVP; remote boundary ในเอกสารนี้เป็น historical scope และถูก supersede โดย ADR #71 สำหรับ remote orchestration

## Context

คำสั่ง `serve`, `connect`, `cp` และ `devices` ใน `src/main.rs` ต้องเลิกคืนค่า success แบบ placeholder และต้องเชื่อมกับ identity, config, authenticated `SessionRuntime`, shell/file stream และ device registry ที่มีอยู่จริง โดย Issue นี้ไม่ควรอ้าง remote signaling-to-WebRTC orchestration หรือ external-provider interoperability ที่ยังไม่มีหลักฐาน

Acceptance ต้องตรวจสอบซ้ำได้โดยไม่ใช้ external secret จึงต้องมี local fixture mode ที่สร้าง `TwoPeerHarness` ใน process, ทำ authenticated handshake ด้วย PIN ที่กำหนดใน config/argument และส่ง transcript ผ่าน SWSP stream จริง

## Decisions

### 1. แยก local MVP กับ remote capability boundary

`serve --local-fixture` เริ่ม `LocalFixtureServer` และสร้างหรือโหลด identity ส่วน `connect --local-fixture` และ `cp --local-fixture` ใช้ `TwoPeerHarness` กับ `SessionRuntime` จริง ไม่สร้าง mock success path

ในขอบเขตเดิมของ Issue #43 หากเรียก remote path ในขณะที่ orchestration ยังไม่มี คำสั่งต้องคืน error ที่บอกข้อจำกัดอย่างชัดเจนแทนการพิมพ์ success ปลอม `connect --target` ตรวจ `DeviceRegistry` ก่อน และปฏิเสธ unknown device ก่อนมีการ dial ใด ๆ; หลัง Issue #71 remote path ใช้ orchestration จริงตาม [ADR #71](issue-71-remote-orchestration.md)

### 2. ใช้ direct argv สำหรับ shell transcript

`connect --command` รับ executable และ arguments เป็นรายการแยก แล้วสร้าง `ShellCommand` ด้วย `Command::new(program).args(args)` ฝั่งรับไม่ผ่าน shell interpolation จึงไม่ตีความ metacharacters เป็นคำสั่งเพิ่มเติม ค่าเริ่มต้นเป็นคำสั่งที่ไม่ใช้ secret และทำงานได้บน Linux/Windows ตาม platform branch ของ shell MVP

### 3. ใช้ authenticated file stream สำหรับ `cp`

`cp --local-fixture SOURCE DESTINATION` ใช้ upload เมื่อ source เป็น local path และใช้ download เมื่อ source มี prefix `remote:` เปิด file stream ผ่าน `SessionRuntime`, ส่ง request/data chunks ตาม SWSP codec, ให้ `FileTransferService` ตรวจ sandbox/size/overwrite policy แล้วรับ response/เขียน destination หลัง response จบเท่านั้น

การสร้าง temporary fixture root ใช้ UUID และลบเมื่อจบทุก path ที่ CLI ควบคุมได้ การถ่ายโอนไม่พิมพ์ PIN, private key, access code หรือ environment ออกมา

### 4. Device registry เก็บ metadata เท่านั้น

`DeviceRegistry` บันทึกเฉพาะ device ID, endpoint และ `last_seen_unix` ใน path จาก config การเรียก `devices --list` แสดงข้อมูลสามฟิลด์นี้เท่านั้น และไม่มี private key, pairing code หรือ access code ใน registry การบันทึกใช้ temporary file แล้ว rename เพื่อไม่เขียนไฟล์ปลายทางแบบครึ่งเดียวใน normal path

local fixture จะ upsert device ID `local-fixture` พร้อม endpoint แบบ `in-process://loopback` หลัง transcript สำเร็จ เพื่อให้ตรวจสอบ unknown/offline behavior ได้โดยไม่ต้องมี server จริง

### 5. Lifecycle และ error behavior

ทุก local command ทำ handshake ก่อนเปิด stream, ปิด stream และปิด runtime/peer เมื่อสำเร็จหรือหลังได้รับ terminal error ที่จัดการได้ ส่วน remote path ที่ยังไม่มี implementation คืน error typed/contextual ของ CLI และไม่สร้าง registry record ว่าการเชื่อมต่อสำเร็จ

`serve --once` ใช้สำหรับ deterministic initialization/fixture smoke test แล้วคืน control หลังหยุด fixture ส่วน `serve` ปกติรอ Ctrl-C และปิด fixture อย่าง graceful หากระบบส่ง shutdown signal

## Security constraints

CLI ไม่พิมพ์ identity PEM/private key, PIN, pairing/access credential หรือ raw config ออก stdout การโหลด identity ใช้ persistence boundary เดิม และ local shell ใช้ policy ของ `ShellStreamHandler`; file transfer ใช้ sandboxed `FileTransferService` ไม่เปิด path ภายนอก sandbox ฝั่งรับ

การใช้ `--local-fixture` เป็น explicit opt-in สำหรับ local test เท่านั้น ไม่ใช่หลักฐานของ network reachability, NAT traversal, remote provider compatibility หรือ production authorization

## Evidence

| Acceptance area | Evidence in this branch |
|---|---|
| Config/identity wiring | `Config::load`, `load_or_create_identity`, config unit tests และ identity persistence tests |
| Input validation | clap argument tests, direct-argv preservation test และ file/shell policy tests |
| Missing device | `unknown_device_is_rejected_before_remote_dial` ตรวจ registry ก่อน remote path |
| Timeout/disconnect | SessionRuntime, peer lifecycle, shell cancellation/timeout และ file cancellation tests ที่รันร่วมใน suite |
| Repeatable local transcript | `connect --local-fixture` และ `cp --local-fixture` ใช้ in-process authenticated WebRTC fixture โดยไม่ใช้ external secret |
| Validation gate | `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo test --all` (75 passed), `cargo clippy --all --all-targets -- -D warnings` และ `git diff --check` ผ่านบน Linux |

## Non-goals and follow-up

Issue นี้ไม่รวม production packaging, mDNS/QR discovery, browser UI, TLS/provider interoperability หรือ verified Linux/Windows/Android release artifacts งานถัดไปจาก Issue #43 คือ [Issue #71](issue-71-remote-orchestration.md) ซึ่งเพิ่ม remote dispatch และคง local fixture เป็น deterministic regression harness
