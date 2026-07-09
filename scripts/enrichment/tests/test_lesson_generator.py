"""
RED scaffold for enrichment.lesson_generator module tests.

These tests are RED stubs closed by plan 17-03 (lesson_generator.py).
Each test imports from the not-yet-existing target module inside the test body
and fails with a plan-naming message.

Tests seed: E2 truncation guard, D-16 partial-failure, ENR-04 payload size guard.
"""
import pytest


def test_truncation_raises(mock_anthropic_truncated, tmp_cache_dir):
    """stop_reason=max_tokens raises ValueError (E2 — no silent truncation).

    Pre-condition: Anthropic API returns stop_reason='max_tokens'.
    Assert: ValueError raised; lesson NOT written to cache or returned in results.

    RED — closed by 17-03 (lesson_generator.py).
    """
    try:
        from enrichment.lesson_generator import generate_lessons  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-03 (lesson_generator.py)")


def test_partial_failure_continues(mock_anthropic_truncated, mock_anthropic_lesson_response, tmp_cache_dir):
    """stop_reason=max_tokens for one lesson: that lesson in failed[], others succeed (D-16).

    Pre-condition: 2 transcripts; one API call returns truncated, one returns success.
    Assert: failed list has 1 entry; results dict has 1 entry; no exception raised.

    RED — closed by 17-03 (lesson_generator.py).
    """
    try:
        from enrichment.lesson_generator import generate_lessons  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-03 (lesson_generator.py)")


def test_payload_size_guard_under_131072(mock_anthropic_lesson_response, tmp_cache_dir):
    """Lesson payloadJson is always <= 131072 bytes (ENR-04 — pack-schema.json maxLength).

    Pre-condition: Lesson generated successfully.
    Assert: json.dumps({'markdown': lesson_text}).encode('utf-8') length <= 131072.

    RED — closed by 17-03 (lesson_generator.py).
    """
    try:
        from enrichment.lesson_generator import generate_lessons  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-03 (lesson_generator.py)")
