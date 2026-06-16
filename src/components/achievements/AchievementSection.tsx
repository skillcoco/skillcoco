// Phase 6 (Certification) — Plan 06-04 (Wave 3) Dashboard section.
//
// Renders the learner's most-recent 6 achievements (D-09 cap) with a
// "View all" link to /achievements when more exist. Drives loadAchievements
// on mount. Also surfaces the non-modal 5-second celebration banner when
// useAchievementsStore.recentCelebration is set (D-10: no OS notifications,
// no modals — pure inline banner that auto-dismisses).

import { useEffect } from "react";
import { Link } from "react-router-dom";
import { useAchievementsStore } from "@/stores/useAchievementsStore";
import { AchievementCard } from "./AchievementCard";

const MAX_SHOWN = 6;
const CELEBRATION_TIMEOUT_MS = 5000;

export function AchievementSection() {
  const achievements = useAchievementsStore((s) => s.achievements);
  const loadAchievements = useAchievementsStore((s) => s.loadAchievements);
  const recentCelebration = useAchievementsStore((s) => s.recentCelebration);
  const clearCelebration = useAchievementsStore((s) => s.clearCelebration);

  // Idempotent on-mount load. The store's loadAchievements re-issues IPC
  // and replaces state, so calling it on every Section mount is safe.
  useEffect(() => {
    loadAchievements();
  }, [loadAchievements]);

  // Non-modal celebration: 5-second auto-dismiss. The store sets
  // recentCelebration whenever appendNewlyIssued ingests at least one new
  // achievement (highest-tier picked).
  useEffect(() => {
    if (!recentCelebration) return;
    const t = setTimeout(clearCelebration, CELEBRATION_TIMEOUT_MS);
    return () => clearTimeout(t);
  }, [recentCelebration, clearCelebration]);

  const visible = achievements.slice(0, MAX_SHOWN);
  const showViewAll = achievements.length > MAX_SHOWN;

  return (
    <section
      data-testid="achievement-section"
      aria-label="Achievements"
      className="space-y-3"
    >
      <header className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-foreground">Achievements</h2>
        {showViewAll && (
          <Link
            to="/achievements"
            data-testid="achievements-view-all"
            className="text-sm font-medium text-primary hover:underline"
          >
            View all
          </Link>
        )}
      </header>

      {recentCelebration && (
        <div
          role="status"
          data-testid="achievement-celebration"
          className="rounded-md border border-amber-300/30 bg-amber-300/10 px-3 py-2 text-sm text-foreground"
        >
          You just earned {recentCelebration.level} in{" "}
          {recentCelebration.trackTopic}
        </div>
      )}

      {achievements.length === 0 ? (
        <div className="glass rounded-xl px-4 py-6 text-sm italic text-muted-foreground">
          No achievements yet
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          {visible.map((a) => (
            <AchievementCard key={a.id} achievement={a} />
          ))}
        </div>
      )}
    </section>
  );
}
