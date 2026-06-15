// Phase 4 Plan 04 (Wave 3) — GREEN store tests.
//
// Sibling-slice pattern (NOT extension of useLearningStore) per Q2 lock +
// Phase 03.1 useLabStore precedent. Plan 03 made both `startDailyChallenge`
// and `completeDailyChallenge` IPCs parameterless — server resolves
// learner_id + challenge_date — so the store actions also take no args.

import { describe, it, expect, vi, beforeEach } from "vitest";

// Vitest hoisting rule: inline literals only inside the factory body.
vi.mock("@/lib/tauri-commands", () => ({
  getDailyChallenge: vi.fn(),
  startDailyChallenge: vi.fn(),
  completeDailyChallenge: vi.fn(),
  isDailyChallengeEnabled: vi.fn(),
}));

import { useDailyChallengeStore, __resetStore } from "@/stores/useDailyChallengeStore";
import {
  getDailyChallenge,
  startDailyChallenge,
  completeDailyChallenge,
  isDailyChallengeEnabled,
} from "@/lib/tauri-commands";

describe("useDailyChallengeStore — Phase 4 Plan 04 (GREEN)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetStore();
  });

  it("loadDailyChallenge — when disabled, sets isEnabled=false and todaysChallenge=null", async () => {
    vi.mocked(isDailyChallengeEnabled).mockResolvedValue({
      enabled: false,
      globalStreakDays: 0,
    });

    const store = useDailyChallengeStore.getState();
    expect(typeof store.loadDailyChallenge).toBe("function");
    await store.loadDailyChallenge();

    const next = useDailyChallengeStore.getState();
    expect(next.isEnabled).toBe(false);
    expect(next.todaysChallenge).toBeNull();
    // When disabled, the daily-challenge IPC must NOT be called (Pitfall 6 —
    // one-IPC mount: gate first, then payload only if enabled).
    expect(getDailyChallenge).not.toHaveBeenCalled();
  });

  it("loadDailyChallenge — when enabled, populates todaysChallenge + globalStreakDays in one mount", async () => {
    vi.mocked(isDailyChallengeEnabled).mockResolvedValue({
      enabled: true,
      globalStreakDays: 5,
    });
    vi.mocked(getDailyChallenge).mockResolvedValue({
      challenge: {
        blockId: "blk-1",
        blockType: "quiz",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 5,
        status: "pending",
      },
    });

    const store = useDailyChallengeStore.getState();
    await store.loadDailyChallenge();

    const next = useDailyChallengeStore.getState();
    expect(next.isEnabled).toBe(true);
    expect(next.globalStreakDays).toBe(5);
    expect(next.todaysChallenge?.blockId).toBe("blk-1");
    expect(next.todaysChallenge?.status).toBe("pending");
  });

  it("loadDailyChallenge — surfaces empty zone as todaysChallenge=null with isEnabled=true (Q3)", async () => {
    // The auto-enable gate fires, but the BKT [0.3, 0.7] zone has no candidates.
    // Card's empty-zone variant consumes this exact shape.
    vi.mocked(isDailyChallengeEnabled).mockResolvedValue({
      enabled: true,
      globalStreakDays: 2,
    });
    vi.mocked(getDailyChallenge).mockResolvedValue({ challenge: null });

    const store = useDailyChallengeStore.getState();
    await store.loadDailyChallenge();

    const next = useDailyChallengeStore.getState();
    expect(next.isEnabled).toBe(true);
    expect(next.todaysChallenge).toBeNull();
    expect(next.globalStreakDays).toBe(2);
  });

  it("completeDailyChallenge — optimistically updates status, syncs streak from IPC result", async () => {
    useDailyChallengeStore.setState({
      isEnabled: true,
      globalStreakDays: 3,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "in_progress",
      },
    });

    vi.mocked(completeDailyChallenge).mockResolvedValue({
      newStreakDays: 4,
      completedAt: "2026-06-15T18:00:00Z",
    });

    const store = useDailyChallengeStore.getState();
    await store.completeDailyChallenge();

    expect(completeDailyChallenge).toHaveBeenCalledTimes(1);
    const next = useDailyChallengeStore.getState();
    expect(next.todaysChallenge?.status).toBe("done");
    expect(next.globalStreakDays).toBe(4);
  });

  it("completeDailyChallenge — server result overrides optimistic streak (e.g., server returns 3 when prior was 0)", async () => {
    // Scenario: client had no prior streak loaded (e.g., a stale tab) but the
    // server had a 2-day streak that the IPC now completes into 3.
    useDailyChallengeStore.setState({
      isEnabled: true,
      globalStreakDays: 0,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "quiz",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 5,
        status: "in_progress",
      },
    });

    vi.mocked(completeDailyChallenge).mockResolvedValue({
      newStreakDays: 3,
      completedAt: "2026-06-15T18:00:00Z",
    });

    const store = useDailyChallengeStore.getState();
    await store.completeDailyChallenge();

    const next = useDailyChallengeStore.getState();
    expect(next.globalStreakDays).toBe(3);
    expect(next.todaysChallenge?.status).toBe("done");
  });

  it("completeDailyChallenge — rolls back on IPC error (Pattern 3 — useLearningStore.markLessonComplete:169-192)", async () => {
    useDailyChallengeStore.setState({
      isEnabled: true,
      globalStreakDays: 3,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "flash_cards",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 5,
        status: "in_progress",
      },
    });

    vi.mocked(completeDailyChallenge).mockRejectedValue(new Error("IPC boom"));

    const store = useDailyChallengeStore.getState();
    await store.completeDailyChallenge();

    const next = useDailyChallengeStore.getState();
    // On error: status reverts to in_progress, streak unchanged.
    expect(next.todaysChallenge?.status).toBe("in_progress");
    expect(next.globalStreakDays).toBe(3);
  });

  it("startDailyChallenge — invokes IPC and flips status pending → in_progress", async () => {
    useDailyChallengeStore.setState({
      isEnabled: true,
      globalStreakDays: 0,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "pending",
      },
    });

    vi.mocked(startDailyChallenge).mockResolvedValue(undefined);

    const store = useDailyChallengeStore.getState();
    await store.startDailyChallenge();

    expect(startDailyChallenge).toHaveBeenCalledTimes(1);
    expect(useDailyChallengeStore.getState().todaysChallenge?.status).toBe("in_progress");
  });
});
