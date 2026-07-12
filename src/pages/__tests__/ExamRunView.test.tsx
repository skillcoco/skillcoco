// Phase 19 plan 19-07 gap-closure delta review (WR-01) — direct test
// coverage for ExamRunView's D-06 history-wiring logic: the post-submit
// examAttemptHistory fetch (own try/catch, silent setHistory(null) on
// failure), the `history.totalAttempts > 1` gate that decides whether the
// note renders, and the `new Date(history.bestAttemptDate).toLocaleDateString()`
// conversion fed into ExamResultsPanel. None of this was covered by any
// existing test before this file (ExamResultsPanel.test.tsx exercises the
// panel in isolation with an already-formatted date supplied directly).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import type {
  ExamAttemptHistoryResult,
  ExamAttemptResult,
  ExamAttemptStartResult,
  ExamBlockRef,
  ModuleBlock,
} from "@/types/learning";

// ── Mocks ────────────────────────────────────────────────────────────────

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useParams: () => ({ trackId: "trk-exam-1", blockId: "blk-exam-1" }),
    useNavigate: () => vi.fn(),
  };
});

// LabBlock mounts a real terminal session; stub it so the "running" view
// renders without needing a live lab session (mirrors the precedent in
// BlockRenderer.test.tsx).
vi.mock("@/components/labs/LabBlock", () => ({
  LabBlock: () => <div data-testid="lab-block-stub" />,
}));

// ExamTimer's countdown/onExpire behavior is covered by its own test file;
// stub it here so this suite stays focused on the history-wiring logic.
vi.mock("@/components/labs/exam/ExamTimer", () => ({
  ExamTimer: () => <div data-testid="exam-timer-stub" />,
}));

const examBlocksForTrackMock =
  vi.fn<() => Promise<ExamBlockRef[]>>();
const getModuleBlocksMock =
  vi.fn<(moduleId: string) => Promise<ModuleBlock[]>>();
const getOrCreateProfileMock = vi.fn();
const examAttemptStartMock =
  vi.fn<() => Promise<ExamAttemptStartResult>>();
const examAttemptSubmitMock = vi.fn();
const examAttemptGetMock = vi.fn<() => Promise<ExamAttemptResult>>();
const examAttemptHistoryMock =
  vi.fn<() => Promise<ExamAttemptHistoryResult>>();

vi.mock("@/lib/tauri-commands", () => ({
  examAttemptHistory: (...args: unknown[]) =>
    examAttemptHistoryMock(...(args as Parameters<typeof examAttemptHistoryMock>)),
  examBlocksForTrack: (...args: unknown[]) =>
    examBlocksForTrackMock(...(args as Parameters<typeof examBlocksForTrackMock>)),
  getModuleBlocks: (...args: unknown[]) =>
    getModuleBlocksMock(...(args as Parameters<typeof getModuleBlocksMock>)),
  getOrCreateProfile: (...args: unknown[]) =>
    getOrCreateProfileMock(...(args as [])),
  examAttemptStart: (...args: unknown[]) =>
    examAttemptStartMock(...(args as Parameters<typeof examAttemptStartMock>)),
  examAttemptSubmit: (...args: unknown[]) =>
    examAttemptSubmitMock(...(args as [])),
  examAttemptGet: (...args: unknown[]) =>
    examAttemptGetMock(...(args as Parameters<typeof examAttemptGetMock>)),
}));

// Import AFTER mocks.
import { ExamRunView } from "@/pages/ExamRunView";
import { __resetStore as resetExamStore } from "@/stores/useExamStore";
import { __resetStore as resetLabStore } from "@/stores/useLabStore";

function makeBlock(): ModuleBlock {
  const spec = {
    slug: "exam-fixture",
    title: "Exam Fixture",
    requiresDocker: false,
    creates: [],
    exam: { timeLimitMinutes: 45, passThresholdPct: 70 },
    steps: [
      {
        id: "write-manifest",
        title: "Write the manifest",
        prompt: "Write a Pod manifest.",
        check: { kind: "file_state", path: "pod.yaml" },
        hints: [],
        weight: 1.0,
      },
    ],
  };
  return {
    id: "blk-exam-1",
    moduleId: "mod-exam-1",
    ordering: 0,
    blockType: "lab",
    status: "ready",
    paramsJson: "{}",
    payloadJson: JSON.stringify({ spec }),
    sourceAnchorsJson: "[]",
    metadataJson: "{}",
    retryCount: 0,
    createdAt: "2026-07-01T00:00:00Z",
    updatedAt: "2026-07-01T00:00:00Z",
  };
}

