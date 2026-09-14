# Bl1nk Product Roadmap & Implementation Specification

**เอกสารประเภท:** Product Roadmap และ Implementation Specification  
**ผลิตภัณฑ์:** Bl1nk Universal Transfer & Secure Workspace  
**ขอบเขตแพลตฟอร์มเฟส 0–7:** Desktop บน Windows และ Linux เท่านั้น  
**สถานะ:** Ready for team breakdown  
**วันที่:** 14 กันยายน 2026  
**เอกสารนี้ไม่ใช่บทสรุปผู้บริหาร:** เอกสารนี้กำหนดงานที่ต้อง implement, dependency, owner, deliverables, acceptance criteria และ phase gate เพื่อให้หลายทีมเริ่มพัฒนาได้โดยไม่ตีความต่างกัน

---

## 1. วิธีใช้เอกสารนี้

ทีมต้องใช้เอกสารนี้เป็น source of truth สำหรับการวางแผน implementation ของ Bl1nk รุ่นแรก หาก requirement, architecture หรือ task ในเอกสารอื่นขัดแย้งกับเอกสารนี้ ให้แจ้งเป็น change request และห้ามแก้ scope เฉพาะในทีมของตนเอง

คำว่า **ต้องทำ** ในเอกสารนี้หมายถึง product requirement ที่ต้องมี implementation ส่วนคำว่า **ฐานเดิม** หมายถึงโค้ดหรือ pattern ที่นำมา reuse ได้จาก `bl1nk-bot/blnk` และ `farion1231/cc-switch` การที่ความสามารถใดยังไม่มีใน codebase ปัจจุบันไม่ใช่เหตุผลในการตัดออกจาก roadmap แต่เป็นงานใน work package ที่ระบุไว้

### 1.1 กติกาการแบ่งงาน

ทุกงานต้องระบุ:

- Product behavior ที่ผู้ใช้จะได้รับ
- Interface หรือ schema ที่ทีมอื่นต้องใช้งาน
- Dependency ที่ต้องเสร็จก่อน
- Test ที่ต้องเพิ่ม
- Artifact ที่ต้องส่งมอบ
- Owner ที่ตัดสินใจเมื่อเกิดความกำกวม
- Phase gate ที่ใช้ตัดสินว่างานพร้อมส่งต่อ

งานที่แก้เฉพาะ UI โดยไม่มี service contract หรือ test ไม่ถือว่าส่งมอบเสร็จ งานที่มี backend แต่ผู้ใช้ยังทำ flow ไม่จบก็ไม่ถือเป็น feature complete

---

## 2. Product definition

### 2.1 Product behavior หลัก

Bl1nk ต้องทำให้ผู้ใช้สามารถทำงานต่อไปนี้ได้จากระบบเดียว:

```text
Capture → Store → Retrieve → Deliver → Verify → Consume/Apply → History/Undo
```

คำอธิบายแต่ละขั้น:

| ขั้น | Product behavior |
|---|---|
| Capture | นำข้อมูลจากไฟล์, clipboard, client config, QR, link หรืออุปกรณ์เข้าระบบ |
| Store | เก็บ object metadata ใน SQLite และเก็บ payload ที่อ่อนไหวใน Encrypted Vault |
| Retrieve | ค้นหา object ด้วยชื่อ, kind, tag, workspace, source และ recent usage |
| Deliver | ส่งผ่าน local file, clipboard, QR, deep link หรือ blnk P2P |
| Verify | ตรวจ source, signature, expiry, recipient, PIN/access code และ policy |
| Consume/Apply | ใช้ object โดยตรง เช่น activate profile, install MCP, copy secret หรือ apply workspace pack |
| History/Undo | แสดง receipt, audit, revision, backup และ rollback |

### 2.2 Product positioning

Bl1nk คือ **Universal Transfer & Secure Workspace** ไม่ใช่แอปแชร์ MCP อย่างเดียว และไม่ใช่ remote shell client อย่างเดียว MCP/AI Workspace Pack เป็น first complete vertical slice เพราะเป็นจุดที่มี use case ชัดและเชื่อมความสามารถจากทั้ง blnk กับ cc-switch ได้เร็วที่สุด

### 2.3 Supported object kinds

ในระดับ schema ต้องรองรับ object registry กลางตั้งแต่ Phase 0 แต่ implementation จะเปิดใช้งานตาม phase:

| Kind | Phase ที่เริ่ม implement | Consumer หลัก |
|---|---:|---|
| `note` | 4 | editor, clipboard, file |
| `mcp.server` | 1 | Claude, Codex, Gemini, OpenCode adapters |
| `provider` | 1 | AI clients |
| `profile` | 1 | AI clients, workspace |
| `prompt` | 2 | `CLAUDE.md`, `AGENTS.md`, client files |
| `skill` | 2 | supported AI clients |
| `workspace.pack` | 1 | Bl1nk workspace activation |
| `bookmark` | 4 | browser/open action |
| `file.reference` | 4 | file manager, transfer |
| `secret.password` | 4 | clipboard, extension later |
| `secret.login` | 4 | autofill later |
| `secret.wifi` | 5 | OS network action later |
| `secret.totp` | 5 | one-time code generator |
| `qr.code` | 3 | QR display/export |
| `webhook` | 6 | connector/runtime |
| `button` | 6 | controlled action runtime |

