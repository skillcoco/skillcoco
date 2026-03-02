/// SQL schema for LearnForge's SQLite database.
/// All tables are created idempotently with IF NOT EXISTS.
pub const CREATE_TABLES: &str = r#"
-- Learner profile (single user for desktop app)
CREATE TABLE IF NOT EXISTS learner_profiles (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL DEFAULT 'Learner',
    learning_style TEXT NOT NULL DEFAULT 'mixed',
    experience_level TEXT NOT NULL DEFAULT 'intermediate',
    preferences_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Learning tracks
CREATE TABLE IF NOT EXISTS learning_tracks (
    id TEXT PRIMARY KEY,
    learner_id TEXT NOT NULL REFERENCES learner_profiles(id),
    topic TEXT NOT NULL,
    domain_module TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'onboarding',
    goal TEXT NOT NULL DEFAULT '',
    current_module_id TEXT,
    progress_percent REAL NOT NULL DEFAULT 0.0,
    total_time_spent INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- AI-generated learning paths (DAGs)
CREATE TABLE IF NOT EXISTS learning_paths (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL REFERENCES learning_tracks(id) ON DELETE CASCADE,
    version INTEGER NOT NULL DEFAULT 1,
    generated_by_model TEXT NOT NULL DEFAULT '',
    modules_json TEXT NOT NULL DEFAULT '[]',
    edges_json TEXT NOT NULL DEFAULT '[]',
    estimated_hours REAL NOT NULL DEFAULT 0.0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Individual modules within a path
CREATE TABLE IF NOT EXISTS modules (
    id TEXT PRIMARY KEY,
    path_id TEXT NOT NULL REFERENCES learning_paths(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    module_type TEXT NOT NULL DEFAULT 'lesson',
    difficulty INTEGER NOT NULL DEFAULT 5,
    estimated_minutes INTEGER NOT NULL DEFAULT 30,
    objectives_json TEXT NOT NULL DEFAULT '[]',
    prerequisites_json TEXT NOT NULL DEFAULT '[]',
    content_json TEXT NOT NULL DEFAULT '{}',
    content TEXT,
    content_generated_at TEXT,
    ordering INTEGER NOT NULL DEFAULT 0
);

-- Progress tracking per module per learner
CREATE TABLE IF NOT EXISTS module_progress (
    id TEXT PRIMARY KEY,
    module_id TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    learner_id TEXT NOT NULL REFERENCES learner_profiles(id),
    status TEXT NOT NULL DEFAULT 'locked',
    score REAL,
    time_spent INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    mastery_level REAL NOT NULL DEFAULT 0.0,
    started_at TEXT,
    completed_at TEXT,
    UNIQUE(module_id, learner_id)
);

-- Exercises
CREATE TABLE IF NOT EXISTS exercises (
    id TEXT PRIMARY KEY,
    module_id TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    exercise_type TEXT NOT NULL,
    difficulty INTEGER NOT NULL DEFAULT 5,
    prompt TEXT NOT NULL,
    hints_json TEXT NOT NULL DEFAULT '[]',
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

-- Exercise attempts
CREATE TABLE IF NOT EXISTS exercise_attempts (
    id TEXT PRIMARY KEY,
    exercise_id TEXT NOT NULL REFERENCES exercises(id) ON DELETE CASCADE,
    learner_id TEXT NOT NULL REFERENCES learner_profiles(id),
    response TEXT NOT NULL,
    score REAL NOT NULL DEFAULT 0.0,
    feedback TEXT NOT NULL DEFAULT '',
    time_spent INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Spaced repetition cards
CREATE TABLE IF NOT EXISTS sr_cards (
    id TEXT PRIMARY KEY,
    module_id TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    concept TEXT NOT NULL,
    card_type TEXT NOT NULL DEFAULT 'active_recall',
    front TEXT NOT NULL,
    back TEXT NOT NULL,
    interval_days REAL NOT NULL DEFAULT 1.0,
    ease_factor REAL NOT NULL DEFAULT 2.5,
    repetitions INTEGER NOT NULL DEFAULT 0,
    next_review TEXT NOT NULL DEFAULT (datetime('now')),
    last_review TEXT
);

-- AI conversation history
CREATE TABLE IF NOT EXISTS ai_conversations (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL REFERENCES learning_tracks(id) ON DELETE CASCADE,
    module_id TEXT,
    messages_json TEXT NOT NULL DEFAULT '[]',
    model_used TEXT NOT NULL DEFAULT '',
    token_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Adaptation audit log
CREATE TABLE IF NOT EXISTS adaptation_events (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL REFERENCES learning_tracks(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    old_value TEXT NOT NULL DEFAULT '',
    new_value TEXT NOT NULL DEFAULT '',
    reason TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- AI provider configuration
CREATE TABLE IF NOT EXISTS ai_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    provider_type TEXT NOT NULL DEFAULT 'claude',
    api_key TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT 'claude-sonnet-4-20250514',
    base_url TEXT NOT NULL DEFAULT '',
    max_tokens INTEGER NOT NULL DEFAULT 4096,
    temperature REAL NOT NULL DEFAULT 0.7
);

-- Insert default AI config if not exists
INSERT OR IGNORE INTO ai_config (id) VALUES (1);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_tracks_learner ON learning_tracks(learner_id);
CREATE INDEX IF NOT EXISTS idx_paths_track ON learning_paths(track_id);
CREATE INDEX IF NOT EXISTS idx_modules_path ON modules(path_id);
CREATE INDEX IF NOT EXISTS idx_progress_module ON module_progress(module_id);
CREATE INDEX IF NOT EXISTS idx_progress_learner ON module_progress(learner_id);
CREATE INDEX IF NOT EXISTS idx_sr_cards_next_review ON sr_cards(next_review);
CREATE INDEX IF NOT EXISTS idx_exercises_module ON exercises(module_id);
"#;
