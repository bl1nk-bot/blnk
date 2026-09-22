//! Encrypted payload vault for sensitive Object Model content.
//!
//! The first implementation uses a caller-provided 32-byte root key. A later
//! platform adapter can unwrap that key from Windows Credential Manager or the
//! Linux Secret Service without changing the vault record format.
// TODO: Replace the caller-provided root key with a platform key-provider boundary
// and preserve this record format across Windows/Linux implementations.

use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use chacha20poly1305::{
    AeadInPlace, KeyInit, XChaCha20Poly1305, XNonce, aead::generic_array::GenericArray,
};
use getrandom::fill as random_fill;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::proto_generated::vault::{GetPayloadResponse, PutPayloadRequest, VaultRecord};

const KEY_VERSION: u32 = 1;
const ALGORITHM: &str = "XChaCha20-Poly1305";
const NONCE_LEN: usize = 24;

pub struct EncryptedVault {
    conn: Mutex<Connection>,
    root_key: Zeroizing<[u8; 32]>,
}

impl EncryptedVault {
    pub fn open(path: impl AsRef<std::path::Path>, root_key: [u8; 32]) -> Result<Self> {
        let conn = Connection::open(path).context("open vault database")?;
        initialize(&conn)?;
        let vault = Self {
            conn: Mutex::new(conn),
            root_key: Zeroizing::new(root_key),
        };
        vault.ensure_key_version()?;
        Ok(vault)
    }

    pub fn open_in_memory(root_key: [u8; 32]) -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory vault")?;
        initialize(&conn)?;
        let vault = Self {
            conn: Mutex::new(conn),
            root_key: Zeroizing::new(root_key),
        };
        vault.ensure_key_version()?;
        Ok(vault)
    }

    pub fn put(&self, request: &PutPayloadRequest) -> Result<VaultRecord> {
        if request.object_id.is_empty() || request.plaintext.is_empty() {
            return Err(anyhow!("object_id and plaintext are required"));
        }
        let payload_ref = if request.payload_ref.is_empty() {
            format!("vault:{}-r{}", request.object_id, request.revision)
        } else {
            request.payload_ref.clone()
        };
        let nonce = random_nonce()?;
        let aad = aad(
            &payload_ref,
            &request.object_id,
            request.revision,
            &request.media_type,
        );
        let mut ciphertext = request.plaintext.clone();
        let cipher = XChaCha20Poly1305::new(GenericArray::from_slice(self.root_key.as_ref()));
        cipher
            .encrypt_in_place(XNonce::from_slice(&nonce), &aad, &mut ciphertext)
            .map_err(|_| anyhow!("vault encryption failed"))?;
        let payload_hash = digest(&request.plaintext);
        let now = unix_now();
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow!("vault mutex poisoned"))?;
        conn.execute(
            "INSERT INTO vault_records
             (payload_ref, object_id, revision, key_version, storage_state, algorithm,
              nonce, ciphertext, aad_hash, payload_hash, payload_size, media_type, created_at)
             VALUES (?1, ?2, ?3, ?4, 'committed', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(payload_ref) DO UPDATE SET
              ciphertext=excluded.ciphertext, nonce=excluded.nonce, payload_hash=excluded.payload_hash,
              payload_size=excluded.payload_size, media_type=excluded.media_type, storage_state='committed'",
            params![payload_ref, request.object_id, request.revision, KEY_VERSION, ALGORITHM, nonce, ciphertext, digest(&aad), payload_hash, request.plaintext.len() as i64, request.media_type, now],
        )?;
        Ok(VaultRecord {
            payload_ref,
            object_id: request.object_id.clone(),
            revision: request.revision,
            key_version: KEY_VERSION,
            storage_state: "committed".into(),
            algorithm: ALGORITHM.into(),
            nonce,
            ciphertext,
            aad_hash: digest(&aad),
            payload_hash,
            payload_size: request.plaintext.len() as u64,
            created_at: now,
            media_type: request.media_type.clone(),
            ..Default::default()
        })
    }

    pub fn get(
        &self,
        request: &crate::proto_generated::vault::GetPayloadRequest,
    ) -> Result<GetPayloadResponse> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow!("vault mutex poisoned"))?;
        let row = conn.query_row(
            "SELECT object_id, revision, algorithm, nonce, ciphertext, aad_hash, payload_hash, media_type
             FROM vault_records WHERE payload_ref = ?1 AND storage_state = 'committed' AND deleted_at IS NULL",
            [&request.payload_ref],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?, row.get::<_, Vec<u8>>(3)?, row.get::<_, Vec<u8>>(4)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?, row.get::<_, String>(7)?)),
        ).optional()?.context("vault payload not found")?;
        let (
            object_id,
            revision,
            algorithm,
            nonce,
            mut ciphertext,
            aad_hash,
            expected_hash,
            media_type,
        ) = row;
        if algorithm != ALGORITHM || nonce.len() != NONCE_LEN {
            return Err(anyhow!("unsupported vault record algorithm or nonce"));
        }
        let aad = aad(
            &request.payload_ref,
            &object_id,
            revision as u64,
            &media_type,
        );
        if digest(&aad) != aad_hash {
            return Err(anyhow!("vault AAD integrity check failed"));
        }
        let cipher = XChaCha20Poly1305::new(GenericArray::from_slice(self.root_key.as_ref()));
        cipher
            .decrypt_in_place(XNonce::from_slice(&nonce), &aad, &mut ciphertext)
            .map_err(|_| anyhow!("vault authentication failed"))?;
        if digest(&ciphertext) != expected_hash {
            return Err(anyhow!("vault payload hash mismatch"));
        }
        Ok(GetPayloadResponse {
            plaintext: ciphertext,
            media_type,
        })
    }

    pub fn delete(&self, payload_ref: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow!("vault mutex poisoned"))?;
        conn.execute(
            "UPDATE vault_records SET storage_state='deleting', deleted_at=?2 WHERE payload_ref=?1",
            params![payload_ref, unix_now()],
        )?;
        Ok(())
    }

    pub fn inspect(&self, payload_ref: &str) -> Result<Option<VaultRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow!("vault mutex poisoned"))?;
        Ok(conn.query_row(
            "SELECT object_id, revision, key_version, storage_state, algorithm, nonce, ciphertext,
                    aad_hash, payload_hash, payload_size, created_at, media_type
             FROM vault_records WHERE payload_ref=?1",
            [payload_ref],
            |row| Ok(VaultRecord { payload_ref: payload_ref.to_owned(), object_id: row.get(0)?, revision: row.get::<_, i64>(1)? as u64, key_version: row.get::<_, i64>(2)? as u32, storage_state: row.get(3)?, algorithm: row.get(4)?, nonce: row.get(5)?, ciphertext: row.get(6)?, aad_hash: row.get(7)?, payload_hash: row.get(8)?, payload_size: row.get::<_, i64>(9)? as u64, created_at: row.get(10)?, media_type: row.get(11)?, ..Default::default() }),
        ).optional()?)
    }

    fn ensure_key_version(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow!("vault mutex poisoned"))?;
        conn.execute("INSERT OR IGNORE INTO vault_keys(version, key_ref, algorithm, status, created_at) VALUES (?1, 'runtime-root-key', ?2, 'active', ?3)", params![KEY_VERSION, ALGORITHM, unix_now()])?;
        Ok(())
    }
}

