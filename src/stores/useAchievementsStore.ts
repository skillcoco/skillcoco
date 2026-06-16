// Phase 6 (Certification) — Plan 06-01 (Wave 0) achievements Zustand slice.
//
// SIBLING slice (NOT an extension of useLearningStore) per Phase 4
// Pitfall 5 + Phase 03.1 useLabStore precedent. Grep guard:
// `rg useLearningStore src/stores/useAchievementsStore.ts` must return 0.
//
// Wave 0 ships the contract — Wave 3 mounts the Dashboard
// AchievementSection that drives loadAchievements() on mount.

import { create } from "zustand";
import { listAchievements } from "@/lib/tauri-commands";
import type { Achievement } from "@/types/achievements";

interface AchievementsState {
  achievements: Achievement[];
  isLoading: boolean;
  error: string | null;

  // Actions
  loadAchievements: () => Promise<void>;
  /// Optimistic-append helper: Wave 2's submit_quiz hook surfaces
  /// `newlyIssuedAchievements: Achievement[]` (per A4 lock) and the
  /// frontend prepends those to the list without a re-fetch.
  appendNewlyIssued: (issued: Achievement[]) => void;
}

const INITIAL: Omit<
  AchievementsState,
  "loadAchievements" | "appendNewlyIssued"
> = {
  achievements: [],
  isLoading: false,
  error: null,
};

export const useAchievementsStore = create<AchievementsState>((set) => ({
  ...INITIAL,

  loadAchievements: async () => {
    set({ isLoading: true, error: null });
    try {
      const list = await listAchievements();
      set({ achievements: list, isLoading: false });
    } catch (err) {
      console.error("[useAchievementsStore] loadAchievements failed:", err);
      set({ error: String(err), isLoading: false });
    }
  },

  appendNewlyIssued: (issued) =>
    set((s) => ({ achievements: [...issued, ...s.achievements] })),
}));

/// Test-only helper — resets the store to its initial state.
/// Mirrors `useDailyChallengeStore.__resetStore`.
export function __resetStore(): void {
  useAchievementsStore.setState({ ...INITIAL });
}
