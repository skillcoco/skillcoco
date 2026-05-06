// Phase 03.1 plan 03.1-06 — surgical reset confirmation dialog (LAB-07).
//
// Lists the spec.creates[] paths that will be wiped. Cancel returns
// silently; Confirm fires onConfirm exactly once. The actual lab_reset
// IPC call lives in the parent (LabBlock) so this dialog stays
// presentational — easier to test, easier to reuse if a future
// "Reset and skip" button needs different IPC chaining.

import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";

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

export function LabResetDialog({
  creates,
  onConfirm,
  onCancel,
  open = true,
}: LabResetDialogProps) {
  const cancelRef = useRef<HTMLButtonElement | null>(null);

  // Auto-focus Cancel on open so Escape doesn't accidentally trigger
  // Reset (Confirm) — the safer default.
  useEffect(() => {
    if (open) cancelRef.current?.focus();
  }, [open]);

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="lab-reset-dialog-title"
      data-testid="lab-reset-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-background/60 p-4 backdrop-blur-sm"
    >
      <div
        className={cn(
          "w-full max-w-md rounded-lg border border-border p-5 shadow-xl",
        )}
        style={{
          background: "var(--glass-bg)",
          borderColor: "var(--glass-border)",
        }}
      >
        <h3
          id="lab-reset-dialog-title"
          className="text-base font-semibold text-foreground"
        >
          Reset lab?
        </h3>
        <p className="mt-2 text-sm text-muted-foreground">
          The following files will be deleted from your workspace.
          Other files (including labs you completed earlier in this
          module) will be kept.
        </p>
        <ul
          data-testid="lab-reset-creates-list"
          className="mt-3 max-h-40 overflow-auto rounded-md border border-border bg-background/40 p-2 font-mono text-xs"
        >
          {creates.length === 0 ? (
            <li className="text-muted-foreground">
              No files declared — only the progress will reset.
            </li>
          ) : (
            creates.map((path) => (
              <li key={path} className="text-foreground">
                {path}
              </li>
            ))
          )}
        </ul>
        <div className="mt-4 flex justify-end gap-2">
          <button
            ref={cancelRef}
            type="button"
            onClick={onCancel}
            className="rounded-md border border-border bg-background px-3 py-1.5 text-sm text-foreground hover:bg-accent hover:text-accent-foreground focus:outline-none focus:ring-2 focus:ring-ring"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onConfirm}
            className="rounded-md bg-destructive px-3 py-1.5 text-sm font-medium text-destructive-foreground hover:opacity-90 focus:outline-none focus:ring-2 focus:ring-ring"
          >
            Reset
          </button>
        </div>
      </div>
    </div>
  );
}
