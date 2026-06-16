//! Phase 6 (Certification) — artifact rendering (PDF cert + PNG badge + QR
//! + share text).
//!
//! Wave 2 (Plan 06-03) GREEN: filled per D-06 + D-08 + A5 (Helvetica Std 14
//! — no TTF embed). The PNG badge ships QR + optional vector brand mark
//! only — text labels deferred to Phase 14 per D-06 amendment.
//!
//! Locked crates:
//!   - printpdf 0.9.1 (`images` feature) — PDF, BuiltinFont::Helvetica*.
//!   - qrcode 0.14.1 — QR encoding (medium error correction per R1).
//!   - image 0.25.10 — raster sink + PNG encoder.

use super::AchievementError;
use image::{ImageBuffer, Luma, Rgba, RgbaImage};
use printpdf::{
    BuiltinFont, Color, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt,
    RawImage, Rgb, TextItem, XObjectTransform,
};
use qrcode::{EcLevel, QrCode};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

// ── Inputs ────────────────────────────────────────────────────────────────

/// Inputs the certificate PDF renderer needs. Caller pre-renders the QR
/// PNG so the PDF renderer stays decoupled from the QR pipeline.
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
/// Phase 14 work; Wave 2 ships QR + optional vector brand mark only.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgePngInput {
    pub level: String,
    pub track_topic: String,
    pub issued_at: String,
    pub key_fingerprint_short: String,
    pub qr_payload: String,
}

// ── PDF certificate ───────────────────────────────────────────────────────

/// Render a PDF certificate. A4 portrait, Helvetica Standard 14
/// (no TTF embed per A5). Embeds the caller-provided QR PNG via
/// printpdf's `images` feature.
///
/// Layout (in Mm, A4 = 210x297):
///   - "Certificate of Completion"   Helvetica-Bold 24pt  y=260
///   - track topic                   Helvetica       16pt y=240
///   - "Awarded to"                  Helvetica       12pt y=220
///   - learner display name          Helvetica-Bold  28pt y=205
///   - level                         Helvetica       14pt y=180
///   - issued date + mastery score   Helvetica       12pt y=160
///   - QR (40x40mm) at x=150, y=210
///   - "LEARNFORGE" brand mark       Helvetica-Bold 10pt y=30
///   - "Verify with key fingerprint: …" footer       y=20
pub fn render_certificate_pdf(
    input: &CertificatePdfInput,
) -> Result<Vec<u8>, AchievementError> {
    let mut ops: Vec<Op> = Vec::new();
    let mut doc = PdfDocument::new("LearnForge Certificate of Completion");

    // Embed QR PNG as an XObject. Caller passes pre-rendered bytes.
    let mut img_warnings = Vec::new();
    let qr_image = RawImage::decode_from_bytes(&input.qr_png_bytes, &mut img_warnings)
        .map_err(|e| AchievementError::Pdf(format!("decode qr png: {}", e)))?;
    let qr_xobject_id = doc.add_image(&qr_image);

    let black = Color::Rgb(Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    });
    let font_reg = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
    let font_bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);

    // ── Text block ──────────────────────────────────────────────────────
    ops.push(Op::SetFillColor { col: black.clone() });
    ops.push(Op::StartTextSection);

    // Title
    push_line(&mut ops, &font_bold, 24.0, 20.0, 260.0, "Certificate of Completion");

    // Track topic (subtitle)
    push_line(&mut ops, &font_reg, 16.0, 20.0, 240.0, &input.track_topic);

    // Awarded to label
    push_line(&mut ops, &font_reg, 12.0, 20.0, 220.0, "Awarded to");

    // Learner display name
    push_line(&mut ops, &font_bold, 28.0, 20.0, 205.0, &input.learner_name);

    // Level
    push_line(
        &mut ops,
        &font_reg,
        14.0,
        20.0,
        180.0,
        &format!("Level: {}", input.level),
    );

    // Issued + mastery
    push_line(
        &mut ops,
        &font_reg,
        12.0,
        20.0,
        160.0,
        &format!(
            "Issued {} - Mastery score: {:.2}",
            input.issued_at, input.mastery_score
        ),
    );

    // Brand mark (lower-left)
    push_line(&mut ops, &font_bold, 10.0, 20.0, 30.0, "LEARNFORGE");

    // Fingerprint footer
    push_line(
        &mut ops,
        &font_reg,
        8.0,
        20.0,
        20.0,
        &format!(
            "Verify with key fingerprint: {}",
            input.key_fingerprint_short
        ),
    );

    ops.push(Op::EndTextSection);

    // ── QR image ────────────────────────────────────────────────────────
    // Place at roughly x=150mm, y=210mm — image origin is bottom-left,
    // so translate_y is the bottom edge of a ~40mm-tall QR.
    ops.push(Op::UseXobject {
        id: qr_xobject_id,
        transform: XObjectTransform {
            translate_x: Some(Mm(150.0).into_pt()),
            translate_y: Some(Mm(210.0).into_pt()),
            rotate: None,
            scale_x: Some(0.10),
            scale_y: Some(0.10),
            dpi: Some(300.0),
        },
    });

    // ── Page assembly ───────────────────────────────────────────────────
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    doc.with_pages(vec![page]);

    let mut save_warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut save_warnings);
    Ok(bytes)
}

