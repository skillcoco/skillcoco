// Phase 08.2 (Cert Simplification + Gamification) — /achievements page.
//
// Closes D-09 (Dashboard "View all" link no longer 404s). Renders every
// achievement with the same Certificates / Milestones grouping as the
// Dashboard section (D-22). Sorted by `issuedAt DESC` within each group.
// Reuses AchievementCard so the certificate vs badge visual variants
// match the Dashboard.

import { useEffect, useMemo } from "react";
import { useAchievementsStore } from "@/stores/useAchievementsStore";
import { AchievementCard } from "@/components/achievements/AchievementCard";

export function Achievements() {
  const achievements = useAchievementsStore((s) => s.achievements);
  const loadAchievements = useAchievementsStore((s) => s.loadAchievements);

  useEffect(() => {
    loadAchievements();
  }, [loadAchievements]);

  // Sort by issuedAt DESC (newest first) within the full list, then
  // partition by kind. The store does not guarantee sort order; we sort
  // here so the page is robust to any store-side ordering changes.
  const { certificates, milestones } = useMemo(() => {
    const sorted = [...achievements].sort((a, b) =>
      b.issuedAt.localeCompare(a.issuedAt),
    );
    return {
      certificates: sorted.filter((a) => a.kind === "certificate"),
      milestones: sorted.filter((a) => a.kind === "badge"),
    };
  }, [achievements]);

  const hasAny = achievements.length > 0;

  return (
    <main
      data-testid="achievements-page"
      aria-label="All Achievements"
      className="mx-auto max-w-4xl space-y-6 px-4 py-8"
    >
      <header className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-foreground">All Achievements</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Every certificate and milestone you have earned, newest first.
          </p>
        </div>
      </header>

      {!hasAny ? (
        <div className="glass space-y-2 rounded-xl px-4 py-8 text-center">
          <div className="italic text-muted-foreground">
            No achievements yet
          </div>
          <div className="text-xs text-muted-foreground">
            Complete modules to earn your first milestone.
          </div>
        </div>
      ) : (
        <div className="space-y-6">
          {certificates.length > 0 && (
            <section
              data-testid="achievements-page-certificates"
              className="space-y-3"
            >
              <h2 className="text-sm uppercase tracking-wider text-muted-foreground">
                Certificates
              </h2>
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                {certificates.map((a) => (
                  <AchievementCard key={a.id} achievement={a} />
                ))}
              </div>
            </section>
          )}

          {milestones.length > 0 && (
            <section
              data-testid="achievements-page-milestones"
              className="space-y-3"
            >
              <h2 className="text-sm uppercase tracking-wider text-muted-foreground">
                Milestones
              </h2>
              <div className="flex flex-wrap gap-2">
                {milestones.map((a) => (
                  <AchievementCard key={a.id} achievement={a} />
                ))}
              </div>
            </section>
          )}
        </div>
      )}
    </main>
  );
}
