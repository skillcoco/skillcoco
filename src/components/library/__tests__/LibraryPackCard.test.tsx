// Phase 16 Plan 02 Task 2 — LibraryPackCard (D-07/D-09/D-10/D-11).
//
// Owned-pack card: Continue (active/onboarding) / Review (completed) /
// Resume (paused/archived) primary action — every import creates an
// 'active' track (course_io.rs import_course_txn), so no "not-yet-started"
// state exists (WR-06). Progress bar for active packs, NO delete affordance
// (D-11 — pack removal out of scope this phase).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import type { LearningTrack } from "@/types";

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual =
    await vi.importActual<typeof import("react-router-dom")>(
      "react-router-dom",
    );
  return {
    ...actual,
    useNavigate: vi.fn(() => mockNavigate),
  };
});

import { LibraryPackCard } from "@/components/library/LibraryPackCard";

// NOTE: track.status "active" AND "onboarding" both count as in-progress
// (Continue) per Dashboard.tsx's activeTracks filter, which LibraryPackCard
// mirrors exactly. Non-in-progress statuses are paused/archived (Resume)
// and completed (Review) — imports never create a not-yet-active track.
function makeTrack(overrides: Partial<LearningTrack> = {}): LearningTrack {
  return {
    id: "track-1",
    learnerId: "learner-1",
    topic: "Kubernetes Fundamentals",
    domainModule: "devops",
    status: "paused",
    goal: "Learn k8s",
    currentModuleId: null,
    progressPercent: 0,
    totalTimeSpent: 0,
    createdAt: "2026-07-01T00:00:00Z",
    updatedAt: "2026-07-01T00:00:00Z",
    ...overrides,
  };
}

function renderCard(track: LearningTrack) {
  return render(
    <MemoryRouter>
      <LibraryPackCard track={track} />
    </MemoryRouter>,
  );
}

describe("LibraryPackCard — Phase 16 Plan 02 Task 2", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // WR-06 — labels match the actual status taxonomy: paused/archived tracks
  // read Resume, completed tracks read Review ("Start" was unreachable-as-
  // designed: imports always create status='active' tracks).
  it("shows a Resume button with aria-label interpolating the pack title for a paused track", async () => {
    renderCard(makeTrack({ status: "paused", topic: "Rust From Zero" }));
    expect(
      await screen.findByRole("button", { name: "Resume Rust From Zero" }),
    ).toBeInTheDocument();
  });

  it("shows a Review button for a completed track", async () => {
    renderCard(makeTrack({ status: "completed", topic: "Rust From Zero" }));
    expect(
      await screen.findByRole("button", { name: "Review Rust From Zero" }),
    ).toBeInTheDocument();
  });

  it("shows a Continue button + progress bar for an active track", async () => {
    renderCard(
      makeTrack({ status: "active", topic: "Python for DevOps", progressPercent: 42 }),
    );
    expect(
      await screen.findByRole("button", { name: "Continue Python for DevOps" }),
    ).toBeInTheDocument();
    expect(screen.getByText("42% complete")).toBeInTheDocument();
  });

  it("clicking Continue navigates to /track/{trackId}", async () => {
    const user = userEvent.setup();
    renderCard(makeTrack({ id: "track-42", status: "active", topic: "Go Basics" }));
    const btn = await screen.findByRole("button", { name: "Continue Go Basics" });
    await user.click(btn);
    expect(mockNavigate).toHaveBeenCalledWith("/track/track-42");
  });

  it("clicking Resume navigates straight to /track/{trackId} (no spinner — navigation cannot fail)", async () => {
    const user = userEvent.setup();
    renderCard(makeTrack({ id: "track-9", status: "paused", topic: "Go Basics" }));
    const btn = await screen.findByRole("button", { name: "Resume Go Basics" });
    await user.click(btn);
    expect(mockNavigate).toHaveBeenCalledWith("/track/track-9");
  });

  it("has no delete affordance (D-11)", async () => {
    renderCard(makeTrack());
    await screen.findByRole("button", { name: /resume|continue|review/i });
    expect(screen.queryByText(/delete/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/delete/i)).not.toBeInTheDocument();
  });
});
