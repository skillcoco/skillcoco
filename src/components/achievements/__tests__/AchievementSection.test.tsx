// Phase 6 (Certification) — Plan 06-04 (Wave 3 GREEN) AchievementSection tests.
//
// Wave 0 shipped a RED skip for the empty-state copy; Wave 3 flips it
// GREEN AND adds the View-all link, 6-card cap, on-mount load, and the
// non-modal 5-second celebration banner.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

const loadAchievementsMock = vi.fn().mockResolvedValue(undefined);
const clearCelebrationMock = vi.fn();
const exportCertificateMock = vi.fn().mockResolvedValue({ saved: true, path: "/p.pdf" });
const exportBadgeMock = vi.fn().mockResolvedValue({ saved: true, path: "/p.png" });

interface SliceShape {
  achievements: import("@/types/achievements").Achievement[];
  recentCelebration: import("@/types/achievements").Achievement | null;
  loadAchievements: typeof loadAchievementsMock;
  clearCelebration: typeof clearCelebrationMock;
  exportCertificate: typeof exportCertificateMock;
  exportBadge: typeof exportBadgeMock;
}

let mockState: SliceShape;

vi.mock("@/stores/useAchievementsStore", () => ({
  useAchievementsStore: vi.fn((selector?: (s: SliceShape) => unknown) => {
    if (typeof selector === "function") return selector(mockState);
    return mockState;
  }),
}));

import { AchievementSection } from "@/components/achievements/AchievementSection";
import type { Achievement } from "@/types/achievements";

function makeAchievement(overrides: Partial<Achievement> = {}): Achievement {
  return {
    id: "ach-1",
    learnerId: "lnr-1",
    trackId: "trk-1",
    packId: null,
    kind: "badge",
    level: "Associate",
    issuedAt: "2026-06-16T12:00:00Z",
    masteryScore: 0.75,
    payloadJson: "",
    signature: "",
    keyFingerprint: "deadbeef",
    trackTopic: "Kubernetes",
    ...overrides,
  };
}

function renderWithRouter() {
  return render(
    <MemoryRouter>
      <AchievementSection />
    </MemoryRouter>,
  );
}

function makeState(overrides: Partial<SliceShape> = {}): SliceShape {
  return {
    achievements: [],
    recentCelebration: null,
    loadAchievements: loadAchievementsMock,
    clearCelebration: clearCelebrationMock,
    exportCertificate: exportCertificateMock,
    exportBadge: exportBadgeMock,
    ...overrides,
  };
}

describe("AchievementSection — Phase 6 Plan 06-04 (Wave 3 GREEN)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockState = makeState();
  });

  it("renders_empty_state_when_no_achievements (Wave 0 RED → GREEN)", () => {
    mockState = makeState({ achievements: [] });
    renderWithRouter();
    expect(screen.getByText(/no achievements yet/i)).toBeInTheDocument();
  });

  it("renders_six_most_recent_achievements", () => {
    const eight = Array.from({ length: 8 }, (_, i) =>
      makeAchievement({
        id: `a-${i}`,
        issuedAt: `2026-06-${String(10 + i).padStart(2, "0")}T00:00:00Z`,
        trackTopic: `Track ${i}`,
      }),
    );
    // Newest first ordering — appendNewlyIssued prepends, so we sort DESC
    // here too so the test mirrors the store contract.
    const sorted = [...eight].sort((a, b) =>
      b.issuedAt.localeCompare(a.issuedAt),
    );
    mockState = makeState({ achievements: sorted });
    renderWithRouter();

    const cards = screen.getAllByTestId(/^achievement-card-/);
    expect(cards).toHaveLength(6);
    // Newest first — Track 7 is highest-numbered (latest date)
    expect(cards[0].textContent).toMatch(/Track 7/);
  });

  it("renders_view_all_link_when_more_than_six", () => {
    const eight = Array.from({ length: 8 }, (_, i) =>
      makeAchievement({ id: `a-${i}`, issuedAt: `2026-06-${String(10 + i).padStart(2, "0")}T00:00:00Z` }),
    );
    mockState = makeState({ achievements: eight });
    renderWithRouter();

    const link = screen.getByTestId("achievements-view-all");
    expect(link).toBeInTheDocument();
    expect(link.getAttribute("href")).toBe("/achievements");
  });

  it("hides_view_all_link_when_five_or_fewer", () => {
    const five = Array.from({ length: 5 }, (_, i) =>
      makeAchievement({ id: `a-${i}` }),
    );
    mockState = makeState({ achievements: five });
    renderWithRouter();

    expect(screen.queryByTestId("achievements-view-all")).toBeNull();
  });

  it("loads_achievements_on_mount", () => {
    mockState = makeState({ achievements: [] });
    renderWithRouter();

    expect(loadAchievementsMock).toHaveBeenCalledTimes(1);
  });

  it("celebration_banner_shows_then_dismisses", async () => {
    vi.useFakeTimers();
    try {
      const celeb = makeAchievement({
        id: "celeb-1",
        level: "Practitioner",
        trackTopic: "Kubernetes",
      });
      mockState = makeState({ recentCelebration: celeb });
      renderWithRouter();

      expect(
        screen.getByText(/You just earned Practitioner in Kubernetes/i),
      ).toBeInTheDocument();

      // Advance 5s — the timeout calls clearCelebration.
      act(() => {
        vi.advanceTimersByTime(5000);
      });

      expect(clearCelebrationMock).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });
});
