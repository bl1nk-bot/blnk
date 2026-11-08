# Agent-Usable CLI and Proxy Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ทำให้ `blnk` ใช้งานจาก agent แบบ non-interactive ได้จริง โดยมี exit code และ stdout/stderr ที่เชื่อถือได้, output แบบ `--json`, pairing แบบ headless ด้วย `-nocode`/`-pin`, และเอกสารภาษาไทยที่ตรงกับ implementation หลังตรวจสอบว่า PR #32 และ Issue #42 อยู่ในสถานะใดแล้ว

**Architecture:** รักษา boundary ปัจจุบันของ CLI (`src/main.rs` + `src/config/args.rs`) และใช้ type ที่ serialize ได้เป็น contract เดียวสำหรับ human กับ machine output โดยให้ stdout มีเฉพาะผลลัพธ์ที่เป็น protocol เมื่อเลือก `--json` และให้ diagnostic ทั้งหมดไป stderr. การ pairing headless จะเพิ่มเฉพาะการรับ credential/โหมดไม่แสดง pairing code ที่ CLI/config boundary แล้วส่งค่าเข้า orchestration เดิม; ไม่สร้าง protocol ใหม่หรือ bypass การตรวจ PIN. งาน proxy จะถือว่า implementation ของ PR #56 มีอยู่แล้ว และแก้เฉพาะจุดเชื่อมต่อ/เอกสารที่พิสูจน์ได้จาก source และ tests.

**Tech Stack:** Rust 2024, Clap derive, Serde/serde_json, Tokio, existing WebRTC/SWSP/session runtime, Cargo tests, `just ci`, GitHub PR/Issue workflow.

**Spec:** ข้อกำหนดจากคำขอนี้: `connect <target> -- cmd` ต้อง non-interactive, `--json` สำหรับ `devices`, session status และผล `cp`, headless pairing ด้วย `-nocode` และ `-pin`, ปิด/ตรวจ #42 และทำ README ให้ proxy ใช้งานจริง รวมถึงเขียน comment/doc ภาษาไทยเมื่อเป็นเอกสารหรือคำอธิบายใหม่

## Global Constraints

- ทำงานบน `main` ที่ตรวจแล้วว่า local `HEAD` ตรงกับ `origin/main` (`4f3bacb`) และเริ่ม feature ใหม่ด้วย branch/PR ที่ target `main`.
- PR #32 ไม่ได้เปิดค้าง: ตรวจพบว่า merge แล้ว (`d2cdd49`) และเปลี่ยน `docs/implementation-status.md`; ไม่ต้องปิดซ้ำ ให้ใช้เป็น baseline ของเอกสาร.
- Issue #42 ไม่ได้เปิดค้าง: ตรวจพบว่าปิดแล้วหลัง PR #56 merge; PR #56 เพิ่ม concrete TCP/WebSocket/HTTP proxy handlers แต่เอกสารปัจจุบันยังบอกว่า dispatch อยู่ใน Issue #42.
- ห้ามอ้าง local fixture เป็นหลักฐาน production interoperability; ต้องแยก `fixture-only`, `runner-tested`, และ `external interoperability` ให้ชัด.
- ห้ามพิมพ์ PIN, pairing/access code, private key, bearer token, raw registry หรือ payload ลับใน stdout, stderr, JSON, tracing หรือ error message.
- การตรวจ PIN ยังคง constant-time และ proxy ยังคง deny-by-default/policy boundary เดิม.
- ความสำเร็จของ remote shell ต้องคืน exit code ของคำสั่งปลายทาง; ความล้มเหลวของการเชื่อมต่อ/validation ต้องคืน non-zero ที่ deterministic.
- comment/code documentation และเอกสารใหม่หรือข้อความที่แก้ ให้ใช้ภาษาไทยเป็นหลัก โดยชื่อ API, command, field และ error code คงภาษาอังกฤษตาม public contract.

## Review Focus

