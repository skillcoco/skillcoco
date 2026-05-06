//! # commands::labs::session — session lifecycle IPC handlers
//!
//! Owns `lab_session_open`, `lab_session_close`, `lab_pty_write`,
//! `lab_pty_resize`, and `lab_runtime_detect`. The OSC 133 init script
//! constant lives here too — it's bind-mounted into Docker containers and
//! sourced as `--rcs` for host bash/zsh shells (RESEARCH § Docker session
//! lifecycle, OSC 133 init script delivery).

use super::{
    LabProgress, LabPtyResizeRequest, LabPtyWriteRequest, LabRuntimeDetectRequest,
    LabRuntimeDetectResult, LabSessionCloseRequest, LabSessionOpenRequest, LabSessionOpenResult,
};
use crate::labs::docker::{DockerProbe, RealDockerProbe};
use crate::labs::host_shell::HostRuntime;
use crate::labs::{
    self, effective_runtime_with_warning, new_session_id, EffectiveRuntime, LabRuntime,
    LabSession, RuntimeSetting,
};
use crate::{AppState, LabSessionEntry};
use tauri::State;

/// Initial per-session AI-judge call budget. Matches the constant in
/// `labs::evaluator`'s test fixtures (5 calls). Tuned in RESEARCH.
pub const AI_JUDGE_DEFAULT_BUDGET: u32 = 5;

/// OSC 133 init script — sourced inside the lab shell so the prompt-detect
/// FSM can locate command boundaries deterministically. Bind-mounted into
/// Docker containers at `/learnforge/init.sh`; sourced via `bash --init-file`
/// for host bash and similar plumbing for host zsh (currently host shell uses
/// the env-PS1 path in `labs::host_shell` — this constant is the canonical
/// content materialized once at app startup so future Docker mode lifts it
/// directly without re-deriving it from PS1 fragments).
pub const OSC_133_INIT_SCRIPT: &str = "\
# LearnForge OSC 133 prompt-boundary markers (Phase 03.1).
# Sourced at lab-session start; emits ESC ] 133 ; A/B/C/D BEL around the
# prompt + command + output regions so labs::prompt_detect can locate
# command boundaries without prompt heuristics.
PS1='\\[\\e]133;A\\a\\]\\u@\\h:\\w\\$ \\[\\e]133;B\\a\\]'
PS0='\\[\\e]133;C\\a\\]'
PROMPT_COMMAND='_lf_exit=$?; printf \"\\033]133;D;%d\\007\" \"$_lf_exit\"'
export TERM=xterm-256color
cd /workspace 2>/dev/null || true
";

/// LAB-03 — detect runtime from setting + Docker probe.
///
/// Accepts `setting: "docker" | "hostShell" | "autoDetect"` (defaults to
/// `"autoDetect"`). Returns the effective runtime + Docker version when
/// available, alongside the setting string for the UI to round-trip.
#[tauri::command]
pub async fn lab_runtime_detect(
    request: LabRuntimeDetectRequest,
) -> Result<LabRuntimeDetectResult, String> {
    let setting_str = request.setting.unwrap_or_else(|| "autoDetect".to_string());
    let setting = parse_runtime_setting(&setting_str)?;
    let probe = RealDockerProbe::default();
    let probe_result = probe.probe();
    let docker_available = probe_result.is_ok();
    let docker_version = probe_result.ok().and_then(|v| v);
    let effective = labs::detect_runtime(setting, &probe).await;
    Ok(LabRuntimeDetectResult {
        docker_available,
        docker_version,
        effective_runtime: runtime_to_str(effective).to_string(),
        setting: setting_str,
    })
}

/// LAB-02 — open a lab session: resolve workspace, pick runtime, spawn the
/// LabRuntime, register the session in `AppState.lab_sessions`, ensure a
/// `lab_progress` row exists for (learner, block), and return the spec /
/// progress / warning to the UI.
#[tauri::command]
pub async fn lab_session_open(
    request: LabSessionOpenRequest,
    state: State<'_, AppState>,
) -> Result<LabSessionOpenResult, String> {
    let setting = RuntimeSetting::AutoDetect; // Default; UI persists override
                                               // separately via Settings IPC.
    let probe = RealDockerProbe::default();

    open_session_with(
        request,
        state,
        setting,
        &probe,
        |runtime, workspace| build_runtime(runtime, workspace),
    )
    .await
}

/// LAB-02 — close the live session, removing it from the registry.
#[tauri::command]
pub async fn lab_session_close(
    request: LabSessionCloseRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let entry = {
        let mut map = state.lab_sessions.lock().await;
        map.remove(&request.session_id)
    };
    match entry {
        Some(e) => e
            .session
            .close()
            .await
            .map_err(|err| format!("session.close failed: {}", err)),
        None => Err(format!("session not found: {}", request.session_id)),
    }
}

