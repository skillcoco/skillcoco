//! # labs — Hands-on lab subsystem (Phase 03.1)
//!
//! Wave 0 scaffold. Real implementations land in plans 03.1-02 .. 03.1-07.
//!
//! Public surface:
//! - `LabRuntime` / `LabSession` traits — hybrid Docker / host-shell isolation.
//! - `LabError` — single error type for spec / eval / runtime / io failures.
//! - `workspace_path()` — resolves `~/.learnforge/labs/<track>/<module>/`.
//! - `detect_runtime()` — picks Docker vs host shell from settings + probe.
//!
//! Submodules carry the per-feature scaffolds:
//!   `pty`, `docker`, `host_shell`, `spec`, `evaluator`, `prompt_detect`,
//!   `pageplanner_labs`. Each contains its own `#[cfg(test)] mod tests` block
//!   with FAILING tests that downstream Wave 1+ tasks turn green.

pub mod docker;
pub mod evaluator;
pub mod host_shell;
pub mod pageplanner_labs;
pub mod prompt_detect;
pub mod pty;
pub mod spec;

#[cfg(test)]
pub mod test_support;

use std::path::PathBuf;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

/// Single error type for the labs subsystem. `From<std::io::Error>` is
/// derived via `thiserror` so `?` works against `std::fs` calls.
#[derive(Debug, thiserror::Error)]
pub enum LabError {
    /// LAB.md parse / validation failure.
    #[error("spec error: {0}")]
    Spec(String),
    /// Step evaluator failure.
    #[error("eval error: {0}")]
    Eval(String),
    /// Runtime (Docker / PTY / host shell) failure.
    #[error("runtime error: {0}")]
    Runtime(String),
    /// IO error bubbled from std::fs / std::io.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// User-selected runtime preference. Persisted in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeSetting {
    Docker,
    HostShell,
    AutoDetect,
}

/// Resolved runtime after consulting `RuntimeSetting` + Docker probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveRuntime {
    Docker,
    HostShell,
}

/// A handle to a running lab (Docker container or host PTY shell). Each lab
/// open creates one of these. Wave 1 (03.1-02) fills in the impls on
/// `docker::DockerRuntime` and `host_shell::HostRuntime`.
pub trait LabRuntime: Send + Sync {
    /// Start the runtime against `workspace` (the per-learner per-module bind
    /// mount root) and return the session handle.
    fn start<'a>(
        &'a self,
        workspace: &'a std::path::Path,
        session_id: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Box<dyn LabSession>, LabError>> + Send + 'a>>;
}

/// A live lab session. Wave 1 fills in the impls; Wave 0 keeps the trait
/// shape stable so downstream files (`commands/labs.rs`, IPC handler tests)
/// can reference it.
pub trait LabSession: Send + Sync {
    /// Write bytes to the PTY's stdin.
    fn write<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>>;

    /// Resize the PTY (cols, rows).
    fn resize<'a>(
        &'a self,
        cols: u16,
        rows: u16,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>>;

    /// Kill the session (close container, drop PTY). Consumes self.
    fn close<'a>(
        self: Box<Self>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>>;

    /// Session ID for event-topic routing.
    fn session_id(&self) -> &str;
}

/// Resolve the per-learner per-module workspace path. Wave 1 (03.1-02)
/// implements; this stub returns `Err` so the workspace_path_resolution test
/// fails loudly.
pub fn workspace_path(_track_id: &str, _module_id: &str) -> Result<PathBuf, LabError> {
    Err(LabError::Runtime(
        "workspace_path: implemented in 03.1-02".to_string(),
    ))
}

/// Resolve `RuntimeSetting` + probe to an `EffectiveRuntime`. Wave 1
/// (03.1-02) wires `docker::DockerProbe` against bollard; this stub returns
/// `Err` so the auto_detect_resolution test fails.
pub fn detect_runtime(
    _setting: RuntimeSetting,
    _probe: &dyn docker::DockerProbe,
) -> Result<EffectiveRuntime, LabError> {
    Err(LabError::Runtime(
        "detect_runtime: implemented in 03.1-02".to_string(),
    ))
}

/// When the learner picked HostShell-only and the lab declares
/// `requires_docker: true`, surface a notice so the UI can offer
/// "switch to Auto-detect for this session". Wave 1 implements.
pub fn requires_docker_notice(
    _setting: RuntimeSetting,
    _spec_requires_docker: bool,
) -> Option<String> {
    // Returning None unconditionally makes the failing test obvious — Wave 1
    // returns Some("...") for the (HostShell, true) case.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LAB-07 / LAB-03 — workspace_path resolves to ~/.learnforge/labs/<track>/<module>/.
    /// Wave 0: workspace_path returns Err so this test fails.
    #[test]
    fn workspace_path_resolution() {
        let p = workspace_path("track-1", "module-1")
            .expect("workspace_path must resolve once 03.1-02 lands");
        assert!(
            p.ends_with(std::path::Path::new("learnforge/labs/track-1/module-1")),
            "workspace_path must end with learnforge/labs/<track>/<module>, got {:?}",
            p
        );
    }

    /// LAB-03 — Auto-detect + DockerProbe(available=true) -> Docker.
    /// Wave 0: detect_runtime returns Err so this test fails.
    #[test]
    fn auto_detect_resolution() {
        let probe = docker::MockDockerProbe::new(true);
        let resolved = detect_runtime(RuntimeSetting::AutoDetect, &probe)
            .expect("detect_runtime must resolve once 03.1-02 lands");
        assert_eq!(
            resolved,
            EffectiveRuntime::Docker,
            "AutoDetect + Docker available must resolve to Docker"
        );
    }

    /// LAB-03 — HostShell + spec.requires_docker=true -> Some(notice).
    /// Wave 0: requires_docker_notice returns None so this test fails.
    #[test]
    fn requires_docker_override_notice() {
        let notice = requires_docker_notice(RuntimeSetting::HostShell, true);
        assert!(
            notice.is_some(),
            "HostShell setting + requires_docker spec must surface a notice"
        );
        let txt = notice.unwrap();
        assert!(
            txt.to_lowercase().contains("docker") || txt.to_lowercase().contains("auto-detect"),
            "notice text should mention Docker or Auto-detect, got {:?}",
            txt
        );
    }
}
