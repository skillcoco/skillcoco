import { useEffect } from "react";
import { useParams, Link } from "react-router-dom";
import { useLearningStore } from "@/stores/useLearningStore";
import { ArrowLeft, Play, CheckCircle2, Lock, Circle } from "lucide-react";
import { cn } from "@/lib/utils";

const statusIcons = {
  locked: Lock,
  available: Circle,
  in_progress: Play,
  completed: CheckCircle2,
  skipped: Circle,
};

export function TrackView() {
  const { trackId } = useParams<{ trackId: string }>();
  const { currentTrack, currentPath, moduleProgress, selectTrack, isLoading } = useLearningStore();

  useEffect(() => {
    if (trackId) selectTrack(trackId);
  }, [trackId]);

  if (isLoading || !currentTrack) {
    return (
      <div className="flex h-64 items-center justify-center text-muted-foreground">
        Loading track...
      </div>
    );
  }

  const progressMap = new Map(moduleProgress.map((p) => [p.moduleId, p]));

  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <div className="flex items-center gap-3">
        <Link to="/" className="rounded-md p-1.5 text-muted-foreground hover:bg-accent">
          <ArrowLeft size={18} />
        </Link>
        <div>
          <h1 className="text-2xl font-bold text-foreground">{currentTrack.topic}</h1>
          <p className="text-sm text-muted-foreground">{currentTrack.goal}</p>
        </div>
      </div>

      {/* Module List (simplified - DAG visualization is a Phase 1 deliverable) */}
      <div className="space-y-2">
        <h2 className="text-lg font-semibold text-foreground">Learning Path</h2>
        {currentPath?.modules.map((mod, index) => {
          const progress = progressMap.get(mod.id);
          const status = progress?.status ?? "locked";
          const StatusIcon = statusIcons[status] ?? Circle;

          return (
            <Link
              key={mod.id}
              to={status !== "locked" ? `/track/${trackId}/module/${mod.id}` : "#"}
              className={cn(
                "flex items-center gap-4 rounded-lg border border-border p-4 transition-colors",
                status === "locked"
                  ? "cursor-not-allowed opacity-50"
                  : "hover:bg-accent cursor-pointer",
                status === "completed" && "border-green-200 bg-green-50/50 dark:border-green-900 dark:bg-green-950/20"
              )}
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full bg-secondary text-sm font-medium">
                {index + 1}
              </div>
              <div className="flex-1">
                <div className="font-medium text-foreground">{mod.title}</div>
                <div className="text-sm text-muted-foreground">{mod.description}</div>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs text-muted-foreground">{mod.estimatedMinutes}m</span>
                <StatusIcon
                  size={18}
                  className={cn(
                    status === "completed" && "text-green-600",
                    status === "in_progress" && "text-blue-600",
                    status === "available" && "text-foreground",
                    status === "locked" && "text-muted-foreground"
                  )}
                />
              </div>
            </Link>
          );
        })}
      </div>
    </div>
  );
}
