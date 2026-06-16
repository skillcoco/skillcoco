// Phase 6 (Certification) — Plan 06-01 (Wave 0) GREEN store tests.
//
// Sibling-slice pattern (NOT extension of useLearningStore) per Phase 4
// Pitfall 5 + Phase 03.1 useLabStore precedent. The store action is real
// in Wave 0, so this test PASSES — it locks the contract for downstream
// waves. The deliberate Wave 0 RED spec lives in
// src/components/achievements/__tests__/AchievementSection.test.tsx.

import { describe, it, expect, vi, beforeEach } from "vitest";

// Vitest hoisting rule: inline literals only inside the factory body.
vi.mock("@/lib/tauri-commands", () => ({
  listAchievements: vi.fn(),
}));

import {
  useAchievementsStore,
  __resetStore,
} from "@/stores/useAchievementsStore";
import { listAchievements } from "@/lib/tauri-commands";

describe("useAchievementsStore — Phase 6 Plan 06-01 (Wave 0 GREEN)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetStore();
  });

  it("initial state has empty achievements array", () => {
    const s = useAchievementsStore.getState();
    expect(s.achievements).toEqual([]);
    expect(s.isLoading).toBe(false);
    expect(s.error).toBeNull();
  });

  it("loadAchievements invokes list_achievements_for_learner and stores the result", async () => {
    vi.mocked(listAchievements).mockResolvedValue([]);

    const store = useAchievementsStore.getState();
    expect(typeof store.loadAchievements).toBe("function");
    await store.loadAchievements();

    expect(listAchievements).toHaveBeenCalledTimes(1);
    const next = useAchievementsStore.getState();
    expect(next.achievements).toEqual([]);
    expect(next.isLoading).toBe(false);
    expect(next.error).toBeNull();
  });

  it("appendNewlyIssued prepends to achievements (preserves order: newest first)", () => {
    // Seed with one existing achievement, then append two new.
    const existing = {
      id: "a-existing",
      learnerId: "lnr-1",
      trackId: "trk-1",
      packId: null,
      kind: "badge" as const,
      level: "Associate" as const,
      issuedAt: "2026-06-01T00:00:00Z",
      masteryScore: 0.75,
      payloadJson: "",
      signature: "",
      keyFingerprint: "deadbeef",
      trackTopic: "Kubernetes",
    };
    useAchievementsStore.setState({ achievements: [existing] });

    const newOne = { ...existing, id: "a-new", level: "Practitioner" as const };
    useAchievementsStore.getState().appendNewlyIssued([newOne]);

    const next = useAchievementsStore.getState();
    expect(next.achievements).toHaveLength(2);
    expect(next.achievements[0].id).toBe("a-new"); // newest first
    expect(next.achievements[1].id).toBe("a-existing");
  });
});
