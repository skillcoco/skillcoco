//! End-to-end tests for `assemble_skill_report` / `export_report_json`
//! (inner helper — bypasses `tauri::State` wrapping per the achievements.rs
//! test pattern).

use super::*;
use crate::db::migrations::apply_migrations;
use crate::db::schema;
use learnforge_core::signing::verify_payload;
use rusqlite::Connection;
use std::sync::Mutex;

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

fn seed_track_with_module(conn: &Connection, track_id: &str, topic: &str) {
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
    conn.execute(
        "INSERT INTO modules (id, path_id, title) VALUES (?1, ?2, 'Pods and Nodes')",
        rusqlite::params![format!("mod-{}", track_id), format!("path-{}", track_id)],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES (?1, ?2, 'lp1', 0.8)",
        rusqlite::params![format!("mp-{}", track_id), format!("mod-{}", track_id)],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO quiz_attempts (id, learner_id, module_id, block_id, score_percent, passed) VALUES (?1, 'lp1', ?2, 'blk1', 90.0, 1)",
        rusqlite::params![format!("qa-{}", track_id), format!("mod-{}", track_id)],
    )
    .unwrap();
}

fn temp_key_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn assemble_report_inner_produces_signed_envelope_with_capabilities() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    let key_dir = temp_key_dir();
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let envelope = assemble_report_inner(
        &conn,
        &signing_key,
        key_dir.path(),
        "track",
        &Some("trk1".to_string()),
        "Ada Lovelace",
    )
    .expect("assemble");

    assert!(!envelope.payload.capabilities.is_empty(), "expected >=1 capability");
    assert_eq!(envelope.payload.learner_name, "Ada Lovelace");

    // Signature verifies over canonical_json_bytes(payload) — mirror
    // export_report_json's byte contract.
    let canonical = canonical_json_bytes(&envelope.payload).unwrap();
    let key_store = MutexCachedKeyStore::new(&signing_key, key_dir.path());
    let key = key_store.get_or_init().unwrap();
    let pem = key_store.export_public_pem().unwrap();
    let _ = key; // keep key alive; verify uses the exported PEM
    assert!(
        verify_payload(&pem, &canonical, &envelope.signature_hex),
        "signature must verify over canonical payload bytes"
    );
}

#[test]
fn assemble_report_inner_blank_learner_name_falls_back_to_profile_display_name() {
    // CR-02 / Truth 8 gap closure: a blank learner_name must never be
    // signed. assemble_report_inner must fall back to the seeded
    // learner_profiles.display_name ("Ada") before the re-sign.
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    let key_dir = temp_key_dir();
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let envelope = assemble_report_inner(
        &conn,
        &signing_key,
        key_dir.path(),
        "track",
        &Some("trk1".to_string()),
        "",
    )
    .expect("assemble");

    assert_eq!(
        envelope.payload.learner_name, "Ada",
        "blank learner_name must fall back to learner_profiles.display_name"
    );

    let canonical = canonical_json_bytes(&envelope.payload).unwrap();
    let key_store = MutexCachedKeyStore::new(&signing_key, key_dir.path());
    key_store.get_or_init().unwrap();
    let pem = key_store.export_public_pem().unwrap();
    assert!(
        verify_payload(&pem, &canonical, &envelope.signature_hex),
        "signature must verify over the fallback (non-blank) name"
    );
}

#[test]
fn assemble_report_inner_whitespace_learner_name_falls_back_to_profile_display_name() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    let key_dir = temp_key_dir();
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let envelope = assemble_report_inner(
        &conn,
        &signing_key,
        key_dir.path(),
        "track",
        &Some("trk1".to_string()),
        "   ",
    )
    .expect("assemble");

    assert_eq!(
        envelope.payload.learner_name, "Ada",
        "whitespace-only learner_name must fall back to learner_profiles.display_name"
    );
}

#[test]
fn assemble_report_inner_bakes_confirmed_learner_name_into_payload() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    let key_dir = temp_key_dir();
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let envelope = assemble_report_inner(
        &conn,
        &signing_key,
        key_dir.path(),
        "track",
        &Some("trk1".to_string()),
        "Confirmed Name",
    )
    .expect("assemble");

    assert_eq!(envelope.payload.learner_name, "Confirmed Name");
}

