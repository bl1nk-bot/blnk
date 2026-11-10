---
name: agent-operations
description: Autonomous operations, context maintenance, decision preservation, verification rules, and delivery protocol for blnk. Use when maintaining documentation, delivering PRs, managing AI context, or verifying changes.
---

# Agent Operations & Autonomous Delivery

This skill defines the operational playbook for AI agents working in the `blnk` repository.
Agents act as an autonomous proxy for the user to maintain project health, protect context budget, preserve historical decisions, and deliver robust code.

## 1. Core Principles (Agent Proxy Mandate)

- **Act as a Proxy, Not an Order-Taker**: Do not wait for micro-instructions or force the user to repeat the same instructions across sessions. Own technical decisions with evidence and clear rationale.
- **Rule Hierarchy**: User Rules & Memory Protocol have absolute precedence over repository markdown files.
- **Calmness Under Questioning**: When the user questions or challenges a decision, explain the technical reasoning calmly. Never panic, flip-flop, or blindly delete/revert files out of distress.
- **Progressive Disclosure**: Keep main context files (`AGENTS.md`, `CLAUDE.md`) lean. Offload detailed procedures to skills and load them on demand to conserve token budget.

## 2. Context Compression & Decision Preservation

- **Reduce, Don't Blindly Delete ("ลดไม่ใช่ลบ")**:
  - Outdated plans or scaffolding notes that have already been implemented should be condensed or archived, NOT erased in a way that destroys the rationale.
  - **Decisions (ADRs) must be preserved**: Never destroy the "why" behind architectural choices (e.g., Non-trickle ICE, WebRTC stream multiplexing, loopback browser control). If detailed files are trimmed, consolidate the decisions and constraints into `docs/architecture.md` or a decision ledger.
- **Single Source of Truth**:
  - Implementation details belong in Rust source code (`src/`) and Protobuf schemas (`proto/`).
  - Do not duplicate code blocks into markdown where they will drift and become stale context.
  - Documentation must focus on architecture, module boundaries, invariants, and decisions.

## 3. Verification Protocol

- **Proportional Verification**:
  - **Documentation / Markdown only**: NEVER run `cargo test`. Cargo tests Rust binaries, not prose.
  - **Rust code changes**: Run `just ci` (which runs `fmt`, `clippy -D warnings`, `check`, and `test`).
  - **Targeted verification**: Run `cargo test <test_name>` during iterative development.
- **Platform Constraints**:
  - `scripts/release_gate.sh` is strictly Linux x86_64 with an early runtime platform guard. Never execute it on Windows or macOS.
  - On Windows, use PowerShell for Windows-specific commands and POSIX shell/bash for git and cargo.

## 4. AI Context Maintenance Protocol

Because sessions start without persistent memory, AI context files act as the project's durable brain:

- **Language Ownership**:
  - **English files (`CLAUDE.md`, `.github/aw/instructions.md`)**: The AI must autonomously maintain and synchronize these 100%. Do not expect or ask the user to maintain English context files.
  - **Thai files (`AGENTS.md`, `docs/agent-operations.md`)**: Written concisely to allow human-agent collaboration and easy user auditing.
- **When to Update**:
  - Immediately when module boundaries, workflows, CI steps, or developer tooling change.
  - When an operational mistake reveals a reusable lesson (record it in `docs/agent-operations.md` under `## Recorded lessons`).
  - When cleaning up obsolete files, ensure all cross-references in `CLAUDE.md`, `AGENTS.md`, and `README.md` are updated simultaneously.

## 5. Delivery & Release Lifecycle

1. Read `TODO.md`, `docs/implementation-status.md`, and the linked issue before starting.
2. Work on a branch linked to the issue; all delivery PRs must target `main`.
3. Update canonical docs to reflect real changes (`docs/architecture.md`, `docs/implementation-status.md`).
4. Run verification (`just ci`).
5. Bump semantic version (`just bump <pr> "msg"`) and record in `CHANGELOG.md`.
6. Open PR with `Closes #<issue>`.
7. After merge, verify the merge commit is present in `origin/main`.

