// Phase 4 Plan 05 (Wave 4) — Daily Challenge focused view.
//
// /daily/today route handler. Renders the single micro-block selected for
// today inside a focused chrome (NOT the full ModuleView shell — D-02).
// Wires block-completion through `useDailyChallengeStore.completeChallenge`
// and routes back to "/" on success.
//
// Architecture notes:
// - `DailyBlockHost` (below) mirrors `BlockRenderer`'s ready-block dispatch
//   for the three eligible block types (section / quiz / flash_cards). We
//   deliberately duplicate the dispatch instead of extending BlockRenderer
//   because the `onComplete` plumbing is a Phase-4-only concern (R1 /
//   isolation lock from Plan 04 SUMMARY). Non-ready blocks should never
//   reach this surface because the selection algorithm filters
//   `status='ready'` (Plan 02 algorithm step 2).
// - Mount sequence (Q4 + R5 + Pitfall 8):
//     1. defensive: if store.todaysChallenge is null, redirect "/" (T-04-13).
//     2. call startChallenge() — fire-and-forget; the store handles rollback.
//     3. fetch block via getModuleBlocks(moduleId) and locate by blockId.
//     4. if the block is missing (module regenerated → FK CASCADE traversal
//        race per R5/Pitfall 8), show "expired" message + redirect "/".
// - On block completion: call completeDailyChallenge() then navigate("/").
// - On unmount without completion: do NOTHING (Q7 — leaves status
//   in_progress; the next daily fetch reflects that the row was started but
//   not completed).

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft, Sparkles } from "lucide-react";
import { useDailyChallengeStore } from "@/stores/useDailyChallengeStore";
import { getModuleBlocks } from "@/lib/tauri-commands";
import { SectionBlock } from "@/components/learning/SectionBlock";
import { QuizBlock } from "@/components/learning/QuizBlock";
import { FlashCardsBlock } from "@/components/learning/FlashCardsBlock";
import type { ModuleBlock } from "@/types/learning";

