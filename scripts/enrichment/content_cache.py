"""
File-based content cache for the enrichment pipeline.

Cache layout (per D-04 / D-14):
  ~/.learnforge/transcripts/
  ├── {video_id}.txt                         # raw transcript (D-04, keyed by video_id)
  └── {video_id}_{hash8}_{prompt_ver}.json   # generated lesson/content (D-14)

  quiz cache:
  ~/.learnforge/transcripts/
  └── quiz_{module_id}_{hash8}_{prompt_ver}.json  # generated quiz (D-14, keyed by module_id)

Security: video_id values are validated against ^[A-Za-z0-9_-]{6,20}$ before
any Path join (T-17-01 path-traversal mitigation).

Cache directory is overrideable for tests:
  1. Set LEARNFORGE_TRANSCRIPTS_DIR env var before import, OR
  2. Monkeypatch `content_cache.CACHE_DIR` in tests (the conftest fixture does this).
"""
import hashlib
import json
import os
import re
from pathlib import Path


# ---------------------------------------------------------------------------
# Cache directory — overrideable via env var or monkeypatch
# ---------------------------------------------------------------------------

_env_override = os.environ.get("LEARNFORGE_TRANSCRIPTS_DIR")
CACHE_DIR: Path = Path(_env_override) if _env_override else Path.home() / ".learnforge" / "transcripts"


# ---------------------------------------------------------------------------
# video_id safety guard (T-17-01)
# ---------------------------------------------------------------------------

VIDEO_ID_RE = re.compile(r"^[A-Za-z0-9_-]{6,20}$")


def _safe_video_id(video_id: str) -> str:
    """Validate video_id against ^[A-Za-z0-9_-]{6,20}$ before any Path join.

    Raises ValueError for any value that does not match, preventing path
    traversal attacks (T-17-01) when video_id originates from an xlsx cell.
    """
    if not VIDEO_ID_RE.match(video_id):
        raise ValueError(
            f"Invalid video_id for cache path: {video_id!r} — "
            f"must match ^[A-Za-z0-9_-]{{6,20}}$"
        )
    return video_id


# ---------------------------------------------------------------------------
# Path helpers
# ---------------------------------------------------------------------------

def transcript_cache_path(video_id: str) -> Path:
    """Return the cache path for a raw transcript.

    Keyed by video_id per D-04 (global transcript cache).
    Validates video_id against the safety guard before joining.
    """
    _safe_video_id(video_id)
    return CACHE_DIR / f"{video_id}.txt"


def content_cache_path(video_id: str, transcript: str, prompt_version: str) -> Path:
    """Return the cache path for AI-generated lesson content.

    Key = (video_id, sha256(transcript)[:8], prompt_version) per D-14.
    Validates video_id against the safety guard before joining.
    """
    _safe_video_id(video_id)
    h = hashlib.sha256(transcript.encode()).hexdigest()[:8]
    return CACHE_DIR / f"{video_id}_{h}_{prompt_version}.json"


def quiz_cache_path(module_id: str, transcripts: list[str], prompt_version: str) -> Path:
    """Return the cache path for an AI-generated quiz.

    Key = (module_id, sha256("".join(transcripts))[:8], prompt_version) per D-14.
    module_id is not a video_id (it may contain hyphens in any position) but
    we do not apply the VIDEO_ID_RE guard here — module_id comes from a
    controlled internal source (slug derived from the xlsx module number).
    """
    combined = "".join(transcripts)
    h = hashlib.sha256(combined.encode()).hexdigest()[:8]
    return CACHE_DIR / f"quiz_{module_id}_{h}_{prompt_version}.json"


# ---------------------------------------------------------------------------
# Transcript read/write
# ---------------------------------------------------------------------------

def read_transcript_cache(video_id: str) -> str | None:
    """Read a cached transcript by video_id. Returns None if not cached."""
    path = transcript_cache_path(video_id)
    if not path.exists():
        return None
    return path.read_text(encoding="utf-8")


def write_transcript_cache(video_id: str, text: str) -> None:
    """Write a transcript to the cache. Creates parent directories as needed."""
    path = transcript_cache_path(video_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


# ---------------------------------------------------------------------------
# Content (lesson / quiz JSON) read/write
# ---------------------------------------------------------------------------

def read_content_cache(path: Path) -> dict | None:
    """Read a cached JSON content file. Returns None if path does not exist."""
    if not path.exists():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def write_content_cache(path: Path, data: dict) -> None:
    """Write a dict to a JSON cache file. Creates parent directories as needed.

    Writes with ensure_ascii=False so Unicode characters (e.g., instructor names,
    non-English examples) are stored as-is rather than as \\uXXXX escapes.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
