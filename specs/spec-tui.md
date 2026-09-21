# blnk-tui Module Specification

> Design spec for the blnk TUI crate — module architecture, interfaces, and integration.
> Last updated: 2026-09-17

## Overview

blnk-tui is the terminal user interface for blnk — a P2P remote access tool. It provides a ratatui-based TUI for:

- **Device discovery** — find and list peers on LAN (`blnk devices`)
- **QR pairing** — display QR codes for quick device pairing (`blnk serve --qr`)
- **Session management** — connect, authenticate, and manage remote sessions
- **Shell streaming** — remote PTY with resize support
- **File transfer** — upload/download with progress display (`blnk cp`)
- **Proxy streams** — TCP/WebSocket/HTTP proxy management
- **Connection status** — peer info, latency, active stream count

### Architecture

```
┌─────────────────────────────────┐
│          TUI (blnk-tui)         │
│  serve / connect / pair / shell │
│  file transfer / proxy / status │
└──────────┬──────────────────────┘
           │
    ┌──────┴──────┐
    │  blnk core  │
    │  signaling  │
    │  WebRTC     │
    │  streams    │
    └──────┬──────┘
           │
    ┌──────┴──────────────┐
    │  Web Control Surface│
    │  (Axum loopback)    │
    │  QR code + PIN      │
    │  บน browser         │
    └─────────────────────┘
```

TUI จัดการทุกอย่าง — browser แสดง QR/PIN สำหรับ pairing

### Design Principles

- **blnk-native, not Codex clone** — inspired by Codex's architecture but designed for blnk's remote access use case
- **Deep modules** — small interface, large implementation (see `specs/spec-tui.md`)
- **URL-aware rendering** — terminal output frequently contains URLs; wrapping preserves them
- **Platform-aware** — kitty keyboard protocol, Sixel/Kitty images, Windows console support
- **Style guide compliant** — colors enforced via clippy.toml (cyan/green/red/magenta only)

## Architecture

### Dependency Graph

```
                    ┌─────────────┐
                    │   tui.rs    │  Terminal lifecycle
                    │  (seam: OS) │
                    └──────┬──────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
   ┌────┴────┐      ┌─────┴─────┐     ┌─────┴─────┐
   │keyboard │      │ terminal  │     │ windows   │
   │ _modes  │      │ _palette  │     │ _console  │
   └─────────┘      └─────┬─────┘     └───────────┘
                          │
                    ┌─────┴─────┐
                    │  color.rs │  RGB math
                    └─────┬─────┘
                          │
                    ┌─────┴──────┐
                    │ shimmer.rs │  Animation
                    └────────────┘

                    ┌────────────┐
                    │  width.rs  │  Display width
                    └─────┬──────┘
                          │
              ┌───────────┼───────────┐
              │           │           │
        ┌─────┴─────┐ ┌──┴───┐ ┌────┴──────┐
        │ terminal  │ │render│ │ wrapping  │
        │_hyperlinks│ │ /    │ │   .rs     │
        └───────────┘ │utils │ │(URL-aware)│
                      └──┬───┘ └───────────┘
                         │
                 ┌───────┴───────┐
                 │ render/       │
                 │ markdown.rs   │
                 │ render/       │
                 │ records.rs    │
                 └───────────────┘

        ┌──────────┐  ┌──────────┐  ┌──────────┐
        │notifi-   │  │  pets.rs │  │workspace │
        │cations   │  │          │  │_messages │
        └──────────┘  └──────────┘  └──────────┘
              │              │              │
              └──────────────┼──────────────┘
                        ┌────┴────┐
                        │ proto.rs│  Type stubs
                        └─────────┘
```

### Layer Model

| Layer | Modules | Role |
|---|---|---|
| **Foundation** | `width`, `color`, `terminal_palette` | Pure math, no I/O |
| **Terminal** | `terminal_hyperlinks`, `wrapping`, `shimmer` | Text rendering primitives |
| **Render** | `render/line_utils`, `render/markdown`, `render/records` | Layout and content rendering |
| **Platform** | `tui`, `keyboard_modes`, `windows_console` | OS interaction, lifecycle |
| **Feature** | `notifications`, `pets`, `workspace_messages` | Domain-specific features |
| **Protocol** | `proto` | Type definitions |

