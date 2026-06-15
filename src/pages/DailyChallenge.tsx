// Phase 4 Wave 0 — typed shell for the focused daily-challenge view.
//
// Wave 0 lands the typed surface only so:
//   - tsc passes (the build gate `pnpm build` succeeds)
//   - vitest still fails at the *assertion* level — the page renders nothing
//     so waitFor(getByTestId("block-renderer-mock")) times out
//
// Plan 05 (Wave 5 — Frontend page) replaces this with the route handler
// that calls startDailyChallenge on mount, renders BlockRenderer for the
// chosen block, subscribes to its completion signal, then calls
// completeDailyChallenge + navigate("/").

import { useDailyChallengeStore } from "@/stores/useDailyChallengeStore";

export function DailyChallenge(): JSX.Element | null {
  // Reference the store so Plan 05's mock factory wires correctly.
  useDailyChallengeStore();

  // Wave 0 RED stub — Plan 05 implements.
  return null;
}
