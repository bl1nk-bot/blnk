# Source Trust Analysis — Codex + RustDesk → blnk

**Date**: 2026-09-16
**Author**: Hermes Agent (automated analysis)
**Scope**: Extract reusable code patterns and adaptable concepts from OpenAI Codex and RustDesk repos, map to blnk architecture/spec/bottlenecks

---

## 1. OpenAI Codex (codex-rs) — Extracted Patterns

### 1.1 Reusable Code (can adopt directly)

| Pattern | Codex Source | blnk Target | Effort |
|---------|-------------|-------------|--------|
| **Layered Config** | `ConfigLayerStack` — defaults → global → project → env → CLI | `src/config/mod.rs` | Small |
| **Feature Flags** | `Feature::enabled("name")` — runtime toggles with `Features.toml` | `src/config/args.rs` | Small |
| **Permission Profiles** | `PermissionProfile` — read-only, workspace, custom profiles | `src/signaling/orchestration.rs` (OperationScope) | Medium |
| **Hooks System** | `Hooks`, `HooksConfig` — pre/post dispatch callbacks | `src/session/runtime.rs` | Medium |
| **Network Proxy** | `NetworkProxy`, `OutboundProxyPolicy` — DNS pinning, redirect validation | `src/stream/proxy.rs` (extend ProxyPolicy) | Medium |
| **Session Resume** | `ThreadStore`, `RolloutItem` — persist session state for reconnection | `src/session/mod.rs` | Large |
| **Plugin Manager** | `PluginsManager`, `PluginLoadOutcome` — dynamic plugin loading | New module | Large |

### 1.2 Adaptable Concepts

1. **Approval Hierarchy**: Codex has `AskForApproval` → `ExecPolicyManager` → `PermissionProfile` → `SandboxEnforcement`. blnk can adapt this as `OperationScope` → `AuditLog` → `DispatchPolicy` for stream access control.

2. **Context Manager**: Codex embeds project-level instructions (AGENTS.md) into session context. blnk can embed device capabilities and connection metadata into `SessionRuntimeConfig`.

3. **Multi-Agent Roles**: Codex supports `Orchestrator`/`Worker` roles for subagent dispatch. blnk can use this pattern for future desktop UI → adapter → vault worker delegation.

4. **Token Budget**: Codex tracks `token_budget` and `rollout_budget` to prevent context overflow. blnk can track `stream_budget` (active streams per session) and `transfer_budget` (bytes per file transfer).

5. **Compaction**: Codex has `compact.rs` for history compression. blnk can compact `AuditLog` receipts after session closure (drain to `audit_events` table).

---

## 2. RustDesk — Extracted Patterns

### 2.1 Reusable Code (can adopt directly)

| Pattern | RustDesk Source | blnk Target | Effort |
|---------|----------------|-------------|--------|
| **Scope Hashing** | `login_scope: Option<[u8; 32]>` — SHA-256 digest of permitted operations | `OperationScope` version field | Small |
| **Job ID Tracking** | `cm_read_job_ids: HashSet<i32>` — filter stale file responses | `FileTransferCancellation` | Small |
| **File Transfer Lifecycle** | `fs::JobType`, `can_enable_overwrite_detection` | `src/stream/file.rs` | Medium |
| **Clipboard Separation** | Text/binary/file clipboard with platform-specific backends | `share.proto` channel mapping | Medium |
| **Connection Audit** | `ConnAuditPrimaryAuth`, `ConnAuditTwoFactor` — per-connection auth audit | `AuditReceipt` (extend) | Small |
| **Login Failure Tracking** | `LOGIN_FAILURES: [HashMap; 3]` — password/2FA/whitelist buckets with decay | `SessionConfig.max_auth_fails` (extend) | Medium |
| **Multi-Server Failover** | `rendezvous_server`, `servers: Vec<String>` — iterate fallback servers | `SignalingClient::connect_with_retry()` (already done) | Done |

### 2.2 Adaptable Concepts

1. **Login Scope Latching**: RustDesk hashes the permitted operation set at authentication time and checks it on every request. blnk's `OperationScope` already does this — the hash approach is a lighter alternative to the Vec<StreamKind> comparison.

2. **Read-Job Filtering**: RustDesk tracks `cm_read_job_ids` to filter stale file responses from cancelled jobs. blnk can add `active_request_ids: HashSet<u64>` to `FileTransferService` to reject stale responses.

