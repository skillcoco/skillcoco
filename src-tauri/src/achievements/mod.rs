//! Transitional shim — Phase 7 Wave 8 (07-08) moved the achievement-
//! issuance algorithm to `learnforge_core::achievements`. This file
//! preserves the legacy free-fn surface so existing callers
//! (`commands/learning.rs`, `commands/achievements.rs`) compile
//! unchanged.
//!
//! ## What lives here
//!
//! - **Re-exports** of every type the pre-Wave-8 module exposed:
//!   `Achievement`, `CertPayloadV1`, `AchievementError`,
//!   `TrackCertifications`, `IssuanceContext`, `CertificatePdfInput`,
//!   `BadgePngInput`, `AchievementStore`. (`maybe_issue` is exposed via
//!   a legacy-shaped wrapper — see below.)
//! - **Legacy `maybe_issue`** with the pre-Wave-8 signature
//!   `(conn, track_id, learner_id, signing_key_mutex, key_dir)`. The
//!   body supplies `Utc::now()` + wraps the connection in
//!   `SqliteAchievementStore` + wraps `(mutex, key_dir)` in
//!   `MutexCachedKeyStore` and dispatches to the core fn.
//! - **`list_for_learner_impl`**, **`lookup_achievement_impl`**,
//!   **`get_track_certifications_impl`** — the IPC-handler helpers from
//!   pre-Wave-8 are preserved as thin wrappers around the trait methods
//!   so `commands/achievements.rs:17-21` imports compile unchanged.
//! - **`pub mod artifacts`** — D-03 amendment + R-7 mitigation: the
//!   PDF / PNG / QR renderers stay here because `printpdf` / `image` /
//!   `qrcode` are not WASM-portable.
//! - **`pub mod signing`** — Wave 5 shim around `learnforge_core::signing`
//!   + `FsKeyStore`.
//! - **`pub mod threshold`** — Wave 4 shim around
//!   `learnforge_core::threshold` + the parked SQL aggregate.
//!
//! ## What was deleted
//!
//! - The pre-Wave-8 bodies of `maybe_issue` (lines 213-307), the four
//!   SQL helpers (`lookup_context` 96-133, `list_for_learner_impl`
//!   348-374, `lookup_achievement_impl` 310-339,
//!   `get_track_certifications_impl` 387+), the `build_signed_achievement`
//!   helper (162-207), and `get_or_load_into_mutex` (138-158) — every
//!   one moved into `learnforge_core::achievements` (algorithm /
//!   types) or `src-tauri/src/storage_impl/achievements.rs` (rusqlite
//!   trait impl) or replaced by the
//!   [`MutexCachedKeyStore`] inline below (key-mutex caching).
//! - Pure tests moved with the algorithm to
//!   `learnforge_core/src/achievements.rs::tests`. SQL-touching tests
//!   stay in this file (they need a real `rusqlite::Connection`).
//!
//! No `#[deprecated]` on re-exports — rustc silently ignores it on
//! `pub use` (R5 / Pitfall 6 from 07-RESEARCH.md). Wave 10
//! grep-and-rewrite is the eventual cleanup target — at that point the
//! IPC handlers + `commands/learning.rs:399` migrate onto
//! `learnforge_core::achievements::maybe_issue` directly +
//! `SqliteAchievementStore(&conn)` + their own clock + `FsKeyStore`,
//! and this shim deletes.

pub mod artifacts;
pub mod signing;
pub mod threshold;

use chrono::Utc;
use ed25519_dalek::SigningKey;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

// ── Re-exports of the core surface ───────────────────────────────────────

pub use learnforge_core::achievements::{
    Achievement, AchievementError, AchievementStore, BadgePngInput, CertPayloadV1,
    CertificatePdfInput, IssuanceContext, TrackCertifications,
};

// Re-export the core `maybe_issue` under a non-clashing name for callers
// that want to invoke it directly (Wave 10 migration target).
pub use learnforge_core::achievements::maybe_issue as maybe_issue_core;

use crate::storage_impl::achievements::SqliteAchievementStore;
use crate::storage_impl::signing::FsKeyStore;
use learnforge_core::signing::{SigningError, SigningKeyStore};

// ── Mutex-cached key store (preserves the Phase 6 lazy-init pattern) ─────

