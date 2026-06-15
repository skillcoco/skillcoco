import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { QuizBlock } from "@/components/learning/QuizBlock";
import type { ModuleBlock, SubmitQuizResult } from "@/types/learning";

// Mock useLearningStore — inline literals only (Vitest hoisting rule)
const mockSubmitQuiz = vi.fn();

vi.mock("@/stores/useLearningStore", () => ({
  useLearningStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    const state = {
      submitQuiz: mockSubmitQuiz,
      // moduleProgress: empty by default — tests that exercise the
      // already-passed gate override this in their own mock.
      moduleProgress: [],
    };
    return typeof selector === "function" ? selector(state) : state;
  }),
}));

const mockQuizPayload = {
  questions: [
    {
      id: "q1",
      stem: "What is a Pod?",
      options: [
        { id: "o1", text: "A group of nodes" },
        { id: "o2", text: "The smallest deployable unit" },
        { id: "o3", text: "A network policy" },
        { id: "o4", text: "A storage volume" },
      ],
      correctOptionId: "o2",
      explanation: "A Pod is the smallest deployable unit in Kubernetes.",
    },
    {
      id: "q2",
      stem: "What controls scheduling?",
      options: [
        { id: "o1", text: "kube-scheduler" },
        { id: "o2", text: "kubelet" },
        { id: "o3", text: "etcd" },
        { id: "o4", text: "kube-proxy" },
      ],
      correctOptionId: "o1",
      explanation: "kube-scheduler assigns pods to nodes.",
    },
    {
      id: "q3",
      stem: "What is etcd?",
      options: [
        { id: "o1", text: "A container runtime" },
        { id: "o2", text: "A load balancer" },
        { id: "o3", text: "A distributed key-value store" },
        { id: "o4", text: "A node agent" },
      ],
      correctOptionId: "o3",
      explanation: "etcd stores all cluster state as key-value pairs.",
    },
  ],
};

const mockBlock: ModuleBlock = {
  id: "blk-quiz-1",
  moduleId: "mod-1",
  ordering: 9,
  blockType: "quiz",
  status: "ready",
  paramsJson: '{"question_count":3}',
  payloadJson: JSON.stringify(mockQuizPayload),
  sourceAnchorsJson: "[]",
  metadataJson: '{"concept_id":null}',
  retryCount: 0,
  createdAt: "2026-05-05T00:00:00Z",
  updatedAt: "2026-05-05T00:00:00Z",
};

const mockPassedResult: SubmitQuizResult = {
  scorePercent: 100,
  passed: true,
  masteryLevel: 0.9,
  moduleCompleted: true,
  newlyUnlockedModuleIds: [],
  cardsCreated: 0,
  review: [
    {
      questionId: "q1",
      stem: "What is a Pod?",
      learnerOptionId: "o2",
      correctOptionId: "o2",
      isCorrect: true,
      explanation: "A Pod is the smallest deployable unit in Kubernetes.",
    },
    {
      questionId: "q2",
      stem: "What controls scheduling?",
      learnerOptionId: "o1",
      correctOptionId: "o1",
      isCorrect: true,
      explanation: "kube-scheduler assigns pods to nodes.",
    },
    {
      questionId: "q3",
      stem: "What is etcd?",
      learnerOptionId: "o3",
      correctOptionId: "o3",
      isCorrect: true,
      explanation: "etcd stores all cluster state as key-value pairs.",
    },
  ],
};

const mockFailedResult: SubmitQuizResult = {
  scorePercent: 0,
  passed: false,
  masteryLevel: 0.3,
  moduleCompleted: false,
  newlyUnlockedModuleIds: [],
  cardsCreated: 0,
  review: [
    {
      questionId: "q1",
      stem: "What is a Pod?",
      learnerOptionId: "o1",
      correctOptionId: "o2",
      isCorrect: false,
      explanation: "A Pod is the smallest deployable unit in Kubernetes.",
    },
    {
      questionId: "q2",
      stem: "What controls scheduling?",
      learnerOptionId: "o2",
      correctOptionId: "o1",
      isCorrect: false,
      explanation: "kube-scheduler assigns pods to nodes.",
    },
    {
      questionId: "q3",
      stem: "What is etcd?",
      learnerOptionId: "o1",
      correctOptionId: "o3",
      isCorrect: false,
      explanation: "etcd stores all cluster state as key-value pairs.",
    },
  ],
};