การเพิ่ม kind ใหม่ต้อง implement `ObjectHandler`, schema version, validator, redacted preview, storage policy, import/export mapping, test fixtures และ UI action contract

---

## 3. Scope และ non-scope ตาม phase

### 3.1 Platform scope

| Phase | Platform scope | หมายเหตุ |
|---|---|---|
| Phase 0–1 | Windows 10+ และ Linux x86_64 | local development และ CI ต้องครอบคลุมทั้งสองระบบ |
| Phase 2–3 | Windows 10+ และ Linux x86_64 | desktop UI, P2P, QR, deep link |
| Phase 4–5 | Windows 10+ และ Linux x86_64 | object expansion, extension prototype local |
| Phase 6–7 | Windows 10+ และ Linux x86_64 | sync, usage, health, advanced desktop |
| Phase 8+ | ตัดสินใจ macOS/mobile หลังจาก desktop stable | ไม่อยู่ในเฟสแรกถึงเฟสกลาง |

ห้ามเพิ่ม macOS, Android, iOS หรือ native mobile เป็น acceptance target ของ Phase 0–7 งานใดที่ต้องมี abstraction เพื่อรองรับ platform อื่นให้สร้าง interface ไว้ได้ แต่ห้ามดึง implementation และ testing effort ของ platform อื่นเข้ามาขัดแผน Windows/Linux

### 3.2 Non-scope ของ Phase 0–3

- Native mobile application
- Browser extension production release
- OAuth authorization server สำหรับ external applications
- Public multi-tenant cloud vault
- Hosted short-link service ที่เป็น dependency บังคับ
- Enterprise team administration
- Full proxy/failover runtime
- General workflow execution ที่มี arbitrary side effects

สิ่งเหล่านี้เป็น roadmap work packages ในเฟสหลัง ไม่ใช่สิ่งที่ถูกยกเลิก

---

## 4. Architecture baseline

### 4.1 Runtime architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│ Desktop: Tauri 2 + React/TypeScript                            │
│ Home | Vault | Send | Receive | Devices | Workspaces | History  │
└─────────────────────────────┬───────────────────────────────────┘
                              │ typed commands/events
┌─────────────────────────────▼───────────────────────────────────┐
│ Rust Application Services                                      │
│ Object Registry | Import Handler | Policy | Adapters | Sync    │
└──────────────┬──────────────────────┬──────────────────────────┘
               │                      │
      ┌────────▼────────┐    ┌────────▼──────────┐
      │ SQLite Metadata  │    │ Encrypted Vault   │
      │ search/revision  │    │ payload/secrets   │
      └────────┬─────────┘    └────────┬──────────┘
               │                       │
      ┌────────▼───────────────────────▼──────────┐
      │ Transport and Channels                    │
      │ blnk P2P | QR | Clipboard | Deep link     │
      └───────────────────────────────────────────┘
```

### 4.2 Technology decisions

| Layer | Decision | Implementation note |
|---|---|---|
| Desktop shell | Tauri 2 | ใช้ rich UI, tray, native dialogs และ installers |
| Frontend | React + TypeScript | ใช้ typed API client, query/cache layer และ design system |
| Backend | Rust | ใช้ application service modules แยกจาก protocol modules |
| Existing transport | blnk WebRTC/signaling/session/SWSP | ทำ adapter ไม่แก้ semantics เดิมโดยไม่จำเป็น |
| Database | SQLite + WAL + migrations | metadata เป็น source of truth |
| Secret store | Encrypted Vault + OS keychain provider | Windows/Linux implementation ก่อน |
| Search | SQLite FTS5 | index เฉพาะ redacted metadata |
| Config integration | AppAdapter registry | รองรับ client-specific format |
| UI clients | Desktop, CLI, TUI, local Web | ทุก surface ใช้ service contract เดียว |
| Sync | P2P ก่อน; local folder/WebDAV/S3 ภายหลัง | selective sync และ conflict required |

### 4.3 Repository target structure

```text
blnk-control-center/
├── apps/
│   ├── desktop/                 # Tauri + React/TypeScript
│   └── cli/                     # existing blnk CLI + object commands
├── crates/
│   ├── domain/                  # object contracts and policies
│   ├── storage/                 # SQLite metadata and migrations
│   ├── vault/                   # encrypted payloads and key providers
│   ├── app-core/                # use cases and application services
│   ├── adapters/                # client adapters
│   ├── sharing/                 # envelope and share lifecycle
│   ├── channels/                # QR, clipboard, deep link, local file
│   ├── sync/                    # P2P and connector sync
│   ├── workspace/               # prompt, skill, pack logic
│   ├── transport-blnk/          # WebRTC integration facade
│   └── release/                 # platform packaging helpers
├── packages/
│   ├── contracts/               # generated/handwritten IPC types
│   ├── ui/                      # design system
│   └── i18n/                    # Thai/English initial resources
└── migrations/
```

### 4.4 Service boundary

UI และ CLI ห้ามเรียก SQL, filesystem adapter หรือ WebRTC โดยตรง ต้องเรียก application service:

```text
Desktop / CLI / TUI / Web
            ↓
