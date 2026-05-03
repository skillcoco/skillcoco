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
  getAuthStatus: vi.fn(),
}));

import { generateModuleContent, getPath, getModuleProgress, completeModuleExercises } from "@/lib/tauri-commands";

// The mock module — inlined as literal (not using outer const) to avoid vi.mock hoisting issue.
// vi.mock factory is hoisted to top of file, so references to outer const variables fail.
vi.mock("@/stores/useLearningStore", () => {
  // Define inline — must NOT reference outer const due to hoisting
  const inlineMockModule = {
    id: "mod-1",
    title: "Test module title",
    description: "Module about testing",
    type: "lesson",
    difficulty: 3,
    estimatedMinutes: 30,
    objectives: ["Understand testing", "Write failing tests"],
    prerequisites: [],
  };

  // Key fix (Plan 04): modulesJson contains the serialized module so ModuleView's
  // JSON.parse(currentPath.modulesJson) returns [inlineMockModule] and finds mod-1.
  const mockPath = {
    id: "path-1",
    trackId: "track-1",
    version: 1,
    generatedByModel: "test",
    modulesJson: JSON.stringify([inlineMockModule]),
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

// Module title used in assertions — must match inlineMockModule.title inside vi.mock
const TEST_MODULE_TITLE = "Test module title";

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
      const elements = screen.getAllByText(TEST_MODULE_TITLE);
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
          moduleTitle: TEST_MODULE_TITLE,
        }),
      );
    });
  });

  it("shows module objectives on render (TEST-02)", async () => {
    renderModuleView();

    // Wait for module to render
    await waitFor(() => {
      expect(screen.getAllByText(TEST_MODULE_TITLE).length).toBeGreaterThan(0);
    });

    // The Learning Objectives section shows the module objectives
    await waitFor(() => {
      expect(screen.getByText("Learning Objectives")).toBeInTheDocument();
      expect(screen.getByText("Understand testing")).toBeInTheDocument();
    });
  });
});
