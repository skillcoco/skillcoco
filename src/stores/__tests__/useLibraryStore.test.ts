// Phase 16 Plan 02 Task 1 — useLibraryStore (starter-pack metadata only).
//
// Mirrors useTopicPacksStore's shape: plain Zustand create, no persist,
// isLoading/error, catch-and-set-error (never throw), __resetStore test
// helper. Owned packs get NO new store — Library reads useLearningStore()
// directly (PATTERNS.md).

import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@/lib/tauri-commands", () => ({
  listStarterPacks: vi.fn(),
}));

import { listStarterPacks, type StarterPackMeta } from "@/lib/tauri-commands";
import { useLibraryStore, __resetStore } from "@/stores/useLibraryStore";

function makeStarterPack(id: string): StarterPackMeta {
  return {
    id,
    title: `Starter ${id}`,
    description: `Description for ${id}`,
    moduleCount: 3,
  };
}

describe("useLibraryStore — Phase 16 Plan 02 Task 1", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetStore();
  });

  it("loadStarterPacks fetches and stores starter packs", async () => {
    const packs = [makeStarterPack("k8s"), makeStarterPack("python")];
    vi.mocked(listStarterPacks).mockResolvedValue(packs);

    await useLibraryStore.getState().loadStarterPacks();

    const next = useLibraryStore.getState();
    expect(next.starterPacks).toHaveLength(2);
    expect(next.starterPacks[0]?.id).toBe("k8s");
    expect(next.isLoading).toBe(false);
    expect(next.error).toBeNull();
  });

  it("loadStarterPacks handles IPC failure gracefully (never throws)", async () => {
    vi.mocked(listStarterPacks).mockRejectedValue(
      new Error("resource_dir unavailable"),
    );

    await expect(
      useLibraryStore.getState().loadStarterPacks(),
    ).resolves.not.toThrow();

    const next = useLibraryStore.getState();
    expect(next.starterPacks).toEqual([]);
    expect(next.isLoading).toBe(false);
    expect(next.error).toMatch(/resource_dir unavailable/);
  });

  it("__resetStore clears the store to its initial state", async () => {
    vi.mocked(listStarterPacks).mockResolvedValue([makeStarterPack("k8s")]);
    await useLibraryStore.getState().loadStarterPacks();
    expect(useLibraryStore.getState().starterPacks).toHaveLength(1);

    __resetStore();

    const next = useLibraryStore.getState();
    expect(next.starterPacks).toEqual([]);
    expect(next.isLoading).toBe(false);
    expect(next.error).toBeNull();
  });
});
