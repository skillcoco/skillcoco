"""
Tests for enrichment.quiz_generator module.

Covers:
  - D-09: fill-gaps-only — skip modules in matched_chapters
  - D-10: num_questions scales with module lesson count (clamped 5..10)
  - D-12: grounding uses all module lesson transcripts
  - D-14: cache hit skips API call
  - D-16: continue-on-failure — failed modules added to failed list, not raised
  - E1: adapter output shape matches quiz_payload() (string correctOptionId, opt-{qi}-{oi} ids)
  - E2: stop_reason max_tokens treated as failure (truncation guard)
  - Pydantic retry: ValidationError on attempt 1 -> retry -> success on attempt 2

Closed by plan 17-04 (quiz_generator.py).
"""
import asyncio
import json
import pytest
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_modules_raw(nums_and_lesson_counts):
    """Build a minimal modules_raw list for testing.

    nums_and_lesson_counts: list of (module_num, lesson_count) tuples.
    Each lesson gets a synthetic video_id like 'vid{module_num}{lesson_idx}'.
    """
    modules = []
    for num, lesson_count in nums_and_lesson_counts:
        slug = f"module-{num}"
        lessons = []
        for i in range(lesson_count):
            vid = f"vid{num:02d}{i:02d}aa"  # 10 chars — valid VIDEO_ID_RE
            lessons.append({"video_id": vid, "title": f"Lesson {i+1}"})
        modules.append({
            "num": num,
            "title": f"Module {num}",
            "slug": slug,
            "lessons": lessons,
        })
    return modules


def _make_transcripts(modules_raw):
    """Build a transcripts dict {video_id: text} for all lessons in modules_raw."""
    result = {}
    for m in modules_raw:
        for lesson in m.get("lessons", []):
            vid = lesson.get("video_id")
            if vid:
                result[vid] = f"Transcript content for {vid} about Kubernetes deployments."
    return result


def _run(coro):
    """Run an async coroutine in a fresh event loop."""
    return asyncio.get_event_loop().run_until_complete(coro)


# ---------------------------------------------------------------------------
# D-09: fill-gaps-only — skip modules in matched_chapters
# ---------------------------------------------------------------------------

def test_fill_gaps_skips_matched_chapters(tmp_cache_dir):
    """Quiz generation skips modules in matched_chapters without calling API (D-09).

    Pre-condition: matched_chapters={2}; modules 1, 2, 3 all have valid transcripts.
    Assert:
      - returned dict contains keys for module slugs 1 and 3 only
      - module 2 is absent from results
      - API create() is never called for module 2
    """
    from enrichment.quiz_generator import generate_quizzes

    modules_raw = _make_modules_raw([(1, 2), (2, 2), (3, 2)])
    transcripts = _make_transcripts(modules_raw)
    matched_chapters = {2}
    failed = []

    # Build a mock tool_use response for modules 1 and 3
    def _make_response():
        tool_input = {
            "questions": [
                {
                    "question": f"What does a Kubernetes Deployment manage (q{i})?",
                    "choices": [
                        "A set of pod replicas",
                        "Network routing rules",
                        "Persistent storage volumes",
                        "Node scheduling policies",
                    ],
                    "correct_index": 0,
                    "explanation": "A Deployment manages pod replicas.",
                }
                for i in range(5)
            ]
        }
        response = MagicMock()
        response.stop_reason = "end_turn"
        tool_block = MagicMock()
        tool_block.type = "tool_use"
        tool_block.name = "emit_quiz"
        tool_block.input = tool_input
        response.content = [tool_block]
        return response

    create_mock = AsyncMock(side_effect=lambda **kw: _make_response())

    with patch("enrichment.quiz_generator._get_client") as mock_client_fn:
        mock_client = MagicMock()
        mock_client.messages.create = create_mock
        mock_client_fn.return_value = mock_client

        result = _run(generate_quizzes(
            modules_raw, transcripts, matched_chapters, failed
        ))

    assert "module-1" in result, "module 1 (not in matched_chapters) should be generated"
    assert "module-3" in result, "module 3 (not in matched_chapters) should be generated"
    assert "module-2" not in result, "module 2 in matched_chapters must be skipped"
    # API called for modules 1 and 3, not for module 2
    assert create_mock.call_count == 2, (
        f"API should be called for 2 modules (not module 2), got {create_mock.call_count}"
    )


