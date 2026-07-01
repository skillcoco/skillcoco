/**
 * video-progress — module-level playback position store.
 *
 * Survives component remounts and lesson navigation within a single app
 * session. Also persists to localStorage so positions survive page reloads.
 *
 * Design:
 * - In-memory Map<videoId, seconds> as the primary store (fast, always available).
 * - localStorage under key `lf.videoProgress.<videoId>` as secondary persistence.
 * - All localStorage access is fail-soft (try/catch, never throws).
 * - Invalid inputs (negative, NaN, non-finite) are silently ignored.
 */

const _store = new Map<string, number>();
const LS_PREFIX = "lf.videoProgress.";

/**
 * Returns the last-known playback position for a videoId.
 *
 * On first access for a given key (not in the in-memory map), attempts to
 * hydrate from localStorage. Falls back to 0 on any error.
 */
export function getVideoProgress(videoId: string): number {
  if (_store.has(videoId)) {
    return _store.get(videoId)!;
  }

  // Hydrate from localStorage on cold read
  try {
    const raw = localStorage.getItem(LS_PREFIX + videoId);
    if (raw !== null) {
      const parsed = Number(raw);
      if (isFinite(parsed) && parsed >= 0) {
        _store.set(videoId, parsed);
        return parsed;
      }
    }
  } catch {
    // fail-soft: localStorage unavailable (e.g., private browsing, Tauri sandbox)
  }

  return 0;
}

/**
 * Stores the playback position for a videoId.
 *
 * Silently ignores non-finite or negative values.
 * Persists to localStorage best-effort (never throws).
 */
export function setVideoProgress(videoId: string, seconds: number): void {
  if (!isFinite(seconds) || seconds < 0) {
    return;
  }

  _store.set(videoId, seconds);

  try {
    localStorage.setItem(LS_PREFIX + videoId, String(seconds));
  } catch {
    // fail-soft: localStorage unavailable or quota exceeded
  }
}
