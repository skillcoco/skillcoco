import { useEffect } from "react";
import { Link } from "react-router-dom";
import { Plus, ArrowRight, Brain, Target, Clock } from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { formatDuration } from "@/lib/utils";

export function Dashboard() {
  const { tracks, dueCards, loadTracks, loadDueCards } = useLearningStore();

  useEffect(() => {
    loadTracks();
    loadDueCards();
  }, []);

  return (
    <div className="mx-auto max-w-5xl space-y-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-foreground">Dashboard</h1>
          <p className="text-sm text-muted-foreground">
            Your learning journey at a glance
          </p>
        </div>
        <Link
          to="/onboarding"
          className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        >
          <Plus size={16} />
          New Track
        </Link>
      </div>

      {/* Quick Actions */}
      {dueCards.length > 0 && (
        <Link
          to="/review"
          className="flex items-center justify-between rounded-lg border border-border bg-card p-4 transition-colors hover:bg-accent"
        >
          <div className="flex items-center gap-3">
            <Brain className="text-primary" size={20} />
            <div>
              <div className="font-medium text-foreground">
                {dueCards.length} cards ready for review
              </div>
              <div className="text-sm text-muted-foreground">
                Keep your streak alive with a quick review session
              </div>
            </div>
          </div>
          <ArrowRight size={18} className="text-muted-foreground" />
        </Link>
      )}

      {/* Active Tracks */}
      <div>
        <h2 className="mb-4 text-lg font-semibold text-foreground">Active Tracks</h2>
        {tracks.length === 0 ? (
          <div className="flex flex-col items-center justify-center rounded-lg border border-dashed border-border py-16 text-center">
            <Target className="mb-4 text-muted-foreground" size={40} />
            <h3 className="text-lg font-medium text-foreground">No learning tracks yet</h3>
            <p className="mb-6 text-sm text-muted-foreground">
              Start your first track and let AI create a personalized learning path
            </p>
            <Link
              to="/onboarding"
              className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
            >
              <Plus size={16} />
              Start Learning
            </Link>
          </div>
        ) : (
          <div className="grid gap-4 md:grid-cols-2">
            {tracks.map((track) => (
              <Link
                key={track.id}
                to={`/track/${track.id}`}
                className="rounded-lg border border-border bg-card p-5 transition-colors hover:bg-accent"
              >
                <div className="mb-3 flex items-center justify-between">
                  <h3 className="font-semibold text-foreground">{track.topic}</h3>
                  <span className="rounded-full bg-secondary px-2.5 py-0.5 text-xs text-secondary-foreground">
                    {track.domainModule}
                  </span>
                </div>
                <p className="mb-4 text-sm text-muted-foreground line-clamp-2">
                  {track.goal}
                </p>
                <div className="space-y-2">
                  <div className="flex justify-between text-xs text-muted-foreground">
                    <span>{track.progressPercent}% complete</span>
                    <span className="flex items-center gap-1">
                      <Clock size={12} />
                      {formatDuration(track.totalTimeSpent)}
                    </span>
                  </div>
                  <div className="h-1.5 rounded-full bg-secondary">
                    <div
                      className="h-1.5 rounded-full bg-primary transition-all"
                      style={{ width: `${track.progressPercent}%` }}
                    />
                  </div>
                </div>
              </Link>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
