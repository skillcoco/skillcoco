import { useState, useEffect, useCallback } from "react";
import { Link } from "react-router-dom";
import {
  ArrowLeft,
  Loader2,
  CheckCircle2,
  ChevronRight,
  Brain,
} from "lucide-react";
import { getDueCards, submitReview } from "@/lib/tauri-commands";
import type { SRCard } from "@/types";
import { cn } from "@/lib/utils";

type SessionState = "loading" | "empty" | "reviewing" | "complete";

// Phase 1 note: getDueCards returns at most 50 cards per session.
// Pagination and adaptive prioritization are Phase 4 microlearning scope.

export function ReviewSession() {
  // Queue-based: cards[0] is always the current card.
  // After each rating, re-fetch due cards so the queue reflects SM-2 scheduling.
  const [cards, setCards] = useState<SRCard[]>([]);
  const [revealed, setRevealed] = useState(false);
  const [sessionState, setSessionState] = useState<SessionState>("loading");
  const [reviewedCount, setReviewedCount] = useState(0);
  // Interval toast: shown after each rating ("Next review in N days"), auto-clears
  const [intervalToast, setIntervalToast] = useState<string | null>(null);

  const fetchDueCards = useCallback(async () => {
    try {
      const due = await getDueCards();
      setCards(due);
      return due;
    } catch (err) {
      console.error("Failed to load due cards:", err);
      return [];
    }
  }, []);

  useEffect(() => {
    async function load() {
      const due = await fetchDueCards();
      setSessionState(due.length === 0 ? "empty" : "reviewing");
    }
    load();
  }, [fetchDueCards]);

  const currentCard = cards[0];

  const handleRate = useCallback(
    async (quality: 1 | 3 | 4 | 5) => {
      if (!currentCard) return;

      let intervalDays: number | null = null;
      try {
        const result = await submitReview(currentCard.id, quality);
        intervalDays = result.newIntervalDays;
      } catch (err) {
        console.error("Failed to submit review:", err);
      }

      setReviewedCount((prev) => prev + 1);
      setRevealed(false);

      // Show interval delta toast
      if (intervalDays !== null) {
        const days = Math.round(intervalDays);
        setIntervalToast(`Next review in ${days} day${days !== 1 ? "s" : ""}`);
        setTimeout(() => setIntervalToast(null), 2500);
      }

      // Re-fetch due cards after submission (LOOP-04 queue re-fetch)
      const remaining = await fetchDueCards();
      if (remaining.length === 0) {
        setSessionState("complete");
      }
    },
    [currentCard, fetchDueCards],
  );

  // ── Loading ──
  if (sessionState === "loading") {
    return (
      <div className="mx-auto flex h-64 max-w-2xl items-center justify-center text-muted-foreground">
        <Loader2 size={20} className="mr-2 animate-spin" />
        <span>Loading review cards...</span>
      </div>
    );
  }

  // ── Empty / No cards due ──
  if (sessionState === "empty") {
    return (
      <div className="mx-auto max-w-2xl space-y-6">
        <div className="flex items-center gap-3">
          <Link to="/" className="rounded-md p-1.5 text-muted-foreground hover:bg-accent">
            <ArrowLeft size={18} />
          </Link>
          <h1 className="text-2xl font-bold text-foreground">Review Session</h1>
        </div>
        <div className="glass flex flex-col items-center justify-center rounded-xl py-16 text-center">
          <CheckCircle2 size={48} className="mb-4 text-emerald-500" />
          <h2 className="text-lg font-semibold text-foreground">All caught up</h2>
          <p className="mt-1 text-sm text-muted-foreground">
            No cards due for review — well done. Keep studying and cards will appear as they become due.
          </p>
          <Link
            to="/"
            className="mt-6 inline-flex items-center gap-1.5 rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
          >
            Back to Dashboard
            <ChevronRight size={16} />
          </Link>
        </div>
      </div>
    );
  }

  // ── Complete ──
  if (sessionState === "complete") {
    return (
      <div className="mx-auto max-w-2xl space-y-6">
        <div className="flex items-center gap-3">
          <Link to="/" className="rounded-md p-1.5 text-muted-foreground hover:bg-accent">
            <ArrowLeft size={18} />
          </Link>
          <h1 className="text-2xl font-bold text-foreground">Review Session</h1>
        </div>
        <div className="glass flex flex-col items-center justify-center rounded-xl py-16 text-center">
          <CheckCircle2 size={48} className="mb-4 text-emerald-500" />
          <h2 className="text-lg font-semibold text-foreground">Session Complete</h2>
          <p className="mt-2 text-sm text-muted-foreground">
            You reviewed {reviewedCount} card{reviewedCount !== 1 ? "s" : ""} this session.
          </p>
          <Link
            to="/"
            className="mt-6 inline-flex items-center gap-1.5 rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
          >
            Back to Dashboard
            <ChevronRight size={16} />
          </Link>
        </div>
      </div>
    );
  }

  // ── Reviewing ──
  // currentCard = cards[0] (head of queue, re-fetched after each submission)
  return (
    <div className="mx-auto max-w-2xl space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <Link to="/" className="rounded-md p-1.5 text-muted-foreground hover:bg-accent">
          <ArrowLeft size={18} />
        </Link>
        <div className="flex-1">
          <h1 className="text-2xl font-bold text-foreground">Review Session</h1>
          <p className="text-sm text-muted-foreground">
            {cards.length} card{cards.length !== 1 ? "s" : ""} remaining
          </p>
        </div>
      </div>

      {/* Interval toast — shown after rating, auto-dismisses in 2.5s */}
      {intervalToast && (
        <div className="rounded-lg bg-emerald-500/10 px-4 py-2 text-center text-sm font-medium text-emerald-600 dark:text-emerald-400">
          {intervalToast}
        </div>
      )}

      {/* Card */}
      {currentCard && (
        <div className="glass rounded-xl p-8">
          {/* Concept badge */}
          <div className="mb-4 flex items-center gap-2">
            <Brain size={14} className="text-primary" />
            <span className="text-xs font-medium text-muted-foreground">
              {currentCard.concept}
            </span>
          </div>

          {/* Front */}
          <div className="mb-6">
            <p className="text-lg font-medium leading-relaxed text-foreground">
              {currentCard.front}
            </p>
          </div>

          {/* Divider + Answer */}
          {revealed ? (
            <>
              <div className="mb-6 border-t border-border" />
              <div className="rounded-lg bg-secondary/50 p-4">
                <p className="text-sm leading-relaxed text-foreground/90">
                  {currentCard.back}
                </p>
              </div>

              {/* Rating buttons — SM-2 quality scale */}
              <div className="mt-6 grid grid-cols-4 gap-2">
                {([
                  { quality: 1 as const, label: "Again", color: "text-red-500", desc: "Forgot" },
                  { quality: 3 as const, label: "Hard", color: "text-orange-500", desc: "Struggled" },
                  { quality: 4 as const, label: "Good", color: "text-emerald-500", desc: "Recalled" },
                  { quality: 5 as const, label: "Easy", color: "text-blue-500", desc: "Instant" },
                ]).map(({ quality, label, color, desc }) => (
                  <button
                    key={quality}
                    onClick={() => handleRate(quality)}
                    className="flex flex-col items-center gap-1 rounded-lg border border-border py-3 text-sm transition-colors hover:bg-accent"
                  >
                    <span className={cn("font-semibold", color)}>{label}</span>
                    <span className="text-[10px] text-muted-foreground">{desc}</span>
                  </button>
                ))}
              </div>
            </>
          ) : (
            <button
              onClick={() => setRevealed(true)}
              className="mt-4 w-full rounded-lg bg-primary py-3 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
            >
              Show Answer
            </button>
          )}
        </div>
      )}
    </div>
  );
}
