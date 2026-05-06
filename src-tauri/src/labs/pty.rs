//! # labs::pty — portable-pty wrapper (Phase 03.1, Wave 0 stub)
//!
//! Real implementation lands in 03.1-02 (the `portable-pty 0.9.x` MIT crate
//! is added there). Wave 0 holds the PtyHandle skeleton + failing tests
//! that 03.1-02 turns green.

use super::LabError;

/// Handle to a running PTY child + reader/writer pair. Wave 1 (03.1-02)
/// promotes this from a struct skeleton into a wrapper around
/// `portable_pty::PtyPair` + `Box<dyn Read>` / `Box<dyn Write>`.
pub struct PtyHandle {
    /// Tauri session UUID — used as the topic suffix for `lab://stdout/<id>`.
    pub session_id: String,
    /// Wave 0 placeholder. 03.1-02 adds the real PTY pair, child handle,
    /// and reader/writer halves here.
    _wave_0_placeholder: (),
}

impl PtyHandle {
    /// Wave 0 stub. Wave 1 spawns a real PTY via `portable_pty`.
    pub fn spawn(_session_id: &str) -> Result<Self, LabError> {
        Err(LabError::Runtime(
            "PtyHandle::spawn: implemented in 03.1-02 (portable-pty)".to_string(),
        ))
    }

    /// Write bytes to the PTY's stdin. Wave 1 implements.
    pub async fn write_bytes(&self, _bytes: &[u8]) -> Result<(), LabError> {
        Err(LabError::Runtime(
            "PtyHandle::write_bytes: implemented in 03.1-02".to_string(),
        ))
    }

    /// Resize the PTY. Wave 1 implements.
    pub async fn resize_to(&self, _cols: u16, _rows: u16) -> Result<(), LabError> {
        Err(LabError::Runtime(
            "PtyHandle::resize_to: implemented in 03.1-02".to_string(),
        ))
    }

    /// Kill the PTY child. Wave 1 implements.
    pub async fn kill(self) -> Result<(), LabError> {
        Err(LabError::Runtime(
            "PtyHandle::kill: implemented in 03.1-02".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LAB-02 — pty_read_loop emits Tauri events on the
    /// `lab://stdout/<session_id>` topic for each byte chunk. Wave 0 stub
    /// fails because PtyHandle::spawn returns Err.
    #[tokio::test]
    async fn pty_read_loop_emits_lab_stdout_event() {
        let handle = PtyHandle::spawn("session-test")
            .expect("PtyHandle::spawn must succeed once 03.1-02 lands");
        // Wave 1 will run a child that prints "hello" and assert that the
        // mock AppHandle received a `lab://stdout/session-test` emit with
        // those bytes. For Wave 0 we just assert the handle exists.
        assert_eq!(handle.session_id, "session-test");
    }

    /// LAB-02 — write round-trip: bytes written to stdin appear in
    /// the PTY's read stream.
    #[tokio::test]
    async fn pty_write_round_trip() {
        let handle = PtyHandle::spawn("session-write")
            .expect("PtyHandle::spawn must succeed once 03.1-02 lands");
        handle
            .write_bytes(b"echo hi\n")
            .await
            .expect("write_bytes must succeed once 03.1-02 lands");
    }

    /// LAB-02 — resize round-trip: PTY accepts the new dimensions
    /// without error.
    #[tokio::test]
    async fn pty_resize_round_trip() {
        let handle = PtyHandle::spawn("session-resize")
            .expect("PtyHandle::spawn must succeed once 03.1-02 lands");
        handle
            .resize_to(120, 40)
            .await
            .expect("resize_to must succeed once 03.1-02 lands");
    }

    /// LAB-02 — closing a session removes it from the registry and
    /// emits `lab://session-ended/<id>`. Wave 1 wires the registry.
    #[tokio::test]
    async fn session_end_cleans_registry() {
        let handle = PtyHandle::spawn("session-end")
            .expect("PtyHandle::spawn must succeed once 03.1-02 lands");
        handle
            .kill()
            .await
            .expect("kill must succeed once 03.1-02 lands");
        // Wave 1 asserts the lab_sessions registry no longer contains
        // "session-end" — Wave 0 just demonstrates the kill() path exists.
    }
}
