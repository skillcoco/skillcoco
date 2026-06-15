// Phase 4 Wave 4 (Plan 05) — GREEN test contract for the DailyChallenge page.
//
// Wave 0 scaffold tests are migrated here to match the parameterless action
// signatures from Plan 03 (server resolves challenge_date + learner_id) and
// the final page contract from Plan 05:
//   1. start fires on mount (Q4 — sets started_at server-side)
//   2. BlockRenderer-equivalent (DailyBlockHost) renders the selected block
//   3. block completion → completeChallenge + navigate("/")
//   4. exit without complete leaves status in_progress (Q7 — no spurious
//      completeChallenge call)
//   5. expired block (R5/Pitfall 8 — FK CASCADE race) → expired message +
//      navigate back
//   6. back-button click → navigate("/") without completing
//   7. todaysChallenge === null in store → mount-time redirect to "/"

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

// Vitest hoisting rule: inline literals only inside the factory body.
vi.mock("@/stores/useDailyChallengeStore", () => ({
  useDailyChallengeStore: vi.fn(),
}));

vi.mock("@/lib/tauri-commands", () => ({
  getModuleBlocks: vi.fn(),
  startDailyChallenge: vi.fn(),
  completeDailyChallenge: vi.fn(),
}));

// react-router-dom — only mock useNavigate; keep MemoryRouter real.
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

// Stub the three block components so DailyBlockHost dispatch is observable
// without coupling to block-internal behavior. Each stub exposes a button
// that invokes onComplete — exercising the daily-challenge completion path.
vi.mock("@/components/learning/SectionBlock", () => ({
  SectionBlock: ({ block, onComplete }: { block: { id: string }; onComplete?: () => void }) => (
    <div data-testid="section-block-mock">
      <span>section-{block.id}</span>
      <button data-testid="section-complete-trigger" onClick={() => onComplete?.()}>
        complete section
      </button>
    </div>
  ),
}));

vi.mock("@/components/learning/QuizBlock", () => ({
  QuizBlock: ({ block, onComplete }: { block: { id: string }; onComplete?: () => void }) => (
    <div data-testid="quiz-block-mock">
      <span>quiz-{block.id}</span>
      <button data-testid="quiz-complete-trigger" onClick={() => onComplete?.()}>
        complete quiz
      </button>
    </div>
  ),
}));

vi.mock("@/components/learning/FlashCardsBlock", () => ({
  FlashCardsBlock: ({ block, onComplete }: { block: { id: string }; onComplete?: () => void }) => (
    <div data-testid="flashcards-block-mock">
      <span>flashcards-{block.id}</span>
      <button data-testid="flashcards-complete-trigger" onClick={() => onComplete?.()}>
        complete flashcards
      </button>
    </div>
  ),
}));

import { DailyChallenge } from "@/pages/DailyChallenge";
import { useDailyChallengeStore } from "@/stores/useDailyChallengeStore";
import * as commands from "@/lib/tauri-commands";
import type { ModuleBlock, BlockType } from "@/types/learning";

interface ChallengeState {
  isEnabled: boolean;
  globalStreakDays: number;
  todaysChallenge:
    | {
        blockId: string;
        blockType: string;
        moduleId: string;
        trackId: string;
        estMinutes: number;
        status: "pending" | "in_progress" | "done";
      }
    | null;
  loadDailyChallenge: () => Promise<void>;
  startDailyChallenge: () => Promise<void>;
  completeDailyChallenge: () => Promise<void>;
}

// The real store hook supports both selector-style invocation and
// full-state reads. Mirror that surface so the page can use either pattern.
function mockState(state: ChallengeState) {
  const mocked = vi.mocked(useDailyChallengeStore) as unknown as ReturnType<typeof vi.fn>;
  mocked.mockImplementation((selector?: (s: ChallengeState) => unknown) =>
    typeof selector === "function" ? selector(state) : state,
  );
}

function makeBlock(
  overrides: Partial<{ id: string; moduleId: string; blockType: BlockType }> = {},
): ModuleBlock {
  return {
    id: overrides.id ?? "blk-1",
    moduleId: overrides.moduleId ?? "mod-1",
    ordering: 0,
    blockType: overrides.blockType ?? "section",
    status: "ready",
    paramsJson: "{}",
    payloadJson: '{"markdown":"# Hi"}',
    sourceAnchorsJson: "[]",
    metadataJson: "{}",
    retryCount: 0,
    createdAt: "2026-06-15T00:00:00Z",
    updatedAt: "2026-06-15T00:00:00Z",
  };
}

function renderPage() {
  return render(
    <MemoryRouter initialEntries={["/daily/today"]}>
      <DailyChallenge />
    </MemoryRouter>,
  );
}

