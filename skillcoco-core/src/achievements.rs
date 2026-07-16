//! Achievement issuance algorithm — moved from
//! `src-tauri/src/achievements/mod.rs` during Phase 7 Wave 8 (07-08).
//!
//! This is the **final** algorithmic move wave. After Wave 8, every
//! algorithm in the SkillCoco learning loop lives in `skillcoco-core`
//! and the WASM build target compiles cleanly without `cargo` ever
//! seeing a `rusqlite` / `printpdf` / `image` / `qrcode` / `tauri` line
//! in the dependency graph.
//!
//! ## What lives here
//!
//! - Data structs that cross IPC (`Achievement`, `CertPayloadV1`,
//!   `TrackCertifications`, `IssuanceContext`) — `#[serde(rename_all =
//!   "camelCase")]` preserved because the frontend round-trips them.
//! - The [`AchievementStore`] trait (A3 lock — per-module storage
//!   location). The rusqlite-backed impl lives in
//!   `src-tauri/src/storage_impl/achievements.rs`.
//!
//! **Note (WR-01):** PDF / PNG renderer input shapes
//! (`CertificatePdfInput`, `BadgePngInput`) intentionally do NOT live
//! here. They sit next to the renderers in
//! `src-tauri/src/achievements/artifacts.rs` because the renderers stay
//! in `src-tauri` per D-03 amendment + R-7 (printpdf / qrcode / image
//! are not WASM-portable). Phase 7 review found the previous core copies
//! had zero external callers — they were dead code freezing public API
//! at 0.1.0 and were removed in the review-fix pass.
//! - The [`maybe_issue`] free function — generic over `<S:
//!   AchievementStore, K: SigningKeyStore>` plus an explicit
//!   `now: chrono::DateTime<chrono::Utc>` parameter (A5 clock injection;
//!   Phase 6 R3 mitigation lifted to the algorithm signature). Body
//!   lifted from pre-Wave-8 `src-tauri/src/achievements/mod.rs:213-307`
//!   with three call-shape rewrites:
//!   - `conn.query_row(...)` SQL calls → `store.<method>(...)` trait calls
//!   - `get_or_load_into_mutex(...)` → `key_store.get_or_init()?`
//!   - `chrono::Utc::now()` → `now` (injected parameter)
//!
//! ## What does NOT live here
//!
//! - `From<rusqlite::Error> for AchievementError` — lives in src-tauri
//!   so the trait surface stays pure.
//! - PDF / PNG / QR rendering — `src-tauri/src/achievements/artifacts.rs`
//!   is intentionally untouched (D-03 amendment + R-7).
//! - FS-backed key loading — `FsKeyStore` lives in
//!   `src-tauri/src/storage_impl/signing.rs` (Wave 5).
//! - The `track_mastery_aggregate` SQL body — lives in the rusqlite
//!   impl of [`AchievementStore::track_mastery_aggregate`] which
//!   delegates to the Wave 4 parked free fn in
//!   `src-tauri/src/storage_impl/threshold.rs`. Wave 8 closes the
//!   Wave-4 forward-declared seam.
//!
//! ## A5 clock injection — why
//!
//! Pre-Wave-8, `maybe_issue` called `chrono::Utc::now()` internally so
//! every issuance carried a different `issued_at`. That worked but made
//! signature byte-stability tests painful (the canonical bytes embed
//! `completion_date`). Wave 8 injects `now` at the call site:
//!
//! - Tests pass a pinned `DateTime<Utc>` so canonical payload bytes are
//!   reproducible across runs.
//! - Production callers (the src-tauri shim's legacy wrapper) pass
//!   `Utc::now()` exactly where they used to.
//!
//! No behavior change in production; deterministic tests in core.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

use crate::canonical_json::{canonical_json_bytes, CanonicalJsonError};
use crate::signing::{public_key_fingerprint, sign_payload, SigningError, SigningKeyStore};
use crate::threshold::{levels_met, TrackAggregate};

