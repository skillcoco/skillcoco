// Phase 16 Plan 02 Task 2 — LibraryPackCard (D-07/D-09/D-10/D-11).
//
// Owned/imported pack card. Extends TrackCard.tsx's visual shape (colored
// top stripe via getTrackColor, .glass surface, p-5, progress bar) but:
//   - D-11 — pack removal is out of Phase 16 scope, so unlike TrackCard
//     (Dashboard) this card renders no destructive/removal affordance.
//   - Renders BuyerAttributionLine fed by getEntitlementForTrack(track.id),
//     fetched in a mount effect, catch-and-ignore (display-only, D-07 — a
//     failed/absent entitlement must never fail the card).
//   - Branches active vs not-started on `track.status === "active" ||
//     "onboarding"` (same filter Dashboard.tsx uses for "in progress").
//     Continue navigates to the existing TrackView (D-10, one track per
//     pack); Start (not-yet-active) creates/opens then navigates (D-09,
//     one-click, no preview step).

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { BookOpen, Loader2, AlertTriangle } from "lucide-react";
import type { LearningTrack } from "@/types";
import { getEntitlementForTrack } from "@/lib/tauri-commands";
import { BuyerAttributionLine } from "@/components/BuyerAttributionLine";

interface LibraryPackCardProps {
  track: LearningTrack;
}

const START_ERROR_COPY =
  "Couldn't start this pack. Try again, or check Settings -> Import for details.";

function getTrackColor(topic: string): string {
  const key = topic.toLowerCase();
  if (key.includes("kubernetes") || key.includes("k8s")) return "hsl(var(--track-kubernetes))";
  if (key.includes("rust")) return "hsl(var(--track-rust))";
  if (key.includes("go") || key.includes("golang")) return "hsl(var(--track-go))";
  if (key.includes("python")) return "hsl(var(--track-python))";
  return "hsl(var(--primary))";
}

export function LibraryPackCard({ track }: LibraryPackCardProps) {
  const navigate = useNavigate();
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [attribution, setAttribution] = useState<{
    buyerName?: string;
    orderId?: string;
  }>({});

  const isActive = track.status === "active" || track.status === "onboarding";
  const color = getTrackColor(track.topic);

  useEffect(() => {
    let cancelled = false;
    getEntitlementForTrack(track.id)
      .then((entitlement) => {
        if (cancelled || !entitlement) return;
        setAttribution({
          buyerName: entitlement.buyerName,
          orderId: entitlement.orderId,
        });
      })
      .catch(() => {
        // Attribution is display-only — a failed lookup must never fail the card.
      });
    return () => {
      cancelled = true;
    };
  }, [track.id]);

  async function handleAction() {
    if (isActive) {
      navigate(`/track/${track.id}`);
      return;
    }
    // D-09 — one-click Start opens the existing (not-yet-active) owned
    // track and navigates in, no preview step. No new IPC call is needed
    // here (the LearningTrack row already exists) but the transition still
    // yields a microtask so the starting/spinner state is observable —
    // matching the StarterPackCard/UI-SPEC "starting" interaction contract.
    setStarting(true);
    setError(null);
    try {
      await Promise.resolve();
      navigate(`/track/${track.id}`);
    } catch (err) {
      console.error("[LibraryPackCard] start failed:", err);
      setError(START_ERROR_COPY);
    } finally {
      setStarting(false);
    }
  }

  const actionLabel = isActive ? "Continue" : "Start";

  return (
    <div className="glass relative flex flex-col overflow-hidden rounded-xl transition-all hover:scale-[1.01] hover:shadow-lg">
      <div className="h-1 w-full" style={{ backgroundColor: color }} />
      <div className="flex flex-col gap-4 p-5">
        <div className="flex items-center gap-2">
          <BookOpen size={18} style={{ color }} />
          <h3 className="line-clamp-1 text-sm font-semibold text-foreground">
            {track.topic}
          </h3>
        </div>

        <p className="line-clamp-2 text-sm text-muted-foreground">
          {track.goal}
        </p>

        <BuyerAttributionLine
          buyerName={attribution.buyerName}
          orderId={attribution.orderId}
        />

        {isActive && (
          <div className="space-y-1.5">
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>{Math.round(track.progressPercent)}% complete</span>
            </div>
            <div className="h-2 rounded-full bg-secondary">
              <div
                className="h-2 rounded-full transition-all"
                style={{
                  width: `${Math.round(track.progressPercent)}%`,
                  backgroundColor: color,
                }}
              />
            </div>
          </div>
        )}

        <button
          type="button"
          onClick={handleAction}
          disabled={starting}
          aria-label={`${actionLabel} ${track.topic}`}
          className="flex items-center justify-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {starting && <Loader2 size={14} className="animate-spin" />}
          {actionLabel}
        </button>

        {error && (
          <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-3">
            <div className="flex items-center gap-2 text-sm font-medium text-destructive">
              <AlertTriangle size={14} />
              {error}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
