//! Tests for `commands::labs::session`. Lives in a sibling file to keep
//! `session.rs` under the 500-line CLAUDE.md cap. Included via
//! `#[path = "session_tests.rs"] #[cfg(test)] mod tests;` from `session.rs`.

use super::*;
use crate::labs::docker::MockDockerProbe;
use crate::labs::test_support::{MockLabRuntime, MockLabSession};
use crate::AppState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Build a synthetic AppState with an in-memory DB, the v006 schema
/// applied, and an empty lab_sessions registry.
fn test_app_state() -> Arc<AppState> {
    let conn = rusqlite::Connection::open_in_memory().expect("open_in_memory");
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(crate::db::schema::CREATE_TABLES)
        .expect("baseline tables");
    crate::db::migrations::apply_migrations(&conn)
        .expect("apply migrations through v006");
    let db = crate::db::Database { conn };
    Arc::new(AppState {
        db: Arc::new(Mutex::new(db)),
        lab_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    })
}

/// Insert a learner, path, module + a lab block with the given spec
/// markdown into the DB. Returns (learner_id, module_id, block_id).
fn insert_lab_fixture(state: &AppState, lab_md: &str) -> (String, String, String) {
    let db = state.db.lock().unwrap();
    let conn = &db.conn;
    let learner = "lp-1".to_string();
    let track = "track-fixt-1".to_string();
    let path = "path-1".to_string();
    let module = "mod-1".to_string();
    let block = "blk-1".to_string();
    conn.execute(
        "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'L')",
        rusqlite::params![learner],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_tracks (id, learner_id, topic, domain_module)
         VALUES (?1, ?2, 'k8s', 'kubernetes')",
        rusqlite::params![track, learner],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
        rusqlite::params![path, track],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO modules (id, path_id, title, ordering)
         VALUES (?1, ?2, 'M1', 0)",
        rusqlite::params![module, path],
    )
    .unwrap();
    let params_json = serde_json::json!({
        "labMd": lab_md,
        "generationSource": "topic_pack",
        "generationPrompt": null,
    })
    .to_string();
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
            params_json, payload_json, source_anchors_json, metadata_json, retry_count,
            created_at, updated_at)
         VALUES (?1, ?2, 0, 'lab', 'ready', ?3, '{}', '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        rusqlite::params![block, module, params_json],
    )
    .unwrap();
    (learner, module, block)
}

/// Custom open helper that builds a `MockLabRuntime` and runs through
/// the same flow as the production handler. Used by every session-test
/// below to bypass real Docker/PTY without touching the production
/// signature.
async fn open_session_for_test(
    state: Arc<AppState>,
    request: LabSessionOpenRequest,
    setting: RuntimeSetting,
    probe: &dyn DockerProbe,
    mock_session: MockLabSession,
) -> Result<LabSessionOpenResult, String> {
    let workspace = crate::labs::workspace_path(&request.track_id, &request.module_id)
        .map_err(|e| format!("workspace_path: {}", e))?;

    // Read spec.
    let (spec, _) = {
        let db = state.db.lock().unwrap();
        let conn = &db.conn;
        let block = crate::db::blocks::get_block(conn, &request.block_id)
            .map_err(|e| format!("get_block: {}", e))?
            .ok_or_else(|| "block not found".to_string())?;
        let params: serde_json::Value = serde_json::from_str(&block.params_json).unwrap();
        let md = params["labMd"].as_str().unwrap();
        crate::labs::spec::parse_lab_md(md).map_err(|e| format!("parse_lab_md: {}", e))?
    };

    let (runtime, warning) =
        effective_runtime_with_warning(setting, probe, spec.requires_docker).await;
    let session_id = new_session_id();

    let runtime_impl = MockLabRuntime::new().with_session(mock_session);
    let session = runtime_impl
        .start(&workspace, &session_id)
        .await
        .map_err(|e| format!("runtime.start: {}", e))?;

    let total_steps = spec.steps.len();
    let progress = {
        let db = state.db.lock().unwrap();
        let conn = &db.conn;
        super::super::state::ensure_lab_progress_row(
            conn,
            &request.learner_id,
            &request.module_id,
            &request.block_id,
            total_steps,
        )?
    };

    {
        let mut map = state.lab_sessions.lock().await;
        let entry = LabSessionEntry {
            session: to_send_box(session),
            block_id: request.block_id.clone(),
            learner_id: request.learner_id.clone(),
            module_id: request.module_id.clone(),
            workspace: workspace.clone(),
            total_steps,
            ai_budget_remaining: AI_JUDGE_DEFAULT_BUDGET,
        };
        map.insert(session_id.clone(), entry);
    }

    Ok(LabSessionOpenResult {
        session_id,
        effective_runtime: runtime_to_str(runtime).to_string(),
        workspace_path: workspace.to_string_lossy().to_string(),
        spec,
        progress,
        warning,
    })
}

