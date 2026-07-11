//! # commands::labs::exam_entry — per-track exam-flag entry-point data
//! (Phase 19, 19-04)
//!
//! `exam_blocks_for_track(track_id)` resolves the track's modules
//! (`learning_paths.track_id` → `modules.path_id`) and, for each module,
//! finds the FIRST `lab`-type `module_blocks` row whose parsed
//! `LabSpec.exam` is `Some(_)` (D-02 — only authored exam specs). Modules
//! with no exam-flagged block are omitted entirely (fail-closed — TrackView
//! only renders a Start Exam entry point where an exam actually exists).
//!
//! Reuses the single promoted `read_lab_spec_conn` helper (19-03,
//! `commands::labs::read_lab_spec_conn`) for payload parsing — this module
//! does NOT define a fourth spec-parse copy (T-19-11 also requires that a
//! single malformed lab payload never break the whole-track query; a
//! parse failure here is skipped, not propagated).

use crate::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamBlocksForTrackRequest {
    pub track_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExamBlockRef {
    pub module_id: String,
    pub block_id: String,
}

/// `Connection`-based inner helper (test seam — mirrors
/// `exam.rs::exam_attempt_start_conn`'s pattern so this can be exercised
/// against an in-memory SQLite connection without a `tauri::State`).
///
/// T-19-03: `track_id`/`module_id` are always bound as rusqlite params,
/// never string-formatted into SQL.
/// T-19-11: a `module_blocks.payload_json` row that fails to parse via
/// `read_lab_spec_conn` is skipped (log-and-continue) — never propagated
/// with `?`, so one bad block cannot break the query for the whole track.
pub(crate) fn exam_blocks_for_track_conn(
    conn: &Connection,
    track_id: &str,
) -> Result<Vec<ExamBlockRef>, String> {
    let mut module_stmt = conn
        .prepare(
            "SELECT m.id FROM modules m
             JOIN learning_paths lp ON lp.id = m.path_id
             WHERE lp.track_id = ?1
             ORDER BY m.ordering ASC",
        )
        .map_err(|e| format!("prepare modules query: {}", e))?;
    let module_ids: Vec<String> = module_stmt
        .query_map([track_id], |r| r.get::<_, String>(0))
        .map_err(|e| format!("query modules for track: {}", e))?
        .filter_map(Result::ok)
        .collect();

    let mut out = Vec::new();

    for module_id in &module_ids {
        let mut block_stmt = conn
            .prepare(
                "SELECT id FROM module_blocks
                 WHERE module_id = ?1 AND block_type = 'lab'
                 ORDER BY ordering ASC",
            )
            .map_err(|e| format!("prepare lab blocks query: {}", e))?;
        let block_ids: Vec<String> = block_stmt
            .query_map([module_id.as_str()], |r| r.get::<_, String>(0))
            .map_err(|e| format!("query lab blocks for module: {}", e))?
            .filter_map(Result::ok)
            .collect();

        for block_id in block_ids {
            // A malformed/unparseable lab payload is skipped, not fatal
            // (T-19-11) — never `?` this out.
            let spec = match super::read_lab_spec_conn(conn, &block_id) {
                Ok((spec, _body)) => spec,
                Err(e) => {
                    log::warn!(
                        "exam_blocks_for_track: skipping unparseable lab block {}: {}",
                        block_id,
                        e
                    );
                    continue;
                }
            };
            if spec.exam.is_some() {
                out.push(ExamBlockRef {
                    module_id: module_id.clone(),
                    block_id,
                });
                // First exam-flagged block wins for this module.
                break;
            }
        }
    }

    Ok(out)
}

#[tauri::command]
pub async fn exam_blocks_for_track(
    request: ExamBlocksForTrackRequest,
    state: State<'_, AppState>,
) -> Result<Vec<ExamBlockRef>, String> {
    let db = state.db.lock().map_err(|e| format!("db lock: {}", e))?;
    exam_blocks_for_track_conn(&db.conn, &request.track_id)
}

#[cfg(test)]
#[path = "exam_entry_tests.rs"]
mod tests;
