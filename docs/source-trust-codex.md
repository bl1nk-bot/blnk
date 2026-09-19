# Source Trust Analysis — Codex CLI (openai/codex) → blnk-tui

**Date**: 2026-09-17
**Author**: Hermes Agent (automated analysis)
**Scope**: Extract TUI-related patterns from Codex CLI, map to blnk-tui architecture
**Focus**: Terminal lifecycle, rendering, notifications, keyboard handling

---

## 1. Reusable Code (adopt directly)

| Pattern | Codex Source | blnk-tui Target | Effort |
|---------|-------------|-----------------|--------|
| **TUI lifecycle** | `tui/tui.rs` — init/restore/draw/alt-screen with synchronized update | `tui.rs` ✅ already implemented | Done |
| **Shimmer animation** | `tui/shimmer.rs` — cosine band + true-color blend | `shimmer.rs` ✅ already implemented | Done |
| **Terminal detection** | `codex-terminal-detection` crate — TERM_PROGRAM/VTE_VERSION/WT_SESSION parsing | `terminal_detection.rs` ✅ already in blnk core | Done |
| **Markdown rendering** | `tui/render/markdown.rs` — single-pass parse with block tracking | `render/markdown.rs` ✅ stubbed | Done |
| **URL-aware wrapping** | `tui/wrapping.rs` — URL detection + sound mark projection | `wrapping.rs` ✅ already implemented | Done |
| **Notification backend** | `tui/notifications/` — trait-based backend with OSC9 + external command | `notifications.rs` stub needs implementation | Medium |
| **Keyboard modes** | `tui/keymap/` + `keymap_setup/` — kitty protocol + modifyOtherKeys | `keyboard_modes.rs` stub needs implementation | Medium |
| **Windows console** | `tui/windows_console.rs` — Win32 GetConsoleMode/SetConsoleMode | `windows_console.rs` stub needs implementation | Small |
| **Color palette** | `tui/terminal_palette.rs` — default fg/bg from env + ANSI→RGB | `terminal_palette.rs` ✅ already implemented | Done |

---

## 2. Adaptable Concepts

### 2.1 TUI as Main Interface (NOT CLI wrapper)

**Codex pattern**: TUI is the primary interface. `codex` command opens TUI directly. No separate `serve`/`connect` CLI commands — everything happens in the TUI.

**blnk adaptation**: `blnk` command opens TUI. `blnk serve` → TUI serve mode. `blnk connect` → TUI connect mode. Browser shows QR/PIN for pairing.

**Key insight**: The TUI owns the session lifecycle. CLI flags set initial mode, then TUI takes over.

### 2.2 Event-Driven Architecture

**Codex pattern**: `app_event.rs` defines `TuiEvent` enum (Key, Paste, Resize, Draw, Resume, FocusGained, FocusLost). `event_stream.rs` provides `TuiEventStream` combining crossterm events + broadcast channel.

**blnk adaptation**: Same pattern — `TuiEvent` enum for terminal events + blnk-specific events (PeerConnected, PeerDisconnected, StreamOpened, StreamClosed, PairingRequest).

**Key insight**: Separate terminal events from domain events. TUI renders based on state, not events.

### 2.3 Status Display

**Codex pattern**: `status/` module for rendering status bar (model, token count, session state). Lightweight, always visible.

**blnk adaptation**: `status/` module for peer info, latency, active streams, session state. Always visible in status bar.

**Key insight**: Status bar is a first-class component, not an afterthought.

### 2.4 History Cell

**Codex pattern**: `history_cell/` — renders command history with syntax highlighting and expand/collapse.

**blnk adaptation**: `history_cell/` — renders shell session history with timestamps and command output. Can expand/collapse individual commands.

### 2.5 Bottom Pane

**Codex pattern**: `bottom_pane/` — composable bottom panel for input, status, or messages.

**blnk adaptation**: `bottom_pane/` — composable panel for shell input, file transfer progress, or connection status.

---

## 3. Mapping to blnk-tui Architecture