---

## Module Specifications

### 1. width.rs — Display Width

**Purpose:** Calculate terminal cell width for text, handling Unicode edge cases.

**Interface:**
```rust
pub(crate) fn display_width(text: &str) -> usize;
pub(crate) fn char_width(ch: char) -> usize;
pub(crate) fn usable_content_width(total_width: usize, reserved_cols: usize) -> Option<usize>;
pub(crate) fn usable_content_width_u16(total_width: u16, reserved_cols: u16) -> Option<usize>;
```

**Invariants:**
- `display_width` matches ratatui's terminal-cell semantics
- Halfwidth sound marks (FF9E, FF9F) count as 1 cell each
- `usable_content_width` returns `Some(n)` where `n > 0`, or `None` when exhausted
- `None` means "render prefix-only fallback", never attempt zero-width rendering

**Used by:** wrapping, render/records, shimmer, render/markdown

---

### 2. color.rs — RGB Color Math

**Purpose:** Perceptual color operations for TUI rendering.

**Interface:**
```rust
pub(crate) fn is_light(bg: (u8, u8, u8)) -> bool;
pub(crate) fn blend(fg: (u8, u8, u8), bg: (u8, u8, u8), alpha: f32) -> (u8, u8, u8);
pub(crate) fn perceptual_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32;
```

**Invariants:**
- `is_light` uses ITU-R BT.601 luma coefficients
- `blend` is standard alpha compositing
- `perceptual_distance` uses CIE76 (Euclidean in Lab space)

**Used by:** shimmer, terminal_palette

---

### 3. terminal_palette.rs — Terminal Default Colors

**Purpose:** Detect terminal's default foreground/background colors.

**Interface:**
```rust
pub(crate) fn default_bg() -> Option<(u8, u8, u8)>;
pub(crate) fn default_fg() -> Option<(u8, u8, u8)>;
```

**Invariants:**
- Results are cached via `OnceLock` (detect once per process)
- Falls back to sensible defaults (dark theme: bg=(30,30,30), fg=(204,204,204))
- Parses `COLORFGBG` environment variable
- Converts ANSI 256-color index to RGB

**Used by:** shimmer, tui (init probe)

---

### 4. terminal_hyperlinks.rs — OSC 8 Hyperlinks

**Purpose:** Annotate ratatui Lines with clickable hyperlink regions.

**Interface:**
```rust
pub struct Hyperlink {
    pub url: String,
    pub columns: Range<usize>,
}

pub struct HyperlinkLine {
    pub line: Line<'static>,
    pub hyperlinks: Vec<Hyperlink>,
}

pub fn remap_wrapped_line(source: &HyperlinkLine, wrapped: Vec<Line<'static>>) -> Vec<HyperlinkLine>;
```

**Invariants:**
- `remap_wrapped_line` adjusts column offsets after word wrapping
- Hyperlink columns are byte-range-based, not grapheme-based
- Empty hyperlinks list means no clickable regions

**Used by:** render/markdown, render/records, wrapping

---

### 5. wrapping.rs — URL-Aware Word Wrapping

**Purpose:** Word-wrap ratatui Lines while preserving URLs and handling Unicode edge cases.

