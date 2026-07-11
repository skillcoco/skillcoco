// Phase 19 plan 19-05 — Zustand slice for the exam-attempt lifecycle.
//
// Per RESEARCH/CONTEXT (mirrors 03.1-06's useLabStore precedent): this
// store is a SIBLING slice — it does NOT import or depend on useLabStore
// or useLearningStore, so the exam attempt lifecycle stays isolated from
// the regular lab session / learner / track / path state (T-19-08).
//
// The store wraps the typed IPC commands in `src/lib/tauri-commands.ts`
// (which forward to the Rust handlers registered by 19-03). All IPC
// payloads cross the Tauri boundary in camelCase per FIX-02.
//
// D-15 — submitAttempt forwards ONLY { attemptId, currentStep? }. Every
// step verdict is derived server-side from lab_progress; the client
// never supplies verdicts (T-19-10 mitigation).

import { create } from "zustand";
import {
  examAttemptStart,
  examAttemptSubmit,
  examAttemptGet,
} from "@/lib/tauri-commands";
import type {
  ExamAttemptResult,
  ExamAttemptStartResult,
} from "@/types/learning";

interface ExamState {
  /** In-progress attempt start results, keyed by blockId. */
  attempts: Map<string, ExamAttemptStartResult>;
  /** Finalized/fetched attempt results, keyed by attemptId. */
  results: Map<string, ExamAttemptResult>;

  // Actions
  startAttempt: (
    learnerId: string,
    moduleId: string,
    blockId: string,
    trackId: string,
  ) => Promise<ExamAttemptStartResult>;
  submitAttempt: (
    attemptId: string,
    currentStep?: number,
  ) => Promise<ExamAttemptResult>;
  getAttempt: (attemptId: string) => Promise<ExamAttemptResult>;
}

export const useExamStore = create<ExamState>((set, _get) => ({
  attempts: new Map(),
  results: new Map(),

  startAttempt: async (learnerId, moduleId, blockId, trackId) => {
    const result = await examAttemptStart({
      learnerId,
      moduleId,
      blockId,
      trackId,
    });
    set((s) => {
      const attempts = new Map(s.attempts);
      attempts.set(blockId, result);
      return { attempts };
    });
    return result;
  },

  submitAttempt: async (attemptId, currentStep) => {
    // D-15 — the request carries ONLY attemptId + optional currentStep.
    // currentStep is display-only telemetry; never omit conditionally in
    // a way that would let a caller sneak additional fields through.
    const request =
      currentStep === undefined ? { attemptId } : { attemptId, currentStep };
    const result = await examAttemptSubmit(request);
    set((s) => {
      const results = new Map(s.results);
      results.set(attemptId, result);
      return { results };
    });
    return result;
  },

  getAttempt: async (attemptId) => {
    const result = await examAttemptGet({ attemptId });
    set((s) => {
      const results = new Map(s.results);
      results.set(attemptId, result);
      return { results };
    });
    return result;
  },
}));

/**
 * Test helper — reset the store to its initial empty state. Mirrors the
 * `useLabStore.__resetStore()` pattern in `src/stores/useLabStore.ts`.
 */
export function __resetStore(): void {
  useExamStore.setState({
    attempts: new Map(),
    results: new Map(),
  });
}
