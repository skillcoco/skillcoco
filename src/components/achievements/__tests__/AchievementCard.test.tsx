// Phase 08.2 (Cert Simplification + Gamification) — AchievementCard tests.
//
// Updated for the new visual variants (D-21):
//   - certificate kind → large card with Download PDF button
//   - badge kind → compact pill, no export button (D-05 — milestones in-app only)
//
// Legacy 3-tier badges still render via the badge variant (D-02). No
// emojis — lucide icons + text only (D-10).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

const { exportCertificateMock, exportBadgeMock } = vi.hoisted(() => ({
  exportCertificateMock: vi.fn().mockResolvedValue({ saved: true, path: "/p.pdf" }),
  exportBadgeMock: vi.fn().mockResolvedValue({ saved: true, path: "/p.png" }),
}));

vi.mock("@/stores/useAchievementsStore", () => {
  const state = {
    exportCertificate: exportCertificateMock,
    exportBadge: exportBadgeMock,
  };
  const useStore = vi.fn((selector?: (s: typeof state) => unknown) => {
    if (typeof selector === "function") return selector(state);
    return state;
  });
  return { useAchievementsStore: useStore };
});

import { AchievementCard } from "@/components/achievements/AchievementCard";
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

describe("AchievementCard — Phase 08.2 (Cert Simplification)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("milestone_badge_renders_pill_variant_with_readable_label", () => {
    const a = makeAchievement({ level: "Milestone25", trackTopic: "Kubernetes" });
    render(<AchievementCard achievement={a} />);

    const card = screen.getByTestId(`achievement-card-${a.id}`);
    expect(card.getAttribute("data-variant")).toBe("badge");
    expect(screen.getByText(/25% Milestone/)).toBeInTheDocument();
    expect(screen.getByText(/Kubernetes/)).toBeInTheDocument();
  });

  it("milestone_pill_does_not_show_download_button", () => {
    const a = makeAchievement({ level: "Milestone50" });
    render(<AchievementCard achievement={a} />);

    expect(screen.queryByRole("button", { name: /download/i })).toBeNull();
  });

  it("completion_certificate_renders_large_card_with_download_button", () => {
    const cert = makeAchievement({
      id: "cert-1",
      kind: "certificate",
      level: "Completion",
      trackTopic: "Kubernetes",
    });
    render(<AchievementCard achievement={cert} />);

    const card = screen.getByTestId(`achievement-card-${cert.id}`);
    expect(card.getAttribute("data-variant")).toBe("certificate");
    // Title and subtitle both contain "Completion" — assert via getAllByText.
    expect(screen.getAllByText(/Completion/i).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/Kubernetes/)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /download/i }),
    ).toBeInTheDocument();
  });

  it("download_button_calls_exportCertificate_for_completion_cert", () => {
    const cert = makeAchievement({
      id: "cert-1",
      kind: "certificate",
      level: "Completion",
    });
    render(<AchievementCard achievement={cert} />);

    fireEvent.click(screen.getByRole("button", { name: /download/i }));

    expect(exportCertificateMock).toHaveBeenCalledTimes(1);
    expect(exportCertificateMock).toHaveBeenCalledWith(cert);
    expect(exportBadgeMock).not.toHaveBeenCalled();
  });

  it("legacy_3tier_badge_still_renders_via_badge_variant", () => {
    // Pre-08.2 testing-data row: kind=badge, level=Practitioner.
    const legacy = makeAchievement({
      id: "legacy-1",
      kind: "badge",
      level: "Practitioner",
    });
    render(<AchievementCard achievement={legacy} />);

    const card = screen.getByTestId(`achievement-card-${legacy.id}`);
    expect(card.getAttribute("data-variant")).toBe("badge");
    expect(screen.getByText(/Practitioner/)).toBeInTheDocument();
    // No download button for badges.
    expect(screen.queryByRole("button", { name: /download/i })).toBeNull();
  });

  it("milestone75_label_is_human_readable", () => {
    const a = makeAchievement({ level: "Milestone75" });
    render(<AchievementCard achievement={a} />);
    expect(screen.getByText(/75% Milestone/)).toBeInTheDocument();
  });

  it("no_emoji_in_rendered_output", () => {
    const cert = makeAchievement({
      kind: "certificate",
      level: "Completion",
      trackTopic: "Rust",
    });
    const { container } = render(<AchievementCard achievement={cert} />);

    const text = container.textContent ?? "";
    const emojiRegex = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;
    expect(text).not.toMatch(emojiRegex);
  });

  // ── D-10 Badge PNG export CTA (Phase 13-03) ─────────────────────────

  // Test 1 (positive, cert): cert kind shows badge-export button alongside Download PDF.
  it("cert_kind_shows_badge_export_button_alongside_pdf_button", () => {
    const cert = makeAchievement({
      id: "cert-badge-1",
      kind: "certificate",
      level: "Completion",
      trackTopic: "Kubernetes",
    });
    render(<AchievementCard achievement={cert} />);

    // Badge PNG export button must be present
    expect(
      screen.getByRole("button", { name: /badge PNG/i }),
    ).toBeInTheDocument();
    // PDF button still present
    expect(
      screen.getByRole("button", { name: /certificate PDF/i }),
    ).toBeInTheDocument();
  });

  // Test 2 (positive, legacy level): Associate/Practitioner/Professional get badge-export button.
  it.each([
    ["Associate"],
    ["Practitioner"],
    ["Professional"],
  ] as const)("legacy_level_%s_shows_badge_export_button", (level) => {
    const legacy = makeAchievement({
      id: `legacy-${level}`,
      kind: "badge",
      level,
    });
    render(<AchievementCard achievement={legacy} />);

    expect(
      screen.getByRole("button", { name: /badge PNG/i }),
    ).toBeInTheDocument();
  });

  // Test 3 (negative, D-10 boundary): Milestone chips must NOT render badge-export button.
  it.each([
    ["Milestone25"],
    ["Milestone50"],
    ["Milestone75"],
  ] as const)("milestone_%s_does_not_show_badge_export_button", (level) => {
    const milestone = makeAchievement({
      id: `milestone-${level}`,
      kind: "badge",
      level,
    });
    render(<AchievementCard achievement={milestone} />);

    expect(
      screen.queryByRole("button", { name: /badge PNG/i }),
    ).not.toBeInTheDocument();
  });

  // Test 4 (wiring): clicking badge-export button calls store exportBadge with achievement.
  it("badge_export_button_click_calls_store_exportBadge", () => {
    const cert = makeAchievement({
      id: "cert-wire-1",
      kind: "certificate",
      level: "Completion",
    });
    render(<AchievementCard achievement={cert} />);

    fireEvent.click(screen.getByRole("button", { name: /badge PNG/i }));

    expect(exportBadgeMock).toHaveBeenCalledTimes(1);
    expect(exportBadgeMock).toHaveBeenCalledWith(cert);
  });
});