**Interface:**
```rust
pub struct RtOptions<'a> {
    pub width: usize,
    pub initial_indent: Line<'a>,
    pub subsequent_indent: Line<'a>,
    pub break_words: bool,
    pub word_separator: textwrap::WordSeparator,
    pub word_splitter: textwrap::WordSplitter,
    pub wrap_algorithm: textwrap::WrapAlgorithm,
    pub line_ending: textwrap::LineEnding,
}

// Standard wrapping (plain prose)
pub(crate) fn word_wrap_line(line: &Line, width_or_options: impl Into<RtOptions>) -> Vec<Line>;
pub(crate) fn word_wrap_lines(lines: impl IntoIterator, width_or_options: impl Into<RtOptions>) -> Vec<Line<'static>>;

// Adaptive wrapping (URL-aware)
pub(crate) fn adaptive_wrap_line(line: &Line, base: RtOptions) -> Vec<Line>;
pub(crate) fn adaptive_wrap_lines(lines: impl IntoIterator, width: RtOptions) -> Vec<Line<'static>>;

// URL detection
pub(crate) fn text_contains_url_like(text: &str) -> bool;
pub(crate) fn line_contains_url_like(line: &Line) -> bool;
pub(crate) fn line_has_mixed_url_and_non_url_tokens(line: &Line) -> bool;

// Range mapping (for cursor positioning)
pub(crate) fn wrap_ranges(text: &str, options: impl Into<textwrap::Options>) -> Vec<Range<usize>>;
pub(crate) fn wrap_ranges_trim(text: &str, options: impl Into<textwrap::Options>) -> Vec<Range<usize>>;
```

**Invariants:**
- URL-like tokens are never split across lines
- Mixed URL/prose: URL stays intact, prose wraps at word boundaries
- Halfwidth sound marks (FF9E, FF9F) are projected to equal-width placeholders
- `wrap_ranges` returns cursor-oriented ranges that may overlap by 1 byte
- `wrap_ranges_trim` returns non-overlapping ranges without trailing spaces

**URL Detection Rules:**
- Absolute URLs: `scheme://host` (http, https, ftp, custom schemes)
- Bare domains: `host[:port]/path` (requires recognized TLD or `www.` prefix)
- IPv4: `192.168.1.1:8080/health`
- Excludes: file paths (`src/main.rs`), `key:value`, plain text with dashes

**Used by:** render/markdown, render/records

---

### 6. shimmer.rs — Shimmer Animation

**Purpose:** Time-based sweep animation for branding text.

**Interface:**
```rust
pub(crate) fn shimmer_spans(text: &str) -> Vec<Span<'static>>;
```

**Invariants:**
- Sweep period: 2 seconds, synchronized to process start
- Band half-width: 5 characters
- Cosine interpolation for smooth gradient
- True-color terminals: uses `blend()` for RGB gradient
- Fallback: dim/bold intensity levels for non-true-color

**Style Guide Compliance:**
- Uses terminal default fg/bg colors (from `terminal_palette`)
- `#[allow(clippy::disallowed_methods)]` for `Color::Rgb` — intentional override

**Used by:** tui (branding display)

---

### 7. tui.rs — Terminal Lifecycle

**Purpose:** Initialize, manage, and restore terminal state.

**Interface:**
```rust
pub type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub fn init() -> Result<Terminal>;
pub fn restore() -> Result<()>;
pub fn restore_after_exit() -> Result<()>;
pub fn draw<F>(terminal: &mut Terminal, draw_fn: F) -> Result<()>;
pub fn enter_alt_screen(terminal: &mut Terminal) -> Result<()>;
pub fn leave_alt_screen(terminal: &mut Terminal) -> Result<()>;
```

**Invariants:**
- `init()` enables raw mode + bracketed paste + alternate screen
- `init()` installs panic hook that calls `restore_after_exit()`
- `draw()` uses synchronized update for flicker-free rendering
- `enter_alt_screen`/`leave_alt_screen` manage alternate scroll
- Non-TTY stdin/stdout returns error immediately

**Integration:**
```
tui.rs::init()
  ├─ keyboard_modes::enable_keyboard_enhancement() → KeyboardCapability
  ├─ windows_console::init_console() → ConsoleState (Windows only)
  ├─ terminal_palette::detect() → cached colors
  └─ return Terminal
```

**Used by:** main.rs (application entry point)

---

### 8. keyboard_modes.rs — Kitty Keyboard Protocol

**Purpose:** Detect and manage keyboard enhancement protocols.

**Interface:**
```rust
pub(crate) fn enable_keyboard_enhancement() -> KeyboardCapability;
pub(crate) fn restore_keyboard_enhancement() -> Result<()>;
pub(crate) fn running_in_vscode_terminal() -> bool;

pub(crate) enum KeyboardCapability {
    KittyProtocol,    // CSI ? u — full modifier disambiguation
    BasicEnhancement, // modifyOtherKeys level 1/2
    None,             // No enhancement available
}
```

