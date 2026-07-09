"""
Evaluation harness for the enrichment pipeline.

AI-SPEC Sections 5-7:
  - checks.py  — Tier 1 deterministic code screens (E1-E5/E7), no API calls
  - judge.py   — Tier 2 LLM judge (E3/E5/E6/E7), forced tool_use, temperature=0
  - report.py  — Per-run JSON + markdown report with Section 7 metrics + alert thresholds

CI usage (no API key needed):
    pytest scripts/enrichment/eval/ -v
"""
