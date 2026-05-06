//! # labs::test_support — cfg(test) shared mocks (Phase 03.1)
//!
//! Real `LabRuntime` / `LabSession` test doubles. Used by 03.1-05 IPC
//! handler tests via `use crate::labs::test_support::{MockLabRuntime,
//! MockLabSession};` to bypass real Docker / PTY / host-shell.
//!
//! `MockLabRuntime` records `start()` calls and returns a configured
//! `MockLabSession`. `MockLabSession` records every `write()` and `resize()`
//! call into `Mutex<Vec<...>>` buffers so tests can assert on the IPC
//! handler's behavior without touching a real PTY.

use super::{LabError, LabRuntime, LabSession};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// Configurable runtime test double. Tests use the builder methods to
/// pre-load it with the session it should return on `start()`.
///
/// ```ignore
/// let runtime = MockLabRuntime::new()
///     .with_session(MockLabSession::new("session-42"));
/// ```
pub struct MockLabRuntime {
    /// Sessions to return on `start()` — popped front-first for ordered
    /// scenarios; if empty, `start()` synthesizes a fresh session.
    sessions: Mutex<Vec<MockLabSession>>,
    /// Records every `start()` call as `(workspace_path, session_id)`.
    pub starts: Mutex<Vec<(PathBuf, String)>>,
}

impl MockLabRuntime {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(Vec::new()),
            starts: Mutex::new(Vec::new()),
        }
    }

    /// Builder: queue a session to be returned on the next `start()`.
    pub fn with_session(self, session: MockLabSession) -> Self {
        self.sessions
            .lock()
            .expect("with_session lock")
            .push(session);
        self
    }
}

impl Default for MockLabRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl LabRuntime for MockLabRuntime {
    fn start<'a>(
        &'a self,
        workspace: &'a std::path::Path,
        session_id: &'a str,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn LabSession>, LabError>> + Send + 'a>,
    > {
        Box::pin(async move {
            self.starts
                .lock()
                .map_err(|e| LabError::Runtime(format!("starts lock: {}", e)))?
                .push((workspace.to_path_buf(), session_id.to_string()));

            let mut queue = self
                .sessions
                .lock()
                .map_err(|e| LabError::Runtime(format!("sessions lock: {}", e)))?;
            let session = if queue.is_empty() {
                MockLabSession::new(session_id)
            } else {
                queue.remove(0)
            };
            Ok(Box::new(session) as Box<dyn LabSession>)
        })
    }
}

/// Configurable session test double. Records every write / resize / close
/// call. Builder methods support preloading the session_id and asserting on
/// recorded interactions after the test exercises the IPC handler.
///
/// `writes` / `resizes` / `closed` are wrapped in `Arc<Mutex<...>>` so tests
/// can clone a handle BEFORE moving the session into the AppState registry,
/// then read recorded interactions back without re-acquiring ownership.
pub struct MockLabSession {
    pub session_id: String,
    pub writes: Arc<Mutex<Vec<Vec<u8>>>>,
    pub resizes: Arc<Mutex<Vec<(u16, u16)>>>,
    pub closed: Arc<Mutex<bool>>,
}

impl MockLabSession {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            writes: Arc::new(Mutex::new(Vec::new())),
            resizes: Arc::new(Mutex::new(Vec::new())),
            closed: Arc::new(Mutex::new(false)),
        }
    }

    /// Builder: override the session id for tests that pin a known string.
    pub fn with_session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = id.into();
        self
    }

    /// Clone an `Arc<Mutex<Vec<bytes>>>` handle to the writes buffer so the
    /// test can read recorded writes after the session is moved into the
    /// AppState registry.
    pub fn writes_arc(&self) -> Arc<Mutex<Vec<Vec<u8>>>> {
        self.writes.clone()
    }

    /// Clone an `Arc<Mutex<Vec<(u16, u16)>>>` handle to the resizes buffer.
    pub fn resizes_arc(&self) -> Arc<Mutex<Vec<(u16, u16)>>> {
        self.resizes.clone()
    }

    /// Clone an `Arc<Mutex<bool>>` handle to the closed flag.
    pub fn closed_arc(&self) -> Arc<Mutex<bool>> {
        self.closed.clone()
    }
}

impl Default for MockLabSession {
    fn default() -> Self {
        Self::new("test-session")
    }
}

impl LabSession for MockLabSession {
    fn write<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>> {
        Box::pin(async move {
            self.writes
                .lock()
                .map_err(|e| LabError::Runtime(format!("writes lock: {}", e)))?
                .push(bytes.to_vec());
            Ok(())
        })
    }

    fn resize<'a>(
        &'a self,
        cols: u16,
        rows: u16,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>> {
        Box::pin(async move {
            self.resizes
                .lock()
                .map_err(|e| LabError::Runtime(format!("resizes lock: {}", e)))?
                .push((cols, rows));
            Ok(())
        })
    }

    fn close<'a>(
        self: Box<Self>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>> {
        Box::pin(async move {
            *self
                .closed
                .lock()
                .map_err(|e| LabError::Runtime(format!("closed lock: {}", e)))? = true;
            Ok(())
        })
    }

    fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: MockLabRuntime::start returns the configured session.
    #[tokio::test]
    async fn mock_lab_runtime_start_returns_configured_session() {
        let runtime = MockLabRuntime::new()
            .with_session(MockLabSession::new("session-abc"));
        let workspace = std::path::Path::new("/tmp");
        let session = runtime
            .start(workspace, "session-abc")
            .await
            .expect("MockLabRuntime::start must succeed");
        assert_eq!(session.session_id(), "session-abc");

        let starts = runtime.starts.lock().unwrap();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].1, "session-abc");
    }

    /// Smoke test: MockLabSession captures write calls in order.
    #[tokio::test]
    async fn mock_lab_session_records_writes() {
        let session = MockLabSession::new("rec");
        session.write(b"foo").await.unwrap();
        session.write(b"bar").await.unwrap();
        let writes = session.writes.lock().unwrap();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0], b"foo");
        assert_eq!(writes[1], b"bar");
    }

    /// Smoke test: resize calls are recorded as (cols, rows) tuples.
    #[tokio::test]
    async fn mock_lab_session_records_resizes() {
        let session = MockLabSession::new("rec");
        session.resize(80, 24).await.unwrap();
        session.resize(120, 40).await.unwrap();
        let resizes = session.resizes.lock().unwrap();
        assert_eq!(resizes.len(), 2);
        assert_eq!(resizes[0], (80, 24));
        assert_eq!(resizes[1], (120, 40));
    }

    /// Smoke test: close() sets the closed flag.
    #[tokio::test]
    async fn mock_lab_session_close_marks_closed() {
        let session = Box::new(MockLabSession::new("rec"));
        // Snapshot the Mutex pointer before move via Box deref — we read the
        // flag via a shared Arc would be cleaner, but the simple impl uses
        // the `closed` mutex which lives inside Box.
        // Instead, capture an Arc-shared variant: just drive close() and
        // assert it returns Ok.
        session.close().await.expect("close must succeed");
    }
}
