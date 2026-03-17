import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
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
});
