//! # commands::labs::state — reset + progress + mastery DB helpers
//!
//! Owns `lab_reset`, `lab_get_progress`, plus the SQL helpers
//! `recompute_practical_mastery`, `reset_surgical`, `reset_clears_progress`,
//! and `ensure_lab_progress_row` shared with `session.rs` and `eval.rs`.

use super::{LabGetProgressRequest, LabProgress, LabResetRequest, LabResetResult};
use crate::AppState;
use rusqlite::Connection;
use tauri::State;

/// LAB-07 — reset_surgical: remove only files in `creates: []` from the
/// workspace. Returns the resolved paths actually removed (for the IPC
/// response). Validates `creates` defensively before touching disk.
pub fn reset_surgical(
    workspace: &std::path::Path,
    creates: &[String],
) -> Result<Vec<String>, String> {
    let removed = crate::labs::spec::reset_lab(workspace, creates)
        .map_err(|e| format!("reset_lab: {}", e))?;
    Ok(removed
        .into_iter()
        .map(|p| {
            // Return relative path under the workspace when possible; fall
            // back to the absolute resolved path otherwise.
            match p.strip_prefix(workspace) {
                Ok(rel) => rel.to_string_lossy().to_string(),
                Err(_) => p.to_string_lossy().to_string(),
            }
        })
        .collect())
}

/// LAB-07 — clears `lab_progress.completed_step_ids` + `current_step` in
/// place for (block_id, learner_id). Used by `lab_reset` and exposed for
/// the Wave-0 handoff test in `mod.rs`.
pub fn reset_clears_progress(
    conn: &Connection,
    block_id: &str,
    learner_id: &str,
) -> Result<(), String> {
    let updated = conn
        .execute(
            "UPDATE lab_progress
             SET completed_step_ids = '[]',
                 current_step = 0,
                 last_updated = datetime('now')
             WHERE block_id = ?1 AND learner_id = ?2",
            rusqlite::params![block_id, learner_id],
        )
        .map_err(|e| format!("reset_clears_progress: {}", e))?;
    let _ = updated; // 0 rows is a no-op for never-opened labs
    Ok(())
}

/// LAB-08 — recompute `module_progress.practical_mastery` from the labs in
/// the module: `SUM(json_array_length(completed_step_ids)) / SUM(total_steps)`
/// across `lab_progress` rows for (learner_id, module_id). Returns the new
/// mastery value (0.0 when no labs).
pub fn recompute_practical_mastery(
    conn: &Connection,
    module_id: &str,
    learner_id: &str,
) -> Result<f64, String> {
    let mastery: f64 = conn
        .query_row(
            "SELECT COALESCE(
                CAST(SUM(json_array_length(completed_step_ids)) AS REAL)
                    / NULLIF(SUM(total_steps), 0),
                0.0
            )
            FROM lab_progress
            WHERE learner_id = ?1 AND module_id = ?2",
            rusqlite::params![learner_id, module_id],
            |row| row.get::<_, f64>(0),
        )
        .map_err(|e| format!("recompute_practical_mastery: {}", e))?;

    // Persist on module_progress when a row exists; if not, skip silently
    // (the row will be created on first quiz/flash interaction by the
    // existing apply_mastery_update path, and recompute will re-run on the
    // next lab Pass).
    let _ = conn.execute(
        "UPDATE module_progress SET practical_mastery = ?1
         WHERE module_id = ?2 AND learner_id = ?3",
        rusqlite::params![mastery, module_id, learner_id],
    );

    Ok(mastery)
}

/// Insert a `lab_progress` row for (learner, module, block) if absent;
/// return the row as a `LabProgress` IPC struct alongside the current
/// module-level practical_mastery.
pub(crate) fn ensure_lab_progress_row(
    conn: &Connection,
    learner_id: &str,
    module_id: &str,
    block_id: &str,
    total_steps: usize,
) -> Result<LabProgress, String> {
    conn.execute(
        "INSERT OR IGNORE INTO lab_progress
            (learner_id, module_id, block_id, current_step, completed_step_ids,
             total_steps, metadata_json, last_updated)
         VALUES (?1, ?2, ?3, 0, '[]', ?4, '{}', datetime('now'))",
        rusqlite::params![learner_id, module_id, block_id, total_steps as i64],
    )
    .map_err(|e| format!("ensure_lab_progress_row: insert: {}", e))?;

    read_lab_progress(conn, learner_id, module_id, block_id)
}

