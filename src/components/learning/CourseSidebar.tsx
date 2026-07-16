import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  Lock,
  CheckCircle2,
  PlayCircle,
  Circle,
  ChevronLeft,
  ChevronDown,
  ChevronRight,
  Compass,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type {
  LearningTrack,
  PathModule,
  ModuleProgress,
} from "@/types/learning";
import { useLearningStore } from "@/stores/useLearningStore";
import { LessonNavList } from "./LessonNavList";
import { pickNextModule } from "@/lib/learning-path";

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
 * in the track with its status and mastery. Phase 3 extends with collapsible
 * per-module lesson sub-lists (LessonNavList component).
 *
 * Each ModuleNavItem can expand to show 8-10 section blocks for that module.
 * The active module auto-expands on mount if blocks are already cached.
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

  // Phase 10 Plan 03 (D-08) — recommended-next module for guidance in both modes.
  // In free mode this is the primary signal; in linear mode it's a secondary hint.
  // Uses the existing pickNextModule: in_progress ?? first available ?? null.
  const recommendedNextId = useMemo(() => {
    const next = pickNextModule(
      rows.map((r) => r.module),
      (id) => rows.find((r) => r.module.id === id)?.status ?? "locked",
    );
    return next?.id ?? null;
  }, [rows]);

  // Phase 10 Plan 03 (D-07) — in free mode every row is treated as openable
  // regardless of its DB status. browseMode === undefined → default linear.
  const isFreeMode = track.browseMode === "free";

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
            // Phase 10 Plan 03 (D-07): in free mode, all rows are openable.
            // In linear mode (or undefined = default linear), use DB status.
            const isLocked = isFreeMode ? false : row.status === "locked";
            const isRecommendedNext = row.module.id === recommendedNextId;
            return (
              <li key={row.module.id}>
                <ModuleNavItem
                  index={index + 1}
                  row={row}
                  trackId={track.id}
                  isActive={isActive}
                  isLocked={isLocked}
                  isRecommendedNext={isRecommendedNext}
                />
              </li>
            );
          })}
        </ul>
      </nav>
    </aside>
  );
}

function ModuleNavItem({
  index,
  row,
  trackId,
  isActive,
  isLocked,
  isRecommendedNext,
}: {
  index: number;
  row: ModuleRow;
  trackId: string;
  isActive: boolean;
  isLocked: boolean;
  isRecommendedNext?: boolean;
}) {
  const navigate = useNavigate();

  // Phase 3 store state
  const moduleBlocks = useLearningStore((s) => s.moduleBlocks);
  const currentLessonId = useLearningStore((s) => s.currentLessonId);
  const lessonCompletions = useLearningStore((s) => s.lessonCompletions);
  const setCurrentLesson = useLearningStore((s) => s.setCurrentLesson);
  const loadModuleBlocks = useLearningStore((s) => s.loadModuleBlocks);

  // Active module auto-expands if blocks already cached; other modules start collapsed
  const hasBlocks = moduleBlocks.has(row.module.id);
  const [expanded, setExpanded] = useState<boolean>(isActive && hasBlocks);

  const blocks = moduleBlocks.get(row.module.id) ?? [];
  const completionsForModule = lessonCompletions.get(row.module.id);

  async function toggleExpand() {
    const nextExpanded = !expanded;
    setExpanded(nextExpanded);

    // Fetch blocks on first expand (cache miss)
    if (nextExpanded && !moduleBlocks.has(row.module.id)) {
      await loadModuleBlocks(row.module.id);
    }
  }

  function handleLessonClick(blockId: string) {
    setCurrentLesson(blockId);
    navigate(`/track/${trackId}/module/${row.module.id}`);
  }

  // Opening the module is the primary row action (LMS convention + matches the
  // "Continue to next module" Link in ModuleView). ModuleView lazily loads and
  // generates this module's blocks on mount, so navigating is enough to enter a
  // not-yet-started "available" module — the lesson sub-list (chevron) is a
  // secondary affordance and is an empty dead-end before blocks are generated.
  function openModule() {
    navigate(`/track/${trackId}/module/${row.module.id}`);
  }

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
          !isActive && row.status === "in_progress" && "bg-accent/20 text-accent",
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
        {/* Phase 10 Plan 03 (D-08) — recommended-next hint shown in both modes.
            Primary guidance signal in free mode; complementary in linear mode. */}
        {isRecommendedNext && (
          <span
            data-testid="recommended-next"
            className="mt-1 inline-flex items-center gap-1 text-[10px] font-medium text-primary"
          >
            <Compass size={10} aria-hidden />
            Recommended next
          </span>
        )}
      </span>
    </div>
  );

  return (
    <>
      {isLocked ? (
        <div aria-disabled className="block">
          {inner}
        </div>
      ) : (
        <div className="flex items-stretch">
          <button
            onClick={openModule}
            className="min-w-0 flex-1 text-left block"
            data-testid={`module-row-${row.module.id}`}
          >
            {inner}
          </button>
          <button
            type="button"
            onClick={toggleExpand}
            aria-label={`Toggle lessons for ${row.module.title}`}
            data-testid={`module-expand-${row.module.id}`}
            className="flex flex-shrink-0 items-center px-2 text-muted-foreground/50 transition-colors hover:bg-secondary/60 hover:text-muted-foreground"
          >
            {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          </button>
        </div>
      )}

      {/* Expandable lesson sub-list */}
      {expanded && (
        <LessonNavList
          blocks={blocks}
          moduleId={row.module.id}
          currentLessonId={currentLessonId}
          lessonCompletions={completionsForModule}
          onLessonClick={handleLessonClick}
        />
      )}
    </>
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
