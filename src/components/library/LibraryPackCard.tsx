// Phase 16 Plan 02 Task 2 — LibraryPackCard (D-07/D-09/D-10/D-11).
//
// Owned/imported pack card. Extends TrackCard.tsx's visual shape (colored
// top stripe via getTrackColor, .glass surface, p-5, progress bar) but:
//   - D-11 — pack removal is out of Phase 16 scope, so unlike TrackCard
//     (Dashboard) this card renders no destructive/removal affordance.
//   - Labels by actual status (WR-06): active/onboarding -> Continue (same
//     filter Dashboard.tsx uses for "in progress"), completed -> Review,
//     paused/archived -> Resume. Every import creates a status='active'
//     track (import_course_txn), so no "not-yet-started" state exists — the
//     action is always a plain navigate into the existing TrackView (D-10,
//     one track per pack), which cannot fail: no spinner/error machinery.

import { useNavigate } from "react-router-dom";
import { BookOpen } from "lucide-react";
import type { LearningTrack } from "@/types";
import { getTrackColor } from "@/lib/track-colors";

interface LibraryPackCardProps {
  track: LearningTrack;
}

export function LibraryPackCard({ track }: LibraryPackCardProps) {
  const navigate = useNavigate();

  const isActive = track.status === "active" || track.status === "onboarding";
  const color = getTrackColor(track.topic);

  // WR-06 — the LearningTrack row already exists for every owned pack, so
  // the action is a plain client-side navigate (D-09/D-10). navigate()
  // cannot fail: no spinner, no error state.
  function handleAction() {
    navigate(`/track/${track.id}`);
  }

  const actionLabel =
    track.status === "completed" ? "Review" : isActive ? "Continue" : "Resume";

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
          aria-label={`${actionLabel} ${track.topic}`}
          className="flex items-center justify-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90"
        >
          {actionLabel}
        </button>
      </div>
    </div>
  );
}
