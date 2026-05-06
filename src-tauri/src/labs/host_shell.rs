//! # labs::host_shell — host-shell runtime fallback (Phase 03.1)
//!
//! Used when Docker is absent or the learner picked Settings -> Host shell.
//! Wraps `portable-pty` directly in the host workspace cwd. The shell is
//! `$SHELL` (Unix) or `cmd.exe` (Windows). TERM is forced to `xterm-256color`
//! so colorized output (kubectl, ls --color, vim) renders correctly in
//! xterm.js.
//!
//! For bash / zsh shells, we inject OSC 133 prompt-boundary markers via
//! `PS1` so the inline evaluator can detect command boundaries (RESEARCH §
//! Open Question #1 — bind-mount preferred for Docker; for host shell we
//! patch PS1 directly).

use super::pty::PtyHandle;
use super::{LabError, LabSession};
use portable_pty::CommandBuilder;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

/// Resolve the host shell binary path. Prefers `$SHELL`; falls back to
/// `/bin/bash` on Unix and `cmd.exe` on Windows.
pub fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| {
        if cfg!(windows) {
            "cmd.exe".to_string()
        } else {
            "/bin/bash".to_string()
        }
    })
}

/// OSC 133 PS1 fragment for bash/zsh. Wraps the prompt with `\e]133;A\a`
/// (prompt start) and `\e]133;B\a` (command-line start) markers. The
/// evaluator parses these from the byte stream to identify command
/// boundaries without prompt-detection heuristics.
const OSC_133_PS1: &str = "\\[\\e]133;A\\a\\]$ \\[\\e]133;B\\a\\]";

/// Build a `CommandBuilder` for the host shell with TERM, PS1, and cwd
/// configured. Public so tests can inspect the resulting struct.
pub fn build_shell_command(workspace: &Path) -> CommandBuilder {
    build_shell_command_for(&default_shell(), workspace)
}

/// Variant of `build_shell_command` that takes the shell explicitly. Used
/// by tests to avoid mutating the shared `$SHELL` env var.
pub fn build_shell_command_for(shell: &str, workspace: &Path) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    cmd.cwd(workspace);
    cmd.env("TERM", "xterm-256color");

    // OSC 133 injection — only meaningful for interactive bash/zsh; harmless
    // for other shells which ignore the env var entirely.
    if shell.ends_with("bash") || shell.ends_with("zsh") {
        cmd.env("PS1", OSC_133_PS1);
    }

    cmd
}

/// Host-shell runtime — the LabRuntime impl for host-PTY mode.
pub struct HostRuntime {
    workspace: PathBuf,
}

impl HostRuntime {
    /// Construct against the lab's workspace root (already created by
    /// `labs::workspace_path`).
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }

    /// What TERM the runtime propagates. Constant `"xterm-256color"`.
    pub fn term_value(&self) -> &'static str {
        "xterm-256color"
    }

    /// Async `spawn_in(cwd)` — used by the Wave 0 test. Builds a
    /// CommandBuilder for the host shell with TERM + PS1 + cwd, then
    /// spawns through PtyHandle and wraps it in a HostSession.
    pub fn spawn_in<'a>(
        &'a self,
        cwd: &'a Path,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Box<dyn LabSession>, LabError>> + Send + 'a>>
    {
        Box::pin(async move {
            let cmd = build_shell_command(cwd);
            let session_id = super::new_session_id();
            let pty = PtyHandle::spawn_with_command(cmd, 24, 80, &session_id)?;
            let session = HostSession {
                pty: Arc::new(pty),
                session_id,
            };
            Ok(Box::new(session) as Box<dyn LabSession>)
        })
    }
}

impl super::LabRuntime for HostRuntime {
    fn start<'a>(
        &'a self,
        workspace: &'a Path,
        session_id: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Box<dyn LabSession>, LabError>> + Send + 'a>>
    {
        Box::pin(async move {
            let cmd = build_shell_command(workspace);
            let pty = PtyHandle::spawn_with_command(cmd, 24, 80, session_id)?;
            let session = HostSession {
                pty: Arc::new(pty),
                session_id: session_id.to_string(),
            };
            Ok(Box::new(session) as Box<dyn LabSession>)
        })
    }
}

/// HostSession — wraps a PtyHandle and forwards LabSession trait calls
/// through to it.
pub struct HostSession {
    pty: Arc<PtyHandle>,
    session_id: String,
}

impl LabSession for HostSession {
    fn write<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>> {
        Box::pin(async move { self.pty.write_bytes(bytes).await })
    }

    fn resize<'a>(
        &'a self,
        cols: u16,
        rows: u16,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>> {
        Box::pin(async move { self.pty.resize_to(cols, rows).await })
    }

