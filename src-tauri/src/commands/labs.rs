//! # commands::labs — Tauri IPC for hands-on labs (Phase 03.1, Wave 0 stub)
//!
//! Per RESEARCH.md § Tauri IPC Contract. Every request / result struct uses
//! `#[serde(rename_all = "camelCase")]` (FIX-02 — non-negotiable). Wave 0
//! lays out the complete struct surface so 03.1-05 IPC handler plan can
//! turn each `#[tauri::command]` body green without touching the wire
//! types.

use crate::labs::spec::LabSpec;
use serde::{Deserialize, Serialize};

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
pub struct LabRuntimeDetectResult {
    pub docker_available: bool,
    pub docker_version: Option<String>,
    pub effective_runtime: String, // "docker" | "host_shell"
    pub setting: String,           // "docker" | "hostShell" | "autoDetect"
}

// ── Stub command handlers (03.1-05 wires the bodies) ──

#[tauri::command]
pub async fn lab_session_open(
    _request: LabSessionOpenRequest,
) -> Result<LabSessionOpenResult, String> {
    Err("lab_session_open: implemented in 03.1-05".to_string())
}

#[tauri::command]
pub async fn lab_session_close(_request: LabSessionCloseRequest) -> Result<(), String> {
    Err("lab_session_close: implemented in 03.1-05".to_string())
}

#[tauri::command]
pub async fn lab_pty_write(_request: LabPtyWriteRequest) -> Result<(), String> {
    Err("lab_pty_write: implemented in 03.1-05".to_string())
}

#[tauri::command]
pub async fn lab_pty_resize(_request: LabPtyResizeRequest) -> Result<(), String> {
    Err("lab_pty_resize: implemented in 03.1-05".to_string())
}

#[tauri::command]
pub async fn lab_check_step(_request: LabCheckStepRequest) -> Result<LabCheckStepResult, String> {
    Err("lab_check_step: implemented in 03.1-05".to_string())
}

#[tauri::command]
pub async fn lab_show_hint(_request: LabShowHintRequest) -> Result<LabShowHintResult, String> {
    Err("lab_show_hint: implemented in 03.1-05".to_string())
}

#[tauri::command]
pub async fn lab_reset(_request: LabResetRequest) -> Result<LabResetResult, String> {
    Err("lab_reset: implemented in 03.1-05".to_string())
}

#[tauri::command]
pub async fn lab_get_progress(_request: LabGetProgressRequest) -> Result<LabProgress, String> {
    Err("lab_get_progress: implemented in 03.1-05".to_string())
}

#[tauri::command]
pub async fn lab_runtime_detect() -> Result<LabRuntimeDetectResult, String> {
    Err("lab_runtime_detect: implemented in 03.1-05".to_string())
}

// ── Helpers (Wave 0 stubs) ──

/// LAB-07 — surgical reset: remove only files in `creates: []` from the
/// workspace. Wave 1 (03.1-05) implements; Wave 0 stub returns Err.
pub fn reset_surgical(
    _workspace: &std::path::Path,
    _creates: &[String],
) -> Result<Vec<String>, String> {
    Err("reset_surgical: implemented in 03.1-05".to_string())
}

/// LAB-07 — clears completed_step_ids + current_step in lab_progress for
/// the given (block_id, learner_id). Wave 1 (03.1-05) implements.
pub fn reset_clears_progress(
    _conn: &rusqlite::Connection,
    _block_id: &str,
    _learner_id: &str,
) -> Result<(), String> {
    Err("reset_clears_progress: implemented in 03.1-05".to_string())
}

