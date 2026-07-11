// Phase 19 (EXAM-02) — post-submit results screen (D-12 full per-step
// breakdown). Copy is verbatim from 19-UI-SPEC.md Results screen table.
//
// Scoring & Verdict Treatment (locked in 19-UI-SPEC.md):
//   Pass                  -> emerald-500 CheckCircle2
//   Fail                  -> destructive XCircle (+ check-failure reason)
//   Manual / Indeterminate -> amber-500 AlertTriangle, "Could not verify
//                             automatically" — NEVER conflated with a
//                             genuine Fail even though it counts against
//                             the denominator server-side (D-12 lock).

import { CheckCircle2, XCircle, AlertTriangle } from "lucide-react";
import { cn } from "@/lib/utils";
import type { ExamAttemptResult } from "@/types/learning";

export interface AttemptHistorySummary {
  attemptNumber: number;
  totalAttempts: number;
  bestScorePercent: number;
  bestAttemptDate: string;
}

export interface ExamResultsPanelProps {
  result: ExamAttemptResult;
  passThresholdPct: number;
  /** D-06 — best-attempt history note. Absent when this is the only attempt. */
  history?: AttemptHistorySummary;
  onRetake?: () => void;
  onBackToCourse?: () => void;
}

export function ExamResultsPanel({
  result,
  passThresholdPct,
  history,
  onRetake,
  onBackToCourse,
}: ExamResultsPanelProps) {
  const score = Math.round(result.scorePercent);

  return (
    <div
      data-testid="exam-results-panel"
      className="glass-strong space-y-6 rounded-xl border border-border p-6"
    >
      <h2 className="text-lg font-semibold text-foreground">Exam results</h2>

      <div className="flex flex-col items-center gap-2">
        <div className="text-3xl font-semibold text-foreground">{score}%</div>
        {result.passed ? (
          <div className="flex items-center gap-1.5 text-sm font-medium text-emerald-500">
            <CheckCircle2 size={16} />
            Passed &middot; threshold was {passThresholdPct}%
          </div>
        ) : (
          <div className="flex items-center gap-1.5 text-sm font-medium text-destructive">
            <XCircle size={16} />
            Not passed &middot; threshold was {passThresholdPct}%
          </div>
        )}
      </div>

      {result.status === "timed_out_partial" && (
        <div
          role="status"
          data-testid="exam-timed-out-banner"
          className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-500"
        >
          Submitted automatically when time ran out &mdash; {result.stepVerdicts.length} of{" "}
          {result.totalSteps} steps were evaluated.
        </div>
      )}

      <div className="space-y-2">
        <h3 className="text-sm font-semibold text-foreground">
          Step-by-step breakdown
        </h3>
        <ul className="space-y-2">
          {result.stepVerdicts.map((verdict, i) => {
            const isManual =
              verdict.outcome === "manual" || verdict.outcome === "indeterminate";
            const isPass = verdict.outcome === "pass";
            return (
              <li
                key={verdict.stepId}
                data-testid={`exam-step-verdict-${i}`}
                className={cn(
                  "flex items-start gap-2 rounded-md border p-3 text-sm",
                  "border-border bg-card/50",
                )}
              >
                {isPass ? (
                  <CheckCircle2
                    size={16}
                    className="mt-0.5 shrink-0 text-emerald-500"
                  />
                ) : isManual ? (
                  <AlertTriangle
                    size={16}
                    className="mt-0.5 shrink-0 text-amber-500"
                  />
                ) : (
                  <XCircle
                    size={16}
                    className="mt-0.5 shrink-0 text-destructive"
                  />
                )}
                <span className="text-foreground">
                  Step {i + 1}: {verdict.title} &mdash;{" "}
                  {isPass
                    ? "Passed"
                    : isManual
                      ? "Could not verify automatically"
                      : `Failed${verdict.checkReason ? ` (${verdict.checkReason})` : ""}`}
                </span>
              </li>
            );
          })}
        </ul>
      </div>

      {history && (
        <p
          data-testid="exam-attempt-history-note"
          className="text-xs text-muted-foreground"
        >
          This is attempt {history.attemptNumber} of {history.totalAttempts}.
          Best score: {Math.round(history.bestScorePercent)}% (
          {history.bestAttemptDate}).
        </p>
      )}

      <div className="flex justify-end gap-2 pt-2">
        {onBackToCourse && (
          <button
            type="button"
            onClick={onBackToCourse}
            className="rounded-lg border border-border px-4 py-2 text-xs font-medium text-foreground transition-colors hover:bg-accent"
          >
            Back to course
          </button>
        )}
        {onRetake && (
          <button
            type="button"
            onClick={onRetake}
            className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
          >
            Retake exam
          </button>
        )}
      </div>
    </div>
  );
}