fn initialize(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA secure_delete=ON;
        CREATE TABLE IF NOT EXISTS vault_keys(version INTEGER PRIMARY KEY, key_ref TEXT NOT NULL, algorithm TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL, retired_at INTEGER);
        CREATE TABLE IF NOT EXISTS vault_records(payload_ref TEXT PRIMARY KEY, object_id TEXT NOT NULL, revision INTEGER NOT NULL, key_version INTEGER NOT NULL, storage_state TEXT NOT NULL DEFAULT 'staged', algorithm TEXT NOT NULL, nonce BLOB NOT NULL, ciphertext BLOB NOT NULL, aad_hash TEXT NOT NULL, payload_hash TEXT NOT NULL, payload_size INTEGER NOT NULL, media_type TEXT NOT NULL DEFAULT 'application/octet-stream', created_at INTEGER NOT NULL, deleted_at INTEGER, FOREIGN KEY(key_version) REFERENCES vault_keys(version));")?;
    Ok(())
}

fn random_nonce() -> Result<Vec<u8>> {
    let mut nonce = vec![0u8; NONCE_LEN];
    random_fill(&mut nonce).map_err(|e| anyhow!("generate vault nonce: {e}"))?;
    Ok(nonce)
}
fn aad(payload_ref: &str, object_id: &str, revision: u64, media_type: &str) -> Vec<u8> {
    format!("blnk/vault/v1\0{payload_ref}\0{object_id}\0{revision}\0{media_type}").into_bytes()
}
fn digest(value: &[u8]) -> String {
    hex_encode(Sha256::digest(value).as_slice())
}
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypts_round_trip_and_rejects_tampering() {
        let vault = EncryptedVault::open_in_memory([7u8; 32]).unwrap();
        let request = PutPayloadRequest {
            payload_ref: "vault:o1-r1".into(),
            object_id: "o1".into(),
            revision: 1,
            media_type: "application/json".into(),
            plaintext: br#"{"token":"secret"}"#.to_vec(),
        };
        let record = vault.put(&request).unwrap();
        assert_eq!(record.storage_state, "committed");
        let response = vault
            .get(&crate::proto_generated::vault::GetPayloadRequest {
                payload_ref: record.payload_ref.clone(),
                purpose: "Apply".into(),
            })
            .unwrap();
        assert_eq!(response.plaintext, request.plaintext);
        let conn = vault.conn.lock().unwrap();
        conn.execute("UPDATE vault_records SET ciphertext = zeroblob(length(ciphertext)) WHERE payload_ref=?1", [&record.payload_ref]).unwrap();
        drop(conn);
        assert!(
            vault
                .get(&crate::proto_generated::vault::GetPayloadRequest {
                    payload_ref: record.payload_ref,
                    purpose: "Apply".into()
                })
                .is_err()
        );
    }
}
