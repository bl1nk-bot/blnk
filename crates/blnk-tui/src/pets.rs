//! Ambient pet image rendering for blnk TUI.
//!
//! Stub — pet images are decorative elements rendered in the terminal background.
//! Implement image decoding and Sixel/Kitty protocol rendering later.
#![allow(dead_code)]

use std::fmt;

#[derive(Debug)]
pub enum PetImageRenderError {
    Terminal(std::io::Error),
    Asset(String),
}

impl fmt::Display for PetImageRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Terminal(e) => write!(f, "terminal error: {e}"),
            Self::Asset(msg) => write!(f, "asset error: {msg}"),
        }
    }
}

impl std::error::Error for PetImageRenderError {}

impl From<std::io::Error> for PetImageRenderError {
    fn from(e: std::io::Error) -> Self {
        Self::Terminal(e)
    }
}

#[derive(Debug, Default)]
pub struct PetImageRenderState {
    // Image data, position, animation state — implement later
}

#[derive(Debug)]
pub struct AmbientPetDraw {
    // Pet selection, position — implement later
}
