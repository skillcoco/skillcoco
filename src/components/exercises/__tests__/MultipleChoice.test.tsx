import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MultipleChoice } from "../MultipleChoice";
import type { Exercise } from "@/types/exercises";

function makeMcqExercise(overrides: Partial<Exercise> = {}): Exercise {
  return {
    id: "ex-1",
    moduleId: "mod-1",
    type: "multiple_choice",
    difficulty: 5,
    prompt: "What is the smallest deployable unit in Kubernetes?",
    hints: ["Think about what Kubernetes schedules"],
    metadata: {
      options: ["Container", "Pod", "Node", "Cluster"],
      correctIndices: [1],
    },
    ...overrides,
  };
}

describe("MultipleChoice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the prompt and all options", () => {
    render(<MultipleChoice exercise={makeMcqExercise()} onComplete={vi.fn()} />);
    expect(
      screen.getByText(/smallest deployable unit/i)
    ).toBeInTheDocument();
    expect(screen.getByText("Container")).toBeInTheDocument();
    expect(screen.getByText("Pod")).toBeInTheDocument();
    expect(screen.getByText("Node")).toBeInTheDocument();
    expect(screen.getByText("Cluster")).toBeInTheDocument();
  });

  it("scores 100 and calls onComplete when correct option selected", () => {
    const onComplete = vi.fn();
    render(<MultipleChoice exercise={makeMcqExercise()} onComplete={onComplete} />);

    fireEvent.click(screen.getByText("Pod"));
    fireEvent.click(screen.getByRole("button", { name: /submit/i }));

    expect(onComplete).toHaveBeenCalledWith(100);
  });

  it("scores 0 when wrong option selected", () => {
    const onComplete = vi.fn();
    render(<MultipleChoice exercise={makeMcqExercise()} onComplete={onComplete} />);

    fireEvent.click(screen.getByText("Container"));
    fireEvent.click(screen.getByRole("button", { name: /submit/i }));

    expect(onComplete).toHaveBeenCalledWith(0);
  });

  it("disables submit when nothing is selected", () => {
    render(<MultipleChoice exercise={makeMcqExercise()} onComplete={vi.fn()} />);
    const submitBtn = screen.getByRole("button", { name: /submit/i });
    expect(submitBtn).toBeDisabled();
  });

  it("shows correct/incorrect feedback after submit", () => {
    render(<MultipleChoice exercise={makeMcqExercise()} onComplete={vi.fn()} />);
    fireEvent.click(screen.getByText("Container"));
    fireEvent.click(screen.getByRole("button", { name: /submit/i }));

    expect(screen.getByText(/incorrect/i)).toBeInTheDocument();
    // Correct answer shown after submit
    expect(screen.getByText(/correct answer/i)).toBeInTheDocument();
  });

  it("supports multi-select MCQ with multiple correct indices", () => {
    const onComplete = vi.fn();
    const ex = makeMcqExercise({
      prompt: "Which of these are Kubernetes resources? (Select all)",
      metadata: {
        options: ["Pod", "Service", "VirtualMachine", "Deployment"],
        correctIndices: [0, 1, 3],
      },
    });
    render(<MultipleChoice exercise={ex} onComplete={onComplete} />);

    fireEvent.click(screen.getByText("Pod"));
    fireEvent.click(screen.getByText("Service"));
    fireEvent.click(screen.getByText("Deployment"));
    fireEvent.click(screen.getByRole("button", { name: /submit/i }));

    expect(onComplete).toHaveBeenCalledWith(100);
  });

  it("scores 0 if multi-select misses one correct answer", () => {
    const onComplete = vi.fn();
    const ex = makeMcqExercise({
      metadata: {
        options: ["Pod", "Service", "VirtualMachine", "Deployment"],
        correctIndices: [0, 1, 3],
      },
    });
    render(<MultipleChoice exercise={ex} onComplete={onComplete} />);

    fireEvent.click(screen.getByText("Pod"));
    fireEvent.click(screen.getByText("Service"));
    fireEvent.click(screen.getByRole("button", { name: /submit/i }));

    expect(onComplete).toHaveBeenCalledWith(0);
  });

  it("locks selection after submit (cannot re-answer)", () => {
    render(<MultipleChoice exercise={makeMcqExercise()} onComplete={vi.fn()} />);
    fireEvent.click(screen.getByText("Pod"));
    fireEvent.click(screen.getByRole("button", { name: /submit/i }));

    // Submit button is hidden after submission
    expect(
      screen.queryByRole("button", { name: /submit/i })
    ).not.toBeInTheDocument();
  });

  it("renders hint button when hints are present", () => {
    render(<MultipleChoice exercise={makeMcqExercise()} onComplete={vi.fn()} />);
    expect(screen.getByText(/show.*hint/i)).toBeInTheDocument();
  });

  it("handles missing options gracefully", () => {
    const ex = makeMcqExercise({ metadata: { options: undefined, correctIndices: [0] } });
    render(<MultipleChoice exercise={ex} onComplete={vi.fn()} />);
    expect(
      screen.getByText(/no options available/i)
    ).toBeInTheDocument();
  });
});
