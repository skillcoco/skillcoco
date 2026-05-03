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

import { assessKnowledge } from "@/lib/tauri-commands";

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
