// Wave 0 stub — Phase 03.1 plan 03.1-06 implements the manual progressive
// 3-tier hint reveal (gentle → partial → full) per RESEARCH question #7.

export interface LabHintPanelProps {
  /** Three-tier hints from LabSpec.steps[i].hints. */
  hints: string[];
  /** Currently revealed tier (0 = none, 1..3 = revealed). */
  revealedTier: number;
  /** Click handler that advances revealedTier by one (capped at 3). */
  onShowNext: () => void;
}

export function LabHintPanel(_props: LabHintPanelProps) {
  return <div data-testid="lab-hint-panel">TODO: 03.1-06</div>;
}
