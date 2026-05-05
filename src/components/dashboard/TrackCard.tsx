import { useState } from "react";
import { Link } from "react-router-dom";
import { BookOpen, BarChart3, Flame, Clock, Trash2, Loader2 } from "lucide-react";
import type { LearningTrack } from "@/types";
import { formatDuration } from "@/lib/utils";
import { useLearningStore } from "@/stores/useLearningStore";
import { cn } from "@/lib/utils";

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
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const deleteTrack = useLearningStore((s) => s.deleteTrack);

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

  const handleOpenConfirm = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setConfirmOpen(true);
    setError(null);
  };

  const handleCancel = () => {
    if (isDeleting) return;
    setConfirmOpen(false);
  };

  const handleConfirmDelete = async () => {
    setIsDeleting(true);
    setError(null);
    try {
      await deleteTrack(track.id);
      setConfirmOpen(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <>
      <Link
        to={`/track/${track.id}`}
        className="glass group relative flex flex-col overflow-hidden rounded-xl transition-all hover:scale-[1.01] hover:shadow-lg"
      >
        {/* Colored top border */}
        <div className="h-1 w-full" style={{ backgroundColor: color }} />

        {/* Delete button — fixed top-right inside the card; preventDefault stops Link nav */}
        <button
          onClick={handleOpenConfirm}
          aria-label="Delete track"
          className={cn(
            "absolute right-2 top-3 z-10 flex h-7 w-7 items-center justify-center rounded-md",
            "text-muted-foreground/60 opacity-0 transition-all hover:bg-destructive/10 hover:text-destructive",
            "group-hover:opacity-100 focus:opacity-100"
          )}
        >
          <Trash2 size={14} />
        </button>

        <div className="flex flex-col gap-4 p-5">
          {/* Header */}
          <div className="flex items-center gap-2">
            <BookOpen size={18} style={{ color }} />
            <h3 className="text-base font-semibold text-foreground">{track.topic}</h3>
            <span className="ml-auto mr-7 rounded-full bg-secondary px-2.5 py-0.5 text-xs text-muted-foreground">
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

      {confirmOpen && (
        <div
          role="dialog"
          aria-labelledby={`delete-track-title-${track.id}`}
          className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm"
          onClick={handleCancel}
        >
          <div
            className="glass-strong w-full max-w-sm rounded-xl border border-border p-6 shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <h2
              id={`delete-track-title-${track.id}`}
              className="text-lg font-semibold text-foreground"
            >
              Delete this track?
            </h2>
            <p className="mt-2 text-sm text-muted-foreground">
              <span className="font-medium text-foreground">{track.topic}</span>
              {" — "}all modules, lessons, progress, and review cards for this track will be
              removed. This action cannot be undone.
            </p>

            {error && (
              <div className="mt-3 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {error}
              </div>
            )}

            <div className="mt-5 flex justify-end gap-2">
              <button
                onClick={handleCancel}
                disabled={isDeleting}
                className="rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:bg-secondary disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={handleConfirmDelete}
                disabled={isDeleting}
                className="flex items-center gap-1.5 rounded-md bg-destructive px-3 py-1.5 text-sm font-medium text-destructive-foreground transition-colors hover:bg-destructive/90 disabled:opacity-50"
              >
                {isDeleting && <Loader2 size={14} className="animate-spin" />}
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
