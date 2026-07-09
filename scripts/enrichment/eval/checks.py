"""
Deterministic evaluation checks (Tier 1) for the enrichment pipeline.

AI-SPEC Section 5 — Tier 1 deterministic code screens:
  E1  schema validity     — QuizOutput Pydantic validation, >= 5 questions
  E2  no truncation       — quiz len >= 5, stop_reason guard (checked by caller)
  E3  grounding           — grounding_ratio < threshold or missing fenced command token
  E4  technical accuracy  — fenced command token diff against transcript
  E5  answer-key          — correct_index valid, choices distinct, stem term overlap
  E7  distractor          — 4 distinct choices, near-duplicate detection

All checks run with NO API key — stdlib + pydantic only.
None of these functions raise on bad input; they return a structured Result.

Security:
  - No network calls; no subprocess; no file I/O.
  - Transcript and lesson text are treated as untrusted strings (no eval/exec).
"""
import difflib
import re
from dataclasses import dataclass, field
from typing import Literal

from pydantic import ValidationError

from ..models import MCQQuestion, QuizOutput


# ---------------------------------------------------------------------------
# Result type
# ---------------------------------------------------------------------------

Status = Literal["pass", "flag", "fail"]


@dataclass
class Result:
    """Structured result for a single evaluation check.

    Attributes:
        dimension: Eval dimension identifier (e.g. "E1", "E3").
        status:    "pass" | "flag" | "fail"
        reason:    Human-readable explanation; empty string on "pass".
        details:   Optional extra diagnostic data (e.g. list of missing tokens).
    """

    dimension: str
    status: Status
    reason: str = ""
    details: dict = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Token extraction helpers
# ---------------------------------------------------------------------------

# Matches fenced code blocks: ```[lang]\n...\n```
_FENCED_BLOCK_RE = re.compile(r"```[^\n]*\n(.*?)```", re.DOTALL)

# Command-shaped tokens: words containing hyphens or starting with -- (flags),
# backtick-quoted items, or words that look like CLI tool names (kubectl, pip, etc.)
_BACKTICK_TOKEN_RE = re.compile(r"`([^`]+)`")

# A "command token" looks like a CLI tool name or flag:
# - starts with a letter and contains a hyphen (e.g. "kubectl", "rollout", "undo")
# - OR is a flag starting with "-"
_CLI_TOKEN_RE = re.compile(r"\b([a-zA-Z][a-zA-Z0-9]*(?:-[a-zA-Z0-9]+)+|--?[a-zA-Z][a-zA-Z0-9-]*)\b")


def _extract_command_tokens(text: str) -> set[str]:
    """Extract fenced-code-block contents + backtick tokens + CLI-shaped tokens from text.

    Returns a set of normalized (lowercase, stripped) tokens representing
    commands, flags, and tool names present in the text.

    Fenced block language identifiers (e.g. ``bash``, ``python``) are excluded
    because they are metadata, not commands introduced by the lesson.
    """
    tokens: set[str] = set()

    # Common fenced-block language identifiers — these are metadata, not commands.
    _LANG_IDENTIFIERS = frozenset(
        ["bash", "sh", "shell", "python", "py", "javascript", "js", "yaml",
         "json", "go", "rust", "ruby", "text", "plaintext", "console",
         "dockerfile", "makefile", "sql", "xml", "html", "css"]
    )

    # 1. Extract tokens from fenced code blocks (body content only, not the fence header)
    for block in _FENCED_BLOCK_RE.findall(text):
        # Split each line into words — include whole-line commands for lookup
        for line in block.splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            # Add the entire command line as a token (for multi-word lookups)
            tokens.add(line.lower())
            # Add individual words/tokens from the line
            for word in line.split():
                cleaned = word.lower().strip(".,;:\"'()[]")
                if cleaned and cleaned not in _LANG_IDENTIFIERS:
                    tokens.add(cleaned)

    # 2. Extract backtick-quoted tokens — strip fenced blocks first to avoid
    #    matching triple-fence content with the single-backtick regex.
    text_without_fences = _FENCED_BLOCK_RE.sub("", text)
    for tok in _BACKTICK_TOKEN_RE.findall(text_without_fences):
        cleaned_tok = tok.strip().lower()
        if cleaned_tok not in _LANG_IDENTIFIERS:
            tokens.add(cleaned_tok)
        # Also add sub-tokens from multi-word backtick sequences
        for word in tok.split():
            cleaned = word.lower().strip(".,;:\"'()[]")
            if cleaned and cleaned not in _LANG_IDENTIFIERS:
                tokens.add(cleaned)

    # 3. Extract CLI-shaped tokens from plain text (also from fences-stripped text)
    for match in _CLI_TOKEN_RE.finditer(text_without_fences):
        tokens.add(match.group(0).lower())

    # Remove empty or pure-whitespace tokens
    tokens.discard("")
    return tokens


