//! # labs::test_support — cfg(test) shared mocks (Phase 03.1, Wave 0 stub)
//!
//! 03.1-02 promotes these to real `LabRuntime` / `LabSession` impls.
//! 03.1-05 IPC handler tests `use crate::labs::test_support::{MockLabRuntime,
//! MockLabSession};` — Wave 0 stubs exist now so 03.1-05 doesn't introduce
//! a fresh file mid-wave.

use super::{LabError, LabRuntime, LabSession};
use std::pin::Pin;

#[derive(Debug)]
pub struct MockLabRuntime {
    pub session_id: String,
}

impl MockLabRuntime {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }
}

impl LabRuntime for MockLabRuntime {
    fn start<'a>(
        &'a self,
        _workspace: &'a std::path::Path,
        _session_id: &'a str,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn LabSession>, LabError>> + Send + 'a>,
    > {
        Box::pin(async move {
            // Wave 0 — the IPC handler tests in 03.1-05 will assert that
            // start() returns Ok(Box<MockLabSession>). Today it returns Err
            // so the still-to-be-written tests fail loudly.
            Err(LabError::Runtime(
                "MockLabRuntime::start: real impl lands in 03.1-02".to_string(),
            ))
        })
    }
}

#[derive(Debug)]
pub struct MockLabSession {
    pub session_id: String,
    pub written: std::sync::Mutex<Vec<u8>>,
    pub last_resize: std::sync::Mutex<Option<(u16, u16)>>,
}

impl MockLabSession {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            written: std::sync::Mutex::new(Vec::new()),
            last_resize: std::sync::Mutex::new(None),
        }
    }
}

impl LabSession for MockLabSession {
    fn write<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>> {
        Box::pin(async move {
            self.written
                .lock()
                .map_err(|e| LabError::Runtime(format!("write lock: {}", e)))?
                .extend_from_slice(bytes);
            Ok(())
        })
    }

    fn resize<'a>(
        &'a self,
        cols: u16,
        rows: u16,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>> {
        Box::pin(async move {
            *self
                .last_resize
                .lock()
                .map_err(|e| LabError::Runtime(format!("resize lock: {}", e)))? =
                Some((cols, rows));
            Ok(())
        })
    }

    fn close<'a>(
        self: Box<Self>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LabError>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn session_id(&self) -> &str {
        &self.session_id
    }
}
