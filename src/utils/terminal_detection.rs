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
    detect_from_term_program()
        .or_else(detect_from_vte)
        .or_else(detect_from_wt)
        .unwrap_or_else(detect_from_term_fallback)
}

fn detect_from_term_program() -> Option<TerminalName> {
    let program = std::env::var("TERM_PROGRAM").ok()?;
    match program.as_str() {
        "ghostty" => Some(TerminalName::Ghostty),
        "iTerm.app" => Some(TerminalName::Iterm2),
        "WezTerm" => Some(TerminalName::WezTerm),
        "kitty" => Some(TerminalName::Kitty),
        "vscode" => Some(TerminalName::VsCode),
        "Alacritty" => Some(TerminalName::Alacritty),
        "Apple_Terminal" => Some(TerminalName::AppleTerminal),
        "WarpTerminal" => Some(TerminalName::WarpTerminal),
        _ => None,
    }
}

fn detect_from_vte() -> Option<TerminalName> {
    if std::env::var_os("VTE_VERSION").is_none() {
        return None;
    }
    let is_konsole = std::env::var("TERM").is_ok_and(|t| t.contains("konsole"));
    if is_konsole {
        Some(TerminalName::Konsole)
    } else {
        Some(TerminalName::Vte)
    }
}

fn detect_from_wt() -> Option<TerminalName> {
    std::env::var_os("WT_SESSION").map(|_| TerminalName::WindowsTerminal)
}

fn detect_from_term_fallback() -> TerminalName {
    match std::env::var("TERM").ok().as_deref() {
        Some("dumb") => TerminalName::Dumb,
        _ => TerminalName::Unknown,
    }
}

fn detect_multiplexer() -> Option<Multiplexer> {
    // tmux sets TMUX
    if std::env::var_os("TMUX").is_some() {
        return Some(Multiplexer::Tmux);
    }
    // screen sets STY
    if std::env::var_os("STY").is_some() {
        return Some(Multiplexer::Screen);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_hyperlink_support() {
        assert!(TerminalName::Ghostty.supports_hyperlinks());
        assert!(TerminalName::Iterm2.supports_hyperlinks());
        assert!(TerminalName::Kitty.supports_hyperlinks());
        assert!(TerminalName::VsCode.supports_hyperlinks());
        assert!(TerminalName::WindowsTerminal.supports_hyperlinks());
        assert!(!TerminalName::AppleTerminal.supports_hyperlinks());
        assert!(!TerminalName::WarpTerminal.supports_hyperlinks());
        assert!(!TerminalName::Dumb.supports_hyperlinks());
        assert!(!TerminalName::Unknown.supports_hyperlinks());
    }

    #[test]
    fn multiplexer_disables_hyperlinks() {
        let with_tmux = TerminalInfo {
            name: TerminalName::Kitty,
            multiplexer: Some(Multiplexer::Tmux),
        };
        assert!(!with_tmux.supports_hyperlinks());

        let direct = TerminalInfo {
            name: TerminalName::Kitty,
            multiplexer: None,
        };
        assert!(direct.supports_hyperlinks());
    }
}
