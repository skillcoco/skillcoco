import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";

// Vitest hoisting rule: inline literals only inside vi.mock factory.
const mockSetCurrentLesson = vi.fn();
const mockLoadModuleBlocks = vi.fn();
const mockNavigate = vi.fn();

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: vi.fn(() => mockNavigate),
  };
});

// Selector-aware store mock via vi.hoisted
const mockStoreState = vi.hoisted(() => ({
  moduleBlocks: new Map<string, import("@/types/learning").ModuleBlock[]>(),
  currentLessonId: null as string | null,
  lessonCompletions: new Map<string, Set<string>>(),
  setCurrentLesson: vi.fn(),
  loadModuleBlocks: vi.fn(),
}));

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn((selector: (s: typeof mockStoreState) => unknown) => {
    const state = {
      moduleBlocks: mockStoreState.moduleBlocks,
      currentLessonId: mockStoreState.currentLessonId,
      lessonCompletions: mockStoreState.lessonCompletions,
      setCurrentLesson: mockSetCurrentLesson,
      loadModuleBlocks: mockLoadModuleBlocks,
    };
    return typeof selector === "function" ? selector(state) : state;
  }),
}));

import { CourseSidebar } from "@/components/learning/CourseSidebar";
import type {
  LearningTrack,
  PathModule,
  ModuleProgress,
  ModuleBlock,
} from "@/types/learning";

const mockTrack: LearningTrack = {
  id: "track-1",
  learnerId: "learner-1",
  topic: "Kubernetes",
  domainModule: "devops",
  status: "active",
  goal: "Pass CKA",
  currentModuleId: "mod-1",
  progressPercent: 10,
  totalTimeSpent: 0,
  createdAt: "2026-05-05T00:00:00Z",
  updatedAt: "2026-05-05T00:00:00Z",
};

const mockTrackFree: LearningTrack = { ...mockTrack, browseMode: "free" as const };
const mockTrackLinear: LearningTrack = { ...mockTrack, browseMode: "linear" as const };

const mockModules: PathModule[] = [
  {
    id: "mod-1",
    title: "Pods and Nodes",
    description: "Introduction to pods",
    type: "lesson",
    difficulty: 2,
    estimatedMinutes: 30,
    objectives: ["Understand pods"],
    prerequisites: [],
  },
];

const mockModulesWithLocked: PathModule[] = [
  {
    id: "mod-1",
    title: "Pods and Nodes",
    description: "Introduction to pods",
    type: "lesson",
    difficulty: 2,
    estimatedMinutes: 30,
    objectives: ["Understand pods"],
    prerequisites: [],
  },
  {
    id: "mod-2",
    title: "Services",
    description: "Introduction to services",
    type: "lesson",
    difficulty: 3,
    estimatedMinutes: 20,
    objectives: ["Understand services"],
    prerequisites: ["mod-1"],
  },
];

const mockProgress: ModuleProgress[] = [
  {
    id: "mp-1",
    moduleId: "mod-1",
    learnerId: "learner-1",
    status: "in_progress",
    score: null,
    timeSpent: 0,
    attempts: 0,
    masteryLevel: 0.2,
    practicalMastery: 0,
    startedAt: "2026-05-05T00:00:00Z",
    completedAt: null,
  },
];

const makeSectionBlock = (overrides: Partial<ModuleBlock> = {}): ModuleBlock => ({
  id: "blk-1",
  moduleId: "mod-1",
  ordering: 0,
  blockType: "section",
  status: "ready",
  paramsJson: '{"lesson_title":"Introduction"}',
  payloadJson: "{}",
  sourceAnchorsJson: "[]",
  metadataJson: '{"concept_id":null}',
  retryCount: 0,
  createdAt: "2026-05-05T00:00:00Z",
  updatedAt: "2026-05-05T00:00:00Z",
  ...overrides,
});

function renderSidebar(currentModuleId = "mod-1") {
  return render(
    <MemoryRouter>
      <CourseSidebar
        track={mockTrack}
        modules={mockModules}
        progress={mockProgress}
        currentModuleId={currentModuleId}
      />
    </MemoryRouter>,
  );
}

