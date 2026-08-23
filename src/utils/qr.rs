use qrcode::QrCode;
use qrcode::render::unicode;

use crate::utils::error::BlnkError;

/// Renders a string as ANSI/Unicode block string QR code suitable for terminal display.
pub fn render_terminal_qr(data: &str) -> Result<String, BlnkError> {
    let code = QrCode::new(data.as_bytes())
        .map_err(|err| BlnkError::Protocol(format!("failed to generate QR code: {err}")))?;

    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Dark)
        .light_color(unicode::Dense1x2::Light)
        .quiet_zone(true)
        .build();

    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_terminal_qr_success() {
        let text = "blnk://pair?code=123456&uid=test-node";
        let qr = render_terminal_qr(text).expect("qr code render must succeed");
        assert!(!qr.is_empty());
        assert!(qr.contains('█') || qr.contains('▀') || qr.contains('▄') || qr.contains(' '));
    }

    #[test]
    fn test_render_terminal_qr_empty_error_or_success() {
        let text = "";
        let qr = render_terminal_qr(text).expect("empty string still produces minimal qr");
        assert!(!qr.is_empty());
    }
}
