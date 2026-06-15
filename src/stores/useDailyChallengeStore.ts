// Phase 4 Plan 04 (Wave 3) — real Zustand slice for the daily-challenge surface.
//
// Sibling slice (NOT an extension of useLearningStore) per Q2 lock + Phase 03.1
// useLabStore precedent (Pitfall 5). NEVER import or extend useLearningStore
// here — grep guard: `rg useLearningStore src/stores/useDailyChallengeStore.ts`
// must return 0.
//
// Mount-time IPC budget (Pitfall 6 — Dashboard load runs ≤ 2 IPCs):
//   1) isDailyChallengeEnabled() — gate + globalStreakDays in one round-trip
//   2) getDailyChallenge() — ONLY when enabled === true
//
// Pattern 3 (RESEARCH §"Optimistic UI + rollback") is honored for the two
// status-mutating actions: state flips locally first, IPC fires, server result
// reconciles (or rolls back on error).

import { create } from "zustand";
import {
  getDailyChallenge,
  startDailyChallenge as ipcStartDailyChallenge,
  completeDailyChallenge as ipcCompleteDailyChallenge,
  isDailyChallengeEnabled,
} from "@/lib/tauri-commands";
import type { DailyChallengePayload } from "@/types/learning";

interface DailyChallengeState {
  isEnabled: boolean;
  globalStreakDays: number;
  todaysChallenge: DailyChallengePayload | null;
  isLoading: boolean;

  // Actions — Plan 03 made both IPC wrappers parameterless (server resolves
  // challenge_date + learner_id), so neither store action takes a date arg.
  loadDailyChallenge: () => Promise<void>;
  startDailyChallenge: () => Promise<void>;
  completeDailyChallenge: () => Promise<void>;
}

const INITIAL: Omit<
  DailyChallengeState,
  "loadDailyChallenge" | "startDailyChallenge" | "completeDailyChallenge"
> = {
  isEnabled: false,
  globalStreakDays: 0,
  todaysChallenge: null,
  isLoading: false,
};

export const useDailyChallengeStore = create<DailyChallengeState>((set, get) => ({
  ...INITIAL,

  loadDailyChallenge: async () => {
    set({ isLoading: true });
    try {
      // Gate first. When disabled, we skip getDailyChallenge entirely
      // (Pitfall 6 — max 2 IPCs on Dashboard mount, but only 1 when gated).
      const enabledResult = await isDailyChallengeEnabled();
      if (!enabledResult.enabled) {
        set({
          isEnabled: false,
          todaysChallenge: null,
          globalStreakDays: enabledResult.globalStreakDays,
          isLoading: false,
        });
        return;
      }

      // Enabled → fetch today's challenge payload.
      const payload = await getDailyChallenge();
      set({
        isEnabled: true,
        // challenge is null when the BKT [0.3, 0.7] zone is empty (Q3 — empty-zone
        // variant). The store surfaces this as todaysChallenge=null with
        // isEnabled=true — that contract is consumed by TodaysChallengeCard.
        todaysChallenge: payload.challenge,
        globalStreakDays: enabledResult.globalStreakDays,
        isLoading: false,
      });
    } catch (err) {
      console.error("[useDailyChallengeStore] loadDailyChallenge failed:", err);
      set({ isLoading: false });
    }
  },

  startDailyChallenge: async () => {
    const current = get().todaysChallenge;
    if (!current) return;
    // Optimistic flip pending → in_progress. Idempotent on non-pending.
    if (current.status !== "pending") {
      // Still fire the IPC so the server records started_at on first call
      // even if a stray render started us in in_progress (defense in depth).
      try {
        await ipcStartDailyChallenge();
      } catch (err) {
        console.error("[useDailyChallengeStore] startDailyChallenge failed:", err);
      }
      return;
    }
    set({ todaysChallenge: { ...current, status: "in_progress" } });
    try {
      await ipcStartDailyChallenge();
    } catch (err) {
      console.error("[useDailyChallengeStore] startDailyChallenge failed:", err);
      // Roll back (Pattern 3 — useLearningStore.markLessonComplete:169-192).
      set({ todaysChallenge: current });
    }
  },

  completeDailyChallenge: async () => {
    const current = get().todaysChallenge;
    if (!current) return;
    if (current.status === "done") return; // idempotent guard

    // Optimistic: mark done; do NOT touch globalStreakDays yet — the IPC
    // result is authoritative (handles same-day idempotency, gap-reset, etc.).
    const priorStreak = get().globalStreakDays;
    set({ todaysChallenge: { ...current, status: "done" } });

    try {
      const result = await ipcCompleteDailyChallenge();
      // Reconcile with server-authoritative streak. The IPC may return a
      // value different from priorStreak+1 (e.g., user had an existing
      // server-side streak the client missed) — trust the server.
      set({ globalStreakDays: result.newStreakDays });
    } catch (err) {
      console.error("[useDailyChallengeStore] completeDailyChallenge failed:", err);
      // Roll back status + streak (Pattern 3).
      set({ todaysChallenge: current, globalStreakDays: priorStreak });
    }
  },
}));

/**
 * Test-only helper — resets the store to its initial state. Mirrors
 * `useLabStore.__resetStore`.
 */
export function __resetStore(): void {
  useDailyChallengeStore.setState({ ...INITIAL });
}
