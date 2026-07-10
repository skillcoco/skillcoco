//! Tests for `reports::mod` — Task 2 (ReportStore trait + assemble()).
//! Sibling file to keep `mod.rs` under the 500-line CLAUDE.md cap.
//! Included via `#[path = "_tests2.rs"] #[cfg(test)] mod tests2;` from
//! `mod.rs`. Reuses the exact StubStore/StubKeyStore/pinned_now harness
//! shape from `achievements.rs::tests`.

use super::*;
use crate::signing::verify_payload;
use chrono::TimeZone;
use ed25519_dalek::pkcs8::EncodePublicKey;
use ed25519_dalek::SigningKey;
use pkcs8::LineEnding;
use rand::rngs::OsRng;

// ── Inline stub store ────────────────────────────────────────────────────

/// One canned per-(track, slug) capability record.
struct StubCapability {
    track_id: &'static str,
    #[allow(dead_code)] // documents intent; per-item track_topic is set on EvidenceItem directly
    track_topic: &'static str,
    slug: &'static str,
    label: &'static str,
    knowledge_pct: f64,
    practical_pct: Option<f64>,
    evidence: Vec<EvidenceItem>,
}

struct StubStore {
    capabilities: Vec<StubCapability>,
}

impl ReportStore for StubStore {
    fn capability_tags_for_scope(
        &self,
        scope: &ReportScope,
        _learner_id: &str,
    ) -> Result<Vec<(String, String, String)>, ReportError> {
        let filtered: Vec<(String, String, String)> = match scope {
            ReportScope::Track(track_id) => self
                .capabilities
                .iter()
                .filter(|c| c.track_id == track_id)
                .map(|c| (c.track_id.to_string(), c.slug.to_string(), c.label.to_string()))
                .collect(),
            ReportScope::WholeProfile => self
                .capabilities
                .iter()
                .map(|c| (c.track_id.to_string(), c.slug.to_string(), c.label.to_string()))
                .collect(),
        };
        Ok(filtered)
    }

    fn knowledge_mastery(
        &self,
        track_id: &str,
        slug: &str,
        _learner_id: &str,
    ) -> Result<f64, ReportError> {
        Ok(self
            .capabilities
            .iter()
            .find(|c| c.track_id == track_id && c.slug == slug)
            .map(|c| c.knowledge_pct)
            .unwrap_or(0.0))
    }

    fn practical_mastery(
        &self,
        track_id: &str,
        slug: &str,
        _learner_id: &str,
    ) -> Result<Option<f64>, ReportError> {
        Ok(self
            .capabilities
            .iter()
            .find(|c| c.track_id == track_id && c.slug == slug)
            .and_then(|c| c.practical_pct))
    }

    fn evidence_ledger(
        &self,
        track_id: &str,
        slug: &str,
        _learner_id: &str,
    ) -> Result<Vec<EvidenceItem>, ReportError> {
        Ok(self
            .capabilities
            .iter()
            .find(|c| c.track_id == track_id && c.slug == slug)
            .map(|c| c.evidence.clone())
            .unwrap_or_default())
    }

    fn report_metadata(
        &self,
        _scope: &ReportScope,
        _learner_id: &str,
    ) -> Result<ReportMetadata, ReportError> {
        Ok(ReportMetadata {
            generated_at: String::new(), // overwritten by assemble() from `now`
            app_version: String::new(),  // overwritten by assemble()
            pack_provenance: None,
            verified_issuer: None,
        })
    }
}

struct StubKeyStore {
    key: SigningKey,
}

impl StubKeyStore {
    fn fresh() -> Self {
        Self {
            key: SigningKey::generate(&mut OsRng),
        }
    }
}

impl SigningKeyStore for StubKeyStore {
    fn get_or_init(&self) -> Result<SigningKey, SigningError> {
        Ok(SigningKey::from_bytes(&self.key.to_bytes()))
    }
    fn export_public_pem(&self) -> Result<String, SigningError> {
        self.key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| SigningError::KeyEncoding(e.to_string()))
    }
}

fn pinned_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap()
}

fn single_track_store() -> StubStore {
    StubStore {
        capabilities: vec![
            StubCapability {
                track_id: "trk1",
                track_topic: "Kubernetes",
                slug: "can configure rbac policies",
                label: "Can configure RBAC policies",
                knowledge_pct: 0.9,
                practical_pct: Some(0.7),
                evidence: vec![EvidenceItem {
                    class: EvidenceClass::Quiz,
                    label: "Quiz: RBAC basics".to_string(),
                    detail: "9/10".to_string(),
                    date: "2026-06-01T00:00:00+00:00".to_string(),
                    track_id: Some("trk1".to_string()),
                    track_topic: Some("Kubernetes".to_string()),
                }],
            },
            StubCapability {
                track_id: "trk1",
                track_topic: "Kubernetes",
                slug: "can debug pod networking",
                label: "Can debug pod networking",
                knowledge_pct: 0.5,
                practical_pct: None, // no-lab capability — "not assessed"
                evidence: vec![],
            },
        ],
    }
}

