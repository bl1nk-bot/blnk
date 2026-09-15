# TODO — Bl1nk Implementation Roadmap

> Source of Truth: `specs/spec.md`, `docs/architecture.md`, `CONTEXT.md`, `specs/blnk-product-roadmap.md`

## Phase 0 — Contracts, Repository, Storage Skeleton (Weeks 1-2)

### Active
- [ ] **P0-01** Freeze ObjectKind, ObjectStatus, Sensitivity, SchemaVersionPolicy (T0/T1)
- [ ] **P0-02** Create `blnk-domain` crate with core types + ObjectHandler trait (T1)
- [ ] **P0-03** SQLite connection pool, migration runner v1, WAL mode, FTS5 (T2)
- [ ] **P0-04** VaultKeyProvider trait, AES-256-GCM vault, OS keychain (Windows DPAPI / Linux Secret Service) + passphrase fallback (T2)
- [ ] **P0-05** Tauri 2 shell + React routing + design tokens (CSS custom properties) (T5)
- [ ] **P0-06** CLI namespace: `object`, `share`, `receive`, `workspace`, `config`, `completion` (T6)
- [ ] **P0-07** CI matrix (Linux/Windows/Android), quality gates (fmt/clippy/test/audit), release gate with SHA256SUMS + PROVENANCE.json (T8)

### Exit Criteria (Phase 0)
- [ ] All teams compile against shared domain contracts
- [ ] Migration runner creates metadata DB
- [ ] Vault lock/unlock works in test fixture
- [ ] Desktop shell opens on Windows/Linux
- [ ] CLI/TUI doesn't duplicate domain types
- [ ] Issue templates for schema/API changes exist

---

## Phase 1 — First Complete Slice: MCP/Workspace Pack Local (Weeks 3-5)

### Active
- [ ] **P1-01** Claude Code adapter (detect/import/preview/apply/rollback) (T3)
- [ ] **P1-02** Codex adapter (T3)
- [ ] **P1-03** Gemini CLI adapter (T3)
- [ ] **P1-04** OpenCode adapter (T3)
- [ ] **P1-05** Normalized Provider/Profile/MCP model + mapping rules (T3)
- [ ] **P1-06** Object revisions, FTS5 search, tags, live-config backup/restore (T2)
- [ ] **P1-07** Home, Vault, Object Detail, Workspace UI (T5)
- [ ] **P1-08** CLI: `capture`, `list`, `get`, `use`, `history` (T6)
- [ ] **P1-09** Adapter contract test harness (same tests for all adapters) (T8)

### User Flow
```
First launch → detect local AI clients → import existing config as draft
→ select items → create profile/workspace pack → preview target changes
→ backup → apply → receipt/history
```

### Exit Criteria (Phase 1)
- [ ] Import existing config doesn't destroy original files
- [ ] User can search MCP/profile
- [ ] Apply to target client works for ≥4 adapters
- [ ] Preview shows diff + masked sensitive fields
- [ ] Every apply creates backup + rollback works
- [ ] `cargo test`, frontend tests, Windows/Linux smoke tests pass

---

## Phase 2 — Desktop Control Center + Workspace Content (Weeks 6-8)

### Planned
- [ ] **P2-01** Home dashboard (recent, expiring, active workspace, quick actions) (T5)
- [ ] **P2-02** Provider/profile management (create, clone, sort, activate, compare) (T5)
- [ ] **P2-03** MCP panel (template, JSON import, per-app binding, validation) (T5)
- [ ] **P2-04** Prompt service (Markdown editor, targets, backfill protection) (T3)
- [ ] **P2-05** Skill service (Git/ZIP/local install, checksum, target mapping) (T3)
- [ ] **P2-06** Workspace Pack (manifest, selective include/exclude, redaction) (T3)
- [ ] **P2-07** System tray (active profile switch, open, backup, quit) (T5)
- [ ] **P2-08** Onboarding/settings/i18n (Thai/English, theme, paths, keychain status) (T5)
- [ ] **P2-09** Desktop E2E (first launch to activation) (T8)

