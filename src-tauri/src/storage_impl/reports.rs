//! `SqliteReportStore` — rusqlite-backed [`ReportStore`] impl.
//!
//! Phase 18 (18-03): the ninth application of the per-module storage-trait
//! recipe (orphan-rule newtype). The trait lives in
//! `learnforge_core::reports`; this file binds it to `rusqlite::Connection`
//! via the same newtype pattern used in Waves 2-8
//! (`SqliteBktStore`/`SqliteSrStore`/.../`SqliteAchievementStore`).
//!
//! ## Per-track granularity
//!
//! Every trait method takes an explicit `track_id` — the store never
//! pre-aggregates across tracks. For [`ReportScope::WholeProfile`],
//! `capability_tags_for_scope` returns one `(track_id, slug, label)` tuple
//! PER (track, tag) pair; `learnforge_core::reports::assemble` does the D-04
//! whole-profile merge + attribution itself.
//!
//! ## Data sources
//!
//! - **capability tags**: `capability_tags` table (18-01) joined to
//!   `modules`/`learning_paths` for track scoping, UNION a title-fallback
//!   capability for every module that has NO `capability_tags` row (D-03.4
//!   — every track reports, tagged content reports better).
//! - **knowledge mastery**: `module_progress.mastery_level` (BKT rolling
//!   estimate) — weighted average across the tag's contributing modules
//!   within the given track. Reuses the existing read path; no new
//!   aggregate SQL.
//! - **practical mastery**: `module_progress.practical_mastery` averaged
//!   over the tag's modules IN THAT TRACK that have lab content (a
//!   `lab_progress` row). `None` ("not assessed") when none do.
//! - **evidence ledger**: `quiz_attempts` (18-01) → `EvidenceClass::Quiz`,
//!   `lab_progress.metadata_json.last_ai_judge` → `EvidenceClass::Lab`,
//!   `achievements` → `EvidenceClass::Cert`. Each item carries `track_id`
//!   (+`track_topic` where resolvable) for D-04 whole-profile attribution.
//!
//! ## Evidence-class validation (Warning 3)
//!
//! `capability_tags.evidence_class` is plain TEXT (18-01, no DB CHECK —
//! reserves the D-07 `exam` slot). [`parse_evidence_class`] validates on
//! read: known variants map through; any unknown string is logged
//! (`log::warn!`, bounded) and mapped to the safe default
//! [`EvidenceClass::Module`] — never blindly trusted, never panicked on.
//!
//! ## Error envelope
//!
//! Same pattern as every other storage-trait impl: `rusqlite::Error` is
//! stringified into [`ReportError::Db`] at the trust boundary via
//! [`db_err`] (orphan rule forbids `impl From<rusqlite::Error> for
//! ReportError` living in either crate).
//!
//! ## Newtype rationale (orphan rule)
//!
//! `SqliteReportStore<'a>(pub &'a Connection)` — same coherence-rule
//! workaround as `SqliteAchievementStore`/`SqliteBlockStore`/etc.

use std::collections::HashMap;

use learnforge_core::reports::{
    normalize_tag, EvidenceClass, EvidenceItem, ReportError, ReportMetadata, ReportScope,
    ReportStore,
};
use rusqlite::Connection;

/// Stringify a [`rusqlite::Error`] into [`ReportError::Db`] at the trust
/// boundary. Orphan-rule mitigation — see module-level docs.
#[inline]
fn db_err(e: rusqlite::Error) -> ReportError {
    ReportError::Db(e.to_string())
}

/// Newtype wrapping a borrowed [`rusqlite::Connection`] so we can carry the
/// [`ReportStore`] trait surface. Cheap to construct
/// (`SqliteReportStore(&conn)`).
pub struct SqliteReportStore<'a>(pub &'a Connection);

/// Validate an `evidence_class` TEXT value read from the DB against the
/// typed [`EvidenceClass`] enum (Warning 3 — the Rust layer is the
/// enforcement point since the column has no DB CHECK constraint).
///
/// Known variants map through case-sensitively (matching the lowercase
/// values written by 18-01's `capability_tags` seed data and the
/// `EvidenceClass` serde camelCase wire format at the *lowercase* Rust
/// enum-name level, i.e. "quiz"/"lab"/"cert"/"module"/"exam"). Any other
/// string is logged as a warning (bounded — only the first 64 bytes of the
/// untrusted value are included) and mapped to the safe default
/// [`EvidenceClass::Module`].
fn parse_evidence_class(s: &str) -> EvidenceClass {
    match s {
        "quiz" => EvidenceClass::Quiz,
        "lab" => EvidenceClass::Lab,
        "cert" => EvidenceClass::Cert,
        "module" => EvidenceClass::Module,
        "exam" => EvidenceClass::Exam,
        other => {
            let bounded: String = other.chars().take(64).collect();
            log::warn!(
                "capability_tags.evidence_class: unknown value {:?} — defaulting to Module",
                bounded
            );
            EvidenceClass::Module
        }
    }
}