const VALID_LAB_MD: &str =
    include_str!("../../../tests/fixtures/labs/specs/valid-pod-create.lab.md");

/// LAB-02 — opening a lab inserts the session into the AppState
/// registry under its session id.
#[tokio::test]
async fn lab_session_open_inserts_session_into_registry() {
    let state = test_app_state();
    let (learner, module, block) = insert_lab_fixture(&state, VALID_LAB_MD);
    let probe = MockDockerProbe::new(true);
    let result = open_session_for_test(
        state.clone(),
        LabSessionOpenRequest {
            block_id: block,
            track_id: "track-1".to_string(),
            module_id: module,
            learner_id: learner,
        },
        RuntimeSetting::AutoDetect,
        &probe,
        MockLabSession::default(),
    )
    .await
    .expect("open must succeed");

    let map = state.lab_sessions.lock().await;
    assert!(
        map.contains_key(&result.session_id),
        "session must be inserted into registry"
    );
}

/// Build a `LabSessionEntry` carrying a MockLabSession plus throwaway
/// metadata so registry-backed tests don't repeat the boilerplate.
fn entry_with_mock(session_id: &str, mock: MockLabSession) -> LabSessionEntry {
    let session: Box<dyn LabSession> = Box::new(mock);
    LabSessionEntry {
        session: to_send_box(session),
        block_id: format!("blk-{}", session_id),
        learner_id: "lp-1".to_string(),
        module_id: "mod-1".to_string(),
        workspace: std::path::PathBuf::from("/tmp"),
        total_steps: 0,
        ai_budget_remaining: AI_JUDGE_DEFAULT_BUDGET,
    }
}

/// LAB-02 — closing a lab removes the session from the registry and
/// invokes session.close().
#[tokio::test]
async fn lab_session_close_removes_from_registry() {
    let state = test_app_state();
    let session_id = "test-close-1".to_string();
    {
        let mut map = state.lab_sessions.lock().await;
        map.insert(
            session_id.clone(),
            entry_with_mock(&session_id, MockLabSession::new(&session_id)),
        );
    }

    let removed = {
        let mut map = state.lab_sessions.lock().await;
        map.remove(&session_id)
    };
    let removed = removed.expect("session must be present");
    removed.session.close().await.expect("close must succeed");

    let map = state.lab_sessions.lock().await;
    assert!(!map.contains_key(&session_id), "registry must be empty after close");
}

/// LAB-02 — bytes written via the IPC handler reach the live session.
#[tokio::test]
async fn lab_pty_write_round_trip() {
    let state = test_app_state();
    let session_id = "test-write-1".to_string();
    let mock = MockLabSession::new(&session_id);
    let writes_handle = mock.writes_arc();
    {
        let mut map = state.lab_sessions.lock().await;
        map.insert(session_id.clone(), entry_with_mock(&session_id, mock));
    }

    {
        let map = state.lab_sessions.lock().await;
        let entry = map.get(&session_id).unwrap();
        entry
            .session
            .write(&[0x68, 0x65, 0x6C, 0x6C, 0x6F])
            .await
            .expect("write must succeed");
    }

    let writes = writes_handle.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0], vec![0x68, 0x65, 0x6C, 0x6C, 0x6F]);
}