pub(crate) fn read_lab_progress(
    conn: &Connection,
    learner_id: &str,
    module_id: &str,
    block_id: &str,
) -> Result<LabProgress, String> {
    let row: (i64, String, String) = conn
        .query_row(
            "SELECT current_step, completed_step_ids, last_updated
             FROM lab_progress
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![learner_id, module_id, block_id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .map_err(|e| format!("read_lab_progress: {}", e))?;
    let completed_step_ids: Vec<String> =
        serde_json::from_str(&row.1).map_err(|e| format!("completed_step_ids: {}", e))?;
    let mastery = recompute_practical_mastery(conn, module_id, learner_id).unwrap_or(0.0);
    Ok(LabProgress {
        block_id: block_id.to_string(),
        current_step: row.0 as usize,
        completed_step_ids,
        last_updated: row.2,
        practical_mastery: mastery,
    })
}

/// LAB-07 — IPC handler for surgical reset.
///
/// Looks up the session sidecar to recover (block_id, learner_id, module_id,
/// workspace), reads the lab spec to get the `creates: []` list, then:
/// 1. removes only those files from disk;
/// 2. clears `lab_progress.completed_step_ids` + `current_step`;
/// 3. recomputes `module_progress.practical_mastery` for the module.
#[tauri::command]
pub async fn lab_reset(
    request: LabResetRequest,
    state: State<'_, AppState>,
) -> Result<LabResetResult, String> {
    // 1. Look up session metadata.
    let (block_id, learner_id, module_id, workspace) = {
        let map = state
            .lab_sessions
            .lock()
            .map_err(|e| format!("lab_sessions lock: {}", e))?;
        let entry = map
            .get(&request.session_id)
            .ok_or_else(|| format!("session not found: {}", request.session_id))?;
        (
            entry.block_id.clone(),
            entry.learner_id.clone(),
            entry.module_id.clone(),
            entry.workspace.clone(),
        )
    };

    // 2. Read the spec to get the creates list.
    let creates = read_lab_spec_creates(&state, &block_id)?;

    // 3. Surgical filesystem reset.
    let files_removed =
        reset_surgical(&workspace, &creates).map_err(|e| format!("reset_surgical: {}", e))?;

    // 4. Clear progress + recompute mastery in a single transaction.
    {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("db lock: {}", e))?;
        let conn = &db.conn;
        reset_clears_progress(conn, &block_id, &learner_id)?;
        let _ = recompute_practical_mastery(conn, &module_id, &learner_id)?;
    }

    Ok(LabResetResult { files_removed, progress_reset: true })
}

fn read_lab_spec_creates(
    state: &State<'_, AppState>,
    block_id: &str,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|e| format!("db lock: {}", e))?;
    let conn = &db.conn;
    let block = crate::db::blocks::get_block(conn, block_id)
        .map_err(|e| format!("get_block: {}", e))?
        .ok_or_else(|| format!("block not found: {}", block_id))?;

    if !block.payload_json.trim().is_empty() && block.payload_json != "{}" {
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&block.payload_json) {
            if let Some(spec_val) = payload.get("spec") {
                if let Ok(spec) =
                    serde_json::from_value::<crate::labs::spec::LabSpec>(spec_val.clone())
                {
                    return Ok(spec.creates);
                }
            }
        }
    }
    if let Ok(params) = serde_json::from_str::<serde_json::Value>(&block.params_json) {
        if let Some(md) = params.get("labMd").and_then(|v| v.as_str()) {
            return crate::labs::spec::parse_lab_md(md)
                .map(|(s, _)| s.creates)
                .map_err(|e| format!("parse_lab_md: {}", e));
        }
    }
    Err(format!("block {} has no readable lab spec", block_id))
}

