// Phase 08.2 (Cert Simplification + Gamification) — PackPicker preview.
//
// Replaces the Phase 6 "3 certifications available" preview (Associate /
// Practitioner / Professional) with the new model (D-19):
//   - "1 completion certificate available" headline
//   - "Progress milestones at 25/50/75/100%" subline
//   - Expandable rationale paragraph explaining how to earn the cert
//
// Static — no data fetching (D-02 thresholds are uniform across packs).
// Lucide icons + plain text. No emojis (D-10 preserved).

import { useState, type KeyboardEvent } from "react";
import { ChevronDown, ChevronUp, Trophy } from "lucide-react";

interface Props {
  /// Kept for future "track size" surface (e.g. "5 modules") but no longer
  /// drives the certifications count — that is always 1 (D-01 + D-19).
  moduleCount?: number;
}

const RATIONALE =
  "Earn a certificate by mastering 100% of modules (BKT mastery >= 0.7), " +
  "reaching an average mastery of 0.85 across the track, and passing every " +
  "lab marked as practically required. Progress milestones at 25/50/75% are " +
  "in-app badges that mark progress; the certificate is the formal " +
  "credential awarded at 100%.";

export function PackPickerCertPreview(_props: Props) {
  const [open, setOpen] = useState(false);

  const toggle = () => setOpen((o) => !o);

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggle();
    }
  };

  return (
    <div data-testid="pack-picker-cert-preview" className="text-xs">
      <button
        type="button"
        aria-expanded={open}
        aria-controls="cert-preview-rationale"
        aria-label="1 completion certificate available"
        onClick={toggle}
        onKeyDown={onKeyDown}
        className="inline-flex items-center gap-1 text-muted-foreground hover:text-foreground"
      >
        <Trophy className="h-3 w-3" aria-hidden />
        <span>1 completion certificate available</span>
        {open ? (
          <ChevronUp className="h-3 w-3" aria-hidden />
        ) : (
          <ChevronDown className="h-3 w-3" aria-hidden />
        )}
      </button>
      <div className="mt-1 text-muted-foreground">
        Progress milestones at 25/50/75/100%
      </div>
      {open && (
        <p
          id="cert-preview-rationale"
          data-testid="pack-picker-cert-preview-rationale"
          className="mt-2 pl-4 text-muted-foreground"
        >
          {RATIONALE}
        </p>
      )}
    </div>
  );
}
