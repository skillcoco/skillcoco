// Phase 6 (Certification) — Plan 06-04 (Wave 3 GREEN) store tests.
//
// Sibling-slice pattern (NOT extension of useLearningStore) per Phase 4
// Pitfall 5 + Phase 03.1 useLabStore precedent. Wave 3 expands the Wave 0
// contract with full action coverage: loadAchievements, appendNewlyIssued
// (idempotent), exportCertificate (PDF), exportBadge (PNG), clearCelebration.

import { describe, it, expect, vi, beforeEach } from "vitest";

// Vitest hoisting rule: inline literals only inside the factory body.
vi.mock("@/lib/tauri-commands", () => ({
  listAchievements: vi.fn(),
  exportCertificate: vi.fn(),
  exportBadge: vi.fn(),
}));

import {
  useAchievementsStore,
  __resetStore,
} from "@/stores/useAchievementsStore";
import {
  listAchievements,
  exportCertificate as exportCertificateCmd,
  exportBadge as exportBadgeCmd,
} from "@/lib/tauri-commands";
import type { Achievement } from "@/types/achievements";

function makeAchievement(overrides: Partial<Achievement> = {}): Achievement {
  return {
    id: "ach-1",
    learnerId: "lnr-1",
    trackId: "trk-1",
    packId: null,
    kind: "badge",
    level: "Associate",
    issuedAt: "2026-06-01T00:00:00Z",
    masteryScore: 0.75,
    payloadJson: "",
    signature: "",
    keyFingerprint: "deadbeef",
    trackTopic: "Kubernetes",
    ...overrides,
  };
}

