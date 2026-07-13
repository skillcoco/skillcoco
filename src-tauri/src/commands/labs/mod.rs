//! # commands::labs — Tauri IPC for hands-on labs (Phase 03.1)
//!
//! Per RESEARCH.md § Tauri IPC Contract. Every request / result struct uses
//! `#[serde(rename_all = "camelCase")]` (FIX-02 — non-negotiable). The
//! handlers compose `labs::{spec, evaluator, prompt_detect, host_shell,
//! docker, test_support}` behind nine `#[tauri::command]` entry points
//! registered in `lib.rs::run`'s `invoke_handler!`.
//!
//! Submodule layout (split per RESEARCH risk row — files <500 lines):
//!
//! - `session` — `lab_session_open`, `lab_session_close`, `lab_pty_write`,
//!   `lab_pty_resize`, `lab_runtime_detect` + the OSC 133 init script.
//! - `eval` — `lab_check_step`, `lab_show_hint` + AI-judge persistence.
//! - `state` — `lab_reset`, `lab_get_progress` + `recompute_practical_mastery`
//!   + `reset_surgical` + `reset_clears_progress`.

use crate::labs::spec::LabSpec;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub mod eval;
pub mod exam;
pub mod exam_entry;
pub mod session;
pub mod state;

// Re-exports — keep `commands::labs::lab_*` paths stable for `lib.rs`.
pub use eval::{lab_check_step, lab_show_hint};
pub use session::{
    lab_pty_resize, lab_pty_write, lab_runtime_detect, lab_session_close, lab_session_open,
    OSC_133_INIT_SCRIPT,
};
pub use state::{
    lab_get_progress, lab_reset, recompute_practical_mastery, reset_clears_progress,
    reset_surgical,
};

