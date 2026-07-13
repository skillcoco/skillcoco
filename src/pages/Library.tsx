// Phase 16 Plan 02 Task 3 — Library page (LIB-01/LIB-02/LIB-03/LIB-04).
//
// Unifies owned/imported packs, bundled starter packs, and the redeem entry
// point into one /library sibling route (D-02 — Dashboard stays default).
// Layout order (16-UI-SPEC.md): page header -> "Your packs" (header row +
// grid/empty-state, active-first per D-06) -> "Starter packs" (grid) ->
// "Redeem a license key" (verbatim RedeemLicenseFlow re-mount, D-04).
//
// 16-03 owns wiring LibraryImportSection below the Redeem section — see the
// marked mount point at the bottom of this file.

import { useEffect } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Plus } from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { useLibraryStore } from "@/stores/useLibraryStore";
import { LibraryPackCard } from "@/components/library/LibraryPackCard";
import { StarterPackCard } from "@/components/library/StarterPackCard";
import { LibraryEmptyState } from "@/components/library/LibraryEmptyState";
import { RedeemLicenseFlow } from "@/components/RedeemLicenseFlow";

export function Library() {
  const navigate = useNavigate();
  const tracks = useLearningStore((s) => s.tracks);
  const loadTracks = useLearningStore((s) => s.loadTracks);
  const starterPacks = useLibraryStore((s) => s.starterPacks);
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
          Manage the courses you own, redeem a license, or pick up something
          new.
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
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {starterPacks.map((pack) => (
            <StarterPackCard key={pack.id} pack={pack} />
          ))}
        </div>
      </div>

      {/* Redeem a license key — LIB-03 (redeem half), D-04 verbatim re-mount */}
      <div className="space-y-4">
        <h2 className="text-lg font-semibold text-foreground">
          Redeem a license key
        </h2>
        <div className="glass rounded-xl p-5">
          <RedeemLicenseFlow
            onImported={(trackId) => navigate(`/track/${trackId}`)}
          />
        </div>
      </div>

      {/* 16-03 mount point — LibraryImportSection (import course file) is
          wired here in the next plan; intentionally not added yet. */}
    </div>
  );
}