fn whole_profile_store() -> StubStore {
    StubStore {
        capabilities: vec![
            // Same capability tag (casing variant) contributed by two tracks.
            StubCapability {
                track_id: "trk1",
                track_topic: "Kubernetes",
                slug: "Can Configure RBAC Policies",
                label: "Can configure RBAC policies",
                knowledge_pct: 0.6,
                practical_pct: Some(0.5),
                evidence: vec![EvidenceItem {
                    class: EvidenceClass::Quiz,
                    label: "Quiz: RBAC (k8s track)".to_string(),
                    detail: "6/10".to_string(),
                    date: "2026-06-01T00:00:00+00:00".to_string(),
                    track_id: Some("trk1".to_string()),
                    track_topic: Some("Kubernetes".to_string()),
                }],
            },
            StubCapability {
                track_id: "trk2",
                track_topic: "DevOps Fundamentals",
                slug: "can-configure-rbac-policies",
                label: "Can configure RBAC policies",
                knowledge_pct: 0.9, // higher — best-evidence-wins
                practical_pct: Some(0.8),
                evidence: vec![EvidenceItem {
                    class: EvidenceClass::Lab,
                    label: "Lab: RBAC lockdown".to_string(),
                    detail: "5/5 steps".to_string(),
                    date: "2026-06-05T00:00:00+00:00".to_string(),
                    track_id: Some("trk2".to_string()),
                    track_topic: Some("DevOps Fundamentals".to_string()),
                }],
            },
            // Capability unique to trk2 — should NOT merge with anything.
            StubCapability {
                track_id: "trk2",
                track_topic: "DevOps Fundamentals",
                slug: "can write terraform modules",
                label: "Can write Terraform modules",
                knowledge_pct: 0.4,
                practical_pct: None,
                evidence: vec![],
            },
        ],
    }
}

// ── Behaviour tests ──────────────────────────────────────────────────────

#[test]
fn assemble_track_scope_one_row_per_tag_with_single_contributing_track() {
    let store = single_track_store();
    let keys = StubKeyStore::fresh();
    let envelope = assemble(
        &store,
        &keys,
        ReportScope::Track("trk1".to_string()),
        "lp1",
        pinned_now(),
    )
    .expect("assemble");

    assert_eq!(envelope.payload.capabilities.len(), 2);
    for row in &envelope.payload.capabilities {
        assert_eq!(row.contributing_tracks, vec!["trk1".to_string()]);
    }
}

#[test]
fn assemble_reports_not_assessed_for_no_lab_capability() {
    let store = single_track_store();
    let keys = StubKeyStore::fresh();
    let envelope = assemble(
        &store,
        &keys,
        ReportScope::Track("trk1".to_string()),
        "lp1",
        pinned_now(),
    )
    .expect("assemble");

    let row = envelope
        .payload
        .capabilities
        .iter()
        .find(|r| r.slug == normalize_tag("can debug pod networking"))
        .expect("row present");
    assert!(row.practical.is_none(), "no-lab capability must be None (not assessed)");
}

#[test]
fn assemble_envelope_is_report_envelope_v1_shape() {
    let store = single_track_store();
    let keys = StubKeyStore::fresh();
    let envelope = assemble(
        &store,
        &keys,
        ReportScope::Track("trk1".to_string()),
        "lp1",
        pinned_now(),
    )
    .expect("assemble");
    assert_eq!(envelope.key_fingerprint.len(), 8);
    assert!(!envelope.signature_hex.is_empty());
}

#[test]
fn assemble_signature_round_trips() {
    let store = single_track_store();
    let keys = StubKeyStore::fresh();
    let envelope = assemble(
        &store,
        &keys,
        ReportScope::Track("trk1".to_string()),
        "lp1",
        pinned_now(),
    )
    .expect("assemble");

    let pub_pem = keys.export_public_pem().expect("public pem");
    let canonical = canonical_json_bytes(&envelope.payload).expect("canonical bytes");
    assert!(verify_payload(&pub_pem, &canonical, &envelope.signature_hex));
}

