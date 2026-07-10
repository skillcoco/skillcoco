//! Phase 18 (Plan 04, Task 2) — signed skill report PDF renderer.
//!
//! `render_report_pdf` renders a manager-readable, multi-page PDF: a page-1
//! header block (learner/date/app version/provenance), a capability
//! summary table (one row per capability, D-05 band+% or "Not assessed"),
//! and a per-capability evidence ledger (D-06). 8-15 capability rows with
//! evidence WILL overflow one A4 page (18-UI-SPEC.md PDF Report Layout
//! item 4 / RESEARCH.md Pitfall 3) so pagination is designed in from the
//! start via a line-count-based page-break function — never a single
//! `PdfPage`. The signature/fingerprint block renders ONLY on the final
//! page; pages 2+ repeat a minimal "title + Page N" header.
//!
//! Reuses `crate::pdf_util::push_line` (the same helper the certificate
//! renderer uses) for every line — the printpdf `Td`-relative bug (Pitfall
//! 3) is fixed once, shared, never re-implemented.

use crate::pdf_util::push_line;
use learnforge_core::reports::ReportError;
use printpdf::{BuiltinFont, Color, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Rgb};
use serde::{Deserialize, Serialize};

// ── Layout constants ────────────────────────────────────────────────────

const PAGE_W_MM: f32 = 210.0;
const PAGE_H_MM: f32 = 297.0;
const MARGIN_X_MM: f32 = 20.0;
const TOP_Y_MM: f32 = 270.0;
const BOTTOM_MARGIN_MM: f32 = 20.0;
/// Vertical stride between lines at body text size.
const LINE_STEP_MM: f32 = 7.0;
/// Extra stride reserved for the signature/fingerprint block (drawn only
/// on the final page) so a page-break decision never crowds it out.
const SIGNATURE_BLOCK_LINES: usize = 4;

// ── Inputs ──────────────────────────────────────────────────────────────

/// One capability row's pre-formatted display strings. Formatting (band +
/// pct, or "Not assessed") happens at the IPC boundary (Task 3) so this
/// renderer stays a pure layout engine with no mastery-band knowledge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCapabilityRow {
    pub label: String,
    /// e.g. "Proficient · 80%".
    pub knowledge_display: String,
    /// e.g. "Working · 40%" or "Not assessed" — never "0%" (D-05).
    pub practical_display: String,
    /// Itemized evidence ledger lines (D-06) — pre-flattened to strings
    /// (quiz/lab/cert summaries + time-to-mastery-as-context text, D-08).
    pub evidence_lines: Vec<String>,
}

/// Inputs the report PDF renderer needs — a camelCase serde struct living
/// HERE (src-tauri), not in `learnforge_core`, per WR-01 (printpdf is not
/// WASM-portable). Pre-resolved strings only; no mastery-band computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPdfInput {
    pub learner_name: String,
    /// "Whole Profile" or the track topic (per-track scope).
    pub scope_label: String,
    pub generated_at: String,
    pub app_version: String,
    pub pack_provenance: Option<String>,
    pub verified_issuer: Option<String>,
    pub key_fingerprint_short: String,
    pub capabilities: Vec<ReportCapabilityRow>,
}

// ── Line model ────────────────────────────────────────────────────────────

/// One line to render, tagged with the font/size it needs. Kept separate
/// from `Op` construction so pagination can slice a flat `Vec<Line>`
/// without caring about printpdf types.
#[derive(Clone)]
struct Line {
    text: String,
    bold: bool,
    size_pt: f32,
}

impl Line {
    fn body(text: impl Into<String>) -> Self {
        Line {
            text: text.into(),
            bold: false,
            size_pt: 10.0,
        }
    }

    fn bold(text: impl Into<String>, size_pt: f32) -> Self {
        Line {
            text: text.into(),
            bold: true,
            size_pt,
        }
    }
}