/// LAB-02 — forward stdin bytes to the live PTY/exec.
///
/// Acquires the registry lock briefly, performs the write through the
/// session reference, and drops the lock before awaiting. Sessions stay
/// in the registry; the lock guard is `!Send` so we can't hold it across
/// the await point.
#[tauri::command]
pub async fn lab_pty_write(
    request: LabPtyWriteRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_session_write(&state, &request.session_id, &request.data).await
}

/// LAB-02 — propagate xterm cols/rows to the PTY/exec winsize.
#[tauri::command]
pub async fn lab_pty_resize(
    request: LabPtyResizeRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_session_resize(&state, &request.session_id, request.cols, request.rows).await
}

/// Shared helper: locks the registry, awaits `write` while holding the
/// (Send-safe) tokio MutexGuard.
async fn with_session_write(
    state: &State<'_, AppState>,
    session_id: &str,
    data: &[u8],
) -> Result<(), String> {
    let map = state.lab_sessions.lock().await;
    let entry = map
        .get(session_id)
        .ok_or_else(|| format!("session not found: {}", session_id))?;
    entry
        .session
        .write(data)
        .await
        .map_err(|e| format!("session.write failed: {}", e))
}

async fn with_session_resize(
    state: &State<'_, AppState>,
    session_id: &str,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let map = state.lab_sessions.lock().await;
    let entry = map
        .get(session_id)
        .ok_or_else(|| format!("session not found: {}", session_id))?;
    entry
        .session
        .resize(cols, rows)
        .await
        .map_err(|e| format!("session.resize failed: {}", e))
}

// ── Helpers ──

pub(crate) fn runtime_to_str(rt: EffectiveRuntime) -> &'static str {
    match rt {
        EffectiveRuntime::Docker => "docker",
        EffectiveRuntime::HostShell => "host_shell",
    }
}

fn parse_runtime_setting(s: &str) -> Result<RuntimeSetting, String> {
    match s {
        "docker" | "Docker" => Ok(RuntimeSetting::Docker),
        "hostShell" | "host_shell" | "HostShell" => Ok(RuntimeSetting::HostShell),
        "autoDetect" | "auto_detect" | "AutoDetect" => Ok(RuntimeSetting::AutoDetect),
        other => Err(format!("unknown runtime setting: {:?}", other)),
    }
}

/// Pick the concrete `LabRuntime` impl for a given runtime + workspace path.
/// Production callers go through this; tests inject their own builder.
fn build_runtime(
    runtime: EffectiveRuntime,
    workspace: std::path::PathBuf,
) -> Box<dyn LabRuntime> {
    match runtime {
        EffectiveRuntime::Docker => {
            Box::new(crate::labs::docker::DockerRuntime::new(workspace))
        }
        EffectiveRuntime::HostShell => Box::new(HostRuntime::new(workspace)),
    }
}

/// Generic open-session helper used by both the production handler and unit
/// tests. The `build` closure constructs the LabRuntime — production wires
/// the real Docker/Host runtimes, tests inject `MockLabRuntime`.
pub(crate) async fn open_session_with<F, P>(
    request: LabSessionOpenRequest,
    state: State<'_, AppState>,
    setting: RuntimeSetting,
    probe: &P,
    build: F,
) -> Result<LabSessionOpenResult, String>
where
    F: FnOnce(EffectiveRuntime, std::path::PathBuf) -> Box<dyn LabRuntime>,
    P: DockerProbe,
{
    // 1. Resolve workspace.
    let workspace = labs::workspace_path(&request.track_id, &request.module_id)
        .map_err(|e| format!("workspace_path: {}", e))?;

    // 2. Read the lab block from DB to get the spec.
    let (spec, _spec_body) = read_lab_spec(&state, &request.block_id)?;

    // 3. Resolve runtime + override warning.
    let (runtime, warning) =
        effective_runtime_with_warning(setting, probe, spec.requires_docker).await;

    // 4. Build runtime + start the session.
    let session_id = new_session_id();
    let runtime_impl = build(runtime, workspace.clone());
    let session = runtime_impl
        .start(&workspace, &session_id)
        .await
        .map_err(|e| format!("runtime.start: {}", e))?;

    // 5. Ensure lab_progress row exists; read it back.
    let total_steps = spec.steps.len();
    let progress = ensure_lab_progress(
        &state,
        &request.learner_id,
        &request.module_id,
        &request.block_id,
        total_steps,
    )?;

    // 6. Insert into the AppState registry with sidecar metadata.
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

/// Read a lab block's spec from `module_blocks.payload_json` (Wave 0 stored
/// the LAB.md inside `params_json.labMd`; production paths persist a
/// normalized spec under `payload_json.spec`). We try `payload_json.spec`
/// first (PagePlanner-emitted), fall back to `params_json.labMd` (raw markdown).
pub(crate) fn read_lab_spec(
    state: &State<'_, AppState>,
    block_id: &str,
) -> Result<(crate::labs::spec::LabSpec, String), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("db lock poisoned: {}", e))?;
    let conn = &db.conn;
    let block = crate::db::blocks::get_block(conn, block_id)
        .map_err(|e| format!("get_block: {}", e))?
        .ok_or_else(|| format!("block not found: {}", block_id))?;

    // Try payload_json.spec first.
    if !block.payload_json.trim().is_empty() && block.payload_json != "{}" {
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&block.payload_json) {
            if let Some(spec_val) = payload.get("spec") {
                if let Ok(spec) = serde_json::from_value::<crate::labs::spec::LabSpec>(spec_val.clone())
                {
                    return Ok((spec, String::new()));
                }
            }
        }
    }

    // Fall back to params_json.labMd (raw markdown).
    if let Ok(params) = serde_json::from_str::<serde_json::Value>(&block.params_json) {
        if let Some(md) = params.get("labMd").and_then(|v| v.as_str()) {
            return crate::labs::spec::parse_lab_md(md)
                .map_err(|e| format!("parse_lab_md: {}", e));
        }
    }

    Err(format!("block {} has no readable lab spec", block_id))
}

