pub mod microlearning;
pub mod migrations;
pub mod models;
pub mod schema;

use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn new(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read performance
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let db = Database { conn };
        db.run_migrations()?;

        Ok(db)
    }

    fn run_migrations(&self) -> Result<(), rusqlite::Error> {
        // Step 1: Apply base schema idempotently (CREATE TABLE IF NOT EXISTS)
        self.conn.execute_batch(schema::CREATE_TABLES)?;
        // Step 2: Apply version-gated migrations (ALTER TABLE, new columns, etc.)
        migrations::apply_migrations(&self.conn)?;
        log::info!("Database migrations completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn test_db() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let db = Database { conn };
        db.run_migrations().unwrap();
        db
    }

    #[test]
    fn test_database_init_creates_tables() {
        let db = test_db();
        let tables: Vec<String> = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"learner_profiles".to_string()));
        assert!(tables.contains(&"learning_tracks".to_string()));
        assert!(tables.contains(&"learning_paths".to_string()));
        assert!(tables.contains(&"modules".to_string()));
        assert!(tables.contains(&"module_progress".to_string()));
        assert!(tables.contains(&"sr_cards".to_string()));
        assert!(tables.contains(&"exercises".to_string()));
        // ai_config table removed in FIX-03 — auth flows through AuthState
        assert!(!tables.contains(&"ai_config".to_string()), "ai_config must NOT exist after FIX-03 migration");
    }

    #[test]
    fn test_migrations_idempotent() {
        let db = test_db();
        // Run migrations again - should not fail
        db.run_migrations().unwrap();
    }

    #[test]
    fn test_ai_config_table_removed() {
        // FIX-03: ai_config table must be absent after migrations run
        let db = test_db();
        let count: i32 = db.conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='ai_config'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "ai_config table must be removed by FIX-03 migration v002");
    }

    #[test]
    fn test_learner_profile_crud() {
        let db = test_db();
        let id = uuid::Uuid::new_v4().to_string();

        db.conn.execute("INSERT INTO learner_profiles (id) VALUES (?1)", [&id]).unwrap();

        let name: String = db
            .conn
            .query_row("SELECT display_name FROM learner_profiles WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Learner"); // default

        db.conn.execute("UPDATE learner_profiles SET display_name = 'Alice' WHERE id = ?1", [&id]).unwrap();
        let name: String = db
            .conn
            .query_row("SELECT display_name FROM learner_profiles WHERE id = ?1", [&id], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Alice");
    }

    #[test]
    fn test_track_creation() {
        let db = test_db();
        let profile_id = uuid::Uuid::new_v4().to_string();
        let track_id = uuid::Uuid::new_v4().to_string();

        db.conn.execute("INSERT INTO learner_profiles (id) VALUES (?1)", [&profile_id]).unwrap();
        db.conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![track_id, profile_id, "Kubernetes", "devops", "Pass CKA"],
        ).unwrap();

        let topic: String = db
            .conn
            .query_row("SELECT topic FROM learning_tracks WHERE id = ?1", [&track_id], |r| r.get(0))
            .unwrap();
        assert_eq!(topic, "Kubernetes");

        let status: String = db
            .conn
            .query_row("SELECT status FROM learning_tracks WHERE id = ?1", [&track_id], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "onboarding"); // default
    }

    #[test]
    fn test_learning_path_with_modules() {
        let db = test_db();
        let profile_id = uuid::Uuid::new_v4().to_string();
        let track_id = uuid::Uuid::new_v4().to_string();
        let path_id = uuid::Uuid::new_v4().to_string();
        let mod_id = uuid::Uuid::new_v4().to_string();

        db.conn.execute("INSERT INTO learner_profiles (id) VALUES (?1)", [&profile_id]).unwrap();
        db.conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module) VALUES (?1, ?2, ?3, ?4)",
            params![track_id, profile_id, "Rust", "programming"],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO learning_paths (id, track_id, generated_by_model) VALUES (?1, ?2, ?3)",
            params![path_id, track_id, "claude-sonnet-4-20250514"],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO modules (id, path_id, title, difficulty) VALUES (?1, ?2, ?3, ?4)",
            params![mod_id, path_id, "Ownership Basics", 3],
        ).unwrap();

        let title: String = db
            .conn
            .query_row("SELECT title FROM modules WHERE id = ?1", [&mod_id], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Ownership Basics");
    }

    #[test]
    fn test_sr_card_defaults() {
        let db = test_db();
        let profile_id = uuid::Uuid::new_v4().to_string();
        let track_id = uuid::Uuid::new_v4().to_string();
        let path_id = uuid::Uuid::new_v4().to_string();
        let mod_id = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();

        db.conn.execute("INSERT INTO learner_profiles (id) VALUES (?1)", [&profile_id]).unwrap();
        db.conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module) VALUES (?1, ?2, 'Rust', 'programming')",
            params![track_id, profile_id],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
            params![path_id, track_id],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES (?1, ?2, 'Module 1')",
            params![mod_id, path_id],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO sr_cards (id, module_id, concept, front, back) VALUES (?1, ?2, 'ownership', 'What is ownership?', 'Rust memory management')",
            params![card_id, mod_id],
        ).unwrap();

        let (interval, ef, reps): (f64, f64, i32) = db
            .conn
            .query_row(
                "SELECT interval_days, ease_factor, repetitions FROM sr_cards WHERE id = ?1",
                [&card_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(interval, 1.0);
        assert_eq!(ef, 2.5);
        assert_eq!(reps, 0);
    }

    #[test]
    fn test_foreign_key_cascade_delete() {
        let db = test_db();
        let profile_id = uuid::Uuid::new_v4().to_string();
        let track_id = uuid::Uuid::new_v4().to_string();
        let path_id = uuid::Uuid::new_v4().to_string();

        db.conn.execute("INSERT INTO learner_profiles (id) VALUES (?1)", [&profile_id]).unwrap();
        db.conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module) VALUES (?1, ?2, 'Go', 'programming')",
            params![track_id, profile_id],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
            params![path_id, track_id],
        ).unwrap();

        // Delete track - path should cascade
        db.conn.execute("DELETE FROM learning_tracks WHERE id = ?1", [&track_id]).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM learning_paths WHERE track_id = ?1", [&track_id], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_module_progress_unique_constraint() {
        let db = test_db();
        let profile_id = uuid::Uuid::new_v4().to_string();
        let track_id = uuid::Uuid::new_v4().to_string();
        let path_id = uuid::Uuid::new_v4().to_string();
        let mod_id = uuid::Uuid::new_v4().to_string();

        db.conn.execute("INSERT INTO learner_profiles (id) VALUES (?1)", [&profile_id]).unwrap();
        db.conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module) VALUES (?1, ?2, 'Rust', 'programming')",
            params![track_id, profile_id],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
            params![path_id, track_id],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES (?1, ?2, 'Module 1')",
            params![mod_id, path_id],
        ).unwrap();

        let mp_id1 = uuid::Uuid::new_v4().to_string();
        db.conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id) VALUES (?1, ?2, ?3)",
            params![mp_id1, mod_id, profile_id],
        ).unwrap();

        // Second insert with same module_id + learner_id should fail
        let mp_id2 = uuid::Uuid::new_v4().to_string();
        let result = db.conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id) VALUES (?1, ?2, ?3)",
            params![mp_id2, mod_id, profile_id],
        );
        assert!(result.is_err());
    }
}
