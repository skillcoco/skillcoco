// Phase 4 Wave 0 — typed shell for the Dashboard "Today's Challenge" card.
//
// Wave 0 lands the typed surface only so:
//   - tsc passes (the build gate `pnpm build` succeeds)
//   - vitest still fails at the *assertion* level — the component returns
//     null in every state so screen.getByText assertions fail
//
// Plan 04 (Wave 4 — Frontend card) replaces this with the
// SmartSessionCard-styled implementation (gradient border + glass interior
// + state-aware Start / Resume / Done copy).

import { useDailyChallengeStore } from "@/stores/useDailyChallengeStore";

export function TodaysChallengeCard(): JSX.Element | null {
  // Reference the store so the test file's mock factory wires correctly;
  // discard the result — Plan 04 reads isEnabled / todaysChallenge here.
  useDailyChallengeStore();

  // Wave 0 RED stub — return nothing so the assertion-level tests fail.
  // Plan 04 replaces this with the real card.
  return null;
}