Typed Application Service API
            ↓
Domain + Storage + Vault + Adapter + Channel
```

สัญญา API ต้องมี structured error เช่น `object_not_found`, `vault_locked`, `payload_corrupt`, `adapter_unsupported`, `share_expired`, `policy_denied` และห้ามให้ frontend แยกประเภทจากข้อความ error

---

## 5. Team topology และ ownership

### 5.1 ทีมหลัก

| Team | Owner scope | ไม่ควรแก้โดยตรง |
|---|---|---|
| T0 Product/UX | requirement, user flows, information architecture, acceptance wording | crypto primitive, SQL migration โดยลำพัง |
| T1 Domain & Contracts | object schema, manifest, errors, traits, versioning | platform-specific UI |
| T2 Storage & Vault | SQLite, FTS, revisions, backup, encryption, keychain | app adapter behavior |
| T3 App Adapters & Workspace | provider, MCP, prompt, skill, profile, pack projection | transport protocol semantics |
| T4 Transport & Sharing | blnk integration, channels, envelope, PIN, TTL, receipts | UI component internals |
| T5 Desktop & Web | Tauri, React, tray, responsive web, onboarding | direct storage schema decisions |
| T6 CLI & TUI | command surface, terminal UX, automation, terminal safety | duplicate business logic |
| T7 Sync & Advanced | sync providers, conflict, usage, health, proxy later | core object contract |
| T8 QA/Release/Security | test matrix, crash tests, threat review, signing, installers | product scope without change request |

### 5.2 Decision ownership

| Decision | Responsible | Consulted | Approval gate |
|---|---|---|---|
| Object schema | T1 | T2, T3, T5, T6 | Product + T1 |
| Vault/key policy | T2 | T4, T8 | Security gate |
| Adapter behavior | T3 | T1, T5 | Adapter contract test |
| Share protocol | T4 | T1, T2, T8 | Interop/security gate |
| Desktop IA/UI | T5 + T0 | all teams | UX acceptance |
| CLI/TUI contract | T6 | T1, T4 | CLI compatibility gate |
| Release support | T8 | all teams | phase release gate |

### 5.3 Team handoff rule

ทีมที่สร้าง contract ต้องส่ง schema, examples, negative cases, version policy และ test fixture พร้อมกัน ทีมที่ใช้ contract ห้ามเดา behavior จาก implementation ภายในของทีมเจ้าของ

---

## 6. Shared contracts ที่ต้องทำก่อนแบ่ง feature

### Contract C-01: Object Manifest

```yaml
version: 1
id: 0192...
kind: mcp.server
metadata:
  title: filesystem
  description: Local filesystem MCP
  tags: [research, local]
  sensitivity: sensitive
provenance:
  type: local
  source: claude-code
content_ref: vault:obj_0192_r1
view:
  icon: terminal
policy:
  requires_confirmation: true
bindings:
  - app: claude-code
    mode: enabled
```

`content_ref` ต้องเป็น opaque ref ไม่ใช่ secret และ `policy` เป็น declarative policy เท่านั้น ไม่ execute side effect เอง

### Contract C-02: Object lifecycle

```text
draft → active → shared → received → verified → applied
  │        │          │         │          │
  ├──────> archived   └──────> expired    └──────> rolled_back
  └──────> deleted/revoked
```

### Contract C-03: Storage split

```text
SQLite:
  id, kind, title, tags, sensitivity, provenance, revision,
  payload_ref, payload_hash, app bindings, history, audit

Vault:
  credential, token, secret env, private key, encrypted content payload
