"""
LLM judge for the enrichment pipeline (Tier 2 — advisory until calibrated).

AI-SPEC Section 5 Eval Tooling:
  Grades sampled artifacts on E3/E5/E6/E7 dimensions via forced tool_use
  at temperature=0. Returns structured score (1-5 + reason + calibrated=False).

Security (T-17-11, T-17-02):
  - Forced tool_use constrains judge output to a schema-defined score; injection
    cannot produce arbitrary output (T-17-11).
  - API key is passed to the client by the caller — this module never reads or
    logs the key value (T-17-02).
  - Judge output is advisory only (calibrated=False) until founder calibration
    confirms >= 0.7 agreement with instructor scores.
"""
from __future__ import annotations

import logging
from typing import Any

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

JUDGE_MODEL = "claude-sonnet-4-6"
JUDGE_TEMPERATURE = 0  # Must be 0 for deterministic, unbiased scoring (AI-SPEC §5)

# Forced tool_use schema for structured score output
_EMIT_SCORE_TOOL: dict[str, Any] = {
    "name": "emit_score",
    "description": "Emit the evaluation score for the artifact on the specified dimension.",
    "input_schema": {
        "type": "object",
        "required": ["score", "reason"],
        "properties": {
            "score": {
                "type": "integer",
                "minimum": 1,
                "maximum": 5,
                "description": "Quality score: 1=poor, 3=acceptable, 5=excellent",
            },
            "reason": {
                "type": "string",
                "description": "Concise explanation referencing the artifact and rubric.",
            },
        },
    },
}

# Rubric system prompts per dimension
_RUBRICS: dict[str, str] = {
    "E3": """\
You are an expert evaluator for technical educational content. Score the lesson's
transcript fidelity / grounding on a 1-5 scale.

Rubric:
  5 = Every factual claim, command, and tool name traces exactly to the transcript.
  4 = Nearly all claims grounded; at most 1 minor embellishment.
  3 = Most claims grounded; 2-3 unverified details.
  2 = Several claims introduce concepts not in the transcript.
  1 = Lesson introduces material the instructor never mentioned.

Instructions:
- Compare every command, flag, tool name, and factual claim in the lesson against the transcript.
- Do NOT penalize for voice/style; only penalize for factual additions not in the transcript.
- Score based only on what is provided; do not use outside knowledge.
""",
    "E5": """\
You are an expert evaluator for multiple-choice quiz questions. Score the answer-key
defensibility on a 1-5 scale for the ENTIRE quiz.

Rubric:
  5 = Every correct answer is unambiguously correct and derivable from the lesson; all
      explanations point to what was taught.
  4 = Nearly all questions correct; at most 1 borderline case.
  3 = Most questions defensible; 2-3 have ambiguous or partially-correct distractors.
  2 = Several questions where another choice could be argued correct.
  1 = Multiple questions with indefensible answer keys.

Instructions:
- Read the provided lesson, then evaluate the quiz.
- For each question, ask: "Could a careful reader of this lesson get this wrong or argue
  another choice is correct?"
- Score the quiz as a whole.
""",
    "E6": """\
You are an expert evaluator for instructor voice preservation in educational content.
Score the voice preservation on a 1-5 scale.

Rubric:
  5 = The instructor's specific analogies, examples, and characteristic phrasing are
      faithfully present; conversational register retained.
  4 = Most voice elements preserved; minor smoothing of phrasing.
  3 = Some analogies/examples present but several flattened to generic prose.
  2 = Most specific examples replaced; reads like generic textbook content.
  1 = Completely generic; no trace of the original instructor's voice.

Instructions:
- Compare the lesson to the transcript for characteristic phrases, analogies, and examples.
- Penalize for replacing concrete instructor examples with generic substitutes.
- Do NOT penalize for minor grammatical cleanup or improved sentence flow.
""",
    "E7": """\
You are an expert evaluator for multiple-choice quiz distractor quality.
Score the distractor plausibility on a 1-5 scale for the ENTIRE quiz.

Rubric:
  5 = All distractors are plausible-but-wrong items a beginner might confuse; none are
      absurd, near-duplicate, or trivially dismissible.
  4 = Nearly all distractors strong; at most 1 weak/obvious wrong answer.
  3 = Most distractors plausible; 2-3 obviously wrong or near-duplicate.
  2 = Several questions with absurd or near-duplicate distractors.
  1 = Most distractors are unrealistic; quiz is too easy to guess by elimination.

Instructions:
- For each question, evaluate whether a beginner who knows the topic could plausibly
  choose each wrong answer.
- Penalize near-duplicates and obviously-wrong choices.
- Score the quiz as a whole.
""",
}

