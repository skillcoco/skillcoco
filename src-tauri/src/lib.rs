pub mod achievements;
mod ai;
pub mod auth;
pub mod commands;
pub mod db;
pub mod labs;
// Phase 18 Plan 4 — shared PDF text-rendering helper (`push_line`) used by
// the certificate renderer (achievements::artifacts). Extracted so the
// printpdf Td-relative fix (and its regression test) lives in one place.
pub mod pdf_util;
// Phase 7 Wave 2 (Plan 07-02) — rusqlite-backed impls of skillcoco_core
// per-module storage traits. The trait `impl BktStore for &Connection`
// in storage_impl::bkt is a coherence requirement to live in src-tauri
// (the trait is defined in skillcoco_core; the impl carries rusqlite).
pub mod storage_impl;
pub mod topic_packs;
mod vector;

use auth::AuthState;
use db::Database;
use labs::LabSession;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use vector::VectorState;

/// Phase 19.3 (D-01) — one recorded command at the eval IPC boundary.
/// `lab_check_step` appends one of these to the owning session's
/// `command_history` on EVERY call (all grains), so milestone-grain
/// validation (`lab_validate_milestone`) can evaluate against the
/// cumulative session history instead of only the last command.
///
/// In-memory only: history lives on `LabSessionEntry` and does NOT persist
/// across app restarts (documented accepted scope for this phase).
#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub command: String,
    pub output: String,
    pub exit_code: Option<i32>,
}

/// Phase 19.3 (D-01) — hard cap on records per session history.
pub const LAB_HISTORY_MAX_RECORDS: usize = 200;
/// Phase 19.3 (D-01) — hard cap on cumulative `command` + `output` bytes
/// per session history (1 MiB, à la `MAX_PACK_BYTES` self-imposed-cap
/// precedent). 19.3-REVIEW CR-03: BOTH fields count toward this budget —
/// counting only output left `command` unbounded (200 records x unbounded
/// renderer-supplied command bytes defeats the T-19.3-01 memory bound).
pub const LAB_HISTORY_MAX_BYTES: usize = 1024 * 1024;
/// 19.3-REVIEW CR-03 — per-record cap on `command` bytes (4 KiB). A real
/// interactive command line is far smaller; a multi-megabyte paste or a
/// hostile renderer gets truncated before insertion.
pub const LAB_HISTORY_MAX_COMMAND_BYTES: usize = 4 * 1024;
/// 19.3-REVIEW CR-03 — per-record cap on `output` bytes (256 KiB). Bounds
/// any single record so no one command can consume the whole session
/// budget (previously a single output could reach the full 1 MiB).
pub const LAB_HISTORY_MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// Truncate `s` to at most `max` bytes at a char boundary (UTF-8 safe).
fn truncate_at_char_boundary(s: &mut String, max: usize) {
    if s.len() > max {
        let mut cut = max;
        while cut > 0 && !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
}

/// Bytes a record contributes to the session budget (CR-03: command AND
/// output).
fn record_bytes(r: &CommandRecord) -> usize {
    r.command.len() + r.output.len()
}

/// Phase 19.3 (D-01) — append `rec` then evict OLDEST records first until
/// BOTH `len <= LAB_HISTORY_MAX_RECORDS` AND cumulative `command + output`
/// bytes `<= LAB_HISTORY_MAX_BYTES`.
///
/// Per-record policy (CR-03): `command` is truncated to
/// `LAB_HISTORY_MAX_COMMAND_BYTES` and `output` to
/// `LAB_HISTORY_MAX_OUTPUT_BYTES` (both at char boundaries) before
/// insertion — len/bytes are all bounded unconditionally.
///
/// Accepted limitation (D-01, documented): eviction can UNDER-REPORT
/// `command_absent` at milestone grain — a record matching the forbidden
/// pattern that was evicted before Validate is pressed will not fail the
/// check. Learner-local, no privilege exposure; the caps exist to bound
/// the higher-severity DoS (T-19.3-01).
pub fn push_command_record(history: &mut Vec<CommandRecord>, mut rec: CommandRecord) {
    truncate_at_char_boundary(&mut rec.command, LAB_HISTORY_MAX_COMMAND_BYTES);
    truncate_at_char_boundary(&mut rec.output, LAB_HISTORY_MAX_OUTPUT_BYTES);
    history.push(rec);
    while history.len() > LAB_HISTORY_MAX_RECORDS {
        history.remove(0);
    }
    let mut total: usize = history.iter().map(record_bytes).sum();
    while total > LAB_HISTORY_MAX_BYTES && history.len() > 1 {
        let evicted = history.remove(0);
        total -= record_bytes(&evicted);
    }
}

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
    /// Phase 19.3 (D-01) — per-session command history appended by
    /// `lab_check_step` on every call; bounded by `push_command_record`
    /// (200 records / 1 MiB, oldest evicted first). In-memory only — not
    /// persisted across app restarts.
    pub command_history: Vec<CommandRecord>,
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
    /// Phase 5 (Topic Packs) — in-memory registry of bundled + skill packs,
    /// populated by `topic_packs::loader::load_all` inside `build_app`'s
    /// setup hook AFTER `Database::new` (so the v008 table exists) and
    /// BEFORE `.invoke_handler` binds (Pitfall 7: pack loading is small-N
    /// file I/O and IPC handlers must see a populated registry on first
    /// call). Uses `std::sync::Mutex` because pack-reads in IPC handlers
    /// don't await.
    pub topic_packs: Arc<Mutex<skillcoco_core::packs::PackRegistry>>,
    /// Phase 6 (Certification) — lazy-loaded Ed25519 signing key for cert
    /// issuance. `None` until the first `achievements::maybe_issue` call,
    /// which loads-or-generates via `signing::get_or_init_key`. Avoids
    /// disk I/O on cold start; the BKT path triggers init only when a
    /// learner actually crosses a threshold.
    pub signing_key: Arc<Mutex<Option<ed25519_dalek::SigningKey>>>,
    /// Phase 6 — `<app_data>/keys/` directory where the signing private +
    /// public PEMs live. Wave 1 reads/writes this path; Wave 5 Settings
    /// "Show signing public key" will expose `cert_signing_public.pem`.
    pub signing_key_path: PathBuf,
}