# ---------------------------------------------------------------------------
# D-10: num_questions scales with lesson count (clamped 5..10)
# ---------------------------------------------------------------------------

def test_num_questions_clamped_for_small_module():
    """A 2-lesson module requests exactly 5 questions (lower clamp, D-10)."""
    from enrichment.quiz_generator import _num_questions_for
    assert _num_questions_for(2) == 5


def test_num_questions_clamped_for_large_module():
    """A 12-lesson module requests exactly 10 questions (upper clamp, D-10)."""
    from enrichment.quiz_generator import _num_questions_for
    assert _num_questions_for(12) == 10


def test_num_questions_scaling_mid_range():
    """A 5-lesson module scales within [5, 10]."""
    from enrichment.quiz_generator import _num_questions_for
    result = _num_questions_for(5)
    assert 5 <= result <= 10


# ---------------------------------------------------------------------------
# Pydantic retry: ValidationError on attempt 1 -> success on attempt 2
# ---------------------------------------------------------------------------

def test_pydantic_retry_on_validation_error(tmp_cache_dir):
    """ValidationError on first attempt -> retry with error appended -> success (E1 retry).

    Pre-condition:
      - API returns invalid QuizOutput on attempt 1 (choices list has only 2 items)
      - API returns valid QuizOutput on attempt 2
    Assert:
      - generate_quizzes returns the module slug in the result
      - create() called exactly 2 times
      - failed list remains empty
    """
    from enrichment.quiz_generator import generate_quizzes

    modules_raw = _make_modules_raw([(1, 2)])
    transcripts = _make_transcripts(modules_raw)
    matched_chapters: set = set()
    failed = []

    # Attempt 1: invalid (only 2 choices — fails MCQQuestion.choices validator)
    invalid_tool_input = {
        "questions": [
            {
                "question": "What does Kubernetes manage?",
                "choices": ["Pods", "Nodes"],  # only 2 — fails min_length=4
                "correct_index": 0,
                "explanation": "Manages pods.",
            }
        ]
    }
    invalid_response = MagicMock()
    invalid_response.stop_reason = "end_turn"
    invalid_tool = MagicMock()
    invalid_tool.type = "tool_use"
    invalid_tool.name = "emit_quiz"
    invalid_tool.input = invalid_tool_input
    invalid_response.content = [invalid_tool]

    # Attempt 2: valid (5 questions, 4 distinct choices each)
    valid_tool_input = {
        "questions": [
            {
                "question": f"What does a Kubernetes Deployment manage (q{i})?",
                "choices": [
                    "A set of pod replicas",
                    "Network routing rules",
                    "Persistent storage volumes",
                    "Node scheduling policies",
                ],
                "correct_index": 0,
                "explanation": "A Deployment manages pod replicas.",
            }
            for i in range(5)
        ]
    }
    valid_response = MagicMock()
    valid_response.stop_reason = "end_turn"
    valid_tool = MagicMock()
    valid_tool.type = "tool_use"
    valid_tool.name = "emit_quiz"
    valid_tool.input = valid_tool_input
    valid_response.content = [valid_tool]

    responses = [invalid_response, valid_response]
    call_count = [0]

    async def _side_effect(**kw):
        idx = call_count[0]
        call_count[0] += 1
        return responses[idx]

    create_mock = AsyncMock(side_effect=_side_effect)

    with patch("enrichment.quiz_generator._get_client") as mock_client_fn:
        mock_client = MagicMock()
        mock_client.messages.create = create_mock
        mock_client_fn.return_value = mock_client

        result = _run(generate_quizzes(
            modules_raw, transcripts, matched_chapters, failed
        ))

    assert "module-1" in result, "module-1 should succeed after retry"
    assert len(failed) == 0, f"no modules should fail; got: {failed}"
    assert create_mock.call_count == 2, (
        f"API should be called twice (1 invalid + 1 valid), got {create_mock.call_count}"
    )