**Invariants:**
- Stack-based save/restore (supports nested `set_modes` calls)
- Kitty protocol detected via CSI ? u flag query
- modifyOtherKeys used as fallback (xterm-compatible)
- VSCode terminal: modifyOtherKeys only (no Kitty)
- Graceful degradation: if query fails, assume `None`

**Detection Flow:**
1. Send CSI ? u query
2. Read response: `CSI ? <flags> u`
3. If flags include `1` → KittyProtocol
4. Else if `modifyOtherKeys` env var set → BasicEnhancement
5. Else → None

**Used by:** tui.rs (init), main.rs (key event parsing)

---

### 9. windows_console.rs — Win32 Console Management

**Purpose:** Save/restore Windows console state for VT processing.

**Interface:**
```rust
pub(crate) fn init_console() -> Result<ConsoleState>;
pub(crate) fn restore_console(state: &ConsoleState) -> Result<()>;
pub(crate) fn flush_input() -> Result<()>;
pub(crate) fn ensure_virtual_terminal_processing() -> Result<()>;

pub(crate) struct ConsoleState {
    input_mode: u32,
    output_mode: u32,
    vt_enabled: bool,
}
```

**Invariants:**
- `ConsoleState` captures original mode on init
- `restore_console` returns to exact original state
- `flush_input` clears buffered typeahead (FlushConsoleInputBuffer)
- `ensure_virtual_terminal_processing` enables VT on stdout+stderr
- Unix: all functions are no-op (terminal handles this natively)

**Win32 API Usage:**
- `GetConsoleMode` / `SetConsoleMode`
- `ENABLE_VIRTUAL_TERMINAL_PROCESSING` (0x0004)
- `ENABLE_PROCESSED_OUTPUT` (0x0001)
- `FlushConsoleInputBuffer`
- `GetStdHandle(STD_OUTPUT_HANDLE)` / `GetStdHandle(STD_ERROR_HANDLE)`

**Used by:** tui.rs (init/restore), main.rs (exit cleanup)

---

### 10. notifications.rs — Desktop Notifications

**Purpose:** Send system notifications on peer connect/disconnect events.

**Interface:**
```rust
pub(crate) trait NotificationBackend: Send + Sync {
    fn notify(&mut self, message: &str) -> Result<(), NotificationError>;
    fn method(&self) -> NotificationMethod;
}

pub(crate) fn detect_backend(method: NotificationMethod) -> Option<Box<dyn NotificationBackend>>;
pub(crate) fn should_emit(condition: NotificationCondition, terminal_focused: bool) -> bool;

pub(crate) enum NotificationMethod {
    Osc9,       // Terminal built-in (Linux/macOS)
    External,   // notify-send / PowerShell (external command)
    Disabled,   // No notifications
}

pub(crate) enum NotificationCondition {
    Unfocused,  // Only when terminal is unfocused
    Always,     // Always notify
}
```

**Invariants:**
- `detect_backend` returns `None` for `Disabled`
- `should_emit` returns `false` when condition doesn't match focus state
- Backend auto-disables on repeated failures (defensive)
- Focus state from crossterm `FocusGained`/`FocusLost` events

**Adapters:**
- `Osc9Backend`: writes `\x1b]9;message\x07` to stdout
- `ExternalBackend`: spawns `notify-send` (Linux) or PowerShell (Windows)

**Events:** Peer connect/disconnect only (not file transfer, not session events)

**Used by:** tui.rs (draw cycle), main.rs (peer event handler)

---

### 11. pets.rs — Sixel/Kitty Image Rendering

**Purpose:** Render ambient pet images in the terminal background.

**Interface:**
```rust
pub(crate) trait PetImageBackend: Send + Sync {
    fn render(&self, writer: &mut impl Write, img: &DynamicImage, pos: Position) -> Result<()>;
    fn clear(&self, writer: &mut impl Write, pos: Position, size: Size) -> Result<()>;
    fn supports_protocol(&self) -> bool;
}

pub(crate) fn detect_backend() -> Box<dyn PetImageBackend>;

pub(crate) struct PetImageRenderState {
    // Cached decoded image, position, animation frame
}

pub(crate) struct AmbientPetDraw {
    // Pet selection, position, opacity
}

pub(crate) enum PetImageRenderError {
    Terminal(std::io::Error),
    Asset(String),
}
```