/// Backend-only community-plugin seam. `NoopPlugin` is the default
/// implementation for the OSS binary; the `run()` function exercises this
/// trait surface in every OSS build. External community plugins may
/// implement this trait to contribute additional backend state or setup
/// without modifying the core binary.
///
/// Mirrors the `LabRuntime` trait shape (`labs::mod`) but is sync — no
/// `Pin<Box<Future>>` needed.
///
/// Tauri 2.x's `invoke_handler` can be called exactly ONCE per builder
/// (RESEARCH.md Pitfall 1). `register_commands` MUST NOT call
/// `.invoke_handler` on the builder it receives — that call is the
/// caller's responsibility in `run()`.
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

/// No-op default implementation used by OSS `run()` so the trait
/// surface is exercised in every OSS build. Mirrors `vector::VectorState`
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
        // caller (run() for OSS builds).
        builder
    }
}

/// Build the Tauri app — plugins + setup ONLY. NO invoke_handler. NO .run().
///
/// The OSS `run()` function calls this and then attaches its own
/// `invoke_handler` via `NoopPlugin`. Community-plugin binaries follow
/// the same pattern. The Tauri 2.x `invoke_handler`-can-be-called-only-once
/// constraint (RESEARCH.md Pitfall 1) lives in the caller — never here.
pub fn build_app() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // Phase 6 (Plan 06-01 / locked answer A7) — dialog + fs plugins
        // back the Wave 2 "Export certificate" + "Save badge as..." flows.
        // Wired in Wave 0 so the Wave 2 frontend save-as code has a
        // ready-to-call API surface.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data dir");

            // Database
            let db_path = app_dir.join("learnforge.db");
            let database = Database::new(&db_path).expect("Failed to initialize database");

            // Phase 5 — load topic packs (bundled + skills) AFTER migrations
            // have run inside Database::new, BEFORE .invoke_handler binds
            // (Pitfall 7 from RESEARCH.md). Synchronous on purpose — pack
            // files are small (~10ms total); spawning would create a window
            // where IPC handlers see an empty registry.
            let topic_packs_registry = topic_packs::loader::load_all(&database.conn)
                .expect("Failed to load topic packs at startup — see logs");

            // Phase 6 (Certification) — keys directory + lazy signing key.
            // Pattern 2 from 06-RESEARCH.md: AppState holds an Arc<Mutex<Option<SigningKey>>>
            // that the first `maybe_issue` call populates by reading or generating
            // `<app_data>/keys/cert_signing_{public,private}.pem`. We do NOT
            // pre-generate the key at startup — keeping cold-start free of disk I/O.
            let signing_key_path = app_dir.join("keys");

            app.manage(AppState {
                db: Arc::new(Mutex::new(database)),
                lab_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                topic_packs: Arc::new(Mutex::new(topic_packs_registry)),
                signing_key: Arc::new(Mutex::new(None)),
                signing_key_path,
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
    // OSS exercises the community-plugin seam via NoopPlugin so the
    // trait surface is tested in every build. The single
    // .invoke_handler call below is the only one Tauri 2.x allows per
    // builder (RESEARCH.md Pitfall 1).
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
            commands::tracks::set_track_browse_mode,
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
            auth::commands::check_ollama_connection,
            auth::commands::is_youtube_key_configured,
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
            // Milestone validation grain (Phase 19.3 — D-04)
            commands::labs::eval::lab_validate_milestone,
            commands::labs::state::lab_reset,
            commands::labs::state::lab_get_progress,
            commands::labs::session::lab_runtime_detect,
            // Microlearning (Phase 4 — daily challenge IPCs)
            commands::microlearning::get_daily_challenge,
            commands::microlearning::start_daily_challenge,
            commands::microlearning::complete_daily_challenge,
            commands::microlearning::is_daily_challenge_enabled,
            commands::microlearning::set_daily_challenge_enabled,
            // Topic Packs (Phase 5 — Wave 2)
            topic_packs::commands::list_topic_packs,
            topic_packs::commands::list_topic_packs_admin,
            topic_packs::commands::set_topic_pack_enabled,
            topic_packs::commands::reload_skills,
            topic_packs::commands::get_topic_pack_modules,
            // Certification (Phase 6 — Wave 2). The completion badge is
            // self-signed; the cert-verify surface was removed in the Phase 23
            // trust-chain strip.
            commands::achievements::list_achievements_for_learner,
            commands::achievements::get_track_certifications,
            commands::achievements::export_certificate,
            commands::achievements::export_badge,
            // Video-enriched lessons (Phase 11 — Plan 02)
            commands::videos::get_lesson_videos,
            commands::videos::refresh_lesson_videos,
            // Course import/export (Phase 12 — Plans 02 & 03)
            commands::course_io::export_course,
            commands::course_io::import_course,
            // Bundled starter packs (Phase 16 — Plan 01, LIB-04/LIB-02, D-12/D-13)
            commands::course_io::list_starter_packs,
            commands::course_io::start_starter_pack,
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

    /// Compile-gate for the Phase 4 microlearning IPC entries. The test body
    /// is empty on purpose — the value is in the four `commands::microlearning::*`
    /// references inside `tauri::generate_handler!`. If any handler signature
    /// drifts (e.g., loses `tauri::State`, returns the wrong Result), the
    /// macro expansion fails and the crate refuses to compile. This test
    /// makes that compile-time check explicit at the file-test level.
    #[test]
    fn run_compiles_with_microlearning() {
        // No assertions — just ensures the test binary linked, which means
        // `tauri::generate_handler!` accepted all four microlearning entries.
    }

    /// Phase 5 (Plan 05-02 Task 3) — compile-time gate proving `AppState`
    /// has the `topic_packs` field with the expected type. Pattern-matches
    /// on every field so a future deletion or rename breaks this test.
    ///
    /// Phase 6 (Plan 06-02 Task 3) — extended to require `signing_key` +
    /// `signing_key_path` so a future deletion would break compilation.
    #[test]
    fn appstate_has_topic_packs_field() {
        fn _type_check(s: AppState) {
            let AppState {
                db: _,
                lab_sessions: _,
                topic_packs,
                signing_key,
                signing_key_path,
            } = s;
            let _typed: Arc<Mutex<skillcoco_core::packs::PackRegistry>> = topic_packs;
            let _key: Arc<Mutex<Option<ed25519_dalek::SigningKey>>> = signing_key;
            let _path: PathBuf = signing_key_path;
        }
        let _: fn(AppState) = _type_check;
    }
}

// Phase 19.3 (D-01) — CommandRecord history tests live in a sibling file
// to keep lib.rs under the 500-line CLAUDE.md cap (same convention as
// labs::spec / commands::labs::eval).
#[cfg(test)]
#[path = "command_history_tests.rs"]
mod command_history_tests;
