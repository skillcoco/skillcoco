mod ai;
pub mod auth;
pub mod commands;
pub mod db;
pub mod labs;
pub mod learning;
pub mod licensing;
mod vector;

use auth::AuthState;
use db::Database;
use labs::LabSession;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use vector::VectorState;

/// Sidecar metadata carried alongside each live `LabSession` in the
/// `AppState.lab_sessions` registry. Lets `lab_reset` / `lab_pty_*` /
/// `lab_check_step` recover the (learner, module, block, workspace) tuple
/// from a session id without re-querying the DB.
pub struct LabSessionEntry {
    pub session: Box<dyn LabSession + Send>,
    pub block_id: String,
    pub learner_id: String,
    pub module_id: String,
    pub workspace: PathBuf,
    pub total_steps: usize,
    /// Per-session AI-judge budget (decremented on each LLM call). Initial
    /// value matches `labs::evaluator` default budget (5).
    pub ai_budget_remaining: u32,
}

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    /// Registry of live lab PTY/Docker sessions keyed by session UUID.
    /// Populated by `commands::labs::lab_session_open` and drained by
    /// `lab_session_close` / PTY exit. Each entry carries sidecar metadata
    /// so per-session IPC handlers don't need a fresh DB lookup.
    ///
    /// Uses `tokio::sync::Mutex` (not `std::sync::Mutex`) so the lock guard
    /// is `Send` and can be held across `await` points inside async IPC
    /// handlers (`lab_pty_write` / `lab_pty_resize` / `lab_check_step`).
    pub lab_sessions: Arc<tokio::sync::Mutex<HashMap<String, LabSessionEntry>>>,
}

/// Contract that Pro overlays satisfy to inject additional Tauri commands.
/// OSS provides `NoopPlugin` as the default (no-op); the OSS `run()`
/// function uses NoopPlugin so the trait surface is exercised in the
/// open-core build. Plan 05's Studio `main.rs` defines `StudioPlugin`
/// implementing this trait and appends Pro commands via the same path.
/// Mirrors the `LabRuntime` trait shape (`labs::mod`) but is sync — no
/// `Pin<Box<Future>>` needed.
///
/// Tauri 2.x's `invoke_handler` can be called exactly ONCE per builder
/// (RESEARCH.md Pitfall 1). `register_commands` MUST NOT call
/// `.invoke_handler` on the builder it receives — that call is the
/// caller's responsibility (`run()` in OSS, `main()` in Studio).
/// register_commands is for setup/plugin/state additions only.
pub trait LearnForgePlugin: Send + Sync {
    /// Stable identifier for the plugin (logging, diagnostics).
    fn plugin_name(&self) -> &'static str;

    /// Extend the builder with plugin-specific setup or state (plugins,
    /// .manage(), .setup hooks). MUST NOT call `.invoke_handler`. The
    /// caller calls `.invoke_handler` exactly once after this method
    /// returns, passing the full (OSS + plugin-contributed) command
    /// list via `tauri::generate_handler![…]`.
    fn register_commands(
        &self,
        builder: tauri::Builder<tauri::Wry>,
    ) -> tauri::Builder<tauri::Wry>;
}

/// No-op default implementation. Used by OSS `run()` so the trait
/// surface is exercised in pure-OSS builds; Plan 05 swaps it for
/// `StudioPlugin` in the Studio binary. Mirrors `vector::VectorState`
/// stub shape.
pub struct NoopPlugin;

impl LearnForgePlugin for NoopPlugin {
    fn plugin_name(&self) -> &'static str {
        "noop"
    }

    fn register_commands(
        &self,
        builder: tauri::Builder<tauri::Wry>,
    ) -> tauri::Builder<tauri::Wry> {
        // Identity: returns the builder unchanged. No setup hooks,
        // no .manage(), no plugins beyond what build_app() already
        // attached. The single .invoke_handler() call happens in the
        // caller (run() for OSS, main() for Studio).
        builder
    }
}

/// Build the Tauri app — plugins + setup ONLY. NO invoke_handler. NO .run().
///
/// Pro's `main.rs` calls this and then attaches its own `invoke_handler`
/// via `StudioPlugin::register_commands`; OSS does the same below via
/// `NoopPlugin`. The Tauri 2.x `invoke_handler`-can-be-called-only-once
/// constraint (RESEARCH.md Pitfall 1) lives in the caller (`run()` for
/// OSS, `main()` for Studio) — never here.
pub fn build_app() -> tauri::Builder<tauri::Wry> {
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
                lab_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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

            // Phase 03.1 — materialize the OSC 133 init script once at app
            // startup so Docker mode can bind-mount it into containers and
            // host shell mode can `--init-file` source it. Idempotent: only
            // writes when missing.
            let init_script_path = app_dir.join("labs").join("init.sh");
            if let Some(parent) = init_script_path.parent() {
                std::fs::create_dir_all(parent)
                    .expect("Failed to create labs/ dir under app data dir");
            }
            if !init_script_path.exists() {
                std::fs::write(
                    &init_script_path,
                    commands::labs::OSC_133_INIT_SCRIPT,
                )
                .expect("Failed to write OSC 133 init script");
            }

            log::info!("LearnForge initialized with DB at {:?}", db_path);
            Ok(())
        })
}

pub fn run() {
    env_logger::init();
    // OSS exercises the open-core seam via NoopPlugin so the trait
    // surface is tested in every OSS build. Plan 05's Studio main.rs
    // does the same with StudioPlugin (which contributes Pro
    // commands). The single .invoke_handler call below is the only
    // one Tauri 2.x allows per builder (RESEARCH.md Pitfall 1).
    let plugin = NoopPlugin;
    plugin
        .register_commands(build_app())
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
            // Lab commands (Phase 03.1 — Wave 2b, 03.1-05)
            commands::labs::session::lab_session_open,
            commands::labs::session::lab_session_close,
            commands::labs::session::lab_pty_write,
            commands::labs::session::lab_pty_resize,
            commands::labs::eval::lab_check_step,
            commands::labs::eval::lab_show_hint,
            commands::labs::state::lab_reset,
            commands::labs::state::lab_get_progress,
            commands::labs::session::lab_runtime_detect,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod build_app_tests {
    use super::*;

    #[test]
    fn build_app_returns_builder() {
        // Compiling this call proves build_app() exists with the right signature.
        let _b: tauri::Builder<tauri::Wry> = build_app();
    }

    #[test]
    fn plugin_trait_minimal_contract() {
        let p = NoopPlugin;
        assert_eq!(p.plugin_name(), "noop");
        // register_commands borrows &self and returns a builder; we
        // construct one via build_app() to feed in.
        let _b: tauri::Builder<tauri::Wry> = p.register_commands(build_app());
    }

    #[test]
    fn noop_plugin_is_object_safe() {
        // Object-safety check — must be possible to store plugins as Box<dyn>.
        let plugins: Vec<Box<dyn LearnForgePlugin>> = vec![Box::new(NoopPlugin)];
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].plugin_name(), "noop");
    }
}
