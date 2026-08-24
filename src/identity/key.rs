use std::fmt::Write as _;
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

/// Selects the on-disk representation for an identity file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityFormat {
    Json,
    Pem,
}

impl Identity {
    pub fn generate() -> Result<Self, BlnkError> {
        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, RSA_BITS)
            .map_err(|error| BlnkError::Identity(format!("generate RSA key: {error}")))?;
        let uid = derive_uid(&private_key)?;
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

    /// Loads either the existing JSON representation or the upstream two-block PEM representation.
    ///
    /// This method only detects the representation; it does not rewrite or migrate the file.
    pub fn load_auto(path: impl AsRef<Path>) -> Result<Self, BlnkError> {
        let path = path.as_ref();
        let contents = fs::read(path)?;
        if looks_like_pem(&contents) {
            Self::load_pem(&contents)
        } else {
            Self::load(path)
        }
    }

    /// Loads the two-block PEM representation used by the upstream BitBang client.
    ///
    /// This explicit boundary is intentionally separate from [`Self::load`]. The
    /// default JSON persistence remains unchanged; PEM persistence is opt-in and
    /// no file is silently migrated.
    pub fn load_pem(contents: &[u8]) -> Result<Self, BlnkError> {
        let text = std::str::from_utf8(contents)
            .map_err(|error| BlnkError::Identity(format!("decode identity PEM: {error}")))?;
        let private_key_der = decode_pem_block(text, "PRIVATE KEY")?;
        let access_code_bytes = decode_pem_block(text, "BITBANG ACCESS CODE")?;
        if access_code_bytes.len() != ACCESS_CODE_BYTES {
            return Err(BlnkError::Identity(format!(
                "access-code block has wrong length: {} (want {ACCESS_CODE_BYTES})",
                access_code_bytes.len()
            )));
        }

        let private_key = RsaPrivateKey::from_pkcs8_der(&private_key_der)
            .map_err(|error| BlnkError::Identity(format!("decode private key: {error}")))?;
        validate_rsa_key_size(&private_key)?;
        let uid = derive_uid(&private_key)?;
        let access_code = URL_SAFE_NO_PAD.encode(access_code_bytes);

        Ok(Self {
            private_key,
            uid,
            pairing_code: generate_pairing_code()?,
            access_code,
        })
    }

    /// Encodes the upstream-compatible two-block PEM representation.
    pub fn save_pem(&self) -> Result<Vec<u8>, BlnkError> {
        validate_rsa_key_size(&self.private_key)?;
        let derived_uid = derive_uid(&self.private_key)?;
        if self.uid != derived_uid {
            return Err(BlnkError::Identity(
                "cannot export a legacy identity to PEM because its UID is not key-derived"
                    .to_owned(),
            ));
        }

        let private_key_der = self
            .private_key
            .to_pkcs8_der()
            .map_err(|error| BlnkError::Identity(format!("encode private key: {error}")))?;
        let access_code_bytes = URL_SAFE_NO_PAD
            .decode(self.access_code.as_bytes())
            .map_err(|error| BlnkError::Identity(format!("decode access code: {error}")))?;
        if access_code_bytes.len() != ACCESS_CODE_BYTES {
            return Err(BlnkError::Identity(format!(
                "access code has wrong length: {} (want {ACCESS_CODE_BYTES})",
                access_code_bytes.len()
            )));
        }

        let mut output = Vec::new();
        output.extend_from_slice(
            encode_pem_block("PRIVATE KEY", private_key_der.as_bytes()).as_bytes(),
        );
        output.extend_from_slice(
            encode_pem_block("BITBANG ACCESS CODE", &access_code_bytes).as_bytes(),
        );
        Ok(output)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), BlnkError> {
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

        write_atomic(path.as_ref(), &payload)
    }

    /// Saves the identity using an explicit JSON or upstream-compatible PEM representation.
    pub fn save_as(&self, path: impl AsRef<Path>, format: IdentityFormat) -> Result<(), BlnkError> {
        match format {
            IdentityFormat::Json => self.save(path),
            IdentityFormat::Pem => self.save_pem_file(path),
        }
    }

