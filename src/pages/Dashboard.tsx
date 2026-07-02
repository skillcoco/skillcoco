import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Plus, Target, BarChart3, Flame, BookOpen, Layers, Award } from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { useDailyChallengeStore } from "@/stores/useDailyChallengeStore";
import * as commands from "@/lib/tauri-commands";
import type { LearnerProfile } from "@/types";
import { StatsCard } from "@/components/dashboard/StatsCard";
import { TrackCard } from "@/components/dashboard/TrackCard";
import { SmartSessionCard } from "@/components/dashboard/SmartSessionCard";
import { TodaysChallengeCard } from "@/components/dashboard/TodaysChallengeCard";
import { AchievementSection } from "@/components/achievements/AchievementSection";

function getGreeting(): string {
  const hour = new Date().getHours();
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

export function Dashboard() {
  const { tracks, dueCards, loadTracks, loadDueCards } = useLearningStore();
  const loadDailyChallenge = useDailyChallengeStore((s) => s.loadDailyChallenge);
  const dailyChallengeEnabled = useDailyChallengeStore((s) => s.isEnabled);
  const globalStreakDays = useDailyChallengeStore((s) => s.globalStreakDays);
  const [profile, setProfile] = useState<LearnerProfile | null>(null);

  useEffect(() => {
    loadTracks();
    loadDueCards();
    loadDailyChallenge();
    commands.getOrCreateProfile()
      .then(setProfile)
      .catch((err) => console.error("Failed to load profile:", err));
  }, []);

  // Treat both "active" and "onboarding" tracks as in-progress. Tracks stay in
  // "onboarding" status while their AI-generated path is being built and during
  // first-pass learning; status transitions to "active" downstream. For Dashboard
  // stats + Smart Session gating, both states count as engaged learning.
  const activeTracks = tracks.filter(
    (t) => t.status === "active" || t.status === "onboarding"
  );
  const [moduleCounts, setModuleCounts] = useState<Record<string, { total: number; completed: number }>>({});

  useEffect(() => {
    async function loadModuleCounts() {
      const counts: Record<string, { total: number; completed: number }> = {};
      for (const track of tracks) {
        try {
          const [path, progress] = await Promise.all([
            commands.getPath(track.id),
            commands.getModuleProgress(track.id),
          ]);
          // Backend returns modulesJson as a JSON-encoded string; parse it.
          // path.modules is deprecated and undefined here.
          const total = (() => {
            try {
              return JSON.parse(path.modulesJson || "[]").length;
            } catch {
              return 0;
            }
          })();
          const completed = progress.filter((p) => p.status === "completed").length;
          counts[track.id] = { total, completed };
        } catch {
          counts[track.id] = { total: 0, completed: 0 };
        }
      }
      setModuleCounts(counts);
    }
    if (tracks.length > 0) loadModuleCounts();
  }, [tracks]);

  const totalModulesAll = Object.values(moduleCounts).reduce((s, c) => s + c.total, 0);
  const completedModulesAll = Object.values(moduleCounts).reduce((s, c) => s + c.completed, 0);

  const displayName = profile?.displayName || "Learner";

  // Summary for subtitle
  const subtitle = [
    dueCards.length > 0 ? `${dueCards.length} reviews due` : null,
    activeTracks.length > 0 ? `${activeTracks.length} active learning track${activeTracks.length !== 1 ? "s" : ""}` : null,
  ]
    .filter(Boolean)
    .join(" and ");

  // Smart session estimates
  const estimatedMinutes = dueCards.length * 2 + (activeTracks.length > 0 ? 15 : 0);
  const nextModule = activeTracks.length > 0 && activeTracks[0].currentModuleId
    ? activeTracks[0].topic
    : null;

  return (
    <div className="mx-auto max-w-6xl space-y-8 pb-12">
      {/* Greeting */}
      <div>
        <h1 className="text-3xl font-bold text-foreground">
          {getGreeting()}, {displayName}.
        </h1>
        {subtitle && (
          <p className="mt-1 text-sm text-muted-foreground">
            You have {subtitle}.
          </p>
        )}
      </div>

      {/* Today's Challenge Card — Phase 4 Plan 04. Sits ABOVE SmartSessionCard
          per RESEARCH section 5: the daily challenge is the lower-friction
          action and the most "daily" surface, so it leads. The card itself
          handles the D-12 gate (returns null when isEnabled === false). */}
      <TodaysChallengeCard />

      {/* Smart Session Card — show when there are due cards OR an active track */}
      {(dueCards.length > 0 || activeTracks.length > 0) && (
        <SmartSessionCard
          dueCount={dueCards.length}
          nextModuleName={nextModule}
          estimatedMinutes={estimatedMinutes}
        />
      )}

      {/* Phase 6 Plan 06-04 (Wave 3) — Achievements section. Lives BETWEEN
          SmartSessionCard and the StatsRow per D-09. Empty-state hides
          itself behind a "No achievements yet" line; cap of 6 newest with
          "View all" link to /achievements. Non-modal 5s celebration
          banner surfaces when submitQuiz issues a new achievement. */}
      <AchievementSection />

      {/* Stats Row */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-5">
        <StatsCard
          label="Reviews Due"
          value={dueCards.length}
          subtitle="across all tracks"
          icon={<Target size={18} />}
          accentColor="hsl(var(--primary))"
        />
        <StatsCard
          label="Modules Done"
          value={`${completedModulesAll} of ${totalModulesAll}`}
          subtitle="total progress"
          icon={<BarChart3 size={18} />}
          accentColor="hsl(var(--info))"
        />
        {/* Best Streak StatsCard replaced with global-streak version (Phase 4
            Plan 04, RESEARCH section 5). Honors D-12: shows "--" until the
            auto-enable gate fires so the streak counter never leaks before
            the daily challenge surface goes live. */}
        <StatsCard
          label="Daily Streak"
          value={dailyChallengeEnabled ? `${globalStreakDays}d` : "--"}
          subtitle={dailyChallengeEnabled ? "consecutive days" : "not yet active"}
          icon={<Flame size={18} />}
          accentColor="hsl(var(--warning))"
        />
        <StatsCard
          label="Active Tracks"
          value={activeTracks.length}
          subtitle="concurrent topics"
          icon={<Layers size={18} />}
          accentColor="hsl(var(--success))"
        />
        {/* Phase 08.2 (D-09) — Points stat card. */}
        <StatsCard
          label="Points"
          value={profile?.points ?? 0}
          subtitle="quizzes + modules + milestones"
          icon={<Award size={18} />}
          accentColor="hsl(var(--warning))"
        />
      </div>

      {/* Your Tracks */}
      <div>
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-foreground">Your Tracks</h2>
          <Link
            to="/onboarding"
            className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
          >
            <Plus size={16} />
            New Track
          </Link>
        </div>

        {tracks.length === 0 ? (
          <div className="glass flex flex-col items-center justify-center rounded-xl py-20 text-center">
            <BookOpen className="mb-4 text-muted-foreground" size={48} />
            <h3 className="text-lg font-semibold text-foreground">No learning tracks yet</h3>
            <p className="mb-6 mt-1 max-w-sm text-sm text-muted-foreground">
              Start your first track and let AI create a personalized learning path for you.
            </p>
            <Link
              to="/onboarding"
              className="inline-flex items-center gap-2 rounded-lg bg-primary px-5 py-2.5 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90"
            >
              <Plus size={16} />
              Start Learning
            </Link>
          </div>
        ) : (
          <div className="grid gap-4 md:grid-cols-2">
            {tracks.map((track) => {
              const trackCounts = moduleCounts[track.id] ?? { total: 0, completed: 0 };
              return (
                <TrackCard
                  key={track.id}
                  track={track}
                  dueReviews={dueCards.length}
                  totalModules={trackCounts.total}
                  completedModules={trackCounts.completed}
                  streakDays={track.streakDays ?? 0}
                  nextModuleName={track.currentModuleId ? "Continue" : null}
                />
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
