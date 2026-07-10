//! Shared PDF text-rendering helper for BOTH the certificate renderer
//! (`achievements::artifacts`) and the report renderer
//! (`reports::artifacts`).
//!
//! Extracted from `achievements/artifacts.rs` (Phase 6) during Phase 18
//! Plan 4 so the hard-won `push_line` fix — and its regression test — is
//! shared instead of re-implemented (and potentially re-broken) by the
//! new report PDF renderer.

use printpdf::{Op, PdfFontHandle, Point, Pt, TextItem};
use printpdf::Mm;

/// Push one line of text at (x_mm, y_mm) with the given font + size.
///
/// Each line is its own text object (BT…ET). printpdf's `SetTextCursor`
/// emits PDF `Td`, which positions RELATIVE to the previous text line.
/// Sharing one text section made every line after the first accumulate the
/// prior offsets and fly off the page — only the title rendered (Phase 06
/// UAT regression). Wrapping each line in its own section resets the text
/// matrix so the cursor is absolute from the page origin.
/// Transliterate to plain ASCII before handing text to printpdf's builtin
/// fonts. printpdf writes builtin-font text as raw bytes, so any non-ASCII
/// char (em dash, the D-05 "·" separator, unicode in pack-authored module
/// titles) renders as mojibake ("â€”", "Â·") in the saved PDF. Common
/// typographic characters get readable ASCII stand-ins; anything else
/// becomes '?'.
fn winansi_safe(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{2010}'..='\u{2015}' | '\u{2212}' => out.push('-'), // hyphens/dashes, minus
            '\u{00B7}' | '\u{2022}' | '\u{2027}' => out.push('-'), // middle dot, bullets
            '\u{2018}' | '\u{2019}' | '\u{201A}' => out.push('\''), // single quotes
            '\u{201C}' | '\u{201D}' | '\u{201E}' => out.push('"'), // double quotes
            '\u{2026}' => out.push_str("..."),                     // ellipsis
            '\u{00A0}' => out.push(' '),                           // nbsp
            c if c.is_ascii() => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

pub(crate) fn push_line(
    ops: &mut Vec<Op>,
    font: &PdfFontHandle,
    size_pt: f32,
    x_mm: f32,
    y_mm: f32,
    text: &str,
) {
    let text = winansi_safe(text);
    ops.push(Op::StartTextSection);
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
        items: vec![TextItem::Text(text)],
    });
    ops.push(Op::EndTextSection);
}

#[cfg(test)]
mod tests {
    use super::*;
    use printpdf::BuiltinFont;

    /// Regression (Phase 18 UAT): builtin-font text is written as raw bytes,
    /// so any non-ASCII char (em dash in the report title, the D-05 "·"
    /// separator, unicode in pack-authored module titles) renders as
    /// mojibake ("â€”", "Â·") in the saved PDF. push_line must transliterate
    /// to ASCII before emitting ShowText.
    #[test]
    fn push_line_transliterates_non_ascii_to_winansi_safe_ascii() {
        let mut ops: Vec<Op> = Vec::new();
        let font = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
        push_line(
            &mut ops,
            &font,
            12.0,
            20.0,
            100.0,
            "Skill Report — Novice · 80% “quoted” it’s… café",
        );

        let text = ops
            .iter()
            .find_map(|o| match o {
                Op::ShowText { items } => match items.first() {
                    Some(TextItem::Text(t)) => Some(t.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("push_line must emit a ShowText op");

        assert!(
            text.is_ascii(),
            "rendered text must be pure ASCII, got: {text:?}"
        );
        assert_eq!(text, "Skill Report - Novice - 80% \"quoted\" it's... caf?");
    }

    /// Regression (Phase 06 UAT): the certificate rendered only its title —
    /// every other line flew off the page. printpdf's SetTextCursor emits PDF
    /// `Td` (relative to the previous text line), so sharing one text section
    /// accumulated offsets. Each line must be its own BT…ET text object so the
    /// cursor is absolute.
    #[test]
    fn push_line_emits_self_contained_text_object() {
        let mut ops: Vec<Op> = Vec::new();
        let font = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
        push_line(&mut ops, &font, 12.0, 20.0, 100.0, "hello");

        assert!(
            matches!(ops.first(), Some(Op::StartTextSection)),
            "line must open its own text section"
        );
        assert!(
            matches!(ops.last(), Some(Op::EndTextSection)),
            "line must close its own text section"
        );
        let starts = ops
            .iter()
            .filter(|o| matches!(o, Op::StartTextSection))
            .count();
        let ends = ops
            .iter()
            .filter(|o| matches!(o, Op::EndTextSection))
            .count();
        assert_eq!(starts, 1, "exactly one StartTextSection per line");
        assert_eq!(ends, 1, "exactly one EndTextSection per line");
    }
}