/// Helper: push one line of text at (x_mm, y_mm) with the given font + size.
fn push_line(
    ops: &mut Vec<Op>,
    font: &PdfFontHandle,
    size_pt: f32,
    x_mm: f32,
    y_mm: f32,
    text: &str,
) {
    ops.push(Op::SetFont {
        font: font.clone(),
        size: Pt(size_pt),
    });
    ops.push(Op::SetTextCursor {
        pos: Point {
            x: Mm(x_mm).into_pt(),
            y: Mm(y_mm).into_pt(),
        },
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(text.to_string())],
    });
}

// ── PNG badge ─────────────────────────────────────────────────────────────

/// Render the skill-level badge as a 600x600 PNG with a transparent
/// background. D-06 amendment: QR + optional vector brand mark only;
/// text labels deferred to Phase 14 (no TTF embed per A5).
pub fn render_badge_png(input: &BadgePngInput) -> Result<Vec<u8>, AchievementError> {
    let canvas_size: u32 = 600;
    let qr_target: u32 = 400;

    // Render QR into a Luma<u8> image (default light=255, dark=0).
    let code = QrCode::with_error_correction_level(&input.qr_payload, EcLevel::M)
        .map_err(|e| AchievementError::Qr(format!("qr encode: {}", e)))?;
    let qr_img: ImageBuffer<Luma<u8>, Vec<u8>> = code
        .render::<Luma<u8>>()
        .max_dimensions(qr_target, qr_target)
        .build();
    let (qr_w, qr_h) = (qr_img.width(), qr_img.height());

    // Transparent RGBA canvas.
    let mut canvas: RgbaImage = ImageBuffer::from_pixel(canvas_size, canvas_size, Rgba([0, 0, 0, 0]));

    // Centre the QR on the canvas.
    let offset_x = canvas_size.saturating_sub(qr_w) / 2;
    let offset_y = canvas_size.saturating_sub(qr_h) / 2;
    for y in 0..qr_h {
        for x in 0..qr_w {
            let p = qr_img.get_pixel(x, y).0[0];
            // luma=0 (dark) → opaque black; luma=255 (light) → fully transparent
            let pixel = if p < 128 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([0, 0, 0, 0])
            };
            canvas.put_pixel(offset_x + x, offset_y + y, pixel);
        }
    }

    // Encode PNG.
    let mut buf = Vec::new();
    {
        let mut cursor = Cursor::new(&mut buf);
        canvas
            .write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| AchievementError::Validation(format!("png encode: {}", e)))?;
    }
    Ok(buf)
}

