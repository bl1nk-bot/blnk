# Bl1nk Protobuf Schema Specification

**สถานะ:** Product contract / implementation schema  
**Version:** v1  
**ขอบเขต:** Object Model, SQLite metadata, Encrypted Vault, Share, Sync, Device Trust, AppAdapter และ Workspace  
**Source files:** `proto/*.proto`  

เอกสารนี้เป็น schema contract ของ Bl1nk รุ่นใหม่ ไม่ได้จำกัดตามตารางหรือ message ที่มีอยู่ใน codebase เดิม ทุก schema ที่ระบุใน roadmap ต้องมี package, message, enum หรือ field mapping ที่รองรับไว้ตั้งแต่ v1 เพื่อให้ทีมแบ่งงานและ implement ข้าม process/device ได้โดยไม่สร้าง contract เฉพาะทีม

## 1. Compatibility policy

1. Existing packages `control`, `identity`, `pairing`, `signaling`, `stream` และ `swsp` ต้องคง field numbers และ semantics เดิมไว้
2. Product packages ใหม่ใช้ namespace `blnk.*` เพื่อป้องกันชื่อชนกับ transport protocol เดิม
3. ห้าม reuse field number ที่เคยปล่อยแล้ว
4. เพิ่ม field ใหม่ได้ แต่ห้ามเปลี่ยนความหมายของ field เดิม
5. การลบ field ต้องทำเป็น `reserved` ใน proto ภายหลัง ไม่ลบแบบเงียบ
6. Payload ที่เป็น secret ใช้ `bytes` หรือ opaque reference และไม่ใส่ใน metadata/search message โดยอัตโนมัติ
7. ทุก envelope ที่ข้าม device ต้องมี `version`, hash/integrity field และ expiry/policy ตามความเหมาะสม
8. `schema_version` ของ object, storage migration version, share envelope version และ protocol version เป็นคนละ version space

## 2. Schema inventory

| Package | Source | Responsibility |
|---|---|---|
| `blnk.common` | `common.proto` | enums, IDs, provenance, policy, paging, errors |
| `blnk.object` | `object_model.proto` | universal object, metadata, revision, preview, CRUD requests |
| `blnk.storage` | `storage.proto` | SQLite records, bindings, workspace, audit, backup |
| `blnk.vault` | `vault.proto` | encrypted payload records, key versions, rotation, recovery |
| `blnk.share` | `share.proto` | share envelope, channels, trust, receive/revoke |
| `blnk.sync` | `sync.proto` | devices, cursors, vector clocks, changes, conflict |
| `blnk.adapter` | `adapter.proto` | client detection/import/apply/rollback contract |
| `blnk.workspace` | `workspace.proto` | packs, selections, projections, prompt, skill, snapshot |
| legacy packages | existing `proto/*.proto` | wire/session/stream compatibility |
| `stream` | `stream.proto` | unified stream message envelope, per-stream-type messages |

## 3. Common contract

### 3.1 Sensitivity

`Sensitivity` มีค่า `PUBLIC`, `INTERNAL`, `SENSITIVE`, `SECRET` ตรงกับ storage policy:

- `PUBLIC`: metadata และ payload อาจส่งออกได้ตาม policy
- `INTERNAL`: ต้องมี local policy; ไม่ public โดย default
- `SENSITIVE`: ต้อง redacted preview และ explicit confirmation
- `SECRET`: payload ต้องอยู่ Vault และห้าม index/log/URL/QR plaintext

### 3.2 Object and storage state

`ObjectStatus` ระบุ lifecycle ของ object: draft, active, archived, revoked, deleted  
`StorageState` ระบุ cross-store write state: staged, ready, deleting, corrupt

`StorageState` ใช้ประสาน SQLite metadata กับ Vault:

```text
Vault staged → SQLite staged → Vault committed → SQLite ready
```

startup recovery ต้องอ่าน state ที่ค้างและ finalize หรือ cleanup ตาม hash/reference

### 3.3 Provenance และ Policy

`Provenance` ระบุแหล่งที่มา เช่น local config, client adapter, P2P device หรือ import file แต่ `source_uri` ต้องผ่าน redaction

`Policy` ระบุ declarative restrictions:

- requires confirmation
- allow export/sync/clipboard
- one-time
- TTL
- allowed device IDs
- extension attributes

