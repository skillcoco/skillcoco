// Phase 03.1 plan 03.1-06 — manual progressive 3-tier hint reveal.
//
// Per RESEARCH q7: tier reveal lives in component-local state (NOT in
// useLabStore) so revealed hints reset on lab close. This matches the
// "lab close = fresh shell" semantic.
//
// The component is presentational — it receives `revealedTier` from the
// parent and calls `onShowNext` when the user clicks "Show hint". The
// parent owns the actual tier counter; this lets the parent invoke the
// `lab_show_hint` IPC and persist any side effects (analytics, AI-cost
// budgeting) without coupling the panel to those concerns.

import { cn } from "@/lib/utils";

export interface LabHintPanelProps {
  /** Three-tier hints from LabSpec.steps[i].hints. */
  hints: string[];
  /** Currently revealed tier (0 = none, 1..3 = revealed). */
  revealedTier: number;
  /** Click handler that advances revealedTier by one (capped at hints.length). */
  onShowNext: () => void;
}

const TIER_LABELS = ["Gentle nudge", "Partial answer", "Full solution"];

export function LabHintPanel({
  hints,
  revealedTier,
  onShowNext,
}: LabHintPanelProps) {
  const total = hints.length;
  const isFinal = revealedTier >= total;
  const visible = hints.slice(0, Math.max(0, Math.min(revealedTier, total)));

  return (
    <section
      data-testid="lab-hint-panel"
      data-final-tier={isFinal ? "true" : "false"}
      className="flex flex-col gap-3 rounded-md border border-border bg-card/40 p-3"
    >
      <header className="flex items-center justify-between">
        <h5 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          Hints {revealedTier > 0 ? `(${revealedTier}/${total})` : ""}
        </h5>
        <button
          type="button"
          onClick={onShowNext}
          disabled={isFinal}
          className={cn(
            "rounded-md border border-border bg-background px-2 py-1 text-xs",
            "transition-colors focus:outline-none focus:ring-2 focus:ring-ring",
            isFinal
              ? "cursor-not-allowed text-muted-foreground opacity-60"
              : "text-foreground hover:bg-accent hover:text-accent-foreground",
          )}
        >
          Show hint
        </button>
      </header>
      {visible.length === 0 ? null : (
        <ol className="flex flex-col gap-2">
          {visible.map((hint, i) => (
            <li
              key={i}
              data-testid={`lab-hint-tier-${i + 1}`}
              className="rounded border border-border/60 bg-background/40 p-2 text-sm text-foreground"
            >
              <div className="mb-1 text-[11px] font-medium uppercase text-muted-foreground">
                Tier {i + 1} — {TIER_LABELS[i] ?? `Hint ${i + 1}`}
              </div>
              <p className="whitespace-pre-wrap">{hint}</p>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
