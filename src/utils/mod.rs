pub mod error;
pub mod qr;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub(crate) fn constant_time_string_eq(expected: &str, provided: &str) -> bool {
    let expected_digest = Sha256::digest(expected.as_bytes());
    let provided_digest = Sha256::digest(provided.as_bytes());
    expected_digest.ct_eq(&provided_digest).into()
}
