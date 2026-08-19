use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::rand_core::OsRng;
use rsa::traits::PublicKeyParts;
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::utils::error::BlnkError;

const RSA_BITS: usize = 2048;
const PAIRING_CODE_SPACE: u32 = 1_000_000;
const ACCESS_CODE_BYTES: usize = 8;
const UID_BYTES: usize = 16;

pub struct Identity {
    private_key: RsaPrivateKey,
    uid: String,
    pairing_code: String,
    access_code: String,
}

#[derive(Serialize, Deserialize)]
struct PersistedIdentity {
    private_key_pem: String,
    uid: String,
    pairing_code: String,
    access_code: String,
}

impl Identity {
    pub fn generate() -> Result<Self, BlnkError> {
        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, RSA_BITS)
            .map_err(|error| BlnkError::Identity(format!("generate RSA key: {error}")))?;
        let uid = generate_uid()?;
        let pairing_code = generate_pairing_code()?;
        let access_code = generate_access_code()?;

        Ok(Self {
            private_key,
            uid,
            pairing_code,
            access_code,
        })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, BlnkError> {
        let contents = fs::read_to_string(path)?;
        let persisted: PersistedIdentity = serde_json::from_str(&contents)
            .map_err(|error| BlnkError::Identity(format!("decode identity file: {error}")))?;
        let private_key = RsaPrivateKey::from_pkcs8_pem(&persisted.private_key_pem)
            .map_err(|error| BlnkError::Identity(format!("decode private key: {error}")))?;

        if private_key.size() * 8 != RSA_BITS {
            return Err(BlnkError::Identity(format!(
                "identity file contains an RSA key with {} bits; expected {RSA_BITS}",
                private_key.size() * 8
            )));
        }

        if !is_valid_uid(&persisted.uid) {
            return Err(BlnkError::Identity(
                "identity file contains an invalid uid".to_owned(),
            ));
        }

        if persisted.pairing_code.len() != 6
            || !persisted
                .pairing_code
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            return Err(BlnkError::Identity(
                "identity file contains an invalid pairing_code".to_owned(),
            ));
        }

        if !is_valid_access_code(&persisted.access_code) {
            return Err(BlnkError::Identity(
                "identity file contains an invalid access_code".to_owned(),
            ));
        }

