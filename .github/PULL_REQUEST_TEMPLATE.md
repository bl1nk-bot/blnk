## ก่อน merge

- [ ] Base branch คือ `main` (ห้าม merge PR ไป feature/release branch)
- [ ] `Cargo.toml` และ `Cargo.lock` เพิ่ม patch จาก version บน `main` หนึ่งครั้งพอดี
- [ ] เพิ่มหัวข้อ `## [x.y.z] — PR #...` ใน `CHANGELOG.md`
- [ ] CI, release metadata และ main-integrity ผ่าน

PR ที่ไม่เข้าตามนี้ต้องแก้ก่อน merge เพื่อให้ทุกงานมี release/tag ที่ตรวจสอบย้อนกลับได้
