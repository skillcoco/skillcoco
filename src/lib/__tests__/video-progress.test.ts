/**
 * TDD RED — video-progress store
 *
 * In-memory + localStorage-backed progress store for YouTube playback continuity.
 * Tests drive the implementation of src/lib/video-progress.ts.
 */
import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";

// ── module import (will fail at RED since the module does not exist yet) ──────
import { getVideoProgress, setVideoProgress } from "@/lib/video-progress";

describe("video-progress store", () => {
  // Each test gets an isolated store. The module uses a Map singleton,
  // so we need to clear it between tests. We do this by spying on the
  // private Map via the re-import trick — instead, we rely on the
  // contract: set then get must return the set value; after clearing
  // localStorage a second getVideoProgress must still return the
  // in-memory value (until it is evicted by a different test run).
  //
  // For isolation, we use unique videoIds per test and reset localStorage.

  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    localStorage.clear();
  });

  // ── Defaults ────────────────────────────────────────────────────────────────

  it("vp_get_default_zero — returns 0 when no progress stored", () => {
    expect(getVideoProgress("unknown-vid")).toBe(0);
  });

  // ── Round-trip ───────────────────────────────────────────────────────────────

  it("vp_set_then_get — set then get returns the stored value", () => {
    setVideoProgress("vid-abc", 42);
    expect(getVideoProgress("vid-abc")).toBe(42);
  });

  it("vp_set_overwrites — second set overwrites first", () => {
    setVideoProgress("vid-overwrite", 10);
    setVideoProgress("vid-overwrite", 77);
    expect(getVideoProgress("vid-overwrite")).toBe(77);
  });

  it("vp_independent_keys — different videoIds are stored independently", () => {
    setVideoProgress("vid-x", 30);
    setVideoProgress("vid-y", 60);
    expect(getVideoProgress("vid-x")).toBe(30);
    expect(getVideoProgress("vid-y")).toBe(60);
  });

  // ── Guard: ignore invalid inputs ─────────────────────────────────────────────

  it("vp_negative_ignored — negative seconds are NOT stored", () => {
    setVideoProgress("vid-neg", -5);
    expect(getVideoProgress("vid-neg")).toBe(0);
  });

  it("vp_nan_ignored — NaN is NOT stored", () => {
    setVideoProgress("vid-nan", NaN);
    expect(getVideoProgress("vid-nan")).toBe(0);
  });

  it("vp_infinity_ignored — Infinity is NOT stored", () => {
    setVideoProgress("vid-inf", Infinity);
    expect(getVideoProgress("vid-inf")).toBe(0);
  });

  it("vp_zero_stored — 0 is a valid value (beginning of video)", () => {
    setVideoProgress("vid-zero-after-set", 30);
    setVideoProgress("vid-zero-after-set", 0);
    expect(getVideoProgress("vid-zero-after-set")).toBe(0);
  });

  // ── localStorage persistence ─────────────────────────────────────────────────

  it("vp_persists_to_localstorage — set writes to localStorage under lf.videoProgress.<videoId>", () => {
    setVideoProgress("vid-persist", 123);
    const raw = localStorage.getItem("lf.videoProgress.vid-persist");
    expect(raw).not.toBeNull();
    expect(Number(raw)).toBe(123);
  });

  it("vp_hydrate_from_localstorage — getVideoProgress hydrates from localStorage when in-memory map is cold", () => {
    // Write directly to localStorage (simulating a prior session)
    localStorage.setItem("lf.videoProgress.vid-hydrate", "99");

    // Because the in-memory Map is a module-level singleton that persists
    // across tests within the same test run, we need a key that hasn't been
    // touched yet in this test suite. We use a unique key.
    const freshKey = `vid-hydrate-${Date.now()}`;
    localStorage.setItem(`lf.videoProgress.${freshKey}`, "88");

    // A get for this fresh key should find the localStorage value
    expect(getVideoProgress(freshKey)).toBe(88);
  });

  it("vp_localstorage_fail_soft — if localStorage throws, getVideoProgress returns 0 without throwing", () => {
    const getItemSpy = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("quota exceeded");
    });

    expect(() => getVideoProgress("vid-ls-error")).not.toThrow();
    expect(getVideoProgress("vid-ls-error")).toBe(0);

    getItemSpy.mockRestore();
  });

  it("vp_localstorage_set_fail_soft — if localStorage.setItem throws, setVideoProgress does not throw", () => {
    const setItemSpy = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("quota exceeded");
    });

    expect(() => setVideoProgress("vid-ls-set-error", 50)).not.toThrow();
    // The in-memory map should still be updated
    expect(getVideoProgress("vid-ls-set-error")).toBe(50);

    setItemSpy.mockRestore();
  });
});