// ── Types ─────────────────────────────────────────────────────────────────

/// Errors raised by the achievement-issuance algorithm.
///
/// `Db` is a stringified envelope: the rusqlite-backed
/// [`AchievementStore`] impl in `src-tauri` constructs it via its own
/// `From<rusqlite::Error>` impl (kept there to honor D-02 — no rusqlite
/// in core). The pure crypto / canonical-JSON variants are covered by
/// `#[from]` conversions.
#[derive(Debug, Error)]
pub enum AchievementError {
    /// Stringified database error from the storage impl side.
    #[error("database error: {0}")]
    Db(String),

    /// I/O error (populated only by the FS-backed key store on the
    /// src-tauri side via the legacy shim wrapper).
    #[error("io error: {0}")]
    Io(String),

    /// PKCS#8 / PEM encoding or decoding failure.
    #[error("pkcs8 / pem error: {0}")]
    Pkcs8(String),

    /// Signature operation failure (sign/verify).
    #[error("signature error: {0}")]
    Signature(String),

    /// JSON serialization / deserialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// PDF rendering failure (only ever raised by the src-tauri
    /// renderer; included here so the unified envelope type works
    /// across both pure + impure layers).
    #[error("pdf error: {0}")]
    Pdf(String),

    /// QR encoding failure (only raised by the src-tauri renderer).
    #[error("qr error: {0}")]
    Qr(String),

    /// Validation / business-rule failure.
    #[error("validation error: {0}")]
    Validation(String),
}

impl From<SigningError> for AchievementError {
    fn from(e: SigningError) -> Self {
        match e {
            SigningError::InvalidSignature => {
                AchievementError::Signature("invalid signature".to_string())
            }
            SigningError::KeyEncoding(msg) => AchievementError::Pkcs8(msg),
            SigningError::Io(msg) => AchievementError::Io(msg),
            SigningError::Canonical(c) => AchievementError::Validation(c.to_string()),
        }
    }
}

impl From<CanonicalJsonError> for AchievementError {
    fn from(e: CanonicalJsonError) -> Self {
        AchievementError::Validation(e.to_string())
    }
}

/// Persisted achievement row. Mirrors the `achievements` table v009 1:1,
/// with camelCase serde for IPC. D-12 + R4 + R5.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    /// UUID v4 row id.
    pub id: String,
    /// FK → `learner_profiles.id`.
    pub learner_id: String,
    /// FK → `learning_tracks.id`.
    pub track_id: String,
    /// CR-03 — present iff the path was generated from a topic pack.
    pub pack_id: Option<String>,
    /// `badge` | `certificate`.
    pub kind: String,
    /// `Associate` | `Practitioner` | `Professional` | `Completion`.
    pub level: String,
    /// RFC-3339 timestamp of issuance.
    pub issued_at: String,
    /// Aggregate mastery score (0.0..=1.0) at the moment of issuance.
    pub mastery_score: f64,
    /// Byte-stable canonical JSON of the [`CertPayloadV1`] used as
    /// signing input.
    pub payload_json: String,
    /// Hex-encoded Ed25519 signature over `payload_json`.
    pub signature: String,
    /// 8-hex SHA-256 fingerprint of the signing public key.
    pub key_fingerprint: String,
    /// Snapshot of the track's display topic at issuance time (R4).
    pub track_topic: String,
}

/// Per-track certification status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackCertifications {
    /// Levels earned so far for this learner/track.
    pub earned_levels: Vec<String>,
    /// Next level on the ladder (`None` if everything is earned).
    pub next_level: Option<String>,
    /// Human-readable description of what the next level needs.
    pub criteria: String,
}

