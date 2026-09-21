//! Web-link presentation policy for terminal output.
//!
//! Decides whether to show a bare URL or just a label, based on terminal
//! hyperlink support (OSC 8). Unknown terminals show the full URL (safe default).

use std::io::IsTerminal;
use std::sync::LazyLock;

use crate::utils::terminal_detection::TerminalInfo;

/// Check if `destination` looks like a web URL we could hyperlink.
pub fn is_web_destination(destination: &str) -> bool {
    destination.starts_with("http://") || destination.starts_with("https://")
}

/// Returns `true` if the URL should be hidden (label-only rendering).
///
/// Use this to decide output format:
/// ```ignore
/// if hide_web_link_destination(url) {
///     // render as clickable label (terminal handles the link)
///     println!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\");
/// } else {
///     // render as plain text with full URL
///     println!("{label}: {url}");
/// }
/// ```
pub fn hide_web_link_destination(destination: &str) -> bool {
    static POLICY: LazyLock<LinkPolicy> = LazyLock::new(LinkPolicy::detect);
    POLICY.should_hide(destination)
}

struct LinkPolicy {
    terminal: &'static TerminalInfo,
    is_tty: bool,
}

impl LinkPolicy {
    fn detect() -> Self {
        Self {
            terminal: TerminalInfo::detect(),
            is_tty: std::io::stdout().is_terminal(),
        }
    }

    fn should_hide(&self, destination: &str) -> bool {
        // Non-TTY: always show full URL
        if !self.is_tty {
            return false;
        }
        // Only hide if terminal supports hyperlinks AND it's a web URL
        self.terminal.supports_hyperlinks() && is_web_destination(destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_web_urls_never_hidden() {
        assert!(!hide_web_link_destination("ftp://example.com/file"));
        assert!(!hide_web_link_destination("/local/path"));
        assert!(!hide_web_link_destination("not a url"));
    }

    #[test]
    fn is_web_destination_check() {
        assert!(is_web_destination("https://example.com"));
        assert!(is_web_destination("http://example.com"));
        assert!(!is_web_destination("ftp://example.com"));
        assert!(!is_web_destination("/local/path"));
    }
}
