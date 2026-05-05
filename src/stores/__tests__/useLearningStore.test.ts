import { describe, it, expect, vi, beforeEach } from "vitest";

// Vitest hoisting rule: inline literals only inside vi.mock factory —
// never reference outer const variables.
vi.mock("@/lib/tauri-commands", () => ({
  listTracks: vi.fn(),
  getTrack: vi.fn(),
  getPath: vi.fn(),
  getModuleProgress: vi.fn(),
  getDueCards: vi.fn(),
  createTrack: vi.fn(),
  completeModuleExercises: vi.fn(),
  markLessonComplete: vi.fn(),
  submitQuiz: vi.fn(),
  getModuleBlocks: vi.fn(),
  regenerateLesson: vi.fn(),
}));

import { useLearningStore } from "@/stores/useLearningStore";
import * as commands from "@/lib/tauri-commands";
import type { ModuleBlock } from "@/types/learning";

function makeBlock(overrides: Partial<ModuleBlock> = {}): ModuleBlock {
  return {
    id: "blk-1",
    moduleId: "mod-1",
    ordering: 0,
    blockType: "section",
    status: "ready",
    paramsJson: "{}",
    payloadJson: '{"markdown":"# Hello"}',
    sourceAnchorsJson: "[]",
    metadataJson: '{"concept_id":null}',
    retryCount: 0,
    createdAt: "2026-05-05T00:00:00Z",
    updatedAt: "2026-05-05T00:00:00Z",
    ...overrides,
  };
}

describe("useLearningStore phase 3 extensions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store to initial state between tests
    useLearningStore.setState({
      currentLessonId: null,
      moduleBlocks: new Map(),
      lessonCompletions: new Map(),
      currentQuizResult: null,
    });
  });

  it("store_current_lesson_id — store exposes currentLessonId and setCurrentLesson action", () => {
    const store = useLearningStore.getState();

    expect(store.currentLessonId).toBeDefined();
    expect(store.currentLessonId).toBeNull(); // initial value
    expect(typeof store.setCurrentLesson).toBe("function");

    store.setCurrentLesson("block-42");
    expect(useLearningStore.getState().currentLessonId).toBe("block-42");
  });

  it("store_mark_lesson_complete — markLessonComplete action calls IPC and exists", async () => {
    vi.mocked(commands.markLessonComplete).mockResolvedValue(undefined);

    const store = useLearningStore.getState();

    expect(typeof store.markLessonComplete).toBe("function");

    await store.markLessonComplete("mod-1", "blk-1");
    expect(commands.markLessonComplete).toHaveBeenCalledWith("mod-1", "blk-1");

    // Optimistic: lessonCompletions should have the blockId
    const completions = useLearningStore.getState().lessonCompletions;
    expect(completions.get("mod-1")?.has("blk-1")).toBe(true);
  });

  it("store_submit_quiz — submitQuiz action calls IPC and stores the result", async () => {
    const mockResult = {
      scorePercent: 75,
      passed: true,
      masteryLevel: 0.8,
      moduleCompleted: true,
      newlyUnlockedModuleIds: [],
      cardsCreated: 0,
      review: [],
    };
    vi.mocked(commands.submitQuiz).mockResolvedValue(mockResult);

    const store = useLearningStore.getState();

    expect(typeof store.submitQuiz).toBe("function");

    const result = await store.submitQuiz({
      moduleId: "mod-1",
      trackId: "trk-1",
      blockId: "blk-quiz",
      answers: [],
    });
    expect(commands.submitQuiz).toHaveBeenCalled();
    expect(result.passed).toBe(true);

    // Result stored in store
    expect(useLearningStore.getState().currentQuizResult?.passed).toBe(true);
  });

  it("store_load_module_blocks — loadModuleBlocks fetches blocks and stores in map", async () => {
    const mockBlocks = Array.from({ length: 8 }, (_, i) =>
      makeBlock({ id: `blk-${i}`, ordering: i })
    );
    vi.mocked(commands.getModuleBlocks).mockResolvedValue(mockBlocks);

    const store = useLearningStore.getState();
    const result = await store.loadModuleBlocks("mod-1");

    expect(commands.getModuleBlocks).toHaveBeenCalledWith("mod-1");
    expect(result).toHaveLength(8);

    const storedBlocks = useLearningStore.getState().moduleBlocks.get("mod-1");
    expect(storedBlocks).toHaveLength(8);
  });

  it("store_regenerate_lesson — regenerateLesson replaces block in map", async () => {
    const oldBlock = makeBlock({ id: "blk-x", status: "ready" });
    const newBlock = makeBlock({ id: "blk-x", status: "ready", payloadJson: '{"markdown":"# Updated"}' });

    // Seed initial state with the block
    useLearningStore.setState({
      moduleBlocks: new Map([["mod-1", [oldBlock]]]),
    });

    vi.mocked(commands.regenerateLesson).mockResolvedValue(newBlock);

    const store = useLearningStore.getState();
    await store.regenerateLesson("blk-x");

    const storedBlocks = useLearningStore.getState().moduleBlocks.get("mod-1");
    expect(storedBlocks?.[0].payloadJson).toBe('{"markdown":"# Updated"}');
  });
});
