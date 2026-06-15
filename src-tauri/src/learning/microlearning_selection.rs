//! Microlearning daily-challenge selection algorithm (Phase 4 Wave 1 GREEN).
//!
//! Pure selection function over a SQLite connection. No `AppState`, no Mutex —
//! the IPC handler holds the lock and calls this directly. Mirrors the shape of
//! `learning::adaptive::update_mastery` (`adaptive.rs:33`) and
//! `commands::learning::update_streak` (`commands/learning.rs:968`).
//!
//! Algorithm (matches RESEARCH §"Selection Algorithm" sketch at lines 506-554):
//!
//!  1. Find candidate modules: mastery in [BKT_LOWER, BKT_UPPER), active track only.
//!  2. List eligible blocks per module: status='ready', block_type ∈ {flash_cards,quiz,section}.
//!  3. Apply 48h recency penalty per block — Q6 lock measures against daily_challenges only.
//!  4. SR-due signal per module (sr_cards.next_review <= now).
//!  5. BKT-decay signal per module (julianday('now') - julianday(last_bkt_update_at)).
//!  6. Pick highest-scoring block; tie-break on (ordering, block_id) for determinism.
//!  7. Return None when every candidate scored at or below W_RECENCY/2 (empty-zone fallback).

use crate::learning::adaptive::MASTERY_THRESHOLD;
use rusqlite::{params, Connection};

// ── Tuning constants (Q5 lock — `const`, not env vars) ──

/// BKT decay half-life in days. Used in `decay_days / DECAY_HALF_LIFE_DAYS`
/// scoring contribution. Plan 02 reads `module_progress.last_bkt_update_at`
/// (added by migration v007) to compute the elapsed delta.
pub const DECAY_HALF_LIFE_DAYS: f64 = 3.0;

/// Recency penalty window. Blocks seen in `daily_challenges` within this many
/// hours score `W_RECENCY` (effectively excluded).
pub const RECENCY_PENALTY_HOURS: i64 = 48;

/// Weight on the BKT-decay signal.
pub const W_DECAY: f64 = 1.0;

/// Weight on the SR-due signal (slight bias toward review).
pub const W_SR_DUE: f64 = 1.2;

/// Weight on the recency penalty (hard penalty if seen within
/// `RECENCY_PENALTY_HOURS`).
pub const W_RECENCY: f64 = -100.0;

/// Lower bound of the BKT candidate window (D-05 — "struggle zone").
pub const BKT_LOWER: f64 = 0.3;

/// Upper bound of the BKT candidate window. Reuses
/// `learning::adaptive::MASTERY_THRESHOLD = 0.7` as the single source of truth
/// for the mastered/not-mastered boundary.
pub const BKT_UPPER: f64 = MASTERY_THRESHOLD;

/// Cap on the decay-multiplier contribution. Without a cap, a module that
/// hasn't been touched in months would dominate any SR-due signal forever.
pub const DECAY_DAYS_CAP_MULT: f64 = 5.0;

/// Result of the selection algorithm. NOT serialized — internal type that the
/// IPC layer translates into a `DailyChallengePayload` for the frontend.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub block_id: String,
    pub module_id: String,
    pub track_id: String,
    pub block_type: String,
    pub score: f64,
}

/// Lightweight projection of `module_progress` joined with `modules` +
/// `learning_paths` + `learning_tracks`. Used by the algorithm internally.
///
/// `mastery_level` + `last_bkt_update_at` are returned from the SQL select
/// (Step 1) for debugging visibility and potential future re-scoring; the
/// active algorithm reads `last_bkt_update_at` via `decay_days_for_module`
/// (which does its own julianday math in SQL) rather than parsing it in Rust.
#[allow(dead_code)]
struct CandidateModule {
    module_id: String,
    track_id: String,
    mastery_level: f64,
    last_bkt_update_at: Option<String>,
}

// ── Helpers (each independently testable) ──

