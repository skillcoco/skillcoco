//! Tauri IPC handler signatures for the topic_packs module.
//!
//! ## Wave 0 status
//!
//! All four handlers panic with `unimplemented!()` quoting Wave 2 (Plan 05-03).
//! Request structs carry `#[serde(rename_all = "camelCase")]` per the
//! project-wide convention.
//!
//! Wave 2 (Plan 05-03) will:
//! 1. Wire `AppState.topic_packs: Arc<Mutex<PackRegistry>>` (Wave 1 lands this).
//! 2. Register these four handlers in `lib.rs::run()` via `generate_handler!`.
//! 3. Implement the bodies using the registry + persistence layer.

use tauri::State;

use super::model::LoadedPack;
use crate::AppState;

#[tauri::command]
pub fn list_topic_packs(state: State<AppState>) -> Result<Vec<LoadedPack>, String> {
    let _ = state;
    unimplemented!("Wave 2 (Plan 05-03) implements list_topic_packs")
}

#[tauri::command]
pub fn list_topic_packs_admin(state: State<AppState>) -> Result<Vec<LoadedPack>, String> {
    let _ = state;
    unimplemented!("Wave 2 (Plan 05-03) implements list_topic_packs_admin")
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTopicPackEnabledRequest {
    pub pack_id: String,
    pub enabled: bool,
}

#[tauri::command]
pub fn set_topic_pack_enabled(
    state: State<AppState>,
    request: SetTopicPackEnabledRequest,
) -> Result<(), String> {
    let _ = (state, request);
    unimplemented!("Wave 2 (Plan 05-03) implements set_topic_pack_enabled")
}

#[tauri::command]
pub fn reload_skills(state: State<AppState>) -> Result<(), String> {
    let _ = state;
    unimplemented!("Wave 2 (Plan 05-03) implements reload_skills (skills-only — Q6 lock)")
}
