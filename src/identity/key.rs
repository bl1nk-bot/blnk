use getrandom::fill;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub uid: String,
    pub pairing_code: String,
}

impl Identity {
    pub fn generate() -> Self {
        let uid = uuid::Uuid::new_v4().to_string();
        let pairing_code = generate_pairing_code();

        Self { uid, pairing_code }
    }
}

fn generate_pairing_code() -> String {
    let mut bytes = [0_u8; 4];
    fill(&mut bytes).expect("secure random source unavailable");
    let value = u32::from_le_bytes(bytes) % 1_000_000;
    format!("{value:06}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_identity_has_uid_and_six_digit_pairing_code() {
        let identity = Identity::generate();

        assert!(!identity.uid.is_empty());
        assert_eq!(identity.pairing_code.len(), 6);
        assert!(
            identity
                .pairing_code
                .chars()
                .all(|character| character.is_ascii_digit())
        );
    }
}
