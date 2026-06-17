//! Wave 3 (Plan 07-03) — rusqlite-backed impl of
//! [`learnforge_core::sm2::SrStore`].
//!
//! Reads and writes the `sr_cards` table. All SQL strings are lifted
//! verbatim from the pre-Wave-3 `commands/learning.rs` and
//! `learning/microlearning_selection.rs` paths so the cross-crate seam is a
//! 1:1 row mapping rather than a behavioral change.
//!
//! ## Orphan-rule note (consistent with Wave 2 / `storage_impl::bkt`)
//!
//! `SrStore` is foreign (from `learnforge_core`) and
//! `rusqlite::Connection` is foreign (from `rusqlite`), so a direct
//! `impl SrStore for &Connection` would violate Rust's orphan rule
//! (E0117). We satisfy the rule by introducing the local newtype
//! [`SqliteSrStore`], which owns a `&Connection` and carries the impl —
//! same recipe as `SqliteBktStore` from Wave 2.
//!
//! ## Trust boundary (T-07-07)
//!
//! `rusqlite::Error` is stringified into `SrError::Db` here so
//! `learnforge-core` never depends on rusqlite. `QueryReturnedNoRows` on
//! the single-card lookup is mapped to `SrError::NotFound` so callers can
//! distinguish "no row" from "I/O failure" without leaking the rusqlite
//! type.

use learnforge_core::sm2::{SM2Result, SrCardRow, SrError, SrStore};
use rusqlite::Connection;

/// Rusqlite-backed [`SrStore`] adapter.
///
/// Construct via `SqliteSrStore(&conn)` at the call site; the wrapper holds
/// the connection reference for the duration of the read/write. The newtype
/// satisfies Rust's orphan-rule requirement that at least one of the trait
/// OR the impl target must be local to the impl-defining crate.
pub struct SqliteSrStore<'a>(pub &'a Connection);