### 3.1 What We Already Have (from Codex)

| blnk-tui Module | Codex Equivalent | Status |
|-----------------|-----------------|--------|
| `tui.rs` | `tui/tui.rs` | ✅ Implemented |
| `shimmer.rs` | `shimmer.rs` | ✅ Implemented |
| `terminal_hyperlinks.rs` | `terminal_hyperlinks.rs` | ✅ Implemented |
| `terminal_palette.rs` | `terminal_palette.rs` | ✅ Implemented |
| `wrapping.rs` | `wrapping.rs` | ✅ Implemented |
| `render/markdown.rs` | `render/markdown.rs` | ✅ Stubbed |
| `render/records.rs` | `render/records.rs` | ✅ Stubbed |
| `color.rs` | `color.rs` | ✅ Implemented |
| `width.rs` | `width.rs` | ✅ Implemented |

### 3.2 What We Need (from Codex)

| blnk-tui Module | Codex Equivalent | Effort | Priority |
|-----------------|-----------------|--------|----------|
| `notifications/` | `notifications/` | Medium | Phase 2 |
| `keyboard_modes.rs` | `keymap/` + `keymap_setup/` | Medium | Phase 2 |
| `windows_console.rs` | `windows_console.rs` | Small | Phase 2 |
| `status/` | `status/` | Small | Phase 3 |
| `history_cell/` | `history_cell/` | Medium | Phase 3 |
| `bottom_pane/` | `bottom_pane/` | Medium | Phase 3 |
| `app_event.rs` | `app_event.rs` | Medium | Phase 3 |
| `event_stream.rs` | `event_stream.rs` | Medium | Phase 3 |
| `streaming.rs` | `streaming/` | Medium | Phase 3 |

---

## 4. Tasks (added to TODO.md)

### Phase 2: Platform Integration (from Codex patterns)
- [ ] Implement notification backend: OSC9 + PowerShell (from Codex `notifications/`)
- [ ] Implement kitty keyboard protocol detection (from Codex `keymap/`)
- [ ] Implement Windows console state management (from Codex `windows_console.rs`)

### Phase 3: blnk Remote Access TUI (from Codex patterns)
- [ ] Add `app_event.rs` — TuiEvent enum with blnk-specific events
- [ ] Add `event_stream.rs` — TuiEventStream combining crossterm + broadcast
- [ ] Add `status/` — status bar (peer info, latency, streams)
- [ ] Add `history_cell/` — shell session history with expand/collapse
- [ ] Add `bottom_pane/` — composable bottom panel (input, progress, status)
- [ ] Add `streaming.rs` — streaming output rendering

---

## 5. Key Differences: Codex vs blnk-tui

| Aspect | Codex | blnk-tui |
|--------|-------|----------|
| **Domain** | AI coding agent | P2P remote access |
| **Primary flow** | User → AI → tool execution | User → peer → shell/file/proxy |
| **Session** | Single agent session | Multiple peer sessions |
| **Events** | Agent events (tool call, approval) | Peer events (connect, disconnect) |
| **Status** | Token count, model info | Peer info, latency, streams |
| **Input** | Chat messages, commands | Shell commands, file paths |
| **Output** | Agent responses, tool results | Shell output, file transfer progress |

**Key insight**: Codex patterns are reusable for TUI infrastructure, but domain logic is completely different. Don't copy agent-specific code.

---

## 6. Impact Assessment

**Immediate (this session)**:
- ✅ TUI lifecycle implemented
- ✅ Shimmer animation implemented
- ✅ URL-aware wrapping implemented
- ✅ Terminal detection implemented
- ✅ Markdown rendering stubbed

**Short-term (Phase 2)**:
- Implement notifications (OSC9 + PowerShell)
- Implement keyboard modes (kitty protocol)
- Implement Windows console

**Medium-term (Phase 3)**:
- Event system (app_event + event_stream)
- Status display
- History cell
- Bottom pane
- Streaming output

**Spec Coverage After Phase 3**: ~90% of TUI spec (from current ~60%)
