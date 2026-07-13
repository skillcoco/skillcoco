// Phase 16 Plan 02 Task 1 — Zustand sibling slice for Library starter-pack
// metadata. Mirrors useTopicPacksStore's shape exactly (plain create, no
// persist, isLoading/error, catch-and-set-error, __resetStore test helper).
//
// Owned/imported packs get NO new store — the Library page reads
// useLearningStore().tracks directly, since every owned pack IS a
// LearningTrack row post-import (16-PATTERNS.md).

import { create } from "zustand";
import { listStarterPacks, type StarterPackMeta } from "@/lib/tauri-commands";

interface LibraryState {
  starterPacks: StarterPackMeta[];
  isLoading: boolean;
  error: string | null;

  loadStarterPacks: () => Promise<void>;
}

const INITIAL: Omit<LibraryState, "loadStarterPacks"> = {
  starterPacks: [],
  isLoading: false,
  error: null,
};

function messageOf(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return String(err);
}

export const useLibraryStore = create<LibraryState>((set) => ({
  ...INITIAL,

  loadStarterPacks: async () => {
    set({ isLoading: true, error: null });
    try {
      const starterPacks = await listStarterPacks();
      set({ starterPacks, isLoading: false });
    } catch (err) {
      console.error("[useLibraryStore] loadStarterPacks failed:", err);
      set({ starterPacks: [], isLoading: false, error: messageOf(err) });
    }
  },
}));

/**
 * Test-only helper — resets the store to its initial state. Mirrors
 * `useTopicPacksStore.__resetStore`.
 */
export function __resetStore(): void {
  useLibraryStore.setState({ ...INITIAL });
}
