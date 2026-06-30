// Phase 5 Plan 05 (Wave 4) — TrackView attribution tests.
//
// Covers R1 / T-05-17: skill-sourced tracks must render "From skill: <id>"
// attribution in the track header. Bundled-pack tracks and free-text AI
// tracks DO NOT show the badge (it'd be noise — bundled is the default
// expectation; AI tracks have no pack).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
  // Plan 06-05 (Wave 4) — CertificationProgress, mounted in TrackView,
  // calls getTrackCertifications on mount. Mock it so the TrackView tests
  // do not error out on missing IPC. We don't care about its exact
  // behavior here — that's covered by CertificationProgress.test.tsx.
  getTrackCertifications: vi.fn().mockResolvedValue({
    earnedLevels: [],
    nextLevel: "Associate",
    criteria: "Master 25% of modules",
  }),
}));

// Plan 06-05 (Wave 4) — CertificationProgress also reads the
// useAchievementsStore. Stub it out so TrackView.test.tsx does not touch
// the real store (which would bring its own IPCs in).
const achievementsMockState = {
  achievements: [] as unknown[],
  exportCertificate: vi.fn(),
};
vi.mock("@/stores/useAchievementsStore", () => ({
  useAchievementsStore: <T,>(
    selector?: (s: typeof achievementsMockState) => T,
  ): T | typeof achievementsMockState => {
    if (typeof selector === "function") return selector(achievementsMockState);
    return achievementsMockState;
  },
}));

import { listTopicPacksAdmin } from "@/lib/tauri-commands";
const listTopicPacksAdminMock = vi.mocked(listTopicPacksAdmin);

// Hoisted store factory so we can mutate state per test.
const mockSetTrackBrowseMode = vi.fn();

const mockStoreState: {
  currentTrack: LearningTrack | null;
  currentPath: LearningPath | null;
  moduleProgress: never[];
  isLoading: boolean;
  selectTrack: () => Promise<void>;
  setTrackBrowseMode: typeof mockSetTrackBrowseMode;
} = {
  currentTrack: null,
  currentPath: null,
  moduleProgress: [],
  isLoading: false,
  selectTrack: vi.fn().mockResolvedValue(undefined),
  setTrackBrowseMode: mockSetTrackBrowseMode,
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

describe("TrackView CertificationProgress mount (Plan 06-05 Wave 4)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStoreState.currentTrack = makeTrack();
    mockStoreState.currentPath = makePath("");
    mockStoreState.isLoading = false;
    mockStoreState.selectTrack = vi.fn().mockResolvedValue(undefined);
  });

  it("mounts CertificationProgress with the current trackId", async () => {
    renderTrackView();

    // CertificationProgress renders a section with testid
    // "certification-progress" once its IPC resolves. The IPC mock above
    // resolves synchronously, so the section appears after a microtask.
    await waitFor(() => {
      expect(screen.getByTestId("certification-progress")).toBeInTheDocument();
    });
  });
});

describe("TrackView browse-mode toggle (Plan 10-03 Task 1)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStoreState.currentTrack = makeTrack();
    mockStoreState.currentPath = makePath("");
    mockStoreState.isLoading = false;
    mockStoreState.selectTrack = vi.fn().mockResolvedValue(undefined);
    mockStoreState.setTrackBrowseMode = mockSetTrackBrowseMode;
    mockSetTrackBrowseMode.mockResolvedValue(undefined);
    listTopicPacksAdminMock.mockResolvedValue([]);
  });

  it("renders browse-mode-toggle in the header", () => {
    renderTrackView();
    expect(screen.getByTestId("browse-mode-toggle")).toBeInTheDocument();
  });

  it("defaults to linear when browseMode is undefined", () => {
    mockStoreState.currentTrack = { ...makeTrack(), browseMode: undefined };
    renderTrackView();
    const toggle = screen.getByTestId("browse-mode-toggle");
    expect(toggle).toHaveValue("linear");
  });

  it("shows free when browseMode is free", () => {
    mockStoreState.currentTrack = { ...makeTrack(), browseMode: "free" as const };
    renderTrackView();
    const toggle = screen.getByTestId("browse-mode-toggle");
    expect(toggle).toHaveValue("free");
  });

  it("calls setTrackBrowseMode with 'free' when toggled to free", async () => {
    const user = userEvent.setup();
    renderTrackView();
    const toggle = screen.getByTestId("browse-mode-toggle");
    await user.selectOptions(toggle, "free");
    expect(mockSetTrackBrowseMode).toHaveBeenCalledWith("trk-attr", "free");
  });

  it("calls setTrackBrowseMode with 'linear' when toggled back to linear", async () => {
    const user = userEvent.setup();
    mockStoreState.currentTrack = { ...makeTrack(), browseMode: "free" as const };
    renderTrackView();
    const toggle = screen.getByTestId("browse-mode-toggle");
    await user.selectOptions(toggle, "linear");
    expect(mockSetTrackBrowseMode).toHaveBeenCalledWith("trk-attr", "linear");
  });
});
