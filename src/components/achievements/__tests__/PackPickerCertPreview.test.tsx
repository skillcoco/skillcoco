// Phase 08.2 (Cert Simplification) — PackPickerCertPreview tests.
//
// Updated for the new model (D-19):
//   - "1 completion certificate available" (not "3 certifications")
//   - "Progress milestones at 25/50/75/100%" subline
//   - Expandable rationale paragraph (replaces the 3-level list)
//   - No emojis (D-10 preserved)
//   - No IPC call (D-02 — static text)

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Spy on tauri-commands at module level — verifies no_ipc_call_made.
const tauriSpies = vi.hoisted(() => ({
  getTrackCertifications: vi.fn(),
  invoke: vi.fn(),
}));

vi.mock("@/lib/tauri-commands", () => ({
  getTrackCertifications: tauriSpies.getTrackCertifications,
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: tauriSpies.invoke,
}));

import { PackPickerCertPreview } from "@/components/achievements/PackPickerCertPreview";

describe("PackPickerCertPreview — Phase 08.2 (Cert Simplification)", () => {
  it("renders_completion_certificate_headline", () => {
    render(<PackPickerCertPreview moduleCount={12} />);
    expect(
      screen.getByText(/1 completion certificate available/i),
    ).toBeInTheDocument();
  });

  it("renders_milestones_subline", () => {
    render(<PackPickerCertPreview moduleCount={12} />);
    expect(
      screen.getByText(/progress milestones at 25\/50\/75\/100%/i),
    ).toBeInTheDocument();
  });

  it("rationale_collapsed_by_default", () => {
    render(<PackPickerCertPreview moduleCount={12} />);
    expect(
      screen.queryByTestId("pack-picker-cert-preview-rationale"),
    ).not.toBeInTheDocument();
  });

  it("expand_button_reveals_rationale_paragraph", async () => {
    const user = userEvent.setup();
    render(<PackPickerCertPreview moduleCount={12} />);

    const toggle = screen.getByRole("button", {
      name: /1 completion certificate available/i,
    });
    await user.click(toggle);

    const rationale = screen.getByTestId("pack-picker-cert-preview-rationale");
    expect(rationale).toBeInTheDocument();
    // Rationale references the 100% mastery + 0.85 avg + labs gates (D-01).
    expect(rationale.textContent ?? "").toMatch(/100% of modules/i);
    expect(rationale.textContent ?? "").toMatch(/0\.85/);
    expect(rationale.textContent ?? "").toMatch(/lab/i);
  });

  it("expand_button_is_keyboard_accessible", () => {
    render(<PackPickerCertPreview moduleCount={12} />);
    const toggle = screen.getByRole("button", {
      name: /completion certificate/i,
    });
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    // Enter key opens
    fireEvent.keyDown(toggle, { key: "Enter" });
    expect(
      screen.getByRole("button", { name: /completion certificate/i }),
    ).toHaveAttribute("aria-expanded", "true");
    expect(
      screen.getByTestId("pack-picker-cert-preview-rationale"),
    ).toBeInTheDocument();

    // Space key closes
    fireEvent.keyDown(
      screen.getByRole("button", { name: /completion certificate/i }),
      { key: " " },
    );
    expect(
      screen.getByRole("button", { name: /completion certificate/i }),
    ).toHaveAttribute("aria-expanded", "false");
  });

  it("collapse_after_expand", async () => {
    const user = userEvent.setup();
    render(<PackPickerCertPreview moduleCount={12} />);
    const toggle = screen.getByRole("button", {
      name: /completion certificate/i,
    });

    await user.click(toggle);
    expect(
      screen.getByTestId("pack-picker-cert-preview-rationale"),
    ).toBeInTheDocument();

    await user.click(toggle);
    expect(
      screen.queryByTestId("pack-picker-cert-preview-rationale"),
    ).not.toBeInTheDocument();
  });

  it("no_emoji_in_rendered_output", async () => {
    const user = userEvent.setup();
    const { container } = render(<PackPickerCertPreview moduleCount={12} />);
    // Expand so the rationale text is also in the DOM and gets scanned.
    await user.click(
      screen.getByRole("button", { name: /completion certificate/i }),
    );

    const text = container.textContent ?? "";
    const emojiRegex = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;
    expect(text).not.toMatch(emojiRegex);
  });

  it("no_ipc_call_made", async () => {
    const user = userEvent.setup();
    render(<PackPickerCertPreview moduleCount={12} />);
    await user.click(
      screen.getByRole("button", { name: /completion certificate/i }),
    );

    expect(tauriSpies.getTrackCertifications).not.toHaveBeenCalled();
    expect(tauriSpies.invoke).not.toHaveBeenCalled();
  });
});
