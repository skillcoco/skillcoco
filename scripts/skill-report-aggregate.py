#!/usr/bin/env python3
"""skill-report-aggregate.py — D-14 team-aggregation tooling (Phase 18).

This-repo-only. ZERO dependency on the private Creator Studio repo. Uses
ONLY the Python standard library (argparse, json, subprocess, pathlib,
sys) — no pip installs, no third-party packages.

It NEVER reimplements signature-verification crypto in Python. Every
report's signature is checked by shelling out to the `forge-sign
verify-report` subcommand (the same Rust binary + the same
`learnforge_core::signing::verify_payload` path the desktop app's Verify
panel uses) — this script only parses that subcommand's JSON result and
does group arithmetic on top of it.

Usage:
    python3 scripts/skill-report-aggregate.py \\
        --public-key pub.pem \\
        report1.json report2.json report3.json

    python3 scripts/skill-report-aggregate.py \\
        --public-key pub.pem --json *.json

Reports whose signature fails to verify are SKIPPED (with a warning to
stderr) and excluded from the aggregate — unverified evidence never enters
a cohort summary (T-18-21 mitigation).
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional


PROFICIENT_BANDS = {"Proficient", "Mastered"}


def find_default_forge_sign() -> Optional[str]:
    """Resolve a default `forge-sign` binary path.

    Checks PATH first (via shutil.which), then falls back to the common
    cargo debug-build location relative to this script (repo-root-anchored
    -> learnforge-core/target/debug/forge-sign), so a fresh `cargo build`
    is discoverable without requiring the caller to pass --forge-sign.
    """
    on_path = shutil.which("forge-sign")
    if on_path:
        return on_path

    repo_root = Path(__file__).resolve().parent.parent
    candidate = repo_root / "target" / "debug" / "forge-sign"
    if candidate.exists():
        return str(candidate)
    candidate = repo_root / "learnforge-core" / "target" / "debug" / "forge-sign"
    if candidate.exists():
        return str(candidate)
    return None


def verify_report(forge_sign_bin: str, public_key: Path, report_path: Path) -> Optional[Dict[str, Any]]:
    """Shell out to `forge-sign verify-report` for one report file.

    Returns the parsed JSON result dict on success (valid OR invalid — the
    caller decides what to do with `result["valid"]`), or None if the
    subprocess itself could not be run or its stdout wasn't parseable JSON
    (a genuine tooling failure, distinct from "signature invalid").
    """
    try:
        proc = subprocess.run(
            [
                forge_sign_bin,
                "verify-report",
                "--input",
                str(report_path),
                "--public-key",
                str(public_key),
            ],
            capture_output=True,
            text=True,
        )
    except OSError as exc:
        print(f"warning: could not run forge-sign for {report_path}: {exc}", file=sys.stderr)
        return None

    stdout = proc.stdout.strip()
    if not stdout:
        print(
            f"warning: forge-sign produced no output for {report_path} "
            f"(exit {proc.returncode}); stderr: {proc.stderr.strip()}",
            file=sys.stderr,
        )
        return None

    try:
        result = json.loads(stdout)
    except json.JSONDecodeError as exc:
        print(f"warning: could not parse forge-sign output for {report_path}: {exc}", file=sys.stderr)
        return None

    return result


def load_verified_reports(
    forge_sign_bin: str, public_key: Path, report_paths: List[Path]
) -> List[Dict[str, Any]]:
    """Verify every report file's signature and return the payloads of
    only the ones that verify. Invalid/unverifiable reports are skipped
    with a warning — never included in the aggregate (T-18-21)."""
    verified_payloads: List[Dict[str, Any]] = []

    for report_path in report_paths:
        result = verify_report(forge_sign_bin, public_key, report_path)
        if result is None:
            print(f"skipping {report_path}: forge-sign did not return a result", file=sys.stderr)
            continue
        if not result.get("valid", False):
            print(f"skipping {report_path}: signature invalid (excluded from aggregate)", file=sys.stderr)
            continue

        # forge-sign's own JSON result only carries a few top-level fields
        # (valid/learnerName/capabilityCount/keyFingerprint) — re-read the
        # report file directly for the full payload (capabilities list)
        # needed for group math. The signature has already been verified
        # by forge-sign above; this second read is trusted because
        # `result["valid"]` was true for this exact file.
        try:
            envelope = json.loads(report_path.read_text())
        except (OSError, json.JSONDecodeError) as exc:
            print(f"skipping {report_path}: could not re-read verified report: {exc}", file=sys.stderr)
            continue

        payload = envelope.get("payload")
        if not isinstance(payload, dict):
            print(f"skipping {report_path}: verified but missing payload object", file=sys.stderr)
            continue

        verified_payloads.append(payload)

    return verified_payloads


def normalize_slug(raw: str) -> str:
    """Normalize a capability slug/label so casing variants collapse.

    Mirrors (a simplified stdlib-only version of)
    `learnforge_core::reports::normalize_tag`: lowercase, collapse runs of
    non-alphanumeric characters to '-', trim leading/trailing '-'. Kept
    intentionally independent (no crypto, pure string munging) — this is
    NOT the crypto boundary D-14/T-18-22 cares about.
    """
    lower = raw.lower()
    out_chars: List[str] = []
    last_was_sep = True
    for ch in lower:
        if ch.isalnum():
            out_chars.append(ch)
            last_was_sep = False
        elif not last_was_sep:
            out_chars.append("-")
            last_was_sep = True
    out = "".join(out_chars).strip("-")
    return out


def build_group_summary(payloads: List[Dict[str, Any]]) -> Dict[str, Any]:
    """Compute the D-14 group summary from verified report payloads.

    Returns a dict with:
      - learner_count: number of verified reports included
      - capabilities: per-capability-slug dict of
          { label, learner_count, band_distribution, completion_proficient_plus,
            mean_knowledge_pct, mean_practical_pct, practical_not_assessed_count }
    """
    # slug -> accumulator
    acc: Dict[str, Dict[str, Any]] = {}
    order: List[str] = []

    for payload in payloads:
        capabilities = payload.get("capabilities", [])
        for cap in capabilities:
            raw_slug = cap.get("slug") or cap.get("label", "")
            slug = normalize_slug(raw_slug)
            if not slug:
                continue

            if slug not in acc:
                order.append(slug)
                acc[slug] = {
                    "label": cap.get("label", raw_slug),
                    "learner_count": 0,
                    "band_distribution": {},
                    "proficient_plus_count": 0,
                    "knowledge_pcts": [],
                    "practical_pcts": [],
                    "practical_not_assessed_count": 0,
                }

            entry = acc[slug]
            entry["learner_count"] += 1

            knowledge = cap.get("knowledge") or {}
            band = knowledge.get("band", "Unknown")
            entry["band_distribution"][band] = entry["band_distribution"].get(band, 0) + 1
            if band in PROFICIENT_BANDS:
                entry["proficient_plus_count"] += 1
            if isinstance(knowledge.get("pct"), (int, float)):
                entry["knowledge_pcts"].append(knowledge["pct"])

            practical = cap.get("practical")
            if practical is None:
                entry["practical_not_assessed_count"] += 1
            elif isinstance(practical.get("pct"), (int, float)):
                entry["practical_pcts"].append(practical["pct"])

    capabilities_summary: Dict[str, Any] = {}
    for slug in order:
        entry = acc[slug]
        learner_count = entry["learner_count"]
        knowledge_pcts = entry["knowledge_pcts"]
        practical_pcts = entry["practical_pcts"]

        mean_knowledge = sum(knowledge_pcts) / len(knowledge_pcts) if knowledge_pcts else None
        mean_practical = sum(practical_pcts) / len(practical_pcts) if practical_pcts else None
        completion_proficient_plus = (
            entry["proficient_plus_count"] / learner_count if learner_count else 0.0
        )

        capabilities_summary[slug] = {
            "label": entry["label"],
            "learnerCount": learner_count,
            "bandDistribution": entry["band_distribution"],
            "completionProficientPlus": completion_proficient_plus,
            "meanKnowledgePct": mean_knowledge,
            "meanPracticalPct": mean_practical,
            "practicalNotAssessedCount": entry["practical_not_assessed_count"],
        }

    return {
        "learnerCount": len(payloads),
        "capabilities": capabilities_summary,
    }


def render_text_summary(summary: Dict[str, Any]) -> str:
    """Render the group summary as manager-readable plain text."""
    lines: List[str] = []
    lines.append(f"Verified reports in cohort: {summary['learnerCount']}")
    lines.append("")

    for slug, cap in summary["capabilities"].items():
        lines.append(f"- {cap['label']} ({slug})")
        lines.append(f"    learners assessed: {cap['learnerCount']}")

        band_dist = ", ".join(f"{band}: {count}" for band, count in cap["bandDistribution"].items())
        lines.append(f"    knowledge band distribution: {band_dist or 'n/a'}")

        pct_str = (
            f"{cap['completionProficientPlus'] * 100:.0f}%"
            if cap["learnerCount"]
            else "n/a"
        )
        lines.append(f"    completion (Proficient+): {pct_str}")

        knowledge_str = (
            f"{cap['meanKnowledgePct'] * 100:.0f}%" if cap["meanKnowledgePct"] is not None else "n/a"
        )
        practical_str = (
            f"{cap['meanPracticalPct'] * 100:.0f}%" if cap["meanPracticalPct"] is not None else "not assessed"
        )
        lines.append(f"    mean knowledge pct: {knowledge_str} | mean practical pct: {practical_str}")
        if cap["practicalNotAssessedCount"]:
            lines.append(
                f"    ({cap['practicalNotAssessedCount']} learner(s) have no lab content for this capability — excluded from practical mean)"
            )
        lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="skill-report-aggregate.py",
        description=(
            "Verify N signed skill-report JSON files (via forge-sign) and print a "
            "group summary: per-capability distribution, completion, and "
            "practical-vs-knowledge mastery. Unverified/tampered reports are excluded."
        ),
    )
    parser.add_argument(
        "reports",
        nargs="+",
        type=Path,
        help="Path(s) to exported report JSON files (ReportEnvelopeV1 shape).",
    )
    parser.add_argument(
        "--public-key",
        required=True,
        type=Path,
        help="Path to the signing public-key PEM to verify reports against.",
    )
    parser.add_argument(
        "--forge-sign",
        type=str,
        default=None,
        help="Path to the forge-sign binary (default: resolve from PATH or target/debug).",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit the group summary as machine-readable JSON instead of text.",
    )
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    parser = build_arg_parser()
    args = parser.parse_args(argv)

    forge_sign_bin = args.forge_sign or find_default_forge_sign()
    if not forge_sign_bin:
        print(
            "error: could not locate the forge-sign binary. Pass --forge-sign <path> "
            "or build it with `cargo build -p learnforge-core --bin forge-sign`.",
            file=sys.stderr,
        )
        return 2

    if not args.public_key.exists():
        print(f"error: public key not found: {args.public_key}", file=sys.stderr)
        return 2

    verified_payloads = load_verified_reports(forge_sign_bin, args.public_key, args.reports)

    if not verified_payloads:
        print("error: no verified reports to aggregate (all inputs invalid or unreadable)", file=sys.stderr)
        return 1

    summary = build_group_summary(verified_payloads)

    if args.json:
        print(json.dumps(summary, indent=2))
    else:
        print(render_text_summary(summary), end="")

    return 0


if __name__ == "__main__":
    sys.exit(main())
