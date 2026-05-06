mod ai;
mod auth;
pub mod commands;
pub mod db;
pub mod labs;
pub mod learning;
mod vector;

use auth::AuthState;
use db::Database;
use labs::LabSession;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use vector::VectorState;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    /// Registry of live lab PTY/Docker sessions keyed by session UUID.
    /// Populated by `commands::labs::lab_session_open` (lands in 03.1-05) and
    /// drained by `lab_session_close` / PTY exit. The `Send` bound makes the
    /// boxed sessions safe to store in tokio task contexts.
    pub lab_sessions: Arc<Mutex<HashMap<String, Box<dyn LabSession + Send>>>>,
}

pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data dir");

            // Database
            let db_path = app_dir.join("learnforge.db");
            let database = Database::new(&db_path).expect("Failed to initialize database");
            app.manage(AppState {
                db: Arc::new(Mutex::new(database)),
                lab_sessions: Arc::new(Mutex::new(HashMap::new())),
            });

            // Auth credential store
            let auth_dir = app_dir.join("auth");
            std::fs::create_dir_all(&auth_dir).expect("Failed to create auth dir");
            app.manage(AuthState::new(&auth_dir));
            app.manage(crate::auth::oauth::OAuthFlowState::new());

            // Vector DB + Graph DB for semantic intelligence
            let vector_path = app_dir.join("vectors.db");
            let vector_state = VectorState::new(
                vector_path.to_str().unwrap_or("vectors.db"),
            )
            .expect("Failed to initialize vector store");
            app.manage(vector_state);

            log::info!("LearnForge initialized with DB at {:?}", db_path);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Track commands
            commands::tracks::list_tracks,
            commands::tracks::create_track,
            commands::tracks::get_track,
            commands::tracks::update_track_status,
            commands::tracks::delete_track,
            // Learning commands
            commands::learning::get_path,
            commands::learning::get_module_progress,
            commands::learning::update_module_progress,
            commands::learning::get_due_cards,
            commands::learning::submit_review,
            commands::learning::complete_module_exercises,
            // AI commands (get_ai_config / update_ai_config removed in FIX-03;
            // complete_module_exercises relocated to commands::learning in Plan 01-03)
            commands::ai::assess_knowledge,
            commands::ai::generate_learning_path,
            commands::ai::send_tutor_message,
            commands::ai::generate_module_content,
            commands::ai::get_exercises,
            commands::ai::generate_exercise,
            commands::ai::evaluate_response,
            // Auth commands
            auth::commands::get_auth_status,
            auth::commands::login_provider,
            auth::commands::set_active_provider,
            auth::commands::logout_provider,
            auth::commands::detect_system_providers,
            auth::oauth::start_oauth_login,
            auth::oauth::check_oauth_status,
            auth::oauth::save_setup_token,
            // Profile commands
            commands::tracks::get_or_create_profile,
            commands::tracks::update_profile,
            // Block commands (Phase 3 — Wave 2: full pipeline + regeneration, 03-03)
            commands::blocks::get_module_blocks,
            commands::blocks::generate_module_blocks,
            commands::blocks::regenerate_lesson,
            commands::blocks::regenerate_module,
            // Quiz + Flash Card commands (Phase 3 — Wave 2 BKT re-rooting, 03-04)
            commands::learning::submit_quiz,
            commands::learning::rate_flash_card,
            // Lesson completion (Phase 3 — Wave 3, 03-05)
            commands::learning::mark_lesson_complete,
            commands::learning::get_lesson_completions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