/// LAB-02 — resize bytes are forwarded to the live session and recorded
/// (mock proxy for the user-observable winsize change).
#[tokio::test]
async fn lab_pty_resize_round_trip() {
    let state = test_app_state();
    let session_id = "test-resize-1".to_string();
    let mock = MockLabSession::new(&session_id);
    let resizes_handle = mock.resizes_arc();
    {
        let mut map = state.lab_sessions.lock().await;
        map.insert(session_id.clone(), entry_with_mock(&session_id, mock));
    }

    {
        let map = state.lab_sessions.lock().await;
        let entry = map.get(&session_id).unwrap();
        entry.session.resize(120, 40).await.expect("resize must succeed");
    }

    let resizes = resizes_handle.lock().unwrap();
    assert_eq!(resizes.len(), 1);
    assert_eq!(resizes[0], (120, 40));
}

/// LAB-03 — runtime detect with a Docker-available probe returns
/// effective_runtime=docker.
#[tokio::test]
async fn lab_runtime_detect_returns_docker_when_probe_ok() {
    let probe = MockDockerProbe::new(true);
    let setting = RuntimeSetting::AutoDetect;
    let runtime = labs::detect_runtime(setting, &probe).await;
    assert_eq!(runtime, EffectiveRuntime::Docker);
    assert_eq!(runtime_to_str(runtime), "docker");
}

/// LAB-03 — runtime detect with an unavailable Docker probe falls back
/// to host_shell.
#[tokio::test]
async fn lab_runtime_detect_returns_host_shell_when_probe_err() {
    let probe = MockDockerProbe::new(false);
    let setting = RuntimeSetting::AutoDetect;
    let runtime = labs::detect_runtime(setting, &probe).await;
    assert_eq!(runtime, EffectiveRuntime::HostShell);
    assert_eq!(runtime_to_str(runtime), "host_shell");
}

/// LAB-03 — opening a docker-required lab while Settings is HostShell
/// surfaces the override warning.
#[tokio::test]
async fn lab_session_open_warning_when_requires_docker_but_setting_host() {
    let state = test_app_state();
    let (learner, module, block) = insert_lab_fixture(&state, VALID_LAB_MD);
    let probe = MockDockerProbe::new(false);
    let result = open_session_for_test(
        state.clone(),
        LabSessionOpenRequest {
            block_id: block,
            track_id: "track-1".to_string(),
            module_id: module,
            learner_id: learner,
        },
        RuntimeSetting::HostShell,
        &probe,
        MockLabSession::default(),
    )
    .await
    .expect("open must succeed");
    assert!(
        result.warning.is_some(),
        "host-only learner + requires_docker spec must surface a warning"
    );
    assert_eq!(result.effective_runtime, "docker");
}

/// Sanity: setting parser accepts the three canonical strings.
#[test]
fn parse_runtime_setting_accepts_canonical_strings() {
    assert_eq!(parse_runtime_setting("docker").unwrap(), RuntimeSetting::Docker);
    assert_eq!(
        parse_runtime_setting("hostShell").unwrap(),
        RuntimeSetting::HostShell
    );
    assert_eq!(
        parse_runtime_setting("autoDetect").unwrap(),
        RuntimeSetting::AutoDetect
    );
    assert!(parse_runtime_setting("nope").is_err());
}

/// OSC 133 init script carries the four expected sequences.
#[test]
fn osc_133_init_script_has_canonical_markers() {
    let s = OSC_133_INIT_SCRIPT;
    assert!(s.contains("133;A"), "missing PromptStart marker");
    assert!(s.contains("133;B"), "missing CommandStart marker");
    assert!(s.contains("133;C"), "missing OutputStart marker");
    assert!(s.contains("133;D"), "missing CommandEnd marker");
    assert!(s.contains("PROMPT_COMMAND"), "missing PROMPT_COMMAND export");
    assert!(s.contains("xterm-256color"), "missing TERM export");
}
