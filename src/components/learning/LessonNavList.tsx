import type { ModuleBlock } from "@/types/learning";
import { cn } from "@/lib/utils";

interface LessonNavListProps {
  blocks: ModuleBlock[];
  moduleId: string;
  currentLessonId: string | null;
  lessonCompletions: Set<string> | undefined;
  onLessonClick: (blockId: string) => void;
}

export function LessonNavList({
  blocks,
  moduleId,
  currentLessonId,
  lessonCompletions,
  onLessonClick,
}: LessonNavListProps) {
  // Only show section blocks in the lesson list
  const sections = blocks.filter((b) => b.blockType === "section");

  return (
    <ul
      className="ml-5 my-1 space-y-0.5 border-l border-border pl-2"
      data-testid={`lesson-nav-list-${moduleId}`}
    >
      {sections.map((b, i) => {
        const isActive = currentLessonId === b.id;
        const isCompleted = lessonCompletions?.has(b.id) ?? false;

        let titleFromParams = `Lesson ${i + 1}`;
        try {
          const params = JSON.parse(b.paramsJson) as Record<string, unknown>;
          if (typeof params.lesson_title === "string") {
            titleFromParams = params.lesson_title;
          } else if (typeof params.lessonTitle === "string") {
            titleFromParams = params.lessonTitle;
          }
        } catch {
          // keep default
        }

        return (
          <li key={b.id}>
            <button
              onClick={() => onLessonClick(b.id)}
              data-testid={`lesson-row-${b.id}`}
              className={cn(
                "w-full text-left flex items-center gap-2 text-xs py-1.5 px-2 rounded transition-colors",
                isActive
                  ? "bg-primary/10 text-foreground"
                  : "text-muted-foreground hover:bg-secondary/60 hover:text-foreground"
              )}
            >
              <span className="flex-shrink-0 w-4 text-right text-foreground/40">
                {i + 1}.
              </span>
              <StatusIcon status={b.status} completed={isCompleted} />
              <span className="flex-1 truncate">{titleFromParams}</span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

function StatusIcon({
  status,
  completed,
}: {
  status: string;
  completed: boolean;
}) {
  if (status === "failed") {
    return (
      <span
        data-testid="status-failed"
        className="flex-shrink-0 h-2 w-2 rounded-full bg-red-400"
        aria-label="Failed"
      />
    );
  }
  if (completed) {
    return (
      <span
        data-testid="status-completed"
        className="flex-shrink-0 h-2 w-2 rounded-full bg-green-400"
        aria-label="Completed"
      />
    );
  }
  if (status === "generating" || status === "pending") {
    return (
      <span
        data-testid="status-generating"
        className="flex-shrink-0 h-2 w-2 rounded-full bg-blue-400 animate-pulse"
        aria-label="Generating"
      />
    );
  }
  // status === "ready" and not completed
  return (
    <span
      data-testid="status-ready"
      className="flex-shrink-0 h-2 w-2 rounded-full bg-foreground/30"
      aria-label="Available"
    />
  );
}