/// Resolve the module ids belonging to `track_id` (via `modules.path_id ->
/// learning_paths.track_id`). Shared by every per-track query below.
fn module_ids_for_track(conn: &Connection, track_id: &str) -> Result<Vec<String>, ReportError> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id FROM modules m
             JOIN learning_paths lp ON lp.id = m.path_id
             WHERE lp.track_id = ?1",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map([track_id], |r| r.get::<_, String>(0))
        .map_err(db_err)?
        .filter_map(Result::ok)
        .collect();
    Ok(rows)
}

/// Resolve the display topic for a track (used for evidence `track_topic`
/// attribution + `report_metadata`).
fn track_topic(conn: &Connection, track_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT topic FROM learning_tracks WHERE id = ?1",
        [track_id],
        |r| r.get(0),
    )
    .ok()
}

impl<'a> ReportStore for SqliteReportStore<'a> {
    fn capability_tags_for_scope(
        &self,
        scope: &ReportScope,
        learner_id: &str,
    ) -> Result<Vec<(String, String, String)>, ReportError> {
        let track_ids: Vec<String> = match scope {
            ReportScope::Track(id) => vec![id.clone()],
            ReportScope::WholeProfile => {
                let mut stmt = self
                    .0
                    .prepare("SELECT id FROM learning_tracks WHERE learner_id = ?1")
                    .map_err(db_err)?;
                let ids: Vec<String> = stmt
                    .query_map([learner_id], |r| r.get::<_, String>(0))
                    .map_err(db_err)?
                    .filter_map(Result::ok)
                    .collect();
                ids
            }
        };

        let mut out: Vec<(String, String, String)> = Vec::new();

        for track_id in &track_ids {
            let module_ids = module_ids_for_track(self.0, track_id)?;
            if module_ids.is_empty() {
                continue;
            }

            // Modules that DO have at least one capability_tags row.
            let mut tagged_module_ids: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            // Dedupe by normalized slug WITHIN this track only (WholeProfile
            // merge across tracks is assemble()'s job, per D-04).
            let mut seen_slugs: HashMap<String, (String, String)> = HashMap::new();

            {
                let mut stmt = self
                    .0
                    .prepare(
                        "SELECT ct.module_id, ct.tag_slug, ct.tag_label, ct.evidence_class
                         FROM capability_tags ct
                         WHERE ct.learner_id = ?1 AND ct.track_id = ?2",
                    )
                    .map_err(db_err)?;
                let rows = stmt
                    .query_map([learner_id, track_id.as_str()], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    })
                    .map_err(db_err)?;
                for row in rows {
                    let (module_id, slug, label, evidence_class) = row.map_err(db_err)?;
                    // Warning 3 — validate the untyped TEXT column on read.
                    // The parsed value isn't threaded further here (this
                    // query only needs module/slug/label to build the
                    // capability row), but every read of evidence_class
                    // MUST go through the validator so an unrecognized
                    // value is logged, never silently trusted.
                    let _ = parse_evidence_class(&evidence_class);
                    tagged_module_ids.insert(module_id);
                    let norm = normalize_tag(&slug);
                    seen_slugs.entry(norm).or_insert((slug, label));
                }
            }

            // D-03.4 title fallback: any module in this track with NO
            // capability_tags row contributes a synthetic capability keyed
            // by its own (normalized) title.
            for module_id in &module_ids {
                if tagged_module_ids.contains(module_id) {
                    continue;
                }
                let title: Option<String> = self
                    .0
                    .query_row(
                        "SELECT title FROM modules WHERE id = ?1",
                        [module_id],
                        |r| r.get(0),
                    )
                    .ok();
                if let Some(title) = title {
                    let norm = normalize_tag(&title);
                    seen_slugs.entry(norm).or_insert((title.clone(), title));
                }
            }

            for (slug, label) in seen_slugs.into_values() {
                out.push((track_id.clone(), slug, label));
            }
        }