**Invariants:**
- `detect_backend` probes terminal: TERM_PROGRAM, DA1 response
- Image decoded once via `image` crate, cached in `PetImageRenderState`
- Position: character grid coordinates (not pixel coordinates)
- `clear` restores original terminal content (if possible)
- No-op backend when no protocol supported

**Adapters:**
- `SixelBackend`: sixel escape sequences (Linux/macOS)
- `KittyBackend`: Kitty graphics protocol (Kitty, WezTerm, Ghostty)
- `NoopBackend`: unsupported terminals

**Detection Flow:**
1. Check `TERM_PROGRAM` for known terminals
2. Send DA1 query (`CSI 0c` or `CSI c`)
3. Parse response for Sixel/Kitty capability
4. Fall back to `NoopBackend`

**Image Formats:** JPEG, PNG, GIF, WEBP (via `image` crate features)

**Used by:** tui.rs (draw cycle, ambient rendering)

---

### 12. workspace_messages.rs — Workspace Headlines

**Purpose:** Extract workspace headlines from protobuf messages.

**Interface:**
```rust
pub(crate) fn workspace_headline_from_response(
    response: GetWorkspaceMessagesResponse,
) -> WorkspaceHeadlineFetchResult;

pub(crate) enum WorkspaceHeadlineFetchResult {
    Available(Option<String>),
    FeatureDisabled,
}

pub(crate) const WORKSPACE_HEADLINE_REFRESH_INTERVAL: Duration;
```

**Invariants:**
- Only `WorkspaceMessageType::Headline` messages are extracted
- Empty/whitespace-only headlines filtered out
- `FeatureDisabled` returned when `feature_enabled == false`
- Refresh interval: 5 minutes

**Used by:** tui.rs (header display)

---

### 13. render/markdown.rs — Streaming Markdown

**Purpose:** Single-pass markdown rendering with block boundary tracking.

**Interface:**
```rust
pub(crate) struct StreamingMarkdownRender {
    pub(crate) lines: Vec<HyperlinkLine>,
    pub(crate) pending_math_start: Option<usize>,
    pub(crate) last_top_level_block_start: Option<usize>,
    pub(crate) has_reference_link_definition: bool,
    pub(crate) first_top_level_block_is_html: bool,
}
```

**Invariants:**
- Single parser pass collects both styled output and metadata
- Block offsets refer to exact source text (not normalized)
- `pending_math_start` tracks unfinished display equations
- `last_top_level_block_start` enables incremental updates
- Reference definitions can retroactively change rendering

**Dependencies:** pulldown-cmark, terminal_hyperlinks, width

**Used by:** main rendering loop (agent output display)

---

### 14. render/records.rs — Vertical Table Rendering

**Purpose:** Render markdown tables in key/value format when grid layout is unreadable.

**Interface:**
```rust
pub(crate) fn should_render_records(rows: &[Vec<TableCell>], column_widths: &[usize], metrics: &[TableColumnMetrics]) -> bool;
pub(crate) fn render_records(headers: &[TableCell], rows: &[Vec<TableCell>], metrics: &[TableColumnMetrics], available_width: Option<usize>, label_style: Style, separator_style: Style) -> Vec<HyperlinkLine>;
```

**Invariants:**
- Auto-switch from grid to key/value when:
  - Token-heavy columns have fragmented words
  - Narrative columns are cramped (≥7 lines in narrow width)
  - 2+ expansive cells are starved (≥4 lines each)
- Aligned mode: label left-aligned, value right-aligned
- Stacked mode: label above value (when width insufficient)
- Separator: `─` character between records

**Thresholds:**
- `MIN_ALIGNED_COMPACT_VALUE_WIDTH`: 12
- `MIN_ALIGNED_EXPANSIVE_VALUE_WIDTH`: 24
- `CRAMPED_EXPANSIVE_CELL_LINES`: 4
- `CATASTROPHIC_NARRATIVE_CELL_LINES`: 7

