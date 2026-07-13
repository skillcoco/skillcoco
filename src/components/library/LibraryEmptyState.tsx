// Phase 16 Plan 02 Task 2 — LibraryEmptyState (D-08).
//
// Renders inside "Your packs" only when the owned-pack list is empty.
// Lighter-touch than Dashboard's full onboarding hero (py-12 vs py-20) —
// D-08 explicitly rules out a duplicate onboarding hero here. No CTA
// buttons: guidance copy points at the Redeem/Import + Starter-pack
// sections that render below it on the same page.

import { BookOpen } from "lucide-react";

export function LibraryEmptyState() {
  return (
    <div className="glass flex flex-col items-center justify-center rounded-xl py-12 text-center">
      <BookOpen className="mb-4 text-muted-foreground" size={40} />
      <h3 className="text-lg font-semibold text-foreground">No packs yet</h3>
      <p className="mt-1 max-w-sm text-sm text-muted-foreground">
        Redeem a license key, import a course file, or pick a starter pack
        below to get going.
      </p>
    </div>
  );
}
