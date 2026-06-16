// Phase 6 (Certification) — Plan 06-01 (Wave 0) PackPicker preview stub.
//
// Wave 4 (Plan 06-05) implements per D-10:
//   - Inline "3 certifications available" line on each PackPicker tile
//   - Expand on click to show Associate / Practitioner / Professional
//     names + criteria
//   - No emoji per PROJECT.md
//
// Phase 6 thresholds are uniform across packs (D-02), so this component
// computes from `pack.modules.length` at render time — no IPC needed.

interface Props {
  packId: string;
}

export function PackPickerCertPreview({ packId }: Props) {
  void packId; // Wave 4 reads pack module count + renders the preview.
  return null;
}
