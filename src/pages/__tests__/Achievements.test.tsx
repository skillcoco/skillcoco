// Phase 6 (Certification) — Plan 06-04 (Wave 3 GREEN) /achievements page.
//
// D-09 fully closes when the Dashboard "View all" link routes to this page
// instead of 404'ing. The page reuses AchievementCard from Wave 3 and
// renders EVERY achievement (no 6-card cap), sorted by issuedAt DESC.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

const loadAchievementsMock = vi.fn().mockResolvedValue(undefined);
const exportCertificateMock = vi.fn().mockResolvedValue({ saved: true, path: "/p.pdf" });
const exportBadgeMock = vi.fn().mockResolvedValue({ saved: true, path: "/p.png" });

interface SliceShape {
  achievements: import("@/types/achievements").Achievement[];
  loadAchievements: typeof loadAchievementsMock;
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

import { Achievements } from "@/pages/Achievements";
import type { Achievement } from "@/types/achievements";

function makeAchievement(overrides: Partial<Achievement> = {}): Achievement {
  return {
    id: "ach-1",
    learnerId: "lnr-1",
    trackId: "trk-1",
    packId: null,
    kind: "badge",
    level: "Associate",
    issuedAt: "2026-06-16T00:00:00Z",
    masteryScore: 0.75,
    payloadJson: "",
    signature: "",
    keyFingerprint: "deadbeef",
    trackTopic: "Kubernetes",
    ...overrides,
  };
}

function makeState(overrides: Partial<SliceShape> = {}): SliceShape {
  return {
    achievements: [],
    loadAchievements: loadAchievementsMock,
    exportCertificate: exportCertificateMock,
    exportBadge: exportBadgeMock,
    ...overrides,
  };
}

function renderAt() {
  return render(
    <MemoryRouter initialEntries={["/achievements"]}>
      <Achievements />
    </MemoryRouter>,
  );
}

describe("Achievements page — Phase 6 Plan 06-04 (Wave 3 GREEN)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockState = makeState();
  });

  it("renders_heading_and_empty_state_when_no_achievements", () => {
    mockState = makeState({ achievements: [] });
    renderAt();

    expect(
      screen.getByRole("heading", { name: /All Achievements/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/no achievements yet/i)).toBeInTheDocument();
    expect(
      screen.getByText(/complete modules to earn your first badge/i),
    ).toBeInTheDocument();
  });

  it("renders_all_achievements_when_store_has_more_than_six", () => {
    const eight = Array.from({ length: 8 }, (_, i) =>
      makeAchievement({
        id: `a-${i}`,
        issuedAt: `2026-06-${String(10 + i).padStart(2, "0")}T00:00:00Z`,
      }),
    );
    mockState = makeState({ achievements: eight });
    renderAt();

    const cards = screen.getAllByTestId(/^achievement-card-/);
    expect(cards).toHaveLength(8);
  });

  it("sorts_by_issuedAt_desc_by_default", () => {
    const a = makeAchievement({ id: "old", issuedAt: "2026-06-01T00:00:00Z" });
    const b = makeAchievement({ id: "mid", issuedAt: "2026-06-10T00:00:00Z" });
    const c = makeAchievement({ id: "new", issuedAt: "2026-06-20T00:00:00Z" });
    // Insert in "wrong" order so the page must sort itself.
    mockState = makeState({ achievements: [a, b, c] });
    renderAt();

    const cards = screen.getAllByTestId(/^achievement-card-/);
    expect(cards.map((el) => el.getAttribute("data-testid"))).toEqual([
      "achievement-card-new",
      "achievement-card-mid",
      "achievement-card-old",
    ]);
  });

  it("calls_loadAchievements_on_mount", () => {
    mockState = makeState({ achievements: [] });
    renderAt();

    expect(loadAchievementsMock).toHaveBeenCalledTimes(1);
  });
});
