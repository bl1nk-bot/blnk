//! Metadata storage for Bl1nk's universal object model.
//!
//! This module owns SQLite metadata, migrations, search indexes, revisions,
//! audit records, sharing metadata, and sync state. Encrypted payloads are
//! intentionally handled by a separate vault module.

pub mod schema;
pub mod store;

pub use schema::{SCHEMA_VERSION, create_tables, schema_version};
pub use store::SqliteStore;
