# Decision: Shell stream MVP boundary

**สถานะ:** Accepted for Issue #40 MVP

## Context

Repository มี `ShellInput`, `ShellOutput` และ `TerminalResize` อยู่ใน `proto/stream.proto` แต่ยังไม่มี message สำหรับเปิด process, จบ process, error หรือ cancellation การทำ Shell MVP ต้องส่ง stdin/stdout/stderr, exit status, timeout และ cancellation ผ่าน authenticated SWSP stream โดยไม่อ้าง full PTY หรือ original-client interoperability

## Decision

เพิ่ม protobuf messages `ShellOpen`, `ShellExit`, `ShellError` และ `ShellCancel` ใน package `stream` และคง `ShellInput`, `ShellOutput` และ `TerminalResize` เดิมไว้เป็น wire boundary ของ shell stream ทุก message ถูกห่อด้วย SWSP frame flags เดิม: `SYN|DAT` สำหรับ open, `DAT|MORE` สำหรับ input/output/resize/error, และ `DAT|FIN` สำหรับ exit/cancel/stream completion

`ShellOpen` ส่งเฉพาะ executable, direct argv และ relative working directory ไม่ส่ง environment map หรือ shell command string ที่ต้องผ่าน shell interpolation ฝั่งรับต้องตรวจ `ShellPolicy` ในเครื่องก่อน spawn process การเลือก executable, working-directory root, environment allowlist, output limit และ timeout เป็น local policy ไม่ใช่สิทธิ์ที่ peer ประกาศเอง

MVP ใช้ pipe-based process I/O โดยแยก stdout และ stderr เป็นคนละ message type ไม่อ้าง PTY, terminal resize ที่ทำงานได้จริง, Android production shell, browser shell หรือ interoperability กับ original client ใน Issue นี้ `TerminalResize` จึงถูก decode และ validate ได้ แต่ handler จะรายงานว่า PTY resize ไม่รองรับใน pipe mode

## Compatibility evidence

- Generated protobuf round-trip tests cover all shell message types.
- Shell frame codec tests cover each message and SWSP flag combination.
- Local two-peer runtime tests cover open, stdin, stdout/stderr, exit, cancellation and FIN cleanup.
- No existing wire message is renamed or repurposed; the new fields use previously unused message names in the shell section of `stream.proto`.

## Security constraints

Command execution uses `Command::new(program).args(args)` and never invokes an implicit shell. The receiver rejects empty programs, absolute or parent-traversing working directories, disallowed executables, oversized arguments, environment variables outside the allowlist and output beyond the configured limit. Logs contain only policy-safe metadata and never environment values or command arguments marked sensitive.
