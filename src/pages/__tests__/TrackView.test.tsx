// Phase 5 Plan 05 (Wave 4) — TrackView attribution tests.
//
// Covers R1 / T-05-17: skill-sourced tracks must render "From skill: <id>"
// attribution in the track header. Bundled-pack tracks and free-text AI
// tracks DO NOT show the badge (it'd be noise — bundled is the default
// expectation; AI tracks have no pack).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { LearningPath, LearningTrack } from "@/types";
import type { TopicPack } from "@/types/topic-packs";

// ── Mocks ────────────────────────────────────────────────────────────────

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useParams: () => ({ trackId: "trk-attr" }),
    useNavigate: () => vi.fn(),
  };
});

vi.mock("@/lib/tauri-commands", () => ({
  listTopicPacksAdmin: vi.fn(),
}));

import { listTopicPacksAdmin } from "@/lib/tauri-commands";
const listTopicPacksAdminMock = vi.mocked(listTopicPacksAdmin);

// Hoisted store factory so we can mutate state per test.
const mockStoreState: {
  currentTrack: LearningTrack | null;
  currentPath: LearningPath | null;
  moduleProgress: never[];
  isLoading: boolean;
  selectTrack: () => Promise<void>;
} = {
  currentTrack: null,
  currentPath: null,
  moduleProgress: [],
  isLoading: false,
  selectTrack: vi.fn().mockResolvedValue(undefined),
};

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: () => mockStoreState,
}));

// Import AFTER mocks
import { TrackView } from "@/pages/TrackView";

function makeTrack(): LearningTrack {
  return {
    id: "trk-attr",
    learnerId: "lp-1",
    topic: "Agentic DevOps",
    domainModule: "devops",
    status: "active",
    goal: "ship things",
    currentModuleId: null,
    progressPercent: 0,
    totalTimeSpent: 0,
    createdAt: "2026-06-15T00:00:00Z",
    updatedAt: "2026-06-15T00:00:00Z",
  };
}

function makePath(generatedByModel: string): LearningPath {
  return {
    id: "path-1",
    trackId: "trk-attr",
    version: 1,
    generatedByModel,
    modulesJson: "[]",
    edgesJson: "[]",
    estimatedHours: 8,
    createdAt: "2026-06-15T00:00:00Z",
  };
}

function makePackEntry(id: string, source: "bundled" | "skill"): TopicPack {
  return {
    pack: {
      id,
      title: `${id} title`,
      description: "desc",
      domain_module: "devops",
      estimated_hours: 8,
      pack_version: "1.0",
      requires_docker: false,
      modules: [],
      edges: [],
    },
    source,
    enabled: true,
    validationStatus: "ok",
    validationMessages: [],
    lastLoadedAt: "2026-06-15T00:00:00Z",
  };
}

function renderTrackView() {
  return render(
    <MemoryRouter>
      <TrackView />
    </MemoryRouter>,
  );
}

describe("TrackView pack-source attribution (R1)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStoreState.currentTrack = makeTrack();
    mockStoreState.currentPath = makePath("");
    mockStoreState.isLoading = false;
    mockStoreState.selectTrack = vi.fn().mockResolvedValue(undefined);
  });

  it("renders 'From skill: <id>' for skill-sourced tracks", async () => {
    mockStoreState.currentPath = makePath("topic-pack:my-cool-skill");
    listTopicPacksAdminMock.mockResolvedValue([
      makePackEntry("my-cool-skill", "skill"),
    ]);

    renderTrackView();

    await waitFor(() => {
      expect(screen.getByTestId("pack-attribution")).toBeInTheDocument();
    });
    const attribution = screen.getByTestId("pack-attribution");
    expect(attribution.textContent).toContain("From skill:");
    expect(attribution.textContent).toContain("my-cool-skill");
  });

  it("does not render attribution for bundled-sourced tracks", async () => {
    mockStoreState.currentPath = makePath("topic-pack:kubernetes-fundamentals");
    listTopicPacksAdminMock.mockResolvedValue([
      makePackEntry("kubernetes-fundamentals", "bundled"),
    ]);

    renderTrackView();

    // Wait for the listTopicPacksAdmin call to resolve before asserting absence.
    await waitFor(() => {
      expect(listTopicPacksAdminMock).toHaveBeenCalled();
    });
    // Tick the microtask queue once more so the resolved promise's .then() has run.
    await new Promise((r) => setTimeout(r, 10));
    expect(screen.queryByTestId("pack-attribution")).not.toBeInTheDocument();
  });

  it("does not render attribution for AI-generated tracks", async () => {
    mockStoreState.currentPath = makePath("claude-haiku-4-5");

    renderTrackView();

    // AI-generated → no `topic-pack:` prefix → no pack lookup → no badge.
    await new Promise((r) => setTimeout(r, 10));
    expect(screen.queryByTestId("pack-attribution")).not.toBeInTheDocument();
    // listTopicPacksAdmin should not even be called for non-pack paths.
    expect(listTopicPacksAdminMock).not.toHaveBeenCalled();
  });
});
