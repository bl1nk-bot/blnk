use qrcode::render::unicode::Dense1x2;
use qrcode::{QrCode, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingQrPayload {
    pub uid: String,
    pub pairing_code: String,
    pub endpoint: String,
}

impl PairingQrPayload {
    pub fn to_payload_string(&self) -> String {
        serde_json::json!({
            "uid": self.uid,
            "pairing_code": self.pairing_code,
            "endpoint": self.endpoint,
        })
        .to_string()
    }
}

/// Render string as terminal-compatible ANSI/Unicode QR code.
pub fn render_qr_terminal(content: &str) -> Result<String, qrcode::types::QrError> {
    let code = QrCode::with_version(content, Version::Normal(4), qrcode::EcLevel::M)
        .or_else(|_| QrCode::new(content))?;
    let image = code
        .render::<Dense1x2>()
        .dark_color(Dense1x2::Light)
        .light_color(Dense1x2::Dark)
        .quiet_zone(true)
        .build();
    Ok(image)
}

/// Render ascii matrix representation as fallback
pub fn render_qr_ascii(content: &str) -> Result<String, qrcode::types::QrError> {
    let code = QrCode::new(content)?;
    let string = code
        .render::<char>()
        .quiet_zone(true)
        .module_dimensions(2, 1)
        .build();
    Ok(string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairing_qr_payload_serialize() {
        let payload = PairingQrPayload {
            uid: "dev-123".to_string(),
            pairing_code: "654321".to_string(),
            endpoint: "ws://127.0.0.1:8080".to_string(),
        };
        let payload_str = payload.to_payload_string();
        assert!(payload_str.contains("dev-123"));
        assert!(payload_str.contains("654321"));
        assert!(payload_str.contains("ws://127.0.0.1:8080"));
    }

    #[test]
    fn test_render_qr_terminal() {
        let text = "blnk:pair:test";
        let rendered = render_qr_terminal(text).expect("render qr");
        assert!(!rendered.is_empty());
    }

    #[test]
    fn test_render_qr_ascii() {
        let text = "blnk:pair:test";
        let rendered = render_qr_ascii(text).expect("render ascii qr");
        assert!(!rendered.is_empty());
    }
}
