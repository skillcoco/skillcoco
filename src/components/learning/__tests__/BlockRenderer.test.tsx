import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock store for SectionBlock + BlockRenderer consumption.
// useLearningStore is called with a selector function in SectionBlock/BlockRenderer,
// so we use vi.hoisted to share the mock state and support selector calls.
const mockStoreState = vi.hoisted(() => ({
  markLessonComplete: vi.fn(),
  lessonCompletions: new Map<string, Set<string>>(),
  regenerateLesson: vi.fn(),
}));

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn((selector: (s: typeof mockStoreState) => unknown) =>
    typeof selector === "function" ? selector(mockStoreState) : mockStoreState
  ),
}));

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
    payloadJson: '{"markdown":"# Test\\nContent here.","word_count":3}',
    sourceAnchorsJson: "[]",
    metadataJson: '{"concept_id":null}',
    retryCount: 0,
    createdAt: "2026-05-05T00:00:00Z",
    updatedAt: "2026-05-05T00:00:00Z",
    ...overrides,
  };
}

describe("BlockRenderer Phase 3", () => {
  it("block_renderer_renders_section — section blockType renders SectionBlock (has mark-complete button)", () => {
    render(
      <BlockRenderer
        block={makeBlock({ blockType: "section" })}
        moduleId="mod-1"
      />
    );
    // SectionBlock renders a mark-complete button
    expect(screen.getByRole("button", { name: /mark complete/i })).toBeInTheDocument();
  });

  it("block_renderer_renders_quiz — quiz blockType renders QuizBlock placeholder", () => {
    render(
      <BlockRenderer
        block={makeBlock({
          blockType: "quiz",
          payloadJson: '{"questions":[]}',
        })}
        moduleId="mod-1"
      />
    );
    // QuizBlock placeholder from Wave 0
    expect(screen.getByTestId("placeholder-quiz-block")).toBeInTheDocument();
  });

  it("block_renderer_unknown_type — unknown blockType renders fallback with specific text", () => {
    render(
      <BlockRenderer
        block={makeBlock({ blockType: "unknown_type" as "section" })}
        moduleId="mod-1"
      />
    );
    expect(screen.getByText(/unsupported block type/i)).toBeInTheDocument();
  });

  it("block_renderer_routes_text_block — text blockType renders TextBlock", () => {
    render(
      <BlockRenderer
        block={makeBlock({ blockType: "text" })}
        moduleId="mod-1"
      />
    );
    // TextBlock renders markdown content; no mark-complete button
    expect(screen.queryByRole("button", { name: /mark complete/i })).not.toBeInTheDocument();
  });

  it("block_renderer_routes_callout_block — callout blockType renders CalloutBlock", () => {
    render(
      <BlockRenderer
        block={makeBlock({
          blockType: "callout",
          payloadJson: '{"variant":"warning","title":"Warning","body":"Be careful!"}',
        })}
        moduleId="mod-1"
      />
    );
    expect(screen.getByTestId("callout-block")).toBeInTheDocument();
  });

  it("block_renderer_shows_skeleton_for_generating — generating status renders skeleton chip", () => {
    render(
      <BlockRenderer
        block={makeBlock({ status: "generating" })}
        moduleId="mod-1"
      />
    );
    expect(screen.getByText(/generating/i)).toBeInTheDocument();
  });

  it("block_renderer_shows_retry_for_failed — failed status renders retry card", () => {
    render(
      <BlockRenderer
        block={makeBlock({ status: "failed" })}
        moduleId="mod-1"
      />
    );
    expect(screen.getByTestId("block-retry-card")).toBeInTheDocument();
  });
});