describe("DailyChallenge — Phase 4 Wave 4 (Plan 05 GREEN)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockNavigate.mockReset();
  });

  it("calls startDailyChallenge on mount (Q4 — sets started_at server-side)", async () => {
    const startSpy = vi.fn().mockResolvedValue(undefined);
    mockState({
      isEnabled: true,
      globalStreakDays: 2,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "pending",
      },
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
      startDailyChallenge: startSpy,
      completeDailyChallenge: vi.fn().mockResolvedValue(undefined),
    });

    vi.mocked(commands.getModuleBlocks).mockResolvedValue([makeBlock()]);

    renderPage();

    await waitFor(() => {
      expect(startSpy).toHaveBeenCalledTimes(1);
    });
  });

  it("renders the selected block via DailyBlockHost when block fetch succeeds", async () => {
    mockState({
      isEnabled: true,
      globalStreakDays: 1,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "in_progress",
      },
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
      startDailyChallenge: vi.fn().mockResolvedValue(undefined),
      completeDailyChallenge: vi.fn().mockResolvedValue(undefined),
    });

    vi.mocked(commands.getModuleBlocks).mockResolvedValue([makeBlock()]);

    renderPage();

    await waitFor(() => {
      expect(screen.getByTestId("daily-challenge-view")).toBeInTheDocument();
    });
    expect(screen.getByTestId("section-block-mock")).toBeInTheDocument();
    expect(screen.getByText(/section-blk-1/)).toBeInTheDocument();
  });

  it("on block completion calls completeDailyChallenge then navigates to /", async () => {
    const completeSpy = vi.fn().mockResolvedValue(undefined);
    mockState({
      isEnabled: true,
      globalStreakDays: 2,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "in_progress",
      },
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
      startDailyChallenge: vi.fn().mockResolvedValue(undefined),
      completeDailyChallenge: completeSpy,
    });

    vi.mocked(commands.getModuleBlocks).mockResolvedValue([makeBlock()]);

    renderPage();

    await waitFor(() => {
      expect(screen.getByTestId("section-block-mock")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("section-complete-trigger"));

    await waitFor(() => {
      expect(completeSpy).toHaveBeenCalledTimes(1);
      expect(mockNavigate).toHaveBeenCalledWith("/", { replace: true });
    });
  });

  it("exit without complete leaves status in_progress — no completeDailyChallenge call (Q7)", async () => {
    const completeSpy = vi.fn().mockResolvedValue(undefined);
    mockState({
      isEnabled: true,
      globalStreakDays: 2,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "in_progress",
      },
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
      startDailyChallenge: vi.fn().mockResolvedValue(undefined),
      completeDailyChallenge: completeSpy,
    });

    vi.mocked(commands.getModuleBlocks).mockResolvedValue([makeBlock()]);

    const { unmount } = renderPage();

    await waitFor(() => {
      expect(screen.getByTestId("section-block-mock")).toBeInTheDocument();
    });

    // Unmount without firing the completion trigger.
    unmount();

    expect(completeSpy).not.toHaveBeenCalled();
  });

  it("expired block (FK CASCADE race) shows expired message and routes back", async () => {
    mockState({
      isEnabled: true,
      globalStreakDays: 2,
      todaysChallenge: {
        blockId: "blk-missing",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "pending",
      },
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
      startDailyChallenge: vi.fn().mockResolvedValue(undefined),
      completeDailyChallenge: vi.fn().mockResolvedValue(undefined),
    });

    // getModuleBlocks returns an array WITHOUT the expected block id.
    vi.mocked(commands.getModuleBlocks).mockResolvedValue([makeBlock({ id: "blk-different" })]);

    renderPage();

    // Wait for the expired message (async fetch settle).
    const expired = await screen.findByTestId("daily-challenge-expired");
    expect(expired).toBeInTheDocument();
    expect(screen.getByText(/expired/i)).toBeInTheDocument();

    // Wait for the deferred navigate (the page schedules a setTimeout(... 2500ms)
    // after surfacing the expired message). waitFor polls at default cadence;
    // bump its timeout above the 2500ms scheduled redirect.
    await waitFor(
      () => {
        expect(mockNavigate).toHaveBeenCalledWith("/", { replace: true });
      },
      { timeout: 4000 },
    );
  });

  it("back button navigates to / without completing", async () => {
    const completeSpy = vi.fn().mockResolvedValue(undefined);
    mockState({
      isEnabled: true,
      globalStreakDays: 2,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "in_progress",
      },
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
      startDailyChallenge: vi.fn().mockResolvedValue(undefined),
      completeDailyChallenge: completeSpy,
    });

    vi.mocked(commands.getModuleBlocks).mockResolvedValue([makeBlock()]);

    renderPage();

    await waitFor(() => {
      expect(screen.getByTestId("daily-challenge-back")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("daily-challenge-back"));

    expect(mockNavigate).toHaveBeenCalledWith("/", { replace: true });
    expect(completeSpy).not.toHaveBeenCalled();
  });

  it("no todaysChallenge in store → mount-time redirect to /", async () => {
    const startSpy = vi.fn().mockResolvedValue(undefined);
    mockState({
      isEnabled: true,
      globalStreakDays: 0,
      todaysChallenge: null,
      loadDailyChallenge: vi.fn().mockResolvedValue(undefined),
      startDailyChallenge: startSpy,
      completeDailyChallenge: vi.fn().mockResolvedValue(undefined),
    });

    renderPage();

    await waitFor(() => {
      expect(mockNavigate).toHaveBeenCalledWith("/", { replace: true });
    });
    // Defensive redirect — must NOT have called start on the empty state.
    expect(startSpy).not.toHaveBeenCalled();
  });
});
