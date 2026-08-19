use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::utils::error::BlnkError;

pub const NONCE_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairCommit {
    pub message_type: String,
    pub commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairChallenge {
    pub message_type: String,
    pub nonce_d: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairReveal {
    pub message_type: String,
    pub nonce_c: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairCredentials {
    pub message_type: String,
    pub uid: String,
    pub public_key: String,
    pub access_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn from_nonce(nonce: &[u8]) -> Self {
        Self {
            message_type: "pair_commit".to_owned(),
            commit: commitment_for(nonce),
        }
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
    // fixes the exact upstream SAS encoding. The input order follows the schema.
    let mut digest = Sha256::new();
    digest.update(&input.nonce_c);
    digest.update(&input.nonce_d);
    digest.update(input.local_fp.as_bytes());
    digest.update(input.remote_fp.as_bytes());
    let digest = digest.finalize();
    let value = u64::from_le_bytes(
        digest[..8]
            .try_into()
            .map_err(|_| BlnkError::Protocol("SAS digest is too short".to_owned()))?,
    ) % 1_000_000;

    Ok(SasResult {
        sas: format!("{value:06}"),
    })
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
        let commit = PairCommit::from_nonce(&nonce);
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
        let commit = PairCommit::from_nonce(&nonce);
        let mut tampered = nonce.clone();
        tampered[0] ^= 1;

        assert!(!verify_commitment(&commit.commit, &tampered).expect("verification should run"));
        assert!(verify_commitment(&commit.commit, &[0_u8; 8]).is_err());
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
}
