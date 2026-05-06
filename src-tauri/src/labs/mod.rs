//! # labs — Hands-on lab subsystem (Phase 03.1)
//!
//! Public surface:
//! - `LabRuntime` / `LabSession` traits — hybrid Docker / host-shell isolation.
//! - `LabError` — single error type for spec / eval / runtime / io failures.
//! - `workspace_path()` — resolves `~/.learnforge/labs/<track>/<module>/` and
//!   creates the directory tree (with traversal-attack guard).
//! - `detect_runtime()` — picks Docker vs host shell from settings + probe.
//! - `requires_docker_notice()` / `effective_runtime_with_warning()` — surface
//!   the override notice when a host-only learner opens a docker-required lab.
//! - `new_session_id()` — UUID v4 string for session topic routing.
//!
//! Submodules: `pty`, `docker`, `host_shell`, `spec`, `evaluator`,
//! `prompt_detect`, `pageplanner_labs`. See each for its own surface.

pub mod docker;
pub mod evaluator;
pub mod host_shell;
pub mod pageplanner_labs;
pub mod prompt_detect;
pub mod pty;
pub mod spec;

#[cfg(test)]
pub mod test_support;

use std::path::{Path, PathBuf};
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
    /// Database / persistence failure.
    #[error("db error: {0}")]
    Db(String),
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
///
/// Two names: `EffectiveRuntime` is the original Wave 0 alias; `RuntimeChoice`
/// is the new Phase 03.1-02 name promoted to public API. Both are kept as
/// type aliases to avoid breaking the wider crate during this wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveRuntime {
    Docker,
    HostShell,
}

/// Promoted alias used by Phase 03.1-02+ code.
pub type RuntimeChoice = EffectiveRuntime;

/// A handle to a running lab (Docker container or host PTY shell). Each lab
/// open creates one of these.
pub trait LabRuntime: Send + Sync {
    /// Start the runtime against `workspace` (the per-learner per-module bind
    /// mount root) and return the session handle.
    fn start<'a>(
        &'a self,
        workspace: &'a Path,
        session_id: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Box<dyn LabSession>, LabError>> + Send + 'a>>;
}

/// A live lab session.
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

