import { useEffect, useRef, useState } from "react";
import { getLessonVideos, isYoutubeKeyConfigured, refreshLessonVideos } from "@/lib/tauri-commands";
import type { LessonVideo } from "@/types/videos";
import { RefreshCw, Maximize2, Search, X, Shuffle, Undo2 } from "lucide-react";

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
 *   block. `key` on the iframe includes the current video id so switching
 *   Replace/Back also remounts the player.
 * - Single video: backend returns at most 1 (VIDEO_RESULT_LIMIT=1).
 * - Replace button: re-runs discovery excluding the currently-shown video so
 *   the learner gets a different pick. New YouTube + LLM quota (same as
 *   Refresh). On empty result, keeps the current video.
 * - Back button: client-side only, pops the previous video from a history
 *   stack. No quota consumed. Absent when history is empty.
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
  const [history, setHistory] = useState<LessonVideo[]>([]);
  const [refreshing, setRefreshing] = useState(false);
  const [replacing, setReplacing] = useState(false);
  const [expanded, setExpanded] = useState(false);
  // null = key-check not yet resolved; true/false = resolved result.
  const [keyConfigured, setKeyConfigured] = useState<boolean | null>(null);
  // generating: true while the manual "Find a reference video" call is in flight.
  const [generating, setGenerating] = useState(false);
  // noneFound: true after a manual generate that returned an empty list.
  const [noneFound, setNoneFound] = useState(false);

  const cancelledRef = useRef(false);
  // Tracks the section this panel is CURRENTLY showing. Async Replace/Refresh
  // handlers capture the sectionId at call time and compare it against this ref
  // before writing state, so an in-flight operation from a PREVIOUS section that
  // resolves after navigation cannot overwrite the new section's video (WR-02).
  const currentSectionRef = useRef(sectionId);

  useEffect(() => {
    setVideo(null);
    setHistory([]);
    setGenerating(false);
    setNoneFound(false);
    let cancelled = false;
    cancelledRef.current = false;
    currentSectionRef.current = sectionId;

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

    // Check whether a YouTube key is stored so we can decide whether to show
    // the manual empty-state affordance (D-06: hidden when no key configured).
    isYoutubeKeyConfigured()
      .then((configured) => {
        if (!cancelled) {
          setKeyConfigured(configured);
        }
      })
      .catch(() => {
        // Fail-soft: treat IPC errors as "not configured".
        if (!cancelled) {
          setKeyConfigured(false);
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
    // D-06: feature fully hidden when no key is configured (or check not yet resolved).
    if (keyConfigured !== true) {
      return null;
    }

    // Key is configured — show a compact empty-state card with a generate button.
    async function handleGenerate() {
      if (generating) return;
      const issuedForSection = sectionId;
      setGenerating(true);
      try {
        const result = await refreshLessonVideos(moduleId, sectionId, sectionTitle);
        if (!cancelledRef.current && currentSectionRef.current === issuedForSection) {
          if (result.videos[0]) {
            setNoneFound(false);
            setVideo(result.videos[0]);
          } else {
            setNoneFound(true);
          }
        }
      } catch {
        // Fail-soft: keep the empty state on error.
      } finally {
        if (!cancelledRef.current && currentSectionRef.current === issuedForSection) {
          setGenerating(false);
        }
      }
    }

    const emptyMsg = noneFound
      ? "No relevant video found for this lesson."
      : "No reference video loaded for this lesson yet.";

    const btnLabel = noneFound ? "Try again" : "Find a reference video";

    return (
      <section
        data-testid="reference-video-panel"
        className="not-prose my-8 w-full overflow-hidden rounded-lg border border-border bg-secondary/20"
      >
        <div className="px-4 pt-4 pb-4">
          <h2 className="text-sm font-semibold text-foreground">Reference video</h2>
          <p className="mt-0.5 text-xs text-muted-foreground leading-relaxed">
            Optional — an extra resource for this lesson.
          </p>
          <p className="mt-3 text-xs text-muted-foreground">{emptyMsg}</p>
          <button
            type="button"
            data-testid="ref-vid-generate-btn"
            onClick={handleGenerate}
            disabled={generating}
            className="mt-3 inline-flex items-center gap-1.5 rounded-md bg-primary/10 px-3 py-1.5 text-xs font-medium text-primary transition-colors hover:bg-primary/20 disabled:opacity-50"
          >
            <Search size={12} className={generating ? "animate-spin" : ""} />
            {btnLabel}
          </button>
        </div>
      </section>
    );
  }

  async function handleRefresh() {
    if (refreshing || replacing) return;
    // Capture the section this operation was issued for. If the user navigates
    // away before it resolves, currentSectionRef will have moved on and we must
    // discard the result rather than write it into the new section's panel.
    const issuedForSection = sectionId;
    setRefreshing(true);
    try {
      const result = await refreshLessonVideos(moduleId, sectionId, sectionTitle);
      if (!cancelledRef.current && currentSectionRef.current === issuedForSection) {
        setVideo(result.videos[0] ?? null);
        setHistory([]);
      }
    } catch {
    } finally {
      if (!cancelledRef.current && currentSectionRef.current === issuedForSection) {
        setRefreshing(false);
      }
    }
  }

  async function handleReplace() {
    if (replacing || refreshing || !video) return;
    const currentVideo = video;
    // Capture the section this Replace was issued for (WR-02).
    const issuedForSection = sectionId;
    setReplacing(true);
    try {
      const result = await refreshLessonVideos(
        moduleId,
        sectionId,
        sectionTitle,
        currentVideo.videoId,
      );
      if (!cancelledRef.current && currentSectionRef.current === issuedForSection) {
        if (result.videos[0]) {
          // Push the current video onto history before swapping.
          setHistory((h) => [...h, currentVideo]);
          setVideo(result.videos[0]);
        }
        // If empty result: keep the current video as-is (no state change).
      }
    } catch {
    } finally {
      if (!cancelledRef.current && currentSectionRef.current === issuedForSection) {
        setReplacing(false);
      }
    }
  }

  function handleBack() {
    if (history.length === 0) return;
    const prev = history[history.length - 1];
    setHistory((h) => h.slice(0, -1));
    setVideo(prev);
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
          {history.length > 0 && (
            <button
              type="button"
              onClick={handleBack}
              aria-label="Go back to previous video"
              className="mt-0.5 flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              <Undo2 size={12} />
              Back
            </button>
          )}
          <button
            type="button"
            onClick={handleReplace}
            disabled={replacing || refreshing}
            aria-label="Replace with a different video"
            className="mt-0.5 flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
          >
            <Shuffle size={12} className={replacing ? "animate-spin" : ""} />
            Replace
          </button>
          <button
            type="button"
            onClick={handleRefresh}
            disabled={refreshing || replacing}
            aria-label="Refresh reference video"
            title="Fetch fresh videos from YouTube"
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
            key={`${sectionId}-${video.videoId}`}
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
