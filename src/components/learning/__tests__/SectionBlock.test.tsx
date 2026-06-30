import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Vitest hoisting rule: inline literals only inside vi.mock factory.
vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn(() => ({
    markLessonComplete: vi.fn(),
  })),
}));

import { SectionBlock } from "@/components/learning/SectionBlock";
import type { ModuleBlock } from "@/types/learning";

function makeBlock(payloadJson: string): ModuleBlock {
  return {
    id: "blk-section-1",
    moduleId: "mod-1",
    ordering: 0,
    blockType: "section",
    status: "ready",
    paramsJson: '{"lesson_title":"Introduction"}',
    payloadJson,
    sourceAnchorsJson: "[]",
    metadataJson: '{"concept_id":null}',
    retryCount: 0,
    createdAt: "2026-05-05T00:00:00Z",
    updatedAt: "2026-05-05T00:00:00Z",
  };
}

describe("SectionBlock Phase 3 scaffolds", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("section_renders_lesson_title — paramsJson.lessonTitle appears as h2 heading above content", () => {
    const block: ModuleBlock = {
      ...makeBlock('{"markdown":"Body text only.","word_count":3}'),
      paramsJson: '{"lessonTitle":"Introduction to Pods"}',
    };
    render(<SectionBlock block={block} lessonIndex={0} />);

    const heading = screen.getByRole("heading", {
      name: /introduction to pods/i,
      level: 2,
    });
    expect(heading).toBeInTheDocument();
    // Lesson number prefix when index provided
    expect(screen.getByText(/lesson\s*1/i)).toBeInTheDocument();
  });

  it("section_renders_lesson_title_snake_case — also accepts lesson_title (snake_case fallback)", () => {
    const block: ModuleBlock = {
      ...makeBlock('{"markdown":"Body.","word_count":1}'),
      paramsJson: '{"lesson_title":"Snake Case Title"}',
    };
    render(<SectionBlock block={block} lessonIndex={2} />);

    expect(
      screen.getByRole("heading", { name: /snake case title/i, level: 2 })
    ).toBeInTheDocument();
    expect(screen.getByText(/lesson\s*3/i)).toBeInTheDocument();
  });

  it("section_renders_markdown — payload markdown renders as HTML heading", () => {
    const block = makeBlock('{"markdown":"# Hello\\nSome content.","word_count":5}');
    render(<SectionBlock block={block} />);

    // FAILS in Wave 0: placeholder renders "Not implemented", not rendered markdown.
    // GREEN in 03-05 Task 2 when MarkdownRenderer is wired up.
    expect(screen.getByRole("heading", { name: "Hello" })).toBeInTheDocument();
  });

  it("section_skip_ahead_banner — lessonIndex > 0 and 0 completions shows banner", () => {
    const block = makeBlock('{"markdown":"# Lesson 5\\nContent.","word_count":10}');

    render(<SectionBlock block={block} lessonIndex={4} priorCompletedCount={0} />);

    // FAILS in Wave 0: placeholder doesn't render the skip-ahead banner.
    // GREEN in 03-05 Task 2.
    expect(screen.getByText(/haven't read prior lessons/i)).toBeInTheDocument();
  });

  it("section_skip_ahead_banner — lessonIndex 0 does NOT show banner", () => {
    const block = makeBlock('{"markdown":"# Lesson 1\\nContent.","word_count":10}');

    render(<SectionBlock block={block} lessonIndex={0} priorCompletedCount={0} />);

    // FAILS in Wave 0: placeholder doesn't conditionally hide the banner.
    // GREEN in 03-05 Task 2.
    expect(screen.queryByText(/haven't read prior lessons/i)).not.toBeInTheDocument();
  });

  it("section_mark_complete — DailyChallenge context renders in-body button that calls onMarkComplete with blockId", async () => {
    // Phase 10-02: the in-body button now renders ONLY in the DailyChallenge
    // context (signaled by onComplete/onMarkComplete being provided). ModuleView
    // hosts its own footer control instead and passes neither prop.
    const user = userEvent.setup();
    const onMarkComplete = vi.fn();
    const block = makeBlock('{"markdown":"# Lesson\\nContent.","word_count":10}');

    render(<SectionBlock block={block} onMarkComplete={onMarkComplete} />);

    await user.click(screen.getByRole("button", { name: /mark complete/i }));
    expect(onMarkComplete).toHaveBeenCalledWith("blk-section-1");
  });

  // ── Phase 4 Wave 4 (04-05 Task 1) — optional onComplete prop ──

  it("section_on_complete_fires — onComplete callback fires after in-body mark-complete click (DailyChallenge context)", async () => {
    const user = userEvent.setup();
    const onComplete = vi.fn();
    const onMarkComplete = vi.fn();
    const block = makeBlock('{"markdown":"# Lesson\\nContent.","word_count":10}');

    render(
      <SectionBlock
        block={block}
        onMarkComplete={onMarkComplete}
        onComplete={onComplete}
      />,
    );

    await user.click(screen.getByRole("button", { name: /mark complete/i }));

    expect(onMarkComplete).toHaveBeenCalledWith("blk-section-1");
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  // ── Phase 10 Plan 02: in-body mark-complete-btn removed from ModuleView context ──

  it("section_no_in_body_mark_complete_btn — ModuleView context (no callbacks) must NOT render in-body mark-complete-btn", () => {
    const block = makeBlock('{"markdown":"# Lesson\\nContent.","word_count":10}');
    // ModuleView passes neither onComplete nor onMarkComplete — completion is
    // hosted by the lesson footer (mark-read-advance-btn) instead (D-05).
    render(<SectionBlock block={block} />);

    expect(screen.queryByTestId("mark-complete-btn")).not.toBeInTheDocument();
  });

  it("section_daily_challenge_onComplete_path_preserved — DailyChallenge context still renders the in-body button (regression guard)", async () => {
    // Regression guard (Rule 1): the DailyChallenge section flow advances ONLY
    // via SectionBlock's in-body button firing onComplete. Removing the button
    // unconditionally would break DailyChallenge; it must remain in this context.
    const user = userEvent.setup();
    const onComplete = vi.fn();
    const block = makeBlock('{"markdown":"# Lesson\\nContent.","word_count":10}');

    render(<SectionBlock block={block} onComplete={onComplete} />);

    expect(screen.getByTestId("mark-complete-btn")).toBeInTheDocument();
    await user.click(screen.getByTestId("mark-complete-btn"));
    expect(onComplete).toHaveBeenCalledTimes(1);
  });
});
