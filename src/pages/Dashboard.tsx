import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Plus, Target, BarChart3, Flame, BookOpen, Layers } from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { getOrCreateProfile } from "@/lib/tauri-commands";
import type { LearnerProfile } from "@/types";
import { StatsCard } from "@/components/dashboard/StatsCard";
import { TrackCard } from "@/components/dashboard/TrackCard";
import { SmartSessionCard } from "@/components/dashboard/SmartSessionCard";

function getGreeting(): string {
  const hour = new Date().getHours();
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

export function Dashboard() {
  const { tracks, dueCards, loadTracks, loadDueCards } = useLearningStore();
  const [profile, setProfile] = useState<LearnerProfile | null>(null);

  useEffect(() => {
    loadTracks();
    loadDueCards();
    getOrCreateProfile()
      .then(setProfile)
      .catch((err) => console.error("Failed to load profile:", err));
  }, []);

  const activeTracks = tracks.filter((t) => t.status === "active");
  const totalModulesAll = tracks.length * 10; // approximate; will be replaced by real data
  const completedModulesAll = tracks.reduce(
    (acc, t) => acc + Math.round((t.progressPercent / 100) * 10),
    0,
  );
  const bestStreak = { days: 0, trackName: "" }; // placeholder until streak tracking is wired
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

      {/* Smart Session Card */}
      {dueCards.length > 0 && (
        <SmartSessionCard
          dueCount={dueCards.length}
          nextModuleName={nextModule}
          estimatedMinutes={estimatedMinutes}
        />
      )}

      {/* Stats Row */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
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
        <StatsCard
          label="Best Streak"
          value={bestStreak.days > 0 ? `${bestStreak.days}d` : "--"}
          subtitle={bestStreak.trackName || "no data yet"}
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
              const trackDueCards = dueCards.filter((c) =>
                // If cards have a track-level association we filter; otherwise count all
                true
              );
              const approxTotal = 10;
              const approxCompleted = Math.round((track.progressPercent / 100) * approxTotal);

              return (
                <TrackCard
                  key={track.id}
                  track={track}
                  dueReviews={trackDueCards.length}
                  totalModules={approxTotal}
                  completedModules={approxCompleted}
                  streakDays={0}
                  nextModuleName={track.currentModuleId ? `Module ${approxCompleted + 1}` : null}
                />
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
