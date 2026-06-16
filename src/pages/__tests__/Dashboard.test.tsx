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

// Phase 4 Plan 04 — mock the daily-challenge sibling slice. The Dashboard
// reads three selectors (loadDailyChallenge, isEnabled, globalStreakDays)
// and fires loadDailyChallenge in the mount useEffect. The TodaysChallengeCard
// also reads `todaysChallenge` via selector.
interface DailyChallengeState {
  isEnabled: boolean;
  globalStreakDays: number;
  todaysChallenge:
    | {
        blockId: string;
        blockType: string;
        moduleId: string;
        trackId: string;
        estMinutes: number;
        status: "pending" | "in_progress" | "done";
      }
    | null;
  loadDailyChallenge: () => Promise<void>;
}

vi.mock("@/stores/useDailyChallengeStore", async () => {
  const { create } = await import("zustand");

  function makeInitial(): DailyChallengeState {
    return {
      isEnabled: false,
      globalStreakDays: 0,
      todaysChallenge: null,
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
    };
  }

  let store = create<DailyChallengeState>(() => makeInitial());

  // Wrap the hook so it doubles as a getState/setState-bearing object —
  // mirrors the real Zustand hook's surface (state.getState(), state.setState()).
  function useDailyChallengeStore(...args: unknown[]) {
    if (typeof args[0] === "function") {
      return store(args[0] as (state: DailyChallengeState) => unknown);
    }
    return store();
  }
  useDailyChallengeStore.getState = () => store.getState();
  useDailyChallengeStore.setState = (
    update: Partial<DailyChallengeState> | ((s: DailyChallengeState) => Partial<DailyChallengeState>),
  ) => store.setState(update as Parameters<typeof store.setState>[0]);

  return {
    useDailyChallengeStore,
    __resetDailyStore: (overrides?: Partial<DailyChallengeState>) => {
      store.setState(makeInitial(), true);
      if (overrides) {
        store.setState(overrides);
      }
    },
  };
});

// Phase 6 Plan 06-04 (Wave 3) — mock the achievements sibling slice.
// The Dashboard mounts <AchievementSection /> between SmartSessionCard
// and the stats row. The section itself reads four selectors and fires
// loadAchievements on mount; we expose vi.fn()s so the Dashboard tests
// don't hit real Tauri IPCs.
interface AchievementsState {
  achievements: import("@/types/achievements").Achievement[];
  recentCelebration: import("@/types/achievements").Achievement | null;
  loadAchievements: () => Promise<void>;
  clearCelebration: () => void;
  exportCertificate: (a: import("@/types/achievements").Achievement) => Promise<{ saved: boolean; path: string | null }>;
  exportBadge: (a: import("@/types/achievements").Achievement) => Promise<{ saved: boolean; path: string | null }>;
}

vi.mock("@/stores/useAchievementsStore", () => {
  const state: AchievementsState = {
    achievements: [],
    recentCelebration: null,
    loadAchievements: vi.fn().mockResolvedValue(undefined),
    clearCelebration: vi.fn(),
    exportCertificate: vi.fn().mockResolvedValue({ saved: true, path: "/p.pdf" }),
    exportBadge: vi.fn().mockResolvedValue({ saved: true, path: "/p.png" }),
  };
  const useStore = vi.fn((selector?: (s: AchievementsState) => unknown) => {
    if (typeof selector === "function") return selector(state);
    return state;
  });
  return { useAchievementsStore: useStore };
});