    /// Saves the upstream-compatible two-block PEM representation atomically to a file.
    pub fn save_pem_file(&self, path: impl AsRef<Path>) -> Result<(), BlnkError> {
        let payload = self.save_pem()?;
        write_atomic(path.as_ref(), &payload)
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

#[cfg(test)]
fn generate_uid() -> Result<String, BlnkError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| BlnkError::Identity(format!("generate uid: {error}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn derive_uid(private_key: &RsaPrivateKey) -> Result<String, BlnkError> {
    let public_key_der = private_key
        .to_public_key()
        .to_public_key_der()
        .map_err(|error| BlnkError::Identity(format!("encode public key: {error}")))?;
    let digest = Sha256::digest(public_key_der.as_ref());
    Ok(URL_SAFE_NO_PAD.encode(&digest[..16]))
}

fn validate_rsa_key_size(private_key: &RsaPrivateKey) -> Result<(), BlnkError> {
    if private_key.n().bits() != RSA_BITS {
        return Err(BlnkError::Identity(format!(
            "identity key must be exactly {RSA_BITS} bits"
        )));
    }
    Ok(())
}

fn looks_like_pem(contents: &[u8]) -> bool {
    std::str::from_utf8(contents)
        .map(|text| text.trim_start().starts_with("-----BEGIN "))
        .unwrap_or(false)
}

fn write_atomic(path: &Path, payload: &[u8]) -> Result<(), BlnkError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        ensure_private_parent(parent)?;
    }

    let temporary_path = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
    let write_result = (|| -> Result<(), BlnkError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        set_private_permissions(&file)?;
        file.write_all(payload)?;
        file.sync_all()?;
        replace_file(&temporary_path, path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }

    write_result
}

fn decode_pem_block(contents: &str, block_type: &str) -> Result<Vec<u8>, BlnkError> {
    let begin = format!("-----BEGIN {block_type}-----");
    let end = format!("-----END {block_type}-----");
    let begin_index = contents.find(&begin).ok_or_else(|| {
        BlnkError::Identity(format!("identity PEM is missing {block_type} block"))
    })?;
    let body_start = begin_index + begin.len();
    let end_index = contents[body_start..]
        .find(&end)
        .map(|index| body_start + index)
        .ok_or_else(|| {
            BlnkError::Identity(format!("identity PEM has unterminated {block_type} block"))
        })?;
    let encoded: String = contents[body_start..end_index]
        .lines()
        .map(str::trim)
        .collect();
    STANDARD
        .decode(encoded.as_bytes())
        .map_err(|error| BlnkError::Identity(format!("decode {block_type} block: {error}")))
}

fn encode_pem_block(block_type: &str, bytes: &[u8]) -> String {
    let encoded = STANDARD.encode(bytes);
    let mut output = format!("-----BEGIN {block_type}-----\n");
    for chunk in encoded.as_bytes().chunks(64) {
        output.push_str(std::str::from_utf8(chunk).expect("base64 output is ASCII"));
        output.push('\n');
    }
    let _ = writeln!(output, "-----END {block_type}-----");
    output
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
        if candidate.as_os_str().is_empty() {
            break;
        }

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
                current = candidate
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty());
            }
            Err(error) => return Err(error.into()),
        }
    }

    for directory in missing.iter().rev() {
        match fs::create_dir(directory) {
            Ok(()) => set_private_directory_permissions(directory)?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let metadata = fs::metadata(directory)?;
                if !metadata.is_dir() {
                    return Err(BlnkError::Identity(format!(
                        "identity parent is not a directory: {}",
                        directory.display()
                    )));
                }
                set_private_directory_permissions(directory)?;
            }
            Err(error) => return Err(error.into()),
        }
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

