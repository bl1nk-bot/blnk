//! RGB color utilities for TUI rendering.

#![allow(dead_code)]
/// Perceived brightness of a background color (0–255 luma).
/// Uses ITU-R BT.601 luminance coefficients.
pub(crate) fn is_light(bg: (u8, u8, u8)) -> bool {
    let (r, g, b) = bg;
    let y = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    y > 128.0
}

/// Alpha-composite foreground over background.
pub(crate) fn blend(fg: (u8, u8, u8), bg: (u8, u8, u8), alpha: f32) -> (u8, u8, u8) {
    let r = (fg.0 as f32 * alpha + bg.0 as f32 * (1.0 - alpha)) as u8;
    let g = (fg.1 as f32 * alpha + bg.1 as f32 * (1.0 - alpha)) as u8;
    let b = (fg.2 as f32 * alpha + bg.2 as f32 * (1.0 - alpha)) as u8;
    (r, g, b)
}

/// Perceptual color distance between two RGB colors.
/// CIE76 formula — Euclidean distance in CIE L*a*b* space.
pub(crate) fn perceptual_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    fn srgb_to_linear(c: u8) -> f32 {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn rgb_to_xyz(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
        let r = srgb_to_linear(r);
        let g = srgb_to_linear(g);
        let b = srgb_to_linear(b);
        let x = r * 0.4124 + g * 0.3576 + b * 0.1805;
        let y = r * 0.2126 + g * 0.7152 + b * 0.0722;
        let z = r * 0.0193 + g * 0.1192 + b * 0.9505;
        (x, y, z)
    }

    fn xyz_to_lab(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
        let xr = x / 0.95047;
        let yr = y / 1.00000;
        let zr = z / 1.08883;

        fn f(t: f32) -> f32 {
            if t > 0.008856 {
                t.powf(1.0 / 3.0)
            } else {
                7.787 * t + 16.0 / 116.0
            }
        }

        let fx = f(xr);
        let fy = f(yr);
        let fz = f(zr);

        let l = 116.0 * fy - 16.0;
        let a = 500.0 * (fx - fy);
        let b = 200.0 * (fy - fz);
        (l, a, b)
    }

    let (x1, y1, z1) = rgb_to_xyz(a.0, a.1, a.2);
    let (x2, y2, z2) = rgb_to_xyz(b.0, b.1, b.2);

    let (l1, a1, b1) = xyz_to_lab(x1, y1, z1);
    let (l2, a2, b2) = xyz_to_lab(x2, y2, z2);

    let dl = l1 - l2;
    let da = a1 - a2;
    let db = b1 - b2;

    (dl * dl + da * da + db * db).sqrt()
}

/// Allowed semantic and terminal colors per STYLE.md.
///
/// Banned colors (`Black`, `White`, `Blue`, `Yellow`, `Indexed`) cannot be constructed
/// through this enum. Raw `ratatui::style::Color` is banned by Clippy (`disallowed_types`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeColor {
    /// User input tips, selection, status indicators.
    Cyan,
    /// Success, additions.
    Green,
    /// Errors, failures, deletions.
    Red,
    /// "blnk" brand text.
    Magenta,
    /// Terminal default / reset foreground.
    #[default]
    Reset,
}

impl ThemeColor {
    /// Convert to the underlying ratatui [`Color`](ratatui::style::Color).
    #[allow(clippy::disallowed_types)]
    #[inline]
    pub fn to_ratatui(self) -> ratatui::style::Color {
        match self {
            Self::Cyan => ratatui::style::Color::Cyan,
            Self::Green => ratatui::style::Color::Green,
            Self::Red => ratatui::style::Color::Red,
            Self::Magenta => ratatui::style::Color::Magenta,
            Self::Reset => ratatui::style::Color::Reset,
        }
    }
}

#[allow(clippy::disallowed_types)]
impl From<ThemeColor> for ratatui::style::Color {
    #[inline]
    fn from(color: ThemeColor) -> Self {
        color.to_ratatui()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::disallowed_types)]
    fn theme_color_converts_to_expected_ratatui_color() {
        assert_eq!(ThemeColor::Cyan.to_ratatui(), ratatui::style::Color::Cyan);
        assert_eq!(ThemeColor::Green.to_ratatui(), ratatui::style::Color::Green);
        assert_eq!(ThemeColor::Red.to_ratatui(), ratatui::style::Color::Red);
        assert_eq!(ThemeColor::Magenta.to_ratatui(), ratatui::style::Color::Magenta);
        assert_eq!(ThemeColor::Reset.to_ratatui(), ratatui::style::Color::Reset);
    }

    #[test]
    fn enforce_no_banned_colors_in_tui_sources() {
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let banned_patterns =
            ["Color::Black", "Color::White", "Color::Blue", "Color::Yellow", "Color::Indexed"];

        fn check_dir(dir: &std::path::Path, banned: &[&str]) {
            let entries = std::fs::read_dir(dir).expect("read dir");
            for entry in entries {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    check_dir(&path, banned);
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                    let content = std::fs::read_to_string(&path).expect("read file");
                    let code_to_check =
                        if path.file_name().and_then(|f| f.to_str()) == Some("color.rs") {
                            content.split("#[cfg(test)]").next().unwrap_or(&content)
                        } else {
                            &content
                        };
                    for &pattern in banned {
                        assert!(
                            !code_to_check.contains(pattern),
                            "Banned color pattern `{pattern}` found in `{}`. Use `ThemeColor` per STYLE.md.",
                            path.display()
                        );
                    }
                }
            }
        }

        check_dir(&src_dir, &banned_patterns);
    }

    #[test]
    fn black_is_not_light() {
        assert!(!is_light((0, 0, 0)));
    }

    #[test]
    fn white_is_light() {
        assert!(is_light((255, 255, 255)));
    }

    #[test]
    fn blend_same_color() {
        let c = (100, 150, 200);
        assert_eq!(blend(c, c, 0.5), c);
    }

    #[test]
    fn blend_opaque_fg() {
        assert_eq!(blend((255, 0, 0), (0, 0, 255), 1.0), (255, 0, 0));
    }

    #[test]
    fn blend_transparent_fg() {
        assert_eq!(blend((255, 0, 0), (0, 0, 255), 0.0), (0, 0, 255));
    }

    #[test]
    fn same_color_zero_distance() {
        let c = (128, 64, 32);
        assert!(perceptual_distance(c, c) < 0.01);
    }

    #[test]
    fn black_white_max_distance() {
        let d = perceptual_distance((0, 0, 0), (255, 255, 255));
        assert!(d > 90.0); // CIE76 black→white ≈ 100
    }
}
