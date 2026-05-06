//! # labs::docker — Docker runtime (Phase 03.1, Wave 0 stub)
//!
//! Real implementation lands in 03.1-02 with `bollard 0.19+` (Apache-2.0).
//! Wave 0 carries:
//! - `DockerProbe` trait with `MockDockerProbe` so unit tests need zero
//!   real Docker daemon.
//! - `DockerRuntime` skeleton (no LabRuntime impl yet — added 03.1-02).
//! - Failing `#[cfg(test)]` scaffolds for probe + lifecycle.

use super::{LabError, LabSession};
use std::pin::Pin;

/// Probes for Docker daemon availability. Production impl wraps
/// `bollard::Docker::connect_with_local_defaults().ping()`. Tests use the
/// `MockDockerProbe` below.
pub trait DockerProbe: Send + Sync {
    /// Returns `Ok(version_string)` when Docker is reachable, `Err(reason)`
    /// otherwise. `Some(...)` version implies the daemon answered ping.
    fn probe(&self) -> Result<Option<String>, LabError>;
}

/// Test-only mock — no real Docker socket touched.
pub struct MockDockerProbe {
    available: bool,
}

impl MockDockerProbe {
    pub fn new(available: bool) -> Self {
        Self { available }
    }
}

impl DockerProbe for MockDockerProbe {
    fn probe(&self) -> Result<Option<String>, LabError> {
        if self.available {
            Ok(Some("MockDocker 0.0.0".to_string()))
        } else {
            Err(LabError::Runtime("docker not available".to_string()))
        }
    }
}

/// Docker runtime. Wave 1 (03.1-02) implements `LabRuntime`.
pub struct DockerRuntime {
    // Wave 0 placeholder — 03.1-02 adds `client: bollard::Docker`, image
    // reference, bind-mount config, etc.
}

impl DockerRuntime {
    pub fn new() -> Self {
        Self {}
    }

    /// Wave 0 stub for container lifecycle test. Wave 1 wires bollard.
    pub fn create_with_bind_mount<'a>(
        &'a self,
        _image: &'a str,
        _workspace: &'a std::path::Path,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Box<dyn LabSession>, LabError>> + Send + 'a>>
    {
        Box::pin(async move {
            Err(LabError::Runtime(
                "DockerRuntime::create_with_bind_mount: implemented in 03.1-02".to_string(),
            ))
        })
    }
}

impl Default for DockerRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LAB-03 — probe returns Some(version) when Docker is available.
    #[test]
    fn probe_returns_version_when_available() {
        let probe = MockDockerProbe::new(true);
        let result = probe.probe().expect("probe must succeed when available=true");
        assert!(
            result.is_some(),
            "probe must return Some(version) when Docker is available"
        );
        let v = result.unwrap();
        assert!(!v.is_empty(), "version string must be non-empty");
        // FAIL until Wave 1 strengthens the assertion to a real bollard
        // version pattern (e.g. starts with a digit). Mock returns
        // "MockDocker 0.0.0" which starts with 'M' — assert against the real
        // pattern so the production-path test goes red.
        assert!(
            v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false),
            "probe must report a real numeric Docker version once 03.1-02 wires bollard, got {:?}",
            v
        );
    }

    /// LAB-03 — probe returns Err when Docker is not available.
    #[test]
    fn probe_returns_err_when_unavailable() {
        let probe = MockDockerProbe::new(false);
        let result = probe.probe();
        assert!(
            result.is_err(),
            "probe must return Err when Docker is not available"
        );
    }

    /// LAB-03 / LAB-07 — DockerRuntime::create_with_bind_mount creates a
    /// container with the workspace bind-mounted at /workspace. Wave 0
    /// stub returns Err so this fails until 03.1-02 wires bollard.
    #[tokio::test]
    async fn container_lifecycle_creates_with_bind_mount() {
        let rt = DockerRuntime::new();
        let workspace = std::path::Path::new("/tmp/learnforge-labs-test");
        let session = rt
            .create_with_bind_mount("kindest/node:v1.30", workspace)
            .await
            .expect("create_with_bind_mount must succeed once 03.1-02 lands");
        assert!(
            !session.session_id().is_empty(),
            "session_id must be non-empty"
        );
    }
}
