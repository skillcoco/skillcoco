//! SQL-touching tests for `SqliteReportStore`. Pure-algorithm tests for
//! `assemble`/`merge_whole_profile` live in `learnforge_core::reports::tests`
//! (run against inline stubs).

use super::*;
use crate::db::migrations::apply_migrations;
use crate::db::schema;
use learnforge_core::reports::ReportStore;
use rusqlite::Connection;

fn fresh_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(schema::CREATE_TABLES).unwrap();
    apply_migrations(&conn).unwrap();
    conn
}

fn seed_learner(conn: &Connection) {
    conn.execute(
        "INSERT OR IGNORE INTO learner_profiles (id, display_name) VALUES ('lp1', 'Ada')",
        [],
    )
    .unwrap();
}

fn seed_track(conn: &Connection, track_id: &str, topic: &str) {
    seed_learner(conn);
    conn.execute(
        "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, 'lp1', ?2, 'devops', 'Learn')",
        rusqlite::params![track_id, topic],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES (?1, ?2, 1, '[]', '[]', 'test-model')",
        rusqlite::params![format!("path-{}", track_id), track_id],
    )
    .unwrap();
}

fn seed_module(conn: &Connection, module_id: &str, track_id: &str, title: &str) {
    conn.execute(
        "INSERT INTO modules (id, path_id, title) VALUES (?1, ?2, ?3)",
        rusqlite::params![module_id, format!("path-{}", track_id), title],
    )
    .unwrap();
}

fn seed_module_progress(conn: &Connection, module_id: &str, mastery: f64, practical: f64) {
    conn.execute(
        "INSERT INTO module_progress (id, module_id, learner_id, mastery_level, practical_mastery) VALUES (?1, ?2, 'lp1', ?3, ?4)",
        rusqlite::params![format!("mp-{}", module_id), module_id, mastery, practical],
    )
    .unwrap();
}

fn seed_block(conn: &Connection, block_id: &str, module_id: &str, block_type: &str) {
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, block_type) VALUES (?1, ?2, ?3)",
        rusqlite::params![block_id, module_id, block_type],
    )
    .unwrap();
}

fn seed_capability_tag(
    conn: &Connection,
    id: &str,
    track_id: &str,
    module_id: &str,
    tag_slug: &str,
    tag_label: &str,
    evidence_class: &str,
) {
    conn.execute(
        "INSERT INTO capability_tags (id, learner_id, track_id, module_id, tag_slug, tag_label, evidence_class) VALUES (?1, 'lp1', ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, track_id, module_id, tag_slug, tag_label, evidence_class],
    )
    .unwrap();
}

// ── Object safety ──────────────────────────────────────────────────────────

#[test]
fn sqlite_report_store_is_object_safe() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    let store = SqliteReportStore(&conn);
    let dyn_store: &dyn ReportStore = &store;
    let tags = dyn_store
        .capability_tags_for_scope(&ReportScope::Track("trk1".to_string()), "lp1")
        .unwrap();
    assert!(tags.is_empty(), "no modules seeded yet");
}

// ── capability_tags_for_scope ──────────────────────────────────────────────

#[test]
fn capability_tags_for_scope_returns_tagged_capability() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    seed_module(&conn, "mod1", "trk1", "Pods and Nodes");
    seed_capability_tag(
        &conn,
        "ct1",
        "trk1",
        "mod1",
        "can-configure-rbac",
        "Can configure RBAC policies",
        "module",
    );

    let store = SqliteReportStore(&conn);
    let tags = store
        .capability_tags_for_scope(&ReportScope::Track("trk1".to_string()), "lp1")
        .expect("tags");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].0, "trk1");
    assert_eq!(tags[0].1, "can-configure-rbac");
    assert_eq!(tags[0].2, "Can configure RBAC policies");
}

/// D-03.4 — a module with NO capability_tags row yields a title-fallback
/// capability so every track reports.
#[test]
fn capability_tags_for_scope_title_fallback_for_untagged_module() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    seed_module(&conn, "mod1", "trk1", "Intro to Pods");

    let store = SqliteReportStore(&conn);
    let tags = store
        .capability_tags_for_scope(&ReportScope::Track("trk1".to_string()), "lp1")
        .expect("tags");
    assert_eq!(tags.len(), 1, "untagged module must still produce a fallback row");
    assert_eq!(tags[0].2, "Intro to Pods");
}

