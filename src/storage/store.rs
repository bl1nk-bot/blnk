//! SQLite metadata store for the universal object model.

use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};

use crate::proto_generated::object::{ObjectKind, ObjectMetadata, ObjectRecord};
use crate::proto_generated::storage::AppBinding;

use super::schema::create_tables;

/// SQLite-backed metadata store. Payload bytes never enter this store.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Open or create a database at `path` and apply the current schema.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let conn = Connection::open(path).context("open metadata sqlite database")?;
        create_tables(&conn).context("create metadata schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an isolated in-memory database for tests and short-lived sessions.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory metadata database")?;
        create_tables(&conn).context("create metadata schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn schema_version(&self) -> Result<i64> {
        let conn = self.lock()?;
        Ok(crate::storage::schema::schema_version(&conn)?)
    }

    /// Insert a new object and its first immutable revision.
    pub fn insert_object(&self, object: &ObjectRecord, change_type: &str) -> Result<()> {
        validate_object(object)?;
        let metadata = object
            .metadata
            .as_ref()
            .context("object metadata is required")?;
        let provenance = metadata
            .provenance
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or_else(|| "{}".to_owned());
        let now = object.updated_at.max(object.created_at);
        let conn = self.lock()?;
        let tx = conn
            .unchecked_transaction()
            .context("begin object transaction")?;
        tx.execute(
            "INSERT INTO objects
             (id, kind, schema_version, title, description, sensitivity, status,
              workspace_id, payload_ref, payload_hash, payload_size, payload_media_type,
              current_revision, storage_state, provenance_type, provenance_json,
              created_by_device, created_at, updated_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                object.id,
                object_kind_name(object.kind),
                object.schema_version,
                metadata.title,
                metadata.description,
                sensitivity_name(metadata.sensitivity),
                status_name(object.status),
                empty_to_none(&metadata.workspace_id),
                object.payload_ref,
                object.payload_hash,
                object.payload_size as i64,
                object.payload_media_type,
                object.current_revision.max(1) as i64,
                storage_state_name(object.storage_state),
                metadata.provenance.as_ref().map(|p| p.source_type.as_str()).unwrap_or("local"),
                provenance,
                empty_to_none(&object.created_by_device_id),
                object.created_at,
                object.updated_at,
                optional_timestamp(object.last_used_at),
            ],
        ).context("insert object metadata")?;
        tx.execute(
            "INSERT INTO object_revisions
             (object_id, revision, payload_ref, payload_hash, payload_size, change_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                object.id,
                object.current_revision.max(1),
                object.payload_ref,
                object.payload_hash,
                object.payload_size as i64,
                change_type,
                now,
            ],
        )
        .context("insert initial object revision")?;
        upsert_search(&tx, object)?;
        tx.commit().context("commit object insert")
    }

    /// Read the metadata envelope without opening the encrypted payload.
    pub fn get_object(&self, id: &str) -> Result<Option<ObjectRecord>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, schema_version, title, description, sensitivity, status,
                    workspace_id, payload_ref, payload_hash, payload_size, payload_media_type,
                    current_revision, storage_state, provenance_type, provenance_json,
                    created_by_device, created_at, updated_at, last_used_at
             FROM objects WHERE id = ?1 AND status != 'deleted'",
        )?;
        let result = stmt.query_row([id], row_to_object).optional()?;
        Ok(result)
    }

    /// Search only redacted metadata indexed in SQLite FTS5.
    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<ObjectRecord>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT o.id, o.kind, o.schema_version, o.title, o.description, o.sensitivity,
                    o.status, o.workspace_id, o.payload_ref, o.payload_hash, o.payload_size,
                    o.payload_media_type, o.current_revision, o.storage_state, o.provenance_type,
                    o.provenance_json, o.created_by_device, o.created_at, o.updated_at, o.last_used_at
             FROM object_search s JOIN objects o ON o.id = s.object_id
             WHERE object_search MATCH ?1 AND o.status != 'deleted'
             ORDER BY o.updated_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![query, limit.max(1)], row_to_object)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Create an immutable revision and update the current object atomically.
    pub fn update_object(&self, object: &ObjectRecord, change_summary: &str) -> Result<()> {
        validate_object(object)?;
        let metadata = object
            .metadata
            .as_ref()
            .context("object metadata is required")?;
        let conn = self.lock()?;
        let tx = conn.unchecked_transaction()?;
        let current: i64 = tx
            .query_row(
                "SELECT current_revision FROM objects WHERE id = ?1",
                [&object.id],
                |row| row.get(0),
            )
            .context("read current object revision")?;
        let revision = object.current_revision as i64;
        if revision != current + 1 {
            return Err(anyhow!(
                "revision conflict: expected {}, received {}",
                current + 1,
                revision
            ));
        }
        tx.execute(
            "UPDATE objects SET title=?2, description=?3, sensitivity=?4, status=?5,
             workspace_id=?6, payload_ref=?7, payload_hash=?8, payload_size=?9,
             payload_media_type=?10, current_revision=?11, storage_state=?12,
             updated_at=?13 WHERE id=?1",
            params![
                object.id,
                metadata.title,
                metadata.description,
                sensitivity_name(metadata.sensitivity),
                status_name(object.status),
                empty_to_none(&metadata.workspace_id),
                object.payload_ref,
                object.payload_hash,
                object.payload_size as i64,
                object.payload_media_type,
                revision,
                storage_state_name(object.storage_state),
                object.updated_at,
            ],
        )?;
        tx.execute(
            "INSERT INTO object_revisions
             (object_id, revision, payload_ref, payload_hash, payload_size, change_type, change_summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'updated', ?6, ?7)",
            params![object.id, revision, object.payload_ref, object.payload_hash, object.payload_size as i64, change_summary, object.updated_at],
        )?;
        upsert_search(&tx, object)?;
        tx.commit().context("commit object revision")
    }

    pub fn bind_app(&self, binding: &AppBinding, now: i64) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO app_bindings
             (app_id, object_id, binding_kind, enabled, mapping_json, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(app_id, object_id, binding_kind) DO UPDATE SET
             enabled=excluded.enabled, mapping_json=excluded.mapping_json,
             revision=excluded.revision, updated_at=excluded.updated_at",
            params![binding.app_id, binding.object_id, binding.binding_kind, binding.enabled, serde_json::to_string(&binding.mapping)?, binding.revision, now],
        )?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow!("metadata database mutex poisoned"))
    }
}

