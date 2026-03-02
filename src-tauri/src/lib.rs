mod ai;
mod auth;
mod commands;
mod db;
mod labs;
mod learning;
mod vector;

use auth::AuthState;
use db::Database;
use std::sync::Mutex;
use tauri::Manager;
use vector::VectorState;

pub struct AppState {
    pub db: Mutex<Database>,
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
                db: Mutex::new(database),
            });

            // Auth credential store
            let auth_dir = app_dir.join("auth");
            std::fs::create_dir_all(&auth_dir).expect("Failed to create auth dir");
            app.manage(AuthState::new(&auth_dir));

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
            // Learning commands
            commands::learning::get_path,
            commands::learning::get_module_progress,
            commands::learning::update_module_progress,
            commands::learning::get_due_cards,
            commands::learning::submit_review,
            // AI commands
            commands::ai::get_ai_config,
            commands::ai::update_ai_config,
            commands::ai::assess_knowledge,
            commands::ai::generate_learning_path,
            commands::ai::send_tutor_message,
            commands::ai::generate_module_content,
            // Auth commands
            auth::commands::get_auth_status,
            auth::commands::login_provider,
            auth::commands::set_active_provider,
            auth::commands::logout_provider,
            // Profile commands
            commands::tracks::get_or_create_profile,
            commands::tracks::update_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
