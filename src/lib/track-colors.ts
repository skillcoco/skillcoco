// WR-07 — single canonical track-color helper.
//
// Replaces four divergent copies (Sidebar.tsx, TrackView.tsx, TrackCard.tsx,
// LibraryPackCard.tsx). The Sidebar copy lacked the k8s/golang aliases, and
// every copy false-matched "go" as a bare substring — painting "Django",
// "MongoDB", "Google Cloud", and "Algorithms" as Go tracks. Go now matches
// only as a whole word (or the "golang" alias).

/** Map a track topic to its accent color CSS value. */
export function getTrackColor(topic: string): string {
  const key = topic.toLowerCase();
  if (key.includes("kubernetes") || key.includes("k8s"))
    return "hsl(var(--track-kubernetes))";
  if (key.includes("rust")) return "hsl(var(--track-rust))";
  if (/\bgo(lang)?\b/.test(key)) return "hsl(var(--track-go))";
  if (key.includes("python")) return "hsl(var(--track-python))";
  return "hsl(var(--primary))";
}
