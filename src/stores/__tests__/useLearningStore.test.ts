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
  deleteTrack: vi.fn(),
  completeModuleExercises: vi.fn(),
  markLessonComplete: vi.fn(),
  submitQuiz: vi.fn(),
  getModuleBlocks: vi.fn(),
  regenerateLesson: vi.fn(),
  setTrackBrowseMode: vi.fn(),
}));

import { useLearningStore, selectModulePracticalMastery } from "@/stores/useLearningStore";
import { useAchievementsStore, __resetStore as __resetAchievementsStore } from "@/stores/useAchievementsStore";
import * as commands from "@/lib/tauri-commands";
import type { ModuleBlock } from "@/types/learning";
import type { ModuleProgress } from "@/types";
import type { Achievement } from "@/types/achievements";

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
      newlyIssuedAchievements: [],
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

  it("store_delete_track — deleteTrack calls IPC and removes from tracks list", async () => {
    vi.mocked(commands.deleteTrack).mockResolvedValue(undefined);

    const trackA = { id: "t1", topic: "K8s" } as unknown as import("@/types").LearningTrack;
    const trackB = { id: "t2", topic: "Rust" } as unknown as import("@/types").LearningTrack;
    useLearningStore.setState({
      tracks: [trackA, trackB],
      currentTrack: trackA,
      currentPath: { id: "p1" } as unknown as import("@/types").LearningPath,
      moduleProgress: [{ moduleId: "m1" } as unknown as import("@/types").ModuleProgress],
    });

    const store = useLearningStore.getState();
    await store.deleteTrack("t1");

    expect(commands.deleteTrack).toHaveBeenCalledWith("t1");

    const state = useLearningStore.getState();
    expect(state.tracks.map((t) => t.id)).toEqual(["t2"]);
    expect(state.currentTrack).toBeNull();
    expect(state.currentPath).toBeNull();
    expect(state.moduleProgress).toEqual([]);
  });

  // ── Phase 03.1 LAB-08 — practical mastery selector ──

  it("select_module_practical_mastery — returns practicalMastery for a loaded module", () => {
    const mp: ModuleProgress = {
      id: "mp-1",
      moduleId: "mod-1",
      learnerId: "p1",
      status: "in_progress",
      score: null,
      timeSpent: 0,
      attempts: 0,
      masteryLevel: 0.8,
      practicalMastery: 0.65,
      startedAt: null,
      completedAt: null,
    };
    useLearningStore.setState({ moduleProgress: [mp] });

    const value = selectModulePracticalMastery("mod-1")(useLearningStore.getState());
    expect(value).toBe(0.65);
  });

  it("select_module_practical_mastery — returns 0 for an unknown module", () => {
    useLearningStore.setState({ moduleProgress: [] });
    const value = selectModulePracticalMastery("missing")(useLearningStore.getState());
    expect(value).toBe(0);
  });

  // ── Phase 6 Plan 06-04 (Wave 3) — sibling-slice integration ──
  //
  // submitQuiz must forward result.newlyIssuedAchievements to the sibling
  // useAchievementsStore.appendNewlyIssued. We do NOT extend useLearningStore
  // state with achievement fields (Phase 4 Pitfall 5).

  function makeAchievement(overrides: Partial<Achievement> = {}): Achievement {
    return {
      id: "ach-1",
      learnerId: "lnr-1",
      trackId: "trk-1",
      packId: null,
      kind: "badge",
      level: "Associate",
      issuedAt: "2026-06-15T00:00:00Z",
      masteryScore: 0.8,
      payloadJson: "",
      signature: "",
      keyFingerprint: "deadbeef",
      trackTopic: "Kubernetes",
      ...overrides,
    };
  }

  it("submitQuiz_appendNewlyIssued_when_result_has_achievements", async () => {
    __resetAchievementsStore();
    const issued = [
      makeAchievement({ id: "A" }),
      makeAchievement({ id: "B", level: "Practitioner" }),
    ];
    vi.mocked(commands.submitQuiz).mockResolvedValue({
      scorePercent: 90,
      passed: true,
      masteryLevel: 0.9,
      moduleCompleted: true,
      newlyUnlockedModuleIds: [],
      cardsCreated: 0,
      review: [],
      newlyIssuedAchievements: issued,
    });
    vi.mocked(commands.getModuleProgress).mockResolvedValue([]);

    await useLearningStore.getState().submitQuiz({
      moduleId: "mod-1",
      trackId: "trk-1",
      blockId: "blk-q",
      answers: [],
    });

    const ids = useAchievementsStore.getState().achievements.map((a) => a.id);
    expect(ids.slice(0, 2)).toEqual(["A", "B"]);
  });

  it("submitQuiz_no_append_when_empty_array", async () => {
    __resetAchievementsStore();
    vi.mocked(commands.submitQuiz).mockResolvedValue({
      scorePercent: 60,
      passed: true,
      masteryLevel: 0.65,
      moduleCompleted: false,
      newlyUnlockedModuleIds: [],
      cardsCreated: 0,
      review: [],
      newlyIssuedAchievements: [],
    });
    vi.mocked(commands.getModuleProgress).mockResolvedValue([]);

    await useLearningStore.getState().submitQuiz({
      moduleId: "mod-1",
      trackId: "trk-1",
      blockId: "blk-q",
      answers: [],
    });

    expect(useAchievementsStore.getState().achievements).toEqual([]);
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

  // ── Phase 10 Plan 03 — setTrackBrowseMode optimistic action ──

  it("setTrackBrowseMode — patches currentTrack.browseMode optimistically and calls IPC", async () => {
    vi.mocked(commands.setTrackBrowseMode).mockResolvedValue(undefined);

    const track = {
      id: "trk-browse",
      learnerId: "lnr-1",
      topic: "Kubernetes",
      domainModule: "devops" as const,
      status: "active" as const,
      goal: "Pass CKA",
      currentModuleId: null,
      progressPercent: 0,
      totalTimeSpent: 0,
      createdAt: "2026-06-30T00:00:00Z",
      updatedAt: "2026-06-30T00:00:00Z",
    };
    useLearningStore.setState({
      currentTrack: track,
      tracks: [track],
    });

    const store = useLearningStore.getState();
    expect(typeof store.setTrackBrowseMode).toBe("function");

    await store.setTrackBrowseMode("trk-browse", "free");

    const state = useLearningStore.getState();
    expect(state.currentTrack?.browseMode).toBe("free");
    expect(state.tracks[0].browseMode).toBe("free");
    expect(commands.setTrackBrowseMode).toHaveBeenCalledWith("trk-browse", "free");
  });

  it("setTrackBrowseMode — rolls back on IPC error", async () => {
    vi.mocked(commands.setTrackBrowseMode).mockRejectedValue(new Error("IPC failed"));

    const track = {
      id: "trk-browse",
      learnerId: "lnr-1",
      topic: "Kubernetes",
      domainModule: "devops" as const,
      status: "active" as const,
      goal: "Pass CKA",
      currentModuleId: null,
      progressPercent: 0,
      totalTimeSpent: 0,
      createdAt: "2026-06-30T00:00:00Z",
      updatedAt: "2026-06-30T00:00:00Z",
      browseMode: "linear" as const,
    };
    useLearningStore.setState({
      currentTrack: track,
      tracks: [track],
    });

    const store = useLearningStore.getState();
    await store.setTrackBrowseMode("trk-browse", "free");

    // Rollback: browseMode should be "linear" again
    const state = useLearningStore.getState();
    expect(state.currentTrack?.browseMode).toBe("linear");
    expect(state.tracks[0].browseMode).toBe("linear");
  });
});