export function DailyChallenge(): JSX.Element | null {
  const navigate = useNavigate();
  const todaysChallenge = useDailyChallengeStore((s) => s.todaysChallenge);
  const globalStreakDays = useDailyChallengeStore((s) => s.globalStreakDays);
  const startChallenge = useDailyChallengeStore((s) => s.startDailyChallenge);
  const completeChallenge = useDailyChallengeStore((s) => s.completeDailyChallenge);

  const [block, setBlock] = useState<ModuleBlock | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [completed, setCompleted] = useState(false);

  // Defensive redirect: direct URL access without a loaded challenge.
  // The Dashboard CTA is the only sanctioned entry point — gate is the
  // card itself (T-04-13). For anything else, send the user home.
  useEffect(() => {
    if (!todaysChallenge) {
      navigate("/", { replace: true });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Mount: start + fetch the block payload. Intentionally one-shot on mount
  // (no deps) — re-running on store updates would re-fire start which is
  // unnecessary (the store's startDailyChallenge is idempotent server-side,
  // but firing once per visit is the cleanest contract).
  useEffect(() => {
    if (!todaysChallenge) return;

    // Fire-and-forget; store handles error rollback (Pattern 3).
    void startChallenge();

    let cancelled = false;
    // WR-01 — hoist the expired-redirect timer id into the outer effect's
    // closure so the cleanup function below can clear it. The previous
    // implementation tried to return `() => clearTimeout(t)` from inside the
    // `.then()` callback, but that return value lives in the Promise chain
    // and is silently discarded — the useEffect cleanup is the `return () =>
    // { ... }` declared below. If the user navigated away during the 2.5s
    // window, the orphan timer would still fire `navigate("/")` on the
    // unmounted component.
    let expiredRedirectTimer: ReturnType<typeof setTimeout> | undefined;

    getModuleBlocks(todaysChallenge.moduleId)
      .then((blocks) => {
        if (cancelled) return;
        const found = blocks.find((b) => b.id === todaysChallenge.blockId);
        if (!found) {
          // R5 / Pitfall 8 — module regeneration deleted this block while
          // the user was offline. The server-side FK CASCADE should have
          // taken `daily_challenges.block_id` with it, so the next Dashboard
          // load will re-select. Show a friendly message and bounce home.
          setError(
            "This challenge expired — we'll pick a new one. Returning to Dashboard.",
          );
          expiredRedirectTimer = setTimeout(() => {
            if (cancelled) return;
            navigate("/", { replace: true });
          }, 2500);
          return;
        }
        setBlock(found);
      })
      .catch((err) => {
        if (cancelled) return;
        console.error("[DailyChallenge] failed to load block:", err);
        setError("Couldn't load today's challenge. Try again from the Dashboard.");
      });

    return () => {
      cancelled = true;
      if (expiredRedirectTimer !== undefined) {
        clearTimeout(expiredRedirectTimer);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleBlockComplete = async () => {
    setCompleted(true);
    try {
      await completeChallenge();
    } finally {
      navigate("/", { replace: true });
    }
  };

  // ── Render branches ─────────────────────────────────────────────────────

  if (!todaysChallenge) {
    // Mid-redirect render — nothing to show.
    return null;
  }

  if (error) {
    return (
      <div
        className="glass mx-auto max-w-2xl my-12 p-6 rounded-xl text-center text-foreground"
        data-testid="daily-challenge-expired"
      >
        <p className="text-base m-0">{error}</p>
        <button
          type="button"
          onClick={() => navigate("/", { replace: true })}
          className="mt-4 inline-flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground"
        >
          <ArrowLeft size={16} />
          Back to Dashboard
        </button>
      </div>
    );
  }

  if (completed) {
    return (
      <div
        className="glass mx-auto max-w-2xl my-12 p-8 rounded-xl text-center"
        data-testid="daily-challenge-done"
      >
        <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-gradient-to-br from-violet-500/30 via-blue-500/30 to-cyan-500/30">
          <Sparkles size={22} className="text-foreground" />
        </div>
        <h2 className="text-xl font-semibold text-foreground m-0">
          Done for today
        </h2>
        <p className="mt-2 text-sm text-muted-foreground">
          Streak: {globalStreakDays}d
        </p>
        <button
          type="button"
          onClick={() => navigate("/", { replace: true })}
          className="mt-6 glass-strong px-4 py-2 rounded-md text-sm font-medium hover:opacity-90 transition-opacity"
        >
          Back to Dashboard
        </button>
      </div>
    );
  }

  if (!block) {
    return (
      <div
        className="glass mx-auto max-w-2xl my-12 p-6 rounded-xl text-center text-muted-foreground"
        data-testid="daily-challenge-loading"
      >
        Loading today's challenge...
      </div>
    );
  }

  return (
    <div
      className="mx-auto max-w-3xl space-y-6 py-8"
      data-testid="daily-challenge-view"
    >
      {/* Daily-specific chrome — distinct from ModuleView's outer shell (D-02). */}
      <header className="flex items-center justify-between">
        <button
          type="button"
          onClick={() => navigate("/", { replace: true })}
          className="inline-flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground"
          data-testid="daily-challenge-back"
        >
          <ArrowLeft size={16} />
          Back to Dashboard
        </button>
        <p className="text-xs uppercase tracking-wide text-muted-foreground">
          Today's Challenge
        </p>
      </header>

      <DailyBlockHost block={block} onComplete={handleBlockComplete} />
    </div>
  );
}

/**
 * Phase-4-local wrapper that mirrors `BlockRenderer`'s ready-block dispatch
 * but threads the `onComplete` prop down to the underlying block component.
 *
 * Why duplicate instead of extending BlockRenderer? `onComplete` is a
 * Phase-4-only concern — the ModuleView surface should not learn about
 * daily-challenge completion semantics. Keeping the dispatch isolated here
 * preserves R1 (BlockStatus enum stays unmodified) and matches the open-core
 * isolation precedent (Plan 04 SUMMARY: sibling-slice never extension).
 *
 * Non-ready blocks should not reach this code path — the selection algorithm
 * filters `status='ready'` (Plan 02 step 2). If a non-ready block somehow
 * lands here we render a fallback notice rather than the skeleton, since the
 * skeleton's regenerate action would be confusing inside the daily flow.
 */
function DailyBlockHost({
  block,
  onComplete,
}: {
  block: ModuleBlock;
  onComplete: () => void;
}) {
  if (block.status !== "ready") {
    return (
      <div
        className="glass rounded-lg p-6 text-sm text-muted-foreground"
        data-testid="daily-challenge-unsupported"
      >
        This challenge isn't ready yet. Try again later.
      </div>
    );
  }

  switch (block.blockType) {
    case "section":
      return (
        <SectionBlock
          block={block}
          moduleId={block.moduleId}
          onComplete={onComplete}
        />
      );
    case "quiz":
      return <QuizBlockWithTrackId block={block} onComplete={onComplete} />;
    case "flash_cards":
      return (
        <FlashCardsBlock
          block={block}
          moduleId={block.moduleId}
          onComplete={onComplete}
        />
      );
    default:
      return (
        <div
          className="glass rounded-lg p-6 text-sm text-muted-foreground"
          data-testid="daily-challenge-unsupported"
        >
          Unsupported block type for daily: {block.blockType}
        </div>
      );
  }
}

/**
 * QuizBlock needs `trackId` (it persists per-track BKT/SR signals via the
 * store action). The trackId lives on `todaysChallenge` — read it here so
 * DailyBlockHost stays a pure block dispatcher.
 */
function QuizBlockWithTrackId({
  block,
  onComplete,
}: {
  block: ModuleBlock;
  onComplete: () => void;
}) {
  const trackId = useDailyChallengeStore((s) => s.todaysChallenge?.trackId);
  return (
    <QuizBlock
      block={block}
      moduleId={block.moduleId}
      trackId={trackId}
      onComplete={onComplete}
    />
  );
}