/// Generate a fresh UUID v4 string for use as a session topic suffix.
pub fn new_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Validate a path component — must match `^[A-Za-z0-9_-]+$` to prevent
/// traversal attacks in `workspace_path`.
fn is_safe_component(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Resolve the per-learner per-module workspace path under
/// `~/.learnforge/labs/<track>/<module>/`. Creates the directory tree
/// idempotently. Rejects components that contain path separators or
/// traversal sequences (`..`).
pub fn workspace_path(track_id: &str, module_id: &str) -> Result<PathBuf, LabError> {
    if !is_safe_component(track_id) {
        return Err(LabError::Runtime(format!(
            "workspace_path: track_id contains unsafe characters: {:?}",
            track_id
        )));
    }
    if !is_safe_component(module_id) {
        return Err(LabError::Runtime(format!(
            "workspace_path: module_id contains unsafe characters: {:?}",
            module_id
        )));
    }
    let home = dirs::home_dir().ok_or_else(|| {
        LabError::Runtime("workspace_path: unable to resolve home directory".to_string())
    })?;
    let path = home
        .join(".learnforge")
        .join("labs")
        .join(track_id)
        .join(module_id);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Resolve `RuntimeSetting` + probe to a `RuntimeChoice`.
///
/// Behavior:
/// - `Docker` setting → always Docker (caller must handle docker-unavailable
///   downstream by surfacing the failure on `start()`).
/// - `HostShell` setting → always HostShell.
/// - `AutoDetect` → Docker iff probe returns Ok, else HostShell.
pub async fn detect_runtime(
    setting: RuntimeSetting,
    probe: &dyn docker::DockerProbe,
) -> RuntimeChoice {
    match setting {
        RuntimeSetting::Docker => RuntimeChoice::Docker,
        RuntimeSetting::HostShell => RuntimeChoice::HostShell,
        RuntimeSetting::AutoDetect => match probe.probe() {
            Ok(Some(_)) => RuntimeChoice::Docker,
            _ => RuntimeChoice::HostShell,
        },
    }
}

/// When the learner picked HostShell-only and the lab declares
/// `requires_docker: true`, surface a notice so the UI can offer
/// "switch to Auto-detect for this session".
pub fn requires_docker_notice(
    setting: RuntimeSetting,
    spec_requires_docker: bool,
) -> Option<String> {
    if spec_requires_docker && setting == RuntimeSetting::HostShell {
        Some(
            "This lab requires Docker. Switch to Auto-detect for this session \
            to run it inside a container."
                .to_string(),
        )
    } else {
        None
    }
}

/// Combine `detect_runtime` + `requires_docker_notice` so the IPC handler can
/// receive both the effective runtime and an optional override notice in a
/// single call. When the lab requires Docker but the user picked HostShell,
/// the runtime is forced to Docker AND a notice is returned.
pub async fn effective_runtime_with_warning(
    setting: RuntimeSetting,
    probe: &dyn docker::DockerProbe,
    requires_docker: bool,
) -> (RuntimeChoice, Option<String>) {
    if requires_docker && setting == RuntimeSetting::HostShell {
        let notice = requires_docker_notice(setting, requires_docker);
        return (RuntimeChoice::Docker, notice);
    }
    let runtime = detect_runtime(setting, probe).await;
    (runtime, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LAB-07 / LAB-03 — workspace_path resolves to ~/.learnforge/labs/<track>/<module>/
    /// and the directory exists after the call.
    #[test]
    fn workspace_path_resolution() {
        let p = workspace_path("track-1", "module-1")
            .expect("workspace_path must resolve");
        assert!(
            p.ends_with(std::path::Path::new(".learnforge/labs/track-1/module-1")),
            "workspace_path must end with .learnforge/labs/<track>/<module>, got {:?}",
            p
        );
        assert!(
            p.exists(),
            "workspace_path must create the directory tree, got {:?}",
            p
        );
        assert!(p.is_dir(), "workspace_path must be a directory");
    }

    /// LAB-07 — traversal attempts are rejected.
    #[test]
    fn workspace_path_rejects_traversal() {
        let result = workspace_path("../etc", "module-1");
        assert!(
            result.is_err(),
            "traversal track_id must be rejected, got {:?}",
            result
        );
        let result = workspace_path("track-1", "../bin");
        assert!(
            result.is_err(),
            "traversal module_id must be rejected, got {:?}",
            result
        );
        let result = workspace_path("a/b", "module-1");
        assert!(result.is_err(), "slash in track_id must be rejected");
    }

    /// LAB-03 — Auto-detect + DockerProbe(available=true) -> Docker;
    /// AutoDetect + probe Err -> HostShell.
    #[tokio::test]
    async fn auto_detect_resolution() {
        let ok_probe = docker::MockDockerProbe::new(true);
        let resolved = detect_runtime(RuntimeSetting::AutoDetect, &ok_probe).await;
        assert_eq!(
            resolved,
            RuntimeChoice::Docker,
            "AutoDetect + Docker available must resolve to Docker"
        );

        let err_probe = docker::MockDockerProbe::new(false);
        let resolved = detect_runtime(RuntimeSetting::AutoDetect, &err_probe).await;
        assert_eq!(
            resolved,
            RuntimeChoice::HostShell,
            "AutoDetect + Docker unavailable must resolve to HostShell"
        );

        // Explicit settings ignore the probe.
        let resolved = detect_runtime(RuntimeSetting::Docker, &err_probe).await;
        assert_eq!(resolved, RuntimeChoice::Docker);
        let resolved = detect_runtime(RuntimeSetting::HostShell, &ok_probe).await;
        assert_eq!(resolved, RuntimeChoice::HostShell);
    }

    /// LAB-03 — HostShell + spec.requires_docker=true -> Some(notice).
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

        // Negative cases
        assert!(
            requires_docker_notice(RuntimeSetting::HostShell, false).is_none(),
            "no notice when spec doesn't require docker"
        );
        assert!(
            requires_docker_notice(RuntimeSetting::AutoDetect, true).is_none(),
            "no notice when setting is AutoDetect"
        );
        assert!(
            requires_docker_notice(RuntimeSetting::Docker, true).is_none(),
            "no notice when setting is Docker"
        );
    }

    /// LAB-03 — effective_runtime_with_warning returns (Docker, Some(notice))
    /// when host-only learner opens docker-required lab; else (resolved, None).
    #[tokio::test]
    async fn effective_runtime_with_warning_host_plus_required() {
        let probe = docker::MockDockerProbe::new(false);
        let (rt, notice) =
            effective_runtime_with_warning(RuntimeSetting::HostShell, &probe, true).await;
        assert_eq!(rt, RuntimeChoice::Docker);
        assert!(notice.is_some());
    }

    /// LAB-02 — new_session_id returns a non-empty UUID string.
    #[test]
    fn new_session_id_is_uuid_v4() {
        let id = new_session_id();
        assert_eq!(id.len(), 36, "UUID v4 string is 36 chars: {}", id);
        assert_eq!(id.matches('-').count(), 4);
    }
}