fn set_private_permissions(file: &fs::File) -> Result<(), BlnkError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }

    #[cfg(not(unix))]
    {
        let _ = file;
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

    #[test]
    fn identity_save_pem_rejects_non_2048_bit_keys() {
        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 1024).expect("test key should be generated");
        let identity = Identity {
            private_key,
            uid: "2iuGA9MzJw9GJY35ilAiHA".to_owned(),
            pairing_code: "123456".to_owned(),
            access_code: "ASNFZ4mrze8".to_owned(),
        };

        assert!(identity.save_pem().is_err());
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
    fn relative_identity_parent_hierarchy_is_created() {
        let relative_root = PathBuf::from(format!(
            "blnk-identity-relative-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let relative_parent = relative_root.join("nested");

        ensure_private_parent(&relative_parent).expect("relative parents should be created");
        assert!(relative_parent.is_dir());
        let _ = fs::remove_dir_all(relative_root);
    }

    #[test]
    fn concurrent_identity_parent_creation_is_idempotent() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let root = std::env::temp_dir().join(format!(
            "blnk-identity-concurrent-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let parent = Arc::new(root.join("nested"));
        let barrier = Arc::new(Barrier::new(8));
        let workers = (0..8)
            .map(|_| {
                let parent = Arc::clone(&parent);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    ensure_private_parent(&parent)
                })
            })
            .collect::<Vec<_>>();

        for worker in workers {
            worker
                .join()
                .expect("directory worker should not panic")
                .expect("concurrent directory creation should succeed");
        }
        assert!(parent.is_dir());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn upstream_pem_fixture_loads_with_derived_identity() {
        let fixture = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/bitbang_identity.pem"
        ));
        let identity = Identity::load_pem(fixture).expect("upstream PEM fixture should load");

        assert_eq!(identity.access_code(), "ASNFZ4mrze8");
        assert_eq!(identity.uid(), "2iuGA9MzJw9GJY35ilAiHA");
        assert_eq!(identity.uid().len(), 22);
        assert_eq!(identity.pairing_code().len(), 6);
        assert!(identity.public_key_b64().is_ok());
    }

    #[test]
    fn upstream_pem_round_trips_without_losing_access_code() {
        let fixture = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/bitbang_identity.pem"
        ));
        let identity = Identity::load_pem(fixture).expect("upstream PEM fixture should load");
        let encoded = identity.save_pem().expect("identity should encode as PEM");
        let reloaded = Identity::load_pem(&encoded).expect("encoded PEM should reload");

        assert_eq!(identity.access_code(), reloaded.access_code());
        assert_eq!(identity.uid(), reloaded.uid());
        assert_eq!(
            identity.public_key_b64().ok(),
            reloaded.public_key_b64().ok()
        );
        assert!(encoded.starts_with(b"-----BEGIN PRIVATE KEY-----\n"));
        assert!(
            encoded
                .windows(b"-----BEGIN BITBANG ACCESS CODE-----".len())
                .any(|window| { window == b"-----BEGIN BITBANG ACCESS CODE-----" })
        );
    }

    #[test]
    fn identity_load_auto_detects_json_and_pem() {
        let (root, json_path) = temporary_identity_path();
        let identity = Identity::generate().expect("identity generation should succeed");
        identity
            .save_as(&json_path, IdentityFormat::Json)
            .expect("JSON identity should save");
        let json_loaded = Identity::load_auto(&json_path).expect("JSON identity should load");
        assert_eq!(identity.uid(), json_loaded.uid());
        assert_eq!(identity.pairing_code(), json_loaded.pairing_code());
        assert_eq!(identity.access_code(), json_loaded.access_code());

        let pem_path = root.join("nested").join("identity.pem");
        identity
            .save_as(&pem_path, IdentityFormat::Pem)
            .expect("PEM identity should save");
        let pem_loaded = Identity::load_auto(&pem_path).expect("PEM identity should load");
        let derived_uid = derive_uid(&identity.private_key).expect("UID should derive");
        assert_eq!(derived_uid, pem_loaded.uid());
        assert_eq!(identity.access_code(), pem_loaded.access_code());
        assert_eq!(
            identity.public_key_b64().ok(),
            pem_loaded.public_key_b64().ok()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn upstream_pem_rejects_wrong_access_code_length() {
        let fixture = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/bitbang_identity.pem"
        ));
        let fixture = String::from_utf8(fixture.to_vec()).expect("fixture should be UTF-8");
        let invalid = fixture.replace("ASNFZ4mrze8=", "AQ==");

        let error = match Identity::load_pem(invalid.as_bytes()) {
            Ok(_) => panic!("short code must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("wrong length"));
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