```

### Contract C-04: Import handler

```text
parse → normalize → validate → policy check → redacted preview
→ user confirmation → stage → commit → receipt
```

ทุก channel ต้องเรียก handler นี้ ไม่สร้าง import logic แยกสำหรับ QR, deep link, clipboard หรือ P2P

### Contract C-05: Adapter

```rust
trait AppAdapter {
    fn id(&self) -> &'static str;
    fn detect(&self) -> DetectionResult;
    fn import_live(&self, ctx: ImportContext) -> Result<ImportSnapshot>;
    fn preview_apply(&self, object: &ObjectRecord) -> Result<ApplyPlan>;
    fn apply(&self, plan: ApplyPlan) -> Result<ApplyReceipt>;
    fn rollback(&self, receipt: &ApplyReceipt) -> Result<()>;
    fn capabilities(&self) -> Capabilities;
}
```

---

## 7. Phase roadmap

## Phase 0 — Contracts, repository, storage skeleton

**เป้าหมาย:** ทำให้ทุกทีมมี contract เดียวและมี runtime foundation ที่ build/test ได้บน Windows/Linux

### Work packages

| ID | Owner | งาน | Deliverable |
|---|---|---|---|
| P0-01 | T0/T1 | freeze product object taxonomy และ lifecycle | `domain-spec.md`, examples, error taxonomy |
| P0-02 | T1 | สร้าง Rust domain crate | `ObjectKind`, `ObjectRecord`, `Sensitivity`, `Provenance`, traits |
| P0-03 | T2 | SQLite connection/migration/WAL layer | `storage` crate, migration v1, test DB |
| P0-04 | T2 | Vault interfaces และ key provider abstraction | `vault` crate interfaces; locked/unlocked states |
| P0-05 | T5 | Tauri shell + React routing + design tokens | desktop opens on Windows/Linux |
| P0-06 | T6 | CLI command namespace proposal | `blnk object`, `blnk share`, `blnk receive` help skeleton |
| P0-07 | T8 | CI matrix and quality gates | fmt, clippy, unit tests, Windows/Linux build |

### Exit criteria

- ทุกทีม compile against shared contracts
- Migration runner สร้าง metadata DB ได้
- Vault สามารถ lock/unlock ใน test fixture ได้
- Desktop shell เปิดได้บน Windows/Linux
- CLI/TUI ไม่ duplicate domain types
- มี issue template สำหรับ schema/API change

---

## Phase 1 — First complete slice: MCP/Workspace Pack local

**เป้าหมาย:** ผู้ใช้ import MCP/AI configuration จากเครื่อง, ค้นหา, preview, apply, backup และ rollback ได้ โดยยังไม่ต้องส่งข้ามเครื่อง

### Work packages

| ID | Owner | งาน | Deliverable |
|---|---|---|---|
| P1-01 | T3 | Claude Code adapter | detect/import/preview/apply/rollback fixture |
| P1-02 | T3 | Codex adapter | detect/import/preview/apply/rollback fixture |
| P1-03 | T3 | Gemini CLI adapter | detect/import/preview/apply/rollback fixture |
| P1-04 | T3 | OpenCode adapter | detect/import/preview/apply/rollback fixture |
| P1-05 | T3 | normalized provider/profile/MCP model | mapping rules and capability matrix |
| P1-06 | T2 | object revisions, FTS, tags, live-config backup | revision/restore tests |
| P1-07 | T5 | Home, Vault, Object Detail, Workspace UI | end-to-end local flow |
| P1-08 | T6 | `capture`, `list`, `get`, `use`, `history` | CLI automation flow |
| P1-09 | T8 | adapter contract test harness | same tests run for every adapter |

### User flow

```text
First launch
→ detect local AI clients
→ import existing config as draft
→ select items
→ create profile/workspace pack
→ preview target changes
→ backup
→ apply
→ receipt/history
```

### Exit criteria

- Import existing config ไม่ทำลายไฟล์ต้นฉบับ
- User ค้นหา MCP/profile ได้
- Apply ไป target client ได้อย่างน้อย 4 adapters
- Preview แสดง diff และ sensitive fields แบบ masked
- ทุก apply สร้าง backup และ rollback ได้
- `cargo test`, frontend tests และ Windows/Linux smoke tests ผ่าน

---

## Phase 2 — Desktop Control Center และ Workspace content

**เป้าหมาย:** เปลี่ยน local slice ให้เป็น desktop product ที่ใช้ซ้ำได้ทุกวัน

### Work packages

| ID | Owner | งาน | Deliverable |
|---|---|---|---|
| P2-01 | T5 | Home dashboard | recent, expiring, active workspace, quick actions |
| P2-02 | T5 | Provider/profile management | create, clone, sort, activate, compare |
| P2-03 | T5 | MCP panel | template, JSON import, per-app binding, validation |
| P2-04 | T3 | Prompt service | Markdown editor, targets, backfill protection |
| P2-05 | T3 | Skill service | Git/ZIP/local install, checksum, target mapping |
| P2-06 | T3 | Workspace Pack | manifest, selective include/exclude, redaction |
| P2-07 | T5 | System tray | active profile switch, open, backup, quit |
| P2-08 | T5 | onboarding/settings/i18n | Thai/English, theme, paths, keychain status |
| P2-09 | T8 | desktop E2E | first launch to activation scenarios |

### Exit criteria

- User สามารถจัดการ provider, MCP, prompt, skill จากหน้าเดียว
- Workspace Pack แสดงสิ่งที่จะรวมและสิ่งที่ไม่รวม
- tray switch profile ได้โดยไม่เปิดหน้าหลัก
- ไม่มี secret ใน screenshot/test artifact/log
- UI มี loading, empty, error, locked vault และ recovery states ครบ

---

## Phase 3 — Secure sharing ผ่าน QR, clipboard และ blnk P2P

**เป้าหมาย:** ส่ง Workspace Pack ระหว่าง Windows/Linux devices และรับไป apply ได้อย่างปลอดภัย

### Work packages

| ID | Owner | งาน | Deliverable |
|---|---|---|---|
| P3-01 | T4 | share envelope | versioned manifest, encrypted payload ref, signature contract |
| P3-02 | T4/T2 | share access code | Argon2id verifier, attempts, lockout, TTL, one-time |
| P3-03 | T4 | blnk transport adapter | map object transfer to existing WebRTC/SWSP |
| P3-04 | T4 | QR and clipboard channel | ANSI/PNG/SVG, auto-clear timer, size limits |
| P3-05 | T5 | Send wizard | select objects, recipient, expiry, included/excluded fields |
| P3-06 | T5 | Receive wizard | preview, verify, destination, diff, commit |
| P3-07 | T6 | CLI/TUI share flow | send/receive/status/revoke commands |
| P3-08 | T8 | adversarial tests | replay, expiry, wrong code, corrupt payload, interruption |

### User flow

```text
เลือก Pack
→ เลือก Work laptop หรือ QR/clipboard
→ เลือก “ครั้งเดียว / หมดอายุเมื่อไร”
→ ตรวจรายการและ redaction
→ ส่ง
→ ผู้รับ verify
→ preview diff
→ apply
→ sender เห็น opened/consumed receipt
```

### Exit criteria

- Windows/Linux สองเครื่องส่งและรับ Pack ได้
- Share ไม่ส่ง secret ที่ผู้ใช้ตัดออก
- access code ถูกเก็บเป็น verifier ไม่ใช่ plaintext
- expired, revoked, replayed share ใช้ไม่ได้
- receive ไม่เขียน live config ก่อน explicit confirmation
- P2P interruption resume/fail state แสดงชัด

---

## Phase 4 — Deep link, web receive และ universal object expansion

**เป้าหมาย:** เพิ่มช่องทางที่ผู้ใช้เริ่มจาก browser/link ได้ และขยาย object โดยยังใช้ lifecycle เดิม

### Work packages

| ID | Owner | งาน | Deliverable |
|---|---|---|---|
| P4-01 | T4/T5 | `blnk://` protocol registration | Windows/Linux association and secure parser |
| P4-02 | T5 | responsive receive web | mobile-friendly landing, handoff to desktop |
| P4-03 | T4 | signed HTTPS link resolver interface | offline/online resolver abstraction |
| P4-04 | T1/T3 | note/bookmark/file-reference handlers | schema, preview, apply/open actions |
| P4-05 | T3 | password/login/TOTP handlers | vault policy, masked copy, no autofill yet |
| P4-06 | T5 | object filters and command palette | kind, tag, source, expiry, workspace |
| P4-07 | T8 | deep-link and malicious payload tests | parser fuzzing and risk UI tests |

