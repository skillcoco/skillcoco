//! Phase 6 (Certification) — artifact rendering (PDF cert + PNG badge + QR).
//!
//! Wave 0 declares the surface only. Wave 2 (Plan 06-03) implements:
//!   - `render_certificate_pdf`  -> printpdf 0.9 with Helvetica Std 14 (A5 lock).
//!   - `render_badge_png`         -> raster PNG, QR + optional brand mark.
//!                                  Text labels deferred to Phase 14 (D-06 amend).
//!   - `render_qr_png`            -> qrcode 0.14 + image 0.25 shared helper.
//!
//! D-06 (amended): PNG badge ships QR + optional vector brand mark only.
//! Phase 14 revisits raster text labels alongside custom-font work.

use super::AchievementError;
use serde::{Deserialize, Serialize};

/// Inputs the certificate PDF renderer needs. Hands the QR PNG in as raw
/// bytes so the renderer doesn't need to know about the QR pipeline. The
/// printpdf `images` feature decodes the PNG before embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificatePdfInput {
    pub learner_name: String,
    pub track_topic: String,
    pub issued_at: String,
    pub mastery_score: f64,
    pub key_fingerprint_short: String,
    pub level: String,
    pub qr_png_bytes: Vec<u8>,
}

/// Inputs the badge PNG renderer needs. D-06 amendment: text labels are
/// Phase 14 work; Wave 2 ships QR + optional vector brand mark only. The
/// fields are still on the struct so Phase 14 doesn't need a wire change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgePngInput {
    pub level: String,
    pub track_topic: String,
    pub issued_at: String,
    pub key_fingerprint_short: String,
    pub qr_png_bytes: Vec<u8>,
}

/// Render a PDF certificate. Wave 2 fills via printpdf 0.9 (`images`
/// feature) + Helvetica Std 14 — no font embed (A5 lock).
pub fn render_certificate_pdf(
    _input: &CertificatePdfInput,
) -> Result<Vec<u8>, AchievementError> {
    Err(AchievementError::Validation(
        "Plan 06-03 (Wave 2) implements render_certificate_pdf".to_string(),
    ))
}

/// Render the skill-level badge as a PNG (transparent background, QR +
/// optional vector brand mark per D-06 amendment).
pub fn render_badge_png(_input: &BadgePngInput) -> Result<Vec<u8>, AchievementError> {
    Err(AchievementError::Validation(
        "Plan 06-03 (Wave 2) implements render_badge_png".to_string(),
    ))
}

/// Render a standalone QR PNG (the same helper both `render_certificate_pdf`
/// and `render_badge_png` call internally). Locked QR stack: qrcode 0.14 +
/// image 0.25 (A2 lock — NOT fast_qr).
pub fn render_qr_png(_qr_payload: &str) -> Result<Vec<u8>, AchievementError> {
    Err(AchievementError::Validation(
        "Plan 06-03 (Wave 2) implements render_qr_png".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    //! Wave 0 smoke tests — focus on proving the locked deps resolve.

    use super::*;

    #[test]
    #[ignore = "Plan 06-03 (Wave 2) implements render_certificate_pdf"]
    fn pdf_smoke() {
        // RED contract: real renderer returns a non-empty Vec<u8> starting
        // with the PDF magic bytes "%PDF-".
        let input = CertificatePdfInput {
            learner_name: "Test Learner".to_string(),
            track_topic: "Kubernetes Fundamentals".to_string(),
            issued_at: "2026-06-15T00:00:00Z".to_string(),
            mastery_score: 0.92,
            key_fingerprint_short: "deadbeef".to_string(),
            level: "Professional".to_string(),
            qr_png_bytes: vec![],
        };
        let bytes = render_certificate_pdf(&input).expect("Wave 2 must produce PDF bytes");
        assert!(!bytes.is_empty(), "PDF must be non-empty");
        assert!(
            bytes.starts_with(b"%PDF-"),
            "PDF must start with magic bytes (got first 8: {:?})",
            &bytes[..bytes.len().min(8)]
        );
    }

    #[test]
    fn qr_dep_resolution_smoke() {
        // Wave 0 dep-availability smoke test (NOT ignored — runs every
        // build). Proves the qrcode 0.14 + image 0.25 crates resolve and
        // their core API surfaces compile. If this test fails to compile,
        // the Cargo.toml dep adds did not take.
        let code = qrcode::QrCode::new("learnforge-wave-0-smoke")
            .expect("qrcode 0.14 must build a QR from a short string");
        // Render to image buffer via `image` 0.25 to prove the pair works
        // together. We only need to compile the call — checking it ran is
        // enough.
        let _img: image::ImageBuffer<image::Luma<u8>, Vec<u8>> = code
            .render::<image::Luma<u8>>()
            .max_dimensions(200, 200)
            .build();
    }

    #[test]
    #[ignore = "Plan 06-03 (Wave 2) implements render_qr_png"]
    fn qr_png_smoke() {
        // RED contract: real renderer returns non-empty PNG bytes.
        let bytes = render_qr_png("learnforge-test-payload")
            .expect("Wave 2 must produce QR PNG bytes");
        assert!(!bytes.is_empty(), "QR PNG must be non-empty");
        // PNG magic bytes: 89 50 4E 47 0D 0A 1A 0A
        assert_eq!(
            &bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            "QR PNG must start with PNG magic bytes"
        );
    }
}
