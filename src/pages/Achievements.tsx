// Phase 6 (Certification) — Plan 06-04 (Wave 3) /achievements page.
//
// Closes D-09 (Dashboard "View all" link no longer 404s). Renders every
// achievement (no 6-card cap), sorted by issuedAt DESC by default. Reuses
// AchievementCard from Wave 3 — no duplicated card styling. Explicit
// sort/filter controls are deferred (Phase 14+); Phase 6 ships DESC only.

import { useEffect, useMemo } from "react";
import { useAchievementsStore } from "@/stores/useAchievementsStore";
import { AchievementCard } from "@/components/achievements/AchievementCard";

export function Achievements() {
  const achievements = useAchievementsStore((s) => s.achievements);
  const loadAchievements = useAchievementsStore((s) => s.loadAchievements);

  useEffect(() => {
    loadAchievements();
  }, [loadAchievements]);

  // Sort by issuedAt DESC (newest first). The store does not maintain
  // sort order — it prepends new arrivals via appendNewlyIssued, which
  // happens to also yield DESC order in practice, but we sort here so
  // the page is robust to any store-side ordering changes.
  const sorted = useMemo(
    () =>
      [...achievements].sort((a, b) => b.issuedAt.localeCompare(a.issuedAt)),
    [achievements],
  );

  return (
    <main
      data-testid="achievements-page"
      aria-label="All Achievements"
      className="mx-auto max-w-4xl space-y-6 px-4 py-8"
    >
      <header>
        <h1 className="text-2xl font-bold text-foreground">All Achievements</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Every badge and certificate you have earned, newest first.
        </p>
      </header>

      {sorted.length === 0 ? (
        <div className="glass space-y-2 rounded-xl px-4 py-8 text-center">
          <div className="italic text-muted-foreground">
            No achievements yet
          </div>
          <div className="text-xs text-muted-foreground">
            Complete modules to earn your first badge.
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          {sorted.map((a) => (
            <AchievementCard key={a.id} achievement={a} />
          ))}
        </div>
      )}
    </main>
  );
}
