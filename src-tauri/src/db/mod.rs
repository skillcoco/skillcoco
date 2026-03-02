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
        self.conn.execute_batch(schema::CREATE_TABLES)?;
        log::info!("Database migrations completed");
        Ok(())
    }
}
