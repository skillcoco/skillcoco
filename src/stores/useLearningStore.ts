import { create } from "zustand";
import type { LearningTrack, LearningPath, ModuleProgress, SRCard } from "@/types";
import type {
  ModuleBlock,
  SubmitQuizRequest,
  SubmitQuizResult,
} from "@/types/learning";
import * as commands from "@/lib/tauri-commands";

interface LearningState {
  tracks: LearningTrack[];
  currentTrack: LearningTrack | null;
  currentPath: LearningPath | null;
  moduleProgress: ModuleProgress[];
  dueCards: SRCard[];
  isLoading: boolean;

  // Phase 3 extensions (03-05)
  currentLessonId: string | null;
  moduleBlocks: Map<string, ModuleBlock[]>;
  lessonCompletions: Map<string, Set<string>>; // moduleId -> Set<blockId>
  currentQuizResult: SubmitQuizResult | null;

  // Actions
  loadTracks: () => Promise<void>;
  selectTrack: (trackId: string) => Promise<void>;
  createTrack: (topic: string, domainModule: string, goal: string) => Promise<LearningTrack>;
  loadDueCards: () => Promise<void>;
  completeExercises: (moduleId: string, trackId: string, scores: number[]) => Promise<import("@/types/learning").CompleteExercisesResult>;

  // Phase 3 actions (03-05)
  setCurrentLesson: (blockId: string | null) => void;
  loadModuleBlocks: (moduleId: string) => Promise<ModuleBlock[]>;
  markLessonComplete: (moduleId: string, blockId: string) => Promise<void>;
  submitQuiz: (req: SubmitQuizRequest) => Promise<SubmitQuizResult>;
  regenerateLesson: (blockId: string) => Promise<void>;
}

export const useLearningStore = create<LearningState>((set, get) => ({
  tracks: [],
  currentTrack: null,
  currentPath: null,
  moduleProgress: [],
  dueCards: [],
  isLoading: false,

  // Phase 3 initial state
  currentLessonId: null,
  moduleBlocks: new Map(),
  lessonCompletions: new Map(),
  currentQuizResult: null,

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

  // Phase 3 actions (03-05)

  setCurrentLesson: (blockId) => {
    set({ currentLessonId: blockId });
  },

  loadModuleBlocks: async (moduleId) => {
    try {
      const blocks = await commands.getModuleBlocks(moduleId);
      set((s) => {
        const next = new Map(s.moduleBlocks);
        next.set(moduleId, blocks);
        return { moduleBlocks: next };
      });
      return blocks;
    } catch (err) {
      console.error("loadModuleBlocks failed:", err);
      return [];
    }
  },

  markLessonComplete: async (moduleId, blockId) => {
    // Optimistic update: add to lessonCompletions immediately
    set((s) => {
      const next = new Map(s.lessonCompletions);
      const moduleSet = new Set(next.get(moduleId) ?? []);
      moduleSet.add(blockId);
      next.set(moduleId, moduleSet);
      return { lessonCompletions: next };
    });

    try {
      await commands.markLessonComplete(moduleId, blockId);
    } catch (err) {
      // Rollback optimistic update on error
      console.error("markLessonComplete IPC failed, rolling back:", err);
      set((s) => {
        const next = new Map(s.lessonCompletions);
        const moduleSet = new Set(next.get(moduleId) ?? []);
        moduleSet.delete(blockId);
        next.set(moduleId, moduleSet);
        return { lessonCompletions: next };
      });
    }
  },

  submitQuiz: async (req) => {
    const result = await commands.submitQuiz(req);
    set({ currentQuizResult: result });
    return result;
  },

  regenerateLesson: async (blockId) => {
    // Optimistically mark the block as generating
    set((s) => {
      const next = new Map(s.moduleBlocks);
      for (const [moduleId, blocks] of next.entries()) {
        const idx = blocks.findIndex((b) => b.id === blockId);
        if (idx !== -1) {
          const updated = [...blocks];
          updated[idx] = { ...updated[idx], status: "generating" };
          next.set(moduleId, updated);
          break;
        }
      }
      return { moduleBlocks: next };
    });

    try {
      const req = { blockId };
      const newBlock = await commands.regenerateLesson(req);
      // Replace block in the map with the returned block
      set((s) => {
        const next = new Map(s.moduleBlocks);
        for (const [moduleId, blocks] of next.entries()) {
          const idx = blocks.findIndex((b) => b.id === blockId);
          if (idx !== -1) {
            const updated = [...blocks];
            updated[idx] = newBlock;
            next.set(moduleId, updated);
            break;
          }
        }
        return { moduleBlocks: next };
      });
    } catch (err) {
      console.error("regenerateLesson failed:", err);
      // Restore block status to failed on error
      set((s) => {
        const next = new Map(s.moduleBlocks);
        for (const [moduleId, blocks] of next.entries()) {
          const idx = blocks.findIndex((b) => b.id === blockId);
          if (idx !== -1) {
            const updated = [...blocks];
            updated[idx] = { ...updated[idx], status: "failed" };
            next.set(moduleId, updated);
            break;
          }
        }
        return { moduleBlocks: next };
      });
    }
  },
}));