        Ok(out)
    }

    fn knowledge_mastery(
        &self,
        track_id: &str,
        slug: &str,
        learner_id: &str,
    ) -> Result<f64, ReportError> {
        let module_ids = contributing_module_ids(self.0, track_id, slug, learner_id)?;
        if module_ids.is_empty() {
            return Ok(0.0);
        }

        let mut total = 0.0;
        let mut count = 0;
        for module_id in &module_ids {
            // Reuse the existing BKT read path — module_progress.mastery_level
            // — no parallel/duplicate aggregate SQL.
            let mastery: Option<f64> = self
                .0
                .query_row(
                    "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
                    [module_id.as_str(), learner_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(m) = mastery {
                total += m;
                count += 1;
            }
        }
        if count == 0 {
            return Ok(0.0);
        }
        Ok(total / count as f64)
    }

    fn practical_mastery(
        &self,
        track_id: &str,
        slug: &str,
        learner_id: &str,
    ) -> Result<Option<f64>, ReportError> {
        let module_ids = contributing_module_ids(self.0, track_id, slug, learner_id)?;
        if module_ids.is_empty() {
            return Ok(None);
        }

        let mut total = 0.0;
        let mut count = 0;
        for module_id in &module_ids {
            // Only modules WITH lab content (a lab_progress row) count
            // toward practical mastery — modules with none must not drag
            // the average toward 0 (that would misreport "not assessed" as
            // a real score).
            let has_lab: bool = self
                .0
                .query_row(
                    "SELECT COUNT(*) FROM lab_progress WHERE module_id = ?1 AND learner_id = ?2",
                    [module_id.as_str(), learner_id],
                    |r| r.get::<_, i64>(0),
                )
                .map(|c| c > 0)
                .unwrap_or(false);
            if !has_lab {
                continue;
            }
            let practical: Option<f64> = self
                .0
                .query_row(
                    "SELECT practical_mastery FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
                    [module_id.as_str(), learner_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(p) = practical {
                total += p;
                count += 1;
            }
        }
        if count == 0 {
            return Ok(None);
        }
        Ok(Some(total / count as f64))
    }

    fn evidence_ledger(
        &self,
        track_id: &str,
        slug: &str,
        learner_id: &str,
    ) -> Result<Vec<EvidenceItem>, ReportError> {
        let module_ids = contributing_module_ids(self.0, track_id, slug, learner_id)?;
        if module_ids.is_empty() {
            return Ok(Vec::new());
        }
        let topic = track_topic(self.0, track_id);
        let mut items: Vec<EvidenceItem> = Vec::new();

        for module_id in &module_ids {
            // Quiz evidence.
            {
                let mut stmt = self
                    .0
                    .prepare(
                        "SELECT block_id, score_percent, passed, completed_at
                         FROM quiz_attempts
                         WHERE module_id = ?1 AND learner_id = ?2
                         ORDER BY completed_at DESC",
                    )
                    .map_err(db_err)?;
                let rows = stmt
                    .query_map([module_id.as_str(), learner_id], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, f64>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    })
                    .map_err(db_err)?;
                for row in rows {
                    let (block_id, score, passed, completed_at) = row.map_err(db_err)?;
                    items.push(EvidenceItem {
                        class: EvidenceClass::Quiz,
                        label: format!("Quiz: {}", block_id),
                        detail: format!(
                            "{:.0}% ({})",
                            score,
                            if passed != 0 { "passed" } else { "not passed" }
                        ),
                        date: completed_at,
                        track_id: Some(track_id.to_string()),
                        track_topic: topic.clone(),
                    });
                }
            }

            // Lab evidence (last AI-judge verdict from metadata_json).
            {
                let mut stmt = self
                    .0
                    .prepare(
                        "SELECT block_id, current_step, total_steps, metadata_json, last_updated
                         FROM lab_progress
                         WHERE module_id = ?1 AND learner_id = ?2",
                    )
                    .map_err(db_err)?;
                let rows = stmt
                    .query_map([module_id.as_str(), learner_id], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, String>(4)?,
                        ))
                    })
                    .map_err(db_err)?;
                for row in rows {
                    let (block_id, current_step, total_steps, metadata_json, last_updated) =
                        row.map_err(db_err)?;
                    let verdict = serde_json::from_str::<serde_json::Value>(&metadata_json)
                        .ok()
                        .and_then(|v| {
                            v.get("last_ai_judge")
                                .and_then(|j| j.get("verdict"))
                                .and_then(|s| s.as_str().map(|s| s.to_string()))
                        });
                    let mut detail = format!("Step {}/{}", current_step, total_steps);
                    if let Some(v) = verdict {
                        detail.push_str(&format!(" — AI-judge: {}", v));
                    }
                    items.push(EvidenceItem {
                        class: EvidenceClass::Lab,
                        label: format!("Lab: {}", block_id),
                        detail,
                        date: last_updated,
                        track_id: Some(track_id.to_string()),
                        track_topic: topic.clone(),
                    });
                }
            }
        }

        // Cert evidence — scoped by track (achievements table), not module,
        // since certificates are issued per track/level, not per module.
        {
            let mut stmt = self
                .0
                .prepare(
                    "SELECT id, level, issued_at FROM achievements
                     WHERE learner_id = ?1 AND track_id = ?2",
                )
                .map_err(db_err)?;
            let rows = stmt
                .query_map([learner_id, track_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .map_err(db_err)?;
            for row in rows {
                let (id, level, issued_at) = row.map_err(db_err)?;
                items.push(EvidenceItem {
                    class: EvidenceClass::Cert,
                    // D-08: time-to-mastery is context in the ledger detail
                    // text, never a numeric score column.
                    label: format!("Certificate: {}", level),
                    detail: format!("id={}", id),
                    date: issued_at,
                    track_id: Some(track_id.to_string()),
                    track_topic: topic.clone(),
                });
            }
        }

        // Only cert evidence is universal per-track (not per-slug), so filter
        // certs into every capability's ledger is intentional per D-06 (certs
        // back every capability in the track they were earned in). Quiz/lab
        // items are already module-scoped above via contributing_module_ids.
        let _ = slug; // slug already used to resolve contributing_module_ids

        Ok(items)
    }

    fn report_metadata(
        &self,
        scope: &ReportScope,
        learner_id: &str,
    ) -> Result<ReportMetadata, ReportError> {
        let (pack_provenance, verified_issuer) = match scope {
            ReportScope::Track(track_id) => {
                let row: Option<(String, i64, Option<String>)> = self
                    .0
                    .query_row(
                        "SELECT generated_by_model, verified, issuer_name
                         FROM learning_paths WHERE track_id = ?1
                         ORDER BY version DESC LIMIT 1",
                        [track_id],
                        |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, i64>(1)?,
                                r.get::<_, Option<String>>(2)?,
                            ))
                        },
                    )
                    .ok();
                match row {
                    Some((generated_by_model, verified, issuer_name)) => {
                        let provenance = generated_by_model
                            .strip_prefix("topic-pack:")
                            .map(|s| s.to_string());
                        let verified_issuer = if verified != 0 { issuer_name } else { None };
                        (provenance, verified_issuer)
                    }
                    None => (None, None),
                }
            }
            ReportScope::WholeProfile => (None, None),
        };
        let _ = learner_id;

        Ok(ReportMetadata {
            // Overwritten by assemble() from the injected clock; placeholder
            // here satisfies the struct shape.
            generated_at: String::new(),
            app_version: String::new(),
            pack_provenance,
            verified_issuer,
        })
    }
}

