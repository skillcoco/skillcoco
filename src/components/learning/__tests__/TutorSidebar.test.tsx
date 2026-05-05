import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Vitest hoisting rule: inline literals only inside vi.mock factory.
vi.mock("@/lib/tauri-commands", () => ({
  sendTutorMessage: vi.fn(),
}));

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn(() => ({
    currentLessonId: "block-3",
  })),
}));

import { TutorSidebar } from "@/components/learning/TutorSidebar";
import { sendTutorMessage } from "@/lib/tauri-commands";

describe("TutorSidebar Phase 3 scaffolds", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("tutor_passes_current_section — sendTutorMessage called with currentSectionId from store", async () => {
    const user = userEvent.setup();
    vi.mocked(sendTutorMessage).mockResolvedValue("Great question about pods!");

    render(
      <TutorSidebar
        isOpen={true}
        onClose={vi.fn()}
        trackId="track-1"
        moduleId="mod-1"
        moduleTitle="Kubernetes Pods"
      />,
    );

    const input = screen.getByRole("textbox");
    await user.type(input, "What is a control plane?");
    await user.click(screen.getByRole("button", { name: /send/i }));

    // FAILS in Wave 0: TutorSidebar doesn't read currentLessonId from store yet.
    // GREEN in 03-07 Task 2 when currentSectionId is wired from useLearningStore.
    expect(sendTutorMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        currentSectionId: "block-3",
      }),
    );
  });
});
