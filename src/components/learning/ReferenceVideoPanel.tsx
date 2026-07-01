import { useEffect, useRef, useState } from "react";
import { getLessonVideos, refreshLessonVideos } from "@/lib/tauri-commands";
import type { LessonVideo } from "@/types/videos";
import { RefreshCw, Maximize2, X } from "lucide-react";

/**
 * Phase 11 (acceptance revision) — Single Reference Video
 *
 * Renders ONE highly-relevant, moderate-length (≤ 10 min) reference video
 * as an OPTIONAL supplementary resource, injected into the MIDDLE of the
 * lesson text (via SectionBlock's referenceSlot). The sequenced text lesson
 * is the primary, generated curriculum content; this video is an additional
 * learning resource the learner may discover mid-read.
 *
 * Design decisions (acceptance):
 * - Full content width — the card spans the same width as the lesson text
 *   above/below it (no max-width cap).
 * - Per-SECTION keying: re-fetches independently for every lesson/section
 *   block. `key={sectionId}` on the iframe forces a full remount so no prior
 *   video lingers on Next-lesson clicks.
 * - Single video: backend returns at most 1 (VIDEO_RESULT_LIMIT=1).
 * - Custom Expand: YouTube's native fullscreen button is unreliable inside
 *   Tauri's macOS WKWebView, so we provide our own Expand control that blows
 *   the player up to a large in-app overlay. The SAME iframe node stays
 *   mounted across the toggle (only its container's classes change), so the
 *   video does not restart and there is no double-audio. Esc / backdrop /
 *   close button collapse it.
 * - D-07: youtube-nocookie.com embed only — no dangerouslySetInnerHTML.
 * - D-09: empty result → return null (clean suppression, no error UI).
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
  const [expanded, setExpanded] = useState(false);

  const cancelledRef = useRef(false);

  useEffect(() => {
    setVideo(null);
    let cancelled = false;
    cancelledRef.current = false;

    getLessonVideos(moduleId, sectionId, sectionTitle)
      .then((result) => {
        if (!cancelled) {
          setVideo(result.videos[0] ?? null);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setVideo(null);
        }
      });

    return () => {
      cancelled = true;
      cancelledRef.current = true;
    };
  }, [moduleId, sectionId, sectionTitle]);

  // Collapse the expanded overlay on Escape.
  useEffect(() => {
    if (!expanded) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setExpanded(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [expanded]);

  // Collapse if the section changes while expanded.
  useEffect(() => {
    setExpanded(false);
  }, [sectionId]);

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
    } finally {
      if (!cancelledRef.current) {
        setRefreshing(false);
      }
    }
  }

  return (
    <section
      data-testid="reference-video-panel"
      className="not-prose my-8 w-full overflow-hidden rounded-lg border border-border bg-secondary/20"
    >
      <div className="flex items-start justify-between gap-3 px-4 pt-4 pb-2">
        <div>
          <h2 className="text-sm font-semibold text-foreground">Reference video</h2>
          <p className="mt-0.5 text-xs text-muted-foreground leading-relaxed">
            Optional — an extra resource for this lesson.
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <button
            type="button"
            onClick={() => setExpanded(true)}
            aria-label="Expand reference video"
            className="mt-0.5 flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <Maximize2 size={12} />
            Expand
          </button>
          <button
            type="button"
            onClick={handleRefresh}
            disabled={refreshing}
            aria-label="Refresh reference video"
            className="mt-0.5 flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
          >
            <RefreshCw size={12} className={refreshing ? "animate-spin" : ""} />
            Refresh
          </button>
        </div>
      </div>

      {/* Backdrop for the expanded overlay (sibling of the player so the iframe
          node stays put and does not remount when expanding). */}
      {expanded && (
        <div
          data-testid="reference-video-backdrop"
          onClick={() => setExpanded(false)}
          className="fixed inset-0 z-40 bg-black/80"
          aria-hidden="true"
        />
      )}

      {/* Player wrapper. Collapsed: inline 16:9 box at full content width.
          Expanded: fixed, centered, large. The iframe below is the SAME node in
          both states (only these classes change) → no remount, no restart. */}
      <div
        data-testid="reference-video-player"
        data-expanded={expanded ? "true" : "false"}
        className={
          expanded
            ? "fixed left-1/2 top-1/2 z-50 w-[min(92vw,1200px)] -translate-x-1/2 -translate-y-1/2"
            : "w-full"
        }
      >
        {expanded && (
          <button
            type="button"
            onClick={() => setExpanded(false)}
            aria-label="Close expanded video"
            className="absolute -top-10 right-0 z-10 flex items-center gap-1.5 rounded-md bg-black/60 px-2.5 py-1.5 text-xs font-medium text-white transition-colors hover:bg-black/80"
          >
            <X size={14} />
            Close
          </button>
        )}
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
      </div>

      <div className="px-4 py-3">
        <p className="text-xs font-medium text-foreground line-clamp-2">{video.title}</p>
        <p className="mt-0.5 text-xs text-muted-foreground">{video.channelTitle}</p>
      </div>
    </section>
  );
}
