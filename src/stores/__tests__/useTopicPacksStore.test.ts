// Phase 5 Plan 04 (Wave 3) — useTopicPacksStore tests (GREEN after Task 1).
//
// Sibling slice (NOT extension of useLearningStore) per STATE.md Phase 03.1-01b
// + Phase 04 useDailyChallengeStore precedent. The store wraps the 5 IPC
// wrappers from `src/lib/tauri-commands.ts` (`listTopicPacksAdmin`,
// `setTopicPackEnabled`, `reloadSkills`). `loadPacks` uses ADMIN list so the
// Settings UI can see disabled packs and error sentinels (per Wave 2 contract).

import { describe, it, expect, vi, beforeEach } from "vitest";

// Vitest hoisting rule: inline literals only inside the factory body. We mock
// every function the store imports — others can stay as `vi.fn()` stubs.
vi.mock("@/lib/tauri-commands", () => ({
  listTopicPacks: vi.fn(),
  listTopicPacksAdmin: vi.fn(),
  setTopicPackEnabled: vi.fn(),
  reloadSkills: vi.fn(),
  getTopicPackModules: vi.fn(),
}));

import {
  listTopicPacksAdmin,
  setTopicPackEnabled,
  reloadSkills,
} from "@/lib/tauri-commands";
import {
  useTopicPacksStore,
  __resetStore,
} from "@/stores/useTopicPacksStore";
import type { TopicPack } from "@/types/topic-packs";

function makePack(id: string, enabled = true): TopicPack {
  return {
    pack: {
      id,
      title: `Pack ${id}`,
      description: `Description for ${id}`,
      domain_module: "devops",
      pack_version: "1.0",
      requires_docker: false,
      modules: [],
      edges: [],
    },
    source: "bundled",
    enabled,
    validationStatus: "ok",
    validationMessages: [],
    lastLoadedAt: "2026-06-15T12:00:00Z",
  };
}

describe("useTopicPacksStore — Phase 5 Plan 04 (GREEN)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetStore();
  });

  it("loadPacks fetches and stores packs", async () => {
    const packs = [makePack("k8s"), makePack("rust")];
    vi.mocked(listTopicPacksAdmin).mockResolvedValue(packs);

    const store = useTopicPacksStore.getState();
    expect(typeof store.loadPacks).toBe("function");

    await store.loadPacks();

    const next = useTopicPacksStore.getState();
    expect(next.packs).toHaveLength(2);
    expect(next.packs[0]?.pack.id).toBe("k8s");
    expect(next.isLoading).toBe(false);
    expect(next.error).toBeNull();
  });

  it("setEnabled calls IPC and refreshes", async () => {
    vi.mocked(setTopicPackEnabled).mockResolvedValue(undefined);
    // After the toggle, loadPacks() runs and pulls the updated list.
    vi.mocked(listTopicPacksAdmin).mockResolvedValue([
      makePack("k8s", false),
    ]);

    await useTopicPacksStore.getState().setEnabled("k8s", false);

    expect(setTopicPackEnabled).toHaveBeenCalledWith({
      packId: "k8s",
      enabled: false,
    });
    // listTopicPacksAdmin must run AFTER the toggle (refresh contract).
    expect(listTopicPacksAdmin).toHaveBeenCalled();
    const setEnabledOrder = vi.mocked(setTopicPackEnabled).mock
      .invocationCallOrder[0];
    const listOrder = vi.mocked(listTopicPacksAdmin).mock
      .invocationCallOrder[0];
    expect(setEnabledOrder).toBeLessThan(listOrder!);

    const next = useTopicPacksStore.getState();
    expect(next.packs[0]?.enabled).toBe(false);
    expect(next.error).toBeNull();
  });

  it("setEnabled sets error on failure", async () => {
    vi.mocked(setTopicPackEnabled).mockRejectedValue(
      new Error("Unknown pack id: ghost"),
    );

    await useTopicPacksStore.getState().setEnabled("ghost", true);

    const next = useTopicPacksStore.getState();
    expect(next.error).toMatch(/Unknown pack id: ghost/);
    // refresh must NOT run after a failed toggle.
    expect(listTopicPacksAdmin).not.toHaveBeenCalled();
  });

  it("reloadSkills sets reloading flag while running", async () => {
    let resolveReload: (() => void) | undefined;
    const reloadPromise = new Promise<void>((r) => {
      resolveReload = r;
    });
    vi.mocked(reloadSkills).mockReturnValue(reloadPromise);
    vi.mocked(listTopicPacksAdmin).mockResolvedValue([]);

    const callPromise = useTopicPacksStore.getState().reloadSkills();

    // Immediately after fire-and-forget kick, reloading must be true.
    expect(useTopicPacksStore.getState().reloading).toBe(true);

    resolveReload!();
    await callPromise;

    expect(useTopicPacksStore.getState().reloading).toBe(false);
    // loadPacks should have run as part of reload's chain.
    expect(listTopicPacksAdmin).toHaveBeenCalled();
  });

  it("loadPacks handles IPC failure gracefully", async () => {
    vi.mocked(listTopicPacksAdmin).mockRejectedValue(
      new Error("topic_packs lock poisoned"),
    );

    await useTopicPacksStore.getState().loadPacks();

    const next = useTopicPacksStore.getState();
    expect(next.packs).toEqual([]);
    expect(next.isLoading).toBe(false);
    expect(next.error).toMatch(/topic_packs lock poisoned/);
  });
});