describe("QuizBlock Phase 3 scaffolds", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSubmitQuiz.mockResolvedValue(mockPassedResult);
  });

  it("quiz_card_navigation — Next advances to question 2, Prev returns to 1", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    expect(screen.getByText("What is a Pod?")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /next/i }));
    expect(screen.getByText("What controls scheduling?")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /prev/i }));
    expect(screen.getByText("What is a Pod?")).toBeInTheDocument();
  });

  it("quiz_submit_shows_review — after submit, review screen shows per-question feedback", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    // Select answers and submit
    await user.click(screen.getByText("The smallest deployable unit"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("kube-scheduler"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("A distributed key-value store"));
    await user.click(screen.getByRole("button", { name: /submit/i }));

    // Review screen should appear with passed badge and explanation
    expect(screen.getByTestId("quiz-review")).toBeInTheDocument();
    expect(screen.getByText("A Pod is the smallest deployable unit in Kubernetes.")).toBeInTheDocument();
  });

  it("quiz_retake_reshuffles_options — after fail retake, option order changes", async () => {
    const user = userEvent.setup();
    mockSubmitQuiz.mockResolvedValue(mockFailedResult);
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    // Capture original option order before answering
    const optionsBefore = screen
      .getAllByRole("option")
      .map((el) => el.textContent ?? "");

    // Answer all questions (wrong answers) then submit
    await user.click(screen.getByText("A group of nodes")); // wrong answer for q1
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("kubelet")); // wrong answer for q2
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("A container runtime")); // wrong answer for q3
    await user.click(screen.getByRole("button", { name: /submit/i }));

    const retakeBtn = await screen.findByTestId("retake-btn");

    // Stub Math.random to produce a different shuffle on retake
    const randomStub = vi.spyOn(Math, "random").mockReturnValue(0.99);
    await user.click(retakeBtn);
    randomStub.mockRestore();

    // After retake, quiz card is shown again with reshuffled options
    const optionsAfter = screen
      .getAllByRole("option")
      .map((el) => el.textContent ?? "");

    // At least one option should be in a different position after shuffle
    expect(optionsBefore.join(",")).not.toBe(optionsAfter.join(","));
  });

  it("quiz_flag_for_review — toggle flag on q2, navigate away and back, flag persists", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByRole("button", { name: /flag for review/i }));

    // Navigate away and back
    await user.click(screen.getByRole("button", { name: /prev/i }));
    await user.click(screen.getByRole("button", { name: /next/i }));

    expect(screen.getByRole("button", { name: /flag for review/i })).toHaveAttribute("aria-pressed", "true");
  });

  it("quiz_displays_x_of_n_progress — shows '1 / 3' on first card, '2 / 3' after Next", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    expect(screen.getByTestId("quiz-progress")).toHaveTextContent("1 / 3");

    await user.click(screen.getByRole("button", { name: /next/i }));
    expect(screen.getByTestId("quiz-progress")).toHaveTextContent("2 / 3");
  });

  it("quiz_per_question_feedback_correct — picking the right answer shows correct + explanation immediately", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    // Q1 correct = o2 ("The smallest deployable unit")
    await user.click(screen.getByText("The smallest deployable unit"));

    // Feedback panel must surface immediately
    const fb = await screen.findByTestId("answer-feedback");
    expect(fb).toBeInTheDocument();
    expect(fb).toHaveTextContent(/correct/i);
    // Explanation rendered
    expect(fb).toHaveTextContent("A Pod is the smallest deployable unit in Kubernetes.");
  });

  it("quiz_per_question_feedback_incorrect — picking the wrong answer reveals correct option + explanation", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    // o1 is wrong on Q1
    await user.click(screen.getByText("A group of nodes"));

    const fb = await screen.findByTestId("answer-feedback");
    expect(fb).toHaveTextContent(/incorrect/i);
    // Correct option text surfaced in the panel
    expect(fb).toHaveTextContent("The smallest deployable unit");
    expect(fb).toHaveTextContent("A Pod is the smallest deployable unit in Kubernetes.");
  });

  it("quiz_answer_locks_after_reveal — clicking another option after reveal does NOT change the recorded answer", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    // First pick (wrong = o1)
    await user.click(screen.getByTestId("option-o1"));
    await screen.findByTestId("answer-feedback");

    // Try to click the correct option (o2) — must be ignored (locked button)
    const correctButton = screen.getByTestId("option-o2");
    expect(correctButton).toBeDisabled();
    await user.click(correctButton);

    // Feedback still says incorrect (locked to first selection)
    const fb = screen.getByTestId("answer-feedback");
    expect(fb).toHaveTextContent(/incorrect/i);
  });

  it("quiz_submit_disabled_until_all_answered — Submit disabled with unanswered questions", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    // Answer only Q1 and navigate to Q3 without answering Q2
    await user.click(screen.getByText("The smallest deployable unit"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    // Skip Q2 - navigate to Q3
    await user.click(screen.getByRole("button", { name: /next/i }));

    // Submit should be disabled
    const submitBtn = screen.getByRole("button", { name: /submit/i });
    expect(submitBtn).toBeDisabled();
  });

  it("quiz_passing_result_shows_passed_badge — passed=true shows Passed badge", async () => {
    const user = userEvent.setup();
    mockSubmitQuiz.mockResolvedValue(mockPassedResult);
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    // Answer all questions
    await user.click(screen.getByText("The smallest deployable unit"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("kube-scheduler"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("A distributed key-value store"));
    await user.click(screen.getByRole("button", { name: /submit/i }));

    expect(await screen.findByTestId("passed-badge")).toBeInTheDocument();
    expect(screen.getByText(/100%/)).toBeInTheDocument();
  });

  it("quiz_failing_result_shows_retake_cta — passed=false shows Retake button", async () => {
    const user = userEvent.setup();
    mockSubmitQuiz.mockResolvedValue(mockFailedResult);
    render(<QuizBlock block={mockBlock} moduleId="mod-1" trackId="track-1" />);

    // Answer all questions (wrong)
    await user.click(screen.getByText("A group of nodes"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("kubelet"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("A container runtime"));
    await user.click(screen.getByRole("button", { name: /submit/i }));

    expect(await screen.findByTestId("retake-btn")).toBeInTheDocument();
    expect(screen.getByTestId("failed-badge")).toBeInTheDocument();
  });

  it("quiz_empty_questions_renders_error — zero questions shows error, no Submit", () => {
    const emptyBlock: ModuleBlock = {
      ...mockBlock,
      payloadJson: JSON.stringify({ questions: [] }),
    };
    render(<QuizBlock block={emptyBlock} moduleId="mod-1" trackId="track-1" />);

    expect(screen.getByTestId("quiz-empty")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /submit/i })).not.toBeInTheDocument();
  });

  // ── Phase 4 Wave 4 (04-05 Task 1) — optional onComplete prop (D-08 engagement-driven) ──

  it("quiz_on_complete_fires_after_submit_passed — onComplete fires when submit resolves (passed)", async () => {
    const user = userEvent.setup();
    const onComplete = vi.fn();
    mockSubmitQuiz.mockResolvedValue(mockPassedResult);
    render(
      <QuizBlock
        block={mockBlock}
        moduleId="mod-1"
        trackId="track-1"
        onComplete={onComplete}
      />,
    );

    // Answer all questions correctly
    await user.click(screen.getByText("The smallest deployable unit"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("kube-scheduler"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("A distributed key-value store"));
    await user.click(screen.getByRole("button", { name: /submit/i }));

    expect(await screen.findByTestId("passed-badge")).toBeInTheDocument();
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it("quiz_on_complete_fires_after_submit_failed — onComplete fires regardless of pass/fail (D-08)", async () => {
    const user = userEvent.setup();
    const onComplete = vi.fn();
    mockSubmitQuiz.mockResolvedValue(mockFailedResult);
    render(
      <QuizBlock
        block={mockBlock}
        moduleId="mod-1"
        trackId="track-1"
        onComplete={onComplete}
      />,
    );

    // Answer all questions wrong but still submit
    await user.click(screen.getByText("A group of nodes"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("kubelet"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("A container runtime"));
    await user.click(screen.getByRole("button", { name: /submit/i }));

    expect(await screen.findByTestId("failed-badge")).toBeInTheDocument();
    // D-08: engagement-driven completion — failed quiz still completes daily.
    expect(onComplete).toHaveBeenCalledTimes(1);
  });
});
