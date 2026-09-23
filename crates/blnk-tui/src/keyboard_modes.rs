//! Keyboard mode management for blnk TUI.
//!
//! Handles keyboard enhancement detection and VSCode terminal detection.
//! Stub — implement platform-specific keyboard handling later.
#![allow(dead_code)]

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

static KEYBOARD_ENHANCEMENT_DISABLED: AtomicBool = AtomicBool::new(false);

/// Check if keyboard enhancement was explicitly disabled via env var.
pub(crate) fn keyboard_enhancement_disabled() -> bool {
    KEYBOARD_ENHANCEMENT_DISABLED.load(Ordering::Relaxed)
}

/// Detect if running inside VSCode's integrated terminal.
pub(crate) fn running_in_vscode_terminal() -> bool {
    std::env::var("TERM_PROGRAM")
        .map(|v| v == "vscode")
        .unwrap_or(false)
}

/// Enable keyboard enhancement (modifyOtherKeys / kitty protocol).
pub(crate) fn enable_keyboard_enhancement() {
    // TODO: detect and enable keyboard enhancement
    // Some terminals don't support this — fail gracefully
}

/// Restore keyboard enhancement state after external program.
pub(crate) fn restore_keyboard_enhancement_stack() {
    // TODO: pop enhancement state from stack
}

/// Reset keyboard reporting to defaults after exit.
pub(crate) fn reset_keyboard_reporting_after_exit() {
    // TODO: send reset sequence
}
