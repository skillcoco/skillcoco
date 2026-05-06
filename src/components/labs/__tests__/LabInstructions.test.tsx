// Wave 0 failing tests for LabInstructions (LAB-10): renders all steps
// in order with active-step highlight + completion checkmark + manual
// per-step "Show hint" button.

import { describe, it, expect, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";

import { LabInstructions } from "@/components/labs/LabInstructions";
import type { LabSpec } from "@/types/learning";

function makeSpec(): LabSpec {
  return {
    slug: "pod-inspect",
    title: "Inspect a Pod",
    requiresDocker: true,
    image: "kindest/node:v1.30",
    creates: ["deploy.yaml"],
    steps: [
      {
        id: "s1",
        title: "List pods",
        prompt: "Run kubectl get pods",
        check: { kind: "command_regex", pattern: "Running" },
        hints: ["a", "b", "c"],
      },
      {
        id: "s2",
        title: "Describe a pod",
        prompt: "Run kubectl describe pod ...",
        check: { kind: "exit_code", expected: 0 },
        hints: ["a", "b", "c"],
      },
      {
        id: "s3",
        title: "Inspect logs",
        prompt: "Run kubectl logs ...",
        check: { kind: "ai_judge", criteria: "explains the output" },
        hints: ["a", "b", "c"],
      },
    ],
  };
}

describe("LabInstructions — Phase 03.1 Wave 0 (failing scaffolds)", () => {
  it("lab_instructions_renders_all_steps_in_order — every spec.steps title is on screen", () => {
    // FAILS today — stub renders a single placeholder div.
    render(
      <LabInstructions spec={makeSpec()} currentStep={0} completedStepIds={[]} />,
    );
    expect(screen.getByText("List pods")).toBeInTheDocument();
    expect(screen.getByText("Describe a pod")).toBeInTheDocument();
    expect(screen.getByText("Inspect logs")).toBeInTheDocument();
  });

  it("lab_instructions_active_step_marked — currentStep gets data-active=\"true\"", () => {
    render(
      <LabInstructions spec={makeSpec()} currentStep={1} completedStepIds={[]} />,
    );
    const step = screen.getByTestId("lab-step-1");
    expect(step.getAttribute("data-active")).toBe("true");
  });

  it("lab_instructions_completed_steps_have_checkmark — completedStepIds get data-completed=\"true\"", () => {
    render(
      <LabInstructions
        spec={makeSpec()}
        currentStep={2}
        completedStepIds={["s1", "s2"]}
      />,
    );
    expect(screen.getByTestId("lab-step-0").getAttribute("data-completed")).toBe("true");
    expect(screen.getByTestId("lab-step-1").getAttribute("data-completed")).toBe("true");
    expect(screen.getByTestId("lab-step-2").getAttribute("data-completed")).toBe("false");
  });

  it("lab_instructions_show_hint_button_per_step — each step exposes Show hint", () => {
    const onShowHint = vi.fn();
    render(
      <LabInstructions
        spec={makeSpec()}
        currentStep={0}
        completedStepIds={[]}
        onShowHint={onShowHint}
      />,
    );
    const step1 = screen.getByTestId("lab-step-0");
    const hintButton = within(step1).getByRole("button", { name: /show hint/i });
    expect(hintButton).toBeInTheDocument();
  });
});
