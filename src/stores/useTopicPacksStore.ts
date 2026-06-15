// Phase 5 Plan 04 (Wave 3) — Zustand sibling slice for Topic Packs Settings UI.
// Sibling slice (NOT extension of useLearningStore) per STATE.md Phase 03.1-01b
// + Phase 04 useDailyChallengeStore precedent. Uses `listTopicPacksAdmin` so
// the Settings UI sees ALL packs (enabled + disabled + error sentinels);
// Onboarding (Wave 4) uses the non-admin variant. Catch blocks set `error` and
// never throw upward, mirroring useLabStore / useDailyChallengeStore.

import { create } from "zustand";
import {
  listTopicPacksAdmin,
  setTopicPackEnabled,
  reloadSkills,
} from "@/lib/tauri-commands";
import type { TopicPack } from "@/types/topic-packs";

interface TopicPacksState {
  packs: TopicPack[];
  isLoading: boolean;
  reloading: boolean;
  error: string | null;

  // Actions
  loadPacks: () => Promise<void>;
  setEnabled: (packId: string, enabled: boolean) => Promise<void>;
  reloadSkills: () => Promise<void>;
}

const INITIAL: Omit<
  TopicPacksState,
  "loadPacks" | "setEnabled" | "reloadSkills"
> = {
  packs: [],
  isLoading: false,
  reloading: false,
  error: null,
};

function messageOf(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return String(err);
}

export const useTopicPacksStore = create<TopicPacksState>((set) => ({
  ...INITIAL,

  loadPacks: async () => {
    set({ isLoading: true, error: null });
    try {
      const packs = await listTopicPacksAdmin();
      set({ packs, isLoading: false });
    } catch (err) {
      console.error("[useTopicPacksStore] loadPacks failed:", err);
      set({ packs: [], isLoading: false, error: messageOf(err) });
    }
  },

  setEnabled: async (packId, enabled) => {
    set({ error: null });
    try {
      await setTopicPackEnabled({ packId, enabled });
    } catch (err) {
      console.error("[useTopicPacksStore] setEnabled failed:", err);
      set({ error: messageOf(err) });
      return; // do NOT refresh after a failed toggle
    }
    // Refresh to pick up the registry's authoritative view.
    await useTopicPacksStore.getState().loadPacks();
  },

  reloadSkills: async () => {
    set({ reloading: true, error: null });
    try {
      await reloadSkills();
      await useTopicPacksStore.getState().loadPacks();
    } catch (err) {
      console.error("[useTopicPacksStore] reloadSkills failed:", err);
      set({ error: messageOf(err) });
    } finally {
      set({ reloading: false });
    }
  },
}));

/**
 * Test-only helper — resets the store to its initial state. Mirrors
 * `useLabStore.__resetStore` / `useDailyChallengeStore.__resetStore`.
 */
export function __resetStore(): void {
  useTopicPacksStore.setState({ ...INITIAL });
}