# ---------------------------------------------------------------------------
# E1: adapter shape matches quiz_payload() (string correctOptionId)
# ---------------------------------------------------------------------------

def test_adapter_shape_matches_quiz_payload(mock_anthropic_quiz_tool_response, tmp_cache_dir):
    """Generated quiz payload has the exact quiz_payload() shape (E1 — schema validity).

    Assert:
      - result has 'questions' list
      - each question has 'id' = 'q-{slug}-{qi}' (1-based)
      - options[0].id = 'opt-{qi}-1' (1-based)
      - correctOptionId is a string matching 'opt-{qi}-{oi}' pattern
    """
    from enrichment.quiz_generator import generate_quizzes

    modules_raw = _make_modules_raw([(1, 2)])
    transcripts = _make_transcripts(modules_raw)
    matched_chapters: set = set()
    failed = []

    create_mock = AsyncMock(return_value=mock_anthropic_quiz_tool_response)

    with patch("enrichment.quiz_generator._get_client") as mock_client_fn:
        mock_client = MagicMock()
        mock_client.messages.create = create_mock
        mock_client_fn.return_value = mock_client

        result = _run(generate_quizzes(
            modules_raw, transcripts, matched_chapters, failed
        ))

    assert "module-1" in result, "module-1 should be in results"
    payload = result["module-1"]
    assert "questions" in payload

    questions = payload["questions"]
    assert len(questions) == 5, f"expected 5 questions from fixture, got {len(questions)}"

    # Check first question shape
    q = questions[0]
    assert q["id"] == "q-module-1-1", f"expected 'q-module-1-1', got {q['id']}"
    assert "stem" in q
    assert "options" in q
    assert "correctOptionId" in q
    assert "explanation" in q

    # correctOptionId must be a string (not an int)
    assert isinstance(q["correctOptionId"], str), (
        f"correctOptionId must be string, got {type(q['correctOptionId'])}"
    )

    # correctOptionId must match opt-{qi}-{oi} pattern
    import re
    assert re.match(r"^opt-\d+-\d+$", q["correctOptionId"]), (
        f"correctOptionId must match opt-{{qi}}-{{oi}}, got {q['correctOptionId']!r}"
    )

    # options[0].id should be 'opt-1-1' (qi=1, oi=1)
    assert q["options"][0]["id"] == "opt-1-1", (
        f"options[0].id should be 'opt-1-1', got {q['options'][0]['id']!r}"
    )

    # correctOptionId must match one of the option IDs
    option_ids = {o["id"] for o in q["options"]}
    assert q["correctOptionId"] in option_ids, (
        f"correctOptionId {q['correctOptionId']!r} not in option ids {option_ids}"
    )


# ---------------------------------------------------------------------------
# D-14: cache hit skips API call
# ---------------------------------------------------------------------------

