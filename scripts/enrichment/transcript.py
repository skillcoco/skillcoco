"""
YouTube transcript fetcher with cache, yt-dlp fallback, and skip policy.

Implements ENR-01: per-lesson transcript fetch + global video_id-keyed cache
at ~/.learnforge/transcripts/ + yt-dlp subtitle fallback on IP/request blocks
+ D-03 skip-on-no-captions without failing the run.

Security:
  - video_id validated by _safe_video_id (^[A-Za-z0-9_-]{6,20}$) before any
    filesystem path construction or URL building (T-17-03).
  - yt-dlp invoked as an argv list with shell=False (default) — no shell
    injection possible (T-17-03).
  - Sequential fetch with configurable delay; D-03 skip keeps batch alive on
    partial block (T-17-04).

Dependencies:
  - youtube-transcript-api>=1.2.4 (1.x instance API, not 0.x static methods)
  - yt-dlp>=2026.7.0 (system install or on PATH)
"""
import subprocess
import tempfile
import time
from pathlib import Path

from youtube_transcript_api import YouTubeTranscriptApi
from youtube_transcript_api._errors import (
    AgeRestricted,
    CouldNotRetrieveTranscript,
    IpBlocked,
    NoTranscriptFound,
    PoTokenRequired,
    RequestBlocked,
    TranscriptsDisabled,
    VideoUnavailable,
    VideoUnplayable,
)

from enrichment.content_cache import (
    read_transcript_cache,
    write_transcript_cache,
)
from enrichment.vtt_parser import vtt_to_text


# ---------------------------------------------------------------------------
# Rate-limit courtesy delay between network fetches (Pitfall 4)
# ---------------------------------------------------------------------------

TRANSCRIPT_DELAY: float = 0.5  # seconds; overrideable in tests via delay= param


# ---------------------------------------------------------------------------
# Primary fetch path: youtube-transcript-api 1.x
# ---------------------------------------------------------------------------

def _fetch_via_api(video_id: str) -> str | None:
    """Fetch transcript using youtube-transcript-api 1.x (manual-first, D-02).

    Must instantiate YouTubeTranscriptApi() — 1.x removed static methods (Pitfall 1).

    Returns:
        Joined plain text string, or None for D-03 skip-eligible exceptions.
    Raises:
        RequestBlocked, IpBlocked: caller must trigger yt-dlp fallback (D-01).
    """
    api = YouTubeTranscriptApi()  # 1.x: MUST instantiate (Pitfall 1 — no static methods)
    try:
        transcript_list = api.list(video_id)

        # D-02: prefer manually created captions; fall back to auto-generated
        try:
            t = transcript_list.find_manually_created_transcript(["en", "en-US", "en-GB"])
        except NoTranscriptFound:
            t = transcript_list.find_generated_transcript(["en", "en-US", "en-GB"])

        fetched = t.fetch()
        return " ".join(snip.text for snip in fetched).strip()

    except (
        TranscriptsDisabled,
        NoTranscriptFound,
        VideoUnavailable,
        VideoUnplayable,
        AgeRestricted,
        PoTokenRequired,
    ):
        return None  # D-03: skip this lesson — no captions available

    except (RequestBlocked, IpBlocked):
        raise  # D-01: caller triggers yt-dlp fallback

    except CouldNotRetrieveTranscript:
        return None  # Unknown caption issue — treat as no captions (D-03)


# ---------------------------------------------------------------------------
# Fallback fetch path: yt-dlp subtitle extraction
# ---------------------------------------------------------------------------