### Exit criteria

- เปิด `blnk://` แล้วเข้าหน้า receive โดยไม่ auto-apply
- HTTPS landing ไม่เก็บ plaintext secret
- object ใหม่ใช้ search/history/share pipeline เดิม
- malicious command, path, URL และ env ถูกเตือนหรือปฏิเสธตาม policy

---

## Phase 5 — Device trust, sync และ recovery

**เป้าหมาย:** ทำให้หลาย Windows/Linux devices ใช้ Bl1nk เป็น secure workspace เดียวกันได้ โดยมี conflict และ recovery ที่ควบคุมได้

### Work packages

| ID | Owner | งาน | Deliverable |
|---|---|---|---|
| P5-01 | T4 | device registry/trust/revoke | trusted device UI and CLI |
| P5-02 | T7 | P2P sync change protocol | object revision, cursor, encrypted bundle |
| P5-03 | T7 | conflict engine | keep local/remote/merge/duplicate |
| P5-04 | T2 | full/object backup and restore | encrypted backup manifest |
| P5-05 | T2 | key rotation and crash recovery | resumable rotation and staged recovery |
| P5-06 | T5 | sync/backup UI | status, conflict inbox, restore wizard |
| P5-07 | T8 | two-device matrix | offline, concurrent edits, restore, revoke |

### Exit criteria

- sync metadata และ selected encrypted payload ได้
- conflict ไม่ถูก overwrite เงียบ
- credential sync เป็น opt-in แยกจาก metadata
- restore จาก backup ใหม่ไปยัง Windows/Linux device ได้
- key rotation ไม่เปลี่ยน payload hash และ recover หลัง kill process ได้

---

## Phase 6 — TUI/CLI parity, usage และ health

**เป้าหมาย:** ให้ terminal-first users ใช้ระบบหลักได้ และให้ผู้ใช้เห็น health/usage ของ AI setup

### Work packages

| ID | Owner | งาน | Deliverable |
|---|---|---|---|
| P6-01 | T6 | TUI five-zone workflow | Home, Send, Receive, Vault, History, Devices |
| P6-02 | T6 | terminal safety | TerminalGuard, panic restore, masking, TestBackend |
| P6-03 | T6 | command palette and scripting | stable exit codes and JSON output |
| P6-04 | T7 | provider health | opt-in probes, timeout, status history |
| P6-05 | T7 | local usage events | tokens/requests/cost where source supports |
| P6-06 | T5 | usage/health dashboard | retention and privacy settings |
| P6-07 | T8 | CLI/TUI regression suite | PTY and Windows console tests |