#[test]
fn capability_tags_for_scope_whole_profile_returns_per_track_rows_no_premerge() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    seed_track(&conn, "trk2", "Docker");
    seed_module(&conn, "mod1", "trk1", "Pods and Nodes");
    seed_module(&conn, "mod2", "trk2", "Container Basics");
    seed_capability_tag(
        &conn,
        "ct1",
        "trk1",
        "mod1",
        "can-debug-networking",
        "Can debug networking",
        "module",
    );
    seed_capability_tag(
        &conn,
        "ct2",
        "trk2",
        "mod2",
        "can-debug-networking",
        "Can debug networking",
        "module",
    );

    let store = SqliteReportStore(&conn);
    let tags = store
        .capability_tags_for_scope(&ReportScope::WholeProfile, "lp1")
        .expect("tags");
    // Two distinct (track, slug) tuples — NOT merged (assemble()'s job).
    assert_eq!(tags.len(), 2);
    let track_ids: std::collections::HashSet<&str> =
        tags.iter().map(|(t, _, _)| t.as_str()).collect();
    assert!(track_ids.contains("trk1"));
    assert!(track_ids.contains("trk2"));
}

// ── knowledge_mastery ────────────────────────────────────────────────────

#[test]
fn knowledge_mastery_reads_module_progress_mastery_level() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    seed_module(&conn, "mod1", "trk1", "Pods and Nodes");
    seed_module_progress(&conn, "mod1", 0.75, 0.0);
    seed_capability_tag(
        &conn,
        "ct1",
        "trk1",
        "mod1",
        "can-configure-rbac",
        "Can configure RBAC policies",
        "module",
    );

    let store = SqliteReportStore(&conn);
    let pct = store
        .knowledge_mastery("trk1", "can-configure-rbac", "lp1")
        .expect("mastery");
    assert!((pct - 0.75).abs() < 1e-9);
}

#[test]
fn knowledge_mastery_weighted_average_across_contributing_modules() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    seed_module(&conn, "mod1", "trk1", "Pods");
    seed_module(&conn, "mod2", "trk1", "Nodes");
    seed_module_progress(&conn, "mod1", 0.6, 0.0);
    seed_module_progress(&conn, "mod2", 1.0, 0.0);
    seed_capability_tag(&conn, "ct1", "trk1", "mod1", "can-x", "Can X", "module");
    seed_capability_tag(&conn, "ct2", "trk1", "mod2", "can-x", "Can X", "module");

    let store = SqliteReportStore(&conn);
    let pct = store.knowledge_mastery("trk1", "can-x", "lp1").expect("mastery");
    assert!((pct - 0.8).abs() < 1e-9, "expected average of 0.6 and 1.0, got {}", pct);
}

// ── practical_mastery ────────────────────────────────────────────────────

/// A tag with no lab content anywhere must return `Ok(None)` ("not
/// assessed"), never `Ok(Some(0.0))`.
#[test]
fn practical_mastery_returns_none_when_no_lab_content() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    seed_module(&conn, "mod1", "trk1", "Pods and Nodes");
    seed_module_progress(&conn, "mod1", 0.75, 0.0);
    seed_capability_tag(
        &conn,
        "ct1",
        "trk1",
        "mod1",
        "can-configure-rbac",
        "Can configure RBAC policies",
        "module",
    );

    let store = SqliteReportStore(&conn);
    let practical = store
        .practical_mastery("trk1", "can-configure-rbac", "lp1")
        .expect("practical");
    assert_eq!(practical, None, "no lab content must report None, never Some(0.0)");
}

#[test]
fn practical_mastery_averages_over_modules_with_lab_content() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    seed_module(&conn, "mod1", "trk1", "Pods");
    seed_module_progress(&conn, "mod1", 0.5, 0.9);
    seed_capability_tag(&conn, "ct1", "trk1", "mod1", "can-x", "Can X", "module");
    seed_block(&conn, "blk1", "mod1", "lab");
    conn.execute(
        "INSERT INTO lab_progress (learner_id, module_id, block_id, total_steps) VALUES ('lp1', 'mod1', 'blk1', 3)",
        [],
    )
    .unwrap();

    let store = SqliteReportStore(&conn);
    let practical = store.practical_mastery("trk1", "can-x", "lp1").expect("practical");
    assert_eq!(practical, Some(0.9));
}

// ── evidence_ledger ──────────────────────────────────────────────────────

