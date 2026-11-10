---
name: tui-feature-inventory
description: รายการฟีเจอร์และสถานะความพร้อมของ blnk-tui ก่อนการปรับปรุงและเชื่อมต่อกับ NSH
created: 2026-11-10
author: bl1nk-bot
---

# blnk-tui Feature Inventory & Readiness

บันทึกประวัติและรายการฟีเจอร์เดิมทั้งหมดใน `apps/blnk-tui` ก่อนการเลาะและต่อเติมด้วย NSH (`nsh_core::mobile`, `nsh_base`, `nsh/desktop`)

---

## 1. ฟีเจอร์ที่สมบูรณ์ ใช้งานได้จริง 100% (ห้ามลบ/ห้ามตัดทิ้ง)

| ไฟล์ / โมดูล | หน้าที่และความสามารถ | สถานะการทำงาน |
|---|---|---|
| `tui.rs` | จัดการ Lifecycle ของ Terminal: Raw mode, Alternate screen, Bracketed paste, Panic hook คืนค่าหน้าจออัตโนมัติ | ✅ ทำงานได้จริง 100% |
| `wrapping.rs` | Adaptive Word Wrap อัจฉริยะ ไม่ตัดกลาง URL, รองรับ CJK / อักขระเอเชีย, คำนวณ offset สำหรับ Hyperlink | ✅ ทำงานได้จริง 100% (มี Unit tests) |
| `width.rs` | คำนวณ Display width ของตัวอักษร, Emoji, อักขระพิเศษ Halfwidth sound marks | ✅ ทำงานได้จริง 100% (มี Unit tests) |
| `color.rs` | ระบบสี RGB: คำนวณความสว่างพื้นหลัง (`is_light`), ผสมสีโปร่งแสง (`blend`), วัดระยะความต่างของสี CIE76 (`perceptual_distance`) | ✅ ทำงานได้จริง 100% (มี Unit tests 5 ตัว) |
| `shimmer.rs` | แอนิเมชันแสงสะท้อน (Shimmer) กวาดผ่านข้อความตามเวลา process time สำหรับหน้าจอโหลด | ✅ ทำงานได้จริง 100% |
| `terminal_palette.rs` | ตรวจจับสีธีม Terminal อัตโนมัติ (Dark/Light) จาก `COLORFGBG` และแมปสี ANSI | ✅ ทำงานได้จริง 100% (มี Unit tests 3 ตัว) |
| `terminal_hyperlinks.rs` | ฝัง OSC 8 Clickable Hyperlink ลงในข้อความ Ratatui และคำนวณตำแหน่งลิงก์ใหม่หลังตัดบรรทัด | ✅ ทำงานได้จริง 100% |
| `render/line_utils.rs` | ยูทิลิตี้แปลงข้อความเป็น static lifetime และ push line buffers | ✅ ทำงานได้จริง 100% |
| `render/markdown.rs` | Single-pass streaming markdown parser (แยก block, code block, equations) | ✅ ทำงานได้จริง 100% |
| `render/records.rs` | เรนเดอร์ Markdown Table และ Key-Value แบบแนวตั้ง (Vertical Layout) สำหรับจอแคบ | ✅ ทำงานได้จริง 100% |

---

## 2. ตรรกะสมบูรณ์ แต่รอต่อท่อกับ OS / Core

| ไฟล์ / โมดูล | หน้าที่และความสามารถ | สิ่งที่ต้องทำต่อ |
|---|---|---|
| `notifications.rs` | ตรวจสอบเงื่อนไขแจ้งเตือน (`should_emit`: แจ้งเมื่อหน้าต่างไม่ได้โฟกัส) และ Interface | รอต่อ Native Notification Driver (OSC9, PowerShell, หรือ NSH mobile notification) |
| `keyboard_modes.rs` | ตรวจจับสภาพแวดล้อม Terminal (`running_in_vscode_terminal`) และโครงเปิด enhancement | รอต่อโปรโตคอล Kitty / modifyOtherKeys |
| `workspace_messages.rs` | ดึงและกรอง Headline แบนเนอร์หัวข้อจาก Workspace Protocol (มี Unit tests 4 ตัว) | รอเปลี่ยนจาก mock proto ไปใช้ proto จริงจาก `blnk` core |

---

## 3. โครงร่าง / Placeholder (รอแทนที่ด้วยของใหม่)

| ไฟล์ / โมดูล | หน้าที่และความสามารถ | สิ่งที่ต้องทำต่อ |
|---|---|---|
| `proto.rs` | Type stubs จำลองของ `workspace.proto` | เปลี่ยนไปใช้ proto codegen จาก core `blnk` |
| `windows_console.rs` | คำนวณ Bitmask สำหรับโหมด Virtual Terminal Input บน Windows (`ENABLE_VIRTUAL_TERMINAL_INPUT`) | เชื่อมต่อกับ Crossterm / Win32 API จริง |
| `pets.rs` | Type `PetImageRenderError` และ Error mapping สำหรับ ambient background image | รอต่อ Image rendering backend (Sixel / Kitty graphics) |