Policy ห้ามเป็น executable workflow และห้ามมี arbitrary shell command ที่ runtime เรียกโดยไม่มี capability review

## 4. Universal Object Model

`blnk.object.ObjectRecord` เป็น metadata envelope กลางของทุก object kind:

```text
id
kind
schema_version
metadata { title, description, tags, sensitivity, provenance, policy, workspace }
status
payload_ref
payload_hash
payload_size
payload_media_type
current_revision
storage_state
timestamps
created_by_device_id
```

### 4.1 ObjectKind registry

```text
NOTE
MCP_SERVER
PROVIDER
PROFILE
PROMPT
SKILL
WORKSPACE_PACK
BOOKMARK
FILE_REFERENCE
SECRET_PASSWORD
SECRET_LOGIN
SECRET_WIFI
SECRET_TOTP
QR_CODE
WEBHOOK
BUTTON
```

การเพิ่ม kind ต้องเพิ่ม handler และ fixture แต่ไม่ควรสร้าง transport หรือ storage protocol ใหม่

### 4.2 Revision contract

ทุก payload update สร้าง `ObjectRevision` ใหม่แบบ immutable:

```text
same object_id + revision++ + new payload_ref + new payload_hash
```

`UpdateObjectRequest.expected_revision` ใช้ optimistic concurrency ป้องกันการเขียนทับการแก้ไขจากอีก device

### 4.3 Preview contract

`ObjectPreview` เป็น redacted projection สำหรับ UI/CLI:

- `PreviewField` ระบุค่าแสดงผลและว่า redacted/sensitive หรือไม่
- `TargetChange` แสดง target path, before/after hash และ dangerous flag
- `PolicyWarning` ระบุเหตุผลที่ต้อง confirmation หรือ deny

ห้ามส่ง `bytes payload` เข้า frontend เพียงเพื่อทำ preview

## 5. SQLite metadata contract

`blnk.storage` map กับ SQLite tables ดังนี้:

| Protobuf message | SQLite table |
|---|---|
| `SchemaMigration` | `schema_migrations` |
| `StorageMetadata` | `app_metadata` |
| `DeviceRecord` | `devices` |
| `ObjectRecord` | `objects` |
| `Workspace` | `workspaces` |
| `AppRecord` | `apps` |
| `AppBinding` | `app_bindings` |
| `Tag` | `tags` / `object_tags` |
| `ObjectSource` | `object_sources` |
| `ApplyReceipt` | `apply_receipts` |
| `BackupRecord` | `backups` |
| `AuditEvent` | `audit_events` |

FTS5 `object_search` ใช้ `title`, `description`, `tags`, `kind`, `provenance` ที่ผ่าน redaction เท่านั้น ไม่มี field สำหรับ password/token/private key

## 6. Encrypted Vault contract

`blnk.vault.VaultRecord` map กับ encrypted record ใน Vault database:

```text
payload_ref + object_id + revision
key_version + algorithm + nonce + ciphertext
AAD hash + payload hash + payload size
storage state + timestamps
```

### 6.1 Key lifecycle

`VaultKey` รองรับ key status `active`, `retiring`, `retired`, `revoked` โดยใช้ `key_ref` ไปยัง OS keychain/passphrase provider ไม่ serialize raw key ลง protobuf

`RotateKeysRequest/Response` ต้องรองรับ resumable batch และรายงาน migrated/failed records

### 6.2 Access purpose

`GetPayloadRequest.purpose` ต้องถูกตรวจที่ service boundary ให้มีค่าอย่างน้อย:

```text
Preview, Apply, Export, Backup, Autofill, Sync
```

Caller ต้องได้รับ payload เท่าที่จำเป็นและต้องไม่ log plaintext

## 7. Share contract

### 7.1 ShareRecord

เก็บ lifecycle metadata: channel, trust policy, recipient, expiry, one-time, attempts, lockout และ status

### 7.2 ShareEnvelope

เป็น transferable envelope สำหรับ QR, clipboard, deep link และ P2P:

```text
version
share_id
sender/recipient device IDs
object metadata projections
encrypted payload
signature + algorithm
payload hash
expiry + one-time + trust policy
```

Envelope ห้ามเก็บ plaintext access code และห้ามรวม secret ที่ user ตัดออกจาก selection