/// LAB-08 — recompute module_progress.practical_mastery from the labs
/// in the module. Linear: sum(completed_steps) / sum(total_steps) across
/// all labs in the module. Wave 1 (03.1-02 / 03.1-05) implements.
pub fn recompute_practical_mastery(
    _conn: &rusqlite::Connection,
    _module_id: &str,
    _learner_id: &str,
) -> Result<f64, String> {
    Err("recompute_practical_mastery: implemented in 03.1-02".to_string())
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
        assert_camel(&json, "blockId");
        assert_camel(&json, "trackId");
        assert_camel(&json, "moduleId");
        assert_camel(&json, "learnerId");
    }

    /// LAB-02 — LabSessionOpenResult serializes to camelCase.
    /// FAILS Wave 0 because `LabSpec` has Default-like skeleton and the
    /// real wave-1 round-trip asserts on `effectiveRuntime`, `workspacePath`,
    /// `practicalMastery` (under progress), `lastUpdated`, etc.
    #[test]
    fn lab_session_open_result_camel_case() {
        // Build a synthetic result with a minimal spec — round-trip tests
        // the IPC wire shape, not the spec content.
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
        assert_camel(&json, "sessionId");
        assert_camel(&json, "effectiveRuntime");
        assert_camel(&json, "workspacePath");
        // Nested progress
        assert_camel(&json, "blockId");
        assert_camel(&json, "currentStep");
        assert_camel(&json, "completedStepIds");
        assert_camel(&json, "lastUpdated");
        assert_camel(&json, "practicalMastery");
        // Nested spec
        assert_camel(&json, "requiresDocker");
    }

    #[test]
    fn lab_pty_write_request_camel_case() {
        let req = LabPtyWriteRequest { session_id: "s".to_string(), data: vec![1, 2, 3] };
        let json = serde_json::to_string(&req).unwrap();
        assert_camel(&json, "sessionId");
        assert_camel(&json, "data");
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
        let result = LabShowHintResult { tier: 1, text: "hint text".to_string(), final_tier: false };
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
        let req = LabGetProgressRequest { block_id: "b".to_string(), learner_id: "l".to_string() };
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
        for k in ["blockId", "currentStep", "completedStepIds", "lastUpdated", "practicalMastery"] {
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
    /// source LAB.md text and generation prompt for regen). Wave 0 stub
    /// uses parse_lab_md which returns Err; the test fails because the
    /// expected LabSpec round-trip doesn't survive the missing parser.
    #[test]
    fn lab_block_paramsjson_roundtrip() {
        let lab_md = include_str!(
            "../../tests/fixtures/labs/specs/valid-pod-create.lab.md"
        );

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
        let spec = crate::labs::spec::parse_lab_md(raw)
            .expect("parse_lab_md must succeed once 03.1-02 lands");
        assert_eq!(spec.slug, "pod-create-and-inspect");
        assert_eq!(spec.steps.len(), 4);
    }

    /// LAB-07 — surgical reset removes only the files listed in
    /// `creates: []`; sibling files remain.
    #[test]
    fn reset_surgical_only_removes_declared() {
        let dir = tempfile::tempdir().expect("tempdir");
        let foo = dir.path().join("foo.txt");
        let bar = dir.path().join("bar.txt");
        let manifest_dir = dir.path().join("manifests");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        let pod_yaml = manifest_dir.join("pod.yaml");
        let notes_dir = dir.path().join("notes");
        std::fs::create_dir_all(&notes_dir).unwrap();
        let run_output = notes_dir.join("run-output.txt");

        for p in [&foo, &bar, &pod_yaml, &run_output] {
            std::fs::write(p, "x").unwrap();
        }

        let creates = vec![
            "manifests/pod.yaml".to_string(),
            "notes/run-output.txt".to_string(),
        ];
        let removed = reset_surgical(dir.path(), &creates)
            .expect("reset_surgical must succeed once 03.1-05 lands");
        assert_eq!(removed.len(), 2);
        assert!(!pod_yaml.exists(), "pod.yaml must be deleted");
        assert!(!run_output.exists(), "run-output.txt must be deleted");
        assert!(foo.exists(), "foo.txt must remain (not in creates)");
        assert!(bar.exists(), "bar.txt must remain (not in creates)");
    }

    /// LAB-07 — reset clears the lab_progress row's
    /// completed_step_ids and current_step.
    #[test]
    fn reset_clears_progress_row() {
        // Wave 0: reset_clears_progress returns Err so this test fails.
        // Wave 1 implements against rusqlite + the v006 lab_progress table.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let result = reset_clears_progress(&conn, "blk-1", "lp-1");
        result.expect("reset_clears_progress must succeed once 03.1-05 lands");
    }

    /// LAB-08 — recompute_practical_mastery sums completed/total across
    /// the module's labs. (3/4) + (5/5) = 8/9 ≈ 0.888…; empty -> 0.0.
    #[test]
    fn practical_mastery_compute() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Empty case
        let zero = recompute_practical_mastery(&conn, "mod-1", "lp-1")
            .expect("recompute must succeed once 03.1-02 lands");
        assert!(zero.abs() < 1e-9, "empty case must return 0.0, got {}", zero);
    }
}
