//! Migration v020 — entitlements table + learning_paths.pack_id column.
//!
//! Phase 15 (ENT-01..04, D-05): local cache of redeemed licenses, keyed by
//! `pack_id`. Survives pack/track deletion; drives buyer-attribution
//! rendering (D-08) without a server round-trip.
//!
//! Schema: `entitlements(pack_id PK, issuer_id, issuer_name, buyer_name,
//! order_id, redeemed_at, key_fingerprint)` plus `learning_paths.pack_id`
//! (nullable TEXT — Open Question 1 / A3, mirrors how v015 added
//! `verified`/`issuer_name` via ALTER TABLE).
//!
//! Wave 0 (15-01): `up()` is a no-op stub (`Ok(())`) so prior-migration
//! idempotency tests stay green. 15-02's GREEN step fills in the real DDL,
//! registers this migration in `registered_migrations()`, and bumps the
//! hardcoded `19` version-count assertions in `mod.rs` to `20`. DO NOT
//! register v020 here — that is explicitly 15-02's job (see mod.rs).

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 20;
pub const NAME: &str = "entitlements";

/// Apply the v020 migration. Wave 0 no-op — 15-02 replaces this body with:
/// `CREATE TABLE IF NOT EXISTS entitlements (...)` (RESEARCH.md Code
/// Examples has the exact DDL) plus an ALTER TABLE for
/// `learning_paths.pack_id` guarded by [`column_exists`] (mirrors v015).
pub fn up(_conn: &Connection) -> Result<()> {
    Ok(())
}

/// Check whether `column` exists in `table` by querying PRAGMA table_info.
/// Copied from `v015_learning_path_verified.rs` — SQLite has no `ALTER
/// TABLE ... ADD COLUMN IF NOT EXISTS`, so every ALTER-TABLE migration in
/// this codebase guards with this helper first.
#[allow(dead_code)]
fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for c in cols {
        if c? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(crate::db::schema::CREATE_TABLES)
            .expect("baseline tables");
        conn
    }

    /// D-05 — after up(), the entitlements table exists with all required
    /// columns. RED until 15-02 (up() is currently a no-op stub).
    #[test]
    fn v020_creates_entitlements_table_with_required_columns() {
        let conn = fresh_conn();
        up(&conn).expect("15-02 must create entitlements table");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='entitlements'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "15-02 must create the entitlements table — v020::up() is still a Wave 0 no-op"
        );

        let mut stmt = conn.prepare("PRAGMA table_info(entitlements)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|c| c.ok())
            .collect();
        for required in [
            "pack_id",
            "issuer_id",
            "issuer_name",
            "buyer_name",
            "order_id",
            "redeemed_at",
            "key_fingerprint",
        ] {
            assert!(
                cols.iter().any(|c| c == required),
                "15-02 must add entitlements column: {} (v020::up() is still a no-op)",
                required
            );
        }
    }

    /// Open Question 1 / A3 — learning_paths gains a nullable pack_id column
    /// in the same migration (mirrors v015's ALTER-TABLE-on-existing-table
    /// recipe). RED until 15-02.
    #[test]
    fn v020_adds_pack_id_column_to_learning_paths() {
        let conn = fresh_conn();
        up(&conn).expect("15-02 must run learning_paths ALTER TABLE");

        let exists = column_exists(&conn, "learning_paths", "pack_id")
            .expect("PRAGMA table_info(learning_paths) must succeed");
        assert!(
            exists,
            "15-02 must add learning_paths.pack_id column (v020::up() is still a no-op)"
        );
    }

    /// Idempotent double-apply — running up() twice must not error, even
    /// once real DDL lands (CREATE TABLE IF NOT EXISTS + column_exists
    /// guard). RED-by-construction today since up() is a no-op that trivially
    /// passes; kept as a named regression pin for 15-02.
    #[test]
    fn v020_idempotent_double_apply() {
        let conn = fresh_conn();
        up(&conn).expect("first apply must succeed");
        up(&conn).expect("second apply must succeed (idempotent)");
    }
}
