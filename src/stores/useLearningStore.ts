import { create } from "zustand";
import type { LearningTrack, LearningPath, ModuleProgress, SRCard } from "@/types";
import type {
  ModuleBlock,
  SubmitQuizRequest,
  SubmitQuizResult,
} from "@/types/learning";
import * as commands from "@/lib/tauri-commands";
import { useAchievementsStore } from "@/stores/useAchievementsStore";

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
  deleteTrack: (trackId: string) => Promise<void>;
  loadDueCards: () => Promise<void>;
  completeExercises: (moduleId: string, trackId: string, scores: number[]) => Promise<import("@/types/learning").CompleteExercisesResult>;

  // Phase 3 actions (03-05)
  setCurrentLesson: (blockId: string | null) => void;
  loadModuleBlocks: (moduleId: string) => Promise<ModuleBlock[]>;
  loadLessonCompletions: (moduleId: string) => Promise<void>;
  markLessonComplete: (moduleId: string, blockId: string) => Promise<void>;
  submitQuiz: (req: SubmitQuizRequest) => Promise<SubmitQuizResult>;
  regenerateLesson: (blockId: string) => Promise<void>;

  // Phase 10 Plan 03 — browse mode action (D-01/D-02)
  setTrackBrowseMode: (trackId: string, mode: "linear" | "free") => Promise<void>;
}

/**
 * Phase 03.1 LAB-08 — practical mastery selector.
 *
 * Reads `practicalMastery` from the per-module `module_progress` row loaded
 * by `selectTrack` / `submitQuiz` / `completeExercises` via the existing
 * `get_module_progress` IPC. The Rust ModuleProgress struct gained the
 * `practical_mastery` field in this plan, so the IPC payload now carries
 * `practicalMastery` automatically (camelCase per `#[serde(rename_all)]`).
 *
 * Returns 0 when no row is loaded for that module — consistent with the
 * v006 migration default. Use as a Zustand selector:
 *
 *   const mastery = useLearningStore(selectModulePracticalMastery(moduleId));
 */
export const selectModulePracticalMastery =
  (moduleId: string) =>
  (s: LearningState): number => {
    const mp = s.moduleProgress.find((p) => p.moduleId === moduleId);
    return mp?.practicalMastery ?? 0;
  };

export const useLearningStore = create<LearningState>((set, _get) => ({
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

  deleteTrack: async (trackId) => {
    await commands.deleteTrack(trackId);
    set((s) => ({
      tracks: s.tracks.filter((t) => t.id !== trackId),
      currentTrack: s.currentTrack?.id === trackId ? null : s.currentTrack,
      currentPath: s.currentTrack?.id === trackId ? null : s.currentPath,
      moduleProgress: s.currentTrack?.id === trackId ? [] : s.moduleProgress,
    }));
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

  loadLessonCompletions: async (moduleId) => {
    try {
      const blockIds = await commands.getLessonCompletions(moduleId);
      set((s) => {
        const next = new Map(s.lessonCompletions);
        next.set(moduleId, new Set(blockIds));
        return { lessonCompletions: next };
      });
    } catch (err) {
      console.error("loadLessonCompletions failed:", err);
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

    // Phase 6 Wave 3 (06-04): forward newlyIssuedAchievements to the
    // sibling achievements slice. Sibling-slice — DO NOT merge fields into
    // useLearningStore state (Phase 4 Pitfall 5). The result type carries
    // newlyIssuedAchievements as a flat array (A4 lock); empty array is a
    // no-op inside appendNewlyIssued.
    if (
      result.newlyIssuedAchievements &&
      result.newlyIssuedAchievements.length > 0
    ) {
      useAchievementsStore
        .getState()
        .appendNewlyIssued(result.newlyIssuedAchievements);
    }

    // Refresh per-module progress so the sidebar/QuizBlock reflect the new
    // mastery_level and any unlocks immediately, without a manual reload.
    try {
      const progress = await commands.getModuleProgress(req.trackId);
      set({ currentQuizResult: result, moduleProgress: progress });
    } catch (err) {
      console.error("submitQuiz: failed to refresh module progress:", err);
      set({ currentQuizResult: result });
    }
    return result;
  },

  // Phase 10 Plan 03 — setTrackBrowseMode with optimistic + rollback
  // (mirrors markLessonComplete shape). Free mode is a presentation-only
  // change: it does NOT relax cert/mastery gates (T-10-06 invariant).
  setTrackBrowseMode: async (trackId, mode) => {
    // Capture prior mode for rollback
    const prevCurrentTrack = useLearningStore.getState().currentTrack;
    const prevMode = prevCurrentTrack?.id === trackId ? prevCurrentTrack.browseMode : undefined;

    // Optimistic patch: currentTrack + matching tracks[] entry
    set((s) => ({
      currentTrack:
        s.currentTrack?.id === trackId
          ? { ...s.currentTrack, browseMode: mode }
          : s.currentTrack,
      tracks: s.tracks.map((t) =>
        t.id === trackId ? { ...t, browseMode: mode } : t,
      ),
    }));

    try {
      await commands.setTrackBrowseMode(trackId, mode);
    } catch (err) {
      // Rollback on IPC error — restore previous browseMode
      console.error("setTrackBrowseMode IPC failed, rolling back:", err);
      set((s) => ({
        currentTrack:
          s.currentTrack?.id === trackId
            ? { ...s.currentTrack, browseMode: prevMode }
            : s.currentTrack,
        tracks: s.tracks.map((t) =>
          t.id === trackId ? { ...t, browseMode: prevMode } : t,
        ),
      }));
    }
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
