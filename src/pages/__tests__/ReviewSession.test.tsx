// Plan 03 (LOOP-04) — queue-based ReviewSession with re-fetch and interval delta.
// All tests in this file should be GREEN after Plan 03.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ReviewSession } from "../ReviewSession";

// Mock tauri commands
const mockGetDueCards = vi.fn();
const mockSubmitReview = vi.fn();

vi.mock("@/lib/tauri-commands", () => ({
  getDueCards: (...args: unknown[]) => mockGetDueCards(...args),
  submitReview: (...args: unknown[]) => mockSubmitReview(...args),
}));

const MOCK_CARD = {
  id: "card-1",
  moduleId: "mod-1",
  concept: "Kubernetes Pods",
  cardType: "active_recall",
  front: "What is a Pod in Kubernetes?",
  back: "A Pod is the smallest deployable unit in Kubernetes, containing one or more containers.",
  intervalDays: 1,
  easeFactor: 2.5,
  repetitions: 0,
  nextReview: new Date().toISOString(),
  lastReview: null,
};

// SubmitReviewResult shape from Plan 01-03 Rust changes
const MOCK_REVIEW_RESULT = {
  newIntervalDays: 3,
  nextReview: new Date(Date.now() + 3 * 86400 * 1000).toISOString(),
  easeFactor: 2.5,
};

function renderReviewSession() {
  return render(
    <MemoryRouter>
      <ReviewSession />
    </MemoryRouter>,
  );
}

describe("ReviewSession", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows loading state initially", () => {
    mockGetDueCards.mockReturnValue(new Promise(() => {})); // never resolves
    renderReviewSession();
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it("shows empty state when no cards due", async () => {
    mockGetDueCards.mockResolvedValue([]);
    renderReviewSession();
    // "All caught up" heading shown when no cards are due
    expect(await screen.findByRole("heading", { name: /all caught up/i })).toBeInTheDocument();
  });

  it("shows card front initially", async () => {
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    renderReviewSession();
    expect(await screen.findByText(/what is a pod/i)).toBeInTheDocument();
  });

  it("reveals answer on click", async () => {
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    renderReviewSession();
    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);
    expect(await screen.findByText(/smallest deployable unit/i)).toBeInTheDocument();
  });

  it("shows rating buttons after reveal", async () => {
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    renderReviewSession();
    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);
    expect(await screen.findByText("Again")).toBeInTheDocument();
    expect(screen.getByText("Hard")).toBeInTheDocument();
    expect(screen.getByText("Good")).toBeInTheDocument();
    expect(screen.getByText("Easy")).toBeInTheDocument();
  });

  it("shows completion when all cards reviewed", async () => {
    // First getDueCards returns [card], after submit returns []
    mockGetDueCards
      .mockResolvedValueOnce([MOCK_CARD])
      .mockResolvedValueOnce([]);
    mockSubmitReview.mockResolvedValue(MOCK_REVIEW_RESULT);
    renderReviewSession();

    // Reveal
    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);

    // Rate
    const goodBtn = await screen.findByText("Good");
    fireEvent.click(goodBtn);

    // Should show completion after re-fetch returns empty
    expect(await screen.findByText(/session complete/i)).toBeInTheDocument();
  });

  // ── LOOP-04: queue re-fetch + interval delta ──

  it("re-fetches due cards after each rating submission", async () => {
    const card2 = { ...MOCK_CARD, id: "card-2", front: "What is a Deployment?" };

    // First call returns [card1, card2], second call (after rating) returns [card2]
    mockGetDueCards
      .mockResolvedValueOnce([MOCK_CARD, card2])
      .mockResolvedValueOnce([card2]);
    mockSubmitReview.mockResolvedValue(MOCK_REVIEW_RESULT);

    renderReviewSession();

    // Show and rate first card
    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);
    const goodBtn = await screen.findByText("Good");
    fireEvent.click(goodBtn);

    // After rating, getDueCards must be called again (2 total: mount + after submit)
    await waitFor(() => {
      expect(mockGetDueCards).toHaveBeenCalledTimes(2);
    }, { timeout: 2000 });
  });

  it("displays interval delta after rating (e.g., 'Next review in N days')", async () => {
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    mockSubmitReview.mockResolvedValue(MOCK_REVIEW_RESULT); // newIntervalDays: 3

    renderReviewSession();

    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);
    const goodBtn = await screen.findByText("Good");
    fireEvent.click(goodBtn);

    // Expect interval delta text after rating
    await waitFor(() => {
      expect(screen.getByText(/next review in \d+ days?/i)).toBeInTheDocument();
    }, { timeout: 2000 });
  });
});
