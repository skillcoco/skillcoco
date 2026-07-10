// Phase 08.2 (Cert Simplification + Gamification) — /achievements page tests.
//
// Updated for the new grouped layout (D-22):
//   - Certificates section (kind=certificate)
//   - Milestones section (kind=badge)
// Empty state copy + on-mount load + DESC sort preserved.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

const loadAchievementsMock = vi.fn().mockResolvedValue(undefined);
const exportCertificateMock = vi.fn().mockResolvedValue({ saved: true, path: "/p.pdf" });
const exportBadgeMock = vi.fn().mockResolvedValue({ saved: true, path: "/p.png" });

// Phase 18 Plan 05 (Wave 3) — Achievements now mounts the "Export skill
// report" primary button + ExportReportDialog. Mock the profile IPC so the
// mount-effect resolves cleanly; the dialog itself is covered by
// ExportReportDialog.test.tsx.
vi.mock("@/lib/tauri-commands", () => ({
  getOrCreateProfile: vi.fn().mockResolvedValue({
    id: "lp-1",
    displayName: "Ada Lovelace",
  }),
}));

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
    level: "Milestone25",
    issuedAt: "2026-06-19T00:00:00Z",
    masteryScore: 0.75,
    payloadJson: "",
    signature: "",
    keyFingerprint: "",
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

describe("Achievements page — Phase 08.2 (Cert Simplification)", () => {
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
      screen.getByText(/complete modules to earn your first milestone/i),
    ).toBeInTheDocument();
  });

  it("groups_certificates_and_milestones_into_separate_sections", () => {
    mockState = makeState({
      achievements: [
        makeAchievement({ id: "cert-1", kind: "certificate", level: "Completion" }),
        makeAchievement({ id: "m25", level: "Milestone25" }),
      ],
    });
    renderAt();

    expect(
      screen.getByTestId("achievements-page-certificates"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("achievements-page-milestones"),
    ).toBeInTheDocument();
  });

  it("renders_all_achievements_no_cap", () => {
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

  it("sorts_by_issuedAt_desc_within_group", () => {
    // All same kind (badge) → all in the milestones section.
    const a = makeAchievement({ id: "old", issuedAt: "2026-06-01T00:00:00Z" });
    const b = makeAchievement({ id: "mid", issuedAt: "2026-06-10T00:00:00Z" });
    const c = makeAchievement({ id: "new", issuedAt: "2026-06-20T00:00:00Z" });
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

  // Phase 18 Plan 05 (Wave 3) — export entry point.
  it("renders a primary 'Export skill report' button in the page header", () => {
    mockState = makeState({ achievements: [] });
    renderAt();

    const button = screen.getByTestId("export-skill-report-button");
    expect(button).toBeInTheDocument();
    expect(button.textContent).toContain("Export skill report");
    expect(button.className).toContain("bg-primary");
  });
});
