// Wave 0 stub — Phase 03.1 plan 03.1-06 implements the real 60/40 split-pane
// layout with glassmorphism theming, lifecycle (openSession on mount /
// closeSession on unmount), and host-shell-fallback warning.
//
// For now this renders a placeholder div so React doesn't throw and the
// failing tests in __tests__/LabBlock.test.tsx can fail on assertion (not on
// a compile or render error).

import type { ModuleBlock } from "@/types/learning";

export interface LabBlockProps {
  block: ModuleBlock;
  /** Learner identity needed for lab_session_open IPC. */
  learnerId: string;
  /** Optional track id for workspace path resolution (LAB-07). */
  trackId?: string;
}

export function LabBlock(_props: LabBlockProps) {
  return <div data-testid="lab-block">TODO: 03.1-06</div>;
}
