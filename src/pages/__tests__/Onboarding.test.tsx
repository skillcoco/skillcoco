import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { Onboarding } from "@/pages/Onboarding";
import type { TopicPack } from "@/types/topic-packs";

// Mock tauri commands
vi.mock("@/lib/tauri-commands", () => ({
  createTrack: vi.fn(),
  assessKnowledge: vi.fn(),
  generateLearningPath: vi.fn(),
  listTopicPacks: vi.fn(),
}));

import {
  createTrack,
  assessKnowledge,
  generateLearningPath,
  listTopicPacks,
} from "@/lib/tauri-commands";

const createTrackMock = vi.mocked(createTrack);
const assessKnowledgeMock = vi.mocked(assessKnowledge);
const generateLearningPathMock = vi.mocked(generateLearningPath);
const listTopicPacksMock = vi.mocked(listTopicPacks);

// Mock useNavigate
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

function makePack(id: string, source: "bundled" | "skill"): TopicPack {
  return {
    pack: {
      id,
      title: `${id} title`,
      description: `description for ${id}`,
      domain_module: "devops",
      estimated_hours: 8,
      pack_version: "1.0",
      requires_docker: false,
      modules: [],
      edges: [],
    },
    source,
    enabled: true,
    validationStatus: "ok",
    validationMessages: [],
    lastLoadedAt: "2026-06-15T00:00:00Z",
  };
}

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
    // Sensible defaults that any test can override before calling render.
    listTopicPacksMock.mockResolvedValue([]);
    createTrackMock.mockResolvedValue({
      id: "trk-xyz",
      learnerId: "lp-1",
      topic: "Whatever",
      domainModule: "devops",
      status: "onboarding",
      goal: "",
      currentModuleId: null,
      progressPercent: 0,
      totalTimeSpent: 0,
      createdAt: "2026-06-15T00:00:00Z",
      updatedAt: "2026-06-15T00:00:00Z",
    });
    assessKnowledgeMock.mockResolvedValue(
      JSON.stringify({ level: "beginner", gaps: [], strengths: [] }),
    );
    generateLearningPathMock.mockResolvedValue({} as never);
  });

  it("renders the pack-picker step on initial load", async () => {
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
    ]);
    renderOnboarding();

    expect(
      screen.getByText(/pick a topic pack or describe what you want to learn/i),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText(/topic packs/i)).toBeInTheDocument();
      expect(screen.getByText(/my skills/i)).toBeInTheDocument();
    });
  });

  it("free-text fallback collapsible navigates to goals step", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    await waitFor(() => {
      expect(screen.getByTestId("custom-topic-toggle")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("custom-topic-toggle"));
    await user.type(
      screen.getByPlaceholderText(/kubernetes/i),
      "Distributed systems",
    );
    await user.click(screen.getByText(/concepts & theory/i));
    await user.click(screen.getByTestId("custom-topic-submit"));

    expect(screen.getByText(/set your learning goals/i)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/pass the cka/i)).toBeInTheDocument();
  });

  it("navigates back from goals to pack-picker step", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    // Open fallback → submit → goals
    await waitFor(() => screen.getByTestId("custom-topic-toggle"));
    await user.click(screen.getByTestId("custom-topic-toggle"));
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Rust");
    await user.click(screen.getByText(/programming language/i));
    await user.click(screen.getByTestId("custom-topic-submit"));

    // Back
    await user.click(screen.getByRole("button", { name: /back/i }));

    expect(
      screen.getByText(/pick a topic pack or describe what you want to learn/i),
    ).toBeInTheDocument();
  });

  it("navigates to level-selection step after setting a goal", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    await waitFor(() => screen.getByTestId("custom-topic-toggle"));
    await user.click(screen.getByTestId("custom-topic-toggle"));
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Kubernetes");
    await user.click(screen.getByText(/devops & infrastructure/i));
    await user.click(screen.getByTestId("custom-topic-submit"));

    // Goals
    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn fundamentals");
    await user.click(screen.getByRole("button", { name: /continue/i }));

    await waitFor(() => {
      expect(screen.getByText(/rate your experience level/i)).toBeInTheDocument();
    });
  });

  it("shows level selection options on the assessment step", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    await waitFor(() => screen.getByTestId("custom-topic-toggle"));
    await user.click(screen.getByTestId("custom-topic-toggle"));
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Kubernetes");
    await user.click(screen.getByText(/devops & infrastructure/i));
    await user.click(screen.getByTestId("custom-topic-submit"));

    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn fundamentals");
    await user.click(screen.getByRole("button", { name: /continue/i }));

    await waitFor(() => {
      expect(screen.getByText("Beginner")).toBeInTheDocument();
      expect(screen.getByText("Intermediate")).toBeInTheDocument();
      expect(screen.getByText("Advanced")).toBeInTheDocument();
    });
  });

  it("enables Generate Path button after selecting a level", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    await waitFor(() => screen.getByTestId("custom-topic-toggle"));
    await user.click(screen.getByTestId("custom-topic-toggle"));
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Kubernetes");
    await user.click(screen.getByText(/devops & infrastructure/i));
    await user.click(screen.getByTestId("custom-topic-submit"));

    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn fundamentals");
    await user.click(screen.getByRole("button", { name: /continue/i }));

    await waitFor(() => {
      expect(screen.getByText("Beginner")).toBeInTheDocument();
    });

    await user.click(screen.getByText("Beginner"));
    const generateBtn = screen.getByRole("button", { name: /create learning path/i });
    expect(generateBtn).not.toBeDisabled();
  });
});

