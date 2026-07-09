"""
Tests for enrichment.token_estimator module.

Covers: D-15 --yes flag skips confirmation, SystemExit on "n" answer.

Closed by plan 17-03 (token_estimator.py).
"""
import pytest


def test_yes_true_prints_estimate_and_returns(capsys):
    """estimate_and_confirm with yes=True prints cost estimate and returns without prompting (D-15 --yes).

    Pre-condition: transcripts dict with 2 items; yes=True.
    Assert: output contains 'Enrichment estimate'; no input() call made; function returns None.
    """
    from enrichment.token_estimator import estimate_and_confirm

    transcripts = {
        "vid001x": "So today we look at Kubernetes deployments. A deployment manages pods.",
        "vid002x": "Let's explore Docker containers and how they isolate processes.",
    }

    # If input() is called it will raise (we inject a raising version to detect it)
    def _raise_if_input_called(prompt=""):
        raise AssertionError("input() must NOT be called when yes=True")

    import builtins
    original_input = builtins.input
    builtins.input = _raise_if_input_called
    try:
        result = estimate_and_confirm(transcripts, module_count=2, yes=True)
    finally:
        builtins.input = original_input

    assert result is None
    out = capsys.readouterr().out
    assert "Enrichment estimate" in out, f"Expected 'Enrichment estimate' in output, got: {out!r}"
    assert "lessons" in out.lower(), f"Expected lesson count in output, got: {out!r}"


def test_no_answer_exits(monkeypatch):
    """estimate_and_confirm with yes=False and user input 'n' calls sys.exit(0) (D-15).

    Pre-condition: transcripts dict; yes=False; monkeypatched input() returns 'n'.
    Assert: SystemExit raised (sys.exit(0)).
    """
    from enrichment.token_estimator import estimate_and_confirm

    transcripts = {"vid001x": "A transcript for the exit test."}
    monkeypatch.setattr("builtins.input", lambda prompt="": "n")

    with pytest.raises(SystemExit) as exc_info:
        estimate_and_confirm(transcripts, module_count=1, yes=False)

    assert exc_info.value.code == 0, f"Expected sys.exit(0) on 'n' answer, got code={exc_info.value.code}"
