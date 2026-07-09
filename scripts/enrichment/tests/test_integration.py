"""
RED scaffold for end-to-end enrichment integration tests.

These tests are RED stubs closed by plan 17-05 (sheet2pack.py wiring).
Each test imports from the not-yet-existing target integration surface
and fails with a plan-naming message.

Tests seed: ENR-04/E1 schema validity, D-03 caption-less video-only lesson,
            D-16 skipped/failed reporting.
"""
import pytest


def test_enriched_pack_validates_against_schema(tmp_cache_dir):
    """Full enriched pack validates against pack-schema.json (ENR-04 / E1).

    Pre-condition: 1 module, 2 lessons with transcripts, mock Anthropic responses.
    Assert: assembled pack validates via jsonschema against learnforge-core/topic-packs/pack-schema.json.
    Assert: pack imports through the existing Settings→Import path (schema-valid output).

    RED — closed by 17-05 (sheet2pack.py wiring).
    """
    pytest.fail("RED — closed by 17-05 (sheet2pack.py wiring)")


def test_caption_less_lesson_stays_video_only(tmp_cache_dir):
    """A lesson without captions stays video-only in the assembled pack (D-03).

    Pre-condition: 1 module, 1 lesson WITHOUT a caption/transcript.
    Assert: lesson block in output has no markdown lesson block added; video block preserved.
    Assert: lesson appears in the skipped report (D-16).

    RED — closed by 17-05 (sheet2pack.py wiring).
    """
    pytest.fail("RED — closed by 17-05 (sheet2pack.py wiring)")


def test_skipped_and_failed_reported(tmp_cache_dir):
    """Skipped and failed lessons appear in the D-16 failure report.

    Pre-condition: 3 lessons: 1 successful, 1 skipped (no captions), 1 failed (API error).
    Assert: skipped list has 1 entry; failed list has 1 entry; results dict has 1 entry.
    Assert: no exception raised (D-16 continue-on-failure).

    RED — closed by 17-05 (sheet2pack.py wiring).
    """
    pytest.fail("RED — closed by 17-05 (sheet2pack.py wiring)")
