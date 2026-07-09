"""
RED scaffold for enrichment.token_estimator module tests.

These tests are RED stubs closed by plan 17-03 (token_estimator.py).
Each test imports from the not-yet-existing target module inside the test body
and fails with a plan-naming message.

Tests seed: D-15 --yes flag skips confirmation, SystemExit on "n" answer.
"""
import pytest


def test_yes_true_prints_estimate_and_returns(tmp_cache_dir, capsys):
    """estimate_and_confirm with yes=True prints cost estimate and returns without prompting (D-15 --yes).

    Pre-condition: transcripts dict with 2 items; yes=True.
    Assert: output contains 'Enrichment estimate'; no input() call made; function returns None.

    RED — closed by 17-03 (token_estimator.py).
    """
    try:
        from enrichment.token_estimator import estimate_and_confirm  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-03 (token_estimator.py)")


def test_no_answer_exits(monkeypatch, tmp_cache_dir):
    """estimate_and_confirm with yes=False and user input 'n' calls sys.exit(0) (D-15).

    Pre-condition: transcripts dict; yes=False; monkeypatched input() returns 'n'.
    Assert: SystemExit raised (sys.exit(0)).

    RED — closed by 17-03 (token_estimator.py).
    """
    try:
        from enrichment.token_estimator import estimate_and_confirm  # noqa: F401
    except ImportError:
        pytest.fail("RED — closed by 17-03 (token_estimator.py)")
