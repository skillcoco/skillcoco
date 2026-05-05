import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { QuizBlock } from "@/components/learning/QuizBlock";
import type { ModuleBlock } from "@/types/learning";

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

describe("QuizBlock Phase 3 scaffolds", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("quiz_card_navigation — Next advances to question 2, Prev returns to 1", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} />);

    // FAILS in Wave 0: placeholder renders "Not implemented".
    // GREEN in 03-06 Task 1 when card navigation is implemented.
    expect(screen.getByText("What is a Pod?")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /next/i }));
    expect(screen.getByText("What controls scheduling?")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /prev/i }));
    expect(screen.getByText("What is a Pod?")).toBeInTheDocument();
  });

  it("quiz_submit_shows_review — after submit, review screen shows per-question feedback", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} />);

    // FAILS in Wave 0: placeholder renders "Not implemented".
    // GREEN in 03-06 Task 1.
    // Select answers and submit
    await user.click(screen.getByText("The smallest deployable unit"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("kube-scheduler"));
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByText("A distributed key-value store"));
    await user.click(screen.getByRole("button", { name: /submit/i }));

    expect(screen.getByText(/review/i)).toBeInTheDocument();
    expect(screen.getByText("A Pod is the smallest deployable unit in Kubernetes.")).toBeInTheDocument();
  });

  it("quiz_retake_reshuffles_options — after fail retake, option order changes", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} />);

    // FAILS in Wave 0: placeholder renders "Not implemented".
    // GREEN in 03-06 Task 1 when Fisher-Yates reshuffle is implemented.
    await user.click(screen.getByRole("button", { name: /submit/i }));
    const optionsBefore = screen
      .getAllByRole("option")
      .map((el) => el.textContent ?? "");

    await user.click(screen.getByRole("button", { name: /retake/i }));
    const optionsAfter = screen
      .getAllByRole("option")
      .map((el) => el.textContent ?? "");

    // At least one option should be in a different position after shuffle
    expect(optionsBefore.join(",")).not.toBe(optionsAfter.join(","));
  });

  it("quiz_flag_for_review — toggle flag on q2, navigate away and back, flag persists", async () => {
    const user = userEvent.setup();
    render(<QuizBlock block={mockBlock} />);

    // FAILS in Wave 0: placeholder renders "Not implemented".
    // GREEN in 03-06 Task 1 when flag-for-review is implemented.
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.click(screen.getByRole("button", { name: /flag for review/i }));

    // Navigate away and back
    await user.click(screen.getByRole("button", { name: /prev/i }));
    await user.click(screen.getByRole("button", { name: /next/i }));

    expect(screen.getByRole("button", { name: /flag for review/i })).toHaveAttribute("aria-pressed", "true");
  });
});