- `connect <target> -- cmd` ที่คำสั่งปลายทางคืน non-zero: ต้องส่งต่อ exit code โดยไม่กลืนเป็น success — ทดสอบใน connect/fixture integration test.
- command ที่มี argument เป็น `;`, whitespace, quote หรือ empty string: ต้องส่งเป็น argv ตรง ๆ ไม่ผ่าน shell interpolation — ทดสอบ parser/orchestration.
- `--json` ที่มีข้อความ progress/error: stdout ต้องเป็น JSON ตาม schema เดียวและ diagnostic ต้องอยู่ stderr — ทดสอบ process-level CLI.
- pairing แบบ `-nocode` โดยไม่มี `-pin`, PIN ผิด, และ PIN จาก environment/config: ต้อง fail ด้วยข้อความทั่วไปและไม่รั่ว credential — ทดสอบ args/config/pairing.
- registry ว่าง, mDNS ไม่พบ peer, session timeout และ `cp` ล้มเหลวบางส่วน: ต้องได้ JSON ที่ parse ได้เมื่อขอ `--json`, status/error code คงที่ และ exit code non-zero — ทดสอบ devices/cp/session status.

---

## ขอบเขตและหลักฐานที่ตรวจแล้ว

- CLI อยู่ใน `src/main.rs` และ `src/config/args.rs`; ปัจจุบันมี `serve`, `connect`, `cp`, `devices`, `web`, `version` แต่ยังพิมพ์ human text โดยตรงและไม่มี global/output mode สำหรับ JSON.
- `run_connect` ส่ง `run_shell_client_collected` และพยายามคืนผลล้มเหลวด้วย `anyhow`; ต้องปรับระดับ process entrypoint ให้ preserve remote exit status อย่างชัดเจน.
- `run_devices` มี table output ทั้ง registry และ mDNS แต่ยังไม่มี JSON.
- `run_cp` มี local fixture/remote flow แต่ยังไม่มี structured result และยังต้องกำหนดว่า JSON สำเร็จ/ล้มเหลวหน้าตาเป็นอย่างไร.
- `docs/api.md` และ `README.md` บอก CLI surface เดิม; `README.md` บอกว่า proxy dispatch ยังอยู่ใน Issue #42 ซึ่งขัดกับ `src/stream/proxy_handler.rs` และสถานะ Issue #42/PR #56.
- งานนี้ไม่ควรเริ่มจากการสร้าง proxy protocol ใหม่: ให้ตรวจและปิด documentation gap ก่อน แล้วแยกงาน dispatch เข้า authenticated session เป็นงาน implementation ที่มี acceptance criteria ของตัวเอง หาก source ปัจจุบันยังไม่มีหลักฐานเชื่อม dispatch.

---

### Task 1: Reconcile PR #32, Issue #42, and canonical documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/implementation-status.md`
- Modify: `docs/api.md`
- Modify: `TODO.md` (เฉพาะรายการที่พิสูจน์ว่าปิดแล้ว/ต้องสร้างงานใหม่)
- Modify: `CHANGELOG.md` (ใน PR implementation แรกที่เปลี่ยน behavior; ไม่ทำ version bump ซ้ำใน documentation-only commit)
- Test: `scripts/` or documentation verification command, plus `just ci`

**Interfaces:**
- Consumes: evidence from merged PR #32, merged PR #56, closed Issue #42, and current source paths.
- Produces: canonical docs that say proxy handlers exist in `src/stream/proxy_handler.rs`, identify whether authenticated `SessionRuntime` dispatch is still missing, and distinguish that gap from the already-closed Issue #42.

- [ ] **Step 1: Write a failing documentation consistency check** that searches canonical docs for the obsolete claim that proxy dispatch remains in Issue #42 and asserts the current source/evidence references are present.
- [ ] **Step 2: Run the check** and record the expected failure against the current README/API wording.
- [ ] **Step 3: Update the Thai documentation** so PR #32 is treated as merged baseline, Issue #42/PR #56 are treated as closed/merged evidence, and any remaining runtime dispatch is named as a new scoped task rather than attributed to #42.
- [ ] **Step 4: Run the check and `just ci`**; expected result is no obsolete Issue #42 claim and all tests pass.
- [ ] **Step 5: Commit** with a Thai documentation message, e.g. `docs: จัดสถานะ proxy และเอกสารให้ตรงกับ main`.

### Task 2: Define the CLI output and error contract

**Files:**
- Modify: `src/config/args.rs`
- Modify: `src/main.rs`
- Create or modify: focused module near `src/` for serializable CLI result/error types, following existing module conventions
- Test: `src/config/args.rs` tests and process-level CLI tests under `tests/`