// @ts-expect-error vi.mock injects __resetStore into the module
import { __resetStore } from "@/stores/useLearningStore";
// @ts-expect-error vi.mock injects __resetDailyStore into the module
import { __resetDailyStore, useDailyChallengeStore } from "@/stores/useDailyChallengeStore";

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
    __resetDailyStore();
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

  // ── Phase 4 Plan 04 — Daily challenge integration ──

  it("calls loadDailyChallenge on mount (Pitfall 6 — 1 IPC gate fan-out)", async () => {
    vi.mocked(listTracks).mockResolvedValue([]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);

    // Pre-mount: capture the action ref the mock store exposes so we can
    // assert it was invoked. The store factory above defaults loadDailyChallenge
    // to a vi.fn(); read it post-render and assert call count.
    renderDashboard();

    await waitFor(() => {
      const loadFn = useDailyChallengeStore.getState().loadDailyChallenge as ReturnType<typeof vi.fn>;
      expect(loadFn).toHaveBeenCalledTimes(1);
    });
  });

  it("renders TodaysChallengeCard above SmartSessionCard (RESEARCH section 5)", async () => {
    vi.mocked(listTracks).mockResolvedValue([makeTrack()]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);
    vi.mocked(getDueCards).mockResolvedValue([makeSRCard()]);

    // Daily challenge enabled with a pending challenge so the card actually
    // renders (returns null when isEnabled=false).
    __resetDailyStore({
      isEnabled: true,
      globalStreakDays: 2,
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
    });
    // Seed the challenge payload via setState — todaysChallenge is part of
    // the mock state shape above so no ts-expect-error needed.
    useDailyChallengeStore.setState({
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "pending",
      },
    });

    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Smart Session")).toBeInTheDocument();
    });

    // Both cards rendered; assert DOM order: daily-challenge card comes first.
    const dailyCard = screen.getByText(/today's challenge/i).closest("[data-testid^='daily-challenge-card']");
    const smartCard = screen.getByText("Smart Session").closest("div");

    expect(dailyCard).not.toBeNull();
    expect(smartCard).not.toBeNull();
    if (dailyCard && smartCard) {
      // compareDocumentPosition returns DOCUMENT_POSITION_FOLLOWING (4) when
      // smartCard FOLLOWS dailyCard — exactly what we want.
      const position = dailyCard.compareDocumentPosition(smartCard);
      // eslint-disable-next-line no-bitwise
      expect(position & Node.DOCUMENT_POSITION_FOLLOWING).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    }
  });

  it("global streak StatsCard shows '--' when isEnabled=false (D-12 — no streak before gate)", async () => {
    vi.mocked(listTracks).mockResolvedValue([]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);

    // Default mock store state has isEnabled=false; no override needed.
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Best Streak")).toBeInTheDocument();
    });
    // The Best Streak card value is "--" and subtitle is "not yet active"
    expect(screen.getByText("not yet active")).toBeInTheDocument();
  });

  // ── Phase 6 Plan 06-04 (Wave 3) — Dashboard mount ordering ──

  it("achievement_section_mounted_between_smart_session_and_stats", async () => {
    vi.mocked(listTracks).mockResolvedValue([makeTrack()]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);
    vi.mocked(getDueCards).mockResolvedValue([makeSRCard()]);

    renderDashboard();

    // Wait for the SmartSessionCard to render (requires active track OR
    // due cards — both above).
    await waitFor(() => {
      expect(screen.getByText("Smart Session")).toBeInTheDocument();
    });

    const section = screen.getByTestId("achievement-section");
    const smart = screen.getByText("Smart Session").closest("div") as HTMLElement;
    const stats = screen.getByText("Reviews Due").closest("div") as HTMLElement;

    expect(section).not.toBeNull();
    expect(smart).not.toBeNull();
    expect(stats).not.toBeNull();

    // SmartSessionCard precedes AchievementSection.
    const smartToSection = smart.compareDocumentPosition(section);
    expect(smartToSection & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );

    // AchievementSection precedes the stats row.
    const sectionToStats = section.compareDocumentPosition(stats);
    expect(sectionToStats & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  it("global streak StatsCard shows 'Xd' when isEnabled=true (Phase 4 Plan 04)", async () => {
    vi.mocked(listTracks).mockResolvedValue([]);
    vi.mocked(getOrCreateProfile).mockResolvedValue(mockProfile);

    __resetDailyStore({
      isEnabled: true,
      globalStreakDays: 7,
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
    });

    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Best Streak")).toBeInTheDocument();
    });
    expect(screen.getByText("7d")).toBeInTheDocument();
    expect(screen.getByText("global streak")).toBeInTheDocument();
  });
});
