//! # labs::host_shell — host-shell runtime fallback (Phase 03.1, Wave 0 stub)
//!
//! Used when Docker is absent or the learner picked Settings -> Host shell.
//! Wave 1 (03.1-02) wires this against `portable-pty` directly (no Docker
//! exec wrapping); the workspace cwd is the resolved
//! `~/.learnforge/labs/<track>/<module>/` directory and TERM is set to
//! `xterm-256color`.

use super::{LabError, LabSession};
use std::pin::Pin;

/// Host-shell runtime. Wave 1 implements `LabRuntime`.
pub struct HostRuntime {
    // Wave 0 placeholder — 03.1-02 adds shell path, env config.
}

impl HostRuntime {
    pub fn new() -> Self {
        Self {}
    }

    /// Wave 0 stub — Wave 1 spawns a host PTY in `cwd` with TERM set.
    pub fn spawn_in<'a>(
        &'a self,
        _cwd: &'a std::path::Path,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Box<dyn LabSession>, LabError>> + Send + 'a>>
    {
        Box::pin(async move {
            Err(LabError::Runtime(
                "HostRuntime::spawn_in: implemented in 03.1-02".to_string(),
            ))
        })
    }

    /// What TERM is propagated to the child shell. Tested separately so
    /// 03.1-02 can prove xterm-256color round-trips. Wave 0 stub returns
    /// the empty string so `host_shell_passes_TERM_xterm_256color` fails.
    pub fn term_value(&self) -> &str {
        ""
    }
}

impl Default for HostRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LAB-02 / LAB-07 — host shell must spawn with cwd set to the
    /// workspace path so `pwd` returns ~/.learnforge/labs/<track>/<module>/.
    #[tokio::test]
    async fn host_shell_uses_workspace_cwd() {
        let rt = HostRuntime::new();
        let cwd = std::path::Path::new("/tmp/learnforge-labs-host-cwd");
        let session = rt
            .spawn_in(cwd)
            .await
            .expect("spawn_in must succeed once 03.1-02 lands");
        // Wave 1 will write `pwd\n` and assert the captured output ends
        // with the expected path. Wave 0 only assertion: handle exists.
        assert!(!session.session_id().is_empty(), "session id must be set");
    }

    /// LAB-02 — TERM=xterm-256color is propagated so colorized output
    /// (kubectl, ls --color, vim) renders correctly in xterm.js.
    #[test]
    #[allow(non_snake_case)]
    fn host_shell_passes_TERM_xterm_256color() {
        let rt = HostRuntime::new();
        assert_eq!(
            rt.term_value(),
            "xterm-256color",
            "host shell must set TERM=xterm-256color (Wave 0 stub returns \"\")"
        );
    }
}
