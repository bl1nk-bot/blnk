//! SQLite schema and migrations for the Bl1nk object store.
//!
//! The schema deliberately stores object metadata separately from encrypted
//! payloads. `payload_ref` is an opaque reference into the vault and must not
//! contain secrets, titles, paths, or raw credentials.

use rusqlite::{Connection, OptionalExtension, Result as SqlResult};

/// Current metadata schema version.
pub const SCHEMA_VERSION: i64 = 1;

/// Create the complete v1 schema on a SQLite connection.
///
/// The function is idempotent and is safe to call during application startup.
pub fn create_tables(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = FULL;
        PRAGMA busy_timeout = 5000;
        PRAGMA trusted_schema = OFF;
        PRAGMA secure_delete = ON;
        PRAGMA recursive_triggers = ON;
        PRAGMA auto_vacuum = INCREMENTAL;

        CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            checksum   TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS app_metadata (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS devices (
            id                TEXT PRIMARY KEY,
            uid               TEXT NOT NULL UNIQUE,
            label             TEXT NOT NULL,
            platform          TEXT,
            public_key        TEXT NOT NULL,
            key_algorithm     TEXT NOT NULL DEFAULT 'rsa-2048',
            trust_state       TEXT NOT NULL DEFAULT 'unknown'
                CHECK (trust_state IN ('unknown','pending','trusted','blocked','revoked')),
            capabilities_json TEXT NOT NULL DEFAULT '{}',
            endpoint_hint     TEXT,
            last_seen_at      INTEGER,
            created_at        INTEGER NOT NULL,
            updated_at        INTEGER NOT NULL,
            revoked_at        INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_devices_trust
            ON devices(trust_state, last_seen_at DESC);

        CREATE TABLE IF NOT EXISTS objects (
            id                 TEXT PRIMARY KEY,
            kind               TEXT NOT NULL,
            schema_version     INTEGER NOT NULL DEFAULT 1,
            title              TEXT NOT NULL,
            description        TEXT,
            sensitivity        TEXT NOT NULL DEFAULT 'internal'
                CHECK (sensitivity IN ('public','internal','sensitive','secret')),
            status             TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('draft','active','archived','revoked','deleted')),
            workspace_id       TEXT,
            payload_ref        TEXT NOT NULL UNIQUE,
            payload_hash       TEXT NOT NULL,
            payload_size       INTEGER NOT NULL DEFAULT 0 CHECK (payload_size >= 0),
            payload_media_type TEXT NOT NULL DEFAULT 'application/json',
            current_revision   INTEGER NOT NULL DEFAULT 1 CHECK (current_revision >= 1),
            storage_state      TEXT NOT NULL DEFAULT 'ready'
                CHECK (storage_state IN ('staged','ready','deleting','corrupt')),
            provenance_type    TEXT NOT NULL DEFAULT 'local',
            provenance_json    TEXT NOT NULL DEFAULT '{}',
            created_by_device  TEXT,
            created_at         INTEGER NOT NULL,
            updated_at         INTEGER NOT NULL,
            last_used_at       INTEGER,
            deleted_at         INTEGER,
            FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE SET NULL,
            FOREIGN KEY (created_by_device) REFERENCES devices(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_objects_kind_status
            ON objects(kind, status);
        CREATE INDEX IF NOT EXISTS idx_objects_workspace
            ON objects(workspace_id, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_objects_updated
            ON objects(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_objects_sensitivity
            ON objects(sensitivity, status);
        CREATE INDEX IF NOT EXISTS idx_objects_payload_hash
            ON objects(payload_hash);

        CREATE TABLE IF NOT EXISTS workspaces (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            description TEXT,
            root_path   TEXT,
            status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active','archived','deleted')),
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_workspaces_status
            ON workspaces(status, updated_at DESC);

        CREATE TABLE IF NOT EXISTS apps (
            id                TEXT PRIMARY KEY,
            adapter_id        TEXT NOT NULL,
            display_name      TEXT NOT NULL,
            version           TEXT,
            detected_path     TEXT,
            capabilities_json TEXT NOT NULL DEFAULT '{}',
            status             TEXT NOT NULL DEFAULT 'unknown'
                CHECK (status IN ('unknown','detected','available','unsupported','disabled')),
            created_at        INTEGER NOT NULL,
            updated_at        INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS app_bindings (
            app_id       TEXT NOT NULL,
            object_id     TEXT NOT NULL,
            binding_kind  TEXT NOT NULL,
            enabled       INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
            mapping_json TEXT NOT NULL DEFAULT '{}',
            revision     INTEGER NOT NULL DEFAULT 1,
            created_at   INTEGER NOT NULL,
            updated_at   INTEGER NOT NULL,
            PRIMARY KEY (app_id, object_id, binding_kind),
            FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE,
            FOREIGN KEY (object_id) REFERENCES objects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS tags (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL COLLATE NOCASE UNIQUE,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS object_tags (
            object_id TEXT NOT NULL,
            tag_id    TEXT NOT NULL,
            PRIMARY KEY (object_id, tag_id),
            FOREIGN KEY (object_id) REFERENCES objects(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS object_search USING fts5(
            object_id UNINDEXED,
            title,
            description,
            tags,
            kind,
            provenance
        );

        CREATE TABLE IF NOT EXISTS object_revisions (
            object_id        TEXT NOT NULL,
            revision         INTEGER NOT NULL,
            payload_ref      TEXT NOT NULL,
            payload_hash     TEXT NOT NULL,
            payload_size     INTEGER NOT NULL,
            change_type      TEXT NOT NULL
                CHECK (change_type IN ('created','updated','imported','applied','restored','merged')),
            change_summary   TEXT,
            author_device_id TEXT,
            created_at       INTEGER NOT NULL,
            PRIMARY KEY (object_id, revision),
            FOREIGN KEY (object_id) REFERENCES objects(id) ON DELETE CASCADE,
            FOREIGN KEY (author_device_id) REFERENCES devices(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS object_sources (
            object_id     TEXT NOT NULL,
            source_type   TEXT NOT NULL,
            source_uri    TEXT,
            source_app_id TEXT,
            source_hash   TEXT,
            imported_at   INTEGER NOT NULL,
            metadata_json TEXT NOT NULL DEFAULT '{}',
            PRIMARY KEY (object_id, source_type, source_uri),
            FOREIGN KEY (object_id) REFERENCES objects(id) ON DELETE CASCADE,
            FOREIGN KEY (source_app_id) REFERENCES apps(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS shares (
            id                   TEXT PRIMARY KEY,
            object_id            TEXT,
            pack_manifest_json   TEXT,
            envelope_ref         TEXT NOT NULL UNIQUE,
            channel              TEXT NOT NULL
                CHECK (channel IN ('local_file','clipboard','qr','deep_link','https','p2p','sync','api')),
            trust_policy         TEXT NOT NULL
                CHECK (trust_policy IN ('local','private_device','selected_recipient','link_bearer','delegated')),
            recipient_device_id  TEXT,
            recipient_hint       TEXT,
            expires_at           INTEGER NOT NULL,
            one_time             INTEGER NOT NULL DEFAULT 1 CHECK (one_time IN (0,1)),
            status               TEXT NOT NULL DEFAULT 'created'
                CHECK (status IN ('created','offered','opened','verified','consumed','revoked','expired','failed')),
            attempts             INTEGER NOT NULL DEFAULT 0,
            max_attempts         INTEGER NOT NULL DEFAULT 3,
            locked_until         INTEGER,
            created_by_device_id TEXT,
            created_at           INTEGER NOT NULL,
            opened_at            INTEGER,
            consumed_at          INTEGER,
            revoked_at           INTEGER,
            FOREIGN KEY (object_id) REFERENCES objects(id) ON DELETE SET NULL,
            FOREIGN KEY (recipient_device_id) REFERENCES devices(id) ON DELETE SET NULL,
            FOREIGN KEY (created_by_device_id) REFERENCES devices(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_shares_status_expiry
            ON shares(status, expires_at);
        CREATE INDEX IF NOT EXISTS idx_shares_recipient
            ON shares(recipient_device_id, status);

        CREATE TABLE IF NOT EXISTS share_events (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            share_id        TEXT NOT NULL,
            event_type      TEXT NOT NULL
                CHECK (event_type IN ('created','offered','opened','pin_failed','verified','consumed','revoked','expired','failed')),
            actor_device_id TEXT,
            result          TEXT NOT NULL DEFAULT 'ok',
            metadata_json   TEXT NOT NULL DEFAULT '{}',
            created_at      INTEGER NOT NULL,
            FOREIGN KEY (share_id) REFERENCES shares(id) ON DELETE CASCADE,
            FOREIGN KEY (actor_device_id) REFERENCES devices(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS apply_receipts (
            id           TEXT PRIMARY KEY,
            object_id    TEXT NOT NULL,
            revision     INTEGER NOT NULL,
            app_id       TEXT,
            adapter_id   TEXT,
            target_path  TEXT,
            before_hash  TEXT,
            after_hash   TEXT,
            backup_id    TEXT,
            status       TEXT NOT NULL
                CHECK (status IN ('planned','applied','rolled_back','failed')),
            error_code   TEXT,
            created_at   INTEGER NOT NULL,
            completed_at INTEGER,
            FOREIGN KEY (object_id) REFERENCES objects(id) ON DELETE CASCADE,
            FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS backups (
            id              TEXT PRIMARY KEY,
            kind            TEXT NOT NULL CHECK (kind IN ('metadata','full','object','live_config')),
            path            TEXT NOT NULL,
            checksum        TEXT NOT NULL,
            encrypted       INTEGER NOT NULL DEFAULT 1 CHECK (encrypted IN (0,1)),
            source_revision INTEGER,
            reason          TEXT NOT NULL,
            created_at      INTEGER NOT NULL,
            expires_at      INTEGER,
            restored_at     INTEGER
        );

        CREATE TABLE IF NOT EXISTS audit_events (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            event_id    TEXT NOT NULL UNIQUE,
            actor_type  TEXT NOT NULL CHECK (actor_type IN ('user','device','system','adapter')),
            actor_id    TEXT,
            action      TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id   TEXT,
            result      TEXT NOT NULL CHECK (result IN ('success','failure','denied')),
            error_code  TEXT,
            metadata_json TEXT NOT NULL DEFAULT '{}',
            created_at  INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_audit_entity
            ON audit_events(entity_type, entity_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_audit_time
            ON audit_events(created_at DESC);

        CREATE TABLE IF NOT EXISTS sync_cursors (
            provider_id     TEXT PRIMARY KEY,
            local_cursor    TEXT,
            remote_cursor   TEXT,
            last_success_at INTEGER,
            last_error_code TEXT,
            updated_at      INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_changes (
            id           TEXT PRIMARY KEY,
            provider_id  TEXT NOT NULL,
            object_id    TEXT NOT NULL,
            revision     INTEGER NOT NULL,
            operation    TEXT NOT NULL CHECK (operation IN ('upsert','delete','revoke')),
            payload_ref  TEXT,
            payload_hash TEXT,
            vector_json  TEXT NOT NULL DEFAULT '{}',
            state        TEXT NOT NULL DEFAULT 'pending'
                CHECK (state IN ('pending','uploaded','downloaded','applied','conflict','failed')),
            created_at   INTEGER NOT NULL,
            updated_at   INTEGER NOT NULL,
            FOREIGN KEY (object_id) REFERENCES objects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS sync_conflicts (
            id              TEXT PRIMARY KEY,
            object_id       TEXT NOT NULL,
            local_revision  INTEGER NOT NULL,
            remote_revision INTEGER NOT NULL,
            local_hash      TEXT NOT NULL,
            remote_hash     TEXT NOT NULL,
            resolution      TEXT CHECK (resolution IN ('keep_local','keep_remote','merge','duplicate')),
            resolved_at     INTEGER,
            created_at      INTEGER NOT NULL,
            FOREIGN KEY (object_id) REFERENCES objects(id) ON DELETE CASCADE
        );
        "#,
    )?;

    migrate_v1(conn)?;
    Ok(())
}

fn migrate_v1(conn: &Connection) -> SqlResult<()> {
    let applied: Option<i64> = conn
        .query_row(
            "SELECT version FROM schema_migrations WHERE version = ?1",
            [SCHEMA_VERSION],
            |row| row.get(0),
        )
        .optional()?;

    if applied.is_none() {
        conn.execute(
            "INSERT INTO schema_migrations(version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, unixepoch())",
            (SCHEMA_VERSION, "object-store-v1", "blnk-object-store-v1"),
        )?;
    }

    conn.execute(
        "INSERT INTO app_metadata(key, value, updated_at)
         VALUES ('schema_version', ?1, unixepoch())
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        [SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

/// Return the highest schema version recorded in the metadata database.
pub fn schema_version(conn: &Connection) -> SqlResult<i64> {
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_idempotent_object_store_schema() {
        let conn = Connection::open_in_memory().expect("open sqlite");
        create_tables(&conn).expect("create schema");
        create_tables(&conn).expect("schema must be idempotent");
        assert_eq!(schema_version(&conn).expect("read version"), SCHEMA_VERSION);
    }

    #[test]
    fn enforces_object_sensitivity_and_storage_state() {
        let conn = Connection::open_in_memory().expect("open sqlite");
        create_tables(&conn).expect("create schema");
        let result = conn.execute(
            "INSERT INTO objects
             (id, kind, title, payload_ref, payload_hash, payload_size, sensitivity, storage_state, created_at, updated_at)
             VALUES ('obj-1', 'note', 'hello', 'vault:obj-1-r1', 'hash', 5, 'invalid', 'ready', 1, 1)",
            [],
        );
        assert!(result.is_err(), "invalid sensitivity must be rejected");
    }

    #[test]
    fn rejects_searchable_secret_payload_design() {
        let conn = Connection::open_in_memory().expect("open sqlite");
        create_tables(&conn).expect("create schema");
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(object_search)")
            .expect("prepare pragma")
            .query_map([], |row| row.get(1))
            .expect("read fts columns")
            .collect::<SqlResult<Vec<String>>>()
            .expect("collect fts columns");
        assert!(!columns.iter().any(|column| column == "secret"));
        assert!(!columns.iter().any(|column| column == "password"));
    }
}