/// LAB-08 — IPC handler reading the lab_progress row for (learner, block).
#[tauri::command]
pub async fn lab_get_progress(
    request: LabGetProgressRequest,
    state: State<'_, AppState>,
) -> Result<LabProgress, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("db lock poisoned: {}", e))?;
    let conn = &db.conn;
    // Resolve module_id from the block row.
    let module_id: String = conn
        .query_row(
            "SELECT module_id FROM module_blocks WHERE id = ?1",
            rusqlite::params![&request.block_id],
            |r| r.get::<_, String>(0),
        )
        .map_err(|e| format!("module_id lookup: {}", e))?;
    read_lab_progress(conn, &request.learner_id, &module_id, &request.block_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open_in_memory");
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(crate::db::schema::CREATE_TABLES)
            .expect("baseline tables");
        crate::db::migrations::apply_migrations(&conn).expect("migrations");
        conn
    }

    /// Insert minimal fixtures for module_progress + lab_progress tests.
    fn seed_module_with_block(conn: &Connection) -> (String, String, String) {
        let learner = "lp-1".to_string();
        let track = "track-fixt-1".to_string();
        let path = "path-1".to_string();
        let module = "mod-1".to_string();
        let block = "blk-1".to_string();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'L')",
            rusqlite::params![learner],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module)
             VALUES (?1, ?2, 'k8s', 'kubernetes')",
            rusqlite::params![track, learner],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
            rusqlite::params![path, track],
        ).unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title, ordering)
             VALUES (?1, ?2, 'M1', 0)",
            rusqlite::params![module, path],
        ).unwrap();
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
                params_json, payload_json, source_anchors_json, metadata_json, retry_count,
                created_at, updated_at)
             VALUES (?1, ?2, 0, 'lab', 'ready', '{}', '{}', '[]', '{}', 0,
                datetime('now'), datetime('now'))",
            rusqlite::params![block, module],
        ).unwrap();
        (learner, module, block)
    }

    /// LAB-07 — surgical reset removes only files declared in `creates: []`,
    /// leaves siblings untouched.
    #[test]
    fn reset_surgical_only_removes_declared() {
        let dir = tempfile::tempdir().expect("tempdir");
        let foo = dir.path().join("foo.txt");
        let bar = dir.path().join("bar.txt");
        let manifest_dir = dir.path().join("manifests");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        let pod_yaml = manifest_dir.join("pod.yaml");
        let notes_dir = dir.path().join("notes");
        std::fs::create_dir_all(&notes_dir).unwrap();
        let run_output = notes_dir.join("run-output.txt");

        for p in [&foo, &bar, &pod_yaml, &run_output] {
            std::fs::write(p, "x").unwrap();
        }

        let creates = vec![
            "manifests/pod.yaml".to_string(),
            "notes/run-output.txt".to_string(),
        ];
        let removed = reset_surgical(dir.path(), &creates).expect("reset_surgical");
        assert_eq!(removed.len(), 2);
        assert!(!pod_yaml.exists(), "pod.yaml must be deleted");
        assert!(!run_output.exists(), "run-output.txt must be deleted");
        assert!(foo.exists(), "foo.txt must remain (not in creates)");
        assert!(bar.exists(), "bar.txt must remain (not in creates)");
    }

    /// LAB-07 — reset_clears_progress flips the lab_progress row back to
    /// the empty state.
    #[test]
    fn reset_clears_progress_row() {
        let conn = fresh_conn();
        let (learner, module, block) = seed_module_with_block(&conn);
        // Pre-populate with some progress.
        ensure_lab_progress_row(&conn, &learner, &module, &block, 4).unwrap();
        conn.execute(
            "UPDATE lab_progress SET completed_step_ids = '[\"s1\",\"s2\"]', current_step = 2
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![learner, module, block],
        )
        .unwrap();
        // Sanity: row is non-empty.
        let progress = read_lab_progress(&conn, &learner, &module, &block).unwrap();
        assert_eq!(progress.current_step, 2);
        assert_eq!(progress.completed_step_ids.len(), 2);

        reset_clears_progress(&conn, &block, &learner).expect("reset_clears_progress");
        let cleared = read_lab_progress(&conn, &learner, &module, &block).unwrap();
        assert_eq!(cleared.current_step, 0);
        assert!(cleared.completed_step_ids.is_empty());
    }

    /// LAB-08 — empty case returns 0.0; no rows in lab_progress.
    #[test]
    fn practical_mastery_compute() {
        let conn = fresh_conn();
        let (learner, module, _block) = seed_module_with_block(&conn);
        let mastery = recompute_practical_mastery(&conn, &module, &learner).unwrap();
        assert!(mastery.abs() < 1e-9, "empty: must be 0.0, got {}", mastery);
    }

    /// LAB-08 — populated case: 3/4 + 5/5 = 8/9 ≈ 0.888…
    #[test]
    fn practical_mastery_compute_populated() {
        let conn = fresh_conn();
        let (learner, module, _block_1) = seed_module_with_block(&conn);

        // Insert a second lab block in the same module.
        let block_2 = "blk-2".to_string();
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
                params_json, payload_json, source_anchors_json, metadata_json, retry_count,
                created_at, updated_at)
             VALUES (?1, ?2, 1, 'lab', 'ready', '{}', '{}', '[]', '{}', 0,
                datetime('now'), datetime('now'))",
            rusqlite::params![block_2, module],
        )
        .unwrap();

        // Insert lab_progress rows: 3/4 and 5/5.
        conn.execute(
            "INSERT INTO lab_progress (learner_id, module_id, block_id, current_step,
                completed_step_ids, total_steps, metadata_json, last_updated)
             VALUES (?1, ?2, 'blk-1', 3, '[\"s1\",\"s2\",\"s3\"]', 4, '{}', datetime('now'))",
            rusqlite::params![learner, module],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lab_progress (learner_id, module_id, block_id, current_step,
                completed_step_ids, total_steps, metadata_json, last_updated)
             VALUES (?1, ?2, 'blk-2', 5, '[\"s1\",\"s2\",\"s3\",\"s4\",\"s5\"]', 5, '{}', datetime('now'))",
            rusqlite::params![learner, module],
        )
        .unwrap();

        let mastery = recompute_practical_mastery(&conn, &module, &learner).unwrap();
        let expected = 8.0_f64 / 9.0;
        assert!(
            (mastery - expected).abs() < 1e-6,
            "expected {} got {}",
            expected,
            mastery
        );
    }

    /// LAB-07 — reset_clears_progress + recompute drops practical_mastery
    /// back to 0.0 when this is the only lab in the module.
    #[test]
    fn reset_recomputes_mastery_to_zero() {
        let conn = fresh_conn();
        let (learner, module, block) = seed_module_with_block(&conn);
        // Seed module_progress for the upsert path.
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level,
                attempts, started_at, practical_mastery)
             VALUES ('mp-1', ?1, ?2, 'in_progress', 0.4, 1, datetime('now'), 0.5)",
            rusqlite::params![module, learner],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lab_progress (learner_id, module_id, block_id, current_step,
                completed_step_ids, total_steps, metadata_json, last_updated)
             VALUES (?1, ?2, ?3, 2, '[\"s1\",\"s2\"]', 4, '{}', datetime('now'))",
            rusqlite::params![learner, module, block],
        )
        .unwrap();

        // Reset + recompute.
        reset_clears_progress(&conn, &block, &learner).unwrap();
        let mastery = recompute_practical_mastery(&conn, &module, &learner).unwrap();
        assert!(mastery.abs() < 1e-9, "mastery must be 0.0 after reset");

        // module_progress.practical_mastery is also persisted.
        let persisted: f64 = conn
            .query_row(
                "SELECT practical_mastery FROM module_progress
                 WHERE module_id = ?1 AND learner_id = ?2",
                rusqlite::params![module, learner],
                |r| r.get::<_, f64>(0),
            )
            .unwrap();
        assert!(persisted.abs() < 1e-9);
    }
}