describe("CourseSidebar Phase 3 lesson expansion", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store state
    mockStoreState.moduleBlocks = new Map();
    mockStoreState.currentLessonId = null;
    mockStoreState.lessonCompletions = new Map();
    // Default: loadModuleBlocks resolves with empty array
    mockLoadModuleBlocks.mockResolvedValue([]);
  });

  it("sidebar_active_module_auto_expanded — active module lessons visible without click", () => {
    // Pre-populate blocks in store for active module
    mockStoreState.moduleBlocks = new Map([
      [
        "mod-1",
        [
          makeSectionBlock({ id: "blk-1" }),
          makeSectionBlock({ id: "blk-2", ordering: 1, paramsJson: '{"lesson_title":"Nodes"}' }),
        ],
      ],
    ]);

    renderSidebar("mod-1");

    // Active module should auto-expand showing lesson list
    expect(screen.getByTestId("lesson-nav-list-mod-1")).toBeInTheDocument();
  });

  it("sidebar_expands_lessons_on_click — click module row to reveal lesson list", async () => {
    const user = userEvent.setup();
    mockLoadModuleBlocks.mockImplementation((moduleId: string) => {
      mockStoreState.moduleBlocks = new Map([
        [moduleId, [makeSectionBlock({ id: "blk-1" })]],
      ]);
      return Promise.resolve([makeSectionBlock({ id: "blk-1" })]);
    });

    // Start with a different module active so mod-1 is not auto-expanded
    renderSidebar("mod-other");

    // Collapse state — lesson list not visible
    expect(screen.queryByTestId("lesson-nav-list-mod-1")).not.toBeInTheDocument();

    // Click the chevron to expand (the row body navigates to the module)
    await user.click(screen.getByTestId("module-expand-mod-1"));

    await waitFor(() => {
      expect(screen.getByTestId("lesson-nav-list-mod-1")).toBeInTheDocument();
    });
  });

  it("sidebar_collapse_on_re_click — click expanded module again to collapse", async () => {
    const user = userEvent.setup();
    mockStoreState.moduleBlocks = new Map([
      ["mod-1", [makeSectionBlock({ id: "blk-1" })]],
    ]);

    renderSidebar("mod-1");

    // Active module auto-expanded
    expect(screen.getByTestId("lesson-nav-list-mod-1")).toBeInTheDocument();

    // Click the chevron to collapse
    await user.click(screen.getByTestId("module-expand-mod-1"));
    expect(screen.queryByTestId("lesson-nav-list-mod-1")).not.toBeInTheDocument();
  });

  it("sidebar_loads_blocks_on_first_expand — loadModuleBlocks called once; cached on re-expand", async () => {
    const user = userEvent.setup();
    let callCount = 0;
    mockLoadModuleBlocks.mockImplementation((moduleId: string) => {
      callCount++;
      mockStoreState.moduleBlocks = new Map([
        [moduleId, [makeSectionBlock({ id: "blk-1" })]],
      ]);
      return Promise.resolve([makeSectionBlock({ id: "blk-1" })]);
    });

    renderSidebar("mod-other");

    // First expand — triggers IPC
    await user.click(screen.getByTestId("module-expand-mod-1"));
    await waitFor(() => expect(screen.getByTestId("lesson-nav-list-mod-1")).toBeInTheDocument());
    expect(callCount).toBe(1);

    // Collapse
    await user.click(screen.getByTestId("module-expand-mod-1"));
    expect(screen.queryByTestId("lesson-nav-list-mod-1")).not.toBeInTheDocument();

    // Re-expand — blocks already cached in store (moduleBlocks.has("mod-1")), no second call
    await user.click(screen.getByTestId("module-expand-mod-1"));
    await waitFor(() => expect(screen.getByTestId("lesson-nav-list-mod-1")).toBeInTheDocument());
    expect(callCount).toBe(1); // still 1 — cache hit
  });

  it("sidebar_lesson_status_icon — status icons reflect block status", async () => {
    const user = userEvent.setup();
    const blocks: ModuleBlock[] = [
      makeSectionBlock({ id: "blk-ready", status: "ready", ordering: 0 }),
      makeSectionBlock({ id: "blk-generating", status: "generating", ordering: 1 }),
      makeSectionBlock({ id: "blk-failed", status: "failed", ordering: 2 }),
    ];
    mockLoadModuleBlocks.mockImplementation((moduleId: string) => {
      mockStoreState.moduleBlocks = new Map([[moduleId, blocks]]);
      return Promise.resolve(blocks);
    });

    renderSidebar("mod-other");
    await user.click(screen.getByTestId("module-expand-mod-1"));

    await waitFor(() => {
      expect(screen.getByTestId("status-ready")).toBeInTheDocument();
      expect(screen.getByTestId("status-generating")).toBeInTheDocument();
      expect(screen.getByTestId("status-failed")).toBeInTheDocument();
    });
  });

  it("sidebar_completed_lesson_dot — completed lesson shows completed icon", async () => {
    mockStoreState.moduleBlocks = new Map([
      ["mod-1", [makeSectionBlock({ id: "blk-1", status: "ready" })]],
    ]);
    // Mark blk-1 as completed
    mockStoreState.lessonCompletions = new Map([["mod-1", new Set(["blk-1"])]]);

    renderSidebar("mod-1");

    expect(screen.getByTestId("status-completed")).toBeInTheDocument();
  });

  it("sidebar_lesson_click_navigates — clicking lesson sets currentLessonId and navigates", async () => {
    const user = userEvent.setup();
    mockStoreState.moduleBlocks = new Map([
      ["mod-1", [makeSectionBlock({ id: "blk-3" })]],
    ]);

    renderSidebar("mod-1");

    // Click the lesson row
    const lessonRow = screen.getByTestId("lesson-row-blk-3");
    await user.click(lessonRow);

    expect(mockSetCurrentLesson).toHaveBeenCalledWith("blk-3");
    expect(mockNavigate).toHaveBeenCalledWith("/track/track-1/module/mod-1");
  });

  it("sidebar_module_row_navigates — clicking a non-active module row opens that module", async () => {
    const user = userEvent.setup();
    // mod-1 is NOT the active module here, so clicking its row should open it.
    renderSidebar("mod-other");

    await user.click(screen.getByText("Pods and Nodes"));

    expect(mockNavigate).toHaveBeenCalledWith("/track/track-1/module/mod-1");
  });

  // Phase 1 regression guard: existing module-level sidebar functionality still works
  it("phase1_regression — module list renders with status labels", () => {
    renderSidebar("mod-1");

    expect(screen.getByText("Pods and Nodes")).toBeInTheDocument();
    expect(screen.getByText("In progress")).toBeInTheDocument();
  });
});

