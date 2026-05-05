import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";

// Vitest hoisting rule: inline literals only inside vi.mock factory.
vi.mock("@/lib/tauri-commands", () => ({
  getModuleBlocks: vi.fn(),
}));

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn(() => ({
    currentTrack: { id: "track-1", topic: "Kubernetes" },
    moduleBlocks: new Map(),
    currentLessonId: null,
    setCurrentLesson: vi.fn(),
  })),
}));

import { CourseSidebar } from "@/components/learning/CourseSidebar";
import { getModuleBlocks } from "@/lib/tauri-commands";
import type { LearningTrack, PathModule, ModuleProgress } from "@/types/learning";

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
    startedAt: "2026-05-05T00:00:00Z",
    completedAt: null,
  },
];

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
  });

  it("sidebar_expands_lessons_on_click — click module row to reveal lesson list", async () => {
    const user = userEvent.setup();
    renderSidebar();

    // Click the module row to expand lessons
    const moduleRow = screen.getByText("Pods and Nodes");
    await user.click(moduleRow);

    // FAILS in Wave 0: CourseSidebar doesn't have expandable lessons yet.
    // GREEN in 03-06 Task 2 when LessonNavList is wired.
    expect(screen.getByTestId("lesson-nav-list-mod-1")).toBeInTheDocument();
  });

  it("sidebar_lesson_status_icon — ready block shows checkmark, generating shows spinner", async () => {
    vi.mocked(getModuleBlocks).mockResolvedValue([
      {
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
      },
    ]);

    renderSidebar();

    // FAILS in Wave 0: status icons not rendered by placeholder CourseSidebar.
    // GREEN in 03-06 Task 2.
    expect(screen.getByTestId("lesson-status-ready-blk-1")).toBeInTheDocument();
  });

  it("sidebar_active_module_auto_expanded — active module lessons visible without click", () => {
    renderSidebar("mod-1");

    // FAILS in Wave 0: CourseSidebar doesn't auto-expand active module yet.
    // GREEN in 03-06 Task 2.
    expect(screen.getByTestId("lesson-nav-list-mod-1")).toBeInTheDocument();
  });
});
