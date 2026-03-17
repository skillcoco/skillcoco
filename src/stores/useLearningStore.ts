import { create } from "zustand";
import type { LearningTrack, LearningPath, ModuleProgress, SRCard } from "@/types";
import * as commands from "@/lib/tauri-commands";

interface LearningState {
  tracks: LearningTrack[];
  currentTrack: LearningTrack | null;
  currentPath: LearningPath | null;
  moduleProgress: ModuleProgress[];
  dueCards: SRCard[];
  isLoading: boolean;

  // Actions
  loadTracks: () => Promise<void>;
  selectTrack: (trackId: string) => Promise<void>;
  createTrack: (topic: string, domainModule: string, goal: string) => Promise<LearningTrack>;
  loadDueCards: () => Promise<void>;
  completeExercises: (moduleId: string, trackId: string, scores: number[]) => Promise<import("@/types/learning").CompleteExercisesResult>;
}

export const useLearningStore = create<LearningState>((set, get) => ({
  tracks: [],
  currentTrack: null,
  currentPath: null,
  moduleProgress: [],
  dueCards: [],
  isLoading: false,

  loadTracks: async () => {
    set({ isLoading: true });
    try {
      const tracks = await commands.listTracks();
      set({ tracks, isLoading: false });
    } catch (err) {
      console.error("Failed to load tracks:", err);
      set({ isLoading: false });
    }
  },

  selectTrack: async (trackId: string) => {
    set({ isLoading: true });
    try {
      const [track, path, progress] = await Promise.all([
        commands.getTrack(trackId),
        commands.getPath(trackId),
        commands.getModuleProgress(trackId),
      ]);
      set({ currentTrack: track, currentPath: path, moduleProgress: progress, isLoading: false });
    } catch (err) {
      console.error("Failed to load track:", err);
      set({ isLoading: false });
    }
  },

  createTrack: async (topic, domainModule, goal) => {
    const track = await commands.createTrack(topic, domainModule, goal);
    set((s) => ({ tracks: [...s.tracks, track] }));
    return track;
  },

  loadDueCards: async () => {
    try {
      const dueCards = await commands.getDueCards();
      set({ dueCards });
    } catch (err) {
      console.error("Failed to load due cards:", err);
    }
  },

  completeExercises: async (moduleId, trackId, scores) => {
    const result = await commands.completeModuleExercises(moduleId, trackId, scores);
    // Refresh module progress to reflect mastery + unlocks
    const progress = await commands.getModuleProgress(trackId);
    set({ moduleProgress: progress });
    return result;
  },
}));
