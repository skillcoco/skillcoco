import { useEffect, useRef, useState } from "react";
import { getLessonVideos, refreshLessonVideos } from "@/lib/tauri-commands";
import type { LessonVideo } from "@/types/videos";
import { RefreshCw } from "lucide-react";

/**
 * Phase 11 — Video-Enriched Lessons (D-01/D-02/D-04/D-06/D-07/D-09)
 *
 * Lazy-fetches related videos for the active lesson module on first mount.
 * Renders youtube-nocookie.com embed iframes (D-07 — nocookie domain enforced,
 * no inline HTML injection). Returns null on any empty result or error
 * (D-09 silent suppression — no empty state, no error UI shown to learner).
 * When no YouTube key is configured the backend returns an empty list, so
 * the panel self-suppresses without needing key-status IPC (D-06).
 */

interface RelatedVideosPanelProps {
  moduleId: string;
}

export function RelatedVideosPanel({ moduleId }: RelatedVideosPanelProps) {
  const [videos, setVideos] = useState<LessonVideo[]>([]);
  const [refreshing, setRefreshing] = useState(false);

  // cancelledRef prevents state updates after unmount or moduleId change —
  // mirrors the MermaidBlock `let cancelled = false` effect-local guard.
  const cancelledRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    cancelledRef.current = false;

    getLessonVideos(moduleId)
      .then((result) => {
        if (!cancelled) {
          setVideos(result.videos);
        }
      })
      .catch(() => {
        // D-09: any error → empty list → render null below. Never surface error UI.
        if (!cancelled) {
          setVideos([]);
        }
      });

    return () => {
      cancelled = true;
      cancelledRef.current = true;
    };
  }, [moduleId]);

  // D-09: empty list (no key, quota exceeded, nothing above threshold) → render nothing.
  if (videos.length === 0) {
    return null;
  }

  async function handleRefresh() {
    if (refreshing) return;
    setRefreshing(true);
    try {
      const result = await refreshLessonVideos(moduleId);
      if (!cancelledRef.current) {
        setVideos(result.videos);
      }
    } catch {
      // D-09: refresh error → keep current list (don't wipe what's already rendered)
    } finally {
      if (!cancelledRef.current) {
        setRefreshing(false);
      }
    }
  }

  return (
    <section
      data-testid="related-videos-panel"
      className="mt-8 space-y-4 border-t border-border pt-6"
    >
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-foreground">Related videos</h2>
        {/* D-04: manual refresh only — no auto-polling */}
        <button
          type="button"
          onClick={handleRefresh}
          disabled={refreshing}
          aria-label="Refresh related videos"
          className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
        >
          <RefreshCw size={12} className={refreshing ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      <div className="space-y-5">
        {/* D-02: backend already caps at VIDEO_RESULT_LIMIT=3; render all returned */}
        {videos.map((v) => (
          <div
            key={v.videoId}
            className="overflow-hidden rounded-md border border-border bg-secondary/20"
          >
            {/* D-07: youtube-nocookie.com only — nocookie domain enforced */}
            <div className="relative w-full" style={{ paddingTop: "56.25%" }}>
              <iframe
                src={`https://www.youtube-nocookie.com/embed/${v.videoId}`}
                title={v.title}
                allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                allowFullScreen
                className="absolute inset-0 h-full w-full border-0"
                loading="lazy"
              />
            </div>
            <div className="px-3 py-2">
              <p className="text-xs font-medium text-foreground line-clamp-2">{v.title}</p>
              <p className="mt-0.5 text-xs text-muted-foreground">{v.channelTitle}</p>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
