// Phase 16 Plan 02 Task 3 — Library.tsx page (LIB-01/LIB-02/LIB-03/LIB-04,
// D-01/D-02/D-04/D-06/D-08).
//
// Assembles: page header -> "Your packs" (header row + grid/empty-state,
// active-first) -> "Starter packs" (grid) -> "Import a course file".

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { LearningTrack } from "@/types";
import type { StarterPackMeta } from "@/lib/tauri-commands";

const mockLoadTracks = vi.fn();
const mockLoadStarterPacks = vi.fn();

// Selector-aware store mocks, mirroring CourseSidebar.test.tsx's precedent.
const mockLearningState = vi.hoisted(() => ({
  tracks: [] as LearningTrack[],
  loadTracks: vi.fn(),
}));
const mockLibraryState = vi.hoisted(() => ({
  starterPacks: [] as StarterPackMeta[],
  isLoading: false,
  error: null as string | null,
  loadStarterPacks: vi.fn(),
}));

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn((selector?: (s: typeof mockLearningState) => unknown) => {
    const state = { ...mockLearningState, loadTracks: mockLoadTracks };
    return typeof selector === "function" ? selector(state) : state;
  }),
}));

vi.mock("@/stores/useLibraryStore", () => ({
  useLibraryStore: vi.fn((selector?: (s: typeof mockLibraryState) => unknown) => {
    const state = { ...mockLibraryState, loadStarterPacks: mockLoadStarterPacks };
    return typeof selector === "function" ? selector(state) : state;
  }),
}));

vi.mock("@/components/library/LibraryPackCard", () => ({
  LibraryPackCard: ({ track }: { track: LearningTrack }) => (
    <div data-testid={`library-pack-card-${track.id}`}>{track.topic}</div>
  ),
}));

vi.mock("@/components/library/StarterPackCard", () => ({
  StarterPackCard: ({ pack }: { pack: StarterPackMeta }) => (
    <div data-testid={`starter-pack-card-${pack.id}`}>{pack.title}</div>
  ),
}));

import { Library } from "@/pages/Library";

function makeTrack(overrides: Partial<LearningTrack> = {}): LearningTrack {
  return {
    id: "track-1",
    learnerId: "learner-1",
    topic: "Kubernetes Fundamentals",
    domainModule: "devops",
    status: "paused",
    goal: "Learn k8s",
    currentModuleId: null,
    progressPercent: 0,
    totalTimeSpent: 0,
    createdAt: "2026-07-01T00:00:00Z",
    updatedAt: "2026-07-01T00:00:00Z",
    ...overrides,
  };
}

function renderLibrary() {
  return render(
    <MemoryRouter>
      <Library />
    </MemoryRouter>,
  );
}

describe("Library — Phase 16 Plan 02 Task 3", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockLoadTracks.mockResolvedValue(undefined);
    mockLoadStarterPacks.mockResolvedValue(undefined);
    mockLearningState.tracks = [];
    mockLibraryState.starterPacks = [];
    mockLibraryState.isLoading = false;
    mockLibraryState.error = null;
  });

  it("renders the page title and locked subtitle copy", () => {
    renderLibrary();
    expect(screen.getByRole("heading", { name: "Library" })).toBeInTheDocument();
    expect(
      screen.getByText(
        "Manage the courses you own or pick up something new.",
      ),
    ).toBeInTheDocument();
  });

  it("calls loadTracks and loadStarterPacks on mount", () => {
    renderLibrary();
    expect(mockLoadTracks).toHaveBeenCalled();
    expect(mockLoadStarterPacks).toHaveBeenCalled();
  });

  it("renders the empty state when there are zero owned tracks", () => {
    mockLearningState.tracks = [];
    renderLibrary();
    expect(screen.getByText("No packs yet")).toBeInTheDocument();
  });

  it("renders a LibraryPackCard grid, active-first, when tracks exist", () => {
    mockLearningState.tracks = [
      makeTrack({ id: "paused-1", status: "paused", topic: "Paused Pack" }),
      makeTrack({ id: "active-1", status: "active", topic: "Active Pack" }),
    ];
    renderLibrary();
    expect(screen.queryByText("No packs yet")).not.toBeInTheDocument();
    const cards = screen.getAllByTestId(/library-pack-card-/);
    expect(cards).toHaveLength(2);
    // active-first (D-06)
    expect(cards[0]).toHaveAttribute("data-testid", "library-pack-card-active-1");
    expect(cards[1]).toHaveAttribute("data-testid", "library-pack-card-paused-1");
  });

  it("renders a StarterPackCard grid from useLibraryStore starterPacks", () => {
    mockLibraryState.starterPacks = [
      { id: "k8s", title: "Kubernetes Fundamentals", description: "d", moduleCount: 3 },
      { id: "py", title: "Python for DevOps", description: "d", moduleCount: 4 },
    ];
    renderLibrary();
    expect(screen.getByTestId("starter-pack-card-k8s")).toBeInTheDocument();
    expect(screen.getByTestId("starter-pack-card-py")).toBeInTheDocument();
  });

  // WR-02 — the page must surface useLibraryStore's isLoading/error instead
  // of rendering a bare section header over an empty grid.
  it("renders the starter-pack load error when the store has an error", () => {
    mockLibraryState.error = "starter-packs resource directory not found";
    renderLibrary();
    expect(
      screen.getByText(
        /Couldn't load starter packs: starter-packs resource directory not found/i,
      ),
    ).toBeInTheDocument();
  });

  it("renders a loading indicator while starter packs are loading", () => {
    mockLibraryState.isLoading = true;
    renderLibrary();
    expect(screen.getByText(/Loading starter packs/i)).toBeInTheDocument();
  });

  it("renders an empty message when load finished with zero starter packs", () => {
    renderLibrary();
    expect(screen.getByText(/No starter packs available/i)).toBeInTheDocument();
  });

  it("shows a New Track header action linking to /onboarding (D-01)", () => {
    renderLibrary();
    const link = screen.getByRole("link", { name: /New Track/i });
    expect(link).toHaveAttribute("href", "/onboarding");
  });
});
