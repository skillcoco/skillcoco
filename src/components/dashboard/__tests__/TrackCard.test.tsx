import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

vi.mock("@/stores/useLearningStore", () => {
  const mockDelete = vi.fn().mockResolvedValue(undefined);
  return {
    useLearningStore: Object.assign(
      (selector: (s: { deleteTrack: typeof mockDelete }) => unknown) =>
        selector({ deleteTrack: mockDelete }),
      { __mockDelete: mockDelete }
    ),
  };
});

import { TrackCard } from "@/components/dashboard/TrackCard";
import { useLearningStore } from "@/stores/useLearningStore";
import type { LearningTrack } from "@/types";

const baseTrack: LearningTrack = {
  id: "t1",
  learnerId: "lp1",
  topic: "Kubernetes",
  domainModule: "devops",
  status: "active",
  goal: "Pass CKA",
  currentModuleId: null,
  progressPercent: 25,
  totalTimeSpent: 3600,
  createdAt: "2026-05-05T00:00:00Z",
  updatedAt: "2026-05-05T00:00:00Z",
  streakDays: 0,
  lastActivityDate: null,
};

const renderCard = () =>
  render(
    <MemoryRouter>
      <TrackCard
        track={baseTrack}
        dueReviews={0}
        totalModules={10}
        completedModules={2}
        streakDays={0}
        nextModuleName={null}
      />
    </MemoryRouter>
  );

describe("TrackCard delete flow", () => {
  beforeEach(() => {
    // Clear the call history but keep the resolved-value mock implementation
    (
      useLearningStore as unknown as { __mockDelete: ReturnType<typeof vi.fn> }
    ).__mockDelete.mockClear();
  });

  it("trackcard_delete_button_visible — renders a delete button", () => {
    renderCard();
    const button = screen.getByRole("button", { name: /delete track/i });
    expect(button).toBeTruthy();
  });

  it("trackcard_delete_opens_confirmation — clicking delete shows a confirm dialog", () => {
    renderCard();
    fireEvent.click(screen.getByRole("button", { name: /delete track/i }));
    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(screen.getByText(/this action cannot be undone/i)).toBeTruthy();
  });

  it("trackcard_delete_cancel_dismisses — Cancel closes dialog without calling deleteTrack", () => {
    renderCard();
    fireEvent.click(screen.getByRole("button", { name: /delete track/i }));
    fireEvent.click(screen.getByRole("button", { name: /^cancel$/i }));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(
      (useLearningStore as unknown as { __mockDelete: ReturnType<typeof vi.fn> })
        .__mockDelete
    ).not.toHaveBeenCalled();
  });

  it("trackcard_delete_confirm_invokes_store — Confirm calls deleteTrack with the trackId", async () => {
    renderCard();
    fireEvent.click(screen.getByRole("button", { name: /delete track/i }));
    fireEvent.click(
      screen.getByRole("button", { name: /^delete$/i, hidden: false })
    );

    expect(
      (useLearningStore as unknown as { __mockDelete: ReturnType<typeof vi.fn> })
        .__mockDelete
    ).toHaveBeenCalledWith("t1");
  });

  it("trackcard_delete_button_does_not_navigate — clicking delete does not navigate to the track", () => {
    renderCard();
    const card = screen.getByRole("link");
    const initialHref = card.getAttribute("href");
    fireEvent.click(screen.getByRole("button", { name: /delete track/i }));
    // After click, dialog is open and the surrounding link is unchanged
    expect(card.getAttribute("href")).toBe(initialHref);
  });
});