describe("useAchievementsStore — Phase 6 Plan 06-04 (Wave 3 GREEN)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetStore();
  });

  it("loads_empty_list_on_init", () => {
    const s = useAchievementsStore.getState();
    expect(s.achievements).toEqual([]);
    expect(s.isLoading).toBe(false);
    expect(s.error).toBeNull();
    expect(s.recentCelebration).toBeNull();
  });

  it("loadAchievements_calls_ipc_and_populates_state", async () => {
    const list = [
      makeAchievement({ id: "a1" }),
      makeAchievement({ id: "a2" }),
      makeAchievement({ id: "a3" }),
    ];
    vi.mocked(listAchievements).mockResolvedValue(list);

    await useAchievementsStore.getState().loadAchievements();

    expect(listAchievements).toHaveBeenCalledTimes(1);
    const next = useAchievementsStore.getState();
    expect(next.achievements).toHaveLength(3);
    expect(next.isLoading).toBe(false);
    expect(next.error).toBeNull();
  });

  it("loadAchievements_handles_error", async () => {
    vi.mocked(listAchievements).mockRejectedValue(new Error("boom"));

    await useAchievementsStore.getState().loadAchievements();

    const next = useAchievementsStore.getState();
    expect(next.error).toMatch(/boom/);
    expect(next.isLoading).toBe(false);
    expect(next.achievements).toEqual([]);
  });

  it("appendNewlyIssued_prepends_items", () => {
    const a = makeAchievement({ id: "a", issuedAt: "2026-06-01T00:00:00Z" });
    const b = makeAchievement({ id: "b", issuedAt: "2026-06-02T00:00:00Z" });
    const c = makeAchievement({ id: "c", issuedAt: "2026-06-10T00:00:00Z" });
    const d = makeAchievement({ id: "d", issuedAt: "2026-06-11T00:00:00Z" });

    useAchievementsStore.setState({ achievements: [a, b] });
    useAchievementsStore.getState().appendNewlyIssued([c, d]);

    const next = useAchievementsStore.getState();
    expect(next.achievements.map((x) => x.id)).toEqual(["c", "d", "a", "b"]);
  });

  it("appendNewlyIssued_deduplicates_by_id", () => {
    const a = makeAchievement({ id: "1" });
    const b = makeAchievement({ id: "2" });
    useAchievementsStore.setState({ achievements: [a, b] });

    useAchievementsStore.getState().appendNewlyIssued([a]);

    const next = useAchievementsStore.getState();
    expect(next.achievements.map((x) => x.id)).toEqual(["1", "2"]);
  });

  it("appendNewlyIssued_sets_recentCelebration_to_highest_tier", () => {
    const ass = makeAchievement({ id: "low", level: "Associate" });
    const prof = makeAchievement({ id: "high", level: "Professional" });
    useAchievementsStore.getState().appendNewlyIssued([ass, prof]);

    const next = useAchievementsStore.getState();
    expect(next.recentCelebration?.id).toBe("high");
  });

  it("clearCelebration_resets_recentCelebration_to_null", () => {
    useAchievementsStore.setState({ recentCelebration: makeAchievement() });
    useAchievementsStore.getState().clearCelebration();
    expect(useAchievementsStore.getState().recentCelebration).toBeNull();
  });

  it("exportCertificate_invokes_ipc_with_suggested_filename", async () => {
    const cert = makeAchievement({
      id: "cert-1",
      kind: "certificate",
      level: "Completion",
      trackTopic: "Kubernetes Fundamentals",
      issuedAt: "2026-06-16T12:34:56Z",
    });
    vi.mocked(exportCertificateCmd).mockResolvedValue("/tmp/cert.pdf");

    const result = await useAchievementsStore
      .getState()
      .exportCertificate(cert);

    expect(exportCertificateCmd).toHaveBeenCalledTimes(1);
    const [reqArg, filenameArg] = vi.mocked(exportCertificateCmd).mock.calls[0];
    expect(reqArg).toEqual({ achievementId: "cert-1" });
    expect(filenameArg).toMatch(/^skillcoco-certificate-kubernetes-fundamentals-20260616\.pdf$/);
    expect(result).toEqual({ saved: true, path: "/tmp/cert.pdf" });
  });

  it("exportCertificate_returns_null_when_user_cancels", async () => {
    const cert = makeAchievement({ id: "c2", kind: "certificate", level: "Completion" });
    vi.mocked(exportCertificateCmd).mockResolvedValue(null);

    const result = await useAchievementsStore
      .getState()
      .exportCertificate(cert);

    expect(result).toEqual({ saved: false, path: null });
  });

  it("exportCertificate_refuses_non_certificate_kinds", async () => {
    const badge = makeAchievement({ id: "b1", kind: "badge", level: "Associate" });
    const result = await useAchievementsStore
      .getState()
      .exportCertificate(badge);

    expect(exportCertificateCmd).not.toHaveBeenCalled();
    expect(result).toEqual({ saved: false, path: null });
  });

  it("exportBadge_invokes_ipc_with_png_suggested_filename", async () => {
    const badge = makeAchievement({
      id: "badge-1",
      kind: "badge",
      level: "Practitioner",
      trackTopic: "DevOps & Tooling!",
      issuedAt: "2026-06-16T00:00:00Z",
    });
    vi.mocked(exportBadgeCmd).mockResolvedValue("/tmp/badge.png");

    const result = await useAchievementsStore
      .getState()
      .exportBadge(badge);

    expect(exportBadgeCmd).toHaveBeenCalledTimes(1);
    const [reqArg, filenameArg] = vi.mocked(exportBadgeCmd).mock.calls[0];
    expect(reqArg).toEqual({ achievementId: "badge-1" });
    // slugified track topic: "devops-tooling"
    expect(filenameArg).toMatch(/^skillcoco-badge-devops-tooling-20260616\.png$/);
    expect(result).toEqual({ saved: true, path: "/tmp/badge.png" });
  });

  it("exportBadge_returns_null_when_user_cancels", async () => {
    const badge = makeAchievement({ id: "b2", kind: "badge" });
    vi.mocked(exportBadgeCmd).mockResolvedValue(null);

    const result = await useAchievementsStore
      .getState()
      .exportBadge(badge);

    expect(result).toEqual({ saved: false, path: null });
  });
});