/// Phase 19 (19-03) — the single promoted read-lab-spec helper, taking a
/// bare `&Connection` so it's usable both from `State`-threaded IPC
/// handlers (via a short-lived `state.db.lock()`) AND directly from unit
/// tests without needing a `tauri::State<AppState>` (which cannot be
/// constructed outside the Tauri runtime). `session::read_lab_spec`
/// delegates to this for its production (State-based) callers so there is
/// still exactly ONE copy of the payload_json/params_json fallback logic
/// (PATTERNS map note — avoids a third copy alongside
/// `eval.rs::read_lab_spec_from_db`).
pub(crate) fn read_lab_spec_conn(
    conn: &Connection,
    block_id: &str,
) -> Result<(LabSpec, String), String> {
    let block = {
        use learnforge_core::blocks::BlockStore;
        crate::storage_impl::blocks::SqliteBlockStore(conn)
            .get_by_id(block_id)
            .map_err(|e| format!("get_block: {}", e))?
            .ok_or_else(|| format!("block not found: {}", block_id))?
    };

    // Try payload_json.spec first (PagePlanner-emitted). WR-01/T-19-02 —
    // a DB-stored spec is re-validated via `validate_spec` (its stated
    // purpose): a stored spec with out-of-range exam calibration,
    // duplicate step ids, or image+dockerfile both set must never reach
    // scoring. Validation failure is treated like a parse failure — fall
    // through to the params_json.labMd path (whose parser validates too).
    if !block.payload_json.trim().is_empty() && block.payload_json != "{}" {
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&block.payload_json) {
            if let Some(spec_val) = payload.get("spec") {
                if let Ok(spec) = serde_json::from_value::<LabSpec>(spec_val.clone()) {
                    match crate::labs::spec::validate_spec(&spec) {
                        Ok(()) => return Ok((spec, String::new())),
                        Err(e) => {
                            log::warn!(
                                "read_lab_spec_conn: stored spec for block {} failed validation: {}",
                                block_id,
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    // Fall back to params_json.labMd (raw markdown).
    if let Ok(params) = serde_json::from_str::<serde_json::Value>(&block.params_json) {
        if let Some(md) = params.get("labMd").and_then(|v| v.as_str()) {
            return crate::labs::spec::parse_lab_md(md).map_err(|e| format!("parse_lab_md: {}", e));
        }
    }

    Err(format!("block {} has no readable lab spec", block_id))
}

// ── IPC structs (all camelCase) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabSessionOpenRequest {
    pub block_id: String,
    pub track_id: String,
    pub module_id: String,
    pub learner_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabSessionOpenResult {
    pub session_id: String,
    pub effective_runtime: String, // "docker" | "host_shell"
    pub workspace_path: String,
    pub spec: LabSpec,
    pub progress: LabProgress,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabPtyWriteRequest {
    pub session_id: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabPtyResizeRequest {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabSessionCloseRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabCheckStepRequest {
    pub session_id: String,
    pub step_index: usize,
    pub last_command: String,
    pub last_output: String,
    pub last_exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabCheckStepResult {
    pub step_index: usize,
    pub passed: bool,
    pub reason: String,
    pub check_kind: String, // "commandRegex" | "exitCode" | "fileState" | "aiJudge"
    pub mastery_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabShowHintRequest {
    pub session_id: String,
    pub step_index: usize,
    pub current_tier: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabShowHintResult {
    pub tier: u8,
    pub text: String,
    pub final_tier: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabResetRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabResetResult {
    pub files_removed: Vec<String>,
    pub progress_reset: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabGetProgressRequest {
    pub block_id: String,
    pub learner_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabProgress {
    pub block_id: String,
    pub current_step: usize,
    pub completed_step_ids: Vec<String>,
    pub last_updated: String,
    pub practical_mastery: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabRuntimeDetectRequest {
    /// Optional: defaults to `"autoDetect"` when omitted.
    #[serde(default)]
    pub setting: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabRuntimeDetectResult {
    pub docker_available: bool,
    pub docker_version: Option<String>,
    pub effective_runtime: String, // "docker" | "host_shell"
    pub setting: String,           // "docker" | "hostShell" | "autoDetect"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_camel(json: &str, key: &str) {
        assert!(
            json.contains(&format!("\"{}\"", key)),
            "expected camelCase key {:?} in JSON, got {}",
            key,
            json
        );
    }

    /// LAB-02 — LabSessionOpenRequest serializes to camelCase.
    #[test]
    fn lab_session_open_request_camel_case() {
        let req = LabSessionOpenRequest {
            block_id: "blk-1".to_string(),
            track_id: "trk-1".to_string(),
            module_id: "mod-1".to_string(),
            learner_id: "lp-1".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        for k in ["blockId", "trackId", "moduleId", "learnerId"] {
            assert_camel(&json, k);
        }
    }

    /// LAB-02 — LabSessionOpenResult serializes to camelCase.
    #[test]
    fn lab_session_open_result_camel_case() {
        let result = LabSessionOpenResult {
            session_id: "sess-1".to_string(),
            effective_runtime: "docker".to_string(),
            workspace_path: "/tmp/ws".to_string(),
            spec: LabSpec {
                slug: "x".to_string(),
                title: "x".to_string(),
                image: Some("alpine".to_string()),
                dockerfile: None,
                requires_docker: true,
                creates: vec![],
                exam: None,
                steps: vec![],
            },
            progress: LabProgress {
                block_id: "blk-1".to_string(),
                current_step: 0,
                completed_step_ids: vec![],
                last_updated: "2026-05-05T00:00:00Z".to_string(),
                practical_mastery: 0.0,
            },
            warning: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        for k in [
            "sessionId",
            "effectiveRuntime",
            "workspacePath",
            "blockId",
            "currentStep",
            "completedStepIds",
            "lastUpdated",
            "practicalMastery",
            "requiresDocker",
        ] {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_pty_write_request_camel_case() {
        let req = LabPtyWriteRequest { session_id: "s".to_string(), data: vec![1, 2, 3] };
        let json = serde_json::to_string(&req).unwrap();
        for k in ["sessionId", "data"] {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_pty_resize_request_camel_case() {
        let req = LabPtyResizeRequest { session_id: "s".to_string(), cols: 80, rows: 24 };
        let json = serde_json::to_string(&req).unwrap();
        for k in ["sessionId", "cols", "rows"] {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_session_close_request_camel_case() {
        let req = LabSessionCloseRequest { session_id: "s".to_string() };
        let json = serde_json::to_string(&req).unwrap();
        assert_camel(&json, "sessionId");
    }

    #[test]
    fn lab_check_step_request_camel_case() {
        let req = LabCheckStepRequest {
            session_id: "s".to_string(),
            step_index: 0,
            last_command: "ls".to_string(),
            last_output: "foo".to_string(),
            last_exit_code: Some(0),
        };
        let json = serde_json::to_string(&req).unwrap();
        for k in ["sessionId", "stepIndex", "lastCommand", "lastOutput", "lastExitCode"] {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_check_step_result_camel_case() {
        let result = LabCheckStepResult {
            step_index: 0,
            passed: true,
            reason: "ok".to_string(),
            check_kind: "commandRegex".to_string(),
            mastery_delta: 0.25,
        };
        let json = serde_json::to_string(&result).unwrap();
        for k in ["stepIndex", "passed", "reason", "checkKind", "masteryDelta"] {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_show_hint_request_camel_case() {
        let req = LabShowHintRequest {
            session_id: "s".to_string(),
            step_index: 0,
            current_tier: 0,
        };
        let json = serde_json::to_string(&req).unwrap();
        for k in ["sessionId", "stepIndex", "currentTier"] {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_show_hint_result_camel_case() {
        let result =
            LabShowHintResult { tier: 1, text: "hint text".to_string(), final_tier: false };
        let json = serde_json::to_string(&result).unwrap();
        for k in ["tier", "text", "finalTier"] {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_reset_request_camel_case() {
        let req = LabResetRequest { session_id: "s".to_string() };
        let json = serde_json::to_string(&req).unwrap();
        assert_camel(&json, "sessionId");
    }

    #[test]
    fn lab_reset_result_camel_case() {
        let result = LabResetResult {
            files_removed: vec!["manifests/pod.yaml".to_string()],
            progress_reset: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        for k in ["filesRemoved", "progressReset"] {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_get_progress_request_camel_case() {
        let req =
            LabGetProgressRequest { block_id: "b".to_string(), learner_id: "l".to_string() };
        let json = serde_json::to_string(&req).unwrap();
        for k in ["blockId", "learnerId"] {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_progress_camel_case() {
        let p = LabProgress {
            block_id: "b".to_string(),
            current_step: 2,
            completed_step_ids: vec!["s1".to_string()],
            last_updated: "now".to_string(),
            practical_mastery: 0.5,
        };
        let json = serde_json::to_string(&p).unwrap();
        for k in ["blockId", "currentStep", "completedStepIds", "lastUpdated", "practicalMastery"]
        {
            assert_camel(&json, k);
        }
    }

    #[test]
    fn lab_runtime_detect_result_camel_case() {
        let r = LabRuntimeDetectResult {
            docker_available: true,
            docker_version: Some("24.0.7".to_string()),
            effective_runtime: "docker".to_string(),
            setting: "autoDetect".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        for k in ["dockerAvailable", "dockerVersion", "effectiveRuntime", "setting"] {
            assert_camel(&json, k);
        }
    }

    /// LAB-04 — round-trip a lab block's params_json (which holds the
    /// source LAB.md text and generation prompt for regen).
    #[test]
    fn lab_block_paramsjson_roundtrip() {
        let lab_md = include_str!("../../../tests/fixtures/labs/specs/valid-pod-create.lab.md");

        // params_json mirrors what 03.1-04 will store: the LAB.md source
        // plus the prompt that produced it (or "topic_pack" sentinel for
        // pack-supplied labs).
        let params = serde_json::json!({
            "labMd": lab_md,
            "generationSource": "topic_pack",
            "generationPrompt": null,
        })
        .to_string();

        // Round-trip: parse params, extract labMd, parse via parse_lab_md.
        let parsed: serde_json::Value = serde_json::from_str(&params).unwrap();
        let raw = parsed["labMd"].as_str().unwrap();
        let (spec, body) = crate::labs::spec::parse_lab_md(raw)
            .expect("parse_lab_md must succeed once 03.1-03 lands");
        assert_eq!(spec.slug, "pod-create-and-inspect");
        // Phase 19.2 (D-10) — fixture gained a 5th (command_absent) step.
        assert_eq!(spec.steps.len(), 5);
        assert!(!body.trim().is_empty(), "body must round-trip for paramsJson");
    }
}
