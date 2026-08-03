//! `skillcoco://` deep-link URL parsing (EXP-05 upstream half, D-08).
//!
//! `skillcoco://import?path=<url-encoded absolute path>` → generic pack
//! import via the existing `import_course_impl` — the handler in `lib.rs`
//! calls [`parse_import_deep_link`] and feeds the decoded path straight to
//! the same fail-closed import gate the file picker uses. GENERIC pack
//! import only: nothing here knows or cares what tool exported the pack
//! (zero coupling to any authoring app, D-08 hard constraint).

/// Parses a `skillcoco://import?path=<enc>` URL and returns the decoded
/// filesystem path. Returns `None` for any other scheme/action or a
/// missing/empty/undecodable `path` param — callers log and ignore
/// (a bad deep link must never crash the app).
pub fn parse_import_deep_link(url: &str) -> Option<String> {
    let rest = url.strip_prefix("skillcoco://import")?;

    // Only "" (bare), "/" (trailing slash), or a query string may follow the
    // action — anything else is a different action (e.g. import-foo).
    let query = match rest.strip_prefix('/').unwrap_or(rest) {
        "" => return None, // no query at all → no path param
        q => q.strip_prefix('?')?,
    };

    let encoded = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("path="))
        .filter(|v| !v.is_empty())?;

    percent_decode(encoded).filter(|p| !p.is_empty())
}

/// Minimal percent-decoder for query values. Returns `None` on malformed
/// escapes. A literal '+' passes through unchanged (query percent-encoding,
/// not application/x-www-form-urlencoded).
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            let hex = std::str::from_utf8(hex).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_import_url_with_encoded_path() {
        let url = "skillcoco://import?path=%2FUsers%2Fgshah%2Fpacks%2Fsfd402.pack.json";
        assert_eq!(
            parse_import_deep_link(url),
            Some("/Users/gshah/packs/sfd402.pack.json".to_string())
        );
    }

    #[test]
    fn decodes_spaces_and_plus_signs_correctly() {
        // %20 decodes to a space; a literal '+' in a path stays '+' (query
        // percent-encoding, not form-encoding).
        let url = "skillcoco://import?path=%2Ftmp%2Fmy%20course%2Bv2.pack.json";
        assert_eq!(
            parse_import_deep_link(url),
            Some("/tmp/my course+v2.pack.json".to_string())
        );
    }

    #[test]
    fn rejects_wrong_action() {
        assert_eq!(parse_import_deep_link("skillcoco://export?path=%2Ftmp%2Fx"), None);
    }

    #[test]
    fn rejects_wrong_scheme() {
        assert_eq!(
            parse_import_deep_link("learnforge://import?path=%2Ftmp%2Fx"),
            None
        );
    }

    #[test]
    fn rejects_missing_or_empty_path_param() {
        assert_eq!(parse_import_deep_link("skillcoco://import"), None);
        assert_eq!(parse_import_deep_link("skillcoco://import?path="), None);
        assert_eq!(parse_import_deep_link("skillcoco://import?other=x"), None);
    }

    #[test]
    fn tolerates_extra_query_params_and_slash_after_action() {
        let url = "skillcoco://import/?source=studio&path=%2Ftmp%2Fp.json&x=1";
        assert_eq!(parse_import_deep_link(url), Some("/tmp/p.json".to_string()));
    }

    #[test]
    fn rejects_malformed_percent_encoding() {
        assert_eq!(parse_import_deep_link("skillcoco://import?path=%2"), None);
        assert_eq!(parse_import_deep_link("skillcoco://import?path=%ZZ"), None);
    }
}