**Interfaces:**
- Consumes: existing `Commands`, `ConnectArgs`, `CpArgs`, `DevicesArgs`, `SessionRuntimeSnapshot`, `DeviceRecord`, and current orchestration results.
- Produces: a single output mode (`Human`/`Json` or equivalent) and serializable result envelopes with stable fields such as `ok`, `command`, `status`, `exit_code`, `error_code`, and command-specific `data`; exact field names must be fixed in tests before implementation.

- [ ] **Step 1: Write failing parser tests** for a global `--json` option and command placement accepted by the documented examples (`blnk --json devices`, `blnk devices --json`, and the chosen supported form); reject ambiguous duplicate forms or document their precedence.
- [ ] **Step 2: Write failing process tests** asserting JSON stdout has exactly one parseable document, human progress is absent from stdout in JSON mode, and errors are JSON on stdout with diagnostics on stderr only.
- [ ] **Step 3: Implement the output writer** with typed `serde::Serialize` structures and a single `emit_result`/`emit_error` boundary; redact secrets before serialization and keep human output backward-compatible where practical.
- [ ] **Step 4: Run parser and process tests**; expected initial failures become PASS without changing transport/session semantics.
- [ ] **Step 5: Commit** with `feat: เพิ่ม contract output สำหรับ agent`.

### Task 3: Make `connect` non-interactive and preserve remote exit status

**Files:**
- Modify: `src/config/args.rs`
- Modify: `src/main.rs`
- Modify: `src/signaling/orchestration.rs` only if the existing collector/API prevents exact exit-code propagation
- Test: `tests/cli_connect.rs` or the repository’s existing CLI integration-test location

**Interfaces:**
- Consumes: `ConnectArgs`, `connect_target`, `run_shell_client_collected`, `ShellCommand`, and output contract from Task 2.
- Produces: `blnk connect --target <id> --command <PROGRAM> [ARGS...]` (or the explicitly documented positional `-- cmd` form) that never prompts, passes argv without shell interpolation, emits a structured completion result, and exits with the remote command’s code on successful session teardown.

- [ ] **Step 1: Write failing tests** for successful command, non-zero remote command, unknown target, missing PIN, and argument containing shell metacharacters; assert exact exit status and absence of secret material.
- [ ] **Step 2: Run the tests** to confirm the current `anyhow`/Clap behavior does not yet satisfy the contract.
- [ ] **Step 3: Implement non-interactive command parsing and result mapping**; use direct argv construction and map transport/config/session errors to stable non-zero CLI error codes without converting them into a fake remote exit code.
- [ ] **Step 4: Implement JSON completion/error emission** using Task 2 and ensure tracing/progress goes to stderr when JSON mode is active.
- [ ] **Step 5: Run focused tests and `just ci`**; expected result is PASS and `connect` preserves remote exit status.
- [ ] **Step 6: Commit** with `feat: รองรับ connect แบบ non-interactive`.

### Task 4: Add machine-readable `devices` and session-status output

**Files:**
- Modify: `src/config/args.rs`
- Modify: `src/main.rs`
- Modify: `src/discovery/mod.rs` only if discovery records need a stable serializable view
- Modify: `src/web/mod.rs` only if the CLI session-status contract intentionally reuses its state vocabulary
- Test: `tests/cli_devices.rs` and focused discovery/status tests

**Interfaces:**
- Consumes: `DeviceRegistry`, `discover_local_peers`, and the existing session state vocabulary (`Connecting`, `Authenticating`, `Ready`, `Closed`).
- Produces: `devices --json` with an array of metadata-only device/peer records and a session-status JSON schema with no credential fields; define whether status is a snapshot command or an option on `connect/serve` before coding.

- [ ] **Step 1: Write failing JSON schema tests** for populated/empty registry, populated/empty mDNS discovery, and each session state; assert stable field types and no `pin`, `pairing_code`, `access_code`, private key, or raw registry contents.
- [ ] **Step 2: Run focused tests** and capture the current table-only behavior as the expected failure.
- [ ] **Step 3: Implement serializable DTOs and JSON output** while keeping human table output unchanged; add a documented `session status`/`--status` surface only after selecting one unambiguous CLI spelling.
- [ ] **Step 4: Run focused tests, documentation examples, and `just ci`**.
- [ ] **Step 5: Commit** with `feat: เพิ่ม JSON output สำหรับ devices และ session`.

