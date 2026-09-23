//! Terminal initialization and lifecycle for blnk TUI.
//!
//! Handles raw mode, alternate screen, panic hook, and synchronized drawing.
//! Platform-specific features (notifications, pets, keyboard, Windows console)
//! are stubbed for later implementation in blnk's own style.

use std::fmt;
use std::io;
use std::io::IsTerminal;
use std::io::Result;
use std::io::Stdout;
use std::io::stdin;
use std::io::stdout;
use std::panic;

use crossterm::Command;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;

pub type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

/// Initialize terminal: raw mode + alternate screen + panic hook.
pub fn init() -> Result<Terminal> {
    if !stdin().is_terminal() {
        return Err(io::Error::other("stdin is not a terminal"));
    }
    if !stdout().is_terminal() {
        return Err(io::Error::other("stdout is not a terminal"));
    }

    enable_raw_mode()?;
    execute!(stdout(), EnableBracketedPaste)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = ratatui::Terminal::new(backend)?;

    // Clear and enter alternate screen
    execute!(stdout(), EnterAlternateScreen)?;
    terminal.clear()?;

    set_panic_hook();
    Ok(terminal)
}

/// Restore terminal to original state.
pub fn restore() -> Result<()> {
    let _ = execute!(stdout(), DisableBracketedPaste);
    let _ = execute!(stdout(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    let _ = execute!(
        stdout(),
        crossterm::cursor::SetCursorStyle::DefaultUserShape,
        crossterm::cursor::Show
    );
    Ok(())
}

/// Restore terminal after exit (stronger reset).
pub fn restore_after_exit() -> Result<()> {
    restore()
}

/// Draw a frame using synchronized update for flicker-free rendering.
pub fn draw<F>(terminal: &mut Terminal, draw_fn: F) -> Result<()>
where
    F: FnOnce(&mut ratatui::Frame),
{
    use crossterm::SynchronizedUpdate;

    stdout().sync_update(|_| {
        terminal.draw(draw_fn)?;
        Ok::<_, io::Error>(())
    })?
}

fn set_panic_hook() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_after_exit();
        hook(panic_info);
    }));
}

// ── Alternate scroll commands ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnableAlternateScroll;

impl Command for EnableAlternateScroll {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[?1007h")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> Result<()> {
        Err(io::Error::other(
            "tried to execute EnableAlternateScroll using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisableAlternateScroll;

impl Command for DisableAlternateScroll {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[?1007l")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> Result<()> {
        Err(io::Error::other(
            "tried to execute DisableAlternateScroll using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

/// Enter alternate screen (for overlays/modals).
pub fn enter_alt_screen(terminal: &mut Terminal) -> Result<()> {
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    execute!(terminal.backend_mut(), EnableAlternateScroll)?;
    terminal.clear()?;
    Ok(())
}

/// Leave alternate screen and return to inline view.
pub fn leave_alt_screen(terminal: &mut Terminal) -> Result<()> {
    execute!(terminal.backend_mut(), DisableAlternateScroll)?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.clear()?;
    Ok(())
}
