//! Phase 15 (15-01/15-02) — rusqlite-backed entitlement cache store.
//!
//! Plain inherent-impl newtype (NOT a `learnforge-core`-defined trait) —
//! entitlements is a Rust-side-only concern with no WASM-portable core
//! algorithm consuming it (15-PATTERNS.md). Follows the same
//! newtype-around-`&Connection` shape as [`super::bkt::SqliteBktStore`], and
//! the same `rusqlite::Error` -> typed-error `.map_err` boundary at the DB
//! edge.
//!
//! D-08: `find_by_pack_id` on a missing row is a clean `Ok(None)`, never an
//! error — most tracks are NOT redeemed, so "no entitlement row" is the
//! expected common case for the attribution lookup, not a failure.
//!
//! Wave 0 (15-01): both methods are `unimplemented!("15-02")` stubs.

use rusqlite::Connection;

/// A single redeemed-license cache row (D-05 columns).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitlementRow {
    pub pack_id: String,
    pub issuer_id: String,
    pub issuer_name: String,
    pub buyer_name: String,
    pub order_id: String,
    pub redeemed_at: String,
    pub key_fingerprint: String,
}

/// Rusqlite-backed entitlement cache store. Construct via
/// `SqliteEntitlementStore(&conn)` at the call site; the wrapper holds the
/// connection reference for the duration of the read/write.
pub struct SqliteEntitlementStore<'a>(pub &'a Connection);

impl<'a> SqliteEntitlementStore<'a> {
    /// Insert a new entitlement row after a successful redeem+import
    /// (D-05). 15-02 implements the real `INSERT INTO entitlements ...`.
    pub fn insert(&self, _row: &EntitlementRow) -> Result<(), String> {
        unimplemented!("15-02")
    }

    /// Look up the entitlement row for `pack_id`, if any. Returns `Ok(None)`
    /// (not an error) when no row exists — most tracks are unlicensed, so a
    /// miss is the expected case (D-08). 15-02 implements the real
    /// `SELECT ... WHERE pack_id = ?1` + `QueryReturnedNoRows` -> `Ok(None)`
    /// mapping.
    pub fn find_by_pack_id(&self, _pack_id: &str) -> Result<Option<EntitlementRow>, String> {
        unimplemented!("15-02")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE entitlements (
                 pack_id TEXT PRIMARY KEY,
                 issuer_id TEXT NOT NULL,
                 issuer_name TEXT NOT NULL,
                 buyer_name TEXT NOT NULL,
                 order_id TEXT NOT NULL,
                 redeemed_at TEXT NOT NULL,
                 key_fingerprint TEXT NOT NULL
             );",
        )
        .unwrap();
        conn
    }

    fn sample_row() -> EntitlementRow {
        EntitlementRow {
            pack_id: "pack-1".to_string(),
            issuer_id: "issuer-1".to_string(),
            issuer_name: "Test Issuer".to_string(),
            buyer_name: "Jane Buyer".to_string(),
            order_id: "ORD-1".to_string(),
            redeemed_at: "2026-07-12T00:00:00Z".to_string(),
            key_fingerprint: "deadbeef".to_string(),
        }
    }

    /// D-05 — insert then find_by_pack_id round-trips all fields. RED until
    /// 15-02 (both methods are unimplemented! stubs today).
    #[test]
    fn entitlement_store_insert_then_find_by_pack_id() {
        let conn = setup_test_db();
        let store = SqliteEntitlementStore(&conn);
        let row = sample_row();

        store.insert(&row).expect("15-02 must implement insert");

        let found = store
            .find_by_pack_id("pack-1")
            .expect("15-02 must implement find_by_pack_id")
            .expect("row must exist after insert");
        assert_eq!(found, row);
    }

    /// D-08 — a miss is a clean Ok(None), never an error (most tracks are
    /// unlicensed). RED until 15-02.
    #[test]
    fn entitlement_store_find_missing_returns_none() {
        let conn = setup_test_db();
        let store = SqliteEntitlementStore(&conn);

        let found = store
            .find_by_pack_id("does-not-exist")
            .expect("15-02: a missing pack_id must be Ok(None), never an Err");
        assert_eq!(found, None);
    }
}
