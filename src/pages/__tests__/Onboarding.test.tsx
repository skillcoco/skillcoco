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

  it("navigates to assessment step after setting a goal", async () => {
    const user = userEvent.setup();
    vi.mocked(assessKnowledge).mockResolvedValue("What do you already know about Kubernetes?");

    renderOnboarding();

    // Topic step
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Kubernetes");
    await user.click(screen.getByText("DevOps & Infrastructure"));
    await user.click(screen.getByRole("button", { name: /continue/i }));

    // Goals step
    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn fundamentals");
    await user.click(screen.getByRole("button", { name: /start assessment/i }));

    // Assessment step
    await waitFor(() => {
      expect(screen.getByText(/assess your current knowledge/i)).toBeInTheDocument();
    });
  });

  it("shows skip assessment button on the assessment step", async () => {
    const user = userEvent.setup();
    vi.mocked(assessKnowledge).mockResolvedValue("Tell me what you know.");

    renderOnboarding();

    // Navigate to assessment
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Kubernetes");
    await user.click(screen.getByText("DevOps & Infrastructure"));
    await user.click(screen.getByRole("button", { name: /continue/i }));
    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn fundamentals");
    await user.click(screen.getByRole("button", { name: /start assessment/i }));

    await waitFor(() => {
      expect(screen.getByText(/skip assessment/i)).toBeInTheDocument();
    });
  });

  it("marks assessment complete after clicking skip", async () => {
    const user = userEvent.setup();
    vi.mocked(assessKnowledge).mockResolvedValue("Tell me what you know.");

    renderOnboarding();

    // Navigate to assessment
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Kubernetes");
    await user.click(screen.getByText("DevOps & Infrastructure"));
    await user.click(screen.getByRole("button", { name: /continue/i }));
    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn fundamentals");
    await user.click(screen.getByRole("button", { name: /start assessment/i }));

    await waitFor(() => {
      expect(screen.getByText(/skip assessment/i)).toBeInTheDocument();
    });

    await user.click(screen.getByText(/skip assessment/i));

    await waitFor(() => {
      expect(screen.getByText(/assessment complete/i)).toBeInTheDocument();
      expect(screen.getByText(/generate my learning path/i)).toBeInTheDocument();
    });
  });
});