fn validate_object(object: &ObjectRecord) -> Result<()> {
    if object.id.trim().is_empty() || object.payload_ref.trim().is_empty() {
        return Err(anyhow!("object id and payload_ref are required"));
    }
    if object.current_revision == 0 || object.payload_hash.trim().is_empty() {
        return Err(anyhow!("object revision and payload_hash are required"));
    }
    if object
        .metadata
        .as_ref()
        .map(|m| m.title.trim().is_empty())
        .unwrap_or(true)
    {
        return Err(anyhow!("object title is required"));
    }
    Ok(())
}

fn row_to_object(row: &rusqlite::Row<'_>) -> rusqlite::Result<ObjectRecord> {
    let kind: String = row.get(1)?;
    let sensitivity: String = row.get(5)?;
    let status: String = row.get(6)?;
    let storage_state: String = row.get(13)?;
    let provenance_json: String = row.get(15)?;
    let provenance = serde_json::from_str(&provenance_json).ok();
    Ok(ObjectRecord {
        id: row.get(0)?,
        kind: parse_object_kind(&kind),
        schema_version: row.get(2)?,
        metadata: Some(ObjectMetadata {
            title: row.get(3)?,
            description: row.get(4)?,
            sensitivity: parse_sensitivity(&sensitivity),
            workspace_id: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
            provenance,
            ..Default::default()
        }),
        status: parse_status(&status),
        payload_ref: row.get(8)?,
        payload_hash: row.get(9)?,
        payload_size: row.get::<_, i64>(10)? as u64,
        payload_media_type: row.get(11)?,
        current_revision: row.get::<_, i64>(12)? as u64,
        storage_state: parse_storage_state(&storage_state),
        created_by_device_id: row.get::<_, Option<String>>(16)?.unwrap_or_default(),
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
        last_used_at: row.get::<_, Option<i64>>(19)?.unwrap_or_default(),
    })
}

fn upsert_search(tx: &rusqlite::Transaction<'_>, object: &ObjectRecord) -> Result<()> {
    let metadata = object
        .metadata
        .as_ref()
        .context("object metadata is required")?;
    tx.execute(
        "DELETE FROM object_search WHERE object_id = ?1",
        [&object.id],
    )?;
    tx.execute(
        "INSERT INTO object_search(object_id, title, description, tags, kind, provenance)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            object.id,
            metadata.title,
            metadata.description,
            metadata.tags.join(" "),
            object_kind_name(object.kind),
            metadata
                .provenance
                .as_ref()
                .map(|p| p.source_type.as_str())
                .unwrap_or("local")
        ],
    )?;
    Ok(())
}