3. **File Overwrite Detection**: RustDesk has `can_enable_overwrite_detection` for OS-level copy-on-write. blnk already does temp+rename, but can add checksum verification (SHA-256 of written bytes vs declared size).

4. **Permission Scope Violation Logging**: RustDesk has `scope_violation_messages: HashSet<&'static str>` to suppress repeated warnings. blnk's `AuditLog` can deduplicate denied-operation records.

5. **Adaptive Retry**: RustDesk uses exponential backoff for UDP NAT tests (`retry_interval = min(interval * 1.5, 200ms)`). blnk's `ReconnectPolicy::limited` can adopt this for signaling reconnection.

---

## 3. Mapping to blnk Architecture & Bottlenecks

### 3.1 Critical Bottlenecks (blocks spec compliance)

| Bottleneck | Spec Req | Impact | Resolution Path |
|------------|----------|--------|-----------------|
| **Proxy dispatch not wired** | 4.3, 4.4, 4.10 | 3 spec requirements blocked | Wire ProxyStreamService into ConnectionSupervisor dispatch loop |
| **No cancellation propagation** | 6.3 (Reliability) | Shell/file tasks don't cancel on session shutdown | Pass CancellationToken from ConnectionSupervisor into dispatch functions |
| **No file-transfer validation profiles** | 4.2 (File Transfer) | No size/range/timeout constraints beyond basics | Add profiles modeled after RustDesk's file-transfer validation |
| **No KeyProvider trait** | 6.2 (Security) | Vault uses direct root-key injection | Add trait abstraction for Windows/Linux keychain |

### 3.2 High-Value Quick Wins

| Task | Source | Effort | Value |
|------|--------|--------|-------|
| Layered config loader | Codex ConfigLayerStack | 1 day | Config flexibility |
| Feature flag system | Codex Features | 1 day | Gradual rollout |
| AuditLog compaction | Codex compact.rs | 0.5 day | Memory cleanup |
| Scope hash alternative | RustDesk login_scope | 0.5 day | Lighter scope check |
| File request ID tracking | RustDesk cm_read_job_ids | 1 day | Stale response rejection |

### 3.3 Medium-Term Adaptations

| Task | Source | Effort | Value |
|------|--------|--------|-------|
| Permission profiles | Codex PermissionProfile | 2 days | Fine-grained access |
| Plugin/hook boundary | Codex Hooks + PluginsManager | 3 days | Extensibility |
| Session resume | Codex ThreadStore | 3 days | Reconnection |
| Multi-agent roles | Codex multi_agents | 5 days | Subagent dispatch |
| Clipboard-as-object | RustDesk clipboard separation | 3 days | Share channel |

---

## 4. New TODO.md Tasks (added)

### RustDesk Reuse Backlog (existing section)
- All 15 items retained, 4 already done

### Codex-Inspired Adaptation Backlog (new section)
8 new tasks added:
1. Layered config loader
2. Feature-flag system
3. Permission-profile abstraction
4. Plugin/hook boundary
5. Outbound network-proxy policy
6. Session-resume capability
7. Context-manager pattern
8. Multi-agent role abstraction

---

## 5. Code Comments Added

| File | Comment Type | Location | Content |
|------|-------------|----------|---------|
| `src/stream/proxy_handler.rs` | FIXME | Module doc | Proxy dispatch not wired, blocks 3 spec reqs |
| `src/session/runtime.rs` | FIXME + TODO | `open_stream()` | Proxy dispatch bottleneck + wiring task |
| `src/stream/file.rs` | TODO | Module doc | File-transfer validation profiles needed |
| `src/signaling/orchestration.rs` | TODO | Before `dispatch_shell()` | Cancellation propagation needed |

---

## 6. Impact Assessment

**Immediate Actions (this week)**:
- Fix main.rs callers of `serve_session_with_shutdown` (pre-existing compile error)
- Wire proxy dispatch into ConnectionSupervisor (unblocks 3 spec reqs)
- Add cancellation propagation (unblocks reliability requirement)

**Short-Term (1-2 weeks)**:
- Layered config + feature flags (Codex patterns)
- File-transfer validation profiles (RustDesk patterns)
- AuditLog compaction

**Medium-Term (1 month)**:
- Permission profiles + plugin system
- Session resume + context manager
- Multi-agent roles

**Spec Coverage After Resolving Bottlenecks**: ~95% (from current ~85%)
