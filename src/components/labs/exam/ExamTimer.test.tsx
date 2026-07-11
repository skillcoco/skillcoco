// Phase 19 (EXAM-01) Wave 0 RED scaffold — ExamTimer does not exist yet;
// this test fails at import until 19-06 builds
// `src/components/labs/exam/ExamTimer.tsx`.
//
// Asserts the 3-phase timer color-state table (19-UI-SPEC.md § Timer color
// states) and MM:SS countdown formatting using fake timers against a fixed
// `deadlineAt`.

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, render, screen } from "@testing-library/react";

// 19-06: implementer plan — module does not exist yet, so this import
// itself fails RED (module not found) until ExamTimer.tsx is created.
import { ExamTimer } from "./ExamTimer";

const NOW = new Date("2026-07-11T12:00:00.000Z");

function deadlineInMinutes(minutes: number): string {
  return new Date(NOW.getTime() + minutes * 60_000).toISOString();
}

describe("ExamTimer — Phase 19 Wave 0 (RED until 19-06)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("exam_timer_renders_remaining_mmss — 20 minutes remaining renders 20:00", () => {
    render(<ExamTimer deadlineAt={deadlineInMinutes(20)} />);
    expect(screen.getByText("20:00")).toBeInTheDocument();
  });

  it("exam_timer_normal_phase — >15min remaining uses text-foreground (Normal phase)", () => {
    render(<ExamTimer deadlineAt={deadlineInMinutes(20)} />);
    const timer = screen.getByTestId("exam-timer");
    expect(timer.className).toContain("text-foreground");
    expect(timer.className).not.toContain("text-amber-500");
    expect(timer.className).not.toContain("text-destructive");
  });

  it("exam_timer_warning_phase — <=15min & >5min remaining uses text-amber-500 (Warning phase)", () => {
    render(<ExamTimer deadlineAt={deadlineInMinutes(10)} />);
    const timer = screen.getByTestId("exam-timer");
    expect(timer.className).toContain("text-amber-500");
    expect(timer.className).not.toContain("text-destructive");
  });

  it("exam_timer_urgent_phase — <=5min remaining uses text-destructive (Urgent phase)", () => {
    render(<ExamTimer deadlineAt={deadlineInMinutes(3)} />);
    const timer = screen.getByTestId("exam-timer");
    expect(timer.className).toContain("text-destructive");
    expect(timer.className).not.toContain("text-amber-500");
    expect(timer.className).not.toContain("text-foreground");
  });

  it("exam_timer_boundary_15min_is_warning_not_normal — exactly 15:00 remaining is Warning phase", () => {
    render(<ExamTimer deadlineAt={deadlineInMinutes(15)} />);
    const timer = screen.getByTestId("exam-timer");
    expect(timer.className).toContain("text-amber-500");
  });

  it("exam_timer_boundary_5min_is_urgent_not_warning — exactly 5:00 remaining is Urgent phase", () => {
    render(<ExamTimer deadlineAt={deadlineInMinutes(5)} />);
    const timer = screen.getByTestId("exam-timer");
    expect(timer.className).toContain("text-destructive");
  });

  it("exam_timer_counts_down_on_tick — advancing 1s decrements the displayed seconds", () => {
    render(<ExamTimer deadlineAt={deadlineInMinutes(1)} />);
    expect(screen.getByText("01:00")).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(screen.getByText("00:59")).toBeInTheDocument();
  });
});
