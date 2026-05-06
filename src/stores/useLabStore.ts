// Phase 03.1 plan 03.1-06 — real Zustand slice for lab session lifecycle.
//
// Per 03.1-RESEARCH.md risk row + 03.1-CONTEXT.md "Frontend module"
// integration point: this store is a SIBLING slice to useLearningStore
// (NOT an extension) so the lab session lifecycle stays isolated from
// learner / track / path state.
//
// The store wraps the typed IPC commands in `src/lib/tauri-commands.ts`
// (which forward to the Rust handlers in `src-tauri/src/commands/labs/`).
// All IPC payloads cross the Tauri boundary in camelCase per FIX-02.

import { create } from "zustand";
import {
  labSessionOpen,
  labSessionClose,
  labCheckStep,
  labGetProgress,
} from "@/lib/tauri-commands";
import type { LabProgress, LabSession } from "@/types/learning";

/** Step-evaluation outcome surfaced to UI. Mirrors Rust CheckOutcome. */
export type CheckOutcome = "pass" | "fail" | "indeterminate" | "manual";

interface LabState {
  /** Active lab session keyed by blockId — populated by openSession. */
  sessions: Map<string, LabSession>;
  /** Per-block lab progress — populated by getProgress / markStepComplete. */
  progress: Map<string, LabProgress>;

  // Actions
  openSession: (
    blockId: string,
    trackId: string,
    moduleId: string,
    learnerId: string,
  ) => Promise<LabSession>;
  closeSession: (sessionId: string) => Promise<void>;
  markStepComplete: (
    sessionId: string,
    stepIndex: number,
    lastCommand: string,
    lastOutput: string,
    lastExitCode: number | null,
  ) => Promise<{ outcome: CheckOutcome }>;
  getProgress: (blockId: string, learnerId: string) => Promise<LabProgress>;
}

function reasonToOutcome(reason: string, passed: boolean): CheckOutcome {
  if (passed) return "pass";
  // The Rust evaluator emits a small set of reason strings — match the
  // ones that are NOT real failures so we can route them to UI states.
  const lc = reason.toLowerCase();
  if (lc.includes("indeterminate") || lc.includes("budget exhausted")) {
    return "indeterminate";
  }
  if (lc.includes("manual")) return "manual";
  return "fail";
}

export const useLabStore = create<LabState>((set, _get) => ({
  sessions: new Map(),
  progress: new Map(),

  openSession: async (blockId, trackId, moduleId, learnerId) => {
    const result = await labSessionOpen({
      blockId,
      trackId,
      moduleId,
      learnerId,
    });
    const session: LabSession = {
      sessionId: result.sessionId,
      effectiveRuntime: result.effectiveRuntime,
      // Plan 03.1-09 GAP-05 — stash learnerId so markStepComplete can
      // call getProgress without re-threading the learner through the
      // call site (LabBlock doesn't currently have it once mount unwinds).
      learnerId,
      ...(result.warning ? { warning: result.warning } : {}),
    };
    set((s) => {
      const sessions = new Map(s.sessions);
      sessions.set(blockId, session);
      const progress = new Map(s.progress);
      progress.set(blockId, result.progress);
      return { sessions, progress };
    });
    return session;
  },

  closeSession: async (sessionId) => {
    await labSessionClose({ sessionId });
    set((s) => {
      const sessions = new Map(s.sessions);
      // sessions are keyed by blockId; find and remove the matching entry.
      for (const [blockId, sess] of sessions.entries()) {
        if (sess.sessionId === sessionId) {
          sessions.delete(blockId);
          break;
        }
      }
      return { sessions };
    });
  },

  markStepComplete: async (
    sessionId,
    stepIndex,
    lastCommand,
    lastOutput,
    lastExitCode,
  ) => {
    const result = await labCheckStep({
      sessionId,
      stepIndex,
      lastCommand,
      lastOutput,
      lastExitCode,
    });
    const outcome = reasonToOutcome(result.reason, result.passed);

    // Plan 03.1-09 GAP-05 — refresh the canonical lab_progress row from
    // Rust on a successful Pass so the UI reflects the new
    // currentStep + completedStepIds without an extra component round-trip.
    if (result.passed) {
      const state = useLabStore.getState();
      let blockId: string | null = null;
      let learnerId: string | null = null;
      for (const [bId, sess] of state.sessions.entries()) {
        if (sess.sessionId === sessionId) {
          blockId = bId;
          learnerId = sess.learnerId ?? null;
          break;
        }
      }
      if (blockId && learnerId) {
        try {
          await state.getProgress(blockId, learnerId);
        } catch {
          // Non-fatal: UI keeps the last-known progress until the next
          // refresh; the Pass already crossed the IPC boundary.
        }
      }
    }

    return { outcome };
  },

  getProgress: async (blockId, learnerId) => {
    const result = await labGetProgress({ blockId, learnerId });
    set((s) => {
      const progress = new Map(s.progress);
      progress.set(blockId, result);
      return { progress };
    });
    return result;
  },
}));

/**
 * Test helper — reset the store to its initial empty state. Mirrors the
 * `useLearningStore.setState({...})` pattern used in
 * `src/stores/__tests__/useLearningStore.test.ts`.
 */
export function __resetStore(): void {
  useLabStore.setState({
    sessions: new Map(),
    progress: new Map(),
  });
}
