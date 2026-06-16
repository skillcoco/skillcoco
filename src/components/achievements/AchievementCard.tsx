// Phase 6 (Certification) — Plan 06-01 (Wave 0) badge/cert card stub.
//
// Renders a single Achievement row in the Dashboard "Achievements" section.
// Wave 3 (Plan 06-04) implements: level icon, track-topic line, issued-at
// date, key fingerprint footer, hover -> export buttons (PDF for cert, PNG
// for badge, Copy share text).

import type { Achievement } from "@/types/achievements";

interface Props {
  a: Achievement;
}

export function AchievementCard({ a }: Props) {
  // Wave 0 render stub. Wave 3 expands.
  return (
    <div data-testid="achievement-card">
      {a.level} - {a.trackTopic}
    </div>
  );
}
