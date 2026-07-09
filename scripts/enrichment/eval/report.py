"""
Per-run report writer for the enrichment pipeline.

AI-SPEC Section 7 — Production Monitoring artifacts:
  Writes both a machine-readable JSON report and a human-readable markdown summary.

Metrics tracked (per run):
  1. Enrichment coverage  — generated / eligible counts
  2. Blocking-failure count by cause  — truncation, schema/Pydantic, API error
  3. Grounding-flag rate  — % lessons marked needs_review
  4. Cost actuals  — input/output tokens and $ vs. D-15 estimate
  5. LLM judge means (E3/E5/E6/E7) — once judge is calibrated

Alert thresholds (AI-SPEC Section 7, surfaced in report summary):
  - Any schema-invalid pack  → hard stop (enforced by caller / E1 gate)
  - Blocking-failure rate > 10%  → alert flag
  - Grounding-flag rate > 20%  → alert flag
  - Cost actual > 1.5× D-15 estimate  → alert flag
  - Judge↔instructor agreement < 0.7  → judge scores advisory-only

Security (T-17-02):
  - This module writes stats only (counts, ratios, costs) — no API key or
    secrets are written to the report files.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any


# ---------------------------------------------------------------------------
# Alert threshold constants (AI-SPEC Section 7)
# ---------------------------------------------------------------------------

BLOCKING_FAILURE_RATE_THRESHOLD = 0.10   # > 10% → alert
GROUNDING_FLAG_RATE_THRESHOLD = 0.20     # > 20% → alert
COST_OVERRUN_MULTIPLIER = 1.5            # > 1.5× estimate → alert
JUDGE_AGREEMENT_THRESHOLD = 0.70         # < 0.7 → advisory-only


# ---------------------------------------------------------------------------
# Alert computation
# ---------------------------------------------------------------------------

def _compute_alerts(stats: dict) -> dict[str, bool]:
    """Compute AI-SPEC Section 7 alert flags from run stats.

    Args:
        stats: Run statistics dict (see write_report for schema).

    Returns:
        Dict of alert_name → bool (True = alert triggered).
    """
    alerts: dict[str, bool] = {
        "blocking_failure_rate_exceeded": False,
        "grounding_flag_rate_exceeded": False,
        "cost_overrun": False,
        "judge_advisory_only": False,
    }

    # Blocking-failure rate
    total_eligible = stats.get("total_eligible", 0)
    blocking_failures = stats.get("blocking_failures", 0)
    if total_eligible > 0:
        rate = blocking_failures / total_eligible
        alerts["blocking_failure_rate_exceeded"] = rate > BLOCKING_FAILURE_RATE_THRESHOLD

    # Grounding-flag rate
    total_lessons = stats.get("total_generated", 0)
    grounding_flags = stats.get("grounding_flagged", 0)
    if total_lessons > 0:
        flag_rate = grounding_flags / total_lessons
        alerts["grounding_flag_rate_exceeded"] = flag_rate > GROUNDING_FLAG_RATE_THRESHOLD

    # Cost overrun
    cost_estimate = stats.get("cost_estimate_usd", 0.0)
    cost_actual = stats.get("cost_actual_usd", 0.0)
    if cost_estimate > 0:
        alerts["cost_overrun"] = cost_actual > (cost_estimate * COST_OVERRUN_MULTIPLIER)

    # Judge advisory-only flag
    judge_agreement = stats.get("judge_instructor_agreement", None)
    if judge_agreement is not None:
        alerts["judge_advisory_only"] = judge_agreement < JUDGE_AGREEMENT_THRESHOLD
    else:
        # No calibration yet → mark advisory
        alerts["judge_advisory_only"] = True

    return alerts


# ---------------------------------------------------------------------------
# Markdown generation
# ---------------------------------------------------------------------------

def _build_markdown(stats: dict, alerts: dict[str, bool]) -> str:
    """Build a human-readable markdown report from stats and computed alerts."""
    lines: list[str] = []

    lines.append("# Enrichment Run Report")
    lines.append("")

    # Summary table
    lines.append("## Run Summary")
    lines.append("")
    lines.append("| Metric | Value |")
    lines.append("|--------|-------|")
    lines.append(f"| Total eligible videos | {stats.get('total_eligible', 0)} |")
    lines.append(f"| Lessons generated | {stats.get('total_generated', 0)} |")
    lines.append(f"| Lessons skipped (caption-less, D-03) | {stats.get('total_skipped', 0)} |")
    lines.append(f"| Blocking failures | {stats.get('blocking_failures', 0)} |")
    lines.append(f"| Quizzes generated | {stats.get('quizzes_generated', 0)} |")
    lines.append(f"| Quizzes skipped (fill-gaps, D-09) | {stats.get('quizzes_skipped', 0)} |")
    lines.append("")

    # Coverage
    total_eligible = stats.get("total_eligible", 0)
    total_generated = stats.get("total_generated", 0)
    coverage = (
        f"{total_generated / total_eligible:.1%}"
        if total_eligible > 0
        else "N/A"
    )
    lines.append(f"**Enrichment coverage:** {coverage}")
    lines.append("")

    # Blocking failures by cause
    lines.append("## Blocking Failures by Cause")
    lines.append("")
    failures_by_cause: dict = stats.get("failures_by_cause", {})
    if failures_by_cause:
        lines.append("| Cause | Count |")
        lines.append("|-------|-------|")
        for cause, count in failures_by_cause.items():
            lines.append(f"| {cause} | {count} |")
    else:
        lines.append("_No blocking failures._")
    lines.append("")

    # Grounding flags
    lines.append("## Grounding Flags (needs_review)")
    lines.append("")
    grounding_flagged: list = stats.get("grounding_flagged_list", [])
    grounding_count = stats.get("grounding_flagged", 0)
    total_gen = stats.get("total_generated", 0)
    flag_rate = (
        f"{grounding_count / total_gen:.1%}" if total_gen > 0 else "N/A"
    )
    lines.append(f"**Grounding-flag rate:** {flag_rate} ({grounding_count}/{total_gen} lessons)")
    lines.append("")
    if grounding_flagged:
        lines.append("| Video ID | Reason |")
        lines.append("|----------|--------|")
        for item in grounding_flagged:
            vid = item.get("video_id", "unknown")
            reason = item.get("reason", "")
            lines.append(f"| {vid} | {reason} |")
    else:
        lines.append("_No grounding flags._")
    lines.append("")

    # Quiz validation outcomes
    lines.append("## Quiz Validation Outcomes")
    lines.append("")
    quiz_outcomes: dict = stats.get("quiz_outcomes", {})
    if quiz_outcomes:
        lines.append("| Module | Status | Details |")
        lines.append("|--------|--------|---------|")
        for module, outcome in quiz_outcomes.items():
            status = outcome.get("status", "unknown")
            details = outcome.get("details", "")
            lines.append(f"| {module} | {status} | {details} |")
    else:
        lines.append("_No quiz validation data._")
    lines.append("")

    # Cost actuals
    lines.append("## Cost Actuals vs. Estimate")
    lines.append("")
    lines.append("| Metric | Value |")
    lines.append("|--------|-------|")
    lines.append(f"| D-15 estimate (USD) | ${stats.get('cost_estimate_usd', 0.0):.4f} |")
    lines.append(f"| Actual cost (USD) | ${stats.get('cost_actual_usd', 0.0):.4f} |")
    lines.append(f"| Input tokens | {stats.get('input_tokens', 0):,} |")
    lines.append(f"| Output tokens | {stats.get('output_tokens', 0):,} |")
    lines.append("")

    # Caption-less skip list (D-03)
    lines.append("## Caption-less Skips (D-03)")
    lines.append("")
    skipped_list: list = stats.get("skipped_list", [])
    if skipped_list:
        for vid in skipped_list:
            lines.append(f"- {vid}")
    else:
        lines.append("_No caption-less skips._")
    lines.append("")

    # Failure list (D-16)
    lines.append("## Failure List (D-16)")
    lines.append("")
    failure_list: list = stats.get("failure_list", [])
    if failure_list:
        for item in failure_list:
            if isinstance(item, (list, tuple)) and len(item) >= 2:
                lines.append(f"- `{item[0]}`: {item[1]}")
            else:
                lines.append(f"- {item}")
    else:
        lines.append("_No failures._")
    lines.append("")

    # LLM judge scores
    judge_scores: dict = stats.get("judge_scores", {})
    if judge_scores:
        lines.append("## LLM Judge Scores (Advisory)")
        lines.append("")
        lines.append(
            "> Judge scores are **advisory only** until founder calibration reaches "
            ">= 0.7 agreement with instructor scores (AI-SPEC §5)."
        )
        lines.append("")
        lines.append("| Dimension | Mean Score | Sample Size |")
        lines.append("|-----------|------------|-------------|")
        for dim, data in judge_scores.items():
            mean = data.get("mean", "N/A")
            n = data.get("n", 0)
            mean_str = f"{mean:.2f}" if isinstance(mean, float) else str(mean)
            lines.append(f"| {dim} | {mean_str} | {n} |")
        lines.append("")

    # Alert flags
    lines.append("## Alert Flags")
    lines.append("")
    any_alert = any(alerts.values())
    if any_alert:
        lines.append("> WARNING: One or more alert thresholds exceeded. Review before publishing.")
        lines.append("")
    for alert_name, triggered in alerts.items():
        status_icon = "ALERT" if triggered else "OK"
        lines.append(f"- **{status_icon}** `{alert_name}`")
    lines.append("")

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def write_report(stats: dict, out_stem: Path) -> None:
    """Write a per-run JSON + markdown report from run statistics.

    Creates two files:
      - {out_stem}.json  — machine-readable report with all metrics
      - {out_stem}.md    — human-readable summary for the founder

    AI-SPEC Section 7 alert thresholds are computed and included in both files.
    No API key or secret values are written (T-17-02).

    Args:
        stats:    Run statistics dict. Expected keys:
                    total_eligible (int)    — total caption-bearing videos
                    total_generated (int)   — lessons successfully generated
                    total_skipped (int)     — D-03 caption-less skips
                    blocking_failures (int) — E1/E2 hard stops
                    failures_by_cause (dict) — {"truncation": N, "schema": N, ...}
                    grounding_flagged (int) — lessons flagged needs_review
                    grounding_flagged_list (list) — [{"video_id": ..., "reason": ...}]
                    skipped_list (list)     — video IDs with no captions (D-03)
                    failure_list (list)     — [(module_slug, error_msg)] (D-16)
                    quiz_outcomes (dict)    — {module_slug: {"status": ..., "details": ...}}
                    quizzes_generated (int)
                    quizzes_skipped (int)
                    cost_estimate_usd (float) — D-15 pre-run estimate
                    cost_actual_usd (float) — actual spend
                    input_tokens (int)
                    output_tokens (int)
                    judge_scores (dict)     — {dimension: {"mean": float, "n": int}}
                    judge_instructor_agreement (float | None)
        out_stem: Path stem (no extension). Both .json and .md will be created.
    """
    out_stem = Path(out_stem)
    out_stem.parent.mkdir(parents=True, exist_ok=True)

    # Compute alert flags
    alerts = _compute_alerts(stats)

    # Build the JSON report
    json_report: dict[str, Any] = {
        "stats": stats,
        "alerts": alerts,
        "thresholds": {
            "blocking_failure_rate": BLOCKING_FAILURE_RATE_THRESHOLD,
            "grounding_flag_rate": GROUNDING_FLAG_RATE_THRESHOLD,
            "cost_overrun_multiplier": COST_OVERRUN_MULTIPLIER,
            "judge_agreement_min": JUDGE_AGREEMENT_THRESHOLD,
        },
    }

    json_path = out_stem.with_suffix(".json")
    with open(json_path, "w", encoding="utf-8") as fh:
        json.dump(json_report, fh, indent=2, ensure_ascii=False)

    # Build the markdown report
    md_content = _build_markdown(stats, alerts)
    md_path = out_stem.with_suffix(".md")
    with open(md_path, "w", encoding="utf-8") as fh:
        fh.write(md_content)