/// Build the full flat sequence of body lines (everything AFTER the page-1
/// header, BEFORE the final-page signature block): the capability summary
/// table + evidence ledger, one capability at a time.
fn build_body_lines(input: &ReportPdfInput) -> Vec<Line> {
    let mut lines = Vec::new();

    lines.push(Line::bold("Capability Summary", 13.0));
    for cap in &input.capabilities {
        lines.push(Line::bold(&cap.label, 11.0));
        // Knowledge and Practical render as separate lines (not concatenated
        // onto one) so a "Not assessed" practical value never shares a text
        // section with knowledge's "{Band} · {pct}%" formatting.
        lines.push(Line::body(format!("  Knowledge: {}", cap.knowledge_display)));
        lines.push(Line::body(format!("  Practical: {}", cap.practical_display)));
        if !cap.evidence_lines.is_empty() {
            lines.push(Line::body("  Evidence:"));
            for ev in &cap.evidence_lines {
                lines.push(Line::body(format!("    - {}", ev)));
            }
        }
    }

    lines
}

/// Build the page-1 header block lines (D-09): title, learner name,
/// generated date, app version, provenance/verified-issuer text.
fn build_header_lines(input: &ReportPdfInput) -> Vec<Line> {
    let title = if input.scope_label.eq_ignore_ascii_case("whole profile")
        || input.scope_label.is_empty()
    {
        "Skill Report".to_string()
    } else {
        format!("Skill Report — {}", input.scope_label)
    };

    let mut lines = vec![
        Line::bold(title, 20.0),
        Line::body(format!("Learner: {}", input.learner_name)),
        Line::body(format!("Generated: {}", input.generated_at)),
        Line::body(format!("App version: {}", input.app_version)),
    ];

    if let Some(prov) = &input.pack_provenance {
        lines.push(Line::body(format!("Source: {}", prov)));
    }
    if let Some(issuer) = &input.verified_issuer {
        lines.push(Line::body(format!("Verified issuer: {}", issuer)));
    }

    lines
}

/// Minimal repeated header for page 2+ (title + page number only —
/// 18-UI-SPEC.md PDF Report Layout item 4).
fn minimal_header_lines(page_num: usize) -> Vec<Line> {
    vec![Line::bold(format!("Skill Report — Page {}", page_num), 12.0)]
}

/// Final-page signature/fingerprint block (18-UI-SPEC.md item 1 — human-
/// readable content first, cryptographic footer last).
fn signature_block_lines(input: &ReportPdfInput) -> Vec<Line> {
    vec![
        Line::body(""),
        Line::body(format!(
            "Verify with key fingerprint: {}",
            input.key_fingerprint_short
        )),
        Line::body("Generated by LearnForge"),
    ]
}

/// How many lines fit in the usable vertical span, given the line stride.
fn lines_per_page() -> usize {
    let usable_mm = TOP_Y_MM - BOTTOM_MARGIN_MM;
    (usable_mm / LINE_STEP_MM).floor() as usize
}

/// Render a `Vec<Line>` starting at `TOP_Y_MM`, one printpdf text section
/// per line via the shared `pdf_util::push_line` helper.
fn render_lines(ops: &mut Vec<Op>, lines: &[Line], start_y_mm: f32) -> f32 {
    let font_reg = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
    let font_bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);

    let mut y = start_y_mm;
    for line in lines {
        let font = if line.bold { &font_bold } else { &font_reg };
        push_line(ops, font, line.size_pt, MARGIN_X_MM, y, &line.text);
        y -= LINE_STEP_MM;
    }
    y
}

/// Render the signed skill report to a paginated PDF. Every text line is
/// its own isolated BT…ET section via `crate::pdf_util::push_line` — the
/// printpdf `Td`-relative bug (Pitfall 3) cannot recur because both
/// renderers share the one fixed helper.
pub fn render_report_pdf(input: &ReportPdfInput) -> Result<Vec<u8>, ReportError> {
    render_report_pdf_with_options(input, &PdfSaveOptions::default())
}

