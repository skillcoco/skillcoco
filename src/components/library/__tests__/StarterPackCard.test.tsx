// Phase 16 Plan 02 Task 2 — StarterPackCard (LIB-04/D-13).
//
// Free starter tile: "Free" pill, single Start button -> startStarterPack ->
// navigate(/track/{trackId}) on success. No progress bar, no attribution.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import type { StarterPackMeta } from "@/lib/tauri-commands";

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
  startStarterPack: vi.fn(),
}));

import { startStarterPack } from "@/lib/tauri-commands";
import { StarterPackCard } from "@/components/library/StarterPackCard";

function makePack(overrides: Partial<StarterPackMeta> = {}): StarterPackMeta {
  return {
    id: "kubernetes-fundamentals-starter",
    title: "Kubernetes Fundamentals",
    description: "Learn the basics of Kubernetes.",
    moduleCount: 5,
    ...overrides,
  };
}

function renderCard(pack: StarterPackMeta) {
  return render(
    <MemoryRouter>
      <StarterPackCard pack={pack} />
    </MemoryRouter>,
  );
}

describe("StarterPackCard — Phase 16 Plan 02 Task 2", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the Free pill", () => {
    renderCard(makePack());
    expect(screen.getByText("Free")).toBeInTheDocument();
  });

  it("shows a Start button with aria-label interpolating the pack title", () => {
    renderCard(makePack({ title: "Python for DevOps" }));
    expect(
      screen.getByRole("button", { name: "Start Python for DevOps" }),
    ).toBeInTheDocument();
  });

  it("calls startStarterPack and navigates to /track/{trackId} on success", async () => {
    vi.mocked(startStarterPack).mockResolvedValue({
      trackId: "track-99",
      moduleCount: 5,
      blockCount: 10,
      warnings: [],
    });
    const user = userEvent.setup();
    renderCard(makePack({ id: "rust-from-zero-starter", title: "Rust From Zero" }));

    await user.click(screen.getByRole("button", { name: "Start Rust From Zero" }));

    expect(startStarterPack).toHaveBeenCalledWith("rust-from-zero-starter");
    await vi.waitFor(() => {
      expect(mockNavigate).toHaveBeenCalledWith("/track/track-99");
    });
  });

  it("shows a Loader2 spinner and disables the button while starting", async () => {
    let resolveFn: ((v: Awaited<ReturnType<typeof startStarterPack>>) => void) | undefined;
    vi.mocked(startStarterPack).mockReturnValue(
      new Promise((resolve) => {
        resolveFn = resolve;
      }),
    );
    const user = userEvent.setup();
    renderCard(makePack());

    const btn = screen.getByRole("button", { name: /start/i });
    await user.click(btn);

    expect(btn).toBeDisabled();
    resolveFn?.({ trackId: "t", moduleCount: 0, blockCount: 0, warnings: [] });
  });

  // WR-04 — the backend's typed, user-facing error messages (D-11 taxonomy)
  // must be surfaced, not discarded behind generic copy.
  it("surfaces the backend error message when startStarterPack rejects", async () => {
    vi.mocked(startStarterPack).mockRejectedValue(
      new Error("This pack was modified after it was signed."),
    );
    const user = userEvent.setup();
    renderCard(makePack());

    await user.click(screen.getByRole("button", { name: /start/i }));

    expect(
      await screen.findByText("This pack was modified after it was signed."),
    ).toBeInTheDocument();
  });

  it("falls back to generic copy (without the removed Settings -> Import pointer) when the error is empty", async () => {
    vi.mocked(startStarterPack).mockRejectedValue(new Error(""));
    const user = userEvent.setup();
    renderCard(makePack());

    await user.click(screen.getByRole("button", { name: /start/i }));

    const fallback = await screen.findByText(/Couldn't start this pack/i);
    expect(fallback).toBeInTheDocument();
    expect(fallback.textContent).not.toMatch(/Settings/i);
  });

  it("renders no progress bar and no attribution line", () => {
    renderCard(makePack());
    expect(screen.queryByText(/% complete/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Licensed to/)).not.toBeInTheDocument();
  });
});
