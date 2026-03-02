import { Link } from "react-router-dom";
import { BookOpen, BarChart3, Flame, Clock } from "lucide-react";
import type { LearningTrack } from "@/types";
import { formatDuration } from "@/lib/utils";

function getTrackColor(topic: string): string {
  const key = topic.toLowerCase();
  if (key.includes("kubernetes") || key.includes("k8s")) return "hsl(var(--track-kubernetes))";
  if (key.includes("rust")) return "hsl(var(--track-rust))";
  if (key.includes("go") || key.includes("golang")) return "hsl(var(--track-go))";
  if (key.includes("python")) return "hsl(var(--track-python))";
  return "hsl(var(--primary))";
}

interface TrackCardProps {
  track: LearningTrack;
  dueReviews: number;
  totalModules: number;
  completedModules: number;
  streakDays: number;
  nextModuleName: string | null;
}

export function TrackCard({
  track,
  dueReviews,
  totalModules,
  completedModules,
  streakDays,
  nextModuleName,
}: TrackCardProps) {
  const color = getTrackColor(track.topic);

  // Estimate ETA based on progress and time spent
  const etaWeeks =
    track.progressPercent > 0 && track.totalTimeSpent > 0
      ? Math.max(
          1,
          Math.ceil(
            ((track.totalTimeSpent / track.progressPercent) * (100 - track.progressPercent)) /
              (7 * 3600),
          ),
        )
      : null;

  return (
    <Link
      to={`/track/${track.id}`}
      className="glass group relative flex flex-col overflow-hidden rounded-xl transition-all hover:scale-[1.01] hover:shadow-lg"
    >
      {/* Colored top border */}
      <div className="h-1 w-full" style={{ backgroundColor: color }} />

      <div className="flex flex-col gap-4 p-5">
        {/* Header */}
        <div className="flex items-center gap-2">
          <BookOpen size={18} style={{ color }} />
          <h3 className="text-base font-semibold text-foreground">{track.topic}</h3>
          <span className="ml-auto rounded-full bg-secondary px-2.5 py-0.5 text-xs text-muted-foreground">
            {track.domainModule}
          </span>
        </div>

        {/* Progress bar */}
        <div className="space-y-1.5">
          <div className="flex justify-between text-xs text-muted-foreground">
            <span>{track.progressPercent}% complete</span>
            <span>{formatDuration(track.totalTimeSpent)}</span>
          </div>
          <div className="h-2 rounded-full bg-secondary">
            <div
              className="h-2 rounded-full transition-all"
              style={{ width: `${track.progressPercent}%`, backgroundColor: color }}
            />
          </div>
        </div>

        {/* Stats grid */}
        <div className="grid grid-cols-4 gap-3 text-center">
          <div>
            <div className="text-sm font-semibold text-foreground">
              {completedModules}/{totalModules}
            </div>
            <div className="text-[10px] text-muted-foreground">Progress</div>
          </div>
          <div>
            <div className="text-sm font-semibold" style={{ color: dueReviews > 0 ? color : undefined }}>
              {dueReviews}
            </div>
            <div className="text-[10px] text-muted-foreground">Reviews</div>
          </div>
          <div>
            <div className="flex items-center justify-center gap-0.5 text-sm font-semibold text-foreground">
              {streakDays}
              <Flame size={12} className="text-orange-400" />
            </div>
            <div className="text-[10px] text-muted-foreground">Streak</div>
          </div>
          <div>
            <div className="flex items-center justify-center gap-0.5 text-sm font-semibold text-foreground">
              {etaWeeks !== null ? `~${etaWeeks}w` : "--"}
              <Clock size={12} className="text-muted-foreground" />
            </div>
            <div className="text-[10px] text-muted-foreground">ETA</div>
          </div>
        </div>

        {/* Next module */}
        {nextModuleName && (
          <div className="flex items-center gap-1.5 rounded-lg bg-secondary/60 px-3 py-2 text-xs text-muted-foreground">
            <BarChart3 size={12} style={{ color }} />
            <span>
              Next: <span className="font-medium text-foreground">{nextModuleName}</span>
            </span>
          </div>
        )}
      </div>
    </Link>
  );
}
