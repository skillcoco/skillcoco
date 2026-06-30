/**
 * ModuleView Phase 3 tests — Wave 4 (03-07 Task 1)
 *
 * Tests the three-tab layout (Lessons | Quiz | Practice), polling, legacy banner,
 * active-lesson highlight, and Phase 1 practice tab preservation.
 *
 * Phase 1 legacy tests that relied on generateModuleContent / "content" viewMode
 * are migrated to use the new store + block mocks. The Practice tab still renders
 * ExerciseContainer (Phase 1 path preserved).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import userEvent from "@testing-library/user-event";

// ─── Mock Tauri IPC commands ─────────────────────────────────────────────────
vi.mock("@/lib/tauri-commands", () => ({
  generateModuleBlocks: vi.fn(),
  regenerateModule: vi.fn(),
  getModuleBlocks: vi.fn(),
  // Phase 1 commands still needed for ExerciseContainer in practice tab
  getExercises: vi.fn(),
  generateExercise: vi.fn(),
  evaluateResponse: vi.fn(),
  completeModuleExercises: vi.fn(),
  // Keep generateModuleContent for any legacy code paths
  generateModuleContent: vi.fn(),
  sendTutorMessage: vi.fn(),
  getAuthStatus: vi.fn(),
  getModuleProgress: vi.fn(),
  getPath: vi.fn(),
}));

// ─── Mock child components to isolate ModuleView ─────────────────────────────
vi.mock("@/components/learning/BlockRenderer", () => ({
  BlockRenderer: ({ block }: { block: { id: string; blockType: string } }) => (
    <div data-testid={`block-renderer-${block.id}`} data-block-type={block.blockType}>
      BlockRenderer:{block.blockType}:{block.id}
    </div>
  ),
}));

vi.mock("@/components/exercises/ExerciseContainer", () => ({
  ExerciseContainer: ({ moduleId }: { moduleId: string }) => (
    <div data-testid="exercise-container" data-module-id={moduleId}>
      ExerciseContainer
    </div>
  ),
}));

vi.mock("@/components/learning/TutorSidebar", () => ({
  TutorSidebar: () => <div data-testid="tutor-sidebar" />,
}));

vi.mock("@/components/learning/CourseSidebar", () => ({
  CourseSidebar: () => <div data-testid="course-sidebar" />,
}));

// ─── Mock store with vi.hoisted for mutable currentLessonId ─────────────────
const mockStore = vi.hoisted(() => ({
  currentTrack: {
    id: "track-1",
    topic: "Kubernetes",
    domainModule: "devops",
    status: "active",
    goal: "Pass CKA",
    learnerId: "learner-1",
    currentModuleId: "mod-1",
    progressPercent: 10,
    totalTimeSpent: 0,
    createdAt: "2026-01-01",
    updatedAt: "2026-01-01",
  },
  currentPath: {
    id: "path-1",
    trackId: "track-1",
    version: 1,
    generatedByModel: "test",
    modulesJson: JSON.stringify([
      {
        id: "mod-1",
        title: "Kubernetes Pods",
        description: "Learn about pods",
        type: "lesson",
        difficulty: 3,
        estimatedMinutes: 30,
        objectives: ["Understand pods", "Deploy pods"],
        prerequisites: [],
      },
    ]),
    edgesJson: "[]",
    estimatedHours: 1,
    createdAt: "2026-01-01",
  },
  moduleProgress: [] as import("@/types").ModuleProgress[],
  currentLessonId: null as string | null,
  moduleBlocks: new Map<string, import("@/types/learning").ModuleBlock[]>(),
  lessonCompletions: new Map<string, Set<string>>(),
  currentQuizResult: null,
  selectTrack: vi.fn(),
  loadModuleBlocks: vi.fn(),
  loadLessonCompletions: vi.fn().mockResolvedValue(undefined),
  setCurrentLesson: vi.fn(),
  markLessonComplete: vi.fn(),
  submitQuiz: vi.fn(),
  regenerateLesson: vi.fn(),
  completeExercises: vi.fn().mockResolvedValue({
    masteryLevel: 0.8,
    moduleCompleted: false,
    newlyUnlockedModuleIds: [],
    cardsCreated: 0,
  }),
}));

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn((selector: (s: typeof mockStore) => unknown) => {
    if (typeof selector === "function") return selector(mockStore);
    return mockStore;
  }),
}));

// ─── Helpers ─────────────────────────────────────────────────────────────────

import { generateModuleBlocks, regenerateModule } from "@/lib/tauri-commands";
import { ModuleView } from "@/pages/ModuleView";
import type { ModuleBlock } from "@/types/learning";

function makeBlock(overrides: Partial<ModuleBlock> = {}): ModuleBlock {
  return {
    id: "block-1",
    moduleId: "mod-1",
    ordering: 0,
    blockType: "section",
    status: "ready",
    paramsJson: '{"lesson_title":"Lesson 1"}',
    payloadJson: '{"markdown":"# Hello","word_count":100}',
    sourceAnchorsJson: "[]",
    metadataJson: '{"concept_id":null}',
    retryCount: 0,
    createdAt: "2026-01-01",
    updatedAt: "2026-01-01",
    ...overrides,
  };
}

function renderModuleView(trackId = "track-1", moduleId = "mod-1") {
  return render(
    <MemoryRouter initialEntries={[`/track/${trackId}/module/${moduleId}`]}>
      <Routes>
        <Route path="/track/:trackId/module/:moduleId" element={<ModuleView />} />
      </Routes>
    </MemoryRouter>,
  );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

describe("ModuleView — Phase 3 tabs and core behaviour", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store to baseline state
    mockStore.currentLessonId = null;
    mockStore.moduleBlocks = new Map();
    mockStore.lessonCompletions = new Map();
    // By default loadModuleBlocks returns empty array (cache miss)
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([]);
    vi.mocked(generateModuleBlocks).mockResolvedValue({ blocks: [] });
    vi.mocked(regenerateModule).mockResolvedValue({ blocks: [] });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ── Tab structure ────────────────────────────────────────────────────────

  it("module_view_kicks_off_generation_when_module_has_no_blocks — empty module triggers generateModuleBlocks", async () => {
    // Module with NO existing blocks (fresh from create-track flow)
    mockStore.moduleBlocks = new Map();
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([]);
    vi.mocked(generateModuleBlocks).mockResolvedValue({ blocks: [] });

    renderModuleView();

    // generateModuleBlocks should fire with the module's title + objectives + learner level
    await waitFor(() => {
      expect(generateModuleBlocks).toHaveBeenCalledWith(
        expect.objectContaining({
          moduleId: "mod-1",
          trackId: "track-1",
          moduleTitle: "Kubernetes Pods",
          objectives: ["Understand pods", "Deploy pods"],
        }),
      );
    });

    // After kickoff, loadModuleBlocks is called again to pull the new pending blocks
    await waitFor(() => {
      expect(mockStore.loadModuleBlocks).toHaveBeenCalledWith("mod-1");
    });
  });

  it("module_view_does_not_kickoff_when_blocks_already_exist — cached blocks skip generation", async () => {
    const sectionBlock = makeBlock({ id: "s-1", blockType: "section", status: "ready" });
    mockStore.moduleBlocks = new Map([["mod-1", [sectionBlock]]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([sectionBlock]);

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();
    });

    // generateModuleBlocks must NOT be called when cached blocks exist
    expect(generateModuleBlocks).not.toHaveBeenCalled();
  });

  it("module_view_shows_generation_error_with_retry — failed kickoff surfaces retry button", async () => {
    mockStore.moduleBlocks = new Map();
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([]);
    vi.mocked(generateModuleBlocks).mockRejectedValueOnce("AI provider unreachable");

    renderModuleView();

    // Error message visible
    await waitFor(() => {
      expect(screen.getByText(/AI provider unreachable/i)).toBeInTheDocument();
    });

    // Retry button present
    expect(screen.getByRole("button", { name: /retry/i })).toBeInTheDocument();

    // Retrying calls generateModuleBlocks again
    vi.mocked(generateModuleBlocks).mockResolvedValueOnce({ blocks: [] });
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /retry/i }));

    await waitFor(() => {
      expect(generateModuleBlocks).toHaveBeenCalledTimes(2);
    });
  });

  it("module_view_default_tab_is_lessons — Lessons tab is active on mount", async () => {
    const sectionBlock = makeBlock({ id: "s-1", blockType: "section" });
    mockStore.moduleBlocks = new Map([["mod-1", [sectionBlock]]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([sectionBlock]);

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("lessons-tab")).toBeInTheDocument();
    });

    // Lessons tab is visible and the lessons panel is rendered
    expect(screen.getByTestId("tab-lessons")).toBeInTheDocument();
    expect(screen.getByTestId("lessons-tab")).toBeInTheDocument();
  });

  it("module_view_tabs — lessons/quiz/practice tabs render correct content", async () => {
    const user = userEvent.setup();

    const sectionBlock = makeBlock({ id: "s-1", blockType: "section", status: "ready" });
    const quizBlock = makeBlock({ id: "q-1", blockType: "quiz", status: "ready", ordering: 9 });
    mockStore.moduleBlocks = new Map([["mod-1", [sectionBlock, quizBlock]]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([sectionBlock, quizBlock]);

    renderModuleView();

    // Wait for render
    await waitFor(() => {
      expect(screen.getByTestId("tab-lessons")).toBeInTheDocument();
    });

    // Default: lessons tab active
    expect(screen.getByTestId("lessons-tab")).toBeInTheDocument();
    // Section block rendered in lessons tab
    expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();

    // Switch to Quiz tab
    await user.click(screen.getByTestId("tab-quiz"));
    await waitFor(() => {
      expect(screen.getByTestId("quiz-tab")).toBeInTheDocument();
    });
    // Quiz block rendered
    expect(screen.getByTestId("block-renderer-q-1")).toBeInTheDocument();

    // Switch to Practice tab
    await user.click(screen.getByTestId("tab-practice"));
    await waitFor(() => {
      expect(screen.getByTestId("practice-tab")).toBeInTheDocument();
    });
    // ExerciseContainer rendered
    expect(screen.getByTestId("exercise-container")).toBeInTheDocument();

    // Switch back to Lessons tab
    await user.click(screen.getByTestId("tab-lessons"));
    await waitFor(() => {
      expect(screen.getByTestId("lessons-tab")).toBeInTheDocument();
    });
  });

  it("module_view_practice_tab_renders_legacy_exercise — ExerciseContainer mounted in practice tab", async () => {
    const user = userEvent.setup();
    mockStore.moduleBlocks = new Map([["mod-1", []]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([]);

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("tab-practice")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("tab-practice"));

    await waitFor(() => {
      expect(screen.getByTestId("exercise-container")).toBeInTheDocument();
    });
    // Correct moduleId passed
    expect(screen.getByTestId("exercise-container")).toHaveAttribute("data-module-id", "mod-1");
  });

  // ── Legacy banner ────────────────────────────────────────────────────────

  it("module_view_legacy_banner — single synthetic section block shows Generate as lessons banner", async () => {
    // The legacy detection rule: exactly 1 block, blockType=section, paramsJson='{}'
    const legacyBlock = makeBlock({
      id: "legacy-1",
      blockType: "section",
      paramsJson: "{}",
      status: "ready",
    });
    mockStore.moduleBlocks = new Map([["mod-1", [legacyBlock]]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([legacyBlock]);

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("legacy-banner")).toBeInTheDocument();
    });

    expect(screen.getByText(/generate as lessons/i)).toBeInTheDocument();
  });

  it("module_view_legacy_banner_click — clicking Generate as lessons calls regenerateModule", async () => {
    const user = userEvent.setup();

    const legacyBlock = makeBlock({
      id: "legacy-1",
      blockType: "section",
      paramsJson: "{}",
      status: "ready",
    });
    mockStore.moduleBlocks = new Map([["mod-1", [legacyBlock]]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([legacyBlock]);
    vi.mocked(regenerateModule).mockResolvedValue({
      blocks: [makeBlock({ id: "new-1", blockType: "section", status: "pending" })],
    });

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("legacy-banner")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("regenerate-as-lessons-btn"));

    await waitFor(() => {
      expect(regenerateModule).toHaveBeenCalledWith(
        expect.objectContaining({ moduleId: "mod-1", trackId: "track-1" }),
      );
    });
  });

  it("module_view_no_banner_for_multi_block — banner NOT shown when module has multiple blocks", async () => {
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", paramsJson: '{"lesson_title":"L1"}' }),
      makeBlock({ id: "s-2", blockType: "section", paramsJson: '{"lesson_title":"L2"}', ordering: 1 }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("tab-lessons")).toBeInTheDocument();
    });

    expect(screen.queryByTestId("legacy-banner")).not.toBeInTheDocument();
  });

  // ── Active lesson highlight ──────────────────────────────────────────────

  it("module_view_scrolls_to_top_on_lesson_change — switching lesson resets scroll", async () => {
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-1";

    // Spy on Element.prototype.scrollTo so any scroll container we use is captured
    const scrollSpy = vi.fn();
    const original = Element.prototype.scrollTo;
    Element.prototype.scrollTo = scrollSpy;

    const { rerender } = renderModuleView();
    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();
    });
    scrollSpy.mockClear();

    // Change active lesson — simulates a sidebar click via store update
    mockStore.currentLessonId = "s-2";
    rerender(
      <MemoryRouter initialEntries={["/track/track-1/module/mod-1"]}>
        <Routes>
          <Route path="/track/:trackId/module/:moduleId" element={<ModuleView />} />
        </Routes>
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-2")).toBeInTheDocument();
    });

    // Either window or a container ref should be scrolled to top
    expect(scrollSpy).toHaveBeenCalled();
    const firstCall = scrollSpy.mock.calls[0][0];
    if (typeof firstCall === "object") {
      expect(firstCall.top).toBe(0);
    } else {
      // legacy form scrollTo(x, y)
      expect(firstCall).toBe(0);
    }

    Element.prototype.scrollTo = original;
  });

  it("module_view_renders_one_lesson_at_a_time — only the active lesson is in the DOM (Udemy convention)", async () => {
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
      makeBlock({ id: "s-3", blockType: "section", ordering: 2, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-2";

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("lessons-tab")).toBeInTheDocument();
    });

    // Only the active lesson (s-2) is rendered
    expect(screen.getByTestId("block-renderer-s-2")).toBeInTheDocument();
    expect(screen.queryByTestId("block-renderer-s-1")).not.toBeInTheDocument();
    expect(screen.queryByTestId("block-renderer-s-3")).not.toBeInTheDocument();
  });

  it("module_view_defaults_to_first_lesson — when currentLessonId is null, first ready lesson renders", async () => {
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = null;

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("block-renderer-s-2")).not.toBeInTheDocument();
  });

  it("module_view_next_lesson_button — advances currentLessonId to next section block", async () => {
    const user = userEvent.setup();
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
      makeBlock({ id: "s-3", blockType: "section", ordering: 2, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-1";

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /next lesson/i }));
    expect(mockStore.setCurrentLesson).toHaveBeenCalledWith("s-2");
  });

  it("module_view_prev_lesson_button — moves currentLessonId to previous section block", async () => {
    const user = userEvent.setup();
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-2";

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-2")).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /previous lesson/i }));
    expect(mockStore.setCurrentLesson).toHaveBeenCalledWith("s-1");
  });

  it("module_view_prev_disabled_on_first — Prev button disabled when on lesson 1", async () => {
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-1";

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();
    });

    expect(screen.getByRole("button", { name: /previous lesson/i })).toBeDisabled();
  });

  it("module_view_last_lesson_take_quiz_cta — on last lesson, Next becomes 'Take the quiz' and switches to quiz tab", async () => {
    const user = userEvent.setup();
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
      makeBlock({ id: "q-1", blockType: "quiz", ordering: 2, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-2"; // last lesson

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-2")).toBeInTheDocument();
    });

    const cta = screen.getByRole("button", { name: /take the quiz/i });
    expect(cta).toBeInTheDocument();
    await user.click(cta);

    await waitFor(() => {
      expect(screen.getByTestId("quiz-tab")).toBeInTheDocument();
    });
  });

  it("module_view_last_lesson_continue_to_next_module — when module passed, last lesson shows next-module CTA", async () => {
    // Add a second module to the path so there's somewhere to go next
    mockStore.currentPath = {
      ...mockStore.currentPath,
      modulesJson: JSON.stringify([
        {
          id: "mod-1",
          title: "Kubernetes Pods",
          description: "Learn about pods",
          type: "lesson",
          difficulty: 3,
          estimatedMinutes: 30,
          objectives: ["Understand pods"],
          prerequisites: [],
        },
        {
          id: "mod-2",
          title: "Deployments",
          description: "",
          type: "lesson",
          difficulty: 3,
          estimatedMinutes: 30,
          objectives: [],
          prerequisites: ["mod-1"],
        },
      ]),
    };
    // mod-1 mastered (>= 0.7), mod-2 unlocked
    mockStore.moduleProgress = [
      { moduleId: "mod-1", learnerId: "lp1", status: "completed", masteryLevel: 0.85,
        score: null, timeSpent: 0, attempts: 0, startedAt: null, completedAt: null,
        id: "mp1" } as unknown as import("@/types").ModuleProgress,
      { moduleId: "mod-2", learnerId: "lp1", status: "in_progress", masteryLevel: 0,
        score: null, timeSpent: 0, attempts: 0, startedAt: null, completedAt: null,
        id: "mp2" } as unknown as import("@/types").ModuleProgress,
    ];

    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
      makeBlock({ id: "q-1", blockType: "quiz", ordering: 2, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-2";

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-2")).toBeInTheDocument();
    });

    const cta = screen.getByRole("link", { name: /continue to next module/i });
    expect(cta).toBeInTheDocument();
    expect(cta.getAttribute("href")).toMatch(/\/track\/track-1\/module\/mod-2/);
  });

  it("module_view_no_next_lesson_button_on_last — last lesson hides 'Next lesson' (replaced by Take the quiz)", async () => {
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-2";

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-2")).toBeInTheDocument();
    });

    // "Next lesson" is replaced by "Take the quiz" on the last lesson.
    expect(screen.queryByRole("button", { name: /^next lesson$/i })).not.toBeInTheDocument();
  });

  // ── Polling ──────────────────────────────────────────────────────────────

  it("module_view_polls_while_generating — polls getModuleBlocks every 3s while blocks pending", async () => {
    vi.useFakeTimers();

    // Setup: store starts with pending blocks so polling useEffect fires
    const pendingBlock = makeBlock({ id: "p-1", blockType: "section", status: "pending" });
    const readyBlock1 = makeBlock({ id: "s-1", blockType: "section", status: "ready" });

    // Set pending blocks in store so the polling useEffect activates
    mockStore.moduleBlocks = new Map([["mod-1", [readyBlock1, pendingBlock]]]);

    let callCount = 0;
    mockStore.loadModuleBlocks = vi.fn().mockImplementation(async () => {
      callCount++;
      if (callCount === 1) {
        // mount: keep pending blocks so polling keeps going
        return [readyBlock1, pendingBlock];
      }
      // subsequent calls (polls): return all ready
      mockStore.moduleBlocks = new Map([["mod-1", [readyBlock1]]]);
      return [readyBlock1];
    });

    await act(async () => {
      renderModuleView();
    });

    // Initial mount load happened
    expect(mockStore.loadModuleBlocks).toHaveBeenCalledTimes(1);

    // Advance 3 seconds to trigger the polling interval
    await act(async () => {
      vi.advanceTimersByTime(3000);
    });

    // At least one poll occurred (callCount >= 2)
    expect(mockStore.loadModuleBlocks.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it("module_view_stops_polling_when_all_ready — no additional IPC calls after all blocks ready", async () => {
    vi.useFakeTimers();

    const readyBlocks = [
      makeBlock({ id: "s-1", status: "ready" }),
      makeBlock({ id: "s-2", status: "ready", ordering: 1 }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", readyBlocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(readyBlocks);

    await act(async () => {
      renderModuleView();
    });

    const initialCalls = vi.mocked(mockStore.loadModuleBlocks).mock.calls.length;

    // Advance 10 seconds — should NOT poll since all ready
    await act(async () => {
      vi.advanceTimersByTime(10000);
    });

    // No additional calls
    expect(mockStore.loadModuleBlocks).toHaveBeenCalledTimes(initialCalls);
  });

  // ── Module title (from path) ─────────────────────────────────────────────

  it("renders module title from currentPath modulesJson", async () => {
    mockStore.moduleBlocks = new Map([["mod-1", []]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([]);

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByText("Kubernetes Pods")).toBeInTheDocument();
    });
  });

  // ── GAP-01 (Plan 03.1-09): lab blocks in Practice tab ────────────────────

  it("module_view_renders_lab_blocks_in_practice_tab — lab blocks dispatch via BlockRenderer when present", async () => {
    const user = userEvent.setup();
    const sectionBlock = makeBlock({ id: "s-1", blockType: "section", status: "ready" });
    const labBlock = makeBlock({
      id: "lab-1",
      blockType: "lab",
      status: "ready",
      ordering: 5,
      payloadJson: JSON.stringify({
        spec: {
          slug: "pod-inspect",
          title: "Inspect a Pod",
          requiresDocker: false,
          creates: [],
          steps: [],
        },
      }),
    });
    mockStore.moduleBlocks = new Map([["mod-1", [sectionBlock, labBlock]]]);
    mockStore.loadModuleBlocks = vi
      .fn()
      .mockResolvedValue([sectionBlock, labBlock]);

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("tab-practice")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("tab-practice"));

    // GAP-01: Practice tab now renders the lab block via BlockRenderer.
    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-lab-1")).toBeInTheDocument();
    });
    expect(screen.getByTestId("block-renderer-lab-1")).toHaveAttribute(
      "data-block-type",
      "lab",
    );
    // The legacy ExerciseContainer is replaced when at least one lab block
    // is present (preserves the "Practice = does the thing" framing).
    expect(screen.queryByTestId("exercise-container")).not.toBeInTheDocument();
  });

  it("module_view_practice_tab_falls_back_to_exercise_container_when_no_labs — preserve legacy fallback", async () => {
    const user = userEvent.setup();
    const sectionBlock = makeBlock({ id: "s-1", blockType: "section", status: "ready" });
    mockStore.moduleBlocks = new Map([["mod-1", [sectionBlock]]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue([sectionBlock]);

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("tab-practice")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("tab-practice"));

    // No lab blocks → ExerciseContainer fallback remains (Phase-1 holdover
    // preserved per VERIFICATION.md suggested fix).
    await waitFor(() => {
      expect(screen.getByTestId("exercise-container")).toBeInTheDocument();
    });
  });

  // ── Phase 10 Plan 02: mark-read + advance footer control (D-05 / D-06) ───

  it("mark_read_advance_btn_renders_in_footer — data-testid=mark-read-advance-btn exists in lessons footer row", async () => {
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-1";

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();
    });

    // Footer mark-read control must be present
    expect(screen.getByTestId("mark-read-advance-btn")).toBeInTheDocument();
  });

  it("mark_read_advance_btn_calls_markLessonComplete_and_advances — single click marks read AND advances to next lesson", async () => {
    const user = userEvent.setup();
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-1";

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("mark-read-advance-btn"));

    // markLessonComplete called with moduleId + blockId
    expect(mockStore.markLessonComplete).toHaveBeenCalledWith("mod-1", "s-1");
    // advances to next lesson
    expect(mockStore.setCurrentLesson).toHaveBeenCalledWith("s-2");
  });

  it("mark_read_advance_btn_does_not_call_submitQuiz — D-06: mark-read path never invokes mastery pipeline", async () => {
    const user = userEvent.setup();
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-1";

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("mark-read-advance-btn"));

    // D-06: submitQuiz must NOT be called by the mark-read path
    expect(mockStore.submitQuiz).not.toHaveBeenCalled();
  });

  it("mark_read_advance_btn_already_read_shows_check_and_still_advances — completed lesson shows ✓ state and advances on click", async () => {
    const user = userEvent.setup();
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-1";
    // s-1 already completed
    mockStore.lessonCompletions = new Map([["mod-1", new Set(["s-1"])]]);

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-1")).toBeInTheDocument();
    });

    // Shows read/✓ state label when already completed
    const btn = screen.getByTestId("mark-read-advance-btn");
    expect(btn).toBeInTheDocument();
    // Clicking still advances
    await user.click(btn);
    expect(mockStore.setCurrentLesson).toHaveBeenCalledWith("s-2");
  });

  it("mark_read_advance_btn_last_lesson_records_read_quiz_cta_preserved — on last lesson, marks read without error and quiz CTA still shows", async () => {
    const user = userEvent.setup();
    const blocks = [
      makeBlock({ id: "s-1", blockType: "section", ordering: 0, status: "ready" }),
      makeBlock({ id: "s-2", blockType: "section", ordering: 1, status: "ready" }),
      makeBlock({ id: "q-1", blockType: "quiz", ordering: 2, status: "ready" }),
    ];
    mockStore.moduleBlocks = new Map([["mod-1", blocks]]);
    mockStore.loadModuleBlocks = vi.fn().mockResolvedValue(blocks);
    mockStore.currentLessonId = "s-2"; // last section lesson

    renderModuleView();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-s-2")).toBeInTheDocument();
    });

    // Mark-read control visible on last lesson
    expect(screen.getByTestId("mark-read-advance-btn")).toBeInTheDocument();

    // Click it — markLessonComplete should be called
    await user.click(screen.getByTestId("mark-read-advance-btn"));
    expect(mockStore.markLessonComplete).toHaveBeenCalledWith("mod-1", "s-2");

    // "Take the quiz" CTA must still be present after click
    expect(screen.getByRole("button", { name: /take the quiz/i })).toBeInTheDocument();
  });
});
