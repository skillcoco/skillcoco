"""
Async lesson generation for the enrichment pipeline.

Implements ENR-02: generate transcript-grounded teaching lessons via the
Anthropic AsyncAnthropic SDK.

Key design decisions:
  - RESTRUCTURE, not rewrite: instructor's voice/analogies/examples preserved (D-05)
  - Length proportional to transcript (D-06): no padding, no summarizing
  - Truncation guard (E2): stop_reason != "end_turn" → ValueError → lands in failed[]
  - Per-lesson continue-on-failure (D-16): one lesson failing does not stop the batch
  - Content caching (D-14): (video_id, transcript_hash, prompt_version) key skips API
  - Payload size guard (ENR-04): payloadJson never exceeds 131072 UTF-8 bytes
  - BYO API key (D-17): ANTHROPIC_API_KEY from env, exit with clear msg if unset
  - Prompt caching: static system block marked cache_control: ephemeral (reduces cost)
"""
import asyncio
import json
import os
import sys
from pathlib import Path

from enrichment.content_cache import content_cache_path, read_content_cache, write_content_cache


# ---------------------------------------------------------------------------
# Model config (AI-SPEC §4 locked values)
# ---------------------------------------------------------------------------

MODEL = "claude-sonnet-4-6"
LESSON_PROMPT_VERSION = "lesson-v1"   # bump this when LESSON_SYSTEM_PROMPT changes (Pitfall 5)

# ---------------------------------------------------------------------------
# System prompt (D-05 grounding: RESTRUCTURE not rewrite, preserve voice)
# ---------------------------------------------------------------------------

LESSON_SYSTEM_PROMPT = """\
You are a curriculum writer reorganizing a video instructor's transcript into a \
clean, structured written lesson. Your task is to RESTRUCTURE, not rewrite: preserve \
the instructor's analogies, mental models, examples, and explanations — reorganize \
them with clear headings, bullet lists, and code blocks so a reader can learn \
without watching the video.

Rules:
- Do NOT add information not present in the transcript.
- Do NOT pad with generic advice. If the transcript is short, the lesson is short.
- Do NOT include a top-level # heading (the app renders the lesson title separately).
- Preserve the instructor's voice: if they use a kitchen analogy, keep it.
- Return ONLY the markdown content. No preamble, no "Here is the lesson:" opener.
- Length should be proportional to the transcript length (D-06).
"""


# ---------------------------------------------------------------------------
# API client factory (D-17: env key only, never log the value)
# ---------------------------------------------------------------------------

def get_anthropic_client(*, max_retries: int = 3, timeout: float = 120.0):
    """Build and return an AsyncAnthropic client using ANTHROPIC_API_KEY from env.

    Exits with a clear human-readable message if ANTHROPIC_API_KEY is not set.
    Never logs or prints the key value (T-17-02).
    """
    key = os.environ.get("ANTHROPIC_API_KEY")
    if not key:
        sys.exit("ERROR: ANTHROPIC_API_KEY environment variable is not set. "
                 "Export your Anthropic API key before running the enrichment pipeline.")
    from anthropic import AsyncAnthropic
    return AsyncAnthropic(api_key=key, max_retries=max_retries, timeout=timeout)


# ---------------------------------------------------------------------------
# Payload size guard (ENR-04: 131072-byte cap from pack-schema.json)
# ---------------------------------------------------------------------------

def _make_section_payload(markdown: str) -> str:
    """Return payloadJson string, loop-truncating markdown to fit the 131072-byte cap.

    The pack-schema.json maxLength on payloadJson is 131072 bytes (UTF-8).
    If the generated markdown exceeds this when serialized, shorten by 200 chars
    and retry until it fits.
    """
    while True:
        raw = json.dumps({"markdown": markdown}, ensure_ascii=False)
        if len(raw.encode("utf-8")) <= 131072:
            return raw
        markdown = markdown[:-200]   # shorten and retry


# ---------------------------------------------------------------------------
# Per-lesson API call (E2 truncation guard)
# ---------------------------------------------------------------------------

async def _call_api_for_lesson(
    client,
    video_id: str,
    transcript: str,
    semaphore: asyncio.Semaphore,
) -> str:
    """Call the Anthropic Messages API for a single lesson.

    Uses the static LESSON_SYSTEM_PROMPT marked cache_control: ephemeral so the
    system block is cached after the first call (reduces cost ~90× on subsequent calls).

    Asserts stop_reason == "end_turn" before using response.content[0].text (E2).
    """
    async with semaphore:
        response = await client.messages.create(
            model=MODEL,
            max_tokens=4096,
            temperature=0.3,
            system=[
                {
                    "type": "text",
                    "text": LESSON_SYSTEM_PROMPT,
                    "cache_control": {"type": "ephemeral"},  # prompt cache on static block
                }
            ],
            messages=[
                {
                    "role": "user",
                    "content": (
                        f"Restructure the following transcript into a teaching lesson.\n\n"
                        f"TRANSCRIPT:\n{transcript}"
                    ),
                }
            ],
        )
        # E2: stop_reason=max_tokens means truncated output — treat as failure
        if response.stop_reason != "end_turn":
            raise ValueError(
                f"Lesson generation truncated for video {video_id!r} "
                f"(stop_reason={response.stop_reason!r}). "
                f"Transcript may be too long; this lesson will remain video-only."
            )
        return response.content[0].text


# ---------------------------------------------------------------------------
# Batch generation entry point
# ---------------------------------------------------------------------------

async def generate_lessons(
    transcripts: dict[str, str],
    failed: list,
    *,
    concurrency: int = 5,
    client=None,
) -> dict[str, str]:
    """Generate teaching lessons for all transcripts in the dict.

    Args:
        transcripts: mapping of video_id → transcript text.
        failed: list to append (video_id, error_str) tuples for failures (D-16).
        concurrency: max simultaneous API calls (default 5, uses asyncio.Semaphore).
        client: AsyncAnthropic client to use (constructed from env key if None).

    Returns:
        dict of video_id → markdown string for successfully generated lessons.
        Lessons that fail (E2 truncation, API error) are absent from results
        and logged in failed[] instead. One lesson failing does not stop the batch (D-16).
    """
    if client is None:
        client = get_anthropic_client()

    sem = asyncio.Semaphore(concurrency)
    results: dict[str, str] = {}

    async def _generate_one(video_id: str, transcript: str) -> None:
        """Generate one lesson: check cache first, then call API, write cache on hit."""
        try:
            # D-14: cache-first — skip the API call if we already have the content
            cache_path: Path = content_cache_path(video_id, transcript, LESSON_PROMPT_VERSION)
            cached = read_content_cache(cache_path)
            if cached is not None:
                results[video_id] = cached["markdown"]
                return

            # Cache miss — call the API
            markdown = await _call_api_for_lesson(client, video_id, transcript, sem)
            # Write to content cache for future reruns (D-14)
            write_content_cache(cache_path, {"markdown": markdown})
            results[video_id] = markdown

        except Exception as exc:
            # D-16: per-lesson continue-on-failure — append to failed, never raise
            failed.append((video_id, str(exc)))

    await asyncio.gather(*[
        _generate_one(vid, txt) for vid, txt in transcripts.items()
    ])
    return results