/// Step 1 — fetch all modules in the [BKT_LOWER, BKT_UPPER) zone for an
/// **active** track. Mastered modules (>= 0.7) and never-seen modules
/// (no module_progress row) are naturally excluded by the WHERE clause.
fn fetch_candidate_modules(
    conn: &Connection,
    learner_id: &str,
) -> Result<Vec<CandidateModule>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT mp.module_id, lp.track_id, mp.mastery_level, mp.last_bkt_update_at
               FROM module_progress mp
               JOIN modules m         ON m.id = mp.module_id
               JOIN learning_paths lp ON lp.id = m.path_id
               JOIN learning_tracks lt ON lt.id = lp.track_id
              WHERE mp.learner_id = ?1
                AND lt.status = 'active'
                AND mp.mastery_level >= ?2
                AND mp.mastery_level <  ?3",
        )
        .map_err(|e| format!("fetch_candidate_modules: prepare failed: {}", e))?;

    let rows = stmt
        .query_map(params![learner_id, BKT_LOWER, BKT_UPPER], |row| {
            Ok(CandidateModule {
                module_id: row.get(0)?,
                track_id: row.get(1)?,
                mastery_level: row.get(2)?,
                last_bkt_update_at: row.get(3)?,
            })
        })
        .map_err(|e| format!("fetch_candidate_modules: query failed: {}", e))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("fetch_candidate_modules: row failed: {}", e))?);
    }
    Ok(out)
}

/// Step 2 — list eligible blocks for a module. Returns `(block_id, block_type, ordering)`.
fn fetch_blocks_for_module(
    conn: &Connection,
    module_id: &str,
) -> Result<Vec<(String, String, i32)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, block_type, ordering
               FROM module_blocks
              WHERE module_id = ?1
                AND status = 'ready'
                AND block_type IN ('flash_cards', 'quiz', 'section')",
        )
        .map_err(|e| format!("fetch_blocks_for_module: prepare failed: {}", e))?;

    let rows = stmt
        .query_map(params![module_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })
        .map_err(|e| format!("fetch_blocks_for_module: query failed: {}", e))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("fetch_blocks_for_module: row failed: {}", e))?);
    }
    Ok(out)
}

/// Step 3 — was this block shown to this learner via daily_challenges in the
/// last `RECENCY_PENALTY_HOURS` hours? Q6 lock — measure against
/// `daily_challenges` history ONLY (not other surfaces).
fn is_recently_seen(
    conn: &Connection,
    learner_id: &str,
    block_id: &str,
) -> Result<bool, String> {
    // SQLite doesn't bind 48 as a placeholder cleanly into datetime('-X hours');
    // build the modifier from the const at call time. Safe — no user input.
    let modifier = format!("-{} hours", RECENCY_PENALTY_HOURS);
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM daily_challenges
              WHERE learner_id = ?1
                AND block_id = ?2
                AND created_at >= datetime('now', ?3)",
            params![learner_id, block_id, modifier],
            |row| row.get(0),
        )
        .map_err(|e| format!("is_recently_seen: {}", e))?;
    Ok(count > 0)
}

/// Step 4 — does this module have at least one SR card due now?
fn module_has_due_sr_card(conn: &Connection, module_id: &str) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sr_cards
              WHERE module_id = ?1 AND next_review <= datetime('now')",
            params![module_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("module_has_due_sr_card: {}", e))?;
    Ok(count > 0)
}

