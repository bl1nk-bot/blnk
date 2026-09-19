# AGENTS.md

## Project Delivery Contract

อ่าน `docs/agent-operations.md` ก่อนเริ่มงานส่งมอบเสมอ.

### Canonical Context

ไฟล์เหล่านี้เป็น contract ที่ agent รุ่นถัดไปต้องอ่าน — ห้ามลบ/ย้าย/เขียนทับโดยไม่มีคำสั่ง:

`AGENTS.md`, `README.md`, `TODO.md`, `CONTEXT.md`, `STYLE.md`, `specs/spec.md`, `docs/architecture.md`, `docs/api.md`, `docs/blueprint.md`, `docs/implementation-status.md`

working report คือเอกสารผลหลังงานที่ผูกกับ issue/PR เท่านั้น — ไม่ใช่ canonical context.

### Delivery Lifecycle

1. อ่าน `TODO.md` + issue ที่เกี่ยวข้อง
2. ทำงานตาม issue
3. อัปเดต canonical docs ตามหลักฐานจริง (ไม่ใช่เดา)
4. เปิด PR ที่ target `main` + `Closes #<issue>`
5. รัน verification (`just ci`)
6. อ่าน/resolve review threads
7. Merge PR → ตรวจ merge commit อยู่ใน `origin/main`

ห้าม merge เข้า feature/release branch แล้วถือว่าเสร็จ.

### Tool Usage

- ใช้ Bash (POSIX shell) สำหรับ git, cargo, just, gh commands
- ใช้ PowerShell สำหรับ Windows-specific commands เท่านั้น
- ใช้เอกสารเพื่อจัดการสถานะ — อย่าพึ่งพา memory ใน context

### Behavior

- เก็บข้อมูลสำคัญไว้ในเอกสารแทนที่จะหวังว่ามันจะอยู่รอด
- รักษาการแก้ไขให้สอดคล้องกัน — เขียนเอกสารฉบับเต็ม ไม่ใช่เพิ่มชิ้นส่วนที่ขัดแย้งกัน
- ปฏิบัติต่อเอกสารของตัวแทนเป็นสมุดบันทึก หน่วยความจำ และพื้นผิวการกำหนดค่า
