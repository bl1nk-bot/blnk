# บันทึกการรวม PR และการจัดการ branch — 2026-10-20

## หลักการปฏิบัติ

- ใช้ `origin/main` เป็นฐานทุกครั้ง และตรวจ `git merge-base` ก่อนนำ patch มาใช้
- ไม่ merge branch ที่มีไฟล์ configuration เก่าติดมาโดยไม่เกี่ยวกับงาน
- เลือกเฉพาะ source และ test ที่ตรงกับ scope แล้วเขียน TDD ที่ public seam
- เปิด PR ใหม่ก่อนปิด PR เดิมที่ถูกแทนที่
- PR เดิมที่ซ้ำหรือถูกแทนที่ต้องติดป้ายและมี comment ลิงก์กลับไปยัง PR ใหม่
- เก็บ commit/PR number/branch/ผลตรวจไว้ที่เอกสารนี้เพื่อย้อนกลับได้

## สถานะเริ่มต้น

- Base: `origin/main` = `4f3bacb`
- PR ที่เปิด: 32 รายการ
- PR ที่ mergeable: 21 รายการ แต่ส่วนใหญ่เป็น draft และมีงานซ้ำ
- PR ที่ conflicting: 11 รายการ; ยังไม่นำมารวมจนกว่าจะมีการคัด patch และตรวจ review
- PR #32: merge แล้ว ไม่อยู่ในรายการที่ต้องปิด
- Issue #42: ปิดแล้วหลัง PR #56; ห้ามนำกลับมาใช้เป็น issue ปลายทาง

## กลุ่มที่พบ

| กลุ่ม | PR ที่พบ | สาระที่ต้องคัด | การดำเนินการ |
|---|---|---|---|
| AuthMessage PIN Debug | #170, #166, #161, #157, #156, #155, #151 | `src/session/mod.rs` + test redaction | เลือก implementation ที่สะอาดที่สุด 1 ชุด แล้ว supersede ที่เหลือ |
| Braintrust API key Debug | #169, #168, #164, #160, #158, #154, #153, #152 | `src/telemetry/braintrust.rs` + test redaction | เลือก implementation ที่สะอาดที่สุด 1 ชุด แล้ว supersede ที่เหลือ |
| Duplicate PIN constant-time | #167, #142, #138, #134 | `src/session/mod.rs`, `src/session/runtime.rs` + test | ตรวจ behavior และเลือก patch ที่ไม่ลบ config ปัจจุบัน |
| CLI secret Debug | #163, #150, #141 | `src/config/args.rs` + test | คัด source/test ออกจาก branch ที่มี config เก่า |
| Identity Debug | #165, #159 | `src/identity/key.rs` + test | เลือก 1 ชุด |
| UNC path | #162 | `src/utils/path.rs`, file/shell call sites + test | แยกเป็นงาน security คนละ scope |
| FTS5 query | #147, #144 | `src/storage/store.rs` + test | ต้องอ่าน review ก่อนรวม |

## การจับคู่ PR ใหม่

จะใช้คู่ที่เป็นคนละ concern เพื่อให้ PR ใหม่ตรวจสอบง่าย:

1. PR ใหม่ชุด A: AuthMessage PIN Debug + Braintrust API key Debug
2. PR ใหม่ชุด B: Duplicate PIN constant-time + CLI secret Debug
3. PR ใหม่ชุด C: Identity Debug + UNC path

ก่อนเปิด PR แต่ละชุดต้องมี test-first evidence และ `just ci` ผ่าน รายการคู่เป็นแผนการจัดกลุ่ม ไม่ใช่หลักฐานว่า patch ทุกบรรทัดใช้ได้โดยไม่คัดกรอง

## Ledger การเปลี่ยนแปลง

| เวลา | รายการ | สถานะ | หลักฐาน |
|---|---|---|---|
| 2026-10-20 | fetch PR heads เป็น local refs | เสร็จ | `refs/remotes/pr/*` |
| 2026-10-20 | สร้าง branch audit จาก `origin/main` | เสร็จ | branch `ops/consolidate-prs-2026-10-20` |
| 2026-10-20 | ติดป้าย PR เดิม | กำลังดำเนินการ | GitHub PR labels/comments |
| 2026-10-20 | เปิด PR ใหม่ชุด A/B/C | ยังไม่เริ่ม | ต้องมี test + CI ก่อน |
| 2026-10-20 | ปิด PR เดิมหลังมี PR ใหม่ | ยังไม่เริ่ม | ต้องใส่ลิงก์ PR ใหม่ใน comment ก่อน |

## วิธี rollback

- ก่อน merge: ใช้ PR เดิมและ branch `refs/remotes/pr/<number>` เป็น source reference
- หลังเปิด PR ใหม่: revert commit ของ PR ใหม่ได้โดยไม่ลบ PR เดิมที่ปิดแล้ว
- หาก patch ใดผิด ให้เปิด PR แก้จาก `origin/main` ใหม่ ไม่ force-push ทับประวัติที่ตรวจสอบแล้ว
