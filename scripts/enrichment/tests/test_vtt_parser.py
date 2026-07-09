"""
RED scaffold for enrichment.vtt_parser module tests.

These tests are RED stubs closed by plan 17-02 (vtt_parser.py).
Each test imports from the not-yet-existing target module inside the test body
and fails with a plan-naming message.

Tests seed: dedup sliding window (overlap removal), HTML/timestamp stripping.
"""
import pytest


def test_dedup_sliding_window_overlap(fixture_vtt_with_overlaps):
    """Overlapping VTT cues produce deduplicated output text (sliding-window longest-common-suffix).

    Pre-condition: VTT content with 2 overlapping cue windows.
    Assert: output text does not contain the overlapping phrase twice.

    RED — closed by 17-02 (vtt_parser.py).
    """
    try:
        from enrichment.vtt_parser import vtt_to_text  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-02 (vtt_parser.py)")


def test_strips_html_and_timestamps():
    """Auto-caption HTML tags (e.g., <c>) and timestamps are stripped from output.

    Pre-condition: VTT content with <c> color tags and timestamp lines.
    Assert: output contains no HTML tags and no VTT timestamp syntax.

    RED — closed by 17-02 (vtt_parser.py).
    """
    vtt = (
        "WEBVTT\n\n"
        "00:00:00.000 --> 00:00:02.000\n"
        "<c>Hello</c> <c>world</c>\n\n"
        "00:00:02.000 --> 00:00:04.000\n"
        "More content here\n"
    )
    try:
        from enrichment.vtt_parser import vtt_to_text  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-02 (vtt_parser.py)")
