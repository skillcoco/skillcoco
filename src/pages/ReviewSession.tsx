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

export function ReviewSession() {
  const [cards, setCards] = useState<SRCard[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [sessionState, setSessionState] = useState<SessionState>("loading");
  const [reviewedCount, setReviewedCount] = useState(0);
  const [ratings, setRatings] = useState<number[]>([]);

  useEffect(() => {
    async function load() {
      try {
        const due = await getDueCards();
        setCards(due);
        setSessionState(due.length === 0 ? "empty" : "reviewing");
      } catch (err) {
        console.error("Failed to load due cards:", err);
        setSessionState("empty");
      }
    }
    load();
  }, []);

  const currentCard = cards[currentIndex];

  const handleRate = useCallback(
    async (quality: 1 | 3 | 4 | 5) => {
      if (!currentCard) return;

      try {
        await submitReview({
          cardId: currentCard.id,
          quality,
          responseTime: 0,
          response: "",
        });
      } catch (err) {
        console.error("Failed to submit review:", err);
      }

      setRatings((prev) => [...prev, quality]);
      setRevealed(false);
      setReviewedCount((prev) => prev + 1);

      if (currentIndex + 1 >= cards.length) {
        setSessionState("complete");
      } else {
        setCurrentIndex((prev) => prev + 1);
      }
    },
    [currentCard, currentIndex, cards.length],
  );

  const avgRating =
    ratings.length > 0
      ? (ratings.reduce((a, b) => a + b, 0) / ratings.length).toFixed(1)
      : "0";

  // ── Loading ──
  if (sessionState === "loading") {
    return (
      <div className="mx-auto flex h-64 max-w-2xl items-center justify-center text-muted-foreground">
        <Loader2 size={20} className="mr-2 animate-spin" />
        <span>Loading review cards...</span>
      </div>
    );
  }

  // ── Empty ──
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
          <h2 className="text-lg font-semibold text-foreground">No cards due for review</h2>
          <p className="mt-1 text-sm text-muted-foreground">
            Keep studying and cards will appear as they become due.
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
            You reviewed {reviewedCount} card{reviewedCount !== 1 ? "s" : ""} with an average
            rating of {avgRating}/5.
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
  const progress = ((currentIndex + 1) / cards.length) * 100;

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
            Card {currentIndex + 1} of {cards.length}
          </p>
        </div>
      </div>

      {/* Progress bar */}
      <div className="h-2 overflow-hidden rounded-full bg-secondary">
        <div
          className="h-full rounded-full bg-primary transition-all duration-300"
          style={{ width: `${progress}%` }}
        />
      </div>

      {/* Card */}
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

            {/* Rating buttons */}
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
    </div>
  );
}
