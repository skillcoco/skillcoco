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

  it("section_mark_complete — Mark complete button calls markLessonComplete with blockId", async () => {
    const user = userEvent.setup();
    const onMarkComplete = vi.fn();
    const block = makeBlock('{"markdown":"# Lesson\\nContent.","word_count":10}');

    render(<SectionBlock block={block} onMarkComplete={onMarkComplete} />);

    // FAILS in Wave 0: placeholder doesn't render a "Mark complete" button.
    // GREEN in 03-05 Task 2.
    await user.click(screen.getByRole("button", { name: /mark complete/i }));
    expect(onMarkComplete).toHaveBeenCalledWith("blk-section-1");
  });
});
