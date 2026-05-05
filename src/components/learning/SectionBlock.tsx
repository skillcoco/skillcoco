import { useState } from "react";
import type { ModuleBlock, SectionPayload } from "@/types/learning";
import { MarkdownRenderer } from "./MarkdownRenderer";
import { useLearningStore } from "@/stores/useLearningStore";

interface SectionBlockProps {
  block: ModuleBlock;
  moduleId?: string;
  lessonIndex?: number;
  priorCompletedCount?: number;
  /** Optional callback — if provided, called instead of the store action (useful for testing). */
  onMarkComplete?: (blockId: string) => void;
}

/**
 * Renders a section block: markdown content via MarkdownRenderer,
 * an optional skip-ahead banner (dismissible), and a "Mark complete" button.
 *
 * Skip-ahead banner is shown when lessonIndex > 0 AND priorCompletedCount < lessonIndex.
 * Re-shows on every mount (no session persistence required in Phase 3).
 *
 * Mark complete: calls onMarkComplete prop if provided, otherwise calls
 * useLearningStore.markLessonComplete. Uses optimistic UI via the store.
 */
export function SectionBlock({
  block,
  moduleId,
  lessonIndex = 0,
  priorCompletedCount = 0,
  onMarkComplete,
}: SectionBlockProps) {
  const [bannerDismissed, setBannerDismissed] = useState(false);

  const markLessonComplete = useLearningStore((s) => s.markLessonComplete);
  const lessonCompletions = useLearningStore((s) => s.lessonCompletions);
  const isCompleted = moduleId
    ? (lessonCompletions.get(moduleId)?.has(block.id) ?? false)
    : false;

  const showSkipBanner =
    lessonIndex > 0 && priorCompletedCount < lessonIndex && !bannerDismissed;

  let payload: SectionPayload;
  try {
    payload = JSON.parse(block.payloadJson) as SectionPayload;
  } catch {
    payload = { markdown: "Content unavailable." };
  }

  // Lesson title from params: prefer camelCase, fall back to snake_case for
  // legacy rows (the prompt instructs the LLM to omit any title heading from
  // the markdown — the UI surfaces it here).
  let lessonTitle: string | null = null;
  try {
    const params = JSON.parse(block.paramsJson) as Record<string, unknown>;
    const t = params.lessonTitle ?? params.lesson_title;
    if (typeof t === "string" && t.trim().length > 0) {
      lessonTitle = t;
    }
  } catch {
    /* keep null */
  }

  function handleMarkComplete() {
    if (onMarkComplete) {
      onMarkComplete(block.id);
    } else if (moduleId) {
      markLessonComplete(moduleId, block.id);
    }
  }

  // prose for typographic spacing only; colors handled per-element by
  // MarkdownRenderer (theme-aware text-foreground tokens). Removed
  // `prose-invert` — it forced light-on-light-bg in light mode.
  return (
    <article className="prose max-w-none my-6 prose-headings:text-foreground prose-p:text-foreground prose-li:text-foreground prose-strong:text-foreground">
      {showSkipBanner && (
        <div
          className="glass rounded-md p-4 mb-4 flex justify-between items-start not-prose"
          data-testid="skip-ahead-banner"
        >
          <p className="text-sm text-foreground/80 m-0">
            You haven't read prior lessons — they may be referenced.
          </p>
          <button
            type="button"
            className="ml-4 text-xs text-foreground/60 hover:text-foreground/90 shrink-0"
            onClick={() => setBannerDismissed(true)}
          >
            Dismiss
          </button>
        </div>
      )}

      {lessonTitle && (
        <header className="not-prose mb-4">
          <p className="text-xs uppercase tracking-wide text-muted-foreground">
            Lesson {lessonIndex + 1}
          </p>
          <h2 className="mt-1 text-3xl font-bold leading-tight text-foreground">
            {lessonTitle}
          </h2>
        </header>
      )}

      <MarkdownRenderer content={payload.markdown} />

      <div className="not-prose mt-8">
        <button
          type="button"
          className="glass-strong px-6 py-3 rounded-lg text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed hover:opacity-90 transition-opacity"
          data-testid="mark-complete-btn"
          disabled={isCompleted}
          onClick={handleMarkComplete}
        >
          {isCompleted ? "Completed" : "Mark complete"}
        </button>
      </div>
    </article>
  );
}