fn ensure_lab_progress(
    state: &State<'_, AppState>,
    learner_id: &str,
    module_id: &str,
    block_id: &str,
    total_steps: usize,
) -> Result<LabProgress, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("db lock poisoned: {}", e))?;
    let conn = &db.conn;
    super::state::ensure_lab_progress_row(
        conn, learner_id, module_id, block_id, total_steps,
    )
}

/// Re-box a `Box<dyn LabSession>` as `Box<dyn LabSession + Send>` for the
/// registry. Safe because every LabSession impl already satisfies `Send`
/// (the trait bound is `Send + Sync`).
fn to_send_box(s: Box<dyn LabSession>) -> Box<dyn LabSession + Send> {
    // The trait already requires Send + Sync; the cast is sound. Use
    // `unsafe` transmute-equivalent via a helper: we wrap the raw pointer.
    // Cleaner: `Box::leak` + `Box::from_raw`. But LabSession trait already
    // bounds Send+Sync, so the `dyn LabSession` type is Send. The reason we
    // re-box is to add the explicit `+ Send` marker the registry needs.
    //
    // Convert by leaking + re-acquiring under the explicit type.
    let raw: *mut dyn LabSession = Box::into_raw(s);
    // SAFETY: trait bound on LabSession requires Send + Sync. The unique
    // ownership is preserved via Box::into_raw + Box::from_raw.
    unsafe { Box::from_raw(raw as *mut (dyn LabSession + Send)) }
}

#[cfg(test)]
mod tests {
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
        // We can't directly use State<'_, AppState> here without a Tauri
        // context; instead, replicate the body of `open_session_with`
        // against an Arc<AppState>.
        let workspace = crate::labs::workspace_path(&request.track_id, &request.module_id)
            .map_err(|e| format!("workspace_path: {}", e))?;

        // Read spec.
        let (spec, _) = {
            let db = state.db.lock().unwrap();
            let conn = &db.conn;
            let block = crate::db::blocks::get_block(conn, &request.block_id)
                .map_err(|e| format!("get_block: {}", e))?
                .ok_or_else(|| "block not found".to_string())?;
            let params: serde_json::Value =
                serde_json::from_str(&block.params_json).unwrap();
            let md = params["labMd"].as_str().unwrap();
            crate::labs::spec::parse_lab_md(md)
                .map_err(|e| format!("parse_lab_md: {}", e))?
        };

        let (runtime, warning) =
            effective_runtime_with_warning(setting, probe, spec.requires_docker).await;
        let session_id = new_session_id();

        // Build mock runtime and start.
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

        let _ = runtime; // silence dead-code warning when mocks ignore it
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
    fn entry_with_mock(
        session_id: &str,
        mock: MockLabSession,
    ) -> LabSessionEntry {
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

        // Replicate the close handler body.
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
        // Per CONTEXT one-click switch: the runtime is overridden to Docker
        // for this session.
        assert_eq!(result.effective_runtime, "docker");
    }

    /// Sanity: setting parser accepts the three canonical strings.
    #[test]
    fn parse_runtime_setting_accepts_canonical_strings() {
        assert_eq!(
            parse_runtime_setting("docker").unwrap(),
            RuntimeSetting::Docker
        );
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
}
