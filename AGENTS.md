# AGENTS.md

## Project Delivery Contract

อ่าน `docs/agent-operations.md` และ skill `.github/skills/agent-operations/SKILL.md` ก่อนเริ่มงานเสมอ เพื่อเข้าใจกระบวนการทำงานและการบำรุงรักษาบริบท.

### Canonical Context

ไฟล์เหล่านี้เป็น contract ที่ agent รุ่นถัดไปต้องอ่าน — ห้ามลบ/ย้าย/เขียนทับโดยไม่มีคำสั่ง:

`AGENTS.md`, `README.md`, `TODO.md`, `CONTEXT.md`, `STYLE.md`, `specs/spec.md`, `docs/architecture.md`, `docs/decisions.md`, `docs/implementation-status.md`

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

### Behavior

- รักษาการแก้ไขให้สอดคล้องกัน — เขียนเอกสารฉบับเต็ม ไม่ใช่เพิ่มชิ้นส่วนที่ขัดแย้งกัน
- กฎของผู้ใช้ (User Rules & Memory Protocol) มีลำดับความสำคัญสูงสุดเหนือเอกสารโปรเจกต์เสมอ ห้ามนำเอกสารโปรเจกต์มาขัดแย้งหรือแทนที่กฎของผู้ใช้

### AI Context & Handoff

- เมื่อมีการเปลี่ยนแปลงสถาปัตยกรรม, CI workflow, build scripts หรือ tooling ต้องอัปเดตไฟล์สำหรับ AI ทันที (`AGENTS.md`, `CLAUDE.md`, `docs/agent-operations.md`)
- ไฟล์ภาษาไทยเขียนให้กระชับเพื่อให้ผู้ใช้ร่วมตรวจสอบได้ ส่วนไฟล์ภาษาอังกฤษ (`CLAUDE.md`) AI ต้องรับผิดชอบเนื้อหาให้ตรงกับสภาพจริงของโค้ด 100% โดยไม่ต้องรอให้ผู้ใช้มาเขียนแทน
- เมื่อผู้ใช้ถามหรือตรวจสอบงาน ให้ตอบด้วยเหตุผลทางเทคนิคอย่างสุขุม รับผิดชอบการตัดสินใจ ห้ามแพนิคหรือแก้โค้ด/ลบข้อมูลมั่วซั่วเพียงเพราะตกใจคำถาม
