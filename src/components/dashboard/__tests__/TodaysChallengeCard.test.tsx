// Phase 4 Wave 0 — RED scaffold for TodaysChallengeCard.
//
// Imports `@/components/dashboard/TodaysChallengeCard` which does NOT exist
// yet. Vitest fails with "Cannot find module" — that IS the RED state and
// the contract Plan 04 satisfies.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

// Vitest hoisting rule: inline literals only inside the factory body.
vi.mock("@/stores/useDailyChallengeStore", () => ({
  useDailyChallengeStore: vi.fn(),
}));

// Wave 0 typed shell lives at `@/components/dashboard/TodaysChallengeCard`.
// The stub returns null so every screen.getByText / getByRole assertion
// fails — preserving the assertion-level RED state. Plan 04 replaces the
// stub with the SmartSessionCard-styled implementation.
import { TodaysChallengeCard } from "@/components/dashboard/TodaysChallengeCard";
import { useDailyChallengeStore } from "@/stores/useDailyChallengeStore";

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
}

function mockState(state: ChallengeState) {
  vi.mocked(useDailyChallengeStore).mockReturnValue(state as unknown as ReturnType<typeof useDailyChallengeStore>);
}

function renderCard() {
  return render(
    <MemoryRouter>
      <TodaysChallengeCard />
    </MemoryRouter>,
  );
}

describe("TodaysChallengeCard — Phase 4 Wave 0 (failing scaffolds)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders nothing when isEnabled is false (D-12 auto-enable gate)", () => {
    mockState({ isEnabled: false, globalStreakDays: 0, todaysChallenge: null });
    const { container } = renderCard();
    expect(container.firstChild).toBeNull();
  });

  it("renders pending state with est minutes + Start CTA when challenge.status === 'pending'", () => {
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
    });

    renderCard();
    expect(screen.getByText(/today's challenge/i)).toBeInTheDocument();
    expect(screen.getByText(/4\s*min/i)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /start/i })).toBeInTheDocument();
  });

  it("renders in-progress state with Resume CTA when challenge.status === 'in_progress'", () => {
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
    });

    renderCard();
    expect(screen.getByRole("link", { name: /resume/i })).toBeInTheDocument();
  });

  it("renders done state with 'Done for today' copy + streak summary, no CTA, when challenge.status === 'done'", () => {
    mockState({
      isEnabled: true,
      globalStreakDays: 4,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "section",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 4,
        status: "done",
      },
    });

    renderCard();
    expect(screen.getByText(/done for today/i)).toBeInTheDocument();
    expect(screen.getByText(/4\s*day/i)).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: /start|resume/i })).not.toBeInTheDocument();
  });

  it("renders empty-zone placeholder 'no challenge today; keep learning' when isEnabled but challenge is null (Q3)", () => {
    mockState({ isEnabled: true, globalStreakDays: 1, todaysChallenge: null });

    renderCard();
    expect(screen.getByText(/no challenge today/i)).toBeInTheDocument();
    expect(screen.getByText(/keep learning/i)).toBeInTheDocument();
  });

  it("Start CTA links to /daily/today (D-11)", () => {
    mockState({
      isEnabled: true,
      globalStreakDays: 0,
      todaysChallenge: {
        blockId: "blk-1",
        blockType: "quiz",
        moduleId: "mod-1",
        trackId: "trk-1",
        estMinutes: 5,
        status: "pending",
      },
    });

    renderCard();
    const link = screen.getByRole("link", { name: /start|resume/i });
    expect(link.getAttribute("href")).toBe("/daily/today");
  });
});