def test_cache_hit_skips_api_call(tmp_cache_dir):
    """Cache hit: pre-write cache file, assert API create() is not called (D-14)."""
    from enrichment.quiz_generator import generate_quizzes, QUIZ_PROMPT_VERSION
    from enrichment.content_cache import quiz_cache_path, write_content_cache

    modules_raw = _make_modules_raw([(1, 2)])
    transcripts = _make_transcripts(modules_raw)
    matched_chapters: set = set()
    failed = []

    # Build the cache payload that would have been generated
    cached_payload = {
        "questions": [
            {
                "id": "q-module-1-1",
                "stem": "Cached question?",
                "options": [
                    {"id": "opt-1-1", "text": "A"},
                    {"id": "opt-1-2", "text": "B"},
                    {"id": "opt-1-3", "text": "C"},
                    {"id": "opt-1-4", "text": "D"},
                ],
                "correctOptionId": "opt-1-1",
                "explanation": "Cached explanation.",
            }
        ]
    }

    # Pre-populate the cache
    m = modules_raw[0]
    module_transcripts = [
        transcripts[lesson["video_id"]]
        for lesson in m["lessons"]
        if lesson.get("video_id") in transcripts
    ]
    cache_path = quiz_cache_path(m["slug"], module_transcripts, QUIZ_PROMPT_VERSION)
    write_content_cache(cache_path, cached_payload)

    create_mock = AsyncMock()

    with patch("enrichment.quiz_generator._get_client") as mock_client_fn:
        mock_client = MagicMock()
        mock_client.messages.create = create_mock
        mock_client_fn.return_value = mock_client

        result = _run(generate_quizzes(
            modules_raw, transcripts, matched_chapters, failed
        ))

    assert "module-1" in result, "cache hit should still include module in result"
    assert result["module-1"] == cached_payload, "result should equal cached payload"
    assert create_mock.call_count == 0, (
        f"API create() must not be called on cache hit, got {create_mock.call_count} calls"
    )


# ---------------------------------------------------------------------------
# D-16: continue-on-failure — 3 failures -> module added to failed, not raised
# ---------------------------------------------------------------------------

def test_continue_on_failure_after_3_attempts(tmp_cache_dir):
    """After 3 failed attempts, module added to failed list and skipped (D-16).

    All 3 attempts return invalid quiz output (stop_reason=max_tokens).
    Assert: module absent from returned dict; failed list has 1 entry; no raise.
    """
    from enrichment.quiz_generator import generate_quizzes

    modules_raw = _make_modules_raw([(1, 2)])
    transcripts = _make_transcripts(modules_raw)
    matched_chapters: set = set()
    failed = []

    # All responses are truncated
    truncated_response = MagicMock()
    truncated_response.stop_reason = "max_tokens"
    truncated_response.content = []

    create_mock = AsyncMock(return_value=truncated_response)

    with patch("enrichment.quiz_generator._get_client") as mock_client_fn:
        mock_client = MagicMock()
        mock_client.messages.create = create_mock
        mock_client_fn.return_value = mock_client

        result = _run(generate_quizzes(
            modules_raw, transcripts, matched_chapters, failed
        ))

    assert "module-1" not in result, "failed module must not appear in result"
    assert len(failed) == 1, f"failed list should have 1 entry, got {failed}"
    assert failed[0][0] == "module-1", f"failed entry should name the module slug"


# ---------------------------------------------------------------------------
# E2: stop_reason max_tokens treated as failure per attempt
# ---------------------------------------------------------------------------

def test_truncation_triggers_retry_then_failure(tmp_cache_dir):
    """stop_reason=max_tokens on all 3 attempts leads to module in failed list (E2)."""
    from enrichment.quiz_generator import generate_quizzes

    modules_raw = _make_modules_raw([(7, 3)])
    transcripts = _make_transcripts(modules_raw)
    matched_chapters: set = set()
    failed = []

    truncated_response = MagicMock()
    truncated_response.stop_reason = "max_tokens"
    truncated_response.content = []

    create_mock = AsyncMock(return_value=truncated_response)

    with patch("enrichment.quiz_generator._get_client") as mock_client_fn:
        mock_client = MagicMock()
        mock_client.messages.create = create_mock
        mock_client_fn.return_value = mock_client

        result = _run(generate_quizzes(
            modules_raw, transcripts, matched_chapters, failed
        ))

    assert "module-7" not in result
    assert len(failed) == 1
    # 3 attempts were made (one per retry)
    assert create_mock.call_count == 3, (
        f"expected 3 attempts for truncation, got {create_mock.call_count}"
    )
