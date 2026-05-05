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
}));

import { useLearningStore } from "@/stores/useLearningStore";
import * as commands from "@/lib/tauri-commands";

// Type helper to access Phase 3 store extensions without breaking TS.
// The actual fields don't exist yet — tests FAIL because store.currentLessonId is undefined.
type Phase3Store = {
  currentLessonId?: string | null;
  setCurrentLesson?: (id: string | null) => void;
  markLessonComplete?: (moduleId: string, blockId: string) => Promise<void>;
  submitQuiz?: (moduleId: string, trackId: string, blockId: string, answers: unknown[]) => Promise<unknown>;
};

describe("useLearningStore phase 3 extensions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("store_current_lesson_id — store exposes currentLessonId and setCurrentLesson action", () => {
    const store = useLearningStore.getState() as typeof useLearningStore.getState extends () => infer T ? T & Phase3Store : Phase3Store;

    // FAILS in Wave 0: currentLessonId and setCurrentLesson don't exist on the store yet.
    // GREEN in 03-05 Task 1 when these are added to useLearningStore.
    expect(store.currentLessonId).toBeDefined();
    expect(typeof store.setCurrentLesson).toBe("function");

    store.setCurrentLesson?.("block-42");
    expect((useLearningStore.getState() as unknown as Phase3Store).currentLessonId).toBe("block-42");
  });

  it("store_mark_lesson_complete — markLessonComplete action calls IPC and exists", async () => {
    vi.mocked(commands.markLessonComplete).mockResolvedValue(undefined);

    const store = useLearningStore.getState() as unknown as Phase3Store;

    // FAILS in Wave 0: markLessonComplete action doesn't exist on the store yet.
    // GREEN in 03-05 Task 1.
    expect(typeof store.markLessonComplete).toBe("function");

    await store.markLessonComplete?.("mod-1", "blk-1");
    expect(commands.markLessonComplete).toHaveBeenCalledWith("mod-1", "blk-1");
  });

  it("store_submit_quiz — submitQuiz action calls IPC and stores the result", async () => {
    vi.mocked(commands.submitQuiz).mockResolvedValue({
      scorePercent: 75,
      passed: true,
      masteryLevel: 0.8,
      moduleCompleted: true,
      newlyUnlockedModuleIds: [],
      cardsCreated: 0,
      review: [],
    });

    const store = useLearningStore.getState() as unknown as Phase3Store;

    // FAILS in Wave 0: submitQuiz action doesn't exist on the store yet.
    // GREEN in 03-05 Task 1.
    expect(typeof store.submitQuiz).toBe("function");

    const result = await store.submitQuiz?.("mod-1", "trk-1", "blk-quiz", []);
    expect(commands.submitQuiz).toHaveBeenCalled();
    expect((result as { passed?: boolean })?.passed).toBe(true);
  });
});
