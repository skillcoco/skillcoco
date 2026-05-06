// Wave 0 failing tests for LabResetDialog (LAB-07): the surgical reset
// confirm dialog must list spec.creates[] and require explicit confirmation.

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { LabResetDialog } from "@/components/labs/LabResetDialog";

const CREATES = ["deploy.yaml", "service.yaml", "configmap.yaml"];

describe("LabResetDialog — Phase 03.1 Wave 0 (failing scaffolds)", () => {
  it("lab_reset_dialog_lists_creates_paths — every creates[] path is on screen", () => {
    // FAILS today — stub renders a placeholder div with no path list.
    render(
      <LabResetDialog
        creates={CREATES}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
        open
      />,
    );
    for (const path of CREATES) {
      expect(screen.getByText(path)).toBeInTheDocument();
    }
  });

  it("lab_reset_dialog_cancel_calls_onCancel_only — Cancel does not reset", async () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    const user = userEvent.setup();
    render(
      <LabResetDialog
        creates={CREATES}
        onConfirm={onConfirm}
        onCancel={onCancel}
        open
      />,
    );
    await user.click(screen.getByRole("button", { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("lab_reset_dialog_confirm_calls_onConfirm_once — Confirm fires exactly once", async () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    const user = userEvent.setup();
    render(
      <LabResetDialog
        creates={CREATES}
        onConfirm={onConfirm}
        onCancel={onCancel}
        open
      />,
    );
    await user.click(screen.getByRole("button", { name: /^reset|confirm/i }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onCancel).not.toHaveBeenCalled();
  });
});
