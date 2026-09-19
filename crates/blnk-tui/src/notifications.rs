//! Desktop notification support for blnk TUI.
//!
//! Sends system notifications when the terminal is unfocused.
//! Stub — implement notification delivery per-platform later.
#![allow(dead_code)]

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationMethod {
    /// Use the terminal's built-in OSC 9 notification protocol.
    Osc9,
    /// Use notify-send / PowerShell (external command).
    External,
    /// Notifications disabled.
    Disabled,
}

impl Default for NotificationMethod {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationCondition {
    /// Notify only when terminal is unfocused.
    Unfocused,
    /// Always notify.
    Always,
}

impl Default for NotificationCondition {
    fn default() -> Self {
        Self::Unfocused
    }
}

#[derive(Debug)]
pub struct NotificationError;

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "notification failed")
    }
}

impl std::error::Error for NotificationError {}

pub trait NotificationBackend: Send + Sync {
    fn notify(&mut self, message: &str) -> Result<(), NotificationError>;
    fn method(&self) -> NotificationMethod;
}

pub fn detect_backend(method: NotificationMethod) -> Option<Box<dyn NotificationBackend>> {
    match method {
        NotificationMethod::Disabled => None,
        _ => {
            // TODO: implement per-platform detection
            None
        }
    }
}

pub fn should_emit(condition: NotificationCondition, terminal_focused: bool) -> bool {
    match condition {
        NotificationCondition::Unfocused => !terminal_focused,
        NotificationCondition::Always => true,
    }
}
