// Phase 6 (Certification) — Plan 06-05 (Wave 4) PackPickerCertPreview tests.
//
// Per D-10 + CERT-10: static "3 certifications available" preview on each
// PackPicker tile. Expandable to show Associate / Practitioner /
// Professional names + criteria from D-02 hardcoded constants. NO IPC
// call (D-02 thresholds are uniform; criteria text is constant). No
// emojis (D-10 explicit no-emoji clause).

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

describe("PackPickerCertPreview — Phase 6 Plan 06-05 (Wave 4)", () => {
  it("renders_static_count_text", () => {
    render(<PackPickerCertPreview moduleCount={12} />);
    expect(screen.getByText(/3 certifications available/i)).toBeInTheDocument();
  });

  it("collapsed_by_default", () => {
    render(<PackPickerCertPreview moduleCount={12} />);
    expect(screen.queryByText(/master 25% of modules/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/master 60% of modules/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/master 100% of modules/i)).not.toBeInTheDocument();
  });

  it("expand_button_reveals_three_levels_with_criteria", async () => {
    const user = userEvent.setup();
    render(<PackPickerCertPreview moduleCount={12} />);

    const toggle = screen.getByRole("button", { name: /certifications/i });
    await user.click(toggle);

    expect(screen.getByText(/Associate/)).toBeInTheDocument();
    expect(screen.getByText(/Practitioner/)).toBeInTheDocument();
    expect(screen.getByText(/Professional/)).toBeInTheDocument();
    expect(screen.getByText(/master 25% of modules/i)).toBeInTheDocument();
    expect(screen.getByText(/master 60% of modules/i)).toBeInTheDocument();
    expect(screen.getByText(/master 100% of modules/i)).toBeInTheDocument();
  });

  it("expand_button_is_keyboard_accessible", async () => {
    render(<PackPickerCertPreview moduleCount={12} />);
    const toggle = screen.getByRole("button", { name: /certifications/i });
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    // Enter key opens
    fireEvent.keyDown(toggle, { key: "Enter" });
    // Re-query in case React rerendered the node identity
    expect(
      screen.getByRole("button", { name: /certifications/i }),
    ).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText(/master 25% of modules/i)).toBeInTheDocument();

    // Space key closes
    fireEvent.keyDown(
      screen.getByRole("button", { name: /certifications/i }),
      { key: " " },
    );
    expect(
      screen.getByRole("button", { name: /certifications/i }),
    ).toHaveAttribute("aria-expanded", "false");
  });

  it("collapse_after_expand", async () => {
    const user = userEvent.setup();
    render(<PackPickerCertPreview moduleCount={12} />);
    const toggle = screen.getByRole("button", { name: /certifications/i });

    await user.click(toggle);
    expect(screen.getByText(/master 25% of modules/i)).toBeInTheDocument();

    await user.click(toggle);
    expect(screen.queryByText(/master 25% of modules/i)).not.toBeInTheDocument();
  });

  it("no_emoji_in_rendered_output", async () => {
    const user = userEvent.setup();
    const { container } = render(<PackPickerCertPreview moduleCount={12} />);
    // Expand so the level details are also in the DOM and get scanned.
    await user.click(screen.getByRole("button", { name: /certifications/i }));

    const text = container.textContent ?? "";
    const emojiRegex = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;
    expect(text).not.toMatch(emojiRegex);
  });

  it("no_ipc_call_made", async () => {
    const user = userEvent.setup();
    render(<PackPickerCertPreview moduleCount={12} />);
    // Click expand to ensure even the on-toggle path doesn't trigger IPC.
    await user.click(screen.getByRole("button", { name: /certifications/i }));

    expect(tauriSpies.getTrackCertifications).not.toHaveBeenCalled();
    expect(tauriSpies.invoke).not.toHaveBeenCalled();
  });
});