fn object_kind_name(value: i32) -> &'static str {
    ObjectKind::try_from(value)
        .map(|v| v.as_str_name())
        .unwrap_or("OBJECT_KIND_UNSPECIFIED")
}
fn sensitivity_name(value: i32) -> &'static str {
    match crate::proto_generated::common::Sensitivity::try_from(value).ok() {
        Some(crate::proto_generated::common::Sensitivity::Public) => "public",
        Some(crate::proto_generated::common::Sensitivity::Internal) => "internal",
        Some(crate::proto_generated::common::Sensitivity::Sensitive) => "sensitive",
        Some(crate::proto_generated::common::Sensitivity::Secret) => "secret",
        _ => "internal",
    }
}
fn status_name(value: i32) -> &'static str {
    match crate::proto_generated::common::ObjectStatus::try_from(value).ok() {
        Some(crate::proto_generated::common::ObjectStatus::Draft) => "draft",
        Some(crate::proto_generated::common::ObjectStatus::Active) => "active",
        Some(crate::proto_generated::common::ObjectStatus::Archived) => "archived",
        Some(crate::proto_generated::common::ObjectStatus::Revoked) => "revoked",
        Some(crate::proto_generated::common::ObjectStatus::Deleted) => "deleted",
        _ => "active",
    }
}
fn storage_state_name(value: i32) -> &'static str {
    match crate::proto_generated::common::StorageState::try_from(value).ok() {
        Some(crate::proto_generated::common::StorageState::Staged) => "staged",
        Some(crate::proto_generated::common::StorageState::Ready) => "ready",
        Some(crate::proto_generated::common::StorageState::Deleting) => "deleting",
        Some(crate::proto_generated::common::StorageState::Corrupt) => "corrupt",
        _ => "ready",
    }
}
fn parse_object_kind(value: &str) -> i32 {
    ObjectKind::from_str_name(&format!(
        "OBJECT_KIND_{}",
        value.to_ascii_uppercase().replace('.', "_")
    ))
    .unwrap_or(ObjectKind::Unspecified) as i32
}
fn parse_sensitivity(value: &str) -> i32 {
    crate::proto_generated::common::Sensitivity::from_str_name(&format!(
        "SENSITIVITY_{}",
        value.to_ascii_uppercase()
    ))
    .unwrap_or(crate::proto_generated::common::Sensitivity::Internal) as i32
}
fn parse_status(value: &str) -> i32 {
    crate::proto_generated::common::ObjectStatus::from_str_name(&format!(
        "OBJECT_STATUS_{}",
        value.to_ascii_uppercase()
    ))
    .unwrap_or(crate::proto_generated::common::ObjectStatus::Active) as i32
}
fn parse_storage_state(value: &str) -> i32 {
    crate::proto_generated::common::StorageState::from_str_name(&format!(
        "STORAGE_STATE_{}",
        value.to_ascii_uppercase()
    ))
    .unwrap_or(crate::proto_generated::common::StorageState::Ready) as i32
}
fn empty_to_none(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}
fn optional_timestamp(value: i64) -> Option<i64> {
    (value != 0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(id: &str, revision: u64) -> ObjectRecord {
        ObjectRecord {
            id: id.to_owned(),
            kind: ObjectKind::Note as i32,
            schema_version: 1,
            metadata: Some(ObjectMetadata {
                title: "Private note".into(),
                tags: vec!["research".into()],
                ..Default::default()
            }),
            payload_ref: format!("vault:{id}-r{revision}"),
            payload_hash: format!("hash-{revision}"),
            payload_size: 10,
            current_revision: revision,
            created_at: 1,
            updated_at: revision as i64,
            ..Default::default()
        }
    }

    #[test]
    fn stores_searches_and_updates_immutable_revisions() {
        let store = SqliteStore::open_in_memory().unwrap();
        assert_eq!(
            store.schema_version().unwrap(),
            crate::storage::SCHEMA_VERSION
        );
        store.insert_object(&fixture("o1", 1), "created").unwrap();
        assert_eq!(store.search("research", 10).unwrap().len(), 1);
        store.update_object(&fixture("o1", 2), "edit").unwrap();
        assert_eq!(store.get_object("o1").unwrap().unwrap().current_revision, 2);
        assert!(store.update_object(&fixture("o1", 2), "stale").is_err());
    }
}