#[test]
fn evidence_ledger_includes_quiz_lab_and_cert_items_with_track_attribution() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    seed_module(&conn, "mod1", "trk1", "Pods");
    seed_capability_tag(&conn, "ct1", "trk1", "mod1", "can-x", "Can X", "module");

    seed_block(&conn, "blk-quiz1", "mod1", "quiz");
    seed_block(&conn, "blk-lab1", "mod1", "lab");
    conn.execute(
        "INSERT INTO quiz_attempts (id, learner_id, module_id, block_id, score_percent, passed) VALUES ('qa1', 'lp1', 'mod1', 'blk-quiz1', 90.0, 1)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO lab_progress (learner_id, module_id, block_id, current_step, total_steps, metadata_json) VALUES ('lp1', 'mod1', 'blk-lab1', 2, 3, '{\"last_ai_judge\":{\"verdict\":\"pass\"}}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO achievements (id, learner_id, track_id, pack_id, kind, level, issued_at, mastery_score, payload_json, signature, key_fingerprint, track_topic) VALUES ('a1', 'lp1', 'trk1', NULL, 'badge', 'Associate', '2026-07-01T00:00:00Z', 0.9, '{}', 'sig', 'fp', 'Kubernetes')",
        [],
    )
    .unwrap();

    let store = SqliteReportStore(&conn);
    let items = store.evidence_ledger("trk1", "can-x", "lp1").expect("ledger");

    assert!(items.iter().any(|i| i.class == EvidenceClass::Quiz));
    assert!(items.iter().any(|i| i.class == EvidenceClass::Lab));
    assert!(items.iter().any(|i| i.class == EvidenceClass::Cert));
    for item in &items {
        assert_eq!(item.track_id.as_deref(), Some("trk1"));
        assert_eq!(item.track_topic.as_deref(), Some("Kubernetes"));
    }
}

// ── evidence_class validation (Warning 3) ───────────────────────────────

/// An unknown evidence_class string read from the DB must map to
/// EvidenceClass::Module (validated on read, never trusted blindly).
#[test]
fn parse_evidence_class_maps_unknown_string_to_module() {
    assert_eq!(parse_evidence_class("quiz"), EvidenceClass::Quiz);
    assert_eq!(parse_evidence_class("lab"), EvidenceClass::Lab);
    assert_eq!(parse_evidence_class("cert"), EvidenceClass::Cert);
    assert_eq!(parse_evidence_class("module"), EvidenceClass::Module);
    assert_eq!(parse_evidence_class("exam"), EvidenceClass::Exam);
    assert_eq!(
        parse_evidence_class("something-totally-unrecognized"),
        EvidenceClass::Module,
        "unknown evidence_class must default to Module, never panic or pass through"
    );
}

/// An unrecognized evidence_class stored in a real capability_tags row must
/// not crash or block the read path — capability_tags_for_scope still
/// returns the row (validated internally via parse_evidence_class).
#[test]
fn capability_tags_for_scope_tolerates_unrecognized_evidence_class_in_db() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");
    seed_module(&conn, "mod1", "trk1", "Pods and Nodes");
    seed_capability_tag(
        &conn,
        "ct1",
        "trk1",
        "mod1",
        "can-configure-rbac",
        "Can configure RBAC policies",
        "totally-bogus-value",
    );

    let store = SqliteReportStore(&conn);
    let tags = store
        .capability_tags_for_scope(&ReportScope::Track("trk1".to_string()), "lp1")
        .expect("must not error on unrecognized evidence_class");
    assert_eq!(tags.len(), 1);
}

// ── report_metadata ──────────────────────────────────────────────────────

#[test]
fn report_metadata_reads_pack_provenance_and_verified_issuer() {
    let conn = fresh_conn();
    seed_learner(&conn);
    conn.execute(
        "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'Kubernetes', 'devops', 'Learn')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model, verified, issuer_name) VALUES ('path-trk1', 'trk1', 1, '[]', '[]', 'topic-pack:k8s-fundamentals', 1, 'School of DevOps')",
        [],
    )
    .unwrap();

    let store = SqliteReportStore(&conn);
    let meta = store
        .report_metadata(&ReportScope::Track("trk1".to_string()), "lp1")
        .expect("metadata");
    assert_eq!(meta.pack_provenance.as_deref(), Some("k8s-fundamentals"));
    assert_eq!(meta.verified_issuer.as_deref(), Some("School of DevOps"));
}

#[test]
fn report_metadata_whole_profile_has_no_single_provenance() {
    let conn = fresh_conn();
    seed_track(&conn, "trk1", "Kubernetes");

    let store = SqliteReportStore(&conn);
    let meta = store
        .report_metadata(&ReportScope::WholeProfile, "lp1")
        .expect("metadata");
    assert_eq!(meta.pack_provenance, None);
    assert_eq!(meta.verified_issuer, None);
}

// ── ReportStore method signatures take an explicit track_id ─────────────
// (compile-time proof: every call site above passes track_id explicitly;
// see also the grep-based acceptance criterion in the plan.)
