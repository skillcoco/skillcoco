// Wave 0 stub — Phase 03.1 plan 03.1-06 wires every action below to the
// matching Tauri IPC command (`lab_session_open`, `lab_session_close`,
// `lab_check_step`, `lab_get_progress`).
//
// Per 03.1-RESEARCH.md risk row + 03.1-CONTEXT.md "Frontend module"
// integration point: this store is a SIBLING slice to useLearningStore
// (NOT an extension) so the lab session lifecycle stays isolated from
// learner / track / path state.

import { create } from "zustand";
import type { LabProgress, LabSession } from "@/types/learning";

interface LabState {
  /** Active lab session keyed by blockId — populated by openSession. */
  sessions: Map<string, LabSession>;
  /** Per-block lab progress — populated by getProgress / markStepComplete. */
  progress: Map<string, LabProgress>;

  // Actions — Wave 0 stubs throw to keep tests RED until 03.1-06.
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
  ) => Promise<{ outcome: "pass" | "fail" | "indeterminate" | "manual" }>;
  getProgress: (blockId: string, learnerId: string) => Promise<LabProgress>;
}

export const useLabStore = create<LabState>((_set, _get) => ({
  sessions: new Map(),
  progress: new Map(),

  openSession: async () => {
    throw new Error("useLabStore.openSession not implemented (Wave 1+ — 03.1-06)");
  },
  closeSession: async () => {
    throw new Error("useLabStore.closeSession not implemented (Wave 1+ — 03.1-06)");
  },
  markStepComplete: async () => {
    throw new Error("useLabStore.markStepComplete not implemented (Wave 1+ — 03.1-06)");
  },
  getProgress: async () => {
    throw new Error("useLabStore.getProgress not implemented (Wave 1+ — 03.1-06)");
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
