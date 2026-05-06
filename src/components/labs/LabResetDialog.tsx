// Wave 0 stub — Phase 03.1 plan 03.1-06 implements the surgical-reset confirm
// dialog (LAB-07): shows the spec.creates[] file list and requires explicit
// confirmation before invoking lab_reset.

export interface LabResetDialogProps {
  /** Files declared in LabSpec.creates that the reset will remove. */
  creates: string[];
  /** Confirm handler — invokes lab_reset on the parent. */
  onConfirm: () => void;
  /** Cancel handler — closes the dialog without resetting. */
  onCancel: () => void;
  /** Optional open/closed control for testing. */
  open?: boolean;
}

export function LabResetDialog(_props: LabResetDialogProps) {
  return <div data-testid="lab-reset-dialog">TODO: 03.1-06</div>;
}