def fetch_transcript_via_ytdlp(video_id: str) -> str | None:
    """yt-dlp subtitle extraction fallback (D-01).

    Downloads .vtt subtitle files only — no video. Prefers manual captions;
    falls back to auto-generated. Passes output through vtt_to_text() to
    deduplicate sliding-window overlap artifacts.

    Security: video_id is already validated by content_cache._safe_video_id
    at path-construction time; the URL is built as a string literal +
    validated video_id (no user-controlled format strings). yt-dlp is invoked
    as an argv list with no shell=True (T-17-03).

    Returns:
        Cleaned plain text string, or None if yt-dlp fails or finds no VTT.
    """
    url = f"https://www.youtube.com/watch?v={video_id}"

    with tempfile.TemporaryDirectory() as tmp:
        cmd = [
            "yt-dlp",
            "--skip-download",
            "--write-subs",
            "--write-auto-subs",
            "--sub-format", "vtt",
            "--sub-langs", "en,en-US,en-GB",
            "--restrict-filenames",
            "--quiet",
            "--output", f"{tmp}/%(id)s.%(ext)s",
            url,
        ]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)

        if result.returncode != 0:
            return None

        # Find all written .vtt files
        vtt_files = sorted(Path(tmp).glob("*.vtt"))
        if not vtt_files:
            return None

        # Prefer manual captions (no 'auto' in filename) over auto-generated
        manual = [f for f in vtt_files if "auto" not in f.stem.lower()]
        chosen = manual[0] if manual else vtt_files[0]

        return vtt_to_text(chosen.read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# Per-lesson fetch with fallback and skip recording
# ---------------------------------------------------------------------------

def _fetch_with_fallback(
    video_id: str,
    skipped: list,
    failed: list,
) -> str | None:
    """Attempt API fetch, fall back to yt-dlp on block, record skip on no-captions.

    Args:
        video_id: Validated YouTube video ID.
        skipped: Caller-owned list; video_id appended on D-03 skip.
        failed:  Caller-owned list; video_id appended on unrecoverable error.

    Returns:
        Transcript text on success, or None (video_id already recorded).
    """
    try:
        text = _fetch_via_api(video_id)
        if text is None:
            # D-03: no captions — skip without failing the run
            skipped.append((video_id, "no captions"))
            return None
        return text

    except (RequestBlocked, IpBlocked):
        # D-01: try yt-dlp fallback
        try:
            text = fetch_transcript_via_ytdlp(video_id)
            if text is None:
                skipped.append((video_id, "yt-dlp: no subtitles found"))
            return text
        except Exception as exc:  # noqa: BLE001
            failed.append((video_id, f"yt-dlp error: {exc}"))
            return None

    except Exception as exc:  # noqa: BLE001
        # Unexpected error — record and continue (D-16: never raise from per-lesson loop)
        failed.append((video_id, str(exc)))
        return None


# ---------------------------------------------------------------------------
# Public entry point
# ---------------------------------------------------------------------------

def fetch_and_cache_transcripts(
    modules_raw: list[dict],
    skipped: list,
    *,
    delay: float = TRANSCRIPT_DELAY,
    failed: list | None = None,
) -> dict[str, str]:
    """Fetch and cache transcripts for all lessons in modules_raw.

    Iterates modules → lessons sequentially. For each lesson:
    1. Skip if no video_id (already warned by stage 1 / plan 17-06).
    2. Return cached transcript if present (D-04 — free reruns).
    3. Otherwise fetch via youtube-transcript-api 1.x (manual-first, D-02).
    4. On RequestBlocked/IpBlocked, fall back to yt-dlp (D-01).
    5. On no captions, record in skipped list (D-03).
    6. Apply time.sleep(delay) between non-cached fetches (Pitfall 4).
    7. Never raise — append to skipped/failed and continue (D-16).

    Args:
        modules_raw: List of module dicts (each with a 'lessons' list).
        skipped: Caller-owned list; (video_id, reason) tuples appended.
        delay:   Seconds to sleep between network fetches (0 in tests).
        failed:  Optional caller-owned list for unrecoverable errors.

    Returns:
        Dict mapping video_id → transcript text for all successfully
        fetched/cached lessons.
    """
    if failed is None:
        failed = []

    transcripts: dict[str, str] = {}
    need_sleep = False  # only sleep BETWEEN fetches, not before the first

    for module in modules_raw:
        for lesson in module.get("lessons", []):
            video_id = lesson.get("video_id")
            if not video_id:
                # No video_id — silently skip (already warned by stage 1)
                continue

            # D-04: check cache first
            cached = read_transcript_cache(video_id)
            if cached is not None:
                transcripts[video_id] = cached
                continue

            # Rate-limit courtesy: sleep between network fetches (Pitfall 4)
            if need_sleep and delay > 0:
                time.sleep(delay)
            need_sleep = True  # next non-cached fetch will sleep

            text = _fetch_with_fallback(video_id, skipped, failed)
            if text is not None:
                write_transcript_cache(video_id, text)
                transcripts[video_id] = text

    return transcripts
