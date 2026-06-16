// Phase 6 (Certification) — Plan 06-04 (Wave 3) achievements Zustand slice.
//
// SIBLING slice (NOT an extension of useLearningStore) per Phase 4
// Pitfall 5 + Phase 03.1 useLabStore precedent. Grep guard:
// `rg useLearningStore src/stores/useAchievementsStore.ts` must return 0.
//
// Wave 0 shipped the contract (initial state + loadAchievements +
// appendNewlyIssued stub). Wave 3 lands:
//   - dedup-by-id in appendNewlyIssued (idempotent)
//   - recentCelebration (non-modal toast surface; D-10 / 06-CONTEXT.md
//     defers OS notifications + modals)
//   - exportCertificate / exportBadge wrappers around the Wave 2 live IPCs
//     (uses the suggested-filename helper for slug + ISO-date naming)
//   - clearCelebration action (component dismisses the toast on 5s timer)

import { create } from "zustand";
import {
  listAchievements,
  exportCertificate as exportCertificateCmd,
  exportBadge as exportBadgeCmd,
} from "@/lib/tauri-commands";
import type { Achievement, AchievementLevel } from "@/types/achievements";

export interface ExportResult {
  saved: boolean;
  path: string | null;
}

interface AchievementsState {
  achievements: Achievement[];
  isLoading: boolean;
  error: string | null;
  /// Non-modal toast surface. Set by `appendNewlyIssued`; cleared by the
  /// AchievementSection's 5-second timer via `clearCelebration`. D-10
  /// (06-CONTEXT.md) defers OS notifications + modals to Phase 14.
  recentCelebration: Achievement | null;

  // Actions
  loadAchievements: () => Promise<void>;
  /// Optimistic-append helper: Wave 1's submit_quiz hook surfaces
  /// `newlyIssuedAchievements: Achievement[]` (per A4 lock) and the
  /// frontend prepends those to the list without a re-fetch. Idempotent
  /// — duplicate ids are skipped.
  appendNewlyIssued: (issued: Achievement[]) => void;
  /// Render the certificate PDF + native save dialog (Wave 2 IPC).
  /// Returns `{ saved: false, path: null }` when the user cancels or when
  /// called on a non-certificate kind.
  exportCertificate: (a: Achievement) => Promise<ExportResult>;
  /// Render the PNG badge + native save dialog (Wave 2 IPC). Works for
  /// both badge and certificate kinds.
  exportBadge: (a: Achievement) => Promise<ExportResult>;
  /// Clear the non-modal celebration toast (5s timer in the section).
  clearCelebration: () => void;
}

const INITIAL: Omit<
  AchievementsState,
  | "loadAchievements"
  | "appendNewlyIssued"
  | "exportCertificate"
  | "exportBadge"
  | "clearCelebration"
> = {
  achievements: [],
  isLoading: false,
  error: null,
  recentCelebration: null,
};

// ── helpers ─────────────────────────────────────────────────────────

/// T-06-14 mitigation: track topic must not contain path-traversal or
/// shell-meaningful characters. We strip everything that is not
/// `[a-z0-9-]` so the suggested filename is always safe to pass to the
/// dialog plugin. The user still confirms the final path interactively.
function slugify(input: string): string {
  return input
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

function isoToCompactDate(iso: string): string {
  // "2026-06-16T12:34:56Z" -> "20260616"
  return iso.slice(0, 10).replace(/-/g, "");
}

function suggestedFilename(
  a: Achievement,
  ext: "pdf" | "png",
): string {
  const slug = slugify(a.trackTopic || a.trackId);
  const date = isoToCompactDate(a.issuedAt);
  const kind = ext === "pdf" ? "certificate" : "badge";
  return `learnforge-${kind}-${slug}-${date}.${ext}`;
}

/// Picks the highest-tier achievement (Completion > Professional >
/// Practitioner > Associate) so the celebration banner showcases the most
/// impressive new badge when multiple thresholds cross in one submission.
function pickHighestTier(list: Achievement[]): Achievement | null {
  if (!list.length) return null;
  const order: AchievementLevel[] = [
    "Completion",
    "Professional",
    "Practitioner",
    "Associate",
  ];
  for (const lvl of order) {
    const found = list.find((a) => a.level === lvl);
    if (found) return found;
  }
  return list[0];
}

// ── store ───────────────────────────────────────────────────────────

export const useAchievementsStore = create<AchievementsState>((set) => ({
  ...INITIAL,

  loadAchievements: async () => {
    set({ isLoading: true, error: null });
    try {
      const list = await listAchievements();
      set({ achievements: list, isLoading: false });
    } catch (err) {
      console.error("[useAchievementsStore] loadAchievements failed:", err);
      const message = err instanceof Error ? err.message : String(err);
      set({ error: message, isLoading: false });
    }
  },

  appendNewlyIssued: (issued) => {
    if (!issued.length) return;
    set((s) => {
      const existingIds = new Set(s.achievements.map((x) => x.id));
      const fresh = issued.filter((x) => !existingIds.has(x.id));
      if (!fresh.length) return s;
      return {
        ...s,
        achievements: [...fresh, ...s.achievements],
        recentCelebration: pickHighestTier(fresh),
      };
    });
  },

  exportCertificate: async (a) => {
    // PDF is only meaningful for the completion certificate (D-06). For
    // badges we surface an empty result rather than firing the wrong IPC.
    if (a.kind !== "certificate") {
      return { saved: false, path: null };
    }
    const filename = suggestedFilename(a, "pdf");
    const path = await exportCertificateCmd(
      { achievementId: a.id },
      filename,
    );
    return { saved: path !== null, path };
  },

  exportBadge: async (a) => {
    const filename = suggestedFilename(a, "png");
    const path = await exportBadgeCmd({ achievementId: a.id }, filename);
    return { saved: path !== null, path };
  },

  clearCelebration: () => set({ recentCelebration: null }),
}));

/// Test-only helper — resets the store to its initial state.
/// Mirrors `useDailyChallengeStore.__resetStore`.
export function __resetStore(): void {
  useAchievementsStore.setState({ ...INITIAL });
}
