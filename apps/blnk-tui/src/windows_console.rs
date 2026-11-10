//! Windows console input mode management.
//!
//! Handles VT input/output mode saving and restoration on Windows.
//! Stub — fill in when Windows support is needed.
#![allow(dead_code)]

#[cfg(windows)]
pub(crate) fn set_input_record_mode() -> std::io::Result<()> {
    // TODO: save and set console input record mode
    Ok(())
}

#[cfg(windows)]
pub(crate) fn restore_input_mode() -> std::io::Result<()> {
    // TODO: restore saved console input mode
    Ok(())
}

#[cfg(windows)]
pub(crate) fn input_record_mode(mode: u32) -> u32 {
    // TODO: compute VT input flag mask
    mode & !0x200
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VirtualTerminalInput {
    Enabled,
    Disabled,
}

#[cfg(windows)]
pub(crate) fn restored_input_mode(original: u32, vt_input: VirtualTerminalInput) -> u32 {
    match vt_input {
        VirtualTerminalInput::Enabled => original | 0x200,
        VirtualTerminalInput::Disabled => original & !0x200,
    }
}