### Task 5: Add machine-readable `cp` results and deterministic failure handling

**Files:**
- Modify: `src/config/args.rs`
- Modify: `src/main.rs`
- Modify: `src/signaling/orchestration.rs` only if transfer statistics/status are unavailable at the current boundary
- Test: `tests/cli_cp.rs` and existing file-transfer tests

**Interfaces:**
- Consumes: `run_file_client_collected`, `FileTransferService` result, `CpArgs`, output contract from Task 2, and existing `--overwrite` semantics.
- Produces: `cp --json` result with operation, direction, source/destination labels, bytes/files if available, session status, and stable error code; never include absolute local paths or secret-bearing values unless explicitly requested and safe.

- [ ] **Step 1: Write failing tests** for upload success, `remote:<path>` download, overwrite rejection, missing source, session failure, and JSON output on both success and failure.
- [ ] **Step 2: Run focused tests** to verify the current command returns only human text/errors and lacks structured transfer status.
- [ ] **Step 3: Implement the typed copy result and deterministic cleanup** so the runtime closes before emission and cleanup errors are represented without hiding the primary copy error.
- [ ] **Step 4: Run focused tests and `just ci`**.
- [ ] **Step 5: Commit** with `feat: เพิ่ม JSON ผลลัพธ์สำหรับ cp`.

### Task 6: Implement headless pairing with `-nocode` and `-pin`

**Files:**
- Modify: `src/config/args.rs`
- Modify: `src/main.rs`
- Modify: `src/config/mod.rs` if a clearly named non-secret configuration source is needed
- Modify: `src/identity/key.rs` only if current pairing-code exposure cannot be suppressed at the output boundary
- Test: `tests/cli_pairing.rs` and existing pairing/session tests

**Interfaces:**
- Consumes: existing identity pairing code, `SessionRuntimeConfig`, remote/local orchestration, and config precedence (`flag > environment/config > default` where applicable).
- Produces: an explicit `--nocode` (also accept the exact requested spelling `-nocode` if Clap can support it without ambiguity) mode that never displays/requests an interactive code, plus `--pin <PIN>` for unattended pairing; document the source and precedence of PIN and the fact that `-nocode` suppresses display, not authentication.

- [ ] **Step 1: Write failing tests** for headless serve/connect with `--nocode --pin`, PIN from `BLNK_PIN`, missing PIN, wrong PIN, and accidental credential output in stdout/stderr/log capture.
- [ ] **Step 2: Run focused tests** to confirm current behavior requires explicit flags in some paths and does not define `-nocode`.
- [ ] **Step 3: Implement the flag/config plumbing** without weakening PIN verification, without logging pairing/access code, and without introducing an interactive prompt fallback.
- [ ] **Step 4: Run focused pairing/session tests and `just ci`**; expected result is deterministic headless success/failure.
- [ ] **Step 5: Commit** with `feat: รองรับ pairing แบบ headless สำหรับ agent`.

### Task 7: Decide and, if required, implement authenticated proxy dispatch as a new issue/PR

**Files:**
- Modify: `src/session/runtime.rs` and/or `src/signaling/orchestration.rs` only after the dispatch boundary is mapped
- Modify: `src/stream/proxy_handler.rs` only for adapter methods required by the existing runtime contract
- Modify: `proto/*.proto` only if an existing stream/control schema cannot represent dispatch; preserve field numbers
- Test: new authenticated two-peer proxy integration tests under `tests/`
- Documentation: `docs/api.md`, `docs/architecture.md`, `README.md`, `docs/implementation-status.md`

**Interfaces:**
- Consumes: `ProxyStreamService::{open_tcp, open_websocket, request_http}`, existing `StreamRegistry`, `SessionRuntime`, policy authorization, and SWSP/protobuf stream messages.
- Produces: a separate issue/PR with an explicit authenticated control-to-proxy dispatch contract; it must not reopen or relabel Issue #42.

