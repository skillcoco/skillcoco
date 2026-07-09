"""
Tests for scripts/enrichment/eval/judge.py and scripts/enrichment/eval/report.py.

All tests run with NO API key — judge() is tested via mock client.
CI command: pytest scripts/enrichment/tests/test_eval_report.py -v

Coverage:
  - write_report(): both .json and .md files are written
  - Alert flags fire on threshold-crossing stats fixtures
  - judge(): uses temperature=0, forced tool_choice, returns calibrated=False
  - judge_sample(): iterates artifacts and calls judge per dimension
"""
import json
import os
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

# No API key required for these tests
os.environ.pop("ANTHROPIC_API_KEY", None)

# ---------------------------------------------------------------------------
# Imports
# ---------------------------------------------------------------------------

from enrichment.eval.judge import (
    JUDGE_TEMPERATURE,
    SUPPORTED_DIMENSIONS,
    judge,
    judge_sample,
)
from enrichment.eval.report import (
    BLOCKING_FAILURE_RATE_THRESHOLD,
    COST_OVERRUN_MULTIPLIER,
    GROUNDING_FLAG_RATE_THRESHOLD,
    write_report,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

def _base_stats(**overrides) -> dict:
    """Build a minimal valid stats dict for write_report tests."""
    base = {
        "total_eligible": 10,
        "total_generated": 8,
        "total_skipped": 2,
        "blocking_failures": 0,
        "failures_by_cause": {},
        "grounding_flagged": 0,
        "grounding_flagged_list": [],
        "skipped_list": ["vid-001", "vid-002"],
        "failure_list": [],
        "quiz_outcomes": {
            "module-1": {"status": "pass", "details": "5 questions"},
        },
        "quizzes_generated": 3,
        "quizzes_skipped": 1,
        "cost_estimate_usd": 1.00,
        "cost_actual_usd": 0.80,
        "input_tokens": 50000,
        "output_tokens": 10000,
        "judge_scores": {},
        "judge_instructor_agreement": None,
    }
    base.update(overrides)
    return base


def _make_mock_client(score: int = 4, reason: str = "Good grounding.") -> MagicMock:
    """Build a synchronous mock Anthropic client that returns a score block."""
    score_block = MagicMock()
    score_block.type = "tool_use"
    score_block.name = "emit_score"
    score_block.input = {"score": score, "reason": reason}

    response = MagicMock()
    response.content = [score_block]

    client = MagicMock()
    client.messages.create.return_value = response
    return client


# ---------------------------------------------------------------------------
# write_report() — file creation
# ---------------------------------------------------------------------------

class TestWriteReport:
    def test_writes_json_and_md_files(self, tmp_path):
        """write_report() creates both .json and .md files."""
        out_stem = tmp_path / "test-report"
        stats = _base_stats()
        write_report(stats, out_stem)
        assert (tmp_path / "test-report.json").exists(), ".json file not created"
        assert (tmp_path / "test-report.md").exists(), ".md file not created"

    def test_json_contains_stats_and_alerts(self, tmp_path):
        """The JSON report contains 'stats', 'alerts', and 'thresholds' keys."""
        out_stem = tmp_path / "run"
        write_report(_base_stats(), out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert "stats" in data
        assert "alerts" in data
        assert "thresholds" in data

    def test_md_contains_expected_sections(self, tmp_path):
        """The markdown report contains key section headers."""
        out_stem = tmp_path / "run"
        write_report(_base_stats(), out_stem)
        md = (tmp_path / "run.md").read_text()
        assert "# Enrichment Run Report" in md
        assert "## Alert Flags" in md
        assert "## Blocking Failures" in md
        assert "## Grounding Flags" in md
        assert "## Cost Actuals" in md

    def test_creates_parent_directories(self, tmp_path):
        """write_report creates missing parent directories."""
        out_stem = tmp_path / "nested" / "deep" / "report"
        write_report(_base_stats(), out_stem)
        assert (tmp_path / "nested" / "deep" / "report.json").exists()
        assert (tmp_path / "nested" / "deep" / "report.md").exists()

    def test_json_is_valid_json(self, tmp_path):
        """The written JSON file parses cleanly."""
        out_stem = tmp_path / "run"
        write_report(_base_stats(), out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert isinstance(data, dict)

    def test_skipped_list_in_report(self, tmp_path):
        """Caption-less skips (D-03) appear in the markdown report."""
        out_stem = tmp_path / "run"
        stats = _base_stats(skipped_list=["vid-003", "vid-007"])
        write_report(stats, out_stem)
        md = (tmp_path / "run.md").read_text()
        assert "vid-003" in md
        assert "vid-007" in md

    def test_failure_list_in_report(self, tmp_path):
        """D-16 failure list appears in the markdown report."""
        out_stem = tmp_path / "run"
        stats = _base_stats(
            failure_list=[("module-4", "Quiz generation failed after 3 attempts")]
        )
        write_report(stats, out_stem)
        md = (tmp_path / "run.md").read_text()
        assert "module-4" in md
        assert "Quiz generation failed" in md


# ---------------------------------------------------------------------------
# Alert flag thresholds
# ---------------------------------------------------------------------------

class TestAlertFlags:
    def test_grounding_flag_rate_above_threshold_sets_alert(self, tmp_path):
        """grounding-flag rate > 20% sets the grounding_flag_rate_exceeded alert."""
        # 3 flagged out of 10 = 30% > 20% threshold
        stats = _base_stats(
            total_generated=10,
            grounding_flagged=3,  # 30% > 20%
        )
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert data["alerts"]["grounding_flag_rate_exceeded"] is True

    def test_grounding_flag_rate_below_threshold_no_alert(self, tmp_path):
        """grounding-flag rate <= 20% does NOT set the alert."""
        stats = _base_stats(
            total_generated=10,
            grounding_flagged=2,  # 20% — NOT above threshold (strictly greater)
        )
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert data["alerts"]["grounding_flag_rate_exceeded"] is False

    def test_cost_overrun_sets_alert(self, tmp_path):
        """cost actual > 1.5× estimate sets the cost_overrun alert."""
        stats = _base_stats(
            cost_estimate_usd=1.00,
            cost_actual_usd=1.60,  # 1.6× > 1.5× threshold
        )
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert data["alerts"]["cost_overrun"] is True

    def test_cost_within_threshold_no_alert(self, tmp_path):
        """cost actual <= 1.5× estimate does NOT set the alert."""
        stats = _base_stats(
            cost_estimate_usd=1.00,
            cost_actual_usd=1.40,  # 1.4× <= 1.5×
        )
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert data["alerts"]["cost_overrun"] is False

    def test_blocking_failure_rate_above_threshold_sets_alert(self, tmp_path):
        """blocking-failure rate > 10% sets the alert."""
        stats = _base_stats(
            total_eligible=10,
            blocking_failures=2,  # 20% > 10%
        )
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert data["alerts"]["blocking_failure_rate_exceeded"] is True

    def test_blocking_failure_rate_at_zero_no_alert(self, tmp_path):
        """Zero blocking failures → no alert."""
        stats = _base_stats(total_eligible=10, blocking_failures=0)
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert data["alerts"]["blocking_failure_rate_exceeded"] is False

    def test_judge_advisory_only_when_no_agreement_data(self, tmp_path):
        """judge_advisory_only is True when no calibration data available."""
        stats = _base_stats(judge_instructor_agreement=None)
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert data["alerts"]["judge_advisory_only"] is True

    def test_judge_advisory_only_when_agreement_below_threshold(self, tmp_path):
        """judge_advisory_only is True when agreement < 0.7."""
        stats = _base_stats(judge_instructor_agreement=0.65)
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert data["alerts"]["judge_advisory_only"] is True

    def test_judge_not_advisory_when_agreement_above_threshold(self, tmp_path):
        """judge_advisory_only is False when agreement >= 0.7."""
        stats = _base_stats(judge_instructor_agreement=0.75)
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        with open(tmp_path / "run.json") as fh:
            data = json.load(fh)
        assert data["alerts"]["judge_advisory_only"] is False

    def test_alert_appears_in_markdown(self, tmp_path):
        """A triggered alert appears visibly in the markdown report."""
        stats = _base_stats(
            total_generated=10,
            grounding_flagged=5,  # 50% → alert
        )
        out_stem = tmp_path / "run"
        write_report(stats, out_stem)
        md = (tmp_path / "run.md").read_text()
        assert "ALERT" in md


# ---------------------------------------------------------------------------
# judge() — mock client assertions
# ---------------------------------------------------------------------------

class TestJudge:
    def test_judge_uses_temperature_zero(self):
        """judge() calls messages.create with temperature=0."""
        client = _make_mock_client()
        judge(client, artifact_text="Some lesson text.", transcript="Some transcript.", dimension="E3")
        call_kwargs = client.messages.create.call_args.kwargs
        assert call_kwargs.get("temperature") == 0 or call_kwargs.get("temperature") == JUDGE_TEMPERATURE
        assert JUDGE_TEMPERATURE == 0

    def test_judge_uses_forced_tool_choice(self):
        """judge() calls messages.create with forced tool_choice for emit_score."""
        client = _make_mock_client()
        judge(client, artifact_text="Some lesson.", transcript="Some transcript.", dimension="E6")
        call_kwargs = client.messages.create.call_args.kwargs
        tool_choice = call_kwargs.get("tool_choice")
        assert tool_choice is not None, "tool_choice not set"
        assert tool_choice.get("type") == "tool"
        assert tool_choice.get("name") == "emit_score"

    def test_judge_returns_calibrated_false(self):
        """judge() returns calibrated=False for all dimensions."""
        client = _make_mock_client()
        for dim in ["E3", "E5", "E6", "E7"]:
            result = judge(client, artifact_text="text", transcript="transcript", dimension=dim)
            assert result["calibrated"] is False, f"calibrated should be False for dimension {dim}"

    def test_judge_returns_score_and_reason(self):
        """judge() returns a dict with dimension, score, reason, calibrated."""
        client = _make_mock_client(score=4, reason="Good grounding coverage.")
        result = judge(client, artifact_text="Lesson text.", transcript="Transcript.", dimension="E3")
        assert result["dimension"] == "E3"
        assert result["score"] == 4
        assert result["reason"] == "Good grounding coverage."
        assert result["calibrated"] is False

    def test_judge_passes_transcript_in_user_message(self):
        """judge() includes the transcript in the user message."""
        client = _make_mock_client()
        judge(
            client,
            artifact_text="The lesson content.",
            transcript="The unique transcript SENTINEL_VALUE_12345.",
            dimension="E3",
        )
        call_kwargs = client.messages.create.call_args.kwargs
        messages = call_kwargs.get("messages", [])
        user_content = next(
            (m["content"] for m in messages if m.get("role") == "user"), ""
        )
        assert "SENTINEL_VALUE_12345" in user_content

    def test_judge_raises_on_unsupported_dimension(self):
        """judge() raises ValueError for an unsupported dimension."""
        client = _make_mock_client()
        with pytest.raises(ValueError, match="Unsupported dimension"):
            judge(client, artifact_text="text", transcript="transcript", dimension="E99")

    def test_judge_includes_emit_score_tool(self):
        """judge() passes the emit_score tool definition to messages.create."""
        client = _make_mock_client()
        judge(client, artifact_text="text", transcript="transcript", dimension="E5")
        call_kwargs = client.messages.create.call_args.kwargs
        tools = call_kwargs.get("tools", [])
        tool_names = [t.get("name") for t in tools]
        assert "emit_score" in tool_names

    def test_all_supported_dimensions_accepted(self):
        """judge() accepts all four supported dimensions without error."""
        client = _make_mock_client()
        for dim in SUPPORTED_DIMENSIONS:
            result = judge(client, artifact_text="text", transcript="transcript", dimension=dim)
            assert isinstance(result, dict)


# ---------------------------------------------------------------------------
# judge_sample() — iteration and error handling
# ---------------------------------------------------------------------------

class TestJudgeSample:
    def test_judge_sample_processes_up_to_sample_n(self):
        """judge_sample() stops after sample_n artifacts."""
        artifacts = [
            {"artifact_text": f"lesson {i}", "transcript": f"transcript {i}", "video_id": f"vid-{i:03d}"}
            for i in range(5)
        ]
        client = _make_mock_client()
        results = judge_sample(artifacts, sample_n=2, client=client, dimensions=["E3"])
        # 2 artifacts × 1 dimension = 2 results
        assert len(results) == 2

    def test_judge_sample_returns_calibrated_false_for_all(self):
        """All judge_sample() results have calibrated=False."""
        artifacts = [
            {"artifact_text": "text", "transcript": "transcript", "video_id": "vid-001"}
        ]
        client = _make_mock_client()
        results = judge_sample(artifacts, sample_n=1, client=client, dimensions=["E3", "E6"])
        for r in results:
            assert r["calibrated"] is False

    def test_judge_sample_includes_artifact_id(self):
        """judge_sample() results include artifact_id."""
        artifacts = [
            {"artifact_text": "text", "transcript": "transcript", "video_id": "my-video"}
        ]
        client = _make_mock_client()
        results = judge_sample(artifacts, sample_n=1, client=client, dimensions=["E3"])
        assert results[0]["artifact_id"] == "my-video"
