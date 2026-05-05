import { useMemo } from "react";
import { Link } from "react-router-dom";
import {
  Lock,
  CheckCircle2,
  PlayCircle,
  Circle,
  ChevronLeft,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type {
  LearningTrack,
  PathModule,
  ModuleProgress,
} from "@/types/learning";

interface CourseSidebarProps {
  track: LearningTrack;
  modules: PathModule[];
  progress: ModuleProgress[];
  currentModuleId?: string;
}

type DerivedStatus = "locked" | "available" | "in_progress" | "completed";

interface ModuleRow {
  module: PathModule;
  status: DerivedStatus;
  masteryPercent: number;
}

/**
 * LMS-style course sidebar (Udemy/Coursera convention). Lists every module
 * in the track with its status and mastery, plus the active module
 * highlighted. Click any unlocked module to navigate.
 *
 * Phase 3 will extend this with a collapsible 8-10-lesson list under each
 * module (block taxonomy / `section` blocks). For Phase 1 it shows just
 * the module level — the data model doesn't yet have lessons.
 */
export function CourseSidebar({
  track,
  modules,
  progress,
  currentModuleId,
}: CourseSidebarProps) {
  const rows: ModuleRow[] = useMemo(() => {
    const progressByModule = new Map(progress.map((p) => [p.moduleId, p]));
    return modules.map((module) => {
      const p = progressByModule.get(module.id);
      const status: DerivedStatus = !p
        ? "available"
        : p.status === "completed"
          ? "completed"
          : p.status === "locked"
            ? "locked"
            : p.status === "in_progress"
              ? "in_progress"
              : "available";
      return {
        module,
        status,
        masteryPercent: Math.round((p?.masteryLevel ?? 0) * 100),
      };
    });
  }, [modules, progress]);

  const completedCount = rows.filter((r) => r.status === "completed").length;
  const overallPercent =
    rows.length === 0 ? 0 : Math.round((completedCount / rows.length) * 100);

  return (
    <aside className="hidden h-screen w-72 flex-shrink-0 border-r border-border bg-secondary/20 lg:flex lg:flex-col">
      {/* Track header */}
      <div className="border-b border-border px-4 py-4">
        <Link
          to="/"
          className="mb-3 inline-flex items-center gap-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          <ChevronLeft size={14} />
          <span>All tracks</span>
        </Link>
        <h2 className="text-base font-semibold leading-snug text-foreground">
          {track.topic}
        </h2>
        {track.goal && (
          <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
            {track.goal}
          </p>
        )}

        {/* Track progress bar */}
        <div className="mt-3 space-y-1">
          <div className="flex items-center justify-between text-[10px] uppercase tracking-wide text-muted-foreground">
            <span>Course Progress</span>
            <span>
              {completedCount} of {rows.length}
            </span>
          </div>
          <div className="h-1.5 overflow-hidden rounded-full bg-secondary">
            <div
              className="h-full rounded-full bg-primary transition-all duration-500"
              style={{ width: `${overallPercent}%` }}
            />
          </div>
        </div>
      </div>

      {/* Module list */}
      <nav
        className="flex-1 overflow-y-auto px-2 py-3"
        aria-label="Course modules"
      >
        <ul className="space-y-1">
          {rows.map((row, index) => {
            const isActive = row.module.id === currentModuleId;
            const isLocked = row.status === "locked";
            return (
              <li key={row.module.id}>
                <ModuleNavItem
                  index={index + 1}
                  row={row}
                  trackId={track.id}
                  isActive={isActive}
                  isLocked={isLocked}
                />
              </li>
            );
          })}
        </ul>
      </nav>

      {/* Footer hint about Phase 3 */}
      <div className="border-t border-border px-4 py-2 text-[10px] text-muted-foreground/70">
        Lessons within each module — coming in Phase 3
      </div>
    </aside>
  );
}

function ModuleNavItem({
  index,
  row,
  trackId,
  isActive,
  isLocked,
}: {
  index: number;
  row: ModuleRow;
  trackId: string;
  isActive: boolean;
  isLocked: boolean;
}) {
  const StatusIcon = pickStatusIcon(row.status);
  const inner = (
    <div
      className={cn(
        "flex items-start gap-2.5 rounded-md px-2.5 py-2 text-sm transition-colors",
        isActive && "bg-primary/10 text-foreground",
        !isActive && !isLocked && "text-foreground hover:bg-secondary/60",
        isLocked && "text-muted-foreground/70 cursor-not-allowed"
      )}
      aria-current={isActive ? "page" : undefined}
    >
      <span
        className={cn(
          "mt-0.5 flex h-5 w-5 flex-shrink-0 items-center justify-center rounded-full text-[10px] font-semibold",
          isActive && "bg-primary text-primary-foreground",
          !isActive && row.status === "completed" && "bg-green-500/20 text-green-700 dark:text-green-400",
          !isActive && row.status === "in_progress" && "bg-orange-500/20 text-orange-700 dark:text-orange-400",
          !isActive && row.status === "available" && "bg-secondary text-muted-foreground",
          !isActive && isLocked && "bg-secondary/50 text-muted-foreground/50"
        )}
        aria-hidden
      >
        {index}
      </span>
      <span className="flex-1 min-w-0">
        <span
          className={cn(
            "block truncate font-medium",
            isActive && "text-foreground"
          )}
        >
          {row.module.title}
        </span>
        <span className="mt-0.5 flex items-center gap-1.5 text-[11px] text-muted-foreground">
          <StatusIcon size={11} aria-hidden />
          <span>{statusLabel(row.status)}</span>
          {row.masteryPercent > 0 && row.status !== "locked" && (
            <span className="ml-auto text-[10px]">
              {row.masteryPercent}% mastery
            </span>
          )}
        </span>
      </span>
    </div>
  );

  if (isLocked) {
    return (
      <div aria-disabled className="block">
        {inner}
      </div>
    );
  }
  return (
    <Link to={`/track/${trackId}/module/${row.module.id}`} className="block">
      {inner}
    </Link>
  );
}

function pickStatusIcon(status: DerivedStatus) {
  switch (status) {
    case "completed":
      return CheckCircle2;
    case "in_progress":
      return PlayCircle;
    case "locked":
      return Lock;
    default:
      return Circle;
  }
}

function statusLabel(status: DerivedStatus): string {
  switch (status) {
    case "completed":
      return "Completed";
    case "in_progress":
      return "In progress";
    case "locked":
      return "Locked";
    default:
      return "Available";
  }
}
