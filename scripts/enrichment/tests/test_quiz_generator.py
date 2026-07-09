"""
RED scaffold for enrichment.quiz_generator module tests.

These tests are RED stubs closed by plan 17-04 (quiz_generator.py).
Each test imports from the not-yet-existing target module inside the test body
and fails with a plan-naming message.

Tests seed: D-09 fill-gaps skipping matched chapters, Pydantic retry on ValidationError,
            E1 adapter shape matching quiz_payload.
"""
import pytest


def test_fill_gaps_skips_matched_chapters(tmp_cache_dir):
    """Quiz generation skips modules already in matched_chapters (D-09).

    Pre-condition: matched_chapters contains module num 2; modules list has modules 1, 2, 3.
    Assert: generate_quizzes only generates quizzes for modules 1 and 3; module 2 untouched.

    RED — closed by 17-04 (quiz_generator.py).
    """
    try:
        from enrichment.quiz_generator import generate_quizzes  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-04 (quiz_generator.py)")


def test_pydantic_retry_on_validation_error(tmp_cache_dir):
    """ValidationError on first attempt → retry with error context appended to prompt.

    Pre-condition: API returns invalid QuizOutput on attempt 1, valid on attempt 2.
    Assert: function succeeds; retry count tracked; error message appended to user prompt.

    RED — closed by 17-04 (quiz_generator.py).
    """
    try:
        from enrichment.quiz_generator import generate_quizzes  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-04 (quiz_generator.py)")


def test_adapter_shape_matches_quiz_payload(mock_anthropic_quiz_tool_response, tmp_cache_dir):
    """Generated quiz adapter output matches the quiz_payload() shape (E1 — schema validity).

    Pre-condition: Successful API call with valid 5-question quiz tool response.
    Assert: generated payload has 'questions' list where each entry has
            id='q-{slug}-{qi}', options[0].id='opt-{qi}-1', correctOptionId is a string.

    RED — closed by 17-04 (quiz_generator.py).
    """
    try:
        from enrichment.quiz_generator import generate_quizzes  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-04 (quiz_generator.py)")