// Phase 10 Plan 03 (Task 2) — free-mode openability + recommended-next
const mockProgressWithLocked: ModuleProgress[] = [
  {
    id: "mp-1",
    moduleId: "mod-1",
    learnerId: "learner-1",
    status: "in_progress",
    score: null,
    timeSpent: 0,
    attempts: 0,
    masteryLevel: 0.2,
    practicalMastery: 0,
    startedAt: "2026-05-05T00:00:00Z",
    completedAt: null,
  },
  {
    id: "mp-2",
    moduleId: "mod-2",
    learnerId: "learner-1",
    status: "locked",
    score: null,
    timeSpent: 0,
    attempts: 0,
    masteryLevel: 0,
    practicalMastery: 0,
    startedAt: null,
    completedAt: null,
  },
];

function renderSidebarWith(track: LearningTrack, modules: PathModule[], progress: ModuleProgress[], currentModuleId = "mod-1") {
  return render(
    <MemoryRouter>
      <CourseSidebar
        track={track}
        modules={modules}
        progress={progress}
        currentModuleId={currentModuleId}
      />
    </MemoryRouter>,
  );
}

describe("CourseSidebar browse-mode free openability (Plan 10-03 Task 2)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStoreState.moduleBlocks = new Map();
    mockStoreState.currentLessonId = null;
    mockStoreState.lessonCompletions = new Map();
    mockLoadModuleBlocks.mockResolvedValue([]);
  });

  it("free_open_all — locked row renders as clickable button in free mode", () => {
    renderSidebarWith(mockTrackFree, mockModulesWithLocked, mockProgressWithLocked);

    // In free mode, mod-2 (locked in DB) should render as a clickable module-row button
    expect(screen.getByTestId("module-row-mod-2")).toBeInTheDocument();
    // The aria-disabled wrapper should NOT exist for mod-2
    const ariaDisabled = document.querySelector('[aria-disabled][data-testid="module-row-mod-2"]');
    expect(ariaDisabled).not.toBeInTheDocument();
  });

  it("linear_locked_unchanged — locked row stays aria-disabled in linear mode", () => {
    renderSidebarWith(mockTrackLinear, mockModulesWithLocked, mockProgressWithLocked);

    // In linear mode, mod-2 (locked) must NOT have module-row testid (it uses aria-disabled wrapper)
    expect(screen.queryByTestId("module-row-mod-2")).not.toBeInTheDocument();
  });

  it("recommended_next — row returned by pickNextModule has recommended-next testid", () => {
    // mod-1 is in_progress → pickNextModule returns mod-1 as recommended next
    renderSidebarWith(mockTrackLinear, mockModulesWithLocked, mockProgressWithLocked);

    expect(screen.getByTestId("recommended-next")).toBeInTheDocument();
  });

  it("free_mode_no_cursor_not_allowed — locked rows do not have cursor-not-allowed class in free mode", () => {
    renderSidebarWith(mockTrackFree, mockModulesWithLocked, mockProgressWithLocked);

    const button = screen.getByTestId("module-row-mod-2");
    // The button wrapper's inner content should not have cursor-not-allowed
    expect(button.closest("div")?.className ?? button.className).not.toContain("cursor-not-allowed");
  });
});
