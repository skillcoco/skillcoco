/**
 * TutorSidebar Phase 3 tests — Wave 4 (03-07 Task 2)
 *
 * Tests that TutorSidebar reads currentLessonId from useLearningStore
 * and forwards it as currentSectionId in sendTutorMessage calls.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// ─── Mock Tauri IPC ───────────────────────────────────────────────────────────
vi.mock("@/lib/tauri-commands", () => ({
  sendTutorMessage: vi.fn(),
}));

// ─── Mutable store mock ───────────────────────────────────────────────────────
// Use vi.hoisted so we can change currentLessonId between tests
const mockStoreState = vi.hoisted(() => ({
  currentLessonId: "block-3" as string | null,
}));

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn((selector: (s: typeof mockStoreState) => unknown) => {
    if (typeof selector === "function") return selector(mockStoreState);
    return mockStoreState;
  }),
}));

import { TutorSidebar } from "@/components/learning/TutorSidebar";
import { sendTutorMessage } from "@/lib/tauri-commands";

// ─── Helpers ─────────────────────────────────────────────────────────────────

function renderSidebar(overrides: Partial<{
  isOpen: boolean;
  trackId: string;
  moduleId: string;
  moduleTitle: string;
}> = {}) {
  return render(
    <TutorSidebar
      isOpen={overrides.isOpen ?? true}
      onClose={vi.fn()}
      trackId={overrides.trackId ?? "track-1"}
      moduleId={overrides.moduleId ?? "mod-1"}
      moduleTitle={overrides.moduleTitle ?? "Kubernetes Pods"}
    />,
  );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

describe("TutorSidebar Phase 3 — currentSectionId wiring", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store state to default
    mockStoreState.currentLessonId = "block-3";
  });

  it("tutor_passes_current_section — sendTutorMessage called with currentSectionId from store", async () => {
    const user = userEvent.setup();
    vi.mocked(sendTutorMessage).mockResolvedValue("Great question about pods!");

    renderSidebar();

    const input = screen.getByRole("textbox");
    await user.type(input, "What is a control plane?");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => {
      expect(sendTutorMessage).toHaveBeenCalledWith(
        expect.objectContaining({
          currentSectionId: "block-3",
        }),
      );
    });
  });

  it("tutor_passes_no_section_when_unset — sendTutorMessage called without currentSectionId when store is null", async () => {
    const user = userEvent.setup();
    // Set currentLessonId to null
    mockStoreState.currentLessonId = null;
    vi.mocked(sendTutorMessage).mockResolvedValue("General module question answered!");

    renderSidebar();

    const input = screen.getByRole("textbox");
    await user.type(input, "Tell me about this module");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => {
      expect(sendTutorMessage).toHaveBeenCalled();
    });

    // currentSectionId should be undefined (not passed) when currentLessonId is null
    const callArg = vi.mocked(sendTutorMessage).mock.calls[0][0];
    expect(callArg.currentSectionId).toBeUndefined();
  });

  it("tutor_updates_when_lesson_changes — second message uses updated currentLessonId", async () => {
    const user = userEvent.setup();
    vi.mocked(sendTutorMessage).mockResolvedValue("Answer!");

    const { rerender } = renderSidebar();

    // First message with block-3
    const input = screen.getByRole("textbox");
    await user.type(input, "First question");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => {
      expect(sendTutorMessage).toHaveBeenCalledTimes(1);
    });

    expect(vi.mocked(sendTutorMessage).mock.calls[0][0]).toMatchObject({
      currentSectionId: "block-3",
    });

    // Now switch lesson to block-7
    mockStoreState.currentLessonId = "block-7";

    // Re-render with new store state
    rerender(
      <TutorSidebar
        isOpen={true}
        onClose={vi.fn()}
        trackId="track-1"
        moduleId="mod-1"
        moduleTitle="Kubernetes Pods"
      />,
    );

    // Second message should use block-7
    vi.clearAllMocks();
    vi.mocked(sendTutorMessage).mockResolvedValue("Answer for block-7!");

    const input2 = screen.getByRole("textbox");
    await user.type(input2, "Second question");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => {
      expect(sendTutorMessage).toHaveBeenCalledTimes(1);
    });

    expect(vi.mocked(sendTutorMessage).mock.calls[0][0]).toMatchObject({
      currentSectionId: "block-7",
    });
  });

  it("tutor_falls_back_to_module_overview — no currentSectionId when currentLessonId is null", async () => {
    const user = userEvent.setup();
    mockStoreState.currentLessonId = null;
    vi.mocked(sendTutorMessage).mockResolvedValue("Module overview answer!");

    renderSidebar();

    const input = screen.getByRole("textbox");
    await user.type(input, "What does this module cover?");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => {
      expect(sendTutorMessage).toHaveBeenCalled();
    });

    // Backend will fall back to module overview when no currentSectionId
    const callArg = vi.mocked(sendTutorMessage).mock.calls[0][0];
    expect(callArg.moduleId).toBe("mod-1");
    expect(callArg.currentSectionId).toBeUndefined();
  });

  it("renders correctly when isOpen is true — shows textarea and send button", () => {
    renderSidebar({ isOpen: true });

    expect(screen.getByRole("textbox")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /send/i })).toBeInTheDocument();
    expect(screen.getByText("Kubernetes Pods")).toBeInTheDocument();
  });

  it("renders hidden when isOpen is false — component still mounts but translated off screen", () => {
    renderSidebar({ isOpen: false });
    // Component is in DOM but translated off screen via CSS
    // Input is still rendered (TutorSidebar uses CSS transform, not conditional render)
    expect(screen.getByRole("textbox")).toBeInTheDocument();
  });
});
