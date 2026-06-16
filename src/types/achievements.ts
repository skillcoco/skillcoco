// Phase 6 (Certification) — Wave 2 (Plan 06-03) TS type contract.
//
// Mirrors the Rust `Achievement` struct + the five IPC handler request /
// result types in `src-tauri/src/achievements/mod.rs` +
// `src-tauri/src/commands/achievements.rs` with camelCase property names
// (Rust uses #[serde(rename_all = "camelCase")] — see CONVENTIONS.md).
// NEVER drift this file from the backend struct shapes.

export type AchievementKind = "badge" | "certificate";

export type AchievementLevel =
  | "Associate"
  | "Practitioner"
  | "Professional"
  | "Completion";

/// Persisted achievement row (one per badge/cert per learner per track).
/// D-04 immutability: once issued, never revoked.
export interface Achievement {
  id: string;
  learnerId: string;
  trackId: string;
  packId: string | null;
  kind: AchievementKind;
  level: AchievementLevel;
  issuedAt: string;
  masteryScore: number;
  payloadJson: string;
  signature: string;
  keyFingerprint: string;
  /// R4 — snapshot of the track topic at issuance time so the cert is
  /// readable even after the track is deleted.
  trackTopic: string;
}

/// Per-track certification status (TrackView "next level" indicator).
export interface TrackCertifications {
  earnedLevels: AchievementLevel[];
  nextLevel: AchievementLevel | null;
  criteria: string;
}

// ── Wave 2 IPC request / result types ────────────────────────────────

export interface ExportCertificateRequest {
  achievementId: string;
}

export interface ExportBadgeRequest {
  achievementId: string;
}

export interface VerifySignatureRequest {
  payloadB64: string;
  publicKeyPemOverride: string | null;
}

export interface VerifySignatureResult {
  valid: boolean;
  learner: string;
  track: string;
  level: string;
  completionDate: string;
  keyFingerprint: string;
  /// Dispatch tag. Phase 6 emits 1; Phase 14 introduces 2.
  payloadVersion: number;
  /// Structured error code on failure ("payload_too_large",
  /// "malformed_envelope", "invalid_base64", "signature_mismatch", …).
  /// `null` on `valid=true`.
  error: string | null;
}

export interface GetTrackCertificationsRequest {
  trackId: string;
}

// ── Legacy Wave 0 names retained for the brief overlap until Wave 5
//     UI lands. New code should use VerifySignatureRequest /
//     VerifySignatureResult.

/** @deprecated Use VerifySignatureRequest. */
export type VerifyCertificateRequest = VerifySignatureRequest;
/** @deprecated Use VerifySignatureResult. */
export type VerifyCertificateResult = VerifySignatureResult;
