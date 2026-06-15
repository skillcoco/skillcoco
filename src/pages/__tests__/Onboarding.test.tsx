import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { Onboarding } from "@/pages/Onboarding";

// Mock tauri commands
vi.mock("@/lib/tauri-commands", () => ({
  createTrack: vi.fn(),
  assessKnowledge: vi.fn(),
  generateLearningPath: vi.fn(),
}));

// Mock useNavigate
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});


function renderOnboarding() {
  return render(
    <MemoryRouter>
      <Onboarding />
    </MemoryRouter>,
  );
}

describe("Onboarding", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the topic step on initial load", () => {
    renderOnboarding();

    expect(screen.getByText("Tell us what you want to learn")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(/kubernetes/i),
    ).toBeInTheDocument();
    expect(screen.getByText("Programming Language")).toBeInTheDocument();
    expect(screen.getByText("DevOps & Infrastructure")).toBeInTheDocument();
  });

  it("disables Continue when no topic or domain is selected", () => {
    renderOnboarding();

    const continueBtn = screen.getByRole("button", { name: /continue/i });
    expect(continueBtn).toBeDisabled();
  });

  it("enables Continue after entering a topic and selecting a domain", async () => {
    const user = userEvent.setup();
    renderOnboarding();

    const input = screen.getByPlaceholderText(/kubernetes/i);
    await user.type(input, "Kubernetes");

    const domainBtn = screen.getByText("DevOps & Infrastructure");
    await user.click(domainBtn);

    const continueBtn = screen.getByRole("button", { name: /continue/i });
    expect(continueBtn).toBeEnabled();
  });

  it("navigates to goals step when Continue is clicked", async () => {
    const user = userEvent.setup();
    renderOnboarding();

    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Rust");
    await user.click(screen.getByText("Programming Language"));
    await user.click(screen.getByRole("button", { name: /continue/i }));

    expect(screen.getByText(/set your learning goals/i)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/pass the cka/i)).toBeInTheDocument();
  });

  it("navigates back from goals to topic step", async () => {
    const user = userEvent.setup();
    renderOnboarding();

    // Go to goals
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Rust");
    await user.click(screen.getByText("Programming Language"));
    await user.click(screen.getByRole("button", { name: /continue/i }));

    // Go back
    await user.click(screen.getByRole("button", { name: /back/i }));

    expect(screen.getByText("Tell us what you want to learn")).toBeInTheDocument();
  });

  it("navigates to level-selection step after setting a goal", async () => {
    // Quick task 1 replaced Socratic assessment with self-rating level picker.
    // The goals step button now says "Continue" (not "Start Assessment").
    const user = userEvent.setup();

    renderOnboarding();

    // Topic step
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Kubernetes");
    await user.click(screen.getByText("DevOps & Infrastructure"));
    await user.click(screen.getByRole("button", { name: /continue/i }));

    // Goals step — button now says "Continue", leads to level-selection ("assessment")
    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn fundamentals");
    await user.click(screen.getByRole("button", { name: /continue/i }));

    // Level selection step
    await waitFor(() => {
      expect(screen.getByText(/rate your experience level/i)).toBeInTheDocument();
    });
  });

  it("shows level selection options on the assessment step", async () => {
    // Quick task 1: level picker shows Beginner / Intermediate / Advanced cards
    const user = userEvent.setup();

    renderOnboarding();

    // Navigate to level selection
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Kubernetes");
    await user.click(screen.getByText("DevOps & Infrastructure"));
    await user.click(screen.getByRole("button", { name: /continue/i }));
    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn fundamentals");
    await user.click(screen.getByRole("button", { name: /continue/i }));

    await waitFor(() => {
      expect(screen.getByText("Beginner")).toBeInTheDocument();
      expect(screen.getByText("Intermediate")).toBeInTheDocument();
      expect(screen.getByText("Advanced")).toBeInTheDocument();
    });
  });

  it("enables Generate Path button after selecting a level", async () => {
    // Quick task 1: selecting a level card enables the Generate Path button
    const user = userEvent.setup();

    renderOnboarding();

    // Navigate to level selection
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Kubernetes");
    await user.click(screen.getByText("DevOps & Infrastructure"));
    await user.click(screen.getByRole("button", { name: /continue/i }));
    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn fundamentals");
    await user.click(screen.getByRole("button", { name: /continue/i }));

    await waitFor(() => {
      expect(screen.getByText("Beginner")).toBeInTheDocument();
    });

    // Select Beginner level
    await user.click(screen.getByText("Beginner"));

    // "Create Learning Path" button should now be enabled
    const generateBtn = screen.getByRole("button", { name: /create learning path/i });
    expect(generateBtn).not.toBeDisabled();
  });
});

/**
 * Plan 05-01 Wave 0 — RED scaffolds for the Topic Packs + My Skills picker
 * (D-08). Wave 4 (Plan 05-05) MUST turn these GREEN by:
 *
 *   - Replacing the free-text topic step with a two-section picker
 *     ("Topic Packs" + "My Skills") sourced from `list_topic_packs` IPC.
 *   - Keeping a collapsible "Or describe your own" fallback at the bottom.
 *   - Threading the selected `packId` into `createTrack` and
 *     `generateLearningPath` so PagePlanner receives pack-curated modules.
 */
describe("Onboarding pack picker (Wave 0 RED scaffold)", () => {
  it("step 2 shows Topic Packs and My Skills sections", () => {
    expect.fail(
      "Wave 4 (Plan 05-05) must add the two-section pack picker to Onboarding step 2 — see plan 05-05",
    );
  });

  it("collapsible 'Or describe your own' fallback exists at bottom", () => {
    expect.fail(
      "Wave 4 (Plan 05-05) must keep the free-text fallback as a collapsible at the bottom of the picker — see plan 05-05",
    );
  });

  it("picking a pack flows packId into createTrack/generateLearningPath", () => {
    expect.fail(
      "Wave 4 (Plan 05-05) must thread packId through createTrack and generateLearningPath — see plan 05-05",
    );
  });
});