        Ok(Self {
            private_key,
            uid: persisted.uid,
            pairing_code: persisted.pairing_code,
            access_code: persisted.access_code,
        })
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), BlnkError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            ensure_private_parent(parent)?;
        }

        let private_key_pem = self
            .private_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|error| BlnkError::Identity(format!("encode private key: {error}")))?
            .to_string();
        let payload = serde_json::to_vec_pretty(&PersistedIdentity {
            private_key_pem,
            uid: self.uid.clone(),
            pairing_code: self.pairing_code.clone(),
            access_code: self.access_code.clone(),
        })
        .map_err(|error| BlnkError::Identity(format!("encode identity file: {error}")))?;

        let temporary_path = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
        let write_result = (|| -> Result<(), BlnkError> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary_path)?;
            set_private_permissions(&file)?;
            file.write_all(&payload)?;
            file.sync_all()?;
            replace_file(&temporary_path, path)?;
            Ok(())
        })();

        if write_result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }

        write_result
    }

    pub fn uid(&self) -> &str {
        &self.uid
    }

    pub fn pairing_code(&self) -> &str {
        &self.pairing_code
    }

    pub fn access_code(&self) -> &str {
        &self.access_code
    }

    pub fn public_key(&self) -> RsaPublicKey {
        self.private_key.to_public_key()
    }

    pub fn public_key_b64(&self) -> Result<String, BlnkError> {
        let der = self
            .public_key()
            .to_public_key_der()
            .map_err(|error| BlnkError::Identity(format!("encode public key: {error}")))?;
        Ok(STANDARD.encode(der.as_ref()))
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, BlnkError> {
        let digest = Sha256::digest(data);
        self.private_key
            .sign(Pkcs1v15Sign::new::<Sha256>(), &digest)
            .map_err(|error| BlnkError::Identity(format!("sign data: {error}")))
    }

    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<(), BlnkError> {
        let digest = Sha256::digest(data);
        self.public_key()
            .verify(Pkcs1v15Sign::new::<Sha256>(), &digest, signature)
            .map_err(|error| BlnkError::Identity(format!("verify signature: {error}")))
    }

    pub fn encrypt_for_peer(
        public_key: &RsaPublicKey,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, BlnkError> {
        let mut rng = OsRng;
        public_key
            .encrypt(&mut rng, Oaep::new::<Sha256>(), plaintext)
            .map_err(|error| BlnkError::Identity(format!("encrypt data: {error}")))
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, BlnkError> {
        self.private_key
            .decrypt(Oaep::new::<Sha256>(), ciphertext)
            .map_err(|error| BlnkError::Identity(format!("decrypt data: {error}")))
    }
}

fn generate_uid() -> Result<String, BlnkError> {
    let mut bytes = [0_u8; UID_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| BlnkError::Identity(format!("generate uid: {error}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn generate_pairing_code() -> Result<String, BlnkError> {
    let mut bytes = [0_u8; 4];
    getrandom::fill(&mut bytes)
        .map_err(|error| BlnkError::Identity(format!("generate pairing_code: {error}")))?;
    let value = u32::from_le_bytes(bytes) % PAIRING_CODE_SPACE;
    Ok(format!("{value:06}"))
}

fn generate_access_code() -> Result<String, BlnkError> {
    let mut bytes = [0_u8; ACCESS_CODE_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| BlnkError::Identity(format!("generate access_code: {error}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn is_valid_uid(value: &str) -> bool {
    value.len() == 22
        && URL_SAFE_NO_PAD
            .decode(value)
            .map(|bytes| bytes.len() == UID_BYTES)
            .unwrap_or(false)
}

fn is_valid_access_code(value: &str) -> bool {
    value.len() == 11
        && URL_SAFE_NO_PAD
            .decode(value)
            .map(|bytes| bytes.len() == ACCESS_CODE_BYTES)
            .unwrap_or(false)
}

fn ensure_private_parent(parent: &Path) -> Result<(), BlnkError> {
    let mut missing = Vec::new();
    let mut current = Some(parent);

    while let Some(candidate) = current {
        match fs::metadata(candidate) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err(BlnkError::Identity(format!(
                        "identity parent is not a directory: {}",
                        candidate.display()
                    )));
                }
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing.push(candidate.to_path_buf());
                current = candidate.parent();
            }
            Err(error) => return Err(error.into()),
        }
    }

    for directory in missing.iter().rev() {
        fs::create_dir(directory)?;
        set_private_directory_permissions(directory)?;
    }

    Ok(())
}

fn set_private_directory_permissions(path: &Path) -> Result<(), BlnkError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }

    #[cfg(windows)]
    {
        // Windows inherits the parent ACL for newly-created directories. The
        // caller must keep the identity root under a user-private location.
        let _ = path;
    }

    Ok(())
}

fn set_private_permissions(file: &std::fs::File) -> Result<(), BlnkError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

#[cfg(not(windows))]
fn replace_file(temporary_path: &Path, path: &Path) -> io::Result<()> {
    fs::rename(temporary_path, path)
}

#[cfg(windows)]
fn replace_file(temporary_path: &Path, path: &Path) -> io::Result<()> {
    use winapi::um::winbase::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW};

    let temporary_wide: Vec<u16> = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            temporary_wide.as_ptr(),
            path_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_identity_path() -> (PathBuf, PathBuf) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("blnk-identity-{suffix}"));
        (root.clone(), root.join("nested").join("identity.json"))
    }

    fn write_persisted(path: &Path, identity: &Identity, access_code: &str) {
        let private_key_pem = identity
            .private_key
            .to_pkcs8_pem(LineEnding::LF)
            .expect("private key should encode")
            .to_string();
        let payload = serde_json::to_vec(&PersistedIdentity {
            private_key_pem,
            uid: identity.uid.clone(),
            pairing_code: identity.pairing_code.clone(),
            access_code: access_code.to_owned(),
        })
        .expect("identity fixture should encode");
        fs::create_dir_all(path.parent().expect("fixture has parent"))
            .expect("fixture directory should be created");
        fs::write(path, payload).expect("identity fixture should be written");
    }

    #[test]
    fn generated_identity_has_expected_public_fields() {
        let identity = Identity::generate().expect("identity generation should succeed");

        assert_eq!(identity.uid().len(), 22);
        assert!(
            identity
                .uid()
                .chars()
                .all(|character| character.is_ascii_alphanumeric()
                    || character == '-'
                    || character == '_')
        );
        assert_eq!(identity.pairing_code().len(), 6);
        assert!(
            identity
                .pairing_code()
                .chars()
                .all(|character| character.is_ascii_digit())
        );
        assert_eq!(identity.access_code().len(), 11);
        assert!(identity.public_key_b64().is_ok());
    }

    #[test]
    fn identity_round_trips_through_atomic_persistence() {
        let (root, path) = temporary_identity_path();
        let identity = Identity::generate().expect("identity generation should succeed");
        identity.save(&path).expect("identity should save");
        let loaded = Identity::load(&path).expect("identity should load");

        assert_eq!(identity.uid(), loaded.uid());
        assert_eq!(identity.pairing_code(), loaded.pairing_code());
        assert_eq!(identity.access_code(), loaded.access_code());
        assert_eq!(identity.public_key_b64().ok(), loaded.public_key_b64().ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn identity_load_rejects_invalid_access_code() {
        let (root, path) = temporary_identity_path();
        let identity = Identity::generate().expect("identity generation should succeed");
        write_persisted(&path, &identity, "not-valid");

        assert!(Identity::load(&path).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn identity_load_rejects_non_2048_bit_keys() {
        let (root, path) = temporary_identity_path();
        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 1024).expect("test key should be generated");
        let identity = Identity {
            private_key,
            uid: generate_uid().expect("uid should be generated"),
            pairing_code: "123456".to_owned(),
            access_code: generate_access_code().expect("access code should be generated"),
        };
        write_persisted(&path, &identity, identity.access_code());

        assert!(Identity::load(&path).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn identity_persistence_uses_private_modes() {
        use std::os::unix::fs::PermissionsExt;

        let (root, path) = temporary_identity_path();
        let identity = Identity::generate().expect("identity generation should succeed");
        identity.save(&path).expect("identity should save");

        let directory_mode = fs::metadata(path.parent().expect("identity has parent"))
            .expect("identity parent should exist")
            .permissions()
            .mode()
            & 0o777;
        let file_mode = fs::metadata(&path)
            .expect("identity file should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(directory_mode, 0o700);
        assert_eq!(file_mode, 0o600);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn identity_signs_and_verifies_data() {
        let identity = Identity::generate().expect("identity generation should succeed");
        let data = b"blnk identity test";
        let signature = identity.sign(data).expect("signing should succeed");

        identity
            .verify(data, &signature)
            .expect("verification should succeed");
        assert!(identity.verify(b"tampered", &signature).is_err());
    }

    #[test]
    fn identity_encrypts_and_decrypts_data() {
        let identity = Identity::generate().expect("identity generation should succeed");
        let plaintext = b"credential exchange";
        let ciphertext = Identity::encrypt_for_peer(&identity.public_key(), plaintext)
            .expect("encryption should succeed");

        assert_eq!(
            identity
                .decrypt(&ciphertext)
                .expect("decryption should succeed"),
            plaintext
        );
    }
}
