import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { BlockRenderer } from "@/components/learning/BlockRenderer";
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
    metadataJson: '{"concept_id":null}',
    retryCount: 0,
    createdAt: "2026-05-05T00:00:00Z",
    updatedAt: "2026-05-05T00:00:00Z",
    ...overrides,
  };
}

describe("BlockRenderer Phase 3 scaffolds", () => {
  it("block_renderer_renders_section — section blockType renders SectionBlock placeholder", () => {
    render(<BlockRenderer block={makeBlock({ blockType: "section" })} />);

    // FAILS in Wave 0: placeholder renders "placeholder-block-renderer", not "placeholder-section-block".
    // GREEN in 03-05 Task 2 when real switch-on-blockType is implemented.
    expect(screen.getByTestId("placeholder-section-block")).toBeInTheDocument();
  });

  it("block_renderer_renders_quiz — quiz blockType renders QuizBlock placeholder", () => {
    render(<BlockRenderer block={makeBlock({ blockType: "quiz" })} />);

    // FAILS in Wave 0: placeholder renders "placeholder-block-renderer", not "placeholder-quiz-block".
    // GREEN in 03-05 Task 2.
    expect(screen.getByTestId("placeholder-quiz-block")).toBeInTheDocument();
  });

  it("block_renderer_unknown_type — unknown blockType renders fallback with specific text", () => {
    // Cast to bypass TS narrowing for test purposes
    render(<BlockRenderer block={makeBlock({ blockType: "unknown_type" as "section" })} />);

    // FAILS in Wave 0: placeholder doesn't handle unknown types with that text.
    // GREEN in 03-05 Task 2.
    expect(screen.getByText(/unsupported block type/i)).toBeInTheDocument();
  });
});