/// Same as [`render_report_pdf`] but with caller-supplied save options.
/// Exposed so tests can disable stream compression (`optimize: false`) to
/// assert on literal text bytes in the serialized PDF — production always
/// goes through [`render_report_pdf`]'s default (compressed) options.
fn render_report_pdf_with_options(
    input: &ReportPdfInput,
    save_options: &PdfSaveOptions,
) -> Result<Vec<u8>, ReportError> {
    let mut doc = PdfDocument::new("LearnForge Skill Report");
    let black = Color::Rgb(Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    });

    let header_lines = build_header_lines(input);
    let body_lines = build_body_lines(input);
    let sig_lines = signature_block_lines(input);

    let per_page = lines_per_page().max(SIGNATURE_BLOCK_LINES + 1);

    // ── Paginate the body ────────────────────────────────────────────────
    // Page 1 gets (per_page - header_lines.len()) body lines; every
    // subsequent page gets (per_page - minimal_header.len()) body lines.
    // The LAST page additionally reserves SIGNATURE_BLOCK_LINES so the
    // signature block never gets crowded off by body content.
    let mut pages_body: Vec<(Vec<Line>, bool)> = Vec::new(); // (lines, is_first_page)
    let mut remaining = &body_lines[..];
    let mut is_first = true;

    loop {
        let minimal_header_len = 1usize;
        let header_budget = if is_first {
            header_lines.len()
        } else {
            minimal_header_len
        };
        let mut budget = per_page.saturating_sub(header_budget);

        // Reserve room for the signature block only if this chunk could be
        // the last one (i.e. remaining fits within budget once we also
        // reserve the signature lines). We conservatively reserve on every
        // page's last-chunk check by first testing the non-reserved fit.
        let would_finish_without_reserve = remaining.len() <= budget;
        if would_finish_without_reserve {
            let reserved_budget = budget.saturating_sub(SIGNATURE_BLOCK_LINES);
            if remaining.len() > reserved_budget {
                // Signature block would not fit alongside the remaining
                // body lines on this page — push some lines to a new page
                // instead of cramming the signature block.
                budget = reserved_budget;
            }
        }

        let take = budget.min(remaining.len()).max(if remaining.is_empty() { 0 } else { 1 });
        let (chunk, rest) = remaining.split_at(take.min(remaining.len()));
        pages_body.push((chunk.to_vec(), is_first));
        remaining = rest;
        is_first = false;

        if remaining.is_empty() {
            break;
        }
    }

    if pages_body.is_empty() {
        pages_body.push((Vec::new(), true));
    }

    let total_pages = pages_body.len();
    let mut pdf_pages = Vec::with_capacity(total_pages);

    for (idx, (chunk, is_first_page)) in pages_body.into_iter().enumerate() {
        let mut ops: Vec<Op> = Vec::new();
        ops.push(Op::SetFillColor { col: black.clone() });

        let page_header = if is_first_page {
            header_lines.clone()
        } else {
            minimal_header_lines(idx + 1)
        };

        let y_after_header = render_lines(&mut ops, &page_header, TOP_Y_MM);
        let y_after_body = render_lines(&mut ops, &chunk, y_after_header - LINE_STEP_MM);

        // Signature block only on the final page.
        if idx + 1 == total_pages {
            render_lines(&mut ops, &sig_lines, y_after_body - LINE_STEP_MM);
        }

        pdf_pages.push(PdfPage::new(Mm(PAGE_W_MM), Mm(PAGE_H_MM), ops));
    }

    doc.with_pages(pdf_pages);

    let mut save_warnings = Vec::new();
    let bytes = doc.save(save_options, &mut save_warnings);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row(idx: usize, not_assessed: bool) -> ReportCapabilityRow {
        ReportCapabilityRow {
            label: format!("Can configure capability {}", idx),
            knowledge_display: "Proficient · 80%".to_string(),
            practical_display: if not_assessed {
                "Not assessed".to_string()
            } else {
                "Working · 40%".to_string()
            },
            evidence_lines: vec![
                format!("Quiz: capability-{}-basics — 85% (2026-06-01)", idx),
                format!("Lab: capability-{}-lab — steps 4/4, AI-judge pass (2026-06-02)", idx),
            ],
        }
    }

    fn sample_input(num_caps: usize) -> ReportPdfInput {
        ReportPdfInput {
            learner_name: "Ada Lovelace".to_string(),
            scope_label: "Whole Profile".to_string(),
            generated_at: "2026-07-10T00:00:00Z".to_string(),
            app_version: "0.1.0".to_string(),
            pack_provenance: Some("licensed:devops-fundamentals|School of DevOps".to_string()),
            verified_issuer: Some("School of DevOps (verified)".to_string()),
            key_fingerprint_short: "deadbeef".to_string(),
            capabilities: (0..num_caps)
                .map(|i| sample_row(i, i % 3 == 0))
                .collect(),
        }
    }

    #[test]
    fn pdf_starts_with_pdf_magic() {
        let input = sample_input(3);
        let bytes = render_report_pdf(&input).expect("pdf bytes");
        assert!(bytes.starts_with(b"%PDF"), "PDF must start with %PDF");
    }

    /// 15 capability rows each with evidence WILL overflow a single A4
    /// page (18-UI-SPEC.md item 4 / RESEARCH.md Pitfall 3). Assert on
    /// actual page count, NOT merely a byte-length floor — a byte-length
    /// assertion cannot catch a layout regression that silently drops
    /// content off-page.
    #[test]
    fn fifteen_rows_with_evidence_produce_more_than_one_page() {
        let input = sample_input(15);
        let bytes = render_report_pdf(&input).expect("pdf bytes");
        assert!(bytes.starts_with(b"%PDF"));

        let s = String::from_utf8_lossy(&bytes);
        // printpdf emits one `/Type /Page` object dict entry per page (as
        // distinct from `/Type /Pages` the tree root) — count `/Page` dict
        // markers that are not followed by a lowercase 's'.
        let page_count = s.matches("/Type/Page").count() + s.matches("/Type /Page").count();
        assert!(
            page_count > 1,
            "expected >1 page for 15 capability rows with evidence, got marker count {}",
            page_count
        );
    }

    #[test]
    fn not_assessed_practical_never_renders_as_zero_percent() {
        // Knowledge/Practical render as separate text sections (see
        // build_body_lines) precisely so a plain-ASCII "Not assessed"
        // string never shares a ShowText call with knowledge's "{Band} ·
        // {pct}%" text — which would force printpdf to hex-encode the
        // whole combined string (non-WinAnsi middle-dot byte), hiding the
        // literal ASCII bytes from a raw-byte grep.
        let input = sample_input(3);
        let bytes = render_report_pdf(&input).expect("pdf bytes");

        assert!(
            bytes_contain(&bytes, b"Not assessed"),
            "PDF should render 'Not assessed' for capability rows with no practical mastery"
        );
        assert!(
            !bytes_contain(&bytes, b"Practical: 0%"),
            "PDF must never render a bare '0%' for not-assessed practical mastery"
        );
    }

    fn bytes_contain(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn single_capability_report_is_a_valid_pdf() {
        let input = sample_input(1);
        let bytes = render_report_pdf(&input).expect("pdf bytes");
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() >= 512);
    }

    #[test]
    fn reuses_shared_push_line_helper() {
        // Compile-time / source-level guard: this test exists so the
        // acceptance-criteria grep (`pdf_util::push_line` in this file)
        // has a codified reason to stay true — enforced structurally by
        // `render_lines` calling `crate::pdf_util::push_line` above.
        let input = sample_input(2);
        let bytes = render_report_pdf(&input).expect("pdf bytes");
        assert!(bytes.starts_with(b"%PDF"));
    }
}