/// `SigningKeyStore` impl that consults a process-level
/// `Mutex<Option<SigningKey>>` cache before falling back to
/// [`FsKeyStore`] for cold load + first-time generation. Mirrors the
/// pre-Wave-8 `get_or_load_into_mutex` helper that lived in
/// `src-tauri/src/achievements/mod.rs:138-158`.
///
/// Wave 10 cleanup: callers migrate to `FsKeyStore` directly + a
/// caching adapter that the IPC layer holds explicitly (instead of this
/// shim closing over a `&Mutex`).
struct MutexCachedKeyStore<'a> {
    cache: &'a Mutex<Option<SigningKey>>,
    fallback: FsKeyStore,
}

impl<'a> MutexCachedKeyStore<'a> {
    fn new(cache: &'a Mutex<Option<SigningKey>>, key_dir: &Path) -> Self {
        Self {
            cache,
            fallback: FsKeyStore::new(key_dir.to_path_buf()),
        }
    }
}

impl<'a> SigningKeyStore for MutexCachedKeyStore<'a> {
    fn get_or_init(&self) -> Result<SigningKey, SigningError> {
        // Fast path — return a fresh clone of the cached key.
        {
            let guard = self
                .cache
                .lock()
                .map_err(|_| SigningError::Io("signing key mutex poisoned".to_string()))?;
            if let Some(k) = guard.as_ref() {
                return Ok(SigningKey::from_bytes(&k.to_bytes()));
            }
        }
        // Cold path — generate or load via FsKeyStore + cache the result.
        let key = self.fallback.get_or_init()?;
        {
            let mut guard = self
                .cache
                .lock()
                .map_err(|_| SigningError::Io("signing key mutex poisoned".to_string()))?;
            if guard.is_none() {
                *guard = Some(SigningKey::from_bytes(&key.to_bytes()));
            }
        }
        Ok(SigningKey::from_bytes(&key.to_bytes()))
    }

    fn export_public_pem(&self) -> Result<String, SigningError> {
        self.fallback.export_public_pem()
    }
}

// ── Legacy maybe_issue wrapper ───────────────────────────────────────────

/// Issue any pending achievements for a learner+track. Legacy signature
/// preserved for the two pre-Wave-8 callers:
/// `commands/learning.rs:399` (`submit_quiz`) and the
/// `commands/learning.rs::tests` + `commands/achievements.rs::tests`
/// fixtures.
///
/// Internally:
/// - wraps `&Connection` in [`SqliteAchievementStore`] so the
///   [`AchievementStore`] trait surface is available;
/// - wraps `(&signing_key_mutex, &key_dir)` in [`MutexCachedKeyStore`]
///   so the Phase 6 lazy-init + per-process cache semantics survive;
/// - supplies `Utc::now()` as the A5 clock (the same call site the
///   pre-Wave-8 body used internally);
/// - delegates to [`maybe_issue_core`].
///
/// Returns the freshly-issued rows (empty `Vec` on the idempotent path).
/// Wave 10 cleanup rewrites callers to invoke `maybe_issue_core` with
/// their own clock + their own `SigningKeyStore`.
pub fn maybe_issue(
    conn: &Connection,
    track_id: &str,
    learner_id: &str,
    signing_key: &Mutex<Option<SigningKey>>,
    key_dir: &Path,
) -> Result<Vec<Achievement>, AchievementError> {
    let store = SqliteAchievementStore(conn);
    let key_store = MutexCachedKeyStore::new(signing_key, key_dir);
    maybe_issue_core(&store, &key_store, track_id, learner_id, Utc::now())
}

// ── Legacy IPC-helper wrappers ───────────────────────────────────────────

/// List the active learner's achievements (`issued_at DESC, id ASC`).
/// Thin wrapper around [`AchievementStore::list_for_learner`] preserving
/// the pre-Wave-8 free-fn surface that `commands/achievements.rs:120`
/// imports.
pub fn list_for_learner_impl(conn: &Connection) -> Result<Vec<Achievement>, AchievementError> {
    SqliteAchievementStore(conn).list_for_learner()
}