impl<'a> SrStore for SqliteSrStore<'a> {
    fn read_due_cards(&self, limit: i32) -> Result<Vec<SrCardRow>, SrError> {
        let mut stmt = self
            .0
            .prepare(
                "SELECT id, module_id, concept, card_type, front, back, interval_days, ease_factor, repetitions, next_review, last_review
                 FROM sr_cards
                 WHERE next_review <= datetime('now')
                 ORDER BY next_review ASC
                 LIMIT ?1",
            )
            .map_err(|e| SrError::Db(e.to_string()))?;

        let rows = stmt
            .query_map(rusqlite::params![limit], |row| {
                Ok(SrCardRow {
                    id: row.get(0)?,
                    module_id: row.get(1)?,
                    concept: row.get(2)?,
                    card_type: row.get(3)?,
                    front: row.get(4)?,
                    back: row.get(5)?,
                    interval_days: row.get(6)?,
                    ease_factor: row.get(7)?,
                    repetitions: row.get(8)?,
                    next_review: row.get(9)?,
                    last_review: row.get(10)?,
                })
            })
            .map_err(|e| SrError::Db(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| SrError::Db(e.to_string()))?;

        Ok(rows)
    }

    fn count_due_cards_for_module(&self, module_id: &str) -> Result<i64, SrError> {
        self.0
            .query_row(
                "SELECT COUNT(*) FROM sr_cards
                  WHERE module_id = ?1 AND next_review <= datetime('now')",
                rusqlite::params![module_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| SrError::Db(e.to_string()))
    }

    fn read_card_by_id(&self, card_id: &str) -> Result<SrCardRow, SrError> {
        self.0
            .query_row(
                "SELECT id, module_id, concept, card_type, front, back, interval_days, ease_factor, repetitions, next_review, last_review
                 FROM sr_cards WHERE id = ?1",
                rusqlite::params![card_id],
                |row| {
                    Ok(SrCardRow {
                        id: row.get(0)?,
                        module_id: row.get(1)?,
                        concept: row.get(2)?,
                        card_type: row.get(3)?,
                        front: row.get(4)?,
                        back: row.get(5)?,
                        interval_days: row.get(6)?,
                        ease_factor: row.get(7)?,
                        repetitions: row.get(8)?,
                        next_review: row.get(9)?,
                        last_review: row.get(10)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => SrError::NotFound {
                    card_id: card_id.to_string(),
                },
                other => SrError::Db(other.to_string()),
            })
    }

    fn apply_review_update(
        &self,
        card_id: &str,
        result: &SM2Result,
    ) -> Result<String, SrError> {
        // Update SM-2 columns + advance next_review and stamp last_review.
        //
        // CR-01 (Phase 7 code review) — `result.interval` is `f64`. SM-2
        // legitimately produces fractional intervals (e.g. `6.0 * 2.6 =
        // 15.6`) after the third successful review. Previously the
        // `datetime('now', '+' || ?N || ' days')` modifier was bound to
        // `result.interval as i64`, truncating 15.6 → 15 and silently
        // drifting `next_review` by ~14h24m per fractional review.
        // SQLite accepts fractional day modifiers natively, so we now bind
        // `result.interval` (f64) to BOTH the column AND the modifier via
        // a single positional slot (?1 referenced twice).
        // Regression: `apply_review_update_preserves_fractional_interval`.
        self.0
            .execute(
                "UPDATE sr_cards SET interval_days = ?1, ease_factor = ?2, repetitions = ?3, next_review = datetime('now', '+' || ?1 || ' days'), last_review = datetime('now') WHERE id = ?4",
                rusqlite::params![
                    result.interval,
                    result.ease_factor,
                    result.repetitions,
                    card_id,
                ],
            )
            .map_err(|e| SrError::Db(e.to_string()))?;

        // Return the freshly-stamped next_review so callers can render
        // "next review in N days" without a follow-up query — matches the
        // submit_review behavior preserved at the shim level.
        self.0
            .query_row(
                "SELECT next_review FROM sr_cards WHERE id = ?1",
                rusqlite::params![card_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => SrError::NotFound {
                    card_id: card_id.to_string(),
                },
                other => SrError::Db(other.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // Lift the sr_cards schema from src-tauri/src/db/schema.rs:97-110 plus
        // a minimal modules parent so the FK isn't dangling. Keep this in-line
        // (don't pull CREATE_TABLES) so the test target stays a focused unit
        // probe for the adapter — not a full src-tauri integration check.
        conn.execute_batch(
            "CREATE TABLE modules (id TEXT PRIMARY KEY);
             CREATE TABLE sr_cards (
                 id TEXT PRIMARY KEY,
                 module_id TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
                 concept TEXT NOT NULL,
                 card_type TEXT NOT NULL DEFAULT 'active_recall',
                 front TEXT NOT NULL,
                 back TEXT NOT NULL,
                 interval_days REAL NOT NULL DEFAULT 1.0,
                 ease_factor REAL NOT NULL DEFAULT 2.5,
                 repetitions INTEGER NOT NULL DEFAULT 0,
                 next_review TEXT NOT NULL DEFAULT (datetime('now')),
                 last_review TEXT
             );",
        )
        .unwrap();
        conn.execute("INSERT INTO modules (id) VALUES ('mod-a')", []).unwrap();
        conn
    }

    fn seed_card(conn: &Connection, id: &str, module_id: &str, due_offset_secs: i64) {
        // due_offset_secs < 0 → already due; > 0 → due in the future.
        let due_clause = if due_offset_secs >= 0 {
            format!("datetime('now', '+{} seconds')", due_offset_secs)
        } else {
            format!("datetime('now', '{} seconds')", due_offset_secs)
        };
        let sql = format!(
            "INSERT INTO sr_cards (id, module_id, concept, card_type, front, back, interval_days, ease_factor, repetitions, next_review)
             VALUES (?1, ?2, 'concept-x', 'active_recall', 'front', 'back', 1.0, 2.5, 0, {})",
            due_clause
        );
        conn.execute(&sql, rusqlite::params![id, module_id]).unwrap();
    }

    #[test]
    fn read_due_cards_returns_due_rows_ordered() {
        let conn = setup_test_db();
        seed_card(&conn, "c1", "mod-a", -60); // due 60s ago
        seed_card(&conn, "c2", "mod-a", -120); // due 120s ago (earlier — should sort first)
        seed_card(&conn, "c3", "mod-a", 3600); // due in 1h — NOT due

        let store = SqliteSrStore(&conn);
        let rows = store.read_due_cards(10).unwrap();
        assert_eq!(rows.len(), 2, "only 2 cards are currently due");
        // ORDER BY next_review ASC → c2 (earlier) comes first
        assert_eq!(rows[0].id, "c2");
        assert_eq!(rows[1].id, "c1");
        assert_eq!(rows[0].module_id, "mod-a");
        assert_eq!(rows[0].concept, "concept-x");
        assert!((rows[0].ease_factor - 2.5).abs() < 1e-9);
    }

    #[test]
    fn read_due_cards_respects_limit() {
        let conn = setup_test_db();
        seed_card(&conn, "c1", "mod-a", -60);
        seed_card(&conn, "c2", "mod-a", -120);
        seed_card(&conn, "c3", "mod-a", -180);

        let store = SqliteSrStore(&conn);
        let rows = store.read_due_cards(2).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn count_due_cards_for_module_counts_only_due() {
        let conn = setup_test_db();
        seed_card(&conn, "c1", "mod-a", -60);
        seed_card(&conn, "c2", "mod-a", -120);
        seed_card(&conn, "c3", "mod-a", 3600); // future

        let store = SqliteSrStore(&conn);
        assert_eq!(store.count_due_cards_for_module("mod-a").unwrap(), 2);
        // Unknown module → 0, no error
        assert_eq!(store.count_due_cards_for_module("mod-missing").unwrap(), 0);
    }

    #[test]
    fn read_card_by_id_returns_row() {
        let conn = setup_test_db();
        seed_card(&conn, "c1", "mod-a", -60);

        let store = SqliteSrStore(&conn);
        let row = store.read_card_by_id("c1").unwrap();
        assert_eq!(row.id, "c1");
        assert_eq!(row.module_id, "mod-a");
        assert_eq!(row.card_type, "active_recall");
        assert_eq!(row.repetitions, 0);
        assert!(row.last_review.is_none());
    }

    #[test]
    fn read_card_by_id_missing_returns_not_found() {
        let conn = setup_test_db();
        let store = SqliteSrStore(&conn);
        match store.read_card_by_id("c-missing").unwrap_err() {
            SrError::NotFound { card_id } => assert_eq!(card_id, "c-missing"),
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn apply_review_update_persists_sm2_and_returns_next_review() {
        let conn = setup_test_db();
        seed_card(&conn, "c1", "mod-a", -60);

        let store = SqliteSrStore(&conn);
        let result = SM2Result {
            interval: 6.0,
            ease_factor: 2.6,
            repetitions: 2,
        };
        let next_review = store.apply_review_update("c1", &result).unwrap();
        assert!(
            !next_review.is_empty(),
            "next_review must be a non-empty ISO datetime"
        );

        // Verify columns persisted
        let row = store.read_card_by_id("c1").unwrap();
        assert!((row.interval_days - 6.0).abs() < 1e-9);
        assert!((row.ease_factor - 2.6).abs() < 1e-9);
        assert_eq!(row.repetitions, 2);
        assert_eq!(row.next_review, next_review);
        assert!(row.last_review.is_some());
    }

    /// CR-01 regression — SM-2 intervals are `f64` and legitimately
    /// fractional (e.g. `6.0 * 2.6 = 15.6`). Before the fix, the SQL bound
    /// `result.interval as i64` to the `datetime('now', '+?N days')`
    /// modifier, truncating 15.6 → 15 and silently drifting `next_review`
    /// by ~14h24m per fractional review. SQLite's `datetime` modifier
    /// accepts fractional days, so the fix passes `result.interval` (f64)
    /// directly to the modifier as well.
    ///
    /// Strategy: apply a review with `interval = 15.6`, then assert the
    /// persisted `next_review` lands within a small window of
    /// `datetime('now', '+15.6 days')` (which SQLite computes server-side
    /// at the same instant we read it back, so the delta is the difference
    /// between the two `datetime('now')` calls — bounded by a few seconds).
    #[test]
    fn apply_review_update_preserves_fractional_interval() {
        let conn = setup_test_db();
        seed_card(&conn, "c1", "mod-a", -60);

        let store = SqliteSrStore(&conn);
        let result = SM2Result {
            interval: 15.6, // 6.0 * 2.6 — typical SM-2 mid-card value
            ease_factor: 2.6,
            repetitions: 3,
        };
        let next_review = store.apply_review_update("c1", &result).unwrap();

        // Column stores the full f64.
        let row = store.read_card_by_id("c1").unwrap();
        assert!(
            (row.interval_days - 15.6).abs() < 1e-9,
            "interval_days column must preserve 15.6 exactly, got {}",
            row.interval_days
        );

        // The persisted next_review must match what SQLite computes from
        // the SAME fractional interval. The two `datetime('now')` calls
        // (one inside apply_review_update, one in this query) are seconds
        // apart, so the SQL-level delta is the test bound.
        let expected: String = conn
            .query_row(
                "SELECT datetime('now', '+' || ?1 || ' days')",
                rusqlite::params![15.6_f64],
                |row| row.get::<_, String>(0),
            )
            .unwrap();

        // Compare via SQLite's julianday() (returns f64 fractional days).
        let delta_days: f64 = conn
            .query_row(
                "SELECT ABS(julianday(?1) - julianday(?2))",
                rusqlite::params![next_review, expected],
                |row| row.get::<_, f64>(0),
            )
            .unwrap();

        // Pre-fix: delta_days ≈ 0.6 (the truncated 15 vs full 15.6).
        // Post-fix: delta_days < 1 second (the inter-statement gap).
        // 60 seconds is a generous CI ceiling; 14h24m would explode it.
        assert!(
            delta_days < 60.0 / 86_400.0,
            "next_review drift exceeds 60s — fractional interval truncated? \
             got {} expected {} delta_days={}",
            next_review,
            expected,
            delta_days
        );
    }
}