### 7.3 Channel semantics

| Channel | ข้อกำหนด |
|---|---|
| local file | encrypted bundle + checksum |
| clipboard | TTL/auto-clear และไม่ log content |
| QR | display envelope reference/short data; ห้ามใส่ secret plaintext |
| deep link | parser ต้องไม่ auto-apply |
| HTTPS | landing/hand-off; ไม่เก็บ secret plaintext server-side |
| P2P | ใช้ blnk session/WebRTC/SWSP adapter |
| sync | ใช้ encrypted bundle และ revision/conflict |
| API | delegated policy ต้องมี scope |

## 8. Device trust และ Sync contract

`DeviceRecord` ใช้ public identity และ trust state เท่านั้น ห้ามเก็บ private key ใน SQLite หรือ protobuf

`SyncChange` เป็น unit ของ replication:

```text
id, provider_id, object_id, revision, operation
payload_ref, payload_hash, vector, state, timestamps
```

`VectorClock` ใช้ตรวจ concurrent updates และ `SyncConflict` ต้องถูกสร้างแทน silent overwrite

Credential sync ต้องเป็น opt-in ผ่าน `SyncRequest.include_credentials`; ค่า default ใน service implementation ต้องเป็น false

## 9. AppAdapter contract

Adapter ทุกตัวต้อง implement behavior เทียบเท่า message เหล่านี้:

```text
DetectionResult
ImportContext → ImportSnapshot
ObjectRecord → ApplyPlan
ApplyRequest → ApplyResponse
RollbackRequest → RollbackResponse
```

ลำดับมาตรฐาน:

```text
Detect → Import → Normalize → Validate → Preview → Backup
→ Apply → Verify → Receipt → Rollback on failure
```

Target adapters ระยะแรก: Claude Code, Codex, Gemini CLI, OpenCode

## 10. Workspace contract

`WorkspacePack` รวม object IDs และ `ObjectSelection` ที่ระบุ include/exclude และ include_secrets แยกชัดเจน

`Projection` เป็น mapping จาก object ไป target app/path และ `ProjectionMode` ระบุ disabled, enabled หรือ merged

`PromptDocument` และ `SkillDocument` เป็น specialized projections ของ object kinds เดิม ไม่ใช่ storage core แยกใหม่

`WorkspaceSnapshot` ใช้สำหรับ preview, backup, sync และ audit โดยมี `snapshot_hash`

## 11. Schema ownership และ implementation order

| Order | Team | Schema |
|---:|---|---|
| 1 | Domain | common + object_model |
| 2 | Storage/Vault | storage + vault |
| 3 | Adapter/Workspace | adapter + workspace |
| 4 | Transport/Security | share |
| 5 | Sync | sync |
| 6 | QA/Release | compatibility fixtures และ cross-platform vectors |

Generated Rust bindings อยู่ใน `src/proto_generated.rs` และห้ามแก้ไฟล์ generated ใน `OUT_DIR` โดยตรง

## 12. Required conformance tests

1. ทุก package compile ผ่าน `prost-build`
2. ทุก message หลัก encode/decode round-trip ได้
3. enum values ไม่เปลี่ยนจาก golden descriptors
4. Share envelope ที่มี unknown fields ยัง decode ได้
5. Object metadata ไม่สามารถนำ secret field เข้า FTS contract
6. `expected_revision` ใช้ตรวจ optimistic concurrency
7. Vault record ตรวจ AAD/hash/payload reference ได้
8. Sync concurrent revision สร้าง conflict
9. Adapter fixture ทุกตัวมี detect/import/preview/apply/rollback
10. Windows/Linux สามารถใช้ generated contract เดียวกัน
11. `StreamMessage` envelope encode/decode round-trip ได้
12. `StreamMessageKind` enum values ไม่เปลี่ยนจาก golden descriptors
13. `StreamMessage` dispatch table ตรงกับ schema ใน `specs/blnk-stream-protocol.md`

## 13. Source of truth

- Protobuf wire contract: `proto/*.proto`
- Rust generated bindings: `src/proto_generated.rs`
- Product implementation roadmap: `specs/blnk-product-roadmap.md`
- SQLite implementation: `src/storage/schema.rs`
- Domain glossary: `CONTEXT.md`
- Executive/product summary: `docs/adr/0002-blnk-product-summary.md`
