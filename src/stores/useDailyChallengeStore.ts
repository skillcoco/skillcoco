// Phase 4 Wave 0 — typed Zustand shell for the daily-challenge sibling slice.
//
// Sibling slice (NOT extension of useLearningStore) per Q2 lock + Phase 03.1
// useLabStore precedent.
//
// Wave 0 lands the typed surface so:
//   - tsc passes (the build gate `pnpm build` succeeds)
//   - vitest still fails at the *assertion* level (every action throws
//     "Plan 04 implements" — preserves the RED contract)
//
// Plan 04 (Wave 4 — Frontend store) replaces each action body with the
// optimistic-update-and-rollback IPC wiring sketched in
// `04-RESEARCH.md` Pattern 3.

import { create } from "zustand";
import type { DailyChallengePayload } from "@/types/learning";

interface DailyChallengeState {
  isEnabled: boolean;
  globalStreakDays: number;
  todaysChallenge: DailyChallengePayload | null;
  isLoading: boolean;

  // Actions — bodies are RED stubs; Plan 04 fills them.
  loadDailyChallenge: () => Promise<void>;
  startDailyChallenge: (challengeDate: string) => Promise<void>;
  completeDailyChallenge: (challengeDate: string) => Promise<void>;
}

const INITIAL: Omit<DailyChallengeState, "loadDailyChallenge" | "startDailyChallenge" | "completeDailyChallenge"> = {
  isEnabled: false,
  globalStreakDays: 0,
  todaysChallenge: null,
  isLoading: false,
};

export const useDailyChallengeStore = create<DailyChallengeState>((set, _get) => ({
  ...INITIAL,

  loadDailyChallenge: async () => {
    // Wave 0 RED stub — Plan 04 wires isDailyChallengeEnabled + getDailyChallenge.
    set({ isLoading: true });
    throw new Error("Plan 04 implements loadDailyChallenge");
  },

  startDailyChallenge: async (_challengeDate: string) => {
    // Wave 0 RED stub — Plan 04 wires startDailyChallenge IPC.
    throw new Error("Plan 04 implements startDailyChallenge");
  },

  completeDailyChallenge: async (_challengeDate: string) => {
    // Wave 0 RED stub — Plan 04 wires completeDailyChallenge IPC with
    // optimistic update + rollback per Pattern 3.
    throw new Error("Plan 04 implements completeDailyChallenge");
  },
}));

/**
 * Test-only helper — resets the store to its initial state. Pattern mirrors
 * `useLabStore.__resetStore`. Plan 04 keeps this export.
 */
export function __resetStore(): void {
  useDailyChallengeStore.setState({ ...INITIAL });
}
