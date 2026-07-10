//! Tests for `reports::mod`. Lives in a sibling file to keep `mod.rs`
//! under the 500-line CLAUDE.md cap. Included via
//! `#[path = "_tests.rs"] #[cfg(test)] mod tests;` from `mod.rs`.
//!
//! Task 1 covers `normalize_tag` dedup + camelCase serde shape. Task 2
//! (assemble/ReportStore) tests are added in the sibling `_tests2.rs`
//! file to keep this file focused and under the line cap; see
//! `reports/_tests2.rs`.

use super::*;

// ── normalize_tag (Pitfall 4 dedup) ────────────────────────────────────

#[test]
fn normalize_tag_dedups_casing_and_whitespace_variants() {
    let a = normalize_tag("Can Configure RBAC Policies");
    let b = normalize_tag("can configure rbac policies");
    let c = normalize_tag("  can-configure  RBAC   policies ");
    assert_eq!(a, b, "casing must dedupe");
    assert_eq!(b, c, "whitespace/hyphen runs must collapse and dedupe");
}

#[test]
fn normalize_tag_produces_stable_slug_shape() {
    let slug = normalize_tag("Can Configure RBAC Policies");
    assert_eq!(slug, "can-configure-rbac-policies");
    assert!(!slug.starts_with('-'));
    assert!(!slug.ends_with('-'));
}

#[test]
fn normalize_tag_handles_empty_and_punctuation_only() {
    assert_eq!(normalize_tag(""), "");
    assert_eq!(normalize_tag("   "), "");
    assert_eq!(normalize_tag("---"), "");
}

// ── EvidenceClass camelCase + reserved exam slot (D-07) ─────────────────

#[test]
fn evidence_class_serializes_camel_case_and_reserves_exam() {
    let quiz = serde_json::to_string(&EvidenceClass::Quiz).unwrap();
    let lab = serde_json::to_string(&EvidenceClass::Lab).unwrap();
    let cert = serde_json::to_string(&EvidenceClass::Cert).unwrap();
    let module = serde_json::to_string(&EvidenceClass::Module).unwrap();
    let exam = serde_json::to_string(&EvidenceClass::Exam).unwrap();
    assert_eq!(quiz, "\"quiz\"");
    assert_eq!(lab, "\"lab\"");
    assert_eq!(cert, "\"cert\"");
    assert_eq!(module, "\"module\"");
    assert_eq!(exam, "\"exam\"");
}

// ── EvidenceItem carries D-04 track attribution ─────────────────────────

#[test]
fn evidence_item_serializes_camel_case_with_track_attribution() {
    let item = EvidenceItem {
        class: EvidenceClass::Quiz,
        label: "Quiz: RBAC basics".to_string(),
        detail: "9/10".to_string(),
        date: "2026-06-15T00:00:00+00:00".to_string(),
        track_id: Some("trk1".to_string()),
        track_topic: Some("Kubernetes".to_string()),
    };
    let s = serde_json::to_string(&item).unwrap();
    assert!(s.contains("\"trackId\":\"trk1\""));
    assert!(s.contains("\"trackTopic\":\"Kubernetes\""));
}

// ── CapabilityRow: practical is optional ("not assessed"), track attribution ──

#[test]
fn capability_row_practical_none_serializes_as_null_not_zero() {
    let row = CapabilityRow {
        slug: "can-debug-pods".to_string(),
        label: "Can debug pod networking".to_string(),
        knowledge: MasteryDimension {
            band: "Working".to_string(),
            pct: 0.4,
        },
        practical: None,
        contributing_tracks: vec!["trk1".to_string()],
        evidence: vec![],
    };
    let s = serde_json::to_string(&row).unwrap();
    assert!(s.contains("\"practical\":null"));
    assert!(!s.contains("\"practical\":0"), "must never render 0% for no-lab capability");
}

#[test]
fn capability_row_carries_contributing_tracks() {
    let row = CapabilityRow {
        slug: "can-debug-pods".to_string(),
        label: "Can debug pod networking".to_string(),
        knowledge: MasteryDimension {
            band: "Mastered".to_string(),
            pct: 0.9,
        },
        practical: Some(MasteryDimension {
            band: "Proficient".to_string(),
            pct: 0.7,
        }),
        contributing_tracks: vec!["trk1".to_string(), "trk2".to_string()],
        evidence: vec![],
    };
    let s = serde_json::to_string(&row).unwrap();
    assert!(s.contains("\"contributingTracks\":[\"trk1\",\"trk2\"]"));
}

// ── ReportEnvelopeV1 — the exact export shape ────────────────────────────

#[test]
fn report_envelope_v1_serializes_expected_camel_case_shape() {
    let payload = ReportPayloadV1 {
        learner_name: "Ada".to_string(),
        learner_id: "lp1".to_string(),
        scope_label: "Kubernetes".to_string(),
        capabilities: vec![],
        metadata: ReportMetadata {
            generated_at: "2026-06-15T12:00:00+00:00".to_string(),
            app_version: "0.1.0".to_string(),
            pack_provenance: None,
            verified_issuer: None,
        },
        issuer: None,
        key_fingerprint: "deadbeef".to_string(),
        payload_version: 1,
    };
    let envelope = ReportEnvelopeV1 {
        payload,
        signature_hex: "abcd".to_string(),
        key_fingerprint: "deadbeef".to_string(),
    };
    let s = serde_json::to_string(&envelope).unwrap();
    assert!(s.contains("\"payload\":"));
    assert!(s.contains("\"signatureHex\":\"abcd\""));
    assert!(s.contains("\"keyFingerprint\":\"deadbeef\""));
    assert!(s.contains("\"learnerName\":\"Ada\""));
    assert!(s.contains("\"scopeLabel\":\"Kubernetes\""));
    assert!(s.contains("\"payloadVersion\":1"));
}

#[test]
fn report_scope_variants_construct() {
    let track = ReportScope::Track("trk1".to_string());
    let whole = ReportScope::WholeProfile;
    match track {
        ReportScope::Track(id) => assert_eq!(id, "trk1"),
        ReportScope::WholeProfile => panic!("expected Track"),
    }
    match whole {
        ReportScope::WholeProfile => {}
        ReportScope::Track(_) => panic!("expected WholeProfile"),
    }
}

#[test]
fn report_error_from_signing_error_maps_variants() {
    let e: ReportError = crate::signing::SigningError::InvalidSignature.into();
    assert!(matches!(e, ReportError::Signature(_)));
}

#[test]
fn report_error_from_canonical_json_error_maps_to_validation() {
    let e: ReportError = crate::canonical_json::CanonicalJsonError::NonFiniteFloat.into();
    assert!(matches!(e, ReportError::Validation(_)));
}
