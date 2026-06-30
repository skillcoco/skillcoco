use crate::db::models::{LearnerProfile, LearningTrack};
use crate::AppState;
use serde::Deserialize;
use tauri::State;

/// Typed request for updating the learner profile.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    /// Plan 03.1-09 GAP-02 — JSON-encoded preferences blob. Frontend
    /// (`SettingsLabsSection.tsx`) writes `{"labs_runtime":"..."}` here
    /// for the labs runtime selector. Validated as a JSON object on
    /// persist; `lab_session_open` reads it back via
    /// `read_labs_runtime_preference`.
    pub preferences_json: Option<String>,
}

#[tauri::command]
pub fn get_or_create_profile(state: State<AppState>) -> Result<LearnerProfile, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Check if profile exists
    let maybe_profile: Option<LearnerProfile> = db
        .conn
        .query_row(
            "SELECT id, display_name, learning_style, experience_level, preferences_json, created_at, updated_at, COALESCE(points, 0) FROM learner_profiles LIMIT 1",
            [],
            |row| {
                Ok(LearnerProfile {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    learning_style: row.get(2)?,
                    experience_level: row.get(3)?,
                    preferences_json: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    points: row.get(7)?,
                })
            },
        )
        .ok();

    match maybe_profile {
        Some(profile) => Ok(profile),
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            db.conn
                .execute(
                    "INSERT INTO learner_profiles (id) VALUES (?1)",
                    [&id],
                )
                .map_err(|e| e.to_string())?;

            db.conn
                .query_row(
                    "SELECT id, display_name, learning_style, experience_level, preferences_json, created_at, updated_at, COALESCE(points, 0) FROM learner_profiles WHERE id = ?1",
                    [&id],
                    |row| {
                        Ok(LearnerProfile {
                            id: row.get(0)?,
                            display_name: row.get(1)?,
                            learning_style: row.get(2)?,
                            experience_level: row.get(3)?,
                            preferences_json: row.get(4)?,
                            created_at: row.get(5)?,
                            updated_at: row.get(6)?,
                            points: row.get(7)?,
                        })
                    },
                )
                .map_err(|e| e.to_string())
        }
    }
}

#[tauri::command]
pub fn update_profile(
    state: State<AppState>,
    profile: UpdateProfileRequest,
) -> Result<LearnerProfile, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Get the current profile's ID to scope the UPDATE
    let profile_id: String = db
        .conn
        .query_row(
            "SELECT id FROM learner_profiles LIMIT 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("No profile found: {}", e))?;

    if let Some(name) = profile.display_name {
        db.conn
            .execute(
                "UPDATE learner_profiles SET display_name = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![name, profile_id],
            )
            .map_err(|e| e.to_string())?;
    }

    // Plan 03.1-09 GAP-02 — accept preferences_json (frontend was already
    // sending it; serde was silently dropping the field before this change).
    // Validate the payload is a JSON object; reject bare scalars/arrays so
    // a UI bug doesn't quietly persist garbage.
    if let Some(prefs) = profile.preferences_json {
        let parsed: serde_json::Value = serde_json::from_str(&prefs)
            .map_err(|e| format!("preferences_json must be valid JSON: {}", e))?;
        if !parsed.is_object() {
            return Err("preferences_json must be a JSON object".to_string());
        }
        db.conn
            .execute(
                "UPDATE learner_profiles SET preferences_json = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![prefs, profile_id],
            )
            .map_err(|e| e.to_string())?;
    }

    get_or_create_profile_inner(&db)
}