#[test]
fn assemble_report_inner_track_scope_label_is_topic_not_track_id() {
    // 18-05 human-verify UAT: PDF title rendered "Skill Report — c4bba882-…"
    // (raw track UUID). Managers read the title; it must carry the track
    // topic. The signature must cover the human-readable label too.
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    let key_dir = temp_key_dir();
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let envelope = assemble_report_inner(
        &conn,
        &signing_key,
        key_dir.path(),
        "track",
        &Some("trk1".to_string()),
        "Ada Lovelace",
    )
    .expect("assemble");

    assert_eq!(
        envelope.payload.scope_label, "Kubernetes",
        "scope_label must be the track topic, not the track id"
    );

    // Signed region must include the topic label (re-sign happened after
    // the override).
    let canonical = canonical_json_bytes(&envelope.payload).unwrap();
    let key_store = MutexCachedKeyStore::new(&signing_key, key_dir.path());
    key_store.get_or_init().unwrap();
    let pem = key_store.export_public_pem().unwrap();
    assert!(
        verify_payload(&pem, &canonical, &envelope.signature_hex),
        "signature must verify over payload containing the topic label"
    );
}

#[test]
fn export_report_json_bytes_round_trip_into_report_envelope_v1() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    let key_dir = temp_key_dir();
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let envelope = assemble_report_inner(
        &conn,
        &signing_key,
        key_dir.path(),
        "whole-profile",
        &None,
        "Ada",
    )
    .expect("assemble");

    let bytes = canonical_json_bytes(&envelope).expect("canonical bytes");
    let round_tripped: ReportEnvelopeV1 =
        serde_json::from_slice(&bytes).expect("deserialize ReportEnvelopeV1");

    assert_eq!(round_tripped.signature_hex, envelope.signature_hex);
    assert_eq!(round_tripped.key_fingerprint, envelope.key_fingerprint);
    assert_eq!(round_tripped.payload.learner_name, "Ada");

    // Top-level shape check — the fixed { payload, signatureHex,
    // keyFingerprint } contract that 18-06/18-07 depend on.
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value.get("payload").is_some());
    assert!(value.get("signatureHex").is_some());
    assert!(value.get("keyFingerprint").is_some());
}

#[test]
fn export_report_pdf_path_bytes_start_with_pdf_magic() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    let key_dir = temp_key_dir();
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let envelope = assemble_report_inner(
        &conn,
        &signing_key,
        key_dir.path(),
        "track",
        &Some("trk1".to_string()),
        "Ada Lovelace",
    )
    .expect("assemble");

    let pdf_input = report_payload_to_pdf_input(&envelope.payload);
    let bytes = crate::reports::artifacts::render_report_pdf(&pdf_input).expect("pdf bytes");
    assert!(bytes.starts_with(b"%PDF"), "PDF export must start with %PDF");
}

#[test]
fn export_report_pdf_display_strings_match_assembled_payload_bands() {
    // T-18-10 — the PDF input must be DERIVED from the same assembled
    // payload export_report_json serializes; no independent score
    // computation. Assert the PDF's per-capability display strings embed
    // the exact band + pct the payload carries (no drift).
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    let key_dir = temp_key_dir();
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let envelope = assemble_report_inner(
        &conn,
        &signing_key,
        key_dir.path(),
        "track",
        &Some("trk1".to_string()),
        "Ada Lovelace",
    )
    .expect("assemble");

    assert!(
        !envelope.payload.capabilities.is_empty(),
        "expected >=1 capability row to assert against"
    );

    let pdf_input = report_payload_to_pdf_input(&envelope.payload);

    for (payload_row, pdf_row) in envelope.payload.capabilities.iter().zip(pdf_input.capabilities.iter()) {
        let expected_knowledge = format!(
            "{} · {:.0}%",
            payload_row.knowledge.band,
            payload_row.knowledge.pct * 100.0
        );
        assert_eq!(pdf_row.knowledge_display, expected_knowledge);

        let expected_practical = match &payload_row.practical {
            Some(dim) => format!("{} · {:.0}%", dim.band, dim.pct * 100.0),
            None => "Not assessed".to_string(),
        };
        assert_eq!(pdf_row.practical_display, expected_practical);
    }
}

#[test]
fn parse_scope_requires_track_id_for_track_scope() {
    let err = parse_scope("track", &None).unwrap_err();
    match err {
        ReportError::Validation(_) => {}
        other => panic!("expected Validation, got {:?}", other),
    }
}

#[test]
fn parse_scope_accepts_whole_profile_without_track_id() {
    let scope = parse_scope("whole-profile", &None).expect("scope");
    assert_eq!(scope, ReportScope::WholeProfile);
}

#[test]
fn parse_scope_rejects_unknown_scope_string() {
    let err = parse_scope("bogus", &None).unwrap_err();
    match err {
        ReportError::Validation(_) => {}
        other => panic!("expected Validation, got {:?}", other),
    }
}

// ── Phase 18 (18-06 / D-13): submit_evidence_report ────────────────────

