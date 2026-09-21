//! Terminal emulator detection — conservative heuristic for output rendering.
//!
//! Detects terminal name, multiplexer presence, and hyperlink capability.
//! Unknown terminals default to "show everything" (no assumptions).

use std::sync::LazyLock;

/// Detected terminal emulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalName {
    Ghostty,
    Iterm2,
    WezTerm,
    Kitty,
    VsCode,
    Alacritty,
    WindowsTerminal,
    Konsole,
    GnomeTerminal,
    Vte,
    AppleTerminal,
    WarpTerminal,
    Dumb,
    Unknown,
}

impl TerminalName {
    /// Whether this terminal supports OSC 8 hyperlinks natively.
    pub fn supports_hyperlinks(self) -> bool {
        matches!(
            self,
            Self::Ghostty
                | Self::Iterm2
                | Self::WezTerm
                | Self::Kitty
                | Self::VsCode
                | Self::Alacritty
                | Self::WindowsTerminal
                | Self::Konsole
                | Self::GnomeTerminal
                | Self::Vte
        )
    }
}

/// Multiplexer detected in the terminal session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Multiplexer {
    Tmux,
    Screen,
}

/// Combined terminal environment info.
#[derive(Clone, Copy, Debug)]
pub struct TerminalInfo {
    pub name: TerminalName,
    pub multiplexer: Option<Multiplexer>,
}

impl TerminalInfo {
    /// Detect from environment variables. Cached via `LazyLock`.
    pub fn detect() -> &'static Self {
        &TERMINAL_INFO
    }

    /// Whether this terminal can render clickable hyperlinks (OSC 8).
    /// Multiplexers generally break hyperlink rendering.
    pub fn supports_hyperlinks(&self) -> bool {
        self.multiplexer.is_none() && self.name.supports_hyperlinks()
    }
}

static TERMINAL_INFO: LazyLock<TerminalInfo> = LazyLock::new(|| {
    let multiplexer = detect_multiplexer();
    let name = detect_terminal_name();
    TerminalInfo { name, multiplexer }
});

fn detect_terminal_name() -> TerminalName {
    // TERM_PROGRAM is the most reliable signal
    if let Ok(program) = std::env::var("TERM_PROGRAM") {
        match program.as_str() {
            "ghostty" => return TerminalName::Ghostty,
            "iTerm.app" => return TerminalName::Iterm2,
            "WezTerm" => return TerminalName::WezTerm,
            "kitty" => return TerminalName::Kitty,
            "vscode" => return TerminalName::VsCode,
            "Alacritty" => return TerminalName::Alacritty,
            "Apple_Terminal" => return TerminalName::AppleTerminal,
            "WarpTerminal" => return TerminalName::WarpTerminal,
            _ => {}
        }
    }

    // VTE-based terminals set VTE_VERSION
    if std::env::var("VTE_VERSION").is_ok() {
        // Could be Konsole, GNOME Terminal, or generic VTE
        if let Ok(term) = std::env::var("TERM") {
            if term.contains("konsole") {
                return TerminalName::Konsole;
            }
        }
        // GNOME Terminal doesn't set a unique signal; treat as VTE
        return TerminalName::Vte;
    }

    // Windows Terminal sets WT_SESSION
    if std::env::var("WT_SESSION").is_ok() {
        return TerminalName::WindowsTerminal;
    }

    // Fallback
    match std::env::var("TERM").ok().as_deref() {
        Some("dumb") => TerminalName::Dumb,
        _ => TerminalName::Unknown,
    }
}

fn detect_multiplexer() -> Option<Multiplexer> {
    // tmux sets TMUX
    if std::env::var("TMUX").is_ok() {
        return Some(Multiplexer::Tmux);
    }
    // screen sets STY
    if std::env::var("STY").is_ok() {
        return Some(Multiplexer::Screen);
    }
    None
}
