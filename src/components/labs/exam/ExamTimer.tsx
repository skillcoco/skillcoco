// Phase 19 (EXAM-01/EXAM-04) — countdown chip against a fixed, backend-
// persisted `deadline_at` (RESEARCH.md Pattern 3 — display-only, never
// authoritative). The backend independently recomputes timeout on submit,
// so a manipulated client clock cannot extend the real deadline (T-19-01).
//
// 3-phase color treatment per 19-UI-SPEC.md Timer color states table:
//   Normal  (> 15 min remaining)         text-foreground
//   Warning (<= 15 min, > 5 min remain)  text-amber-500
//   Urgent  (<= 5 min remaining)         text-destructive
//
// Reuses TrackView.tsx's stat-chip class verbatim, unchanged:
// "glass flex items-center gap-2 rounded-lg px-4 py-2.5"

import { useEffect, useRef, useState } from "react";
import { Clock } from "lucide-react";
import { cn } from "@/lib/utils";

export interface ExamTimerProps {
  /** RFC-3339 deadline timestamp, backend-persisted at exam start. */
  deadlineAt: string;
  /** Fires exactly once when remaining time hits 0. */
  onExpire?: () => void;
}

type TimerPhase = "normal" | "warning" | "urgent";

const WARNING_THRESHOLD_SECONDS = 15 * 60;
const URGENT_THRESHOLD_SECONDS = 5 * 60;

function computeRemainingSeconds(deadlineAt: string): number {
  const deadlineMs = new Date(deadlineAt).getTime();
  const remainingMs = deadlineMs - Date.now();
  return Math.max(0, Math.ceil(remainingMs / 1000));
}

function phaseFor(remainingSeconds: number): TimerPhase {
  if (remainingSeconds <= URGENT_THRESHOLD_SECONDS) return "urgent";
  if (remainingSeconds <= WARNING_THRESHOLD_SECONDS) return "warning";
  return "normal";
}

function formatMMSS(remainingSeconds: number): string {
  const mins = Math.floor(remainingSeconds / 60);
  const secs = remainingSeconds % 60;
  return `${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
}

const PHASE_TEXT_CLASS: Record<TimerPhase, string> = {
  normal: "text-foreground",
  warning: "text-amber-500",
  urgent: "text-destructive",
};

const PHASE_ICON_CLASS: Record<TimerPhase, string> = {
  normal: "text-muted-foreground",
  warning: "text-amber-500",
  urgent: "text-destructive",
};

export function ExamTimer({ deadlineAt, onExpire }: ExamTimerProps) {
  const [remainingSeconds, setRemainingSeconds] = useState(() =>
    computeRemainingSeconds(deadlineAt),
  );
  const hasExpiredRef = useRef(false);

  useEffect(() => {
    hasExpiredRef.current = false;
    setRemainingSeconds(computeRemainingSeconds(deadlineAt));

    const interval = setInterval(() => {
      setRemainingSeconds(computeRemainingSeconds(deadlineAt));
    }, 1000);

    return () => clearInterval(interval);
  }, [deadlineAt]);

  useEffect(() => {
    if (remainingSeconds <= 0 && !hasExpiredRef.current) {
      hasExpiredRef.current = true;
      onExpire?.();
    }
  }, [remainingSeconds, onExpire]);

  const phase = phaseFor(remainingSeconds);

  return (
    <div
      data-testid="exam-timer"
      className={cn(
        "glass flex items-center gap-2 rounded-lg px-4 py-2.5",
        PHASE_TEXT_CLASS[phase],
      )}
    >
      <Clock size={16} className={PHASE_ICON_CLASS[phase]} />
      <span className="tabular-nums text-sm font-semibold">
        {formatMMSS(remainingSeconds)}
      </span>
    </div>
  );
}