### Exit Criteria
- [ ] User manages provider, MCP, prompt, skill from single view
- [ ] Workspace Pack shows included/excluded items
- [ ] Tray switches profile without opening main window
- [ ] No secrets in screenshots/test artifacts/logs
- [ ] UI has loading, empty, error, locked vault, recovery states

---

## Phase 3 — Secure Sharing via QR, Clipboard, blnk P2P (Weeks 9-11)

### Planned
- [ ] **P3-01** Share envelope (versioned manifest, encrypted payload ref, signature) (T4)
- [ ] **P3-02** Access code (Argon2id verifier, attempts, lockout, TTL, one-time) (T4/T2)
- [ ] **P3-03** blnk transport adapter (map object transfer to WebRTC/SWSP) (T4)
- [ ] **P3-04** QR/clipboard channel (ANSI/PNG/SVG, auto-clear, size limits) (T4)
- [ ] **P3-05** Send wizard (select objects, recipient, expiry, included/excluded fields) (T5)
- [ ] **P3-06** Receive wizard (preview, verify, destination, diff, commit) (T5)
- [ ] **P3-07** CLI/TUI share flow (send/receive/status/revoke) (T6)
- [ ] **P3-08** Adversarial tests (replay, expiry, wrong code, corrupt payload, interruption) (T8)

### Exit Criteria
- [ ] Windows/Linux two-device send/receive Pack
- [ ] Share doesn't send user-redacted secrets
- [ ] Access code stored as verifier, not plaintext
- [ ] Expired/revoked/replayed shares rejected
- [ ] Receive doesn't write live config before explicit confirmation
- [ ] P2P interruption shows clear resume/fail state

---

## Phase 4 — Deep Link, Web Receive, Object Expansion (Weeks 12-14)

### Planned
- [ ] **P4-01** `blnk://` protocol registration (Windows/Linux) (T4/T5)
- [ ] **P4-02** Responsive receive web (mobile-friendly, handoff to desktop) (T5)
- [ ] **P4-03** Signed HTTPS link resolver (offline/online abstraction) (T4)
- [ ] **P4-04** note/bookmark/file-reference handlers (T1/T3)
- [ ] **P4-05** password/login/TOTP handlers (vault policy, masked copy) (T3)
- [ ] **P4-06** Object filters + command palette (T5)
- [ ] **P4-07** Deep-link + malicious payload tests (T8)

### Exit Criteria
- [ ] `blnk://` opens receive page (no auto-apply)
- [ ] HTTPS landing doesn't store plaintext secrets
- [ ] New objects use same search/history/share pipeline
- [ ] Malicious commands/paths/URLs/env warned or rejected

---

## Phase 5 — Device Trust, Sync, Recovery (Weeks 15-17)

### Planned
- [ ] **P5-01** Device registry/trust/revoke (T4)
- [ ] **P5-02** P2P sync change protocol (T7)
- [ ] **P5-03** Conflict engine (keep local/remote/merge/duplicate) (T7)
- [ ] **P5-04** Full/object backup + restore (T2)
- [ ] **P5-05** Key rotation + crash recovery (T2)
- [ ] **P5-06** Sync/backup UI (T5)
- [ ] **P5-07** Two-device matrix tests (T8)

### Exit Criteria
- [ ] Sync metadata + selected encrypted payloads
- [ ] Conflicts not silently overwritten
- [ ] Credential sync opt-in, separate from metadata
- [ ] Restore from backup to new Windows/Linux device
- [ ] Key rotation preserves payload hash, recovers after kill

---

## Phase 6 — TUI/CLI Parity, Usage, Health (Weeks 18-20)