### Exit criteria

- TUI ทำงานได้ที่ 80×24 และมี fallback ที่ไม่มี ANSI
- TUI ใช้ application service เดียวกับ desktop
- sensitive value masked ในทุก widget
- health/usage ปิดได้และมี retention policy
- CLI stable สำหรับ automation

---

## Phase 7 — Production hardening บน Windows/Linux

**เป้าหมาย:** ปิดความเสี่ยงก่อนเปิดใช้จริงในสองแพลตฟอร์มที่กำหนด

### Work packages

| ID | Owner | งาน | Deliverable |
|---|---|---|---|
| P7-01 | T8 | threat model review | abuse cases, trust boundaries, mitigations |
| P7-02 | T8/T2 | vault security audit | keychain, memory zeroization, backup exposure |
| P7-03 | T8 | filesystem/permission audit | DB, vault, backup, temp, logs |
| P7-04 | T8/T5 | Windows packaging | installer, protocol association, tray, update |
| P7-05 | T8/T5 | Linux packaging | AppImage/deb or selected distribution targets |
| P7-06 | T8 | release CI | Windows/Linux build, integration, smoke, checksum |
| P7-07 | T0/T5 | documentation and support matrix | user guide, recovery guide, known limits |

### Exit criteria

- installer/upgrade/uninstall ผ่านบน Windows/Linux
- database migration จาก previous version ผ่าน
- vault lock/unlock และ restore ผ่าน
- no-secret logging audit ผ่าน
- P2P, QR, clipboard, deep link และ adapter flows ผ่าน smoke tests
- support matrix ระบุเฉพาะสิ่งที่ release นี้ส่งมอบจริง

---

## 8. Phase 8+ backlog: ทำหลัง desktop foundation เสถียร

เฟสต่อไปนี้ไม่อยู่ใน acceptance target ของ Phase 0–7 แต่ต้องรักษา contract ให้ต่อยอดได้:

| Phase | Capability | เหตุผลที่วางภายหลัง |
|---|---|---|
| 8 | Browser extension/autofill | ต้องมี object slot model, local bridge และ security review |
| 9 | Base/View formula runtime | ต้องมี schema grammar, evaluator และ permission sandbox |
| 10 | Workflow/action runtime | มี side effect จึงต้องมี capability policy และ audit |
| 11 | Proxy/failover | มี network blast radius สูงและไม่จำเป็นต่อ core transfer |
| 12 | OAuth/passkey/delegated access | ต้องมี service identity, scopes และ enterprise use case |
| 13 | Public short link/webhook/API | ต้องมี abuse prevention, ownership และ operations |
| 14 | macOS/mobile | ต้องตัดสินจาก desktop usage และ platform research |

---

## 9. Cross-team backlog ที่ต้องเปิดเป็น tickets ทันที

### 9.1 Foundation tickets

```text
BLNK-001 Freeze ObjectKind enum and schema_version policy
BLNK-002 Define structured error codes and API error envelope
BLNK-003 Create Rust domain crate and shared TypeScript contracts
BLNK-004 Add SQLite migration runner with schema_migrations
BLNK-005 Add WAL/foreign-key/busy-timeout connection policy
BLNK-006 Define VaultKeyProvider trait and locked/unlocked states
BLNK-007 Create Tauri shell with Windows/Linux dev build
BLNK-008 Add CI matrix for Rust, frontend, Windows, Linux
```

### 9.2 Local product tickets

```text
BLNK-020 Implement objects/object_revisions/tags/FTS5
BLNK-021 Implement object CRUD and redacted preview
BLNK-022 Implement AppAdapter trait and contract fixtures
BLNK-023 Implement Claude Code adapter
BLNK-024 Implement Codex adapter
BLNK-025 Implement Gemini CLI adapter
BLNK-026 Implement OpenCode adapter
BLNK-027 Implement workspace.pack manifest and redaction
BLNK-028 Implement apply plan, live backup and rollback
BLNK-029 Build Home/Vault/Object Detail screens
BLNK-030 Build profile/MCP/workspace screens
```

### 9.3 Secure transfer tickets

```text
BLNK-040 Define share envelope v1
BLNK-041 Implement TTL/one-time/revoke state machine
BLNK-042 Implement access-code verifier and lockout
BLNK-043 Map share payload to blnk WebRTC/SWSP stream
BLNK-044 Implement QR ANSI/PNG/SVG channels
BLNK-045 Implement clipboard channel with countdown
BLNK-046 Build Send wizard
BLNK-047 Build Receive wizard and apply confirmation
BLNK-048 Add share receipt/history and adversarial tests
```

### 9.4 Multi-device and hardening tickets

```text
BLNK-060 Implement device trust registry
BLNK-061 Implement P2P sync change bundle
BLNK-062 Implement conflict table and resolver
BLNK-063 Implement encrypted full/object backup
BLNK-064 Implement resumable key rotation
BLNK-065 Implement crash recovery worker
BLNK-066 Build sync/conflict/restore UI
BLNK-067 Build TUI parity and PTY tests
BLNK-068 Build Windows installer and protocol association
BLNK-069 Build Linux package and desktop entry
BLNK-070 Run security/release gate
```