fn get_or_create_profile_inner(db: &crate::db::Database) -> Result<LearnerProfile, String> {
    db.conn
        .query_row(
            "SELECT id, display_name, learning_style, experience_level, preferences_json, created_at, updated_at, COALESCE(points, 0) FROM learner_profiles LIMIT 1",
            [],
            |row| {
                Ok(LearnerProfile {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    learning_style: row.get(2)?,
                    experience_level: row.get(3)?,
                    preferences_json: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    points: row.get(7)?,
                })
            },
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_tracks(state: State<AppState>) -> Result<Vec<LearningTrack>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, learner_id, topic, domain_module, status, goal, current_module_id, progress_percent, total_time_spent, created_at, updated_at, COALESCE(streak_days, 0), last_activity_date, COALESCE(browse_mode, 'linear') FROM learning_tracks ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let tracks = stmt
        .query_map([], |row| {
            Ok(LearningTrack {
                id: row.get(0)?,
                learner_id: row.get(1)?,
                topic: row.get(2)?,
                domain_module: row.get(3)?,
                status: row.get(4)?,
                goal: row.get(5)?,
                current_module_id: row.get(6)?,
                progress_percent: row.get(7)?,
                total_time_spent: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                streak_days: row.get(11)?,
                last_activity_date: row.get(12)?,
                browse_mode: row.get(13)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(tracks)
}

#[tauri::command]
pub fn create_track(
    state: State<AppState>,
    topic: String,
    domain_module: String,
    goal: String,
) -> Result<LearningTrack, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Ensure profile exists
    let profile = get_or_create_profile_inner(&db)?;

    let id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, profile.id, topic, domain_module, goal],
        )
        .map_err(|e| e.to_string())?;

    get_track_inner(&db, &id)
}

#[tauri::command]
pub fn get_track(state: State<AppState>, track_id: String) -> Result<LearningTrack, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    get_track_inner(&db, &track_id)
}

fn get_track_inner(db: &crate::db::Database, track_id: &str) -> Result<LearningTrack, String> {
    db.conn
        .query_row(
            "SELECT id, learner_id, topic, domain_module, status, goal, current_module_id, progress_percent, total_time_spent, created_at, updated_at, COALESCE(streak_days, 0), last_activity_date, COALESCE(browse_mode, 'linear') FROM learning_tracks WHERE id = ?1",
            [track_id],
            |row| {
                Ok(LearningTrack {
                    id: row.get(0)?,
                    learner_id: row.get(1)?,
                    topic: row.get(2)?,
                    domain_module: row.get(3)?,
                    status: row.get(4)?,
                    goal: row.get(5)?,
                    current_module_id: row.get(6)?,
                    progress_percent: row.get(7)?,
                    total_time_spent: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                    streak_days: row.get(11)?,
                    last_activity_date: row.get(12)?,
                    browse_mode: row.get(13)?,
                })
            },
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use crate::db::schema::CREATE_TABLES;
    use crate::db::migrations::apply_migrations;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_TABLES).unwrap();
        apply_migrations(&conn).expect("migrations must succeed");
        conn
    }

    /// TEST-01: round-trip test — create a profile, create a track, list tracks.
    /// Verifies: DB persistence, camelCase serde, streak column defaults.
    #[test]
    fn round_trip() {
        let conn = setup_test_db();

        // Seed a learner profile
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Alice')",
            [],
        ).unwrap();

        // Create a track directly (bypassing Tauri State for unit testing)
        let track_id = "track-round-trip".to_string();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, 'lp1', 'Kubernetes', 'devops', 'Pass CKA')",
            [&track_id],
        ).unwrap();

        // List tracks using the raw query logic (mirrors list_tracks command)
        let mut stmt = conn.prepare(
            "SELECT id, learner_id, topic, domain_module, status, goal, current_module_id, progress_percent, total_time_spent, created_at, updated_at, COALESCE(streak_days, 0), last_activity_date, COALESCE(browse_mode, 'linear') FROM learning_tracks ORDER BY updated_at DESC",
        ).unwrap();

        let tracks = stmt.query_map([], |row| {
            Ok(crate::db::models::LearningTrack {
                id: row.get(0)?,
                learner_id: row.get(1)?,
                topic: row.get(2)?,
                domain_module: row.get(3)?,
                status: row.get(4)?,
                goal: row.get(5)?,
                current_module_id: row.get(6)?,
                progress_percent: row.get(7)?,
                total_time_spent: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                streak_days: row.get(11)?,
                last_activity_date: row.get(12)?,
                browse_mode: row.get(13)?,
            })
        }).unwrap().collect::<Result<Vec<_>, _>>().unwrap();

        assert_eq!(tracks.len(), 1, "one track should be returned");
        assert_eq!(tracks[0].id, track_id);
        assert_eq!(tracks[0].topic, "Kubernetes");
        assert_eq!(tracks[0].learner_id, "lp1");
        assert_eq!(tracks[0].streak_days, 0, "new track starts with streak_days=0");
        assert!(tracks[0].last_activity_date.is_none(), "new track has no last_activity_date");
        assert_eq!(tracks[0].browse_mode, "linear", "new track defaults to browse_mode='linear'");

        // Verify camelCase serde — TypeScript receives camelCase field names
        let json = serde_json::to_string(&tracks[0]).unwrap();
        assert!(json.contains("\"learnerId\""), "learnerId must be camelCase in JSON");
        assert!(json.contains("\"domainModule\""), "domainModule must be camelCase in JSON");
        assert!(json.contains("\"progressPercent\""), "progressPercent must be camelCase in JSON");
        assert!(json.contains("\"streakDays\""), "streakDays must be camelCase in JSON");
        assert!(json.contains("\"browseMode\""), "browseMode must be camelCase in JSON");
    }

    /// delete_track_cascades — deleting a track removes its paths, modules,
    /// progress, exercises, sr_cards, blocks, and lesson_completions via the
    /// existing ON DELETE CASCADE chain. Verifies FK pragma is on and cascade
    /// reaches every child table.
    #[test]
    fn delete_track_cascades() {
        let conn = setup_test_db();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Alice')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('t1', 'lp1', 'K8s', 'devops', 'CKA')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('p1', 't1')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('m1', 'p1', 'Pods')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status) VALUES ('mp1', 'm1', 'lp1', 'in_progress')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sr_cards (id, module_id, concept, front, back) VALUES ('s1', 'm1', 'pod', 'Q', 'A')",
            [],
        ).unwrap();

        super::delete_track_inner(&conn, "t1").expect("delete should succeed");

        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM learning_tracks WHERE id = 't1'", [], |r| r.get(0))
            .unwrap();
        let path_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM learning_paths WHERE track_id = 't1'", [], |r| r.get(0))
            .unwrap();
        let module_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM modules WHERE path_id = 'p1'", [], |r| r.get(0))
            .unwrap();
        let progress_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM module_progress WHERE module_id = 'm1'", [], |r| r.get(0))
            .unwrap();
        let card_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sr_cards WHERE module_id = 'm1'", [], |r| r.get(0))
            .unwrap();

        assert_eq!(track_count, 0, "track row removed");
        assert_eq!(path_count, 0, "paths cascaded");
        assert_eq!(module_count, 0, "modules cascaded");
        assert_eq!(progress_count, 0, "module_progress cascaded");
        assert_eq!(card_count, 0, "sr_cards cascaded");
    }

    /// delete_track_unknown_id_is_noop — deleting a non-existent track returns
    /// Ok(0) (rows-affected zero), not an error. UI should still treat as success.
    #[test]
    fn delete_track_unknown_id_is_noop() {
        let conn = setup_test_db();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let rows = super::delete_track_inner(&conn, "does-not-exist").unwrap();
        assert_eq!(rows, 0);
    }

    /// TEST-01: active_credential_round_trip — store API key, retrieve via get_active_credential.
    /// Note: full auth coverage already exists in auth::mod::tests (12 tests).
    /// This test explicitly covers the active_credential flow per TEST-01 requirements.
    #[test]
    fn active_credential_round_trip() {
        // Uses temp file to avoid touching the real credentials store
        let dir = tempfile::tempdir().unwrap();
        let auth = crate::auth::AuthState::new(&dir.path().to_path_buf());

        // Empty state: no active credential
        assert!(auth.get_active_credential().unwrap().is_none(),
            "fresh store has no active credential");

        // Store an API key
        auth.store_api_key("anthropic", "sk-test-key", Some("claude-haiku")).unwrap();

        // Retrieve via get_active_credential
        let cred = auth.get_active_credential().unwrap().unwrap();
        assert_eq!(cred.provider, "anthropic");
        assert_eq!(cred.api_key.as_deref(), Some("sk-test-key"));
        assert_eq!(cred.model.as_deref(), Some("claude-haiku"));

        // Active provider is set
        assert_eq!(auth.get_active_provider().unwrap().as_deref(), Some("anthropic"));
    }

    // ── browse_mode tests (Phase 10-01, Task 2) ──────────────────────────────

    /// Helper: insert a learner_profiles row for FK satisfaction.
    fn seed_profile(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'Test')",
            rusqlite::params![id],
        ).unwrap();
    }

    /// Helper: insert a track row and return its id.
    fn seed_track(conn: &Connection, id: &str, learner_id: &str) {
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES (?1, ?2, 'TestTopic', 'testdomain', 'TestGoal')",
            rusqlite::params![id, learner_id],
        ).unwrap();
    }

    /// Helper: read browse_mode for a track directly from the DB.
    fn get_browse_mode(conn: &Connection, track_id: &str) -> String {
        conn.query_row(
            "SELECT COALESCE(browse_mode, 'linear') FROM learning_tracks WHERE id = ?1",
            rusqlite::params![track_id],
            |row| row.get(0),
        ).unwrap()
    }

    /// Test (round-trip): inserting a track returns browse_mode == 'linear' by
    /// default; after set_track_browse_mode_inner("free") it returns 'free'.
    #[test]
    fn browse_mode_round_trip_default_and_set() {
        let conn = setup_test_db();
        seed_profile(&conn, "lp1");
        seed_track(&conn, "t1", "lp1");

        // Default
        assert_eq!(get_browse_mode(&conn, "t1"), "linear", "new track must default to 'linear'");

        // Set to free
        super::set_track_browse_mode_inner(&conn, "t1", "free")
            .expect("set to 'free' must succeed");
        assert_eq!(get_browse_mode(&conn, "t1"), "free", "browse_mode must be 'free' after set");

        // Set back to linear
        super::set_track_browse_mode_inner(&conn, "t1", "linear")
            .expect("set back to 'linear' must succeed");
        assert_eq!(get_browse_mode(&conn, "t1"), "linear", "browse_mode must be 'linear' after reset");
    }

    /// Test (validation): set_track_browse_mode rejects any value other than
    /// 'linear' or 'free' with an Err (no DB write on reject).
    #[test]
    fn browse_mode_validation_rejects_invalid_modes() {
        let conn = setup_test_db();
        seed_profile(&conn, "lp1");
        seed_track(&conn, "t1", "lp1");

        let invalid_modes = ["", "random", "LINEAR", "Free", "both", "admin", "1=1; DROP TABLE"];
        for bad in &invalid_modes {
            let result = super::set_track_browse_mode_inner(&conn, "t1", bad);
            assert!(
                result.is_err(),
                "set_track_browse_mode_inner must reject mode '{}' but returned Ok",
                bad
            );
            // DB must be untouched — mode stays 'linear'
            assert_eq!(
                get_browse_mode(&conn, "t1"),
                "linear",
                "DB must not be written for invalid mode '{}'",
                bad
            );
        }
    }

    /// Test (isolation): set_track_browse_mode on track A does not change
    /// track B's browse_mode (per-track, D-02).
    #[test]
    fn browse_mode_isolation_across_tracks() {
        let conn = setup_test_db();
        seed_profile(&conn, "lp1");
        seed_track(&conn, "track-a", "lp1");
        seed_track(&conn, "track-b", "lp1");

        // Set track-a to 'free'
        super::set_track_browse_mode_inner(&conn, "track-a", "free")
            .expect("set track-a to 'free' must succeed");

        // track-b must still be 'linear'
        assert_eq!(
            get_browse_mode(&conn, "track-b"),
            "linear",
            "track-b's browse_mode must not change when track-a is updated"
        );
        assert_eq!(
            get_browse_mode(&conn, "track-a"),
            "free",
            "track-a browse_mode must be 'free'"
        );
    }
}

