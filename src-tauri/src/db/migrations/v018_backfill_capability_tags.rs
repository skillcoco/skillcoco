//! Migration v018 — backfill `capability_tags` from existing `modules.skills_json`
//!
//! CR-01 gap closure (Phase 18, 18-08): prior to this migration, no production
//! code path ever wrote into `capability_tags` — `ai.rs::build_skills_json`
//! only persisted tags into `modules.skills_json`, and
//! `SqliteReportStore::capability_tags_for_scope` reads exclusively from
//! `capability_tags`, so every AI-generated track created BEFORE the 18-08
//! writer landed silently falls back to the D-03.4 module-title capability.
//!
//! This one-shot migration backfills `capability_tags` for every existing
//! module whose `skills_json` is already non-empty, so those pre-existing
//! tracks report by AI-authored capability too, not just newly generated ones.
//!
//! Idempotent: a `NOT EXISTS` guard on `(module_id, tag_slug, learner_id)`
//! means re-running `up()` — or running it on a DB where the 18-08 writer
//! already inserted the same rows for freshly generated tracks — never
//! duplicates rows.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 18;
pub const NAME: &str = "backfill_capability_tags";

/// Apply the v018 migration.
///
/// For every `modules` row with a non-empty `skills_json`, resolve:
/// - `track_id` via the owning `learning_paths` row
/// - `learner_id` via `module_progress` for that module (falling back to the
///   first `learner_profiles` row when no progress row exists yet — mirrors
///   the desktop single-learner resolution pattern used elsewhere, e.g.
///   `commands/reports.rs::resolve_active_learner`)
///
/// Then decode `skills_json`'s `{label, slug}` entries and INSERT one
/// `capability_tags` row per entry (`evidence_class = 'module'`), skipping
/// any `(module_id, tag_slug, learner_id)` combination that already exists.
pub fn up(conn: &Connection) -> Result<()> {
    struct Row {
        module_id: String,
        skills_json: String,
        track_id: String,
        learner_id: Option<String>,
    }

    let mut stmt = conn.prepare(
        "SELECT m.id, m.skills_json, lp.track_id,
                (SELECT mp.learner_id FROM module_progress mp WHERE mp.module_id = m.id LIMIT 1)
         FROM modules m
         JOIN learning_paths lp ON m.path_id = lp.id
         WHERE m.skills_json IS NOT NULL AND m.skills_json <> '[]' AND m.skills_json <> ''",
    )?;

    let rows: Vec<Row> = stmt
        .query_map([], |r| {
            Ok(Row {
                module_id: r.get(0)?,
                skills_json: r.get(1)?,
                track_id: r.get(2)?,
                learner_id: r.get(3)?,
            })
        })?
        .filter_map(std::result::Result::ok)
        .collect();
    drop(stmt);

    // Fallback learner when a module has no module_progress row yet.
    let fallback_learner_id: Option<String> = conn
        .query_row(
            "SELECT id FROM learner_profiles ORDER BY id ASC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();

    for row in rows {
        let learner_id = match row.learner_id.or_else(|| fallback_learner_id.clone()) {
            Some(id) => id,
            None => continue, // No learner profile at all — nothing to attribute to.
        };

        let tags: Vec<serde_json::Value> =
            serde_json::from_str(&row.skills_json).unwrap_or_default();

        for tag in &tags {
            let (label, slug) = match (tag["label"].as_str(), tag["slug"].as_str()) {
                (Some(label), Some(slug))
                    if !label.trim().is_empty() && !slug.trim().is_empty() =>
                {
                    (label, slug)
                }
                _ => continue,
            };

            conn.execute(
                "INSERT INTO capability_tags (id, learner_id, track_id, module_id, tag_slug, tag_label, evidence_class)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, 'module'
                 WHERE NOT EXISTS (
                     SELECT 1 FROM capability_tags
                     WHERE module_id = ?4 AND tag_slug = ?5 AND learner_id = ?2
                 )",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    learner_id,
                    row.track_id,
                    row.module_id,
                    slug,
                    label,
                ],
            )?;
        }
    }

    Ok(())
}