ทุก ticket ต้องมี `owner_team`, `dependency_ids`, `contract_version`, `test_plan`, `demo_scenario` และ `phase_gate` ใน issue template

---

## 10. Sprint execution model

### 10.1 Sprint length และ parallelism

ใช้ sprint 2 สัปดาห์ โดยมี planning, contract review, implementation, integration demo และ gate review ในทุก sprint งานที่ทำคู่ขนานได้:

```text
T1 Domain ────────┐
T2 Storage/Vault ─┼─> T3 Adapters ──┐
T4 Transport ────┘                   ├─> T5 Desktop/T6 CLI-TUI
T8 QA/Release ──────────────────────┘
```

ทีมไม่ควรเริ่ม implementation ที่มี dependency ต่อ contract ซึ่งยังไม่ merge ให้ใช้ fixture/mock contract เพื่อทำงานคู่ขนานแทนการเดา field

### 10.2 Sprint ceremony ที่บังคับ

| Ceremony | ผู้เข้าร่วม | ผลลัพธ์ |
|---|---|---|
| Contract review | T1, affected teams, T8 | schema/API version locked |
| Integration planning | ทุกทีม | dependency/order และ risk list |
| Mid-sprint integration | owner teams | compile/test across branches |
| Demo | T0 + all owners | user behavior evidence |
| Gate review | T0, T1, T8 | pass, rework หรือ scope change |

### 10.3 Definition of Ready

งานจะเริ่มได้เมื่อมี:

- user behavior ที่ต้องเกิด
- input/output contract
- dependency list
- security classification
- test scenario อย่างน้อยหนึ่ง happy และสอง negative cases
- owner team และ reviewer
- phase gate ที่เกี่ยวข้อง

### 10.4 Definition of Done

งานจะเสร็จเมื่อ:

- code และ migration merge แล้ว
- test ผ่านบน target platform ของ phase
- ไม่มี secret ใน log/test artifact
- error/empty/locked/recovery state ทำงาน
- API/contract documentation อัปเดต
- integration demo ผ่าน
- QA มี evidence link
- rollback หรือ recovery behavior ระบุชัด

---

## 11. Test strategy

### 11.1 Test layers

| Layer | เจ้าของหลัก | สิ่งที่ต้องพิสูจน์ |
|---|---|---|
| Unit | ทุกทีม | parser, policy, schema, crypto wrapper, state transition |
| Contract | T1/T3/T4 | same input/output across adapters/channels |
| Integration | T2/T3/T4 | SQLite + Vault + adapter + transport |
| UI component | T5/T6 | loading, locked, masked, error, keyboard |
| E2E desktop | T8/T5 | first launch → import → apply → share → restore |
| P2P two-device | T8/T4 | Windows↔Windows, Linux↔Linux, Windows↔Linux |
| Crash/recovery | T8/T2 | kill during write, rotate, sync, apply |
| Release smoke | T8 | installer, permissions, deep link, tray, update |

### 11.2 Required cross-platform matrix

| Scenario | Windows → Windows | Linux → Linux | Windows → Linux | Linux → Windows |
|---|---:|---:|---:|---:|
| Local object CRUD | yes | yes | n/a | n/a |
| QR share | yes | yes | yes | yes |
| Clipboard share | yes | yes | manual boundary | manual boundary |
| P2P Workspace Pack | yes | yes | yes | yes |
| Apply Claude/Codex/Gemini/OpenCode | adapter-dependent | adapter-dependent | receive-side | receive-side |
| Backup/restore | yes | yes | bundle transfer | bundle transfer |
| Deep link | yes | yes | link handoff | link handoff |
| Vault lock/unlock | yes | yes | n/a | n/a |

### 11.3 Security tests

- wrong access code and attempt lockout
- replay consumed share
- expired share
- revoked share while receiver is offline
- recipient mismatch
- tampered ciphertext
- AAD mismatch
- payload swapped between object IDs
- path traversal in skill/file import
- malicious command/env in MCP config
- secret in title/tag/description/log/FTS
- crash before and after transaction finalization
- backup copied without vault key

---

## 12. Release plan

### 12.1 Release channels

| Channel | เป้าหมาย | Gate |
|---|---|---|
| `dev` | daily integration | unit + typecheck |
| `nightly` | internal Windows/Linux | E2E smoke + migration |
| `beta` | selected users | P2P matrix + rollback + security review |
| `stable` | public desktop release | full Phase 7 gate |

### 12.2 Versioning

- Product version ใช้ semantic versioning
- Object manifest มี independent `schema_version`
- Storage migration version เพิ่มแบบ monotonic
- Share envelope มี version และ reject/upgrade policy
- Adapter capability มี version และ compatibility range
- Protocol compatibility ของ blnk เดิมต้องระบุแยกจาก product version

### 12.3 Migration rules

Migration ทุกตัวต้อง:

1. backup metadata ก่อน
2. ตรวจ database integrity
3. สร้าง staging หรือใช้ transaction ที่ rollback ได้
4. บันทึก schema checksum
5. มี downgrade/recovery note แม้ไม่รองรับ downgrade อัตโนมัติ
6. ทดสอบจากอย่างน้อยสอง previous versions

---

## 13. Product metrics สำหรับตัดสิน phase gate

| Metric | Target ระยะแรก |
|---|---:|
| First launch ถึง import object แรก | ≤ 2 นาที |
| Search object หลังพิมพ์คำค้น | ผลลัพธ์แรก ≤ 2 วินาที |
| Local apply ที่ผ่าน validation | ≥ 95% ใน fixture suite |
| Rollback หลัง apply failure | 100% ใน fault-injection tests |
| Share receive completion | ≥ 95% ใน lab Windows/Linux matrix |
| Secret leakage ใน log/FTS/test artifact | 0 known cases |
| Migration success | 100% บน supported previous versions |
| Crash recovery orphan records | 0 unreconciled records |

Metrics เหล่านี้ใช้เป็น product quality gate ไม่ใช่ข้อจำกัดของ product vision หาก target ยังไม่ถึง ให้แก้ implementation หรือเลื่อน release ไม่ใช่ตัด requirement หลักออก

---

## 14. Open decisions ที่ต้องปิดตามกำหนด

| Decision | Owner | ต้องปิดภายใน | ผลกระทบหากช้า |
|---|---|---:|---|
| Object Manifest JSON/YAML/CBOR | T1 | Phase 0 Sprint 1 | ทุกทีมทำ schema ต่อไม่ได้ |
| SQLite driver | T2 | Phase 0 Sprint 1 | storage implementation ชะงัก |
| AEAD implementation | T2/T8 | Phase 0 Sprint 2 | Vault และ sharing ชะงัก |
| Windows/Linux keychain backend | T2/T8 | Phase 0 Sprint 2 | vault lock/unlock ชะงัก |
| First four adapters | T0/T3 | Phase 0 Sprint 1 | MVP scope ไม่ชัด |
| Desktop shell integration strategy | T5/T1 | Phase 0 Sprint 1 | frontend/backend boundary ไม่ชัด |
| P2P signaling deployment | T4 | Phase 1 Sprint 2 | cross-device test ชะงัก |
| Share code UX | T0/T4 | Phase 1 Sprint 2 | send/receive UI ชะงัก |
| Credential sync policy | T2/T4/T7 | Phase 3 | sync security scope ไม่ชัด |
| Extension bridge | T5/T6 | Phase 5 | extension phase ไม่เริ่มจน contract พร้อม |

Open decision ไม่ได้หมายความว่า requirement ถูกยกเลิก แต่หมายความว่าทีมต้องทำ spike, threat review หรือ prototype เพื่อปิดวิธี implement

---

## 15. Final implementation sequence

```text
Phase 0  Contracts + Storage/Vault skeleton + Tauri shell
    ↓
Phase 1  Local MCP/Workspace Pack complete slice
    ↓
Phase 2  Desktop Control Center + profiles/MCP/prompts/skills
    ↓
Phase 3  Secure QR/clipboard/P2P sharing
    ↓
Phase 4  Deep link + responsive receive web + object expansion
    ↓
Phase 5  Devices + sync + backup/restore + conflict
    ↓
Phase 6  TUI/CLI parity + usage + health
    ↓
Phase 7  Windows/Linux production hardening and stable release
    ↓
Phase 8+ Extension, Base/Workflow runtime, proxy, OAuth, mobile
```

**คำสั่งเริ่มงานของทีม:** เริ่มจาก Phase 0 พร้อมกัน แต่ให้ T1 ปิด C-01 ถึง C-05 ก่อน ทีมอื่นใช้ mock fixtures ที่ versioned แล้วเริ่ม parallel implementation ได้ทันที เมื่อ Phase 1 จบ ต้องมี local product ที่ใช้ได้จริงก่อนเปิด scope ไป secure sharing ใน Phase 3

## References

[1]: https://github.com/bl1nk-bot/blnk "blnk repository and existing Rust P2P/WebRTC/CLI foundation"
[2]: https://github.com/bl1nk-bot/blnk/blob/main/docs/architecture.md "blnk architecture documentation"
[3]: https://github.com/bl1nk-bot/blnk/blob/main/CONTEXT.md "blnk domain glossary and runtime model"
[4]: https://github.com/bl1nk-bot/blnk/blob/main/PROTOCOL.md "blnk wire protocol specification"
[5]: https://github.com/farion1231/cc-switch "cc-switch product features and Tauri architecture"
[6]: https://github.com/farion1231/cc-switch/blob/main/src-tauri/src/database/schema.rs "cc-switch SQLite schema and migration patterns"
[7]: https://github.com/farion1231/cc-switch/tree/main/src-tauri/src/mcp "cc-switch MCP service and client-specific integration patterns"
[8]: https://github.com/farion1231/cc-switch/tree/main/src-tauri/src/deeplink "cc-switch deep-link parsing and import patterns"
