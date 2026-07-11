// Phase 19 plan 19-05 — sibling Zustand slice for the exam-attempt
// lifecycle. Mirrors useLabStore's action-wraps-IPC-then-set shape
// (src/stores/useLabStore.ts) and its test pattern
// (src/stores/__tests__/useLabStore.test.ts).
//
// D-15 — examAttemptSubmit forwards only { attemptId, currentStep? };
// every step verdict is server-derived from lab_progress, never
// supplied by the client (T-19-10 mitigation). No test in this file
// should assert a stepVerdicts payload being sent on submit.

import { describe, it, expect, vi, beforeEach } from "vitest";

// Vitest hoisting: inline literals only inside the factory.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { useExamStore, __resetStore } from "@/stores/useExamStore";

describe("useExamStore — Phase 19 plan 19-05", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetStore();
  });

  it("exam_store_start_attempt — invokes exam_attempt_start and stores the attempt keyed by blockId", async () => {
    vi.mocked(invoke).mockResolvedValue({
      attemptId: "att-1",
      startedAt: "2026-07-11T00:00:00Z",
      deadlineAt: "2026-07-11T00:30:00Z",
      timeLimitMinutes: 30,
      passThresholdPct: 70,
      totalSteps: 3,
    });

    const store = useExamStore.getState();
    expect(typeof store.startAttempt).toBe("function");

    const result = await store.startAttempt(
      "learner-1",
      "mod-1",
      "blk-1",
      "trk-1",
    );

    expect(invoke).toHaveBeenCalledWith(
      "exam_attempt_start",
      expect.objectContaining({
        request: expect.objectContaining({
          learnerId: "learner-1",
          moduleId: "mod-1",
          blockId: "blk-1",
          trackId: "trk-1",
        }),
      }),
    );
    expect(result.attemptId).toBe("att-1");

    const attempts = useExamStore.getState().attempts;
    expect(attempts.get("blk-1")?.attemptId).toBe("att-1");
    expect(attempts.get("blk-1")?.deadlineAt).toBe("2026-07-11T00:30:00Z");
    expect(attempts.get("blk-1")?.totalSteps).toBe(3);
  });

  it("exam_store_submit_attempt — invokes exam_attempt_submit with attemptId + currentStep ONLY and stores the finalized result", async () => {
    vi.mocked(invoke).mockResolvedValue({
      attemptId: "att-1",
      status: "completed",
      scorePercent: 66.7,
      passed: false,
      startedAt: "2026-07-11T00:00:00Z",
      finishedAt: "2026-07-11T00:20:00Z",
      deadlineAt: "2026-07-11T00:30:00Z",
      totalSteps: 3,
      stepVerdicts: [
        {
          stepId: "s1",
          title: "List pods",
          outcome: "pass",
          passedTowardScore: true,
          checkReason: "Command output matched pattern",
        },
      ],
    });

    const store = useExamStore.getState();
    const result = await store.submitAttempt("att-1", 1);

    // D-15 — forwards ONLY attemptId + currentStep; no stepVerdicts field.
    expect(invoke).toHaveBeenCalledWith(
      "exam_attempt_submit",
      expect.objectContaining({
        request: { attemptId: "att-1", currentStep: 1 },
      }),
    );
    const [, callArgs] = vi.mocked(invoke).mock.calls[0] as [string, { request: object }];
    expect(callArgs.request).not.toHaveProperty("stepVerdicts");

    expect(result.status).toBe("completed");
    expect(result.scorePercent).toBe(66.7);
    expect(result.passed).toBe(false);

    const results = useExamStore.getState().results;
    expect(results.get("att-1")?.status).toBe("completed");
    expect(results.get("att-1")?.stepVerdicts).toHaveLength(1);
  });

  it("exam_store_submit_attempt_omits_current_step_when_absent — currentStep is optional telemetry", async () => {
    vi.mocked(invoke).mockResolvedValue({
      attemptId: "att-2",
      status: "completed",
      scorePercent: 100,
      passed: true,
      startedAt: "2026-07-11T00:00:00Z",
      finishedAt: "2026-07-11T00:05:00Z",
      deadlineAt: "2026-07-11T00:30:00Z",
      totalSteps: 1,
      stepVerdicts: [],
    });

    const store = useExamStore.getState();
    await store.submitAttempt("att-2");

    expect(invoke).toHaveBeenCalledWith(
      "exam_attempt_submit",
      expect.objectContaining({
        request: { attemptId: "att-2" },
      }),
    );
  });

  it("exam_store_get_attempt — invokes exam_attempt_get and stores the result", async () => {
    vi.mocked(invoke).mockResolvedValue({
      attemptId: "att-3",
      status: "in_progress",
      scorePercent: 0,
      passed: false,
      startedAt: "2026-07-11T00:00:00Z",
      finishedAt: null,
      deadlineAt: "2026-07-11T00:30:00Z",
      totalSteps: 3,
      stepVerdicts: [],
    });

    const store = useExamStore.getState();
    const result = await store.getAttempt("att-3");

    expect(invoke).toHaveBeenCalledWith(
      "exam_attempt_get",
      expect.objectContaining({
        request: { attemptId: "att-3" },
      }),
    );
    expect(result.status).toBe("in_progress");

    const results = useExamStore.getState().results;
    expect(results.get("att-3")?.status).toBe("in_progress");
  });

  it("exam_store_is_separate_slice — useExamStore does NOT depend on useLabStore/useLearningStore (T-19-08 grep guard)", () => {
    // Documenting the architecture invariant: the exam store is its own
    // sibling slice. If a future refactor merges them, this test fails
    // on import or state shape. The real guard is the source-level grep
    // (rg -c "useLabStore|useLearningStore" src/stores/useExamStore.ts
    // returns 0) run as an acceptance check outside this test file.
    const state = useExamStore.getState();
    expect(state.attempts).toBeInstanceOf(Map);
    expect(state.results).toBeInstanceOf(Map);
  });
});