#[test]
fn assemble_byte_stable_under_pinned_clock() {
    let key = SigningKey::generate(&mut OsRng);
    let keys = StubKeyStore { key: SigningKey::from_bytes(&key.to_bytes()) };

    let store_a = single_track_store();
    let store_b = single_track_store();

    let envelope_a = assemble(
        &store_a,
        &keys,
        ReportScope::Track("trk1".to_string()),
        "lp1",
        pinned_now(),
    )
    .expect("a");
    let envelope_b = assemble(
        &store_b,
        &keys,
        ReportScope::Track("trk1".to_string()),
        "lp1",
        pinned_now(),
    )
    .expect("b");

    let canonical_a = canonical_json_bytes(&envelope_a.payload).expect("canonical a");
    let canonical_b = canonical_json_bytes(&envelope_b.payload).expect("canonical b");
    assert_eq!(canonical_a, canonical_b, "byte-stable under pinned clock");
    assert_eq!(envelope_a.signature_hex, envelope_b.signature_hex);
}

#[test]
fn assemble_tampering_after_sign_fails_verify() {
    let store = single_track_store();
    let keys = StubKeyStore::fresh();
    let envelope = assemble(
        &store,
        &keys,
        ReportScope::Track("trk1".to_string()),
        "lp1",
        pinned_now(),
    )
    .expect("assemble");

    let pub_pem = keys.export_public_pem().expect("public pem");
    let mut tampered_payload = envelope.payload.clone();
    if let Some(row) = tampered_payload.capabilities.first_mut() {
        row.knowledge.pct = 1.0; // score inflation attempt (T-18-04)
    }
    let tampered_canonical = canonical_json_bytes(&tampered_payload).expect("canonical bytes");
    assert!(
        !verify_payload(&pub_pem, &tampered_canonical, &envelope.signature_hex),
        "mutated pct must invalidate signature"
    );
}

#[test]
fn assemble_whole_profile_merges_shared_capability_with_attribution() {
    let store = whole_profile_store();
    let keys = StubKeyStore::fresh();
    let envelope = assemble(&store, &keys, ReportScope::WholeProfile, "lp1", pinned_now())
        .expect("assemble");

    // 2 distinct slugs after normalization: the shared rbac tag (merged)
    // and the trk2-only terraform tag.
    assert_eq!(envelope.payload.capabilities.len(), 2);

    let merged = envelope
        .payload
        .capabilities
        .iter()
        .find(|r| r.slug == normalize_tag("can configure rbac policies"))
        .expect("merged row present");
    assert_eq!(merged.contributing_tracks.len(), 2, "both tracks contributed");
    assert!(merged.contributing_tracks.contains(&"trk1".to_string()));
    assert!(merged.contributing_tracks.contains(&"trk2".to_string()));
    // Best-evidence-wins: trk2's higher knowledge pct (0.9) is kept.
    assert_eq!(merged.knowledge.pct, 0.9);
    assert_eq!(merged.knowledge.band, bands::band_for(0.9));
    // Evidence from both tracks retained with attribution.
    assert_eq!(merged.evidence.len(), 2);
    assert!(merged.evidence.iter().any(|e| e.track_id.as_deref() == Some("trk1")));
    assert!(merged.evidence.iter().any(|e| e.track_id.as_deref() == Some("trk2")));

    let unique = envelope
        .payload
        .capabilities
        .iter()
        .find(|r| r.slug == normalize_tag("can write terraform modules"))
        .expect("unique row present");
    assert_eq!(unique.contributing_tracks, vec!["trk2".to_string()]);
}

#[test]
fn no_utc_now_inline_clock_is_injected() {
    // Compile-time/behavioral proxy: calling assemble twice with two
    // DIFFERENT pinned clocks must produce different generated_at values
    // that match each call's `now`, proving the clock is threaded through
    // rather than read from the environment.
    let store = single_track_store();
    let keys = StubKeyStore::fresh();
    let now_a = pinned_now();
    let now_b = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap();

    let envelope_a = assemble(&store, &keys, ReportScope::Track("trk1".to_string()), "lp1", now_a)
        .expect("a");
    let envelope_b = assemble(&store, &keys, ReportScope::Track("trk1".to_string()), "lp1", now_b)
        .expect("b");

    assert_eq!(envelope_a.payload.metadata.generated_at, now_a.to_rfc3339());
    assert_eq!(envelope_b.payload.metadata.generated_at, now_b.to_rfc3339());
    assert_ne!(envelope_a.payload.metadata.generated_at, envelope_b.payload.metadata.generated_at);
}

#[test]
fn report_store_is_object_safe() {
    let store = single_track_store();
    let _dyn_store: &dyn ReportStore = &store;
}