def grounding_ratio(lesson_md: str, transcript: str) -> float:
    """Return the fraction of command/tool tokens in lesson_md that appear in transcript.

    Extracts command tokens from the lesson (fenced blocks, backtick quotes, CLI
    flag patterns) and checks each against the transcript text (case-insensitive).

    Returns 1.0 when the lesson introduces no command/tool tokens (nothing to check).

    Args:
        lesson_md:  Generated lesson markdown text.
        transcript: Source transcript text from which the lesson was generated.

    Returns:
        float in [0.0, 1.0] — fraction of lesson command tokens found in transcript.
    """
    tokens = _extract_command_tokens(lesson_md)
    if not tokens:
        return 1.0

    transcript_lower = transcript.lower()
    found = sum(1 for t in tokens if t in transcript_lower)
    return found / len(tokens)


def _get_fenced_command_tokens(lesson_md: str) -> set[str]:
    """Return the set of tokens from fenced code blocks in lesson_md (normalized)."""
    tokens: set[str] = set()
    for block in _FENCED_BLOCK_RE.findall(lesson_md):
        for line in block.splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            for word in line.split():
                tokens.add(word.lower().strip(".,;:\"'()[]"))
    return tokens


# ---------------------------------------------------------------------------
# E3 / E4 — Grounding check
# ---------------------------------------------------------------------------

def check_grounding(
    lesson_md: str,
    transcript: str,
    threshold: float = 0.8,
) -> Result:
    """Check E3/E4: transcript fidelity and technical accuracy.

    Flags lessons where:
    - grounding_ratio < threshold (low term overlap), OR
    - any fenced code block token is absent from the transcript (E4 command diff).

    A "flag" means the lesson requires founder review; it does NOT block shipment.
    Returns "pass" when grounding_ratio >= threshold AND all fenced tokens found.

    Args:
        lesson_md:  Generated lesson markdown.
        transcript: Source transcript.
        threshold:  Minimum grounding ratio (default 0.8).

    Returns:
        Result with dimension="E3/E4", status in {"pass", "flag"}.
    """
    ratio = grounding_ratio(lesson_md, transcript)
    transcript_lower = transcript.lower()

    # E4: check fenced code block tokens specifically
    fenced_tokens = _get_fenced_command_tokens(lesson_md)
    missing_tokens = [t for t in fenced_tokens if t not in transcript_lower]

    if ratio < threshold or missing_tokens:
        reasons = []
        if ratio < threshold:
            reasons.append(f"grounding_ratio={ratio:.2f} < threshold={threshold:.2f}")
        if missing_tokens:
            reasons.append(f"fenced command tokens absent from transcript: {missing_tokens}")
        return Result(
            dimension="E3/E4",
            status="flag",
            reason="needs_review: " + "; ".join(reasons),
            details={
                "grounding_ratio": ratio,
                "missing_fenced_tokens": missing_tokens,
                "threshold": threshold,
            },
        )

    return Result(
        dimension="E3/E4",
        status="pass",
        reason="",
        details={"grounding_ratio": ratio},
    )


# ---------------------------------------------------------------------------
# E1 / E2 — Schema and truncation check
# ---------------------------------------------------------------------------

def check_quiz_schema(quiz_dict: dict) -> Result:
    """Check E1/E2: schema validity and minimum question count.

    Wraps QuizOutput(**quiz_dict) in a try/except. Returns "fail" on:
    - Pydantic ValidationError (schema invalid, wrong types, out-of-range values)
    - fewer than 5 questions (E2 truncation proxy)

    Returns "pass" when the quiz_dict validates as a full QuizOutput.

    Args:
        quiz_dict: Raw quiz dict (e.g. from tool_use block or fixture file).

    Returns:
        Result with dimension="E1/E2", status in {"pass", "fail"}.
    """
    try:
        qo = QuizOutput(**quiz_dict)
    except (ValidationError, TypeError) as exc:
        return Result(
            dimension="E1/E2",
            status="fail",
            reason=f"Schema validation failed: {exc}",
            details={"error": str(exc)},
        )

    n = len(qo.questions)
    if n < 5:
        return Result(
            dimension="E1/E2",
            status="fail",
            reason=f"Quiz has {n} questions (minimum 5 required, E2)",
            details={"question_count": n},
        )

    return Result(
        dimension="E1/E2",
        status="pass",
        reason="",
        details={"question_count": n},
    )


