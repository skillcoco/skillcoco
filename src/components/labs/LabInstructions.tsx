// Phase 03.1 plan 03.1-06 — ordered all-visible step list with active-step
// highlight, completion checkmark, and per-step Show-hint button.
//
// Per CONTEXT.md "Steps = ordered, all visible. Learners see the full
// sequence from the start; the evaluator marks them complete in order.
// Read-ahead is allowed."

import { Check } from "lucide-react";
import { MarkdownRenderer } from "@/components/learning/MarkdownRenderer";
import type { LabSpec } from "@/types/learning";
import { cn } from "@/lib/utils";

export interface LabInstructionsProps {
  spec: LabSpec;
  /** 0-based index of the currently-active step. */
  currentStep: number;
  /** Step ids that have been marked complete. */
  completedStepIds: string[];
  /** Manual hint reveal handler — receives the 0-based step index. */
  onShowHint?: (stepIndex: number) => void;
  /**
   * Phase 19 (EXAM-01/D-11) — when true, suppresses the completion
   * checkmark for every step (blind scoring) AND the Show-hint button,
   * even if `onShowHint` is passed. Step number, title, and active-step
   * highlight still render (position stays visible). LabBlock's D-10
   * zero-diff gate (omitting `onShowHint` entirely) is the primary path;
   * this in-component gate is defense-in-depth (T-19-07) so a caller
   * that forgets to omit the handler still can't leak hints in exam
   * mode. Defaults to false so regular (non-exam) labs are byte-identical
   * (D-13).
   */
  examMode?: boolean;
}

export function LabInstructions({
  spec,
  currentStep,
  completedStepIds,
  onShowHint,
  examMode = false,
}: LabInstructionsProps) {
  const completed = new Set(completedStepIds);
  return (
    <ol
      data-testid="lab-instructions"
      className="flex flex-col gap-3 p-4"
    >
      {spec.steps.map((step, i) => {
        const isActive = i === currentStep;
        const isCompleted = completed.has(step.id);
        return (
          <li
            key={step.id}
            data-testid={`lab-step-${i}`}
            data-active={isActive ? "true" : "false"}
            data-completed={isCompleted ? "true" : "false"}
            className={cn(
              "rounded-md border p-3 transition-colors",
              isActive
                ? "border-primary/60 bg-primary/5"
                : "border-border bg-card/50",
            )}
          >
            <div className="mb-2 flex items-start justify-between gap-2">
              <h4 className="flex items-center gap-2 text-sm font-semibold text-foreground">
                <span className="text-muted-foreground">
                  {String(i + 1).padStart(2, "0")}.
                </span>
                <span>{step.title}</span>
                {isCompleted && !examMode ? (
                  <Check
                    className="h-4 w-4 text-success"
                    aria-label="Completed"
                    data-testid={`lab-step-${i}-check`}
                  />
                ) : null}
              </h4>
              {onShowHint && !examMode ? (
                <button
                  type="button"
                  onClick={() => onShowHint(i)}
                  className="shrink-0 rounded-md border border-border bg-background px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                >
                  Show hint
                </button>
              ) : null}
            </div>
            <div className="text-sm text-muted-foreground">
              <MarkdownRenderer content={step.prompt} />
            </div>
          </li>
        );
      })}
    </ol>
  );
}
