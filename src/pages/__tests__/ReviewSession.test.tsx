// Wave 0 scaffold — Plan 03 (LOOP-04) makes the queue and interval-delta assertions green.
// Existing tests (loading, empty state, reveal, rating) pass today.
// Wave 0 additions (re-fetch after rating, interval delta display) FAIL today.

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
    expect(await screen.findByText(/no cards due/i)).toBeInTheDocument();
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
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    mockSubmitReview.mockResolvedValue({ ...MOCK_CARD, intervalDays: 6 });
    renderReviewSession();

    // Reveal
    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);

    // Rate
    const goodBtn = await screen.findByText("Good");
    fireEvent.click(goodBtn);

    // Should show completion
    expect(await screen.findByText(/session complete/i)).toBeInTheDocument();
  });

  // ── Wave 0 scaffolds — Plan 03 (LOOP-04) makes these green ──

  it("re-fetches due cards after each rating submission", async () => {
    // FAILING TODAY — Plan 03 (LOOP-04) will implement queue re-fetch after submit.
    // Today ReviewSession uses index-based nav and does not re-fetch after rating.
    const card2 = { ...MOCK_CARD, id: "card-2", front: "What is a Deployment?" };

    // First call returns [card1, card2], second call (after rating) returns [card2]
    mockGetDueCards
      .mockResolvedValueOnce([MOCK_CARD, card2])
      .mockResolvedValueOnce([card2]);
    mockSubmitReview.mockResolvedValue({ ...MOCK_CARD, intervalDays: 3 });

    renderReviewSession();

    // Show and rate first card
    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);
    const goodBtn = await screen.findByText("Good");
    fireEvent.click(goodBtn);

    // After rating, getDueCards must be called again (2 total: mount + after submit)
    // This assertion FAILS today — Plan 03 LOOP-04 must wire re-fetch in submit handler
    await waitFor(() => {
      expect(mockGetDueCards).toHaveBeenCalledTimes(2);
    }, { timeout: 2000 });
  });

  it("displays interval delta after rating (e.g., 'Next review in N days')", async () => {
    // FAILING TODAY — Plan 03 (LOOP-04) will implement interval delta display.
    // After rating, the review session should show "Next review in 3 days" or similar.
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    mockSubmitReview.mockResolvedValue({ ...MOCK_CARD, intervalDays: 3 });

    renderReviewSession();

    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);
    const goodBtn = await screen.findByText("Good");
    fireEvent.click(goodBtn);

    // Expect interval delta text after rating
    // This assertion FAILS today — Plan 03 LOOP-04 must add "Next review in N days" UI
    await waitFor(() => {
      expect(screen.getByText(/next review in \d+ days?/i)).toBeInTheDocument();
    }, { timeout: 2000 });
  });
});