# ---------------------------------------------------------------------------
# E5 — Answer-key check
# ---------------------------------------------------------------------------

def check_answer_key(quiz_dict: dict, module_transcript: str) -> Result:
    """Check E5: answer-key defensibility screen.

    Asserts:
    - correct_index is in [0, 3] for each question
    - choices are distinct (case-insensitive)
    - question stem terms overlap with the module transcript

    Args:
        quiz_dict:         Raw quiz dict (questions list).
        module_transcript: Concatenated transcript text for the module.

    Returns:
        Result with dimension="E5", status in {"pass", "flag", "fail"}.
    """
    questions = quiz_dict.get("questions", [])
    if not questions:
        return Result(
            dimension="E5",
            status="fail",
            reason="No questions found in quiz_dict",
        )

    transcript_lower = module_transcript.lower()
    issues: list[str] = []

    for i, q in enumerate(questions):
        qi = i + 1  # 1-based for error messages

        # Validate correct_index range
        correct_index = q.get("correct_index")
        if correct_index is None or not isinstance(correct_index, int):
            issues.append(f"Q{qi}: correct_index missing or not an integer")
        elif correct_index < 0 or correct_index > 3:
            issues.append(f"Q{qi}: correct_index={correct_index} out of range [0, 3]")

        # Validate choices are distinct
        choices = q.get("choices", [])
        if len(choices) < 2:
            issues.append(f"Q{qi}: fewer than 2 choices")
        else:
            normalized_choices = [c.strip().lower() for c in choices]
            if len(set(normalized_choices)) != len(normalized_choices):
                issues.append(f"Q{qi}: choices are not distinct (case-insensitive)")

        # Check stem-term overlap with transcript (at least one word from stem in transcript)
        stem = q.get("question", "")
        # Extract meaningful words (>= 4 chars) from the stem
        stem_words = [
            w.lower().strip(".,;:?!")
            for w in stem.split()
            if len(w.strip(".,;:?!")) >= 4
        ]
        if stem_words:
            overlap = [w for w in stem_words if w in transcript_lower]
            if not overlap:
                issues.append(
                    f"Q{qi}: no stem words found in transcript (grounding gap) — "
                    f"stem words: {stem_words[:5]}"
                )

    if issues:
        return Result(
            dimension="E5",
            status="flag",
            reason="Answer-key issues: " + "; ".join(issues),
            details={"issues": issues},
        )

    return Result(dimension="E5", status="pass", reason="")


# ---------------------------------------------------------------------------
# E7 — Distractor check
# ---------------------------------------------------------------------------

def _normalize_choice(choice: str) -> str:
    """Normalize a choice for near-duplicate detection."""
    return re.sub(r"\s+", " ", choice.strip().lower())


def check_distractors(quiz_dict: dict) -> Result:
    """Check E7: distractor plausibility screen.

    Detects:
    - fewer than 4 distinct choices (case-insensitive) per question
    - near-duplicate choices (normalized string similarity >= 0.85)

    Args:
        quiz_dict: Raw quiz dict (questions list).

    Returns:
        Result with dimension="E7", status in {"pass", "flag"}.
    """
    questions = quiz_dict.get("questions", [])
    if not questions:
        return Result(
            dimension="E7",
            status="fail",
            reason="No questions found in quiz_dict",
        )

    issues: list[str] = []
    SIMILARITY_THRESHOLD = 0.85

    for i, q in enumerate(questions):
        qi = i + 1
        choices = q.get("choices", [])
        if len(choices) < 4:
            issues.append(f"Q{qi}: fewer than 4 choices ({len(choices)} found)")
            continue

        normalized = [_normalize_choice(c) for c in choices]

        # Check exact duplicates (case-insensitive, whitespace-normalized)
        if len(set(normalized)) < len(normalized):
            issues.append(f"Q{qi}: choices contain exact duplicates after normalization")
            continue

        # Check near-duplicates via SequenceMatcher
        n = len(normalized)
        for a in range(n):
            for b in range(a + 1, n):
                ratio = difflib.SequenceMatcher(
                    None, normalized[a], normalized[b]
                ).ratio()
                if ratio >= SIMILARITY_THRESHOLD:
                    issues.append(
                        f"Q{qi}: choices[{a}] and choices[{b}] are near-duplicates "
                        f"(similarity={ratio:.2f}): "
                        f"'{choices[a]}' vs '{choices[b]}'"
                    )

    if issues:
        return Result(
            dimension="E7",
            status="flag",
            reason="Distractor issues: " + "; ".join(issues),
            details={"issues": issues},
        )

    return Result(dimension="E7", status="pass", reason="")
