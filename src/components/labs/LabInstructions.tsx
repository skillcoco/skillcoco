// Wave 0 stub — Phase 03.1 plan 03.1-06 renders the ordered all-visible step
// list with active-step highlight and per-step "Show hint" button.

import type { LabSpec } from "@/types/learning";

export interface LabInstructionsProps {
  spec: LabSpec;
  /** 0-based index of the currently-active step. */
  currentStep: number;
  /** Step ids that have been marked complete. */
  completedStepIds: string[];
  /** Manual hint reveal handler — component-state controlled per RESEARCH q7. */
  onShowHint?: (stepId: string) => void;
}

export function LabInstructions(_props: LabInstructionsProps) {
  return <div data-testid="lab-instructions">TODO: 03.1-06</div>;
}