/**
 * Plan 05-05 Wave 4 — Wave 0 RED scaffolds turn GREEN here. Tests verify
 * the picker structure, the collapsible fallback, and packId flowing
 * through `generateLearningPath`.
 */
describe("Onboarding pack picker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listTopicPacksMock.mockResolvedValue([]);
    createTrackMock.mockResolvedValue({
      id: "trk-xyz",
      learnerId: "lp-1",
      topic: "x",
      domainModule: "devops",
      status: "onboarding",
      goal: "",
      currentModuleId: null,
      progressPercent: 0,
      totalTimeSpent: 0,
      createdAt: "2026-06-15T00:00:00Z",
      updatedAt: "2026-06-15T00:00:00Z",
    });
    assessKnowledgeMock.mockResolvedValue(
      JSON.stringify({ level: "beginner", gaps: [], strengths: [] }),
    );
    generateLearningPathMock.mockResolvedValue({} as never);
  });

  it("step 2 shows Topic Packs and My Skills sections", async () => {
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
      makePack("rust-from-zero", "bundled"),
      makePack("my-custom-skill", "skill"),
    ]);
    renderOnboarding();

    await waitFor(() => {
      expect(screen.getByText(/topic packs/i)).toBeInTheDocument();
      expect(screen.getByText(/my skills/i)).toBeInTheDocument();
      expect(
        screen.getByTestId("pack-card-kubernetes-fundamentals"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("pack-card-my-custom-skill"),
      ).toBeInTheDocument();
    });
  });

  it("collapsible 'Or describe your own' fallback exists at bottom", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    await waitFor(() => {
      expect(screen.getByTestId("custom-topic-toggle")).toBeInTheDocument();
    });
    expect(screen.getByText(/or describe your own/i)).toBeInTheDocument();
    // Form is collapsed by default — the free-text input is absent.
    expect(screen.queryByPlaceholderText(/kubernetes/i)).not.toBeInTheDocument();
  });

  it("picking a pack flows packId into generateLearningPath", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([
      makePack("agentic-devops", "bundled"),
    ]);
    renderOnboarding();

    // Pick the pack
    await waitFor(() => screen.getByTestId("pack-card-agentic-devops"));
    await user.click(screen.getByTestId("pack-card-agentic-devops"));

    // Goal
    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Master DevOps");
    await user.click(screen.getByRole("button", { name: /continue/i }));

    // Level
    await waitFor(() => screen.getByText("Beginner"));
    await user.click(screen.getByText("Beginner"));
    await user.click(screen.getByRole("button", { name: /create learning path/i }));

    await waitFor(() => {
      expect(generateLearningPathMock).toHaveBeenCalledTimes(1);
    });
    const call = generateLearningPathMock.mock.calls[0][0];
    expect(call.packId).toBe("agentic-devops");
    expect(call.topic).toBe("agentic-devops title");
    expect(call.domain).toBe("devops");
  });

  it("free-text path leaves packId undefined in generateLearningPath call", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    await waitFor(() => screen.getByTestId("custom-topic-toggle"));
    await user.click(screen.getByTestId("custom-topic-toggle"));
    await user.type(screen.getByPlaceholderText(/kubernetes/i), "Distributed systems");
    await user.click(screen.getByText(/concepts & theory/i));
    await user.click(screen.getByTestId("custom-topic-submit"));

    await user.type(screen.getByPlaceholderText(/pass the cka/i), "Learn the patterns");
    await user.click(screen.getByRole("button", { name: /continue/i }));

    await waitFor(() => screen.getByText("Beginner"));
    await user.click(screen.getByText("Beginner"));
    await user.click(screen.getByRole("button", { name: /create learning path/i }));

    await waitFor(() => {
      expect(generateLearningPathMock).toHaveBeenCalledTimes(1);
    });
    const call = generateLearningPathMock.mock.calls[0][0];
    expect(call.packId).toBeUndefined();
    expect(call.topic).toBe("Distributed systems");
    expect(call.domain).toBe("concepts");
  });
});
