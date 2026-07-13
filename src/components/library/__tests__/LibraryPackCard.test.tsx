// Phase 16 Plan 02 Task 2 — LibraryPackCard (D-07/D-09/D-10/D-11).
//
// Owned-pack card: Start (not-started) / Continue (active) primary action,
// progress bar for active packs, issuer/verified badge + BuyerAttributionLine
// attribution (display-only, never fails the card), NO delete affordance
// (D-11 — pack removal out of scope this phase).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
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

vi.mock("@/lib/tauri-commands", () => ({
  getEntitlementForTrack: vi.fn(),
}));

import { getEntitlementForTrack } from "@/lib/tauri-commands";
import { LibraryPackCard } from "@/components/library/LibraryPackCard";

// NOTE: track.status "active" AND "onboarding" both count as in-progress
// (Continue) per Dashboard.tsx's activeTracks filter, which LibraryPackCard
// mirrors exactly. Tests needing a genuinely not-started pack use "paused"
// as the not-in-progress fixture status.
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
    vi.mocked(getEntitlementForTrack).mockResolvedValue(null);
  });

  it("shows a Start button with aria-label interpolating the pack title for a not-started track", async () => {
    renderCard(makeTrack({ status: "paused", topic: "Rust From Zero" }));
    expect(
      await screen.findByRole("button", { name: "Start Rust From Zero" }),
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

  it("shows a Loader2 spinner and disables the button while starting", async () => {
    renderCard(makeTrack({ id: "track-9", status: "paused", topic: "Go Basics" }));
    const btn = await screen.findByRole("button", { name: "Start Go Basics" });
    // fireEvent (unlike userEvent) does not auto-flush microtasks, so we can
    // observe the synchronous starting=true render before navigate resolves.
    fireEvent.click(btn);
    expect(btn).toBeDisabled();
    await waitFor(() => expect(mockNavigate).toHaveBeenCalledWith("/track/track-9"));
  });

  it("renders BuyerAttributionLine when getEntitlementForTrack resolves an entitlement", async () => {
    vi.mocked(getEntitlementForTrack).mockResolvedValue({
      issuerName: "SODA",
      buyerName: "Jane Doe",
      orderId: "ORD-1",
    });
    renderCard(makeTrack({ status: "active" }));
    expect(
      await screen.findByText("Licensed to Jane Doe · order #ORD-1"),
    ).toBeInTheDocument();
  });

  it("never fails the card when getEntitlementForTrack rejects", async () => {
    vi.mocked(getEntitlementForTrack).mockRejectedValue(new Error("db locked"));
    renderCard(makeTrack({ status: "active", topic: "Resilient Pack" }));
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "Continue Resilient Pack" }),
      ).toBeInTheDocument();
    });
  });

  it("has no delete affordance (D-11)", async () => {
    renderCard(makeTrack());
    await screen.findByRole("button", { name: /start|continue/i });
    expect(screen.queryByText(/delete/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/delete/i)).not.toBeInTheDocument();
  });
});
