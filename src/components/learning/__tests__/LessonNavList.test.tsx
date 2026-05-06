// Phase 03.1 Wave 0 — locks the LessonNavList exclusion contract for the
// new `lab` block type (LAB-01). Per RESEARCH q10, the existing component
// already filters to `section` only — this test asserts the behavior is
// preserved, so a future regression that adds lab to the allowlist fails
// loudly. This test is expected to PASS today (per plan critical_pitfall #7);
// flagged in the SUMMARY as "automatic via existing allowlist, no code change".

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { LessonNavList } from "@/components/learning/LessonNavList";
import type { ModuleBlock } from "@/types/learning";

function makeBlock(overrides: Partial<ModuleBlock>): ModuleBlock {
  return {
    id: "blk-1",
    moduleId: "mod-1",
    ordering: 0,
    blockType: "section",
    status: "ready",
    paramsJson: "{}",
    payloadJson: "{}",
    sourceAnchorsJson: "[]",
    metadataJson: "{}",
    retryCount: 0,
    createdAt: "2026-05-05T00:00:00Z",
    updatedAt: "2026-05-05T00:00:00Z",
    ...overrides,
  };
}

describe("LessonNavList — Phase 03.1 Wave 0 (LAB-01 exclusion)", () => {
  it("lesson_nav_list_excludes_lab_blocks — only section blocks render in nav", () => {
    const blocks: ModuleBlock[] = [
      makeBlock({
        id: "sec-1",
        ordering: 0,
        blockType: "section",
        paramsJson: JSON.stringify({ lesson_title: "Section One" }),
      }),
      makeBlock({
        id: "lab-1",
        ordering: 1,
        blockType: "lab",
        paramsJson: JSON.stringify({ lab_title: "Lab One — should NOT show" }),
      }),
      makeBlock({
        id: "sec-2",
        ordering: 2,
        blockType: "section",
        paramsJson: JSON.stringify({ lesson_title: "Section Two" }),
      }),
      makeBlock({
        id: "quiz-1",
        ordering: 3,
        blockType: "quiz",
        paramsJson: JSON.stringify({ lesson_title: "Quiz One — should NOT show" }),
      }),
    ];

    render(
      <LessonNavList
        blocks={blocks}
        moduleId="mod-1"
        currentLessonId={null}
        lessonCompletions={undefined}
        onLessonClick={() => {}}
      />,
    );

    // Both section titles must be visible.
    expect(screen.getByText("Section One")).toBeInTheDocument();
    expect(screen.getByText("Section Two")).toBeInTheDocument();
    // Lab and quiz titles must NOT appear.
    expect(screen.queryByText(/Lab One/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Quiz One/)).not.toBeInTheDocument();
    // Per-row testid only for sections (sec-1, sec-2).
    expect(screen.queryByTestId("lesson-row-lab-1")).not.toBeInTheDocument();
    expect(screen.queryByTestId("lesson-row-quiz-1")).not.toBeInTheDocument();
    expect(screen.getByTestId("lesson-row-sec-1")).toBeInTheDocument();
    expect(screen.getByTestId("lesson-row-sec-2")).toBeInTheDocument();
  });
});