- [ ] **Step 1: Write a design/acceptance test first** showing an authenticated session opens a policy-authorized TCP/WS/HTTP stream, rejects unauthorized targets, and cleans up the stream without taking down unrelated session streams.
- [ ] **Step 2: Run it** and record the current failure proving whether dispatch is absent or only documentation is stale.
- [ ] **Step 3: Implement the smallest adapter** that routes existing wire messages through `ProxyPolicy` and `ProxyStreamService`; do not add browser/unauthenticated shortcuts.
- [ ] **Step 4: Add negative tests** for missing authorization, SSRF/private targets, malformed frames, size/concurrency limits, redirect policy, and remote-first close.
- [ ] **Step 5: Update Thai canonical docs and changelog** with evidence and explicit limits (`wss`/external interoperability as applicable).
- [ ] **Step 6: Run `just ci` plus targeted integration tests** and open a new PR linked to the new issue.

### Task 8: Final documentation, review, and delivery gate

**Files:**
- Modify: `README.md`
- Modify: `docs/api.md`
- Modify: `docs/architecture.md`
- Modify: `docs/implementation-status.md`
- Modify: `TODO.md`
- Modify: `CHANGELOG.md`
- Release metadata: for each behavior-changing implementation PR, run `just bump <pr_number> "<message>"`; confirm the next semantic version, synchronized `Cargo.lock`, and a `CHANGELOG.md` entry under `## [0.2.x]`. Do not repeat the version bump for a documentation-only commit.
- Test: documentation examples, `just ci`, and release/PR checks

**Interfaces:**
- Consumes: shipped CLI schemas, command examples, pairing precedence, proxy evidence, and all task-level test results.
- Produces: README usage for agent automation, JSON examples, headless pairing instructions, truthful proxy status, Thai comments/documentation, and a delivery PR that closes only the newly scoped issue(s).

- [ ] **Step 1: Replace README examples** with non-interactive `connect`, `devices --json`, session status, `cp --json`, and `--nocode --pin`; mark fixture-only examples as fixture-only.
- [ ] **Step 2: Add a documentation check** that runs every example’s `--help`/parser form where practical and searches for stale Issue #42 wording.
- [ ] **Step 3: Run `just ci`, targeted CLI integration tests, and the documentation check**; save command output as PR evidence.
- [ ] **Step 4: Review all code comments and user-facing docs** for Thai language, secret redaction, stable field names, and consistency with actual source.
- [ ] **Step 5: Open PR(s) to `main`** with `Closes #<new issue>` only, include scope/evidence/limitations, read and resolve all review threads, then verify the merge commit is in `origin/main`.

## Recommended execution order

1. Task 1 — reconcile the already-merged/closed history and remove stale documentation.
2. Task 2 — freeze output/error schemas before changing command behavior.
3. Task 3 — non-interactive `connect` and exit-code correctness.
4. Task 4 — JSON `devices` and session status.
5. Task 5 — JSON `cp` and deterministic transfer errors.
6. Task 6 — headless pairing (`-nocode`/`-pin`).
7. Task 7 — only if the acceptance test proves authenticated proxy dispatch is still absent; otherwise document the existing evidence and do not duplicate Issue #42.
8. Task 8 — final README/docs synchronization and delivery gate.

The first six tasks form the agent-usable CLI milestone and can ship independently of proxy dispatch. Task 7 is deliberately conditional because the repository evidence proves concrete proxy handlers from PR #56 but does not, by itself, prove runtime dispatch through `SessionRuntime`.

## Self-review

- **Spec coverage:** non-interactive connect/exit code (Task 3), JSON devices/session/cp (Tasks 2, 4, 5), headless pairing (Task 6), Issue #42/README reconciliation (Task 1), Thai code/documentation comments (Tasks 1 and 8), and proxy usability/dispatch decision (Task 7) are all assigned.
- **Step scan:** every step has an action and a command/testable result; unresolved CLI spellings are explicitly selected before implementation rather than left as “TBD”.
- **Type consistency:** Tasks 3–6 consume the output contract produced by Task 2; Task 7 consumes the existing proxy service APIs and must define a new dispatch contract before modifying runtime code.
- **Review focus:** each listed failure mode is owned by a task with a corresponding test requirement.
- **Proportion:** the plan records decisions, file boundaries, interfaces, and verification gates without transcribing implementation bodies.
