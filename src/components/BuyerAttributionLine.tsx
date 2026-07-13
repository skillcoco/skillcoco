// Phase 16 Plan 02 Task 1 — BuyerAttributionLine shared extract (D-07).
//
// The buyer-attribution line ("Licensed to {buyerName} · order #{orderId}")
// previously existed as inline JSX duplicated across
// SettingsCourseImportSection.tsx and RedeemLicenseFlow.tsx. This is the
// third call site (LibraryPackCard, wired in Task 2) — per UI-SPEC's
// "extend, don't duplicate" rule, this shared presentational component
// becomes the single source of that copy across all three call sites.
// TrackView.tsx's existing inline attribution is refactored onto this
// component in 16-03 Task 4 (deliberately NOT done here).
//
// Copy/markup is identical to SettingsCourseImportSection.tsx lines 178-182 —
// no new copy introduced.

export interface BuyerAttributionLineProps {
  buyerName?: string;
  orderId?: string;
}

export function BuyerAttributionLine({
  buyerName,
  orderId,
}: BuyerAttributionLineProps) {
  if (!buyerName || !orderId) return null;

  return (
    <p className="text-xs text-muted-foreground">
      Licensed to {buyerName} · order #{orderId}
    </p>
  );
}
