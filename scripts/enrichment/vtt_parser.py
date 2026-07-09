"""
VTT subtitle to plain text converter for the enrichment pipeline.

This module is stdlib-only (re). It is used exclusively by the yt-dlp fallback
path in transcript.py — the youtube-transcript-api primary path returns clean
text directly (no VTT overhead).

Security: No external input is written to disk here; this is a pure transform.
The yt-dlp fallback path that calls this already validates video_id before
constructing the URL (T-17-03).
"""
import re


def vtt_to_text(vtt_content: str) -> str:
    """Convert WebVTT subtitle content to clean deduplicated plain text.

    Handles YouTube auto-caption VTT quirks:
    1. Strips WEBVTT header, metadata lines (Key: Value), timestamp lines
       (containing '-->'), bare cue number lines, and blank lines.
    2. Strips inline HTML tags (auto-captions use <c> for word-level timing).
    3. Deduplicates overlapping sliding-window segments: YouTube auto-captions
       use overlapping time windows where each cue overlaps heavily with the
       previous. This produces repeated phrases if naively concatenated.
       Strategy: find the longest suffix of the previous accumulated text that
       is a prefix of the current cue, and append only the new portion.

    Args:
        vtt_content: Raw VTT file content as a string.

    Returns:
        Clean plain text with deduped content, joined with spaces.
        Returns empty string if no text lines are found.
    """
    lines = vtt_content.splitlines()
    raw_texts = []

    for line in lines:
        line = line.strip()

        # Skip: blank lines
        if not line:
            continue

        # Skip: WEBVTT header
        if line.startswith("WEBVTT"):
            continue

        # Skip: timestamp lines (contain -->)
        if "-->" in line:
            continue

        # Skip: metadata lines (Key: Value format, e.g. "Kind: captions")
        if re.match(r'^[A-Za-z][A-Za-z0-9-]*:', line):
            continue

        # Skip: bare cue number lines (digits only)
        if re.match(r'^\d+$', line):
            continue

        # Strip inline HTML tags (e.g., <c>, </c>, <00:00:01.000>)
        line = re.sub(r'<[^>]+>', '', line).strip()

        if line:
            raw_texts.append(line)

    if not raw_texts:
        return ""

    # Dedup overlapping segments using longest-common-suffix detection.
    # YouTube auto-captions use a sliding window: cue N+1 starts before cue N
    # ends, causing the overlapping portion to repeat verbatim.
    # Algorithm: for each new cue, find the longest suffix of the previous
    # accumulated result that is a prefix of the current cue. If the overlap
    # is more than 3 characters (threshold avoids false positives on short
    # common words), append only the non-overlapping new portion.
    result = []
    for text in raw_texts:
        if not result:
            result.append(text)
            continue

        prev = result[-1]
        overlap = ""

        # Find longest suffix of prev that is a prefix of text
        for i in range(1, min(len(prev), len(text)) + 1):
            if text.startswith(prev[-i:]):
                overlap = prev[-i:]

        if overlap and len(overlap) > 3:
            # Append only the new (non-overlapping) portion
            new_part = text[len(overlap):].strip()
            if new_part:
                result.append(new_part)
            # If new_part is empty, the cue is entirely contained in prev — skip it
        else:
            result.append(text)

    return " ".join(result)