/// Step 5 — days since the BKT update last fired for this module/learner.
/// Returns `0.0` when `last_bkt_update_at` is NULL (cold module — Phase 4
/// treats "warm" as the safe default; no decay penalty or bonus).
fn decay_days_for_module(
    conn: &Connection,
    learner_id: &str,
    module_id: &str,
) -> Result<f64, String> {
    let raw: Option<f64> = conn
        .query_row(
            "SELECT julianday('now') - julianday(last_bkt_update_at)
               FROM module_progress
              WHERE module_id = ?1 AND learner_id = ?2",
            params![module_id, learner_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("decay_days_for_module: {}", e))?;
    Ok(raw.unwrap_or(0.0).max(0.0))
}

// ── Public selection function ──

/// Picks today's micro-challenge block for a learner.
///
/// Returns `Ok(None)` when no candidate exists (empty 0.3–0.7 BKT zone) OR
/// when every candidate block was already seen within `RECENCY_PENALTY_HOURS`
/// — Q3 fallback the frontend renders as the "no challenge today" placeholder.
pub fn select_daily_challenge(
    conn: &Connection,
    learner_id: &str,
) -> Result<Option<Candidate>, String> {
    let modules = fetch_candidate_modules(conn, learner_id)?;
    if modules.is_empty() {
        return Ok(None);
    }

    // Per-module signals
    let mut scored: Vec<(Candidate, i32)> = Vec::new(); // (cand, ordering for tie-break)

    for cm in &modules {
        let blocks = fetch_blocks_for_module(conn, &cm.module_id)?;
        if blocks.is_empty() {
            continue;
        }

        let sr_due = module_has_due_sr_card(conn, &cm.module_id)?;
        let decay_days = decay_days_for_module(conn, learner_id, &cm.module_id)?;
        let decay_contrib =
            W_DECAY * (decay_days / DECAY_HALF_LIFE_DAYS).min(DECAY_DAYS_CAP_MULT);
        let sr_contrib = if sr_due { W_SR_DUE } else { 0.0 };
        let module_base_score = decay_contrib + sr_contrib;

        for (block_id, block_type, ordering) in blocks {
            let recency_penalty = if is_recently_seen(conn, learner_id, &block_id)? {
                W_RECENCY
            } else {
                0.0
            };
            let score = module_base_score + recency_penalty;
            scored.push((
                Candidate {
                    block_id,
                    module_id: cm.module_id.clone(),
                    track_id: cm.track_id.clone(),
                    block_type,
                    score,
                },
                ordering,
            ));
        }
    }

    if scored.is_empty() {
        return Ok(None);
    }

    // Q3 empty-zone fallback — every block was recency-penalized. We treat
    // W_RECENCY/2 as the cutoff because even a maxed-out decay+sr_due contribution
    // (W_SR_DUE + W_DECAY*DECAY_DAYS_CAP_MULT ≈ 6.2) cannot bring a recency-
    // penalized block (W_RECENCY = -100) above this line.
    if scored.iter().all(|(c, _)| c.score <= W_RECENCY / 2.0) {
        return Ok(None);
    }

    // Pick highest score; deterministic tie-break on (ordering asc, block_id asc).
    scored.sort_by(|a, b| {
        b.0
            .score
            .partial_cmp(&a.0.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.0.block_id.cmp(&b.0.block_id))
    });

    Ok(Some(scored.into_iter().next().unwrap().0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
        crate::db::migrations::apply_migrations(&conn).unwrap();
        conn
    }

    /// Seed `learner_profiles -> learning_tracks (active) -> learning_paths ->
    /// modules`. Returns ids: (learner, track, path, module).
    fn seed_active_learner_path_module(conn: &Connection) -> (String, String, String, String) {
        let learner_id = "learner-1".to_string();
        let track_id = "trk-1".to_string();
        let path_id = "pth-1".to_string();
        let module_id = "mod-1".to_string();

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'Tester')",
            params![&learner_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal, status)
             VALUES (?1, ?2, 'Rust', 'programming', 'Learn Rust', 'active')",
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

        (learner_id, track_id, path_id, module_id)
    }

    fn insert_module_progress(
        conn: &Connection,
        learner_id: &str,
        module_id: &str,
        mastery: f64,
        last_bkt_update_at: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO module_progress
                (id, module_id, learner_id, status, mastery_level, last_bkt_update_at)
             VALUES (?1, ?2, ?3, 'in_progress', ?4, ?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                module_id,
                learner_id,
                mastery,
                last_bkt_update_at,
            ],
        )
        .unwrap();
    }

    fn insert_block(
        conn: &Connection,
        block_id: &str,
        module_id: &str,
        ordering: i32,
        block_type: &str,
        status: &str,
    ) {
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![block_id, module_id, ordering, block_type, status],
        )
        .unwrap();
    }

    // ── Integration tests for select_daily_challenge ──

    /// MICRO-02 — module in the [0.3, 0.7) BKT zone is selected.
    #[test]
    fn selects_block_in_bkt_zone() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");

        let result = select_daily_challenge(&conn, &learner_id).expect("ok");
        let cand = result.expect("must return Some(Candidate) when module is in zone");
        assert_eq!(cand.module_id, module_id);
        assert_eq!(cand.block_id, "blk-1");
    }

    /// MICRO-02 — mastered modules (mastery >= 0.7) are excluded.
    #[test]
    fn excludes_mastered_modules() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.8, None);
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");

        let result = select_daily_challenge(&conn, &learner_id).expect("ok");
        assert!(
            result.is_none(),
            "mastered modules (>=0.7) must not produce a candidate"
        );
    }

    /// MICRO-02 — modules with no module_progress row (never-seen) are excluded.
    #[test]
    fn excludes_never_seen_modules() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        // NO insert_module_progress — the JOIN with module_progress eliminates this row.
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");

        let result = select_daily_challenge(&conn, &learner_id).expect("ok");
        assert!(
            result.is_none(),
            "never-seen modules (no module_progress row) must not produce a candidate"
        );
    }

    /// MICRO-02 — recency penalty: a block seen in daily_challenges within
    /// `RECENCY_PENALTY_HOURS` is excluded; the same module's other block is
    /// picked instead.
    #[test]
    fn applies_recency_penalty_within_48h() {
        let conn = fresh_conn();
        let (learner_id, track_id, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");
        insert_block(&conn, "blk-2", &module_id, 1, "section", "ready");

        // Seed a daily_challenges row that "showed" blk-1 1 hour ago
        conn.execute(
            "INSERT INTO daily_challenges
                (learner_id, challenge_date, block_id, module_id, track_id, block_type, created_at)
             VALUES (?1, '2026-06-14', ?2, ?3, ?4, 'section', datetime('now', '-1 hours'))",
            params![&learner_id, "blk-1", &module_id, &track_id],
        )
        .unwrap();

        let cand = select_daily_challenge(&conn, &learner_id)
            .expect("ok")
            .expect("must return the un-penalized block");
        assert_eq!(
            cand.block_id, "blk-2",
            "recently-seen blk-1 must be deprioritized; blk-2 picked instead"
        );
    }

    /// MICRO-02 — when two candidate modules tie on decay, the one with an
    /// SR-due card scores higher (W_SR_DUE adds positive weight).
    #[test]
    fn prefers_sr_due_modules() {
        let conn = fresh_conn();
        let (learner_id, _, path_id, _) = seed_active_learner_path_module(&conn);

        // Override mod-1 → mod-A, and add mod-B
        conn.execute("UPDATE modules SET id = 'mod-A' WHERE id = 'mod-1'", [])
            .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('mod-B', ?1, 'Module B')",
            params![&path_id],
        )
        .unwrap();

        // Same mastery + same last_bkt_update_at on both
        insert_module_progress(&conn, &learner_id, "mod-A", 0.5, None);
        insert_module_progress(&conn, &learner_id, "mod-B", 0.5, None);
        insert_block(&conn, "blk-A", "mod-A", 0, "section", "ready");
        insert_block(&conn, "blk-B", "mod-B", 0, "section", "ready");

        // mod-A has an SR card due NOW
        conn.execute(
            "INSERT INTO sr_cards (id, module_id, concept, front, back, next_review)
             VALUES ('sr-1', 'mod-A', 'concept-x', 'front', 'back', datetime('now', '-1 minutes'))",
            [],
        )
        .unwrap();

        let cand = select_daily_challenge(&conn, &learner_id)
            .expect("ok")
            .expect("must return the SR-due module's block");
        assert_eq!(cand.module_id, "mod-A");
        assert_eq!(cand.block_id, "blk-A");
    }

    /// Determinism — two ready blocks with equal scoring signals: smaller
    /// `ordering` wins.
    #[test]
    fn picks_block_with_lowest_ordering_on_tie() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);
        // Insert blocks in REVERSE ordering on purpose to confirm sort, not insertion order, wins
        insert_block(&conn, "blk-late", &module_id, 5, "section", "ready");
        insert_block(&conn, "blk-early", &module_id, 1, "section", "ready");

        let cand = select_daily_challenge(&conn, &learner_id)
            .expect("ok")
            .expect("ok");
        assert_eq!(
            cand.block_id, "blk-early",
            "tie-break: smaller ordering must win (got {:?})",
            cand
        );
    }

    // ── Helper-level micro-tests ──

    #[test]
    fn fetch_candidate_modules_filters_by_track_status() {
        let conn = fresh_conn();
        let (learner_id, track_id, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);
        // Active — should appear
        let active = fetch_candidate_modules(&conn, &learner_id).expect("ok");
        assert_eq!(active.len(), 1);

        // Flip track to archived — should disappear
        conn.execute(
            "UPDATE learning_tracks SET status = 'archived' WHERE id = ?1",
            params![&track_id],
        )
        .unwrap();
        let archived = fetch_candidate_modules(&conn, &learner_id).expect("ok");
        assert_eq!(
            archived.len(),
            0,
            "archived tracks must not contribute candidates"
        );
    }

    #[test]
    fn fetch_blocks_for_module_filters_by_status_and_type() {
        let conn = fresh_conn();
        let (_learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        // Eligible
        insert_block(&conn, "blk-ready-section", &module_id, 0, "section", "ready");
        insert_block(&conn, "blk-ready-quiz", &module_id, 1, "quiz", "ready");
        // Wrong status
        insert_block(&conn, "blk-pending", &module_id, 2, "section", "pending");
        // Wrong type
        insert_block(&conn, "blk-lab", &module_id, 3, "lab", "ready");

        let blocks = fetch_blocks_for_module(&conn, &module_id).expect("ok");
        let ids: Vec<&str> = blocks.iter().map(|(id, _, _)| id.as_str()).collect();
        assert!(ids.contains(&"blk-ready-section"));
        assert!(ids.contains(&"blk-ready-quiz"));
        assert!(!ids.contains(&"blk-pending"));
        assert!(!ids.contains(&"blk-lab"));
    }

    #[test]
    fn is_recently_seen_true_within_window_false_after() {
        let conn = fresh_conn();
        let (learner_id, track_id, _, module_id) = seed_active_learner_path_module(&conn);
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");
        insert_block(&conn, "blk-2", &module_id, 1, "section", "ready");

        // blk-1 was seen 1 hour ago — recent
        conn.execute(
            "INSERT INTO daily_challenges
                (learner_id, challenge_date, block_id, module_id, track_id, block_type, created_at)
             VALUES (?1, '2026-06-14', 'blk-1', ?2, ?3, 'section', datetime('now', '-1 hours'))",
            params![&learner_id, &module_id, &track_id],
        )
        .unwrap();
        // blk-2 was seen 50 hours ago — outside the 48h window
        conn.execute(
            "INSERT INTO daily_challenges
                (learner_id, challenge_date, block_id, module_id, track_id, block_type, created_at)
             VALUES (?1, '2026-06-12', 'blk-2', ?2, ?3, 'section', datetime('now', '-50 hours'))",
            params![&learner_id, &module_id, &track_id],
        )
        .unwrap();

        assert!(is_recently_seen(&conn, &learner_id, "blk-1").unwrap());
        assert!(!is_recently_seen(&conn, &learner_id, "blk-2").unwrap());
    }

    #[test]
    fn decay_days_for_module_returns_zero_for_null_last_bkt_update_at() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);
        let days = decay_days_for_module(&conn, &learner_id, &module_id).expect("ok");
        assert_eq!(days, 0.0, "NULL last_bkt_update_at must produce 0.0 decay");
    }
}