/// Look up a single [`Achievement`] row by id. `Err(Validation)` on miss.
/// Thin wrapper around [`AchievementStore::lookup_achievement`].
pub fn lookup_achievement_impl(
    conn: &Connection,
    achievement_id: &str,
) -> Result<Achievement, AchievementError> {
    SqliteAchievementStore(conn).lookup_achievement(achievement_id)
}

/// Per-track certifications: earned levels + next-level criteria.
///
/// Computes `next_level` + `criteria` from the badge-level set returned
/// by [`AchievementStore::earned_badge_levels`]. The string template
/// + the ladder ordering match the pre-Wave-8 body verbatim — frontend
/// surfaces (Settings / Achievements panel) depend on the exact wording.
pub fn get_track_certifications_impl(
    conn: &Connection,
    track_id: &str,
    learner_id: &str,
) -> Result<TrackCertifications, AchievementError> {
    let store = SqliteAchievementStore(conn);
    let earned_levels = store.earned_badge_levels(track_id, learner_id)?;
    let has = |name: &str| earned_levels.iter().any(|l| l == name);

    let (next_level, criteria) = if !has("Associate") {
        (
            Some("Associate".to_string()),
            "25% of modules mastered".to_string(),
        )
    } else if !has("Practitioner") {
        (
            Some("Practitioner".to_string()),
            "60% of modules mastered".to_string(),
        )
    } else if !has("Professional") {
        (
            Some("Professional".to_string()),
            "100% of modules mastered, average mastery >= 0.85, plus all practical labs if required"
                .to_string(),
        )
    } else {
        (None, String::new())
    };

    Ok(TrackCertifications {
        earned_levels,
        next_level,
        criteria,
    })
}

// ── SQL-touching integration tests (Phase 6 acceptance preserved) ───────

