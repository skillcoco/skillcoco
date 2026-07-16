// Phase 16 Plan 02 Task 3 — Library page (LIB-01/LIB-02/LIB-03/LIB-04).
//
// Unifies owned/imported packs, bundled starter packs, and course import
// into one /library sibling route (D-02 — Dashboard stays default).
// Layout order (16-UI-SPEC.md): page header -> "Your packs" (header row +
// grid/empty-state, active-first per D-06) -> "Starter packs" (grid) ->
// "Import a course file".

import { useEffect } from "react";
import { Link } from "react-router-dom";
import { Plus } from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { useLibraryStore } from "@/stores/useLibraryStore";
import { LibraryPackCard } from "@/components/library/LibraryPackCard";
import { StarterPackCard } from "@/components/library/StarterPackCard";
import { LibraryEmptyState } from "@/components/library/LibraryEmptyState";
import { LibraryImportSection } from "@/components/library/LibraryImportSection";

export function Library() {
  const tracks = useLearningStore((s) => s.tracks);
  const loadTracks = useLearningStore((s) => s.loadTracks);
  const starterPacks = useLibraryStore((s) => s.starterPacks);
  const starterPacksLoading = useLibraryStore((s) => s.isLoading);
  const starterPacksError = useLibraryStore((s) => s.error);
  const loadStarterPacks = useLibraryStore((s) => s.loadStarterPacks);

  useEffect(() => {
    loadTracks().catch((err) => console.error("[Library] loadTracks failed:", err));
    loadStarterPacks().catch((err) =>
      console.error("[Library] loadStarterPacks failed:", err),
    );
  }, []);

  // D-06 — active packs (active/onboarding, matching Dashboard's in-progress
  // filter) sort first within "Your packs".
  const sortedTracks = [...tracks].sort((a, b) => {
    const aActive = a.status === "active" || a.status === "onboarding" ? 0 : 1;
    const bActive = b.status === "active" || b.status === "onboarding" ? 0 : 1;
    return aActive - bActive;
  });

  return (
    <div className="mx-auto max-w-6xl space-y-8 pb-12">
      <div>
        <h1 className="text-3xl font-bold text-foreground">Library</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Manage the courses you own or pick up something new.
        </p>
      </div>

      {/* Your packs — LIB-01, D-06/D-07/D-08/D-09/D-10 */}
      <div>
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-foreground">Your packs</h2>
          <Link
            to="/onboarding"
            className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90"
          >
            <Plus size={16} />
            New Track
          </Link>
        </div>

        {sortedTracks.length === 0 ? (
          <LibraryEmptyState />
        ) : (
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {sortedTracks.map((track) => (
              <LibraryPackCard key={track.id} track={track} />
            ))}
          </div>
        )}
      </div>

      {/* Starter packs — LIB-04, D-12/D-13 */}
      <div>
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-foreground">Starter packs</h2>
        </div>
        {/* WR-02 — surface the store's error/loading states instead of a bare
            header over an empty grid. */}
        {starterPacksError ? (
          <p className="text-xs text-destructive">
            Couldn't load starter packs: {starterPacksError}
          </p>
        ) : starterPacksLoading ? (
          <p className="text-xs text-muted-foreground">Loading starter packs...</p>
        ) : starterPacks.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            No starter packs available.
          </p>
        ) : (
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {starterPacks.map((pack) => (
              <StarterPackCard key={pack.id} pack={pack} />
            ))}
          </div>
        )}
      </div>

      {/* Import a course file — LIB-03 (import-file half), relocated from
          Settings (D-03) */}
      <div className="space-y-4">
        <h2 className="text-lg font-semibold text-foreground">
          Import a course file
        </h2>
        <div className="glass rounded-xl p-5">
          <LibraryImportSection />
        </div>
      </div>
    </div>
  );
}