function makeStartResult(): ExamAttemptStartResult {
  return {
    attemptId: "exam-attempt-1",
    startedAt: "2026-07-10T10:00:00.000Z",
    deadlineAt: "2026-07-10T10:45:00.000Z",
    timeLimitMinutes: 45,
    passThresholdPct: 70,
    totalSteps: 1,
  };
}

function makeExamResult(
  overrides: Partial<ExamAttemptResult> = {},
): ExamAttemptResult {
  return {
    attemptId: "exam-attempt-1",
    status: "completed",
    scorePercent: 100,
    passed: true,
    startedAt: "2026-07-10T10:00:00.000Z",
    finishedAt: "2026-07-10T10:20:00.000Z",
    deadlineAt: "2026-07-10T10:45:00.000Z",
    totalSteps: 1,
    stepVerdicts: [
      {
        stepId: "write-manifest",
        title: "Write the manifest",
        outcome: "pass",
        passedTowardScore: true,
        checkReason: null,
      },
    ],
    ...overrides,
  };
}

function renderExamRunView() {
  return render(
    <MemoryRouter>
      <ExamRunView />
    </MemoryRouter>,
  );
}

/** Drives the view from "start" through to "results" via the real UI flow. */
async function runToResults() {
  const user = userEvent.setup();
  renderExamRunView();

  const beginButton = await screen.findByTestId("exam-begin-button");
  await user.click(beginButton);

  const submitButton = await screen.findByTestId("exam-submit-button");
  await user.click(submitButton);

  const confirmButton = await screen.findByTestId(
    "exam-confirm-submit-button",
  );
  await user.click(confirmButton);

  await screen.findByTestId("exam-results-panel");
}

describe("ExamRunView — D-06 history wiring (WR-01)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetExamStore();
    resetLabStore();

    examBlocksForTrackMock.mockResolvedValue([
      { moduleId: "mod-exam-1", blockId: "blk-exam-1" },
    ]);
    getModuleBlocksMock.mockResolvedValue([makeBlock()]);
    getOrCreateProfileMock.mockResolvedValue({
      id: "lp-1",
      displayName: "Ada Lovelace",
    });
    examAttemptStartMock.mockResolvedValue(makeStartResult());
    examAttemptSubmitMock.mockResolvedValue(makeExamResult());
    examAttemptGetMock.mockResolvedValue(makeExamResult());
  });

  it("suppresses the history note when totalAttempts === 1", async () => {
    examAttemptHistoryMock.mockResolvedValue({
      attemptNumber: 1,
      totalAttempts: 1,
      bestScorePercent: 100,
      bestAttemptDate: "2026-07-10T10:20:00.000Z",
    });

    await runToResults();

    await waitFor(() => {
      expect(examAttemptHistoryMock).toHaveBeenCalledWith({
        attemptId: "exam-attempt-1",
      });
    });
    // The gate gives the fetch's promise a tick to resolve into state.
    await new Promise((r) => setTimeout(r, 10));
    expect(
      screen.queryByTestId("exam-attempt-history-note"),
    ).not.toBeInTheDocument();
  });

  it("renders the note with a locale-formatted date when totalAttempts > 1", async () => {
    examAttemptHistoryMock.mockResolvedValue({
      attemptNumber: 2,
      totalAttempts: 2,
      bestScorePercent: 88,
      bestAttemptDate: "2026-07-10T10:20:00.000Z",
    });

    await runToResults();

    const note = await screen.findByTestId("exam-attempt-history-note");
    const expectedDate = new Date(
      "2026-07-10T10:20:00.000Z",
    ).toLocaleDateString();
    expect(note.textContent).toContain("This is attempt 2 of 2");
    expect(note.textContent).toContain(expectedDate);
    // The raw ISO string must NOT leak through unconverted.
    expect(note.textContent).not.toContain("2026-07-10T10:20:00.000Z");
  });

  it("still renders the results screen when examAttemptHistory rejects", async () => {
    examAttemptHistoryMock.mockRejectedValue(new Error("db unavailable"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    await runToResults();

    await waitFor(() => {
      expect(examAttemptHistoryMock).toHaveBeenCalled();
    });
    await new Promise((r) => setTimeout(r, 10));

    expect(screen.getByTestId("exam-results-panel")).toBeInTheDocument();
    expect(
      screen.queryByTestId("exam-attempt-history-note"),
    ).not.toBeInTheDocument();
    expect(warnSpy).toHaveBeenCalled();

    warnSpy.mockRestore();
  });
});
