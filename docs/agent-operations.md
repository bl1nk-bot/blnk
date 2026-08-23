# Agent operations

เอกสารนี้คือ operating memory ของ agent สำหรับ repository นี้ ไม่ใช่ working
report และต้องอ่านร่วมกับ `AGENTS.md` ก่อนเริ่มงานส่งมอบใด ๆ. เป้าหมายคือให้
process พัฒนาได้จากหลักฐานและข้อผิดพลาด แทนที่จะพึ่งการจำของ agent ใน session.

## Delivery lifecycle

1. อ่าน `TODO.md`, `docs/implementation-status.md` และ issue ที่เกี่ยวข้องก่อน
   เริ่ม; ถ้ายังไม่มี issue ให้เปิด issue ที่มี scope/acceptance criteria ก่อนทำงาน.
2. ทำงานบน branch/PR ที่ผูกกับ issue เดียวเป็นหลัก. หาก scope ใหม่หรือ remediation
   ใหญ่มาก ให้เปิด issue และ PR ใหม่ แล้ว link กลับไปหา issue/PR ต้นทาง.
3. เมื่อ implementation เสร็จ ให้ปรับ canonical docs ที่ข้อเท็จจริงเปลี่ยนจริง
   (`implementation-status`, architecture/API/spec/README ตามผลกระทบ). ห้ามใช้
   report แทน source of truth.
4. เปิด PR เข้า `main` พร้อม `Closes #<issue>`, documentation decision, และ
   verification evidence. PR gate บังคับ version/Cargo.lock/CHANGELOG patch.
5. อ่าน review ทั้งหมด. แก้ข้อที่ถูกต้องและ resolve thread; หากไม่แก้ ให้ตอบด้วย
   เหตุผลเชิงเทคนิคที่ตรวจได้. ห้ามเงียบหรือ merge ทั้งที่ thread ค้าง.
6. หลัง merge ตรวจว่า merge commit เป็น ancestor ของ `origin/main`. ระบบสร้าง tag
   และ GitHub Release จาก version ของ PR; ตรวจ target ของ tag/release อีกครั้ง.
7. ปิด issue ผ่าน PR only after acceptance criteria ผ่านและเอกสารสะท้อนสภาพจริง.

## Documents and reports

ไฟล์ context หลักเป็นสินทรัพย์ระยะยาว: `TODO.md`, `specs/spec.md`,
`docs/architecture.md`, `docs/api.md`, `docs/blueprint.md`,
`docs/implementation-status.md`, `docs/plans/**`, `AGENTS.md`, และ `.github/**`.
ห้ามลบจากชื่อหรือ location. working report ลบได้เฉพาะหลังพิสูจน์ว่า issue ปิด,
งานอยู่ใน `main`, และไม่ใช่ context ที่อ้างอิงอยู่.

## Learning loop

เมื่อเกิดความผิดพลาด process, merge, release, document cleanup, หรือ automation:

1. แก้ผลกระทบและตรวจหลักฐานก่อน.
2. เพิ่มบทเรียนที่นำไปป้องกันซ้ำได้ในหัวข้อด้านล่าง.
3. เพิ่ม deterministic check/configuration เมื่อเป็นไปได้; ใช้ agent review เฉพาะ
   การตัดสินเชิงความหมายที่ตรวจด้วย script ไม่ได้.
4. ก่อน hand-off ตรวจว่า `AGENTS.md` หรือเอกสารนี้พอให้ agent ใหม่ทำงานต่อได้
   โดยไม่ต้องอาศัย memory ของ session.

## Recorded lessons

### 2026-08-23 — remote merge reconciliation

ห้ามถือว่า branch ที่ clone ได้หรือ PR ที่ถูกปิดแปลว่าอยู่ใน `main`. ต้อง fetch
refs ที่เกี่ยวข้องและตรวจ ancestry ของ remote branch/PR head ทุกตัวกับ
`origin/main` ก่อนสรุปหรือ cleanup.

### 2026-08-23 — report cleanup is evidence-driven

ห้ามลบเอกสารตามชื่อโฟลเดอร์หรือความรู้สึกว่า “งานน่าจะเสร็จ”. แยก canonical
context ออกจาก working report ก่อน แล้วตรวจ issue state, PR implementation และ
references รายไฟล์.

### 2026-08-23 — release metadata is delivery state

version, changelog, tag และ GitHub release ต้อง map กลับได้ถึง PR/commit เดียวกัน.
ทุก PR เพิ่ม patch หนึ่งครั้งและหลัง merge ต้องตรวจ tag/release target; อย่าเก็บ
release state ไว้ในคำตอบหรือจำด้วยมือ.

### 2026-08-23 — automation is not delegation

workflow หลักต้องเป็น deterministic gate ที่ให้หลักฐานเอง. agent-task ใช้เฉพาะ
exception ที่ต้องตีความ; ไม่ใช่ทางลัดเพื่อโยน lifecycle หรือการตัดสินใจให้ agent
อีกตัว. งานตรวจซ้ำต้องมีความถี่ต่ำพอและขับเคลื่อนจาก event/หลักฐาน.
