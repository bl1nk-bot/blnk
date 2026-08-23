## ก่อน merge

- [ ] Base branch คือ `main` (ห้าม merge PR ไป feature/release branch)
- [ ] PR body มี `Closes #<issue>` สำหรับ issue ที่ส่งมอบ
- [ ] อัปเดต canonical docs/status ที่งานนี้กระทบ หรือเขียนเหตุผลในหัวข้อ Documentation decision
- [ ] `Cargo.toml` และ `Cargo.lock` เพิ่ม patch จาก version บน `main` หนึ่งครั้งพอดี
- [ ] เพิ่มหัวข้อ `## [x.y.z] — PR #...` ใน `CHANGELOG.md`
- [ ] รัน verification ที่เกี่ยวข้องและระบุผลในหัวข้อ Evidence
- [ ] อ่าน review comments ทั้งหมดแล้ว; resolve review threads หรือบันทึกเหตุผลที่ไม่แก้

## Documentation decision

<!-- บอกไฟล์ canonical ที่อัปเดต หรือ "N/A: <เหตุผลเฉพาะ>" -->

## Evidence

<!-- คำสั่งและผลที่รันจริง; ห้ามเขียนว่า "ไม่ได้รัน" โดยไม่บอก blocker -->

PR ที่ไม่เข้าตามนี้ต้องแก้ก่อน merge เพื่อให้ทุกงานมี release/tag ที่ตรวจสอบย้อนกลับได้
