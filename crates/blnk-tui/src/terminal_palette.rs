//! Terminal default color palette detection.
//!
//! Reads the terminal's default foreground/background via OSC 10/11 queries
//! or falls back to sensible defaults.
#![allow(dead_code)]

use std::sync::OnceLock;

/// Default background color (dark theme assumption).
const DEFAULT_BG: (u8, u8, u8) = (30, 30, 30);

/// Default foreground color (light on dark assumption).
const DEFAULT_FG: (u8, u8, u8) = (204, 204, 204);

static BG: OnceLock<Option<(u8, u8, u8)>> = OnceLock::new();
static FG: OnceLock<Option<(u8, u8, u8)>> = OnceLock::new();

/// Detect or return the terminal's default background color.
pub(crate) fn default_bg() -> Option<(u8, u8, u8)> {
    *BG.get_or_init(|| detect_color("COLORFGBG", true))
}

/// Detect or return the terminal's default foreground color.
pub(crate) fn default_fg() -> Option<(u8, u8, u8)> {
    *FG.get_or_init(|| detect_color("COLORFGBG", false))
}

/// Parse COLORFGBG="fg;bg" or fallback to defaults.
fn detect_color(env_var: &str, want_bg: bool) -> Option<(u8, u8, u8)> {
    // COLORFGBG is typically "fg;bg" or "fg;extra;bg"
    if let Ok(val) = std::env::var(env_var) {
        let parts: Vec<&str> = val.split(';').collect();
        if parts.len() >= 2 {
            let idx = if want_bg { parts.len() - 1 } else { 0 };
            if let Ok(n) = parts[idx].trim().parse::<u8>() {
                return ansi_to_rgb(n);
            }
        }
    }
    Some(if want_bg { DEFAULT_BG } else { DEFAULT_FG })
}

/// Convert ANSI color index (0–255) to RGB.
fn ansi_to_rgb(n: u8) -> Option<(u8, u8, u8)> {
    match n {
        0 => Some((0, 0, 0)),
        1 => Some((170, 0, 0)),
        2 => Some((0, 170, 0)),
        3 => Some((170, 85, 0)),
        4 => Some((0, 0, 170)),
        5 => Some((170, 0, 170)),
        6 => Some((0, 170, 170)),
        7 => Some((170, 170, 170)),
        8 => Some((85, 85, 85)),
        9 => Some((255, 85, 85)),
        10 => Some((85, 255, 85)),
        11 => Some((255, 255, 85)),
        12 => Some((85, 85, 255)),
        13 => Some((255, 85, 255)),
        14 => Some((85, 255, 255)),
        15 => Some((255, 255, 255)),
        16..=231 => {
            let n = n - 16;
            let r = n / 36;
            let g = (n % 36) / 6;
            let b = n % 6;
            let conv = |v: u8| -> u8 { if v == 0 { 0 } else { 55 + v * 40 } };
            Some((conv(r), conv(g), conv(b)))
        }
        232..=255 => {
            let v = 8 + (n - 232) * 10;
            Some((v, v, v))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_basic_colors() {
        assert_eq!(ansi_to_rgb(0), Some((0, 0, 0)));
        assert_eq!(ansi_to_rgb(15), Some((255, 255, 255)));
    }

    #[test]
    fn ansi_cube_colors() {
        // 16 = index 0,0,0 in 6x6x6 cube → (55,55,55)
        assert_eq!(ansi_to_rgb(16), Some((55, 55, 55)));
        // 231 = index 5,5,5 → (255,255,255)
        assert_eq!(ansi_to_rgb(231), Some((255, 255, 255)));
    }

    #[test]
    fn ansi_grayscale() {
        assert_eq!(ansi_to_rgb(232), Some((8, 8, 8)));
        assert_eq!(ansi_to_rgb(255), Some((238, 238, 238)));
    }
}
