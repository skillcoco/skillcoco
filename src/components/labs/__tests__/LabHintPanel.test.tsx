// Wave 0 failing tests for LabHintPanel (LAB-10): manual progressive 3-tier
// hint reveal per RESEARCH question #7 (revealedTier lives in component
// state, not the store; resets when the lab block remounts).

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { LabHintPanel } from "@/components/labs/LabHintPanel";

const HINTS = [
  "Tier 1 — gentle nudge: re-read the prompt.",
  "Tier 2 — partial answer: think about which kubectl flag.",
  "Tier 3 — full solution: kubectl get pods --all-namespaces",
];

describe("LabHintPanel — Phase 03.1 Wave 0 (failing scaffolds)", () => {
  it("lab_hint_panel_renders_no_hints_at_tier_0 — only the Show hint button", () => {
    // FAILS today — stub renders a placeholder div.
    render(<LabHintPanel hints={HINTS} revealedTier={0} onShowNext={vi.fn()} />);
    expect(screen.queryByText(HINTS[0])).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /show hint/i })).toBeInTheDocument();
  });

  it("lab_hint_panel_reveals_tier_1 — first click reveals tier 1 text", async () => {
    const onShowNext = vi.fn();
    const user = userEvent.setup();
    render(<LabHintPanel hints={HINTS} revealedTier={0} onShowNext={onShowNext} />);
    await user.click(screen.getByRole("button", { name: /show hint/i }));
    expect(onShowNext).toHaveBeenCalledTimes(1);
  });

  it("lab_hint_panel_reveals_tier_3_then_disables_button — final tier is terminal", () => {
    render(<LabHintPanel hints={HINTS} revealedTier={3} onShowNext={vi.fn()} />);
    expect(screen.getByText(HINTS[0])).toBeInTheDocument();
    expect(screen.getByText(HINTS[1])).toBeInTheDocument();
    expect(screen.getByText(HINTS[2])).toBeInTheDocument();
    const btn = screen.getByRole("button", { name: /show hint/i });
    expect(btn).toBeDisabled();
  });

  it("lab_hint_panel_marks_final_tier_via_data_attr — data-final-tier=true at tier 3", () => {
    render(<LabHintPanel hints={HINTS} revealedTier={3} onShowNext={vi.fn()} />);
    const root = screen.getByTestId("lab-hint-panel");
    expect(root.getAttribute("data-final-tier")).toBe("true");
  });
});
