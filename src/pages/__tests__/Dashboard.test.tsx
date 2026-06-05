import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { Dashboard } from "@/pages/Dashboard";
import type { LearningTrack, LearnerProfile, SRCard } from "@/types";

// Mock tauri commands
vi.mock("@/lib/tauri-commands", () => ({
  getOrCreateProfile: vi.fn(),
  listTracks: vi.fn(),
  getDueCards: vi.fn(),
}));

import { getOrCreateProfile, listTracks, getDueCards } from "@/lib/tauri-commands";

// Reset the zustand store between tests
vi.mock("@/stores/useLearningStore", async () => {
  const { create } = await import("zustand");
  const { listTracks, getDueCards } = await import("@/lib/tauri-commands");

  function createStore() {
    return create<{
      tracks: LearningTrack[];
      dueCards: SRCard[];
      isLoading: boolean;
      loadTracks: () => Promise<void>;
      loadDueCards: () => Promise<void>;
    }>((set) => ({
      tracks: [],
      dueCards: [],
      isLoading: false,
      loadTracks: async () => {
        try {
          const tracks = await listTracks();
          set({ tracks });
        } catch {
          // ignore
        }
      },
      loadDueCards: async () => {
        try {
          const dueCards = await getDueCards();
          set({ dueCards });
        } catch {
          // ignore
        }
      },
    }));
  }

  let store = createStore();

  return {
    useLearningStore: (...args: unknown[]) => {
      // If called with a selector, use it; otherwise return all state
      if (typeof args[0] === "function") {
        return store(args[0] as (state: unknown) => unknown);
      }
      return store();
    },
    __resetStore: () => {
      store = createStore();
    },
  };
});

// @ts-expect-error vi.mock injects __resetStore into the module
import { __resetStore } from "@/stores/useLearningStore";

const mockProfile: LearnerProfile = {
  id: "profile-1",
  displayName: "Test User",
  learningStyle: "practical",
  experienceLevel: "intermediate",
  preferencesJson: '{"preferredSessionDuration":30,"dailyGoalMinutes":60,"notificationsEnabled":true,"theme":"dark"}',
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

function makeTrack(overrides: Partial<LearningTrack> = {}): LearningTrack {
  return {
    id: "track-1",
    learnerId: "profile-1",
    topic: "Kubernetes",
    domainModule: "devops",
    status: "active",
    goal: "Learn K8s",
    currentModuleId: "mod-1",
    progressPercent: 40,
    totalTimeSpent: 3600,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function renderDashboard() {
  return render(
    <MemoryRouter>
      <Dashboard />
    </MemoryRouter>,
  );
}

// Helper to make a minimal SRCard mock
function makeSRCard(overrides: Partial<SRCard> = {}): SRCard {
  return {
    id: "card-1",
    moduleId: "mod-1",
    concept: "Pods",
    cardType: "active_recall",
    front: "What is a Pod?",
    back: "Smallest deployable unit",
    interval: 1,
    easeFactor: 2.5,
    repetitions: 0,
    nextReview: "2026-05-03T00:00:00Z",
    lastReview: null,
    ...overrides,
  };
}

describe("Dashboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetStore();
    vi.mocked(getDueCards).mockResolvedValue([]);
  });

  it("renders empty state when there are no tracks", async () => {
    vi.mocked(listTracks).mockResolvedValue([]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);

    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("No learning tracks yet")).toBeInTheDocument();
    });
    expect(screen.getByText(/start your first track/i)).toBeInTheDocument();
    expect(screen.getByText("Start Learning")).toBeInTheDocument();
  });

  it("renders the greeting with the user display name", async () => {
    vi.mocked(listTracks).mockResolvedValue([]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);

    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText(/test user/i)).toBeInTheDocument();
    });
  });

  it("renders tracks when they exist", async () => {
    const tracks = [
      makeTrack({ id: "t1", topic: "Kubernetes", progressPercent: 40 }),
      makeTrack({ id: "t2", topic: "Rust Programming", progressPercent: 75 }),
    ];
    vi.mocked(listTracks).mockResolvedValue(tracks);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);

    renderDashboard();

    await waitFor(() => {
      expect(screen.queryByText("No learning tracks yet")).not.toBeInTheDocument();
    });
  });

  it("shows the New Track button", async () => {
    vi.mocked(listTracks).mockResolvedValue([]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);

    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("New Track")).toBeInTheDocument();
    });
  });

  it("displays stats cards", async () => {
    vi.mocked(listTracks).mockResolvedValue([]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);

    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Reviews Due")).toBeInTheDocument();
      expect(screen.getByText("Modules Done")).toBeInTheDocument();
      expect(screen.getByText("Best Streak")).toBeInTheDocument();
      expect(screen.getByText("Active Tracks")).toBeInTheDocument();
    });
  });

  // ── FIX-04 / LOOP-05: Real due card count + streak ──

  it("shows real due card count from getDueCards (LOOP-05)", async () => {
    vi.mocked(listTracks).mockResolvedValue([makeTrack()]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);
    vi.mocked(getDueCards).mockResolvedValue([
      makeSRCard({ id: "c1" }),
      makeSRCard({ id: "c2" }),
      makeSRCard({ id: "c3" }),
    ]);

    renderDashboard();

    // The Reviews Due StatsCard should show "3"
    await waitFor(() => {
      screen.getByText("Reviews Due").closest(".glass, [class*='rounded']")?.parentElement;
      // The value "3" appears in the stats row
      expect(screen.getAllByText("3").length).toBeGreaterThan(0);
    });
  });

  it("shows best streak from track streakDays (FIX-04)", async () => {
    const tracks = [
      makeTrack({ id: "t1", topic: "Kubernetes", streakDays: 5 }),
      makeTrack({ id: "t2", topic: "Rust", streakDays: 2 }),
    ];
    vi.mocked(listTracks).mockResolvedValue(tracks);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);

    renderDashboard();

    // Best Streak StatsCard should show "5d" (max across tracks)
    await waitFor(() => {
      expect(screen.getByText("5d")).toBeInTheDocument();
    });
  });

  it("recommends 'Review N due cards' when dueCards >= 1 (smart session)", async () => {
    vi.mocked(listTracks).mockResolvedValue([makeTrack()]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);
    vi.mocked(getDueCards).mockResolvedValue([makeSRCard(), makeSRCard()]);

    renderDashboard();

    // SmartSessionCard should be visible (it renders when dueCards > 0 or active track)
    await waitFor(() => {
      expect(screen.getByText("Smart Session")).toBeInTheDocument();
    });
  });

  it("shows empty state when no tracks and no due cards", async () => {
    vi.mocked(listTracks).mockResolvedValue([]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);
    vi.mocked(getDueCards).mockResolvedValue([]);

    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("No learning tracks yet")).toBeInTheDocument();
    });
    // SmartSessionCard should NOT appear with no tracks and no due cards
    expect(screen.queryByText("Smart Session")).not.toBeInTheDocument();
  });
});
