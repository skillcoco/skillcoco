// Phase 6 (Certification) — Plan 06-01 (Wave 0) TS type contract.
//
// Mirrors the Rust `Achievement` struct in src-tauri/src/achievements/mod.rs
// 1:1 with camelCase property names (Rust uses #[serde(rename_all =
// "camelCase")] — see CONVENTIONS.md). NEVER drift this file from the
// backend struct without updating both sides.

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

/// Verifier IPC request shape — pasted base64 payload + optional public-key
/// PEM override (when the user wants to verify against a key that isn't
/// the local one).
export interface VerifyCertificateRequest {
  payloadB64: string;
  publicKeyPemOverride: string | null;
}

/// Verifier IPC result — decoded + checked.
export interface VerifyCertificateResult {
  valid: boolean;
  learner: string;
  track: string;
  level: string;
  completionDate: string;
  keyFingerprint: string;
  /// Dispatch tag. Phase 6 emits 1; Phase 14 introduces 2.
  payloadVersion: number;
}