SUPPORTED_DIMENSIONS = frozenset(_RUBRICS.keys())


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def judge(
    client,
    artifact_text: str,
    transcript: str,
    dimension: str,
) -> dict:
    """Grade an artifact on a single eval dimension via LLM judge.

    Issues ONE forced tool_use call at temperature=0 returning a structured score
    (1-5 + reason). Output is marked advisory (calibrated=False) until the founder
    reaches >= 0.7 agreement with instructor scores (AI-SPEC §5 labeling / §7 alert).

    Security (T-17-11): forced tool_choice + schema-constrained output prevents
    injection from steering the score to arbitrary values.
    Security (T-17-02): the client is provided by the caller; this function never
    reads, logs, or writes the API key.

    Args:
        client:        Anthropic client instance (Anthropic or AsyncAnthropic).
                       For sync usage (tests), pass a sync client.
        artifact_text: The generated lesson or quiz text to be evaluated.
        transcript:    Source transcript used to generate the artifact.
        dimension:     Eval dimension: one of "E3", "E5", "E6", "E7".

    Returns:
        dict with keys: dimension, score (int 1-5), reason (str), calibrated (False).

    Raises:
        ValueError: if dimension is not supported.
        anthropic.APIError: propagated from the API call.
    """
    if dimension not in SUPPORTED_DIMENSIONS:
        raise ValueError(
            f"Unsupported dimension '{dimension}'. "
            f"Supported: {sorted(SUPPORTED_DIMENSIONS)}"
        )

    rubric = _RUBRICS[dimension]
    user_message = (
        f"TRANSCRIPT:\n{transcript}\n\n"
        f"---\n\n"
        f"ARTIFACT TO EVALUATE:\n{artifact_text}"
    )

    response = client.messages.create(
        model=JUDGE_MODEL,
        max_tokens=512,
        temperature=JUDGE_TEMPERATURE,
        system=rubric,
        messages=[{"role": "user", "content": user_message}],
        tools=[_EMIT_SCORE_TOOL],
        tool_choice={"type": "tool", "name": "emit_score"},
    )

    # Extract the emit_score tool_use block
    score_block = next(
        (b for b in response.content if getattr(b, "type", None) == "tool_use"
         and b.name == "emit_score"),
        None,
    )
    if score_block is None:
        logger.warning(
            "Judge response for dimension %s did not contain emit_score tool block",
            dimension,
        )
        return {
            "dimension": dimension,
            "score": None,
            "reason": "No score block in response",
            "calibrated": False,
        }

    return {
        "dimension": dimension,
        "score": score_block.input["score"],
        "reason": score_block.input["reason"],
        "calibrated": False,  # advisory until founder calibration >= 0.7 (AI-SPEC §5)
    }


def judge_sample(
    artifacts: list[dict],
    sample_n: int,
    client,
    *,
    dimensions: list[str] | None = None,
) -> list[dict]:
    """Grade a signal-weighted sample of artifacts via LLM judge.

    Iterates over the first sample_n artifacts (caller is responsible for ordering
    by signal weight: flagged items first, then random remainder).

    Args:
        artifacts:  List of dicts, each with keys: "artifact_text", "transcript",
                    and optionally "video_id" or "module_slug" for identification.
        sample_n:   Maximum number of artifacts to evaluate.
        client:     Anthropic client instance (sync).
        dimensions: Dimensions to grade (default: all supported E3/E5/E6/E7).

    Returns:
        List of result dicts (one per artifact × dimension), each with
        keys: artifact_id, dimension, score, reason, calibrated.
    """
    if dimensions is None:
        dimensions = sorted(SUPPORTED_DIMENSIONS)

    results = []
    for artifact in artifacts[:sample_n]:
        artifact_id = artifact.get("video_id") or artifact.get("module_slug") or "unknown"
        for dim in dimensions:
            try:
                score_result = judge(
                    client,
                    artifact_text=artifact["artifact_text"],
                    transcript=artifact["transcript"],
                    dimension=dim,
                )
                score_result["artifact_id"] = artifact_id
                results.append(score_result)
            except Exception as exc:
                logger.warning(
                    "Judge failed for artifact %s on dimension %s: %s",
                    artifact_id, dim, exc,
                )
                results.append({
                    "artifact_id": artifact_id,
                    "dimension": dim,
                    "score": None,
                    "reason": f"Judge error: {exc}",
                    "calibrated": False,
                })

    return results
