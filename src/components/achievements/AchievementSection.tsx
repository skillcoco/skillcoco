// Phase 08.2 (Cert Simplification + Gamification) — Dashboard section.
//
// Groups achievements into two sections (D-21):
//   - "Certificates" — kind=certificate (Completion only in OSS)
//   - "Milestones" — kind=badge (Milestone25/50/75 + any legacy 3-tier
//     rows from pre-08.2 testing data)
//
// Renders the learner's most-recent achievements (D-09 cap of 6 per
// section). Drives `loadAchievements` on mount. Also surfaces the
// non-modal 5-second celebration banner when
// `useAchievementsStore.recentCelebration` is set.

import { useEffect } from "react";
import { Link } from "react-router-dom";
import { useAchievementsStore } from "@/stores/useAchievementsStore";
import { AchievementCard } from "./AchievementCard";
import type { Achievement } from "@/types/achievements";

const MAX_PER_SECTION = 6;
const CELEBRATION_TIMEOUT_MS = 5000;

function isCertificate(a: Achievement): boolean {
  return a.kind === "certificate";
}

function isBadge(a: Achievement): boolean {
  return a.kind === "badge";
}

export function AchievementSection() {
  const achievements = useAchievementsStore((s) => s.achievements);
  const loadAchievements = useAchievementsStore((s) => s.loadAchievements);
  const recentCelebration = useAchievementsStore((s) => s.recentCelebration);
  const clearCelebration = useAchievementsStore((s) => s.clearCelebration);

  useEffect(() => {
    loadAchievements();
  }, [loadAchievements]);

  useEffect(() => {
    if (!recentCelebration) return;
    const t = setTimeout(clearCelebration, CELEBRATION_TIMEOUT_MS);
    return () => clearTimeout(t);
  }, [recentCelebration, clearCelebration]);

  const certificates = achievements.filter(isCertificate).slice(0, MAX_PER_SECTION);
  const milestones = achievements.filter(isBadge).slice(0, MAX_PER_SECTION);
  const showViewAll = achievements.length > MAX_PER_SECTION * 2;
  const hasAny = achievements.length > 0;

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

      {!hasAny ? (
        <div className="glass rounded-xl px-4 py-6 text-sm italic text-muted-foreground">
          No achievements yet
        </div>
      ) : (
        <div className="space-y-4">
          {/* Certificates section — large cards, only renders when ≥1. */}
          {certificates.length > 0 && (
            <div data-testid="achievements-certificates" className="space-y-2">
              <h3 className="text-xs uppercase tracking-wider text-muted-foreground">
                Certificates
              </h3>
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                {certificates.map((a) => (
                  <AchievementCard key={a.id} achievement={a} />
                ))}
              </div>
            </div>
          )}

          {/* Milestones section — compact pills, wraps. */}
          {milestones.length > 0 && (
            <div data-testid="achievements-milestones" className="space-y-2">
              <h3 className="text-xs uppercase tracking-wider text-muted-foreground">
                Milestones
              </h3>
              <div className="flex flex-wrap gap-2">
                {milestones.map((a) => (
                  <AchievementCard key={a.id} achievement={a} />
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
