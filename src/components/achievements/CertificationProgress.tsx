// Phase 6 (Certification) — Plan 06-01 (Wave 0) TrackView progress stub.
//
// Wave 4 (Plan 06-05) implements per D-11:
//   - Reads getTrackCertifications(trackId) on mount
//   - Renders earned levels (Associate / Practitioner / Professional)
//   - Shows criteria for the next unearned level
//
// Wave 0 stub — Wave 4 fills.

interface Props {
  trackId: string;
}

export function CertificationProgress({ trackId }: Props) {
  void trackId; // Wave 4 reads via getTrackCertifications(trackId).
  return null;
}
