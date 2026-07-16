// Phase 16 Plan 02 Task 2 — StarterPackCard (LIB-04/D-13).
//
// Free bundled starter tile. Same card shell/visual language as
// LibraryPackCard but lighter: "Free" pill (secondary, non-accent per the
// accent-reservation rule), single Start button that routes through
// startStarterPack (16-01's unchanged import gate) and navigates into the
// new TrackView on success. No progress bar — starter packs are bundled
// content, imported locally.

import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { BookOpen, Loader2, AlertTriangle } from "lucide-react";
import { startStarterPack, type StarterPackMeta } from "@/lib/tauri-commands";

interface StarterPackCardProps {
  pack: StarterPackMeta;
}

// WR-04 — fallback only: the backend's typed error messages (D-11 taxonomy)
// are user-facing plain language and are surfaced directly. The pointer no
// longer references Settings -> Import (removed this phase, D-03) — the
// import entry point now lives in the Library's own import section.
const START_ERROR_FALLBACK =
  "Couldn't start this pack. Try again, or use the import section below.";

function startErrorMessage(err: unknown): string {
  const msg =
    err instanceof Error ? err.message : typeof err === "string" ? err : "";
  return msg || START_ERROR_FALLBACK;
}

export function StarterPackCard({ pack }: StarterPackCardProps) {
  const navigate = useNavigate();
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleStart() {
    setStarting(true);
    setError(null);
    try {
      const result = await startStarterPack(pack.id);
      navigate(`/track/${result.trackId}`);
    } catch (err) {
      console.error("[StarterPackCard] startStarterPack failed:", err);
      setError(startErrorMessage(err));
    } finally {
      setStarting(false);
    }
  }

  return (
    <div className="glass relative flex flex-col overflow-hidden rounded-xl transition-all hover:scale-[1.01] hover:shadow-lg">
      <div className="h-1 w-full bg-secondary" />
      <div className="flex flex-col gap-4 p-5">
        <div className="flex items-center gap-2">
          <BookOpen size={18} className="text-muted-foreground" />
          <h3 className="line-clamp-1 text-sm font-semibold text-foreground">
            {pack.title}
          </h3>
          <span className="ml-auto rounded-full bg-secondary px-2.5 py-0.5 text-xs text-muted-foreground">
            Free
          </span>
        </div>

        <p className="line-clamp-2 text-sm text-muted-foreground">
          {pack.description}
        </p>

        <button
          type="button"
          onClick={handleStart}
          disabled={starting}
          aria-label={`Start ${pack.title}`}
          className="flex items-center justify-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {starting && <Loader2 size={14} className="animate-spin" />}
          Start
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