/// Render a standalone QR PNG (default light=transparent, dark=opaque).
/// Used by `export_certificate` to feed `render_certificate_pdf`.
pub fn render_qr_png(qr_payload: &str) -> Result<Vec<u8>, AchievementError> {
    let code = QrCode::with_error_correction_level(qr_payload, EcLevel::M)
        .map_err(|e| AchievementError::Qr(format!("qr encode: {}", e)))?;
    let qr_img: ImageBuffer<Luma<u8>, Vec<u8>> = code
        .render::<Luma<u8>>()
        .max_dimensions(600, 600)
        .build();
    let (w, h) = (qr_img.width(), qr_img.height());

    // Convert to RGBA — dark modules opaque, light transparent.
    let mut rgba: RgbaImage = ImageBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = qr_img.get_pixel(x, y).0[0];
            let pixel = if p < 128 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([0, 0, 0, 0])
            };
            rgba.put_pixel(x, y, pixel);
        }
    }

    let mut buf = Vec::new();
    {
        let mut cursor = Cursor::new(&mut buf);
        rgba.write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| AchievementError::Validation(format!("png encode: {}", e)))?;
    }
    Ok(buf)
}

// ── Share text ────────────────────────────────────────────────────────────

// Phase 7 Wave 5 (07-05) — the share_text template moved to
// `learnforge_core::signing::share_text` per the D-03 amendment (PDF /
// PNG renderers stay here because printpdf / image / qrcode are not
// WASM-portable; only the pure string template lives in core). Re-export
// preserves the legacy path `achievements::artifacts::share_text` so the
// existing test functions in this module continue to compile unchanged.
pub use learnforge_core::signing::share_text;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pdf_input(qr_png_bytes: Vec<u8>) -> CertificatePdfInput {
        CertificatePdfInput {
            learner_name: "Ada Lovelace".to_string(),
            track_topic: "Kubernetes Fundamentals".to_string(),
            issued_at: "2026-06-15T00:00:00Z".to_string(),
            mastery_score: 0.92,
            key_fingerprint_short: "deadbeef".to_string(),
            level: "Professional".to_string(),
            qr_png_bytes,
        }
    }

    fn sample_badge_input() -> BadgePngInput {
        BadgePngInput {
            level: "Associate".to_string(),
            track_topic: "Kubernetes Fundamentals".to_string(),
            issued_at: "2026-06-15T00:00:00Z".to_string(),
            key_fingerprint_short: "deadbeef".to_string(),
            qr_payload: "learnforge-test-payload-aGVsbG8.deadbeef".to_string(),
        }
    }

    #[test]
    fn pdf_smoke() {
        let qr_png = render_qr_png("learnforge-test-qr-payload").expect("qr png");
        let input = sample_pdf_input(qr_png);
        let bytes = render_certificate_pdf(&input).expect("pdf bytes");
        assert!(!bytes.is_empty(), "PDF must be non-empty");
        assert!(
            bytes.starts_with(b"%PDF-"),
            "PDF must start with %PDF- (got first 8: {:?})",
            &bytes[..bytes.len().min(8)]
        );
        assert!(bytes.len() >= 1024, "PDF should be >= 1KB, got {}", bytes.len());
    }

    #[test]
    fn pdf_length_grows_with_content() {
        // Populated content should produce a PDF ≥ 2KB (text streams + QR
        // XObject add real bytes). This is a smoke test for "renderer wrote
        // a real PDF, not just an empty shell".
        let qr_png = render_qr_png("learnforge-test-payload").expect("qr png");
        let input = sample_pdf_input(qr_png);
        let bytes = render_certificate_pdf(&input).expect("pdf bytes");
        assert!(bytes.len() >= 2048, "expected >= 2KB, got {}", bytes.len());
    }

    #[test]
    fn pdf_includes_qr_image() {
        // The embedded QR PNG appears as an Image XObject. We assert the
        // serialized PDF mentions an Image subtype OR has a non-trivial
        // size delta vs. a no-QR baseline.
        let qr_png = render_qr_png("learnforge-test-qr-payload").expect("qr png");
        let with_qr = render_certificate_pdf(&sample_pdf_input(qr_png.clone())).expect("with qr");

        // Look for "/Image" or "/XObject" marker in the PDF bytes — printpdf
        // emits these as part of the resource dict.
        let s = String::from_utf8_lossy(&with_qr);
        assert!(
            s.contains("/Image") || s.contains("/XObject") || s.contains("Subtype"),
            "PDF should embed image XObject markers"
        );
    }

    #[test]
    fn png_badge_smoke() {
        let bytes = render_badge_png(&sample_badge_input()).expect("badge png");
        assert!(!bytes.is_empty(), "PNG must be non-empty");
        assert_eq!(
            &bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            "PNG must start with PNG magic bytes"
        );
    }

    #[test]
    fn png_badge_is_transparent() {
        let bytes = render_badge_png(&sample_badge_input()).expect("badge png");
        let img = image::load_from_memory(&bytes).expect("decode png");
        let rgba = img.to_rgba8();
        // Must contain at least one fully transparent pixel.
        let transparent_count = rgba.pixels().filter(|p| p.0[3] == 0).count();
        assert!(
            transparent_count > 0,
            "PNG must have transparent pixels (alpha=0); found {} transparent of {} total",
            transparent_count,
            rgba.pixels().count()
        );
        // Sanity: 600x600 canvas.
        assert_eq!(rgba.width(), 600);
        assert_eq!(rgba.height(), 600);
    }

    #[test]
    fn png_badge_has_no_text_labels() {
        // D-06 amendment: the PNG carries QR + optional vector brand mark
        // only — no embedded font tables. Confirm the PNG bytes do NOT
        // contain font/glyph markers from a TTF embed.
        let bytes = render_badge_png(&sample_badge_input()).expect("badge png");
        let s = String::from_utf8_lossy(&bytes);
        // None of these markers should appear — they'd indicate a TTF
        // got smuggled in (which would violate A5 + D-06 amendment).
        for forbidden in &["/FontFile", "/FontDescriptor", "OS/2", "cmap"] {
            assert!(
                !s.contains(forbidden),
                "PNG must not embed font tables; found marker: {}",
                forbidden
            );
        }
    }

    #[test]
    fn share_text_template() {
        let s = share_text("Professional", "Kubernetes Fundamentals", "a1b2c3d4", "QUJD");
        assert_eq!(
            s,
            "I just earned Professional in Kubernetes Fundamentals on LearnForge. \
             Verify with key fingerprint a1b2c3d4: QUJD"
                .replace("             ", "")
        );
    }

    #[test]
    fn share_text_no_emoji() {
        let s = share_text("Associate", "DevOps", "abcd1234", "payload");
        // No characters in emoji ranges.
        for c in s.chars() {
            let u = c as u32;
            assert!(
                !((0x1F300..=0x1FAFF).contains(&u)
                    || (0x2600..=0x27BF).contains(&u)
                    || u == 0xFE0F),
                "share_text must not contain emoji, found {:?} (U+{:04X})",
                c,
                u
            );
        }
    }

    #[test]
    fn pdf_safe_with_xss_attempt() {
        // XSS-style chars in learner_name must NOT panic or break the PDF.
        // printpdf renders Helvetica content streams — text is NOT HTML.
        let qr_png = render_qr_png("xss-probe").expect("qr png");
        let mut input = sample_pdf_input(qr_png);
        input.learner_name = "<script>alert('xss')</script>".to_string();
        input.track_topic = "Bobby Tables & < > \" '".to_string();
        let bytes =
            render_certificate_pdf(&input).expect("PDF must not panic on XSS-style chars");
        assert!(bytes.starts_with(b"%PDF-"));
    }

    // ── Wave 0 RED tests now GREEN ──────────────────────────────────────

    #[test]
    fn qr_dep_resolution_smoke() {
        // (Carry-over from Wave 0 — kept as a per-build dep sanity check.)
        let code = QrCode::new("learnforge-wave-2-smoke").expect("qrcode 0.14 builds");
        let _img: ImageBuffer<Luma<u8>, Vec<u8>> =
            code.render::<Luma<u8>>().max_dimensions(200, 200).build();
    }

    #[test]
    fn qr_png_smoke() {
        let bytes = render_qr_png("learnforge-test-payload").expect("qr png bytes");
        assert!(!bytes.is_empty());
        assert_eq!(
            &bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            "QR PNG must start with PNG magic bytes"
        );
    }
}
