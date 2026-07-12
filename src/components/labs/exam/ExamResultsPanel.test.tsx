// Phase 19 plan 19-07 (EXAM-02 gap closure) — locks the D-06 history-note
// render contract on the EXISTING ExamResultsPanel component before wiring
// ExamRunView to supply the `history` prop. Both tests target the current
// component as-is; if either fails, the component regressed (fix the
// component, not the test).

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ExamResultsPanel } from "./ExamResultsPanel";
import type { ExamAttemptResult } from "@/types/learning";

const baseResult: ExamAttemptResult = {
  attemptId: "exam-attempt-1",
  status: "completed",
  scorePercent: 100,
  passed: true,
  startedAt: "2026-07-10T10:00:00.000Z",
  finishedAt: "2026-07-10T10:20:00.000Z",
  deadlineAt: "2026-07-10T10:30:00.000Z",
  totalSteps: 1,
  stepVerdicts: [
    {
      stepId: "write-manifest",
      title: "Write the manifest",
      outcome: "pass",
      passedTowardScore: true,
      checkReason: null,
    },
  ],
};

describe("ExamResultsPanel — D-06 history note (19-07)", () => {
  it("renders the D-06 history note when history is supplied", () => {
    render(
      <ExamResultsPanel
        result={baseResult}
        passThresholdPct={70}
        history={{
          attemptNumber: 2,
          totalAttempts: 3,
          bestScorePercent: 88.4,
          bestAttemptDate: "2026-07-10",
        }}
      />,
    );

    const note = screen.getByTestId("exam-attempt-history-note");
    expect(note.textContent).toContain("This is attempt 2 of 3");
    expect(note.textContent).toContain("Best score: 88%");
    expect(note.textContent).toContain("2026-07-10");
  });

  it("renders no history note when history is absent", () => {
    render(<ExamResultsPanel result={baseResult} passThresholdPct={70} />);

    expect(screen.queryByTestId("exam-attempt-history-note")).toBeNull();
  });
});
