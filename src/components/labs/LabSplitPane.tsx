// Phase 03.1 plan 03.1-06 — reusable horizontal split-pane.
//
// 40/60 default split (instructions left, terminal right — terminal needs
// the bigger half so commands and output are readable). Draggable
// separator, glassmorphism aesthetic via the existing `var(--glass-bg)`
// / `var(--glass-border)` tokens. No third-party split-pane dependency —
// pure CSS flex + a pointerdown listener on the separator. Stays under
// 200 lines per the plan.

import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

const MIN_PERCENT = 30;
const MAX_PERCENT = 80;

export interface LabSplitPaneProps {
  left: ReactNode;
  right: ReactNode;
  /** Initial left-pane percentage (clamped 30..80). Defaults to 40 — the
   *  terminal needs the bigger half so commands and output are readable. */
  defaultLeftPercent?: number;
}

export function LabSplitPane({
  left,
  right,
  defaultLeftPercent = 40,
}: LabSplitPaneProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const draggingRef = useRef(false);
  const [leftPercent, setLeftPercent] = useState(() =>
    clampPercent(defaultLeftPercent),
  );

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    draggingRef.current = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!draggingRef.current) return;
    const container = containerRef.current;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    if (rect.width <= 0) return;
    const offset = e.clientX - rect.left;
    const pct = (offset / rect.width) * 100;
    setLeftPercent(clampPercent(pct));
  }, []);

  const onPointerUp = useCallback((e: React.PointerEvent) => {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    try {
      (e.target as HTMLElement).releasePointerCapture(e.pointerId);
    } catch {
      // Pointer may already be released — ignore.
    }
  }, []);

  // Keyboard accessibility: arrow keys move the divider in 5% steps.
  const onKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      setLeftPercent((p) => clampPercent(p - 5));
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      setLeftPercent((p) => clampPercent(p + 5));
    }
  }, []);

  // Cleanup: if a pointerup is missed, reset on unmount.
  useEffect(() => {
    return () => {
      draggingRef.current = false;
    };
  }, []);

  return (
    <div
      ref={containerRef}
      data-testid="lab-split-pane"
      className="flex h-full w-full overflow-hidden rounded-md"
      style={{
        background: "var(--glass-bg)",
        border: "1px solid var(--glass-border)",
      }}
    >
      <div
        className="overflow-auto"
        style={{ width: `${leftPercent}%` }}
        data-testid="lab-split-left"
      >
        {left}
      </div>
      <div
        role="separator"
        aria-orientation="vertical"
        aria-valuenow={Math.round(leftPercent)}
        aria-valuemin={MIN_PERCENT}
        aria-valuemax={MAX_PERCENT}
        tabIndex={0}
        data-testid="lab-split-separator"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        onKeyDown={onKeyDown}
        className="flex w-1 cursor-col-resize items-center justify-center transition-colors hover:bg-primary/30 focus:bg-primary/40 focus:outline-none"
        style={{ background: "var(--glass-border)" }}
      />
      <div
        className="flex-1 overflow-hidden"
        data-testid="lab-split-right"
      >
        {right}
      </div>
    </div>
  );
}

function clampPercent(p: number): number {
  if (p < MIN_PERCENT) return MIN_PERCENT;
  if (p > MAX_PERCENT) return MAX_PERCENT;
  return p;
}
