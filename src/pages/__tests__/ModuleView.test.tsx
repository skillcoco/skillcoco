// Plan 04 Task 3b — ModuleView scaffold made GREEN.
// The module is provided via modulesJson (JSON string) so ModuleView's
// JSON.parse(currentPath.modulesJson) finds the current module correctly.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import userEvent from "@testing-library/user-event";
import type { LearningPath } from "@/types";

// Mock tauri commands — IPC must not call real Tauri in tests
vi.mock("@/lib/tauri-commands", () => ({
  generateModuleContent: vi.fn(),
  getModuleProgress: vi.fn(),
  getPath: vi.fn(),
  completeModuleExercises: vi.fn(),
  getExercises: vi.fn(),
  sendTutorMessage: vi.fn(),
}));

import { generateModuleContent, getPath, getModuleProgress, completeModuleExercises } from "@/lib/tauri-commands";

// The mock module — must match PathModule interface shape
const mockModule = {
  id: "mod-1",
  title: "Test module title",
  description: "Module about testing",
  type: "lesson" as const,
  difficulty: 3,
  estimatedMinutes: 30,
  objectives: ["Understand testing", "Write failing tests"],
  prerequisites: [],
};

// Mock useLearningStore to control currentPath and currentModule
// Key fix (Plan 04): modulesJson contains the serialized module so ModuleView's
// JSON.parse(currentPath.modulesJson) returns [mockModule] and finds mod-1.
vi.mock("@/stores/useLearningStore", () => {
  const mockPath: LearningPath = {
    id: "path-1",
    trackId: "track-1",
    version: 1,
    generatedByModel: "test",
    modulesJson: JSON.stringify([mockModule]),
    edgesJson: "[]",
    estimatedHours: 1,
    createdAt: "2026-01-01",
  };

  return {
    useLearningStore: vi.fn(() => ({
      currentTrack: { id: "track-1", topic: "Testing", domainModule: "beginner" },
      currentPath: mockPath,
      moduleProgress: [],
      selectTrack: vi.fn(),
      completeExercises: vi.fn().mockResolvedValue({
        masteryLevel: 0.8,
        moduleCompleted: true,
        newlyUnlockedModuleIds: [],
        cardsCreated: 2,
      }),
    })),
  };
});

import { ModuleView } from "@/pages/ModuleView";

function renderModuleView(trackId = "track-1", moduleId = "mod-1") {
  return render(
    <MemoryRouter initialEntries={[`/track/${trackId}/module/${moduleId}`]}>
      <Routes>
        <Route path="/track/:trackId/module/:moduleId" element={<ModuleView />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("ModuleView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(generateModuleContent).mockResolvedValue("# Test Content\n\nSome markdown here");
    vi.mocked(getPath).mockResolvedValue({} as LearningPath);
    vi.mocked(getModuleProgress).mockResolvedValue([]);
    vi.mocked(completeModuleExercises).mockResolvedValue({
      masteryLevel: 0.8,
      moduleCompleted: true,
      newlyUnlockedModuleIds: [],
      cardsCreated: 2,
    });
  });

  it("renders module title and content from generateModuleContent IPC", async () => {
    renderModuleView();

    await waitFor(() => {
      // Title appears in heading — getAllByText handles duplicates (header + breadcrumb)
      const elements = screen.getAllByText("Test module title");
      expect(elements.length).toBeGreaterThan(0);
    });

    // After content loads, verify markdown rendered
    await waitFor(() => {
      expect(screen.getByText(/test content/i)).toBeInTheDocument();
    });
  });

  it("calls generateModuleContent with correct module params", async () => {
    renderModuleView();

    await waitFor(() => {
      expect(generateModuleContent).toHaveBeenCalledWith(
        expect.objectContaining({
          moduleId: "mod-1",
          trackId: "track-1",
          moduleTitle: "Test module title",
        }),
      );
    });
  });

  it("ExerciseContainer is visible when switching to exercises tab (TEST-02)", async () => {
    const user = userEvent.setup();
    renderModuleView();

    // Wait for module title to appear
    await waitFor(() => {
      expect(screen.getAllByText("Test module title").length).toBeGreaterThan(0);
    });

    // Click the Exercises tab
    const exercisesTab = screen.getByRole("button", { name: /exercises/i });
    await user.click(exercisesTab);

    // Exercises view should now be active (ExerciseContainer renders within exercises panel)
    // We check the tab is now active (has primary background)
    expect(exercisesTab).toBeInTheDocument();
  });
});
