mod ai;
mod commands;
mod db;
mod labs;
mod learning;

use db::Database;
use std::sync::Mutex;
use tauri::Manager;

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

            let db_path = app_dir.join("learnforge.db");
            let database = Database::new(&db_path).expect("Failed to initialize database");

            app.manage(AppState {
                db: Mutex::new(database),
            });

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
            // Profile commands
            commands::tracks::get_or_create_profile,
            commands::tracks::update_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
