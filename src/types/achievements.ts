// Phase 6 (Certification) — Wave 2 (Plan 06-03) TS type contract.
//
// Mirrors the Rust `Achievement` struct + the five IPC handler request /
// result types in `src-tauri/src/achievements/mod.rs` +
// `src-tauri/src/commands/achievements.rs` with camelCase property names
// (Rust uses #[serde(rename_all = "camelCase")] — see CONVENTIONS.md).
// NEVER drift this file from the backend struct shapes.

export type AchievementKind = "badge" | "certificate";

/**
 * Phase 08.2 — cert simplification + gamification.
 *
 * Legacy 3-tier levels (Associate / Practitioner / Professional) are
 * still part of the union for the rare case where a local DB carries
 * pre-08.2 testing data (rendered as-is per D-02). New OSS desktop
 * issuances are Milestone25 / Milestone50 / Milestone75 (kind=badge)
 * + Completion (kind=certificate) only.
 */
export type AchievementLevel =
  | "Associate"
  | "Practitioner"
  | "Professional"
  | "Completion"
  | "Milestone25"
  | "Milestone50"
  | "Milestone75";

/** Phase 08.2 — milestone-only subset (kind=badge, in-app only). */
export type MilestoneLevel = "Milestone25" | "Milestone50" | "Milestone75";

export const MILESTONE_LEVELS: ReadonlyArray<MilestoneLevel> = [
  "Milestone25",
  "Milestone50",
  "Milestone75",
];

/** Numeric threshold (percent) for each milestone level. */
export function milestoneThreshold(level: MilestoneLevel): number {
  switch (level) {
    case "Milestone25":
      return 25;
    case "Milestone50":
      return 50;
    case "Milestone75":
      return 75;
  }
}

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

export interface GetTrackCertificationsRequest {
  trackId: string;
}
