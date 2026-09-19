use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::utils::error::BlnkError;

pub const NONCE_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairCommit {
    #[serde(rename = "type")]
    pub message_type: String,
    pub commit: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairChallenge {
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(with = "base64_bytes")]
    pub nonce_d: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairReveal {
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(with = "base64_bytes")]
    pub nonce_c: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairCredentials {
    #[serde(rename = "type")]
    pub message_type: String,
    pub uid: String,
    pub public_key: String,
    pub access_code: String,
}

impl std::fmt::Debug for PairCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairCredentials")
            .field("message_type", &self.message_type)
            .field("uid", &self.uid)
            .field("public_key", &self.public_key)
            .field("access_code", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SasInput {
    pub nonce_c: Vec<u8>,
    pub nonce_d: Vec<u8>,
    pub local_fp: String,
    pub remote_fp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SasResult {
    pub sas: String,
}

impl PairCommit {
    pub fn from_nonce(nonce: &[u8]) -> Result<Self, BlnkError> {
        validate_nonce(nonce)?;
        Ok(Self {
            message_type: "pair_commit".to_owned(),
            commit: commitment_for(nonce),
        })
    }
}

impl PairChallenge {
    pub fn new() -> Result<Self, BlnkError> {
        Ok(Self {
            message_type: "pair_challenge".to_owned(),
            nonce_d: generate_nonce()?,
        })
    }
}

impl PairReveal {
    pub fn from_nonce(nonce: &[u8]) -> Result<Self, BlnkError> {
        validate_nonce(nonce)?;
        Ok(Self {
            message_type: "pair_reveal".to_owned(),
            nonce_c: nonce.to_vec(),
        })
    }
}

impl PairCredentials {
    pub fn new(uid: String, public_key: String, access_code: String) -> Self {
        Self {
            message_type: "pair_credentials".to_owned(),
            uid,
            public_key,
            access_code,
        }
    }
}

pub fn generate_nonce() -> Result<Vec<u8>, BlnkError> {
    let mut nonce = vec![0_u8; NONCE_LEN];
    getrandom::fill(&mut nonce)
        .map_err(|error| BlnkError::Protocol(format!("generate nonce: {error}")))?;
    Ok(nonce)
}

pub fn commitment_for(nonce: &[u8]) -> String {
    STANDARD.encode(Sha256::digest(nonce))
}

pub fn verify_commitment(commit: &str, nonce: &[u8]) -> Result<bool, BlnkError> {
    validate_nonce(nonce)?;
    let expected = commitment_for(nonce);
    Ok(expected.as_bytes().ct_eq(commit.as_bytes()).into())
}

pub fn compute_sas(input: &SasInput) -> Result<SasResult, BlnkError> {
    validate_nonce(&input.nonce_c)?;
    validate_nonce(&input.nonce_d)?;

    // Provisional deterministic construction until an interoperability fixture
    // fixes the exact upstream SAS encoding. Fingerprint order is canonicalized
    // so both peers derive the same value when they pass local/remote oppositely.
    let (first_fingerprint, second_fingerprint) = if input.local_fp <= input.remote_fp {
        (&input.local_fp, &input.remote_fp)
    } else {
        (&input.remote_fp, &input.local_fp)
    };
    let mut digest = Sha256::new();
    digest.update(&input.nonce_c);
    digest.update(&input.nonce_d);
    digest.update(first_fingerprint.as_bytes());
    digest.update(second_fingerprint.as_bytes());
    let digest = digest.finalize();
    let value = u64::from_le_bytes(
        digest[..8]
            .try_into()
            .map_err(|_| BlnkError::Protocol("SAS digest is too short".to_owned()))?,
    ) % 1_000_000;

    Ok(SasResult { sas: format!("{value:06}") })
}

fn validate_nonce(nonce: &[u8]) -> Result<(), BlnkError> {
    if nonce.len() != NONCE_LEN {
        return Err(BlnkError::Protocol(format!(
            "nonce must be {NONCE_LEN} bytes, got {}",
            nonce.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_reveal_round_trip_verifies() {
        let nonce = generate_nonce().expect("nonce generation should succeed");
        let commit = PairCommit::from_nonce(&nonce).expect("commit should validate");
        let reveal = PairReveal::from_nonce(&nonce).expect("reveal should validate");

        assert_eq!(commit.message_type, "pair_commit");
        assert_eq!(reveal.message_type, "pair_reveal");
        assert!(
            verify_commitment(&commit.commit, &reveal.nonce_c).expect("verification should run")
        );
    }

    #[test]
    fn commitment_rejects_tampered_nonce_and_invalid_length() {
        let nonce = generate_nonce().expect("nonce generation should succeed");
        let commit = PairCommit::from_nonce(&nonce).expect("commit should validate");
        let mut tampered = nonce.clone();
        tampered[0] ^= 1;

        assert!(!verify_commitment(&commit.commit, &tampered).expect("verification should run"));
        assert!(verify_commitment(&commit.commit, &[0_u8; 8]).is_err());
        assert!(PairCommit::from_nonce(&[0_u8; 8]).is_err());
        assert!(PairReveal::from_nonce(&[0_u8; 8]).is_err());
    }

    #[test]
    fn sas_is_six_digits_and_changes_with_nonce() {
        let nonce_c = vec![1_u8; NONCE_LEN];
        let nonce_d = vec![2_u8; NONCE_LEN];
        let input = SasInput {
            nonce_c: nonce_c.clone(),
            nonce_d: nonce_d.clone(),
            local_fp: "local".to_owned(),
            remote_fp: "remote".to_owned(),
        };
        let result = compute_sas(&input).expect("SAS should compute");
        let changed = compute_sas(&SasInput {
            nonce_c: vec![3_u8; NONCE_LEN],
            ..input.clone()
        })
        .expect("SAS should compute");
        let reversed_fingerprints = compute_sas(&SasInput {
            local_fp: "remote".to_owned(),
            remote_fp: "local".to_owned(),
            ..input
        })
        .expect("SAS should compute");

        assert_eq!(result.sas.len(), 6);
        assert!(
            result
                .sas
                .chars()
                .all(|character| character.is_ascii_digit())
        );
        assert_ne!(result.sas, changed.sas);
        assert_eq!(result.sas, reversed_fingerprints.sas);
    }

    #[test]
    fn json_pairing_messages_use_wire_names_and_unpadded_base64_nonces() {
        let challenge = PairChallenge {
            message_type: "pair_challenge".to_owned(),
            nonce_d: vec![0x01; NONCE_LEN],
        };
        let encoded = serde_json::to_value(&challenge).expect("challenge should serialize");
        assert_eq!(encoded["type"], "pair_challenge");
        assert_eq!(encoded["nonce_d"], STANDARD_NO_PAD.encode([0x01; NONCE_LEN]));
        assert!(
            !encoded["nonce_d"]
                .as_str()
                .expect("nonce_d should be a string")
                .contains('=')
        );
        assert!(encoded.get("message_type").is_none());

        let decoded: PairChallenge =
            serde_json::from_value(encoded).expect("challenge should deserialize");
        assert_eq!(decoded.nonce_d, challenge.nonce_d);
    }

    #[test]
    fn credentials_keep_access_code_separate() {
        let credentials = PairCredentials::new(
            "uid".to_owned(),
            "public-key".to_owned(),
            "access-code".to_owned(),
        );

        assert_eq!(credentials.message_type, "pair_credentials");
        assert_eq!(credentials.access_code, "access-code");
    }

    #[test]
    fn pair_credentials_debug_redacts_access_code() {
        let credentials = PairCredentials::new(
            "test_uid".to_owned(),
            "test_pub_key".to_owned(),
            "secret_access_code_123".to_owned(),
        );
        let debug_output = format!("{credentials:?}");
        assert!(!debug_output.contains("secret_access_code_123"));
        assert!(debug_output.contains("<redacted>"));
    }
}

mod base64_bytes {
    use super::*;

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD_NO_PAD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        STANDARD_NO_PAD.decode(encoded).map_err(DeError::custom)
    }
}
