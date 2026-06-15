// Phase 4 Plan 04 (Wave 3) - Dashboard "Today's Challenge" card.
//
// Visual template: SmartSessionCard.tsx (gradient outer border + glass interior
// + lucide icon in a gradient circle + Link CTA). Differentiated by palette
// (violet -> blue -> cyan) and icon (Sparkles) so users can tell the two cards
// apart at a glance.
//
// FIVE rendered states:
//
//   1. isEnabled=false              -> returns null (D-12 gate not yet fired)
//   2. isEnabled, challenge=null    -> "No challenge today; keep learning." (Q3 empty zone)
//   3. challenge.status=pending     -> "Today's Challenge" + Start CTA
//   4. challenge.status=in_progress -> "Today's Challenge - in progress" + Resume CTA
//   5. challenge.status=done        -> "Done for today" + streak summary, no CTA
//
// All copy is ASCII-only (no emojis - CONVENTIONS rule).

import { Link } from "react-router-dom";
import { Sparkles, ArrowRight } from "lucide-react";
import { useDailyChallengeStore } from "@/stores/useDailyChallengeStore";

function humanizeBlockType(blockType: string): string {
  switch (blockType) {
    case "flash_cards":
      return "Flash cards";
    case "quiz":
      return "Quiz";
    case "section":
      return "Lesson recap";
    case "text":
      return "Reading";
    case "callout":
      return "Quick tip";
    case "lab":
      return "Lab";
    default:
      return blockType;
  }
}

export function TodaysChallengeCard(): JSX.Element | null {
  const isEnabled = useDailyChallengeStore((s) => s.isEnabled);
  const todaysChallenge = useDailyChallengeStore((s) => s.todaysChallenge);
  const globalStreakDays = useDailyChallengeStore((s) => s.globalStreakDays);

  // State 1 -gate not yet fired (D-12). Render nothing.
  if (!isEnabled) {
    return null;
  }

  // State 2 -gate fired but BKT [0.3, 0.7] zone empty (Q3).
  if (todaysChallenge === null) {
    return (
      <div
        className="relative overflow-hidden rounded-xl border border-muted bg-[hsl(var(--card))]"
        data-testid="daily-challenge-card-empty"
      >
        <div className="flex items-center gap-4 px-6 py-5">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-muted">
            <Sparkles size={20} className="text-muted-foreground" />
          </div>
          <div className="min-w-0">
            <h3 className="text-sm font-semibold text-foreground">No challenge today</h3>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Your active modules aren't in the practice zone yet - keep learning.
            </p>
          </div>
        </div>
      </div>
    );
  }

  // Derived copy for pending / in_progress states.
  const estMinutesLabel = `~${todaysChallenge.estMinutes} min - ${humanizeBlockType(todaysChallenge.blockType)}`;

  // State 5 -done. No CTA; show streak summary.
  if (todaysChallenge.status === "done") {
    const dayWord = globalStreakDays === 1 ? "day" : "days";
    return (
      <div
        className="relative overflow-hidden rounded-xl border border-muted bg-[hsl(var(--card))]"
        data-testid="daily-challenge-card-done"
      >
        <div className="flex items-center justify-between gap-4 px-6 py-5">
          <div className="flex items-center gap-4">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-muted">
              <Sparkles size={20} className="text-muted-foreground" />
            </div>
            <div className="min-w-0">
              <h3 className="text-sm font-semibold text-foreground">Done for today</h3>
              <p className="mt-0.5 text-xs text-muted-foreground">See you tomorrow.</p>
            </div>
          </div>
          <span className="text-xs font-medium text-muted-foreground">
            Streak: {globalStreakDays} {dayWord}
          </span>
        </div>
      </div>
    );
  }

  // States 3 + 4 -pending or in_progress. Gradient + Start/Resume CTA.
  const isInProgress = todaysChallenge.status === "in_progress";
  const title = isInProgress ? "Today's Challenge - in progress" : "Today's Challenge";
  const ctaLabel = isInProgress ? "Resume" : "Start";
  const testId = isInProgress
    ? "daily-challenge-card-in-progress"
    : "daily-challenge-card-pending";

  return (
    <div className="relative overflow-hidden rounded-xl p-[2px]" data-testid={testId}>
      {/* Gradient border - distinct from SmartSessionCard palette so the two
          cards are visually distinguishable. */}
      <div className="absolute inset-0 rounded-xl bg-gradient-to-r from-violet-500 via-blue-500 to-cyan-500" />

      {/* Card interior */}
      <div className="relative flex items-center justify-between gap-4 rounded-[10px] bg-[hsl(var(--card))] px-6 py-5">
        <div className="flex items-center gap-4">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-violet-500 to-cyan-500">
            <Sparkles size={20} className="text-white" />
          </div>
          <div className="min-w-0">
            <h3 className="text-sm font-semibold text-foreground">{title}</h3>
            <p className="mt-0.5 text-xs text-muted-foreground">{estMinutesLabel}</p>
          </div>
        </div>

        <Link
          to="/daily/today"
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg bg-gradient-to-r from-violet-500 to-cyan-500 px-5 py-2.5 text-sm font-semibold text-white transition-opacity hover:opacity-90"
        >
          {ctaLabel}
          <ArrowRight size={16} />
        </Link>
      </div>
    </div>
  );
}
