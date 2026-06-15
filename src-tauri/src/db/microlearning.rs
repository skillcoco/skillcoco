//! Daily-challenge + learner-streak DB helpers (Phase 4 Wave 1 GREEN).
//!
//! Pure helpers over `&rusqlite::Connection`. The IPC layer (Plan 03) holds
//! the lock exactly once and calls these. Mirrors the helper layout of
//! `db/blocks.rs` and `db/lab_progress.rs`.

use rusqlite::{params, Connection, OptionalExtension};

// ── Row structs (internal — IPC payloads live in commands/microlearning.rs) ──

/// In-memory representation of a `daily_challenges` row. Composite PK is
/// `(learner_id, challenge_date)`. See migration v007 for the schema.
#[derive(Debug, Clone)]
pub struct DailyChallengeRow {
    pub learner_id: String,
    pub challenge_date: String,
    pub block_id: String,
    pub module_id: String,
    pub track_id: String,
    pub block_type: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// In-memory representation of a `learner_streaks` row. PK is `learner_id`.
/// Mirrors `learning_tracks.streak_days` semantics from Phase 1 but at
/// learner-global scope (D-06).
#[derive(Debug, Clone)]
pub struct LearnerStreakRow {
    pub learner_id: String,
    pub streak_days: i32,
    pub last_activity_date: Option<String>,
    pub updated_at: String,
}

// ── CRUD helpers (daily_challenges) ──

/// Returns the persisted daily-challenge row for the given learner + date,
/// or `Ok(None)` if no row exists.
pub fn get_daily_challenge_for_date(
    conn: &Connection,
    learner_id: &str,
    challenge_date: &str,
) -> Result<Option<DailyChallengeRow>, String> {
    conn.query_row(
        "SELECT learner_id, challenge_date, block_id, module_id, track_id,
                block_type, started_at, completed_at
           FROM daily_challenges
          WHERE learner_id = ?1 AND challenge_date = ?2",
        params![learner_id, challenge_date],
        |row| {
            Ok(DailyChallengeRow {
                learner_id: row.get(0)?,
                challenge_date: row.get(1)?,
                block_id: row.get(2)?,
                module_id: row.get(3)?,
                track_id: row.get(4)?,
                block_type: row.get(5)?,
                started_at: row.get(6)?,
                completed_at: row.get(7)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("get_daily_challenge_for_date: {}", e))
}

/// Inserts a new daily-challenge row. INSERT-only — the composite PK means
/// a second insert for `(learner, date)` errors loudly because the selection
/// algorithm should never re-run mid-day.
pub fn insert_daily_challenge(
    conn: &Connection,
    row: &DailyChallengeRow,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO daily_challenges
            (learner_id, challenge_date, block_id, module_id, track_id, block_type)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.learner_id,
            row.challenge_date,
            row.block_id,
            row.module_id,
            row.track_id,
            row.block_type,
        ],
    )
    .map_err(|e| format!("insert_daily_challenge: {}", e))?;
    Ok(())
}

/// Marks the daily challenge started (sets `started_at` if NULL). Idempotent —
/// second call preserves the FIRST timestamp via COALESCE.
pub fn mark_daily_challenge_started(
    conn: &Connection,
    learner_id: &str,
    challenge_date: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE daily_challenges
            SET started_at = COALESCE(started_at, datetime('now'))
          WHERE learner_id = ?1 AND challenge_date = ?2",
        params![learner_id, challenge_date],
    )
    .map_err(|e| format!("mark_daily_challenge_started: {}", e))?;
    Ok(())
}

/// Marks the daily challenge complete. Returns the row's `completed_at`
/// (idempotent — second call returns the SAME timestamp, no double-bump).
/// Plan 03 echoes this to the frontend so the UI shows "done at HH:MM".
pub fn mark_daily_challenge_completed(
    conn: &Connection,
    learner_id: &str,
    challenge_date: &str,
) -> Result<String, String> {
    conn.execute(
        "UPDATE daily_challenges
            SET completed_at = COALESCE(completed_at, datetime('now'))
          WHERE learner_id = ?1 AND challenge_date = ?2",
        params![learner_id, challenge_date],
    )
    .map_err(|e| format!("mark_daily_challenge_completed: {}", e))?;

    // Echo the resulting completed_at so callers don't need a second read.
    let completed_at: Option<String> = conn
        .query_row(
            "SELECT completed_at FROM daily_challenges
              WHERE learner_id = ?1 AND challenge_date = ?2",
            params![learner_id, challenge_date],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("mark_daily_challenge_completed: read-back failed: {}", e))?
        .flatten();

    completed_at.ok_or_else(|| {
        format!(
            "mark_daily_challenge_completed: no row for ({}, {})",
            learner_id, challenge_date
        )
    })
}

// ── Streak helpers (learner_streaks) ──

/// Reads the global streak row for a learner. Returns a synthetic
/// zeroed `LearnerStreakRow` on a brand-new learner (no row yet) so callers
/// don't need to special-case the first-ever activity. Other DB errors propagate.
pub fn get_learner_streak(
    conn: &Connection,
    learner_id: &str,
) -> Result<LearnerStreakRow, String> {
    match conn.query_row(
        "SELECT learner_id, streak_days, last_activity_date, updated_at
           FROM learner_streaks
          WHERE learner_id = ?1",
        params![learner_id],
        |row| {
            Ok(LearnerStreakRow {
                learner_id: row.get(0)?,
                streak_days: row.get(1)?,
                last_activity_date: row.get(2)?,
                updated_at: row.get(3)?,
            })
        },
    ) {
        Ok(row) => Ok(row),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(LearnerStreakRow {
            learner_id: learner_id.to_string(),
            streak_days: 0,
            last_activity_date: None,
            updated_at: String::new(),
        }),
        Err(e) => Err(format!("get_learner_streak: {}", e)),
    }
}

/// Sibling of `commands::learning::update_streak` (`commands/learning.rs:968`)
/// scoped to the global `learner_streaks` table. R3 forbids modifying the
/// per-track helper's signature; this is a copy-adapt, not a generalization.
///
/// Mirrors `update_streak`'s four branches verbatim:
/// - Branch 1 (None last_activity): first activity ever — set streak=1.
/// - Branch 2 (same calendar day): no-op, return current streak.
/// - Branch 3 (within 24h, different day): increment streak.
/// - Branch 4 (gap > 24h): hard reset to 1. **R4 — locked behavior; no Freeze.**
///
/// Returns the resulting `streak_days` value after the update.
pub fn update_global_streak(
    conn: &Connection,
    learner_id: &str,
) -> Result<i32, String> {
    // Read current streak state — match Branch 1 by absence of row OR NULL last_activity.
    let existing: Option<(i32, Option<String>)> = conn
        .query_row(
            "SELECT COALESCE(streak_days, 0), last_activity_date
               FROM learner_streaks
              WHERE learner_id = ?1",
            params![learner_id],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|e| format!("update_global_streak: read failed: {}", e))?;

    let (current_streak, last_activity) = match existing {
        Some(tuple) => tuple,
        None => (0, None),
    };

    match last_activity {
        None => {
            // Branch 1: First activity ever (no row OR row with NULL last_activity).
            // INSERT OR REPLACE handles both — fresh learner gets a row, an existing
            // row with NULL last_activity is overwritten cleanly.
            conn.execute(
                "INSERT OR REPLACE INTO learner_streaks
                    (learner_id, streak_days, last_activity_date, updated_at)
                 VALUES (?1, 1, datetime('now'), datetime('now'))",
                params![learner_id],
            )
            .map_err(|e| format!("update_global_streak: {}", e))?;
            Ok(1)
        }
        Some(_) => {
            // Branch 2: same calendar day — no-op, return current streak unchanged.
            let is_today: bool = conn
                .query_row(
                    "SELECT date(last_activity_date) = date('now')
                       FROM learner_streaks
                      WHERE learner_id = ?1",
                    params![learner_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if is_today {
                return Ok(current_streak);
            }

            // Branch 3 vs 4: within-24h different-day increment, or gap > 24h reset.
            let within_24h: bool = conn
                .query_row(
                    "SELECT last_activity_date >= datetime('now', '-1 day')
                       FROM learner_streaks
                      WHERE learner_id = ?1",
                    params![learner_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            let new_streak = if within_24h {
                // Branch 3
                current_streak + 1
            } else {
                // Branch 4 — R4 locked hard reset; do NOT add Freeze logic.
                1
            };

            conn.execute(
                "UPDATE learner_streaks
                    SET streak_days = ?1,
                        last_activity_date = datetime('now'),
                        updated_at = datetime('now')
                  WHERE learner_id = ?2",
                params![new_streak, learner_id],
            )
            .map_err(|e| format!("update_global_streak: {}", e))?;

            Ok(new_streak)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
        crate::db::migrations::apply_migrations(&conn).unwrap();
        conn
    }

    /// Seeds `learner_profiles -> learning_tracks -> learning_paths -> modules ->
    /// module_blocks` with deterministic IDs. Returns the IDs.
    fn seed_learner_track_module_block(
        conn: &Connection,
    ) -> (String, String, String, String) {
        let learner_id = "learner-1".to_string();
        let track_id = "trk-1".to_string();
        let path_id = "pth-1".to_string();
        let module_id = "mod-1".to_string();
        let block_id = "blk-1".to_string();

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'Tester')",
            params![&learner_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal)
             VALUES (?1, ?2, 'Rust', 'programming', 'Learn Rust')",
            params![&track_id, &learner_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
            params![&path_id, &track_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES (?1, ?2, 'Module 1')",
            params![&module_id, &path_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status)
             VALUES (?1, ?2, 0, 'section', 'ready')",
            params![&block_id, &module_id],
        )
        .unwrap();

        (learner_id, track_id, module_id, block_id)
    }

    // ── daily_challenges CRUD ──

    /// MICRO-03 — round-trip insert + read.
    #[test]
    fn insert_and_get_daily_challenge_roundtrip() {
        let conn = fresh_conn();
        let (learner_id, track_id, module_id, block_id) = seed_learner_track_module_block(&conn);
        let row = DailyChallengeRow {
            learner_id: learner_id.clone(),
            challenge_date: "2026-06-15".into(),
            block_id: block_id.clone(),
            module_id: module_id.clone(),
            track_id: track_id.clone(),
            block_type: "section".into(),
            started_at: None,
            completed_at: None,
        };
        insert_daily_challenge(&conn, &row).expect("insert ok");
        let fetched = get_daily_challenge_for_date(&conn, &learner_id, "2026-06-15")
            .expect("ok")
            .expect("row must exist");
        assert_eq!(fetched.block_id, block_id);
        assert_eq!(fetched.module_id, module_id);
        assert_eq!(fetched.track_id, track_id);
        assert_eq!(fetched.block_type, "section");
        assert_eq!(fetched.started_at, None);
        assert_eq!(fetched.completed_at, None);
    }

    #[test]
    fn mark_daily_challenge_started_idempotent() {
        let conn = fresh_conn();
        let (learner_id, track_id, module_id, block_id) = seed_learner_track_module_block(&conn);
        let row = DailyChallengeRow {
            learner_id: learner_id.clone(),
            challenge_date: "2026-06-15".into(),
            block_id,
            module_id,
            track_id,
            block_type: "section".into(),
            started_at: None,
            completed_at: None,
        };
        insert_daily_challenge(&conn, &row).expect("insert ok");

        // First mark sets started_at
        mark_daily_challenge_started(&conn, &learner_id, "2026-06-15").expect("first mark ok");
        let first_started: Option<String> = conn
            .query_row(
                "SELECT started_at FROM daily_challenges WHERE learner_id = ?1 AND challenge_date = ?2",
                params![&learner_id, "2026-06-15"],
                |r| r.get(0),
            )
            .unwrap();
        let first_ts = first_started.expect("started_at must be set");

        // Sleep is not required — SQLite datetime('now') resolves to seconds, but
        // even at the same second, COALESCE preserves the first timestamp.
        mark_daily_challenge_started(&conn, &learner_id, "2026-06-15").expect("second mark ok");
        let second_started: Option<String> = conn
            .query_row(
                "SELECT started_at FROM daily_challenges WHERE learner_id = ?1 AND challenge_date = ?2",
                params![&learner_id, "2026-06-15"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            second_started.expect("started_at must remain set"),
            first_ts,
            "second mark_started must preserve the FIRST timestamp (COALESCE idempotency)"
        );
    }

    #[test]
    fn mark_daily_challenge_completed_idempotent() {
        let conn = fresh_conn();
        let (learner_id, track_id, module_id, block_id) = seed_learner_track_module_block(&conn);
        let row = DailyChallengeRow {
            learner_id: learner_id.clone(),
            challenge_date: "2026-06-15".into(),
            block_id,
            module_id,
            track_id,
            block_type: "section".into(),
            started_at: None,
            completed_at: None,
        };
        insert_daily_challenge(&conn, &row).expect("insert ok");

        let first = mark_daily_challenge_completed(&conn, &learner_id, "2026-06-15")
            .expect("first complete ok");
        let second = mark_daily_challenge_completed(&conn, &learner_id, "2026-06-15")
            .expect("second complete ok");
        assert_eq!(
            first, second,
            "second mark_completed must return SAME completed_at (no double-bump)"
        );
    }

    /// R5 — daily_challenge row cascades on module_blocks DELETE.
    #[test]
    fn daily_challenge_cascade_on_block_delete() {
        let conn = fresh_conn();
        let (learner_id, track_id, module_id, block_id) = seed_learner_track_module_block(&conn);
        let row = DailyChallengeRow {
            learner_id: learner_id.clone(),
            challenge_date: "2026-06-15".into(),
            block_id: block_id.clone(),
            module_id,
            track_id,
            block_type: "section".into(),
            started_at: None,
            completed_at: None,
        };
        insert_daily_challenge(&conn, &row).expect("insert ok");

        // Confirm row exists pre-delete
        let pre: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM daily_challenges WHERE block_id = ?1",
                params![&block_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pre, 1);

        // DELETE the module_blocks row — daily_challenges row must cascade-disappear
        conn.execute("DELETE FROM module_blocks WHERE id = ?1", params![&block_id])
            .expect("delete block ok");

        let post: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM daily_challenges WHERE block_id = ?1",
                params![&block_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            post, 0,
            "R5 — daily_challenges row must cascade-delete when module_blocks row is deleted"
        );
    }

    // ── learner_streaks ──

    /// MICRO-04 — first activity sets streak_days = 1.
    #[test]
    fn update_global_streak_first_activity_sets_1() {
        let conn = fresh_conn();
        let (learner_id, _, _, _) = seed_learner_track_module_block(&conn);

        let streak = update_global_streak(&conn, &learner_id).expect("ok");
        assert_eq!(streak, 1, "first activity ever must yield streak=1");

        // Verify row landed
        let row = get_learner_streak(&conn, &learner_id).expect("ok");
        assert_eq!(row.streak_days, 1);
        assert!(row.last_activity_date.is_some());
    }

    /// MICRO-04 — same calendar day is a no-op (Pitfall 2 critical).
    #[test]
    fn update_global_streak_same_day_idempotent() {
        let conn = fresh_conn();
        let (learner_id, _, _, _) = seed_learner_track_module_block(&conn);

        // Seed row with streak=3 + today's last_activity_date
        conn.execute(
            "INSERT INTO learner_streaks (learner_id, streak_days, last_activity_date, updated_at)
             VALUES (?1, 3, datetime('now', '-5 minutes'), datetime('now'))",
            params![&learner_id],
        )
        .unwrap();

        let streak = update_global_streak(&conn, &learner_id).expect("ok");
        assert_eq!(
            streak, 3,
            "same calendar day must be a no-op and return current streak unchanged"
        );
    }

    /// Branch 3 — within 24h different calendar day increments.
    #[test]
    fn update_global_streak_within_24h_increments() {
        let conn = fresh_conn();
        let (learner_id, _, _, _) = seed_learner_track_module_block(&conn);

        // Seed with last_activity_date = 23 hours ago (within 24h window)
        conn.execute(
            "INSERT INTO learner_streaks (learner_id, streak_days, last_activity_date, updated_at)
             VALUES (?1, 4, datetime('now', '-23 hours'), datetime('now'))",
            params![&learner_id],
        )
        .unwrap();

        // Branch only fires if 23h-ago is a DIFFERENT calendar day. If the test
        // runs early in the day this branch may collapse to same-day (Branch 2).
        // Mirror the v003 update_streak_within_24h test's defensive guard.
        let is_diff_day: bool = conn
            .query_row(
                "SELECT date(datetime('now', '-23 hours')) != date('now')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        let streak = update_global_streak(&conn, &learner_id).expect("ok");
        if is_diff_day {
            assert_eq!(streak, 5, "within-24h different-day must increment 4->5");
        } else {
            assert_eq!(streak, 4, "same calendar day no-op must keep streak at 4");
        }
    }

    /// R4 — gap > 24h hard-resets to 1 (NO Freeze logic).
    #[test]
    fn update_global_streak_gap_resets_to_1() {
        let conn = fresh_conn();
        let (learner_id, _, _, _) = seed_learner_track_module_block(&conn);

        // Seed with last_activity_date = 3 days ago
        conn.execute(
            "INSERT INTO learner_streaks (learner_id, streak_days, last_activity_date, updated_at)
             VALUES (?1, 7, datetime('now', '-3 days'), datetime('now'))",
            params![&learner_id],
        )
        .unwrap();

        let streak = update_global_streak(&conn, &learner_id).expect("ok");
        assert_eq!(streak, 1, "gap > 24h must hard-reset to 1 (R4 — no Freeze)");

        let stored: i32 = conn
            .query_row(
                "SELECT streak_days FROM learner_streaks WHERE learner_id = ?1",
                params![&learner_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, 1);
    }

    /// `get_learner_streak` returns synthetic zero for unknown learner.
    #[test]
    fn get_learner_streak_returns_synthetic_zero_for_unknown_learner() {
        let conn = fresh_conn();
        let row = get_learner_streak(&conn, "unknown-id").expect("ok");
        assert_eq!(row.learner_id, "unknown-id");
        assert_eq!(row.streak_days, 0);
        assert!(row.last_activity_date.is_none());
    }
}