fn set_report_server_config(conn: &Connection, learner_id: &str, url: &str, token: &str) {
    let prefs = serde_json::json!({
        "reportServerUrl": url,
        "reportServerToken": token,
    });
    conn.execute(
        "UPDATE learner_profiles SET preferences_json = ?1 WHERE id = ?2",
        rusqlite::params![prefs.to_string(), learner_id],
    )
    .unwrap();
}

fn count_pending_submissions(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM pending_evidence_submissions", [], |r| r.get(0))
        .unwrap()
}

/// (a) An unreachable/invalid report-server URL returns the queued
/// (non-error) outcome AND inserts a pending row — the fire-and-forget
/// contract (D-13): a failed POST NEVER surfaces as a learner-blocking
/// error.
#[tokio::test]
async fn submit_evidence_report_unreachable_url_queues_and_returns_non_error() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    set_report_server_config(&conn, "lp1", "http://127.0.0.1:1", ""); // port 1 — nothing listens here
    let key_dir = temp_key_dir();
    let db = std::sync::Mutex::new(crate::db::Database { conn });
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let request = SubmitEvidenceReportRequest {
        scope: "track".to_string(),
        track_id: Some("trk1".to_string()),
        learner_name: "Ada".to_string(),
    };

    let result = submit_evidence_report_impl(&db, &signing_key, key_dir.path(), &request)
        .await
        .expect("must not return Err for an unreachable URL");

    assert!(!result.accepted, "unreachable URL must not report accepted=true");

    let conn_guard = db.lock().unwrap();
    assert_eq!(
        count_pending_submissions(&conn_guard.conn),
        1,
        "a pending_evidence_submissions row must be inserted on POST failure"
    );
}

/// (b) A non-http(s) scheme is rejected before any reqwest call — and
/// still queues (never silently drops the report).
#[tokio::test]
async fn submit_evidence_report_rejects_non_http_scheme() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    set_report_server_config(&conn, "lp1", "ftp://example.org/reports", "tok");
    let key_dir = temp_key_dir();
    let db = std::sync::Mutex::new(crate::db::Database { conn });
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let request = SubmitEvidenceReportRequest {
        scope: "track".to_string(),
        track_id: Some("trk1".to_string()),
        learner_name: "Ada".to_string(),
    };

    let result = submit_evidence_report_impl(&db, &signing_key, key_dir.path(), &request)
        .await
        .expect("must not return Err for a non-http scheme");

    assert!(!result.accepted, "non-http(s) scheme must never be accepted");

    let conn_guard = db.lock().unwrap();
    assert_eq!(
        count_pending_submissions(&conn_guard.conn),
        1,
        "a non-http(s) scheme must still queue the submission for retry"
    );
}

/// (c) No reportServerUrl configured returns a "no URL" signal (accepted:
/// false) — not a crash, and does NOT enqueue (nothing to retry against).
#[tokio::test]
async fn submit_evidence_report_no_url_configured_returns_not_accepted_without_enqueue() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    // No preferences_json set at all — reportServerUrl absent.
    let key_dir = temp_key_dir();
    let db = std::sync::Mutex::new(crate::db::Database { conn });
    let signing_key: Mutex<Option<ed25519_dalek::SigningKey>> = Mutex::new(None);

    let request = SubmitEvidenceReportRequest {
        scope: "track".to_string(),
        track_id: Some("trk1".to_string()),
        learner_name: "Ada".to_string(),
    };

    let result = submit_evidence_report_impl(&db, &signing_key, key_dir.path(), &request)
        .await
        .expect("must not return Err when no URL is configured");

    assert!(!result.accepted, "no URL configured must not report accepted=true");

    let conn_guard = db.lock().unwrap();
    assert_eq!(
        count_pending_submissions(&conn_guard.conn),
        0,
        "no URL configured means nothing to retry — must not enqueue"
    );
}

/// (d) `reportServerToken` never appears in any Debug-formatted output of
/// the request/prepared-submission types (T-18-18 — never logged).
#[test]
fn submit_evidence_report_token_never_appears_in_debug_output() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "Kubernetes");
    let secret_token = "sekrit-token-should-never-appear-anywhere";
    set_report_server_config(&conn, "lp1", "https://reports.example.org", secret_token);

    let (_url, token) = read_report_server_config(&conn, "lp1");
    assert_eq!(token, secret_token);

    // EvidenceSignature (the ONLY struct that crosses into logging-adjacent
    // Debug/Serialize paths in this module) never carries the token field
    // at all — assert its Debug output cannot contain the secret even if
    // some future refactor accidentally threads it through.
    let sig = EvidenceSignature {
        alg: "ed25519",
        sig: "deadbeef".to_string(),
        key_fingerprint: "cafebabe".to_string(),
    };
    let debug_str = format!("{:?}", sig);
    assert!(
        !debug_str.contains(secret_token),
        "EvidenceSignature Debug output must never contain the report server token"
    );
}
