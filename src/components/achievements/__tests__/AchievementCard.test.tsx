// Phase 6 (Certification) — Plan 06-04 (Wave 3 GREEN) AchievementCard tests.
//
// Card renders one Achievement row: level + track topic + issued date + a
// kind-aware Export button (PDF for certificate / PNG for badge). No
// emojis — lucide icons + text only (D-08 + D-10).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Hoisted mock — emulates the real Zustand hook surface (selector +
// getState). The card reads exportCertificate / exportBadge actions via
// selectors and triggers them imperatively via `useAchievementsStore.getState()`
// is NOT used here — selectors only. We expose vi.fn()'s for both actions
// so the component invocation routes through them.
const exportCertificateMock = vi.fn().mockResolvedValue({ saved: true, path: "/p.pdf" });
const exportBadgeMock = vi.fn().mockResolvedValue({ saved: true, path: "/p.png" });

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

describe("AchievementCard — Phase 6 Plan 06-04 (Wave 3 GREEN)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows_level_track_date_export_button", () => {
    const a = makeAchievement({ level: "Practitioner", trackTopic: "Kubernetes" });
    render(<AchievementCard achievement={a} />);

    // Level + track topic rendered
    expect(screen.getByText(/Practitioner/)).toBeInTheDocument();
    expect(screen.getByText(/Kubernetes/)).toBeInTheDocument();
    // Export button rendered
    expect(screen.getByRole("button", { name: /export/i })).toBeInTheDocument();
  });

  it("export_button_routes_pdf_for_certificate_kind", async () => {
    const cert = makeAchievement({
      id: "cert-1",
      kind: "certificate",
      level: "Completion",
    });
    render(<AchievementCard achievement={cert} />);

    fireEvent.click(screen.getByRole("button", { name: /export/i }));

    expect(exportCertificateMock).toHaveBeenCalledTimes(1);
    expect(exportCertificateMock).toHaveBeenCalledWith(cert);
    expect(exportBadgeMock).not.toHaveBeenCalled();
  });

  it("export_button_routes_png_for_badge_kind", async () => {
    const badge = makeAchievement({
      id: "badge-1",
      kind: "badge",
      level: "Associate",
    });
    render(<AchievementCard achievement={badge} />);

    fireEvent.click(screen.getByRole("button", { name: /export/i }));

    expect(exportBadgeMock).toHaveBeenCalledTimes(1);
    expect(exportBadgeMock).toHaveBeenCalledWith(badge);
    expect(exportCertificateMock).not.toHaveBeenCalled();
  });

  it("no_emoji_in_rendered_output", () => {
    const a = makeAchievement({ level: "Professional", trackTopic: "Rust" });
    const { container } = render(<AchievementCard achievement={a} />);

    // Strip any text node content + attribute values + check for
    // pictographic emoji code points (a coarse net: U+1F300-U+1FAFF,
    // U+2600-U+27BF for older emoji blocks).
    const text = container.textContent ?? "";
    const emojiRegex = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;
    expect(text).not.toMatch(emojiRegex);
  });
});