/// V1 signed-payload contract — see `docs/CERT-PAYLOAD-V1.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertPayloadV1 {
    /// Learner display name at issuance time.
    pub learner: String,
    /// Learner profile id (UUID).
    pub learner_id: String,
    /// Track display topic at issuance time.
    pub track: String,
    /// Track id (UUID).
    pub track_id: String,
    /// Level being certified.
    pub level: String,
    /// RFC-3339 completion timestamp (same as the row `issued_at`).
    pub completion_date: String,
    /// Aggregate mastery score at issuance (0.0..=1.0).
    pub mastery_score: f64,
    /// 8-hex SHA-256 fingerprint of the signing public key.
    pub key_fingerprint: String,
    /// CR-03 — present iff the path was generated from a topic pack.
    pub pack_id: Option<String>,
    /// Dispatch tag — `1` for Phase 6 v1 payloads. Phase 14 introduces
    /// `2` if/when it switches to JWS-EdDSA.
    pub payload_version: u32,
}

/// Snapshot read by [`AchievementStore::lookup_issuance_context`] —
/// (learner_display, track_topic per R4, pack_id snapshot per CR-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuanceContext {
    /// Learner display name at issuance time.
    pub learner_display: String,
    /// Track display topic at issuance time (R4 immutability).
    pub track_topic: String,
    /// Pack id snapshot, parsed out of
    /// `learning_paths.generated_by_model = "topic-pack:<id>"`. `None`
    /// for AI-generated tracks (no prefix) and legacy tracks.
    pub pack_id: Option<String>,
}

// ── Artifact input shapes — DELIBERATELY ABSENT ──────────────────────────
//
// WR-01 (Phase 7 code review) — the `CertificatePdfInput` /
// `BadgePngInput` data structs previously lived here AND in
// `src-tauri/src/achievements/artifacts.rs`. Both were declared with
// identical fields + `#[serde(rename_all = "camelCase")]`, and the core
// versions had ZERO external callers — every src-tauri call site (PDF
// renderer, PNG renderer, IPC handler, tests) uses the src-tauri-local
// copies. The core types were dead code freezing public API at 0.1.0;
// a future field add to one would silently diverge from the other.
//
// Since the renderers stay in `src-tauri` per D-03 amendment + R-7
// (printpdf / qrcode / image are not WASM-portable), the renderer-input
// shapes belong next to the renderer. Cross-platform consumers that need
// to construct the inputs without pulling the src-tauri crate can
// either redeclare the (small, stable) shapes locally or import them
// when/if a follow-up wave promotes them back to core with a real
// caller in tow.
//
// ── Storage trait (A3 lock — 8th and final application) ──────────────────

/// Abstract storage surface for the achievements algorithm.
///
/// Declared next to the algorithm per A3 lock. The rusqlite-backed
/// implementation lives in `src-tauri/src/storage_impl/achievements.rs`
/// where `&rusqlite::Connection` is wrapped so it can carry the
/// trait surface (orphan-rule newtype pattern from Wave 4-7).
///
/// ## Methods
///
/// - [`track_mastery_aggregate`](AchievementStore::track_mastery_aggregate)
///   — single-row aggregate from `module_progress`. The rusqlite
///   impl delegates to the Wave 4 parked free fn in
///   `src-tauri/src/storage_impl/threshold.rs`; Wave 8 closes the
///   forward-declared seam.
/// - `existing_levels` — read the set of already-issued levels for
///   a (learner, track) so we know what to skip (idempotency).
/// - `insert_achievement_or_ignore` — `INSERT OR IGNORE` returning
///   `true` if a new row was actually written.
/// - `lookup_issuance_context` — read the per-track + per-learner
///   display snapshot used in the canonical payload.
/// - `list_for_learner` — sorted-by-issued-at-DESC stream for the IPC
///   handler `list_achievements_for_learner`.
/// - `lookup_achievement` — fetch one row by id (powers
///   `export_certificate` / `export_badge`).
/// - `earned_badge_levels` — read the `kind = 'badge'` levels for a
///   (learner, track) to compute next-level criteria.
pub trait AchievementStore {
    /// Compute the live track aggregate. Body lives in the rusqlite
    /// impl which delegates to the Wave 4 parked free fn.
    fn track_mastery_aggregate(
        &self,
        track_id: &str,
        learner_id: &str,
    ) -> Result<TrackAggregate, AchievementError>;

