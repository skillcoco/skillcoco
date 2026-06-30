import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock store for SectionBlock + BlockRenderer consumption.
// useLearningStore is called with a selector function in SectionBlock/BlockRenderer,
// so we use vi.hoisted to share the mock state and support selector calls.
const mockStoreState = vi.hoisted(() => ({
  markLessonComplete: vi.fn(),
  lessonCompletions: new Map<string, Set<string>>(),
  regenerateLesson: vi.fn(),
  submitQuiz: vi.fn(),
  // QuizBlock looks up prior mastery from moduleProgress; default empty.
  moduleProgress: [],
}));

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn((selector: (s: typeof mockStoreState) => unknown) =>
    typeof selector === "function" ? selector(mockStoreState) : mockStoreState
  ),
}));

// Phase 03.1 Wave 0 — mock LabBlock so the dispatch test asserts routing
// without depending on the real LabBlock implementation (which lands in
// 03.1-06). FAILS today because BlockRenderer has no `case "lab":` arm.
vi.mock("@/components/labs/LabBlock", () => ({
  LabBlock: vi.fn(() => <div data-testid="lab-block-stub" />),
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
  it("block_renderer_renders_section — section blockType renders SectionBlock content; in-body mark-complete button suppressed without completion callbacks (10-02 footer relocation)", () => {
    render(
      <BlockRenderer
        block={makeBlock({ blockType: "section" })}
        moduleId="mod-1"
      />
    );
    // SectionBlock routes and renders the section markdown content.
    expect(screen.getByText(/content here/i)).toBeInTheDocument();
    // Phase 10-02 relocated the mark-complete control to the ModuleView footer.
    // The in-body button now renders only when DailyChallenge threads
    // onComplete/onMarkComplete; the bare BlockRenderer path (ModuleView) omits it.
    expect(screen.queryByRole("button", { name: /mark complete/i })).not.toBeInTheDocument();
  });

  it("block_renderer_renders_quiz — quiz blockType renders QuizBlock (empty quiz error state)", () => {
    render(
      <BlockRenderer
        block={makeBlock({
          blockType: "quiz",
          payloadJson: '{"questions":[]}',
        })}
        moduleId="mod-1"
      />
    );
    // Empty quiz payload renders the quiz-empty error state (implemented in 03-06 Task 1)
    expect(screen.getByTestId("quiz-empty")).toBeInTheDocument();
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

  // ── Phase 03.1 Wave 0 — failing scaffolds (LAB-01) ──

  it("block_renderer_routes_lab_block — lab blockType renders LabBlock", () => {
    // FAILS until 03.1-06 adds `case "lab":` arm. Today the dispatcher falls
    // through to the unsupported-block fallback.
    render(
      <BlockRenderer
        block={makeBlock({ blockType: "lab" })}
        moduleId="mod-1"
      />
    );
    expect(screen.getByTestId("lab-block-stub")).toBeInTheDocument();
  });

  it("block_renderer_lab_skeleton_for_non_ready — lab block in pending state renders skeleton", () => {
    // PASSES already — non-ready dispatch is type-agnostic. Locks the
    // expectation that lab blocks reuse the skeleton flow during generation.
    render(
      <BlockRenderer
        block={makeBlock({ blockType: "lab", status: "generating" })}
        moduleId="mod-1"
      />
    );
    expect(screen.getByText(/generating/i)).toBeInTheDocument();
  });
});