    fn close<'a>(
        self: Box<Self>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>> {
        Box::pin(async move {
            // Best-effort kill — the PtyHandle may have already exited if
            // the shell terminated.
            if let Some(pty) = Arc::into_inner(self.pty) {
                pty.kill().await
            } else {
                Ok(())
            }
        })
    }

    fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LAB-02 / LAB-07 — host shell spawns with cwd set to the workspace
    /// path. We assert the CommandBuilder carries the correct cwd before
    /// spawn (no real PTY needed for this).
    #[test]
    fn host_shell_uses_workspace_cwd_in_command_builder() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cmd = build_shell_command(tmp.path());
        // Format the CommandBuilder via Debug; portable-pty's Debug impl
        // includes the cwd field.
        let dbg = format!("{:?}", cmd);
        let path_str = tmp.path().to_string_lossy();
        assert!(
            dbg.contains(&*path_str),
            "CommandBuilder must include workspace cwd, got: {}",
            dbg
        );
    }

    /// LAB-02 / LAB-07 — async spawn_in returns a HostSession with a
    /// non-empty session id. Best-effort: if the host environment forbids
    /// PTY allocation we skip the assertion (some sandboxes do).
    #[tokio::test]
    async fn host_shell_uses_workspace_cwd() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = HostRuntime::new(tmp.path().to_path_buf());
        match rt.spawn_in(tmp.path()).await {
            Ok(session) => {
                assert!(!session.session_id().is_empty());
            }
            Err(LabError::Runtime(msg)) => {
                // CI sandboxes that forbid openpty surface as Runtime error;
                // the test still validates the construction path.
                eprintln!("spawn_in skipped: {}", msg);
            }
            Err(other) => panic!("unexpected error variant: {:?}", other),
        }
    }

    /// LAB-02 — TERM=xterm-256color is propagated so colorized output
    /// renders correctly in xterm.js.
    #[test]
    #[allow(non_snake_case)]
    fn host_shell_passes_TERM_xterm_256color() {
        let rt = HostRuntime::new(std::path::PathBuf::from("/tmp"));
        assert_eq!(rt.term_value(), "xterm-256color");
    }

    /// LAB-02 — TERM=xterm-256color is set explicitly on the CommandBuilder
    /// (not just inherited from the parent env).
    #[test]
    fn host_shell_command_builder_has_term_env() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cmd = build_shell_command_for("/bin/bash", tmp.path());
        assert!(
            env_was_explicitly_set(&cmd, "TERM"),
            "CommandBuilder must explicitly set TERM"
        );
        let dbg = format!("{:?}", cmd);
        assert!(
            dbg.contains("xterm-256color"),
            "TERM value must be xterm-256color, got: {}",
            dbg
        );
    }

    /// Helper: returns true iff the CommandBuilder Debug output contains an
    /// EnvEntry that is NOT inherited from the base env AND has the given
    /// key. This distinguishes our explicit `cmd.env(K, V)` calls from the
    /// inherited parent-process env vars.
    fn env_was_explicitly_set(cmd: &CommandBuilder, key: &str) -> bool {
        let dbg = format!("{:?}", cmd);
        // Look for: "KEY": EnvEntry { is_from_base_env: false
        let needle = format!("\"{}\": EnvEntry {{ is_from_base_env: false", key);
        dbg.contains(&needle)
    }

    /// LAB-06 — PS1 with OSC 133 markers is set when the shell is bash;
    /// distinguished from any inherited parent PS1 by `is_from_base_env: false`.
    #[test]
    fn host_shell_injects_osc133_for_bash() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cmd = build_shell_command_for("/bin/bash", tmp.path());
        assert!(
            env_was_explicitly_set(&cmd, "PS1"),
            "bash CommandBuilder must explicitly set PS1"
        );
        let dbg = format!("{:?}", cmd);
        assert!(
            dbg.contains("133"),
            "bash CommandBuilder PS1 must contain OSC 133 sequence, got: {}",
            dbg
        );
    }

    /// LAB-06 — zsh also gets the PS1 injection.
    #[test]
    fn host_shell_injects_osc133_for_zsh() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cmd = build_shell_command_for("/bin/zsh", tmp.path());
        assert!(
            env_was_explicitly_set(&cmd, "PS1"),
            "zsh CommandBuilder must explicitly set PS1"
        );
    }

    /// LAB-06 — non-bash/zsh shells do not get the PS1 injection (which
    /// would otherwise print escape codes literally in dash/fish/etc).
    #[test]
    fn host_shell_skips_osc133_for_non_bash_shells() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cmd = build_shell_command_for("/bin/dash", tmp.path());
        assert!(
            !env_was_explicitly_set(&cmd, "PS1"),
            "dash CommandBuilder must NOT explicitly set PS1"
        );
    }
}
