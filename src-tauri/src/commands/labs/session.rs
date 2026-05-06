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
#[path = "session_tests.rs"]
mod tests;