### Planned
- [ ] **P6-01** TUI five-zone workflow (Home, Send, Receive, Vault, History, Devices) (T6)
- [ ] **P6-02** Terminal safety (TerminalGuard, panic restore, masking, TestBackend) (T6)
- [ ] **P6-03** Command palette + scripting (stable exit codes, JSON output) (T6)
- [ ] **P6-04** Provider health (opt-in probes, timeout, status history) (T7)
- [ ] **P6-05** Local usage events (tokens/requests/cost) (T7)
- [ ] **P6-06** Usage/health dashboard (retention, privacy) (T5)
- [ ] **P6-07** CLI/TUI regression suite (PTY, Windows console) (T8)

### Exit Criteria
- [ ] TUI works at 80×24 with non-ANSI fallback
- [ ] TUI uses same app services as desktop
- [ ] Sensitive values masked in all widgets
- [ ] Health/usage disableable with retention policy
- [ ] CLI stable for automation

---

## Phase 7 — Production Hardening Windows/Linux (Weeks 21-23)

### Planned
- [ ] **P7-01** Threat model review (T8)
- [ ] **P7-02** Vault security audit (keychain, memory zeroization, backup exposure) (T8/T2)
- [ ] **P7-03** Filesystem/permission audit (DB, vault, backup, temp, logs) (T8)
- [ ] **P7-04** Windows packaging (installer, protocol association, tray, update) (T8/T5)
- [ ] **P7-05** Linux packaging (AppImage/deb) (T8/T5)
- [ ] **P7-06** Release CI (build, integration, smoke, checksum) (T8)
- [ ] **P7-07** Documentation + support matrix (T0/T5)

### Exit Criteria
- [ ] Installer/upgrade/uninstall works on Windows/Linux
- [ ] DB migration from previous version works
- [ ] Vault lock/unlock/restore works
- [ ] No-secret logging audit passes
- [ ] P2P, QR, clipboard, deep link, adapter flows pass smoke tests
- [ ] Support matrix documents only what this release delivers

---

## Phase 8+ Backlog (Post Desktop Stability)

| Phase | Capability | Blocked By |
|-------|------------|------------|
| 8 | Browser extension/autofill | Object slot model, local bridge, security review |
| 9 | Base/View formula runtime | Schema grammar, evaluator, permission sandbox |
| 10 | Workflow/action runtime | Capability policy, audit |
| 11 | Proxy/failover | Network blast radius |
| 12 | OAuth/passkey/delegated access | Service identity, scopes |
| 13 | Public short link/webhook/API | Abuse prevention, ownership |
| 14 | macOS/mobile | Desktop usage data, platform research |

---

## Cross-Team Tickets (Open Immediately)

```
BLNK-001 Freeze ObjectKind enum + schema_version policy
BLNK-002 Define structured error codes + API error envelope
BLNK-003 Create Rust domain crate + shared TS contracts
BLNK-004 SQLite migration runner with schema_migrations
BLNK-005 WAL/FK/busy-timeout connection policy
BLNK-006 VaultKeyProvider trait + locked/unlocked states
BLNK-007 Tauri shell with Windows/Linux dev build
BLNK-008 CI matrix for Rust, frontend, Windows, Linux
BLNK-020 Objects/revisions/tags/FTS5
BLNK-021 Object CRUD + redacted preview
BLNK-022 AppAdapter trait + contract fixtures
BLNK-023..026 Four adapters (Claude, Codex, Gemini, OpenCode)
BLNK-027 Workspace.pack manifest + redaction
BLNK-028 Apply plan, live backup, rollback
BLNK-029 Home/Vault/Object Detail screens
BLNK-030 Profile/MCP/Workspace screens
BLNK-040..048 Secure sharing (envelope, access code, QR, P2P, wizards, tests)
BLNK-060..070 Multi-device, hardening, packaging
```

Every ticket must have: `owner_team`, `dependency_ids`, `contract_version`, `test_plan`, `demo_scenario`, `phase_gate`