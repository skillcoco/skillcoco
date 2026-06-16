// Phase 6 (Certification) — Plan 06-05 (Wave 4) PackPicker preview.
//
// Per D-10 + CERT-10: tiny "3 certifications available" preview shown on
// each PackPicker tile. Click to expand and reveal Associate /
// Practitioner / Professional names + criteria text. The text is STATIC
// — Phase 6 thresholds are uniform per D-02 so the component does no
// data fetching at all (no IPC, no network, no pack-specific lookups).
// No emojis per D-10. Lucide icons + plain text only.

import { useState, type KeyboardEvent } from "react";
import { ChevronDown, ChevronUp, Trophy } from "lucide-react";

interface Props {
  /// Optional — kept for future "track size" surface (e.g. "5 modules,
  /// 3 certifications available"). Currently the count is fixed at 3
  /// because D-02 thresholds are uniform across all packs.
  moduleCount?: number;
}

interface LevelSpec {
  name: "Associate" | "Practitioner" | "Professional";
  criteria: string;
}

// D-02 hardcoded criteria — uniform across all packs. Per-pack
// configurability is deferred per 06-CONTEXT.md.
const LEVELS: LevelSpec[] = [
  { name: "Associate", criteria: "Master 25% of modules" },
  { name: "Practitioner", criteria: "Master 60% of modules" },
  {
    name: "Professional",
    criteria:
      "Master 100% of modules + 0.85 average mastery (and any required practical labs)",
  },
];

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
        aria-controls="cert-preview-list"
        aria-label="3 certifications available"
        onClick={toggle}
        onKeyDown={onKeyDown}
        className="inline-flex items-center gap-1 text-muted-foreground hover:text-foreground"
      >
        <Trophy className="h-3 w-3" aria-hidden />
        <span>3 certifications available</span>
        {open ? (
          <ChevronUp className="h-3 w-3" aria-hidden />
        ) : (
          <ChevronDown className="h-3 w-3" aria-hidden />
        )}
      </button>
      {open && (
        <ul
          id="cert-preview-list"
          data-testid="pack-picker-cert-preview-list"
          className="mt-2 space-y-1 pl-4"
        >
          {LEVELS.map((l) => (
            <li key={l.name} className="text-muted-foreground">
              <span className="font-medium text-foreground">{l.name}</span>{" "}
              — {l.criteria}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
