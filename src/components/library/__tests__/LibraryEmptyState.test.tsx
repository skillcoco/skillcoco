// Phase 16 Plan 02 Task 2 — LibraryEmptyState (D-08).
//
// Renders only inside "Your packs" when zero owned tracks exist. py-12
// centered block, muted BookOpen, heading + body, NO CTA buttons (guidance
// copy points at Redeem/Starter sections instead of duplicating an
// onboarding hero).

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { LibraryEmptyState } from "@/components/library/LibraryEmptyState";

describe("LibraryEmptyState — Phase 16 Plan 02 Task 2", () => {
  it("renders the locked heading and body copy", () => {
    render(<LibraryEmptyState />);
    expect(screen.getByText("No packs yet")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Redeem a license key, import a course file, or pick a starter pack below to get going.",
      ),
    ).toBeInTheDocument();
  });

  it("contains no CTA buttons or links", () => {
    render(<LibraryEmptyState />);
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(screen.queryAllByRole("link")).toHaveLength(0);
  });
});
