// Phase 4 Wave 0 — RED scaffold for useDailyChallengeStore.
//
// This file imports `@/stores/useDailyChallengeStore` which does NOT exist
// yet. Vitest fails with "Cannot find module" — that IS the RED state and
// the contract Plan 04 satisfies (Plan 04 lands the store).
//
// Sibling-slice pattern (NOT extension of useLearningStore) per Q2 lock +
// Phase 03.1 useLabStore precedent.

import { describe, it, expect, vi, beforeEach } from "vitest";

// Vitest hoisting rule: inline literals only inside the factory body.
vi.mock("@/lib/tauri-commands", () => ({
  getDailyChallenge: vi.fn(),
  startDailyChallenge: vi.fn(),
  completeDailyChallenge: vi.fn(),
  isDailyChallengeEnabled: vi.fn(),
}));

// Wave 0 typed shell lives at `@/stores/useDailyChallengeStore`. Plan 04
// replaces its stub bodies with the real IPC wiring. Each action currently
// throws "Plan 04 implements ..." so the *assertion-level* RED state is
// preserved (vitest fails on expect(...).toBe(...) — not on imports).
import { useDailyChallengeStore, __resetStore } from "@/stores/useDailyChallengeStore";
import {
  getDailyChallenge,
  startDailyChallenge,
  completeDailyChallenge,
  isDailyChallengeEnabled,
} from "@/lib/tauri-commands";

describe("useDailyChallengeStore — Phase 4 Wave 0 (failing scaffolds)", () => {
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

  it("completeDailyChallenge — optimistically updates completedAt, increments globalStreakDays from IPC result", async () => {
    // Seed the store with an in-progress challenge + streak=3.
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
    await store.completeDailyChallenge("2026-06-15");

    expect(completeDailyChallenge).toHaveBeenCalledWith("2026-06-15");

    const next = useDailyChallengeStore.getState();
    expect(next.todaysChallenge?.status).toBe("done");
    expect(next.globalStreakDays).toBe(4);
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
    await store.completeDailyChallenge("2026-06-15");

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
    await store.startDailyChallenge("2026-06-15");

    expect(startDailyChallenge).toHaveBeenCalledWith("2026-06-15");
    expect(useDailyChallengeStore.getState().todaysChallenge?.status).toBe("in_progress");
  });
});