/// Resolve the module ids in `track_id` that contribute to capability
/// `slug` for `learner_id` — either via a `capability_tags` row whose
/// normalized `tag_slug` matches, or (title-fallback, D-03.4) a module
/// with no tags at all whose normalized title matches `slug`.
fn contributing_module_ids(
    conn: &Connection,
    track_id: &str,
    slug: &str,
    learner_id: &str,
) -> Result<Vec<String>, ReportError> {
    let mut out = Vec::new();

    {
        let mut stmt = conn
            .prepare(
                "SELECT module_id, tag_slug FROM capability_tags
                 WHERE learner_id = ?1 AND track_id = ?2",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([learner_id, track_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(db_err)?;
        for row in rows {
            let (module_id, tag_slug) = row.map_err(db_err)?;
            if normalize_tag(&tag_slug) == slug {
                out.push(module_id);
            }
        }
    }

    if !out.is_empty() {
        return Ok(out);
    }

    // Title-fallback path: `slug` may be the normalized module title for an
    // untagged module. Only consider modules with NO capability_tags rows
    // at all (mirrors capability_tags_for_scope's fallback condition).
    let module_ids = module_ids_for_track(conn, track_id)?;
    for module_id in module_ids {
        let has_tags: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM capability_tags WHERE module_id = ?1",
                [module_id.as_str()],
                |r| r.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);
        if has_tags {
            continue;
        }
        let title: Option<String> = conn
            .query_row(
                "SELECT title FROM modules WHERE id = ?1",
                [module_id.as_str()],
                |r| r.get(0),
            )
            .ok();
        if let Some(title) = title {
            if normalize_tag(&title) == slug {
                out.push(module_id);
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