    /// Read the levels already issued for this (learner, track) so the
    /// algorithm can skip them on re-issuance.
    fn existing_levels(
        &self,
        learner_id: &str,
        track_id: &str,
    ) -> Result<Vec<String>, AchievementError>;

    /// `INSERT OR IGNORE` the achievement row. Returns `true` iff a
    /// row was actually written (UNIQUE(learner_id, track_id, level)
    /// constraint suppresses duplicates — R4 immutability).
    fn insert_achievement_or_ignore(
        &self,
        a: &Achievement,
    ) -> Result<bool, AchievementError>;

    /// Snapshot the per-track display context for the canonical
    /// payload (R4 immutability — captured at issuance time).
    fn lookup_issuance_context(
        &self,
        track_id: &str,
        learner_id: &str,
    ) -> Result<IssuanceContext, AchievementError>;

    /// Stream every achievement row in `issued_at DESC, id ASC` order.
    /// Phase 6 is single-learner desktop.
    fn list_for_learner(&self) -> Result<Vec<Achievement>, AchievementError>;

    /// Fetch one row by id. `Err(Validation)` on miss.
    fn lookup_achievement(&self, id: &str) -> Result<Achievement, AchievementError>;

    /// Read the levels for `kind = 'badge'` rows so the IPC handler
    /// can compute the next-level criteria string.
    fn earned_badge_levels(
        &self,
        track_id: &str,
        learner_id: &str,
    ) -> Result<Vec<String>, AchievementError>;
}

// ── Pure helpers ─────────────────────────────────────────────────────────

/// Build a [`CertPayloadV1`] + canonical bytes + signature for one
/// `(kind, level)` tuple. Pure function; takes the resolved
/// `SigningKey` so the signing-key store is not invoked per achievement.
#[allow(clippy::too_many_arguments)]
fn build_signed_achievement(
    key: &ed25519_dalek::SigningKey,
    key_fingerprint: &str,
    learner_display: &str,
    learner_id: &str,
    track_topic: &str,
    track_id: &str,
    pack_id: Option<&str>,
    kind: &str,
    level: &str,
    mastery_score: f64,
    issued_at: &str,
) -> Result<Achievement, AchievementError> {
    let payload = CertPayloadV1 {
        learner: learner_display.to_string(),
        learner_id: learner_id.to_string(),
        track: track_topic.to_string(),
        track_id: track_id.to_string(),
        level: level.to_string(),
        completion_date: issued_at.to_string(),
        mastery_score,
        key_fingerprint: key_fingerprint.to_string(),
        pack_id: pack_id.map(|s| s.to_string()),
        payload_version: 1,
    };
    let canonical = canonical_json_bytes(&payload)?;
    let sig = sign_payload(key, &canonical);
    let sig_hex = hex::encode(sig.to_bytes());
    let payload_json = String::from_utf8(canonical)
        .map_err(|_| AchievementError::Validation("non-utf8 canonical".into()))?;

    Ok(Achievement {
        id: uuid::Uuid::new_v4().to_string(),
        learner_id: learner_id.to_string(),
        track_id: track_id.to_string(),
        pack_id: pack_id.map(|s| s.to_string()),
        kind: kind.to_string(),
        level: level.to_string(),
        issued_at: issued_at.to_string(),
        mastery_score,
        payload_json,
        signature: sig_hex,
        key_fingerprint: key_fingerprint.to_string(),
        track_topic: track_topic.to_string(),
    })
}

// ── maybe_issue (A5 clock injection) ────────────────────────────────────