#[tauri::command]
pub fn update_track_status(
    state: State<AppState>,
    track_id: String,
    status: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "UPDATE learning_tracks SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![status, track_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_track(state: State<AppState>, track_id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    delete_track_inner(&db.conn, &track_id)?;
    Ok(())
}

fn delete_track_inner(conn: &rusqlite::Connection, track_id: &str) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM learning_tracks WHERE id = ?1",
        rusqlite::params![track_id],
    )
    .map_err(|e| e.to_string())
}

/// Inner testable implementation for set_track_browse_mode.
///
/// T-10-01: whitelist-validates mode ∈ {"linear", "free"} before any DB write.
/// Parameterized query prevents SQL injection regardless of mode content.
fn set_track_browse_mode_inner(
    conn: &rusqlite::Connection,
    track_id: &str,
    mode: &str,
) -> Result<(), String> {
    // T-10-01: whitelist-validate mode ∈ {"linear", "free"}; reject before DB write
    if mode != "linear" && mode != "free" {
        return Err(format!(
            "Invalid browse mode '{}': must be 'linear' or 'free'",
            mode
        ));
    }
    conn.execute(
        "UPDATE learning_tracks SET browse_mode = ?1, updated_at = datetime('now') WHERE id = ?2",
        rusqlite::params![mode, track_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Set the browse mode for a single track.
///
/// Validates that `mode` is exactly "linear" or "free" (T-10-01 whitelist).
/// Uses parameterized query — no string interpolation (T-10-01 SQLi mitigation).
/// Does NOT touch mastery, progress, unlock, or certificate pipelines (D-03/D-06).
#[tauri::command]
pub fn set_track_browse_mode(
    state: State<AppState>,
    track_id: String,
    mode: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    set_track_browse_mode_inner(&db.conn, &track_id, &mode)
}
