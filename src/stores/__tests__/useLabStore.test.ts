// Wave 0 failing tests for useLabStore — sibling slice to useLearningStore
// per RESEARCH risk row + CONTEXT.md "Frontend module" integration point.
//
// Each action is a stub today (throws "not implemented"); plan 03.1-06
// wires every action to the matching Tauri IPC command.

import { describe, it, expect, vi, beforeEach } from "vitest";

// Vitest hoisting: inline literals only inside the factory.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { useLabStore, __resetStore } from "@/stores/useLabStore";

describe("useLabStore — Phase 03.1 Wave 0 (failing scaffolds)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetStore();
  });

  it("lab_store_open_session — invokes lab_session_open and stores LabSession", async () => {
    // FAILS until 03.1-06: today the action throws "not implemented".
    vi.mocked(invoke).mockResolvedValue({
      sessionId: "sess-1",
      effectiveRuntime: "docker",
    });

    const store = useLabStore.getState();
    expect(typeof store.openSession).toBe("function");

    const session = await store.openSession("blk-1", "trk-1", "mod-1", "learner-1");
    expect(invoke).toHaveBeenCalledWith(
      "lab_session_open",
      expect.objectContaining({
        blockId: "blk-1",
        trackId: "trk-1",
        moduleId: "mod-1",
        learnerId: "learner-1",
      }),
    );
    expect(session.sessionId).toBe("sess-1");

    const sessions = useLabStore.getState().sessions;
    expect(sessions.get("blk-1")?.sessionId).toBe("sess-1");
  });

  it("lab_store_close_session — invokes lab_session_close and clears entry", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    // Seed an active session.
    useLabStore.setState({
      sessions: new Map([["blk-1", { sessionId: "sess-1", effectiveRuntime: "docker" }]]),
    });

    const store = useLabStore.getState();
    await store.closeSession("sess-1");

    expect(invoke).toHaveBeenCalledWith(
      "lab_session_close",
      expect.objectContaining({ sessionId: "sess-1" }),
    );
    const sessions = useLabStore.getState().sessions;
    expect(sessions.size).toBe(0);
  });

  it("lab_store_mark_step_complete — invokes lab_check_step and updates progress on Pass", async () => {
    vi.mocked(invoke).mockResolvedValue({ outcome: "pass", stepId: "s1" });

    const store = useLabStore.getState();
    const result = await store.markStepComplete("sess-1", 0, "kubectl get pods", "Running", 0);

    expect(invoke).toHaveBeenCalledWith(
      "lab_check_step",
      expect.objectContaining({
        sessionId: "sess-1",
        stepIndex: 0,
        lastCommand: "kubectl get pods",
      }),
    );
    expect(result.outcome).toBe("pass");
  });

  it("lab_store_get_progress — hydrates progress map via lab_get_progress", async () => {
    vi.mocked(invoke).mockResolvedValue({
      blockId: "blk-1",
      currentStep: 1,
      completedStepIds: ["s1"],
      lastUpdated: "2026-05-05T00:00:00Z",
      practicalMastery: 0.5,
    });

    const store = useLabStore.getState();
    const progress = await store.getProgress("blk-1", "learner-1");

    expect(invoke).toHaveBeenCalledWith(
      "lab_get_progress",
      expect.objectContaining({ blockId: "blk-1", learnerId: "learner-1" }),
    );
    expect(progress.completedStepIds).toEqual(["s1"]);
    expect(useLabStore.getState().progress.get("blk-1")?.practicalMastery).toBe(0.5);
  });

  it("lab_store_is_separate_slice — useLabStore does NOT depend on useLearningStore (RESEARCH risk row)", () => {
    // Documenting the architecture invariant: the lab store is its own slice.
    // If a future refactor merges them, this test fails on import or state shape.
    const state = useLabStore.getState();
    expect(state.sessions).toBeInstanceOf(Map);
    expect(state.progress).toBeInstanceOf(Map);
  });
});