/// Issue any pending achievements for `(learner_id, track_id)` and
/// persist them. Idempotent across repeated calls (UNIQUE constraint
/// suppresses dupes; the algorithm also skips levels already in
/// [`AchievementStore::existing_levels`]).
///
/// **A5 clock injection** — `now` is supplied by the caller, never
/// pulled from `chrono::Utc::now()` inside this function. Tests pin
/// `now` so canonical payload bytes are reproducible; the src-tauri
/// shim wrapper supplies `Utc::now()` at the call site.
///
/// **R4 immutability** — existing achievement rows are never updated.
/// Even if mastery later decays, the historical proof remains.
///
/// # Behavior summary
///
/// 1. Compute the [`TrackAggregate`] via the store trait.
/// 2. Compute the set of `levels_met` via the pure
///    [`crate::threshold::levels_met`] predicate.
/// 3. Subtract the [`AchievementStore::existing_levels`] set to find
///    the levels to issue.
/// 4. If `Professional` is in the to-issue set AND `Completion` has
///    not been issued yet, append a `(certificate, Completion)`
///    issuance too.
/// 5. Look up the per-track snapshot via
///    [`AchievementStore::lookup_issuance_context`].
/// 6. Resolve the signing key via [`SigningKeyStore::get_or_init`].
/// 7. For each `(kind, level)` to issue, build a signed row and
///    `insert_achievement_or_ignore`. Only rows that the impl reports
///    as actually inserted (returning `true`) appear in the returned
///    `Vec<Achievement>`.
pub fn maybe_issue<S, K>(
    store: &S,
    key_store: &K,
    track_id: &str,
    learner_id: &str,
    now: DateTime<Utc>,
) -> Result<Vec<Achievement>, AchievementError>
where
    S: AchievementStore,
    K: SigningKeyStore,
{
    let agg = store.track_mastery_aggregate(track_id, learner_id)?;
    if agg.modules_total == 0 {
        return Ok(Vec::new());
    }
    let now_met = levels_met(&agg);
    if now_met.is_empty() {
        return Ok(Vec::new());
    }

    let already: HashSet<String> = store
        .existing_levels(learner_id, track_id)?
        .into_iter()
        .collect();

    let mut to_issue: Vec<(&'static str, &'static str)> = Vec::new();
    for level in &now_met {
        if !already.contains(*level) {
            to_issue.push(("badge", *level));
        }
    }
    if now_met.contains(&"Professional") && !already.contains("Completion") {
        to_issue.push(("certificate", "Completion"));
    }
    if to_issue.is_empty() {
        return Ok(Vec::new());
    }

    let ctx = store.lookup_issuance_context(track_id, learner_id)?;

    let key = key_store.get_or_init()?;
    let pub_fp = public_key_fingerprint(&key.verifying_key());

    let issued_at = now.to_rfc3339();
    let mut issued: Vec<Achievement> = Vec::new();

    for (kind, level) in to_issue {
        let ach = build_signed_achievement(
            &key,
            &pub_fp,
            &ctx.learner_display,
            learner_id,
            &ctx.track_topic,
            track_id,
            ctx.pack_id.as_deref(),
            kind,
            level,
            agg.avg_mastery,
            &issued_at,
        )?;
        let inserted = store.insert_achievement_or_ignore(&ach)?;
        if inserted {
            issued.push(ach);
        }
    }
    Ok(issued)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    //! Pure algorithm tests using inline stub `AchievementStore` +
    //! `SigningKeyStore` impls. SQL-touching integration tests stay
    //! in `src-tauri/src/achievements/mod.rs::tests` (they need a real
    //! `rusqlite::Connection`).

    use super::*;
    use crate::signing::verify_payload;
    use chrono::TimeZone;
    use ed25519_dalek::pkcs8::EncodePublicKey;
    use ed25519_dalek::SigningKey;
    use pkcs8::LineEnding;
    use rand::rngs::OsRng;
    use std::cell::RefCell;

    // ── Inline stubs ──────────────────────────────────────────────────

    /// In-memory `AchievementStore` driven by per-test canned data.
    /// Tracks `insert_achievement_or_ignore` calls so tests can verify
    /// the row was written.
    struct StubStore {
        aggregate: TrackAggregate,
        ctx: IssuanceContext,
        existing: RefCell<Vec<String>>,
        inserted: RefCell<Vec<Achievement>>,
    }

    impl StubStore {
        fn new(aggregate: TrackAggregate, ctx: IssuanceContext) -> Self {
            Self {
                aggregate,
                ctx,
                existing: RefCell::new(Vec::new()),
                inserted: RefCell::new(Vec::new()),
            }
        }

        fn with_existing(self, levels: Vec<String>) -> Self {
            *self.existing.borrow_mut() = levels;
            self
        }
    }

    impl AchievementStore for StubStore {
        fn track_mastery_aggregate(
            &self,
            _track_id: &str,
            _learner_id: &str,
        ) -> Result<TrackAggregate, AchievementError> {
            Ok(self.aggregate.clone())
        }
        fn existing_levels(
            &self,
            _learner_id: &str,
            _track_id: &str,
        ) -> Result<Vec<String>, AchievementError> {
            Ok(self.existing.borrow().clone())
        }
        fn insert_achievement_or_ignore(
            &self,
            a: &Achievement,
        ) -> Result<bool, AchievementError> {
            // Mirror the UNIQUE(learner_id, track_id, level) constraint:
            // suppress when this level has already been recorded.
            let already_inserted = self
                .inserted
                .borrow()
                .iter()
                .any(|x| x.level == a.level && x.kind == a.kind);
            if already_inserted {
                return Ok(false);
            }
            self.inserted.borrow_mut().push(a.clone());
            self.existing.borrow_mut().push(a.level.clone());
            Ok(true)
        }
        fn lookup_issuance_context(
            &self,
            _track_id: &str,
            _learner_id: &str,
        ) -> Result<IssuanceContext, AchievementError> {
            Ok(self.ctx.clone())
        }
        fn list_for_learner(&self) -> Result<Vec<Achievement>, AchievementError> {
            Ok(self.inserted.borrow().clone())
        }
        fn lookup_achievement(&self, id: &str) -> Result<Achievement, AchievementError> {
            self.inserted
                .borrow()
                .iter()
                .find(|a| a.id == id)
                .cloned()
                .ok_or(AchievementError::Validation(format!(
                    "stub: id {} not found",
                    id
                )))
        }
        fn earned_badge_levels(
            &self,
            _track_id: &str,
            _learner_id: &str,
        ) -> Result<Vec<String>, AchievementError> {
            Ok(self
                .inserted
                .borrow()
                .iter()
                .filter(|a| a.kind == "badge")
                .map(|a| a.level.clone())
                .collect())
        }
    }

    /// Caches a single in-memory key. Bytewise-deterministic across
    /// calls — exactly the property we need for byte-stable signatures.
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

    fn associate_only_aggregate() -> TrackAggregate {
        TrackAggregate {
            modules_total: 4,
            modules_mastered: 1, // 25% — Associate only
            avg_mastery: 0.5,
            has_practical_required: false,
            all_practical_labs_passed: true,
        }
    }

    fn professional_aggregate() -> TrackAggregate {
        TrackAggregate {
            modules_total: 4,
            modules_mastered: 4, // 100% — all tiers
            avg_mastery: 0.91,
            has_practical_required: false,
            all_practical_labs_passed: true,
        }
    }

    fn ctx_with_pack(pack_id: Option<&str>) -> IssuanceContext {
        IssuanceContext {
            learner_display: "Ada".to_string(),
            track_topic: "Kubernetes".to_string(),
            pack_id: pack_id.map(|s| s.to_string()),
        }
    }

    fn pinned_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap()
    }

    // ── Behaviour tests ──────────────────────────────────────────────

    #[test]
    fn issues_associate_on_first_module_completion() {
        let store = StubStore::new(associate_only_aggregate(), ctx_with_pack(None));
        let keys = StubKeyStore::fresh();
        let issued = maybe_issue(&store, &keys, "trk1", "lp1", pinned_now()).expect("issue");
        assert_eq!(issued.len(), 1, "1/4 = 25% — Associate only");
        let a = &issued[0];
        assert_eq!(a.level, "Associate");
        assert_eq!(a.kind, "badge");
        assert_eq!(a.signature.len(), 128, "Ed25519 sig hex = 64 bytes * 2");
        assert_eq!(a.key_fingerprint.len(), 8);
        assert_eq!(a.track_topic, "Kubernetes");
    }

    #[test]
    fn idempotent_on_second_call() {
        let store = StubStore::new(associate_only_aggregate(), ctx_with_pack(None));
        let keys = StubKeyStore::fresh();
        let first = maybe_issue(&store, &keys, "trk1", "lp1", pinned_now()).expect("first");
        let second = maybe_issue(&store, &keys, "trk1", "lp1", pinned_now()).expect("second");
        assert_eq!(first.len(), 1);
        assert!(second.is_empty(), "second call must be a no-op");
    }

    #[test]
    fn professional_emits_badge_and_certificate() {
        let store = StubStore::new(professional_aggregate(), ctx_with_pack(None));
        let keys = StubKeyStore::fresh();
        let issued = maybe_issue(&store, &keys, "trk1", "lp1", pinned_now()).expect("issue");
        // 4 rows: 3 badges + 1 certificate.
        assert_eq!(issued.len(), 4);
        let kinds: std::collections::HashSet<(String, String)> =
            issued.iter().map(|a| (a.kind.clone(), a.level.clone())).collect();
        assert!(kinds.contains(&("badge".into(), "Associate".into())));
        assert!(kinds.contains(&("badge".into(), "Practitioner".into())));
        assert!(kinds.contains(&("badge".into(), "Professional".into())));
        assert!(kinds.contains(&("certificate".into(), "Completion".into())));
    }

    /// A5 clock injection — given a pinned `now`, the canonical bytes
    /// (and thus the Ed25519 signature) are byte-for-byte reproducible
    /// across two independent runs that share the same signing key.
    #[test]
    fn signature_byte_stable_under_pinned_clock() {
        let key = SigningKey::generate(&mut OsRng);
        let keys = StubKeyStore { key: SigningKey::from_bytes(&key.to_bytes()) };

        let store_a = StubStore::new(associate_only_aggregate(), ctx_with_pack(None));
        let store_b = StubStore::new(associate_only_aggregate(), ctx_with_pack(None));

        let issued_a = maybe_issue(&store_a, &keys, "trk1", "lp1", pinned_now()).expect("a");
        let issued_b = maybe_issue(&store_b, &keys, "trk1", "lp1", pinned_now()).expect("b");

        // The achievement id is a fresh UUID per run — that's the only
        // non-deterministic field. payload_json + signature must match.
        assert_eq!(issued_a[0].payload_json, issued_b[0].payload_json);
        assert_eq!(issued_a[0].signature, issued_b[0].signature);
    }

    #[test]
    fn signed_payload_round_trips() {
        let store = StubStore::new(associate_only_aggregate(), ctx_with_pack(None));
        let keys = StubKeyStore::fresh();
        let issued = maybe_issue(&store, &keys, "trk1", "lp1", pinned_now()).expect("issue");
        assert_eq!(issued.len(), 1);
        let pub_pem = keys.export_public_pem().expect("public pem");
        let a = &issued[0];
        assert!(verify_payload(&pub_pem, a.payload_json.as_bytes(), &a.signature));
        let mut tampered = a.payload_json.clone();
        tampered.push(' ');
        assert!(!verify_payload(&pub_pem, tampered.as_bytes(), &a.signature));
    }

    #[test]
    fn skips_already_existing_levels() {
        // Pre-seed the store with Associate already earned.
        let store = StubStore::new(professional_aggregate(), ctx_with_pack(None))
            .with_existing(vec!["Associate".into()]);
        let keys = StubKeyStore::fresh();
        let issued = maybe_issue(&store, &keys, "trk1", "lp1", pinned_now()).expect("issue");
        // Professional aggregate would normally yield 4 (3 badges +
        // Completion). With Associate pre-seeded, we expect only 3
        // (Practitioner + Professional + Completion).
        assert_eq!(issued.len(), 3);
        let levels: std::collections::HashSet<String> =
            issued.iter().map(|a| a.level.clone()).collect();
        assert!(levels.contains("Practitioner"));
        assert!(levels.contains("Professional"));
        assert!(levels.contains("Completion"));
        assert!(!levels.contains("Associate"), "must not re-issue");
    }

    #[test]
    fn empty_track_returns_no_issuance() {
        let agg = TrackAggregate {
            modules_total: 0,
            modules_mastered: 0,
            avg_mastery: 0.0,
            has_practical_required: false,
            all_practical_labs_passed: true,
        };
        let store = StubStore::new(agg, ctx_with_pack(None));
        let keys = StubKeyStore::fresh();
        let issued = maybe_issue(&store, &keys, "trk-empty", "lp1", pinned_now()).expect("issue");
        assert!(issued.is_empty());
    }

    #[test]
    fn pack_id_propagates_into_payload_and_row() {
        let store = StubStore::new(
            associate_only_aggregate(),
            ctx_with_pack(Some("k8s-fundamentals")),
        );
        let keys = StubKeyStore::fresh();
        let issued = maybe_issue(&store, &keys, "trk1", "lp1", pinned_now()).expect("issue");
        assert_eq!(issued[0].pack_id.as_deref(), Some("k8s-fundamentals"));
        let payload: CertPayloadV1 =
            serde_json::from_str(&issued[0].payload_json).expect("parse v1 payload");
        assert_eq!(payload.pack_id.as_deref(), Some("k8s-fundamentals"));
    }

    #[test]
    fn achievement_store_is_object_safe() {
        let store = StubStore::new(associate_only_aggregate(), ctx_with_pack(None));
        let _dyn_store: &dyn AchievementStore = &store;
    }

    #[test]
    fn cert_payload_v1_serializes_camel_case() {
        let p = CertPayloadV1 {
            learner: "Ada".into(),
            learner_id: "lp1".into(),
            track: "K8s".into(),
            track_id: "t1".into(),
            level: "Associate".into(),
            completion_date: "2026-06-15T00:00:00+00:00".into(),
            mastery_score: 0.9,
            key_fingerprint: "deadbeef".into(),
            pack_id: None,
            payload_version: 1,
        };
        let s = serde_json::to_string(&p).expect("serialize");
        // Must use camelCase keys (learnerId, packId, payloadVersion).
        assert!(s.contains("\"learnerId\":"));
        assert!(s.contains("\"trackId\":"));
        assert!(s.contains("\"completionDate\":"));
        assert!(s.contains("\"masteryScore\":"));
        assert!(s.contains("\"keyFingerprint\":"));
        assert!(s.contains("\"packId\":"));
        assert!(s.contains("\"payloadVersion\":1"));
    }

    #[test]
    fn achievement_serializes_camel_case() {
        let store = StubStore::new(associate_only_aggregate(), ctx_with_pack(None));
        let keys = StubKeyStore::fresh();
        let issued = maybe_issue(&store, &keys, "trk1", "lp1", pinned_now()).expect("issue");
        let s = serde_json::to_string(&issued[0]).expect("serialize");
        assert!(s.contains("\"learnerId\":"));
        assert!(s.contains("\"trackId\":"));
        assert!(s.contains("\"issuedAt\":"));
        assert!(s.contains("\"masteryScore\":"));
        assert!(s.contains("\"payloadJson\":"));
        assert!(s.contains("\"keyFingerprint\":"));
        assert!(s.contains("\"trackTopic\":"));
    }
}