#[cfg(test)]
mod tests {
    //! SQL-touching integration tests covering the end-to-end shim
    //! seam: `&Connection` → `SqliteAchievementStore` →
    //! `MutexCachedKeyStore` → `learnforge_core::achievements::maybe_issue`.
    //! Pure-algorithm tests live in
    //! `learnforge_core::achievements::tests` (run against inline stubs).

    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;
    use std::path::PathBuf;
    use std::sync::Mutex;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    /// modules: (module_id, mastery_level, practical_required, practical_mastery).
    fn seed_track(
        conn: &Connection,
        track_id: &str,
        learner_id: &str,
        topic: &str,
        modules: &[(&str, f64, bool, f64)],
    ) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id, display_name) VALUES (?1, 'Alice')",
            [learner_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, ?2, ?3, 'devops', 'Cert')",
            rusqlite::params![track_id, learner_id, topic],
        ).unwrap();
        let path_id = format!("p-{}", track_id);
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES (?1, ?2, 1, '[]', '[]', 'test')",
            rusqlite::params![path_id, track_id],
        ).unwrap();
        for (i, (mid, ml, pr, pm)) in modules.iter().enumerate() {
            let content_json = if *pr { r#"{"practical_required": true}"# } else { "{}" };
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering, content_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![mid, path_id, format!("M{}", i), i as i64, content_json],
            ).unwrap();
            conn.execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level, practical_mastery) VALUES (?1, ?2, ?3, 'in_progress', ?4, ?5)",
                rusqlite::params![format!("mp-{}", mid), mid, learner_id, ml, pm],
            ).unwrap();
        }
    }

    fn empty_key_slot() -> Mutex<Option<SigningKey>> {
        Mutex::new(None)
    }
    fn fresh_key_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    /// 1/4 modules above 0.7 = 25% (Associate only).
    const ONE_OF_FOUR: &[(&str, f64, bool, f64)] = &[
        ("m1", 0.75, false, 0.0),
        ("m2", 0.40, false, 0.0),
        ("m3", 0.30, false, 0.0),
        ("m4", 0.30, false, 0.0),
    ];

    #[test]
    fn issues_associate_on_first_module_completion() {
        let conn = fresh_conn();
        seed_track(&conn, "trk1", "lp1", "Kubernetes", ONE_OF_FOUR);
        let key_slot = empty_key_slot();
        let key_dir = fresh_key_dir();
        let issued = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("issue");
        assert_eq!(issued.len(), 1, "1/4 = 25% — Associate only");
        let a = &issued[0];
        assert_eq!(a.level, "Associate");
        assert_eq!(a.kind, "badge");
        assert_eq!(a.signature.len(), 128, "Ed25519 sig hex = 64 bytes * 2");
        assert_eq!(a.key_fingerprint.len(), 8);
        assert_eq!(a.track_topic, "Kubernetes");
    }

    #[test]
    fn idempotent_on_second_call() {
        let conn = fresh_conn();
        seed_track(&conn, "trk1", "lp1", "Kubernetes", ONE_OF_FOUR);
        let key_slot = empty_key_slot();
        let key_dir = fresh_key_dir();
        let first = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("first");
        let second = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("second");
        assert_eq!(first.len(), 1);
        assert!(second.is_empty(), "second call must be a no-op");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM achievements WHERE learner_id='lp1' AND track_id='trk1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn professional_emits_badge_and_certificate() {
        let conn = fresh_conn();
        // All 4 mastered; avg 0.9125 >= 0.85; m2 lab passed (0.80).
        seed_track(
            &conn,
            "trk1",
            "lp1",
            "Kubernetes",
            &[
                ("m1", 0.90, false, 0.0),
                ("m2", 0.88, true, 0.80),
                ("m3", 0.95, false, 0.0),
                ("m4", 0.92, false, 0.0),
            ],
        );
        let issued = maybe_issue(
            &conn,
            "trk1",
            "lp1",
            &empty_key_slot(),
            fresh_key_dir().path(),
        )
        .expect("issue");
        // 4 rows: 3 badges + 1 certificate.
        assert_eq!(issued.len(), 4);
        let kinds: std::collections::HashSet<(String, String)> = issued
            .iter()
            .map(|a| (a.kind.clone(), a.level.clone()))
            .collect();
        assert!(kinds.contains(&("badge".into(), "Associate".into())));
        assert!(kinds.contains(&("badge".into(), "Practitioner".into())));
        assert!(kinds.contains(&("badge".into(), "Professional".into())));
        assert!(kinds.contains(&("certificate".into(), "Completion".into())));
    }

    #[test]
    fn professional_blocked_when_practical_lab_missing() {
        let conn = fresh_conn();
        // m2 practical_required=true but practical_mastery 0.3 < 0.7.
        seed_track(
            &conn,
            "trk1",
            "lp1",
            "Kubernetes",
            &[
                ("m1", 0.90, false, 0.0),
                ("m2", 0.88, true, 0.30),
                ("m3", 0.95, false, 0.0),
                ("m4", 0.92, false, 0.0),
            ],
        );
        let issued = maybe_issue(
            &conn,
            "trk1",
            "lp1",
            &empty_key_slot(),
            fresh_key_dir().path(),
        )
        .expect("issue");
        let levels: Vec<&str> = issued.iter().map(|a| a.level.as_str()).collect();
        assert!(levels.contains(&"Associate") && levels.contains(&"Practitioner"));
        assert!(
            !levels.contains(&"Professional"),
            "missing labs blocks Professional"
        );
        assert!(!levels.contains(&"Completion"));
    }

    /// R4 / D-04 — mastery decay must NOT re-issue or mutate prior rows.
    #[test]
    fn immutability_under_decay() {
        let conn = fresh_conn();
        seed_track(&conn, "trk1", "lp1", "Kubernetes", ONE_OF_FOUR);
        let key_slot = empty_key_slot();
        let key_dir = fresh_key_dir();
        let first = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("issue");
        assert_eq!(first.len(), 1);
        let original = first[0].clone();

        // Decay m1 below 0.7.
        conn.execute(
            "UPDATE module_progress SET mastery_level = 0.40 WHERE module_id = 'm1' AND learner_id = 'lp1'",
            [],
        ).unwrap();

        let second =
            maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("re-issue");
        assert!(second.is_empty(), "decay must NOT trigger re-issuance");

        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM achievements WHERE learner_id='lp1' AND track_id='trk1' AND level='Associate'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1, "row must remain after decay");

        let (sig_db, score_db): (String, f64) = conn
            .query_row(
                "SELECT signature, mastery_score FROM achievements WHERE id = ?1",
                [&original.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(sig_db, original.signature, "signature unchanged");
        assert!(
            (score_db - original.mastery_score).abs() < 1e-9,
            "snapshot unchanged"
        );
    }

    /// Canonical bytes of payload_json verify against the on-disk public key.
    #[test]
    fn signed_payload_round_trips() {
        let conn = fresh_conn();
        seed_track(&conn, "trk1", "lp1", "Kubernetes", ONE_OF_FOUR);
        let key_slot = empty_key_slot();
        let key_dir = fresh_key_dir();
        let issued = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("issue");
        assert_eq!(issued.len(), 1);
        let public_pem = signing::read_public_pem(key_dir.path()).expect("public pem");
        let a = &issued[0];
        assert!(signing::verify_payload(
            &public_pem,
            a.payload_json.as_bytes(),
            &a.signature
        ));
        let mut tampered = a.payload_json.clone();
        tampered.push(' ');
        assert!(!signing::verify_payload(
            &public_pem,
            tampered.as_bytes(),
            &a.signature
        ));
    }

    #[test]
    fn empty_track_returns_no_issuance() {
        let conn = fresh_conn();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'T')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk-empty', 'lp1', 'X', 'd', 'g')",
            []
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES ('p-empty', 'trk-empty', 1, '[]', '[]', 'test')",
            []
        ).unwrap();
        let issued = maybe_issue(
            &conn,
            "trk-empty",
            "lp1",
            &empty_key_slot(),
            fresh_key_dir().path(),
        )
        .expect("issue");
        assert!(issued.is_empty(), "no modules = no achievements");
    }

    /// Wave 0 RED test — turns GREEN here.
    #[test]
    fn maybe_issue_idempotent() {
        let conn = fresh_conn();
        seed_track(&conn, "trk-x", "lnr-x", "Topic", ONE_OF_FOUR);
        let key_slot: Mutex<Option<SigningKey>> = Mutex::new(None);
        let key_dir = fresh_key_dir();
        let _key_path: PathBuf = key_dir.path().to_path_buf();
        let first = maybe_issue(&conn, "trk-x", "lnr-x", &key_slot, key_dir.path()).expect("first");
        let second =
            maybe_issue(&conn, "trk-x", "lnr-x", &key_slot, key_dir.path()).expect("second");
        assert!(!first.is_empty());
        assert!(
            second.is_empty(),
            "second call must yield no new achievements"
        );
    }

    // ── CR-03 (pack_id provenance) tests ──────────────────────────────

    /// Variant of seed_track that lets each test choose the
    /// `learning_paths.generated_by_model` value — the column we now
    /// parse `topic-pack:<id>` out of inside
    /// `SqliteAchievementStore::lookup_issuance_context`.
    fn seed_track_with_model(
        conn: &Connection,
        track_id: &str,
        learner_id: &str,
        topic: &str,
        generated_by_model: &str,
        modules: &[(&str, f64, bool, f64)],
    ) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id, display_name) VALUES (?1, 'Alice')",
            [learner_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, ?2, ?3, 'devops', 'Cert')",
            rusqlite::params![track_id, learner_id, topic],
        ).unwrap();
        let path_id = format!("p-{}", track_id);
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES (?1, ?2, 1, '[]', '[]', ?3)",
            rusqlite::params![path_id, track_id, generated_by_model],
        ).unwrap();
        for (i, (mid, ml, pr, pm)) in modules.iter().enumerate() {
            let content_json = if *pr { r#"{"practical_required": true}"# } else { "{}" };
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering, content_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![mid, path_id, format!("M{}", i), i as i64, content_json],
            ).unwrap();
            conn.execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level, practical_mastery) VALUES (?1, ?2, ?3, 'in_progress', ?4, ?5)",
                rusqlite::params![format!("mp-{}", mid), mid, learner_id, ml, pm],
            ).unwrap();
        }
    }

    /// CR-03 — pack-sourced tracks (Phase 5 generate_path_from_pack
    /// writes `learning_paths.generated_by_model = "topic-pack:<id>"`)
    /// must propagate the pack id into the signed CertPayloadV1 AND the
    /// `achievements.pack_id` column.
    #[test]
    fn pack_sourced_track_writes_pack_id_to_payload_and_row() {
        let conn = fresh_conn();
        seed_track_with_model(
            &conn,
            "trk-pk",
            "lp1",
            "Kubernetes",
            "topic-pack:k8s-fundamentals",
            ONE_OF_FOUR,
        );
        let key_slot = empty_key_slot();
        let key_dir = fresh_key_dir();
        let issued =
            maybe_issue(&conn, "trk-pk", "lp1", &key_slot, key_dir.path()).expect("issue");
        assert_eq!(issued.len(), 1, "1/4 mastered = Associate only");

        // Persisted row carries the pack_id snapshot.
        let row: Option<String> = conn
            .query_row(
                "SELECT pack_id FROM achievements WHERE id = ?1",
                [&issued[0].id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            row.as_deref(),
            Some("k8s-fundamentals"),
            "achievements.pack_id must hold the parsed pack id"
        );

        // Signed payload carries the same pack_id.
        let payload: CertPayloadV1 =
            serde_json::from_str(&issued[0].payload_json).expect("parse v1 payload");
        assert_eq!(
            payload.pack_id.as_deref(),
            Some("k8s-fundamentals"),
            "CertPayloadV1.packId must carry the pack provenance"
        );

        // In-memory Achievement struct also reflects it (used by IPC consumers).
        assert_eq!(issued[0].pack_id.as_deref(), Some("k8s-fundamentals"));
    }

    /// CR-03 — free-text / AI-generated tracks (no `topic-pack:` prefix)
    /// still issue achievements with pack_id = None.
    #[test]
    fn free_text_track_keeps_pack_id_none() {
        let conn = fresh_conn();
        seed_track_with_model(
            &conn,
            "trk-free",
            "lp1",
            "Kubernetes",
            "gpt-4o-mini", // arbitrary non-pack model id
            ONE_OF_FOUR,
        );
        let issued = maybe_issue(
            &conn,
            "trk-free",
            "lp1",
            &empty_key_slot(),
            fresh_key_dir().path(),
        )
        .expect("issue");
        assert_eq!(issued.len(), 1);

        let row: Option<String> = conn
            .query_row(
                "SELECT pack_id FROM achievements WHERE id = ?1",
                [&issued[0].id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            row.is_none(),
            "AI-generated tracks must NOT claim pack provenance"
        );

        let payload: CertPayloadV1 =
            serde_json::from_str(&issued[0].payload_json).expect("parse v1 payload");
        assert!(payload.pack_id.is_none());
        assert!(issued[0].pack_id.is_none());
    }

    /// CR-03 edge case — legacy track that predates the Phase 5 column
    /// regime (no learning_paths row at all). Threshold reads
    /// module_progress by learner+track via the
    /// modules→learning_paths→learning_tracks chain. The legacy track
    /// has no learning_paths row, so the aggregate yields 0 modules and
    /// maybe_issue returns Ok([]) without ever calling
    /// lookup_issuance_context. That's the safe "no provenance, no
    /// crash" outcome.
    #[test]
    fn track_without_learning_paths_row_issues_with_pack_id_none() {
        let conn = fresh_conn();
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id, display_name) VALUES ('lp1', 'Alice')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk-legacy', 'lp1', 'Kubernetes', 'devops', 'Cert')",
            [],
        )
        .unwrap();
        // Orphan path linked to a *different* track to demonstrate the
        // lookup returns no rows for trk-legacy.
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk-other', 'lp1', 'Other', 'devops', 'Cert')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES ('p-other', 'trk-other', 1, '[]', '[]', 'topic-pack:other')",
            [],
        ).unwrap();
        for (i, (mid, ml)) in [("ml1", 0.75), ("ml2", 0.40), ("ml3", 0.30), ("ml4", 0.30)]
            .iter()
            .enumerate()
        {
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering, content_json) VALUES (?1, 'p-other', ?2, ?3, '{}')",
                rusqlite::params![mid, format!("M{}", i), i as i64],
            ).unwrap();
            conn.execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level, practical_mastery) VALUES (?1, ?2, 'lp1', 'in_progress', ?3, 0.0)",
                rusqlite::params![format!("mp-{}", mid), mid, ml],
            ).unwrap();
        }

        let issued = maybe_issue(
            &conn,
            "trk-legacy",
            "lp1",
            &empty_key_slot(),
            fresh_key_dir().path(),
        )
        .expect("must not crash on legacy track shape");
        assert!(
            issued.is_empty(),
            "legacy track without learning_paths has no modules to grade — no issuance"
        );
    }
}
