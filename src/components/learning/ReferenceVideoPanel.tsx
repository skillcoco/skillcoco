import { useEffect, useRef, useState } from "react";
import { getLessonVideos, refreshLessonVideos } from "@/lib/tauri-commands";
import type { LessonVideo } from "@/types/videos";
import { RefreshCw } from "lucide-react";

/**
 * Phase 11 (acceptance revision) — Single Reference Video Hero
 *
 * Renders ONE highly-relevant, moderate-length (≤ 10 min) reference video
 * above the lesson text as an OPTIONAL supplementary resource. The sequenced
 * text lesson below is the primary, generated curriculum content; this video
 * is an additional learning resource the learner may choose to watch first.
 *
 * Design decisions (acceptance):
 * - Per-SECTION keying: re-fetches independently for every lesson/section
 *   block (fixes stale-iframe on Next-lesson clicks). `key={sectionId}` on
 *   the iframe forces a full remount so no prior video lingers.
 * - Single video: backend returns at most 1 (VIDEO_RESULT_LIMIT=1). We render
 *   0 or 1 — never a list.
 * - Hero placement: rendered ABOVE BlockRenderer (ModuleView mounts this
 *   before the lesson content div, not in the footer).
 * - D-07: youtube-nocookie.com embed only — no dangerouslySetInnerHTML.
 * - D-09: empty result (no key, quota exceeded, nothing above threshold,
 *   duration over cap) → return null (clean suppression, no error UI).
 * - D-06: backend silently returns [] when no YouTube key configured.
 * - D-04 (revised): manual Refresh button scoped to this section only.
 */

interface ReferenceVideoPanelProps {
  moduleId: string;
  sectionId: string;
  sectionTitle: string;
}

export function ReferenceVideoPanel({
  moduleId,
  sectionId,
  sectionTitle,
}: ReferenceVideoPanelProps) {
  const [video, setVideo] = useState<LessonVideo | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  // cancelledRef prevents state updates after unmount or sectionId change —
  // mirrors the MermaidBlock `let cancelled = false` effect-local guard.
  const cancelledRef = useRef(false);

  useEffect(() => {
    // Reset on every section change so stale video doesn't flash.
    setVideo(null);
    let cancelled = false;
    cancelledRef.current = false;

    getLessonVideos(moduleId, sectionId, sectionTitle)
      .then((result) => {
        if (!cancelled) {
          // Backend caps at 1; take the first (best) or null.
          setVideo(result.videos[0] ?? null);
        }
      })
      .catch(() => {
        // D-09: any error → empty → render null below. Never surface error UI.
        if (!cancelled) {
          setVideo(null);
        }
      });

    return () => {
      cancelled = true;
      cancelledRef.current = true;
    };
  // Re-fetch independently for every section (acceptance: per-lesson cache).
  }, [moduleId, sectionId, sectionTitle]);

  // D-09: no video (no key, quota exceeded, nothing above threshold) → nothing.
  if (!video) {
    return null;
  }

  async function handleRefresh() {
    if (refreshing) return;
    setRefreshing(true);
    try {
      const result = await refreshLessonVideos(moduleId, sectionId, sectionTitle);
      if (!cancelledRef.current) {
        setVideo(result.videos[0] ?? null);
      }
    } catch {
      // D-09: refresh error → keep current video (don't wipe what's rendered)
    } finally {
      if (!cancelledRef.current) {
        setRefreshing(false);
      }
    }
  }

  return (
    <section
      data-testid="reference-video-panel"
      className="not-prose my-8 mx-auto max-w-lg overflow-hidden rounded-lg border border-border bg-secondary/20"
    >
      {/* Header: "Reference video" label + optional sub-copy + Refresh button */}
      <div className="flex items-start justify-between gap-3 px-4 pt-4 pb-2">
        <div>
          <h2 className="text-sm font-semibold text-foreground">Reference video</h2>
          <p className="mt-0.5 text-xs text-muted-foreground leading-relaxed">
            Optional — an extra resource for this lesson. Your text lesson below is
            the primary, sequenced content.
          </p>
        </div>
        {/* D-04 (revised): manual refresh scoped to this section only */}
        <button
          type="button"
          onClick={handleRefresh}
          disabled={refreshing}
          aria-label="Refresh reference video"
          className="mt-0.5 flex shrink-0 items-center gap-1.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
        >
          <RefreshCw size={12} className={refreshing ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Embed: youtube-nocookie.com only (D-07). key=sectionId forces iframe
          remount on lesson change so the prior video is never stuck. */}
      <div className="relative w-full" style={{ paddingTop: "56.25%" }}>
        <iframe
          key={sectionId}
          src={`https://www.youtube-nocookie.com/embed/${video.videoId}`}
          title={video.title}
          allow="accelerometer; autoplay; clipboard-write; encrypted-media; fullscreen; gyroscope; picture-in-picture"
          allowFullScreen
          className="absolute inset-0 h-full w-full border-0"
          loading="lazy"
        />
      </div>

      {/* Video metadata: title + channel */}
      <div className="px-4 py-3">
        <p className="text-xs font-medium text-foreground line-clamp-2">{video.title}</p>
        <p className="mt-0.5 text-xs text-muted-foreground">{video.channelTitle}</p>
      </div>
    </section>
  );
}
