// Phase 08.2 (Cert Simplification + Gamification) — Dashboard section tests.
//
// Updated for the new grouped layout (D-21):
//   - Certificates section (kind=certificate, large cards)
//   - Milestones section (kind=badge, compact pills)
// Empty state + on-mount load + non-modal celebration banner all
// preserved from Phase 6.

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
    level: "Milestone25",
    issuedAt: "2026-06-19T12:00:00Z",
    masteryScore: 0.75,
    payloadJson: "",
    signature: "",
    keyFingerprint: "",
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

describe("AchievementSection — Phase 08.2 (Cert Simplification)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockState = makeState();
  });

  it("renders_empty_state_when_no_achievements", () => {
    mockState = makeState({ achievements: [] });
    renderWithRouter();
    expect(screen.getByText(/no achievements yet/i)).toBeInTheDocument();
  });

  it("groups_certificates_and_milestones_separately", () => {
    mockState = makeState({
      achievements: [
        makeAchievement({ id: "cert-1", kind: "certificate", level: "Completion" }),
        makeAchievement({ id: "m25", level: "Milestone25" }),
        makeAchievement({ id: "m50", level: "Milestone50" }),
      ],
    });
    renderWithRouter();

    expect(screen.getByTestId("achievements-certificates")).toBeInTheDocument();
    expect(screen.getByTestId("achievements-milestones")).toBeInTheDocument();
  });

  it("certificates_section_omits_when_no_certificates", () => {
    mockState = makeState({
      achievements: [
        makeAchievement({ id: "m25", level: "Milestone25" }),
      ],
    });
    renderWithRouter();

    expect(screen.queryByTestId("achievements-certificates")).toBeNull();
    expect(screen.getByTestId("achievements-milestones")).toBeInTheDocument();
  });

  it("milestones_section_omits_when_no_milestones", () => {
    mockState = makeState({
      achievements: [
        makeAchievement({ id: "cert-1", kind: "certificate", level: "Completion" }),
      ],
    });
    renderWithRouter();

    expect(screen.getByTestId("achievements-certificates")).toBeInTheDocument();
    expect(screen.queryByTestId("achievements-milestones")).toBeNull();
  });

  it("certificates_section_caps_at_six_cards", () => {
    const eight = Array.from({ length: 8 }, (_, i) =>
      makeAchievement({
        id: `cert-${i}`,
        kind: "certificate",
        level: "Completion",
        trackId: `t${i}`,
        issuedAt: `2026-06-${String(10 + i).padStart(2, "0")}T00:00:00Z`,
      }),
    );
    mockState = makeState({ achievements: eight });
    renderWithRouter();

    const cards = screen.getAllByTestId(/^achievement-card-/);
    expect(cards).toHaveLength(6);
  });

  it("milestones_section_caps_at_six_pills", () => {
    const eight = Array.from({ length: 8 }, (_, i) =>
      makeAchievement({
        id: `m-${i}`,
        kind: "badge",
        level: "Milestone25",
        trackId: `t${i}`,
        issuedAt: `2026-06-${String(10 + i).padStart(2, "0")}T00:00:00Z`,
      }),
    );
    mockState = makeState({ achievements: eight });
    renderWithRouter();

    const cards = screen.getAllByTestId(/^achievement-card-/);
    expect(cards).toHaveLength(6);
  });

  it("renders_view_all_link_when_more_than_12_total", () => {
    const cards = Array.from({ length: 14 }, (_, i) =>
      makeAchievement({
        id: `a-${i}`,
        kind: i % 2 === 0 ? "certificate" : "badge",
        level: i % 2 === 0 ? "Completion" : "Milestone25",
        trackId: `t${i}`,
      }),
    );
    mockState = makeState({ achievements: cards });
    renderWithRouter();

    const link = screen.getByTestId("achievements-view-all");
    expect(link).toBeInTheDocument();
    expect(link.getAttribute("href")).toBe("/achievements");
  });

  it("hides_view_all_link_when_12_or_fewer", () => {
    const cards = Array.from({ length: 5 }, (_, i) =>
      makeAchievement({ id: `a-${i}`, trackId: `t${i}` }),
    );
    mockState = makeState({ achievements: cards });
    renderWithRouter();

    expect(screen.queryByTestId("achievements-view-all")).toBeNull();
  });

  it("loads_achievements_on_mount", () => {
    mockState = makeState({ achievements: [] });
    renderWithRouter();

    expect(loadAchievementsMock).toHaveBeenCalledTimes(1);
  });

  it("celebration_banner_shows_then_dismisses", () => {
    vi.useFakeTimers();
    try {
      const celeb = makeAchievement({
        id: "celeb-1",
        level: "Milestone25",
        trackTopic: "Kubernetes",
      });
      mockState = makeState({ recentCelebration: celeb });
      renderWithRouter();

      expect(
        screen.getByText(/You just earned Milestone25 in Kubernetes/i),
      ).toBeInTheDocument();

      act(() => {
        vi.advanceTimersByTime(5000);
      });

      expect(clearCelebrationMock).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });
});