**Used by:** markdown table rendering

---

### 15. proto.rs — Type Stubs

**Purpose:** Stub protobuf types for development.

**Interface:**
```rust
pub mod blnk {
    pub mod workspace {
        pub enum WorkspaceMessageType { Unspecified, Headline, Announcement, System }
        pub struct WorkspaceMessage { message_id, message_type, message_body, ... }
        pub struct GetWorkspaceMessagesRequest { workspace_id, limit }
        pub struct GetWorkspaceMessagesResponse { feature_enabled, messages }
    }
}
```

**Invariants:**
- Mirrors `proto/workspace.proto` structure exactly
- Temporary: replace with prost-generated types when integrating with blnk core

---

## Integration Flow

### Application Startup

```
main()
  └─ tui::init()
       ├─ keyboard_modes::enable_keyboard_enhancement() → KeyboardCapability
       ├─ windows_console::init_console() → ConsoleState (Windows)
       ├─ terminal_palette::default_bg()/default_fg() → cached colors
       ├─ enable_raw_mode()
       ├─ execute!(EnterAlternateScreen, EnableBracketedPaste)
       └─ return Terminal
```

### Draw Cycle

```
tui::draw(terminal, |frame| {
    // 1. Header (workspace headline)
    if let Some(headline) = cached_headline {
        render_headline(frame, headline);
    }

    // 2. Content (markdown output)
    let rendered = render_streaming_markdown(source, width);
    render_lines(frame, &rendered.lines);

    // 3. Shimmer (branding)
    if show_branding {
        let spans = shimmer_spans("blnk");
        render_branding(frame, &spans);
    }

    // 4. Ambient pet (background)
    if let Some(pet) = pet_state {
        pets::render(frame, &pet);
    }
});
```

### Event Handling

```
loop {
    match event::read()? {
        Event::Key(key) => {
            // keyboard_modes capability determines key parsing
            match capability {
                KittyProtocol => parse_kitty_key(key),
                BasicEnhancement => parse_modify_other_keys(key),
                None => parse_legacy_key(key),
            }
        }
        Event::FocusLost => {
            if should_emit(notification_condition, false) {
                notification_backend.notify("Peer connected");
            }
        }
        _ => {}
    }
}
```

---

## Style Guide Compliance

### Color Usage (clippy.toml enforced)

| Context | Color | Source |
|---|---|---|
| Headers | bold | ratatui Modifier::BOLD |
| Secondary text | dim | ratatui Modifier::DIM |
| User input tips | cyan | Color::Cyan |
| Success/additions | green | Color::Green |
| Errors/failures | red | Color::Red |
| "blnk" branding | magenta | Color::Magenta |
| Shimmer gradient | RGB blend | `#[allow(clippy::disallowed_methods)]` |

### Banned Colors

- `Color::Black`, `Color::White` as foreground (use default/reset)
- `Color::Blue`, `Color::Yellow` (not in style guide)
- `Color::Rgb` (except shimmer, with explicit allow)
- `Color::Indexed` (use named ANSI colors)

---

## Testing Strategy

### Unit Tests (per module)
- `color.rs`: blend, luma, perceptual distance
- `width.rs`: display width, sound marks, usable width
- `wrapping.rs`: URL detection, wrap ranges, adaptive wrapping
- `terminal_palette.rs`: ANSI→RGB conversion
- `workspace_messages.rs`: headline extraction, filtering

### Integration Tests
- `tui.rs`: init/restore cycle, panic hook
- `render/records.rs`: grid→key/value threshold detection
- `render/markdown.rs`: streaming block boundary tracking

### Manual Testing
- Kitty protocol detection (real terminal)
- Sixel rendering (if available)
- Cross-platform: Linux, macOS, Windows, Android/Termux

---

## Future Work

1. **Incremental markdown rendering** — use `last_top_level_block_start` for partial re-renders
2. **Image caching** — decode once, render multiple frames
3. **Protocol negotiation** — dynamic capability detection at runtime
4. **Accessibility** — screen reader support, high contrast mode
