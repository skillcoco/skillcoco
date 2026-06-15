// Phase 4 Wave 0 — RED scaffold for DailyChallenge page.
//
// Imports `@/pages/DailyChallenge` which does NOT exist yet. Vitest fails
// with "Cannot find module" — that IS the RED state and the contract Plan 05
// satisfies (Plan 05 lands the page).

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

// Stub BlockRenderer so the page test isn't coupled to block rendering.
vi.mock("@/components/learning/BlockRenderer", () => ({
  BlockRenderer: ({ block }: { block: { id: string } }) => (
    <div
      data-testid="block-renderer-mock"
      onClick={() => window.dispatchEvent(new CustomEvent("daily-block-complete"))}
    >
      block-{block.id}
    </div>
  ),
}));

// Wave 0 typed shell lives at `@/pages/DailyChallenge`. The stub renders
// null so the BlockRenderer-mock test-id never appears — assertion-level
// RED state is preserved (waitFor times out). Plan 05 replaces the stub.
import { DailyChallenge } from "@/pages/DailyChallenge";
import { useDailyChallengeStore } from "@/stores/useDailyChallengeStore";
import * as commands from "@/lib/tauri-commands";

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
  startDailyChallenge: (challengeDate: string) => Promise<void>;
  completeDailyChallenge: (challengeDate: string) => Promise<void>;
}

function mockState(state: ChallengeState) {
  vi.mocked(useDailyChallengeStore).mockReturnValue(state as unknown as ReturnType<typeof useDailyChallengeStore>);
}

function renderPage() {
  return render(
    <MemoryRouter initialEntries={["/daily/today"]}>
      <DailyChallenge />
    </MemoryRouter>,
  );
}

describe("DailyChallenge — Phase 4 Wave 0 (failing scaffolds)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockNavigate.mockReset();
  });

  it("calls startDailyChallenge on mount (sets started_at server-side)", async () => {
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

    vi.mocked(commands.getModuleBlocks).mockResolvedValue([
      {
        id: "blk-1",
        moduleId: "mod-1",
        ordering: 0,
        blockType: "section",
        status: "ready",
        paramsJson: "{}",
        payloadJson: '{"markdown":"# Hi"}',
        sourceAnchorsJson: "[]",
        metadataJson: "{}",
        retryCount: 0,
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:00:00Z",
      },
    ]);

    renderPage();

    await waitFor(() => {
      expect(startSpy).toHaveBeenCalledTimes(1);
    });
  });

  it("renders BlockRenderer with the selected block payload", async () => {
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

    vi.mocked(commands.getModuleBlocks).mockResolvedValue([
      {
        id: "blk-1",
        moduleId: "mod-1",
        ordering: 0,
        blockType: "section",
        status: "ready",
        paramsJson: "{}",
        payloadJson: '{"markdown":"# Hi"}',
        sourceAnchorsJson: "[]",
        metadataJson: "{}",
        retryCount: 0,
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:00:00Z",
      },
    ]);

    renderPage();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-mock")).toBeInTheDocument();
      expect(screen.getByText(/block-blk-1/)).toBeInTheDocument();
    });
  });

  it("on block completion, calls completeDailyChallenge then navigates to /", async () => {
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

    vi.mocked(commands.getModuleBlocks).mockResolvedValue([
      {
        id: "blk-1",
        moduleId: "mod-1",
        ordering: 0,
        blockType: "section",
        status: "ready",
        paramsJson: "{}",
        payloadJson: '{"markdown":"# Hi"}',
        sourceAnchorsJson: "[]",
        metadataJson: "{}",
        retryCount: 0,
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:00:00Z",
      },
    ]);

    renderPage();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-mock")).toBeInTheDocument();
    });

    // Simulate underlying block completion (the stub dispatches the event on
    // click; the real DailyChallenge page subscribes to a completion signal
    // from BlockRenderer's child blocks).
    fireEvent.click(screen.getByTestId("block-renderer-mock"));

    await waitFor(() => {
      expect(completeSpy).toHaveBeenCalled();
      expect(mockNavigate).toHaveBeenCalledWith("/");
    });
  });

  it("exit without complete (unmount before completion signal) leaves status in_progress — no completeDailyChallenge call", async () => {
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

    vi.mocked(commands.getModuleBlocks).mockResolvedValue([
      {
        id: "blk-1",
        moduleId: "mod-1",
        ordering: 0,
        blockType: "section",
        status: "ready",
        paramsJson: "{}",
        payloadJson: '{"markdown":"# Hi"}',
        sourceAnchorsJson: "[]",
        metadataJson: "{}",
        retryCount: 0,
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:00:00Z",
      },
    ]);

    const { unmount } = renderPage();

    await waitFor(() => {
      expect(screen.getByTestId("block-renderer-mock")).toBeInTheDocument();
    });

    // Unmount without triggering the completion event.
    unmount();

    expect(completeSpy).not.toHaveBeenCalled();
  });
});
