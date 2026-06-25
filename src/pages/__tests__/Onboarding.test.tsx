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

// Phase 08.3 — Onboarding step 1 now mounts <TopicPicker /> (free-text
// primary, chip cloud below, templates collapsible). Tests below replace
// the Phase 5 PackPicker assertions (Topic Packs heading, custom-topic
// toggle, domain-selector chip click) with the topic-first surface
// equivalents. Component-level coverage of the picker itself lives in
// src/components/onboarding/__tests__/TopicPicker.test.tsx — this file
// only exercises the wizard flow that surrounds it.
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

  it("renders the topic-picker step on initial load with new wizard copy", async () => {
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
    ]);
    renderOnboarding();

    // Phase 08.3 — wizard <h1> + TopicPicker <h2> both carry the
    // 'What do you want to learn?' string by design. Scope the wizard
    // header check to level 1.
    expect(
      screen.getByRole("heading", {
        level: 1,
        name: /what do you want to learn\?/i,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/type anything, tap a topic, or pick a template/i),
    ).toBeInTheDocument();
    // Free-text input is the primary surface (D-04).
    expect(screen.getByTestId("topic-freetext-input")).toBeInTheDocument();
    // Templates collapsible exists, but bundled packs are NOT yet
    // rendered (collapsed by default per D-06).
    await waitFor(() => {
      expect(screen.getByTestId("templates-toggle")).toBeInTheDocument();
    });
    expect(
      screen.queryByTestId("pack-card-kubernetes-fundamentals"),
    ).not.toBeInTheDocument();
  });

  it("free-text submit advances to the goals step", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    await user.type(
      screen.getByTestId("topic-freetext-input"),
      "Distributed systems",
    );
    await user.click(screen.getByTestId("topic-freetext-submit"));

    expect(screen.getByText(/set your learning goals/i)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/pass the cka/i)).toBeInTheDocument();
  });

  it("Back button on goals step returns to topic-picker", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    // Free-text → goals.
    await user.type(screen.getByTestId("topic-freetext-input"), "Rust");
    await user.click(screen.getByTestId("topic-freetext-submit"));

    // Back to topic-picker.
    await user.click(screen.getByRole("button", { name: /back/i }));

    expect(
      screen.getByText(/type anything, tap a topic, or pick a template/i),
    ).toBeInTheDocument();
    // Free-text input visible again.
    expect(screen.getByTestId("topic-freetext-input")).toBeInTheDocument();
  });

  it("navigates to level-selection step after setting a goal", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    await user.type(screen.getByTestId("topic-freetext-input"), "Kubernetes");
    await user.click(screen.getByTestId("topic-freetext-submit"));

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

    await user.type(screen.getByTestId("topic-freetext-input"), "Kubernetes");
    await user.click(screen.getByTestId("topic-freetext-submit"));

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

    await user.type(screen.getByTestId("topic-freetext-input"), "Kubernetes");
    await user.click(screen.getByTestId("topic-freetext-submit"));

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
 * Phase 08.3 W2 — wizard-level coverage for the template-pick path and
 * the free-text path through generateLearningPath. The Phase 5 RED
 * scaffolds turned GREEN in 05-05 are preserved in spirit: we still
 * verify packId flows to the backend when a template is picked, and is
 * undefined for free-text. The surface that reveals templates moved to
 * the collapsible (D-06), so each test expands it before clicking a
 * pack card.
 */
describe("Onboarding template + free-text wizard flow", () => {
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

  it("expanding templates reveals bundled packs and My Skills section", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([
      makePack("kubernetes-fundamentals", "bundled"),
      makePack("rust-from-zero", "bundled"),
      makePack("my-custom-skill", "skill"),
    ]);
    renderOnboarding();

    await waitFor(() => {
      expect(screen.getByTestId("templates-toggle")).toBeInTheDocument();
    });
    await user.click(screen.getByTestId("templates-toggle"));

    expect(screen.getByText(/my skills/i)).toBeInTheDocument();
    expect(
      screen.getByTestId("pack-card-kubernetes-fundamentals"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("pack-card-my-custom-skill"),
    ).toBeInTheDocument();
  });

  it("templates collapsible is collapsed by default (D-06)", async () => {
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    await waitFor(() => {
      expect(screen.getByTestId("templates-toggle")).toBeInTheDocument();
    });
    expect(screen.getByText(/or use a curated template/i)).toBeInTheDocument();
    // Without expanding, the My Skills section is NOT rendered.
    expect(screen.queryByText(/my skills/i)).not.toBeInTheDocument();
  });

  it("picking a template flows packId into generateLearningPath", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([
      makePack("agentic-devops", "bundled"),
    ]);
    renderOnboarding();

    // Expand templates → click the pack card.
    await waitFor(() => screen.getByTestId("templates-toggle"));
    await user.click(screen.getByTestId("templates-toggle"));
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

    await user.type(
      screen.getByTestId("topic-freetext-input"),
      "Distributed systems",
    );
    await user.click(screen.getByTestId("topic-freetext-submit"));

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
    // Phase 08.3 — domain is no longer surfaced; TopicPicker passes
    // DEFAULT_DOMAIN ('general') through to the wizard.
    expect(call.domain).toBe("general");
  });

  it("chip click prefills the free-text input and Continue submits with that topic", async () => {
    const user = userEvent.setup();
    listTopicPacksMock.mockResolvedValue([]);
    renderOnboarding();

    // Click the "Spanish" chip — input should prefill.
    await user.click(screen.getByTestId("chip-spanish"));
    const input = screen.getByTestId(
      "topic-freetext-input",
    ) as HTMLInputElement;
    expect(input.value).toBe("Spanish");

    // Submit → goals step uses the chip-derived topic.
    await user.click(screen.getByTestId("topic-freetext-submit"));
    expect(screen.getByPlaceholderText(/pass the cka/i)).toBeInTheDocument();
  });
});
