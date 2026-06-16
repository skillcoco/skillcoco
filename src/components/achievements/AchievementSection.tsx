// Phase 6 (Certification) — Plan 06-01 (Wave 0) Dashboard section stub.
//
// Wave 3 (Plan 06-04) implements:
//   - Section header "Achievements" with optional "View all" link
//   - Empty state: "No achievements yet" copy
//   - Max 6 cards, newest first (D-09)
//
// Wave 0 deliberately renders NULL — the AchievementSection.test.tsx
// asserts the "No achievements yet" empty-state copy, which FAILS today.
// That is the RED contract this Wave 0 file pins.

import { useAchievementsStore } from "@/stores/useAchievementsStore";

export function AchievementSection() {
  // Read achievements via selector to keep this re-render scoped to the
  // slice. Wave 3 expands to a real render path.
  const achievements = useAchievementsStore((s) => s.achievements);

  // Wave 0 stub — Wave 3 implements empty state + card list + "View all".
  // Reference `achievements` so unused-locals lint passes.
  void achievements;
  return null;
}
