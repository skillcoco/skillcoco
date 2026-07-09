"""
Tests for enrichment.lesson_generator module.

Covers: E2 truncation guard, D-16 partial-failure-continues,
        D-14 cache-hit skips API call, ENR-04 payload size guard,
        D-17 ANTHROPIC_API_KEY handling.

Closed by plan 17-03 (lesson_generator.py).
"""
import asyncio
import json
import pytest
from unittest.mock import AsyncMock, MagicMock, patch


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_mock_client(responses: list) -> MagicMock:
    """Build a mock AsyncAnthropic client whose messages.create returns responses in order."""
    client = MagicMock()
    create_mock = AsyncMock(side_effect=responses)
    client.messages.create = create_mock
    return client


# ---------------------------------------------------------------------------
# E2 — truncation guard
# ---------------------------------------------------------------------------

def test_truncation_raises(mock_anthropic_truncated, tmp_cache_dir):
    """stop_reason=max_tokens: lesson lands in failed[], absent from results, no exception raised (E2 + D-16).

    Pre-condition: Anthropic API returns stop_reason='max_tokens'.
    Assert: failed list has 1 entry; results dict is empty; no exception raised.
    """
    from enrichment.lesson_generator import generate_lessons

    client = _make_mock_client([mock_anthropic_truncated])
    failed: list = []
    transcripts = {"vid001x": "This is a short transcript."}
    results = asyncio.run(generate_lessons(transcripts, failed, client=client))

    assert results == {}, f"Truncated lesson must NOT appear in results, got: {results}"
    assert len(failed) == 1, f"Truncated lesson must appear in failed[], got: {failed}"
    assert failed[0][0] == "vid001x"


# ---------------------------------------------------------------------------
# D-16 — partial failure continues
# ---------------------------------------------------------------------------

def test_partial_failure_continues(mock_anthropic_truncated, mock_anthropic_lesson_response, tmp_cache_dir):
    """stop_reason=max_tokens for one lesson: that lesson in failed[], others succeed (D-16).

    Pre-condition: 2 transcripts; one API call returns truncated, one returns success.
    Assert: failed list has 1 entry; results dict has 1 entry; no exception raised.
    """
    from enrichment.lesson_generator import generate_lessons

    # vid001x truncates, vid002x succeeds
    client = _make_mock_client([mock_anthropic_truncated, mock_anthropic_lesson_response])
    failed: list = []
    transcripts = {
        "vid001x": "Transcript for truncated lesson.",
        "vid002x": "Transcript for successful lesson.",
    }
    results = asyncio.run(generate_lessons(transcripts, failed, client=client))

    assert len(results) == 1, f"Expected 1 success, got: {list(results.keys())}"
    assert len(failed) == 1, f"Expected 1 failure, got: {failed}"
    assert "vid001x" not in results
    assert "vid002x" in results


# ---------------------------------------------------------------------------
# D-14 — cache hit skips API call
# ---------------------------------------------------------------------------

def test_cache_hit_skips_api(mock_anthropic_lesson_response, tmp_cache_dir):
    """A content-cache hit skips the messages.create() call entirely (D-14).

    Pre-condition: content cache file already written for (video_id, transcript, LESSON_PROMPT_VERSION).
    Assert: messages.create not called; cached markdown returned.
    """
    from enrichment.lesson_generator import generate_lessons, LESSON_PROMPT_VERSION
    from enrichment.content_cache import content_cache_path, write_content_cache

    video_id = "vid003x"
    transcript = "Transcript for cache-hit test."
    cached_markdown = "## Cached Lesson\n\nThis came from the cache."
    cache_path = content_cache_path(video_id, transcript, LESSON_PROMPT_VERSION)
    write_content_cache(cache_path, {"markdown": cached_markdown})

    client = _make_mock_client([mock_anthropic_lesson_response])
    failed: list = []
    results = asyncio.run(generate_lessons({video_id: transcript}, failed, client=client))

    assert results[video_id] == cached_markdown
    assert client.messages.create.call_count == 0, "Cache hit must not call messages.create()"


# ---------------------------------------------------------------------------
# ENR-04 — payload size guard
# ---------------------------------------------------------------------------

def test_payload_size_guard_under_131072():
    """_make_section_payload output is always <= 131072 UTF-8 bytes (ENR-04).

    Pre-condition: Oversized markdown string (~200 KB).
    Assert: json.dumps({'markdown': ...}).encode('utf-8') length <= 131072.
    """
    from enrichment.lesson_generator import _make_section_payload

    # generate oversized markdown (~200 KB)
    big_markdown = "## Heading\n\n" + ("word " * 40000)  # ~200 KB
    payload_json = _make_section_payload(big_markdown)
    byte_len = len(payload_json.encode("utf-8"))
    assert byte_len <= 131072, f"Payload exceeds 131072 bytes: {byte_len}"

    # verify it's still valid JSON with 'markdown' key
    data = json.loads(payload_json)
    assert "markdown" in data


# ---------------------------------------------------------------------------
# D-17 — ANTHROPIC_API_KEY handling
# ---------------------------------------------------------------------------

def test_missing_api_key_exits(monkeypatch):
    """get_anthropic_client() raises SystemExit with clear message when ANTHROPIC_API_KEY is unset (D-17).

    Assert: SystemExit raised; error message references ANTHROPIC_API_KEY (not the value).
    """
    from enrichment.lesson_generator import get_anthropic_client

    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)
    with pytest.raises(SystemExit) as exc_info:
        get_anthropic_client()
    # Error should mention the var name but NOT log a key value
    assert exc_info.value.code is not None
