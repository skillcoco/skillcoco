//! Migration v1: baseline schema marker.
//!
//! Version 1 records that the baseline CREATE_TABLES schema (managed by schema.rs)
//! has been applied. No DDL is executed here — schema.rs already runs CREATE TABLE IF NOT EXISTS
//! for all base tables idempotently. This migration just ensures the version table
//! records "v1 = initial baseline" so that future ALTER TABLE migrations can gate on version.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 1;
pub const NAME: &str = "initial_baseline";

pub fn up(_conn: &Connection) -> Result<()> {
    // v1 = baseline. schema::CREATE_TABLES already ran via run_migrations.
    // This migration just records that the baseline schema is present at v1.
    // Future migrations (v002+) use ALTER TABLE to extend the schema.
    Ok(())
}
