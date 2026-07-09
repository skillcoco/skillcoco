"""
RED scaffold for enrichment.transcript module tests.

These tests are RED stubs closed by plan 17-02 (transcript.py + vtt_parser.py).
Each test imports from the not-yet-existing target module inside the test body
and fails with a plan-naming message to guide the next executor.

Tests seed: D-01 (fetch), D-03 (no captions skip), D-04 (cache hit), sequential delay.
"""
import pytest


def test_cache_hit_skips_fetch(tmp_cache_dir):
    """Cache hit: write a transcript file, assert API fetch() not called.

    Pre-condition: transcript already cached for video_id.
    Assert: fetch_and_cache_transcripts returns the cached text without calling the API.

    RED — closed by 17-02 (transcript.py).
    """
    try:
        from enrichment.transcript import fetch_and_cache_transcripts  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-02 (transcript.py)")


def test_ipblocked_triggers_ytdlp_fallback(tmp_cache_dir, monkeypatch):
    """IpBlocked on api.list() → yt-dlp subprocess called as fallback.

    Pre-condition: YouTubeTranscriptApi.list() raises IpBlocked.
    Assert: yt-dlp subprocess is invoked; transcript extracted from VTT output.

    RED — closed by 17-02 (transcript.py).
    """
    try:
        from enrichment.transcript import fetch_and_cache_transcripts  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-02 (transcript.py)")


def test_no_captions_skips_lesson(tmp_cache_dir):
    """TranscriptsDisabled → lesson in skipped list, not in transcripts dict.

    Pre-condition: video has no captions (TranscriptsDisabled raised).
    Assert: video_id absent from returned transcripts dict; entry in skipped list (D-03).

    RED — closed by 17-02 (transcript.py).
    """
    try:
        from enrichment.transcript import fetch_and_cache_transcripts  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-02 (transcript.py)")


def test_sequential_delay_between_fetches(tmp_cache_dir, monkeypatch):
    """Sequential delay (TRANSCRIPT_DELAY) between API fetches.

    Pre-condition: 2 videos without cached transcripts.
    Assert: a delay is applied between the two fetch calls (D-01 rate-limit courtesy).

    RED — closed by 17-02 (transcript.py).
    """
    try:
        from enrichment.transcript import fetch_and_cache_transcripts  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-02 (transcript.py)")
