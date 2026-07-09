"""
MCQ quiz generator for the enrichment pipeline.

Generates transcript-grounded multiple-choice quizzes per module using
the Anthropic API with forced tool_use for guaranteed structured JSON output.

Key design decisions:
  - D-09: Fill-gaps-only — modules with matched hand-authored xlsx quizzes are skipped
  - D-10: Quiz size scales with module lesson count, clamped to [5, 10]
  - D-11: Mix of recall + applied/scenario questions (instructed via system prompt)
  - D-12: All lesson transcripts of the module used as grounding input
  - D-14: Per-module quiz cache keyed by (module_id, transcripts_hash, prompt_version)
  - D-16: Continue-on-failure — after 3 failed attempts, module added to failed list
  - E1: Output shape matches quiz_payload() via quiz_output_to_payload() adapter
  - E2: stop_reason == "max_tokens" treated as a failure (truncation guard)
  - T-17-02: ANTHROPIC_API_KEY read from env only; value never logged

Security (T-17-02): Client key logic reads from env var only; key value never
logged or stored anywhere.
"""
import asyncio
import logging
import os
import sys
from typing import Any

from pydantic import ValidationError

from .content_cache import quiz_cache_path, read_content_cache, write_content_cache
from .models import QuizOutput, quiz_output_to_payload

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

QUIZ_PROMPT_VERSION = "quiz-v1"  # bump when QUIZ_SYSTEM_PROMPT or QUIZ_TOOL_SCHEMA changes (D-14)

QUIZ_TOOL_NAME = "emit_quiz"

QUIZ_TOOL_SCHEMA: dict[str, Any] = {
    "name": QUIZ_TOOL_NAME,
    "description": "Emit the final MCQ quiz as structured JSON.",
    "input_schema": {
        "type": "object",
        "required": ["questions"],
        "properties": {
            "questions": {
                "type": "array",
                "minItems": 5,
                "maxItems": 10,
                "items": {
                    "type": "object",
                    "required": ["question", "choices", "correct_index", "explanation"],
                    "properties": {
                        "question": {"type": "string"},
                        "choices": {
                            "type": "array",
                            "minItems": 4,
                            "maxItems": 4,
                            "items": {"type": "string"},
                        },
                        "correct_index": {
                            "type": "integer",
                            "minimum": 0,
                            "maximum": 3,
                        },
                        "explanation": {"type": "string"},
                    },
                },
            }
        },
    },
}

# Static system prompt — must not include any dynamic per-call content (AI-SPEC §4b.3).
# Placed in cache_control block for prompt-cache savings across module calls.
QUIZ_SYSTEM_PROMPT = """\
You are an expert instructional designer creating multiple-choice quizzes for \
technical courses. Your job is to generate high-quality questions grounded \
exclusively in the provided lesson transcripts.

Rules:
- ONLY ask about concepts, commands, and ideas explicitly covered in the transcripts.
- Do NOT introduce facts, tools, or concepts not present in the transcripts.
- Mix question types: approximately half concept-recall ("what does X do?") and \
half applied/scenario ("which command would you use to...", "what happens if...").
- Scale question count to module size (5 for small modules, 10 for large ones).
- Every question must have exactly 4 distinct answer choices.
- Distractors must be plausible — real commands/concepts a beginner might confuse, \
not obviously wrong or absurd.
- The correct answer must be unambiguously correct given the transcript content.
- The explanation must reference what the module taught.
- correct_index is 0-based (0 = first choice, 3 = fourth choice).\
"""

MODEL = "claude-sonnet-4-6"
MAX_TOKENS = 2048
TEMPERATURE = 0.2
MAX_RETRIES = 3


# ---------------------------------------------------------------------------
# Client factory (T-17-02: key from env only, never logged)
# ---------------------------------------------------------------------------

def _get_client():
    """Return an AsyncAnthropic client using ANTHROPIC_API_KEY from env.

    Reads key from environment; never logs the key value (T-17-02).
    Exits with a clear message if key is not set.
    """
    key = os.environ.get("ANTHROPIC_API_KEY")
    if not key:
        sys.exit("ERROR: ANTHROPIC_API_KEY environment variable not set. "
                 "Export it before running the enrichment pipeline.")
    from anthropic import AsyncAnthropic
    return AsyncAnthropic(
        api_key=key,
        max_retries=3,   # SDK exponential backoff for 429/500/408/409
        timeout=120.0,   # 2-min timeout; quiz calls can take 30-60 s
    )


# ---------------------------------------------------------------------------
# Num-questions scaling (D-10)
# ---------------------------------------------------------------------------

def _num_questions_for(lesson_count: int) -> int:
    """Return the number of quiz questions for a module with lesson_count lessons.

    Scales linearly from 5 (2 lessons) to 10 (10+ lessons), clamped to [5, 10].
    A 2-lesson module -> 5 questions; a 10+ lesson module -> 10 questions.
    """
    return max(5, min(10, lesson_count))


# ---------------------------------------------------------------------------
# Internal: single-module quiz generation with Pydantic retry (E1 / D-16)
# ---------------------------------------------------------------------------

async def _generate_quiz_with_retry(
    client,
    module_slug: str,
    module_title: str,
    module_transcripts: list[str],
    num_questions: int,
    semaphore: asyncio.Semaphore,
) -> QuizOutput:
    """Generate a quiz for one module with up to 3 attempts (D-16 / E1).

    Uses forced tool_choice to guarantee structured JSON (AI-SPEC §4).
    On ValidationError, appends the error JSON to the user prompt and retries.
    On stop_reason == "max_tokens", treats as failure (E2 truncation guard).
    Raises ValueError after MAX_RETRIES failures.
    """
    combined_transcripts = "\n\n---\n\n".join(module_transcripts)
    base_prompt = (
        f"Generate exactly {num_questions} MCQ questions for this module titled "
        f'"{module_title}". Ground every question in these transcripts.\n\n'
        f"TRANSCRIPTS:\n{combined_transcripts}"
    )
    prompt = base_prompt
    last_error: str = ""

    for attempt in range(1, MAX_RETRIES + 1):
        async with semaphore:
            response = await client.messages.create(
                model=MODEL,
                max_tokens=MAX_TOKENS,
                temperature=TEMPERATURE,
                system=[
                    {
                        "type": "text",
                        "text": QUIZ_SYSTEM_PROMPT,
                        "cache_control": {"type": "ephemeral"},
                    }
                ],
                messages=[{"role": "user", "content": prompt}],
                tools=[QUIZ_TOOL_SCHEMA],
                tool_choice={"type": "tool", "name": QUIZ_TOOL_NAME},
            )

        # E2: truncation guard — assert stop_reason == "end_turn" before parsing.
        # stop_reason == "max_tokens" means the response was truncated; the SDK
        # does NOT raise an exception in this case (AI-SPEC Common Pitfall 1).
        if response.stop_reason != "end_turn":
            last_error = (
                f"Response not 'end_turn' (stop_reason={response.stop_reason!r}) "
                f"on attempt {attempt}"
            )
            logger.warning(
                "Quiz truncated for module %s on attempt %d (stop_reason=%r)",
                module_slug,
                attempt,
                response.stop_reason,
            )
            if attempt < MAX_RETRIES:
                continue
            raise ValueError(
                f"Quiz generation failed after {MAX_RETRIES} attempts for module "
                f"'{module_slug}': {last_error}"
            )

        # Extract the emit_quiz tool_use block
        tool_block = next(
            (
                b
                for b in response.content
                if b.type == "tool_use" and b.name == QUIZ_TOOL_NAME
            ),
            None,
        )
        if tool_block is None:
            last_error = f"No '{QUIZ_TOOL_NAME}' tool call in response"
            logger.warning(
                "No %s tool block for module %s on attempt %d",
                QUIZ_TOOL_NAME,
                module_slug,
                attempt,
            )
            if attempt < MAX_RETRIES:
                continue
            raise ValueError(
                f"Quiz generation failed after {MAX_RETRIES} attempts for module "
                f"'{module_slug}': {last_error}"
            )

        # Pydantic validation (E1 schema gate)
        try:
            return QuizOutput(**tool_block.input)
        except ValidationError as exc:
            last_error = exc.json()
            logger.warning(
                "Quiz validation failed for module %s on attempt %d: %s",
                module_slug,
                attempt,
                last_error,
            )
            if attempt < MAX_RETRIES:
                # Append validation error to prompt so model can self-correct
                prompt = (
                    base_prompt
                    + f"\n\n[RETRY {attempt}: Fix these validation errors: {last_error}]"
                )
                continue
            raise ValueError(
                f"Quiz validation failed after {MAX_RETRIES} attempts for module "
                f"'{module_slug}': {last_error}"
            )

    raise ValueError(
        f"Quiz generation failed after {MAX_RETRIES} attempts for module '{module_slug}'"
    )


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

async def generate_quizzes(
    modules_raw: list[dict],
    transcripts: dict[str, str],
    matched_chapters: set[int],
    failed: list,
    *,
    concurrency: int = 5,
) -> dict[str, dict]:
    """Generate transcript-grounded MCQ quizzes for modules not in matched_chapters.

    Args:
        modules_raw:       Parsed module list from sheet2pack (dicts with num/title/slug/lessons).
        transcripts:       {video_id: text} from transcript fetch stage (D-12 grounding source).
        matched_chapters:  Set of module nums that already have a hand-authored xlsx quiz (D-09).
        failed:            Mutable list; each failure appends (module_slug, error_message).
        concurrency:       Max concurrent Anthropic API calls (default 5 per AI-SPEC §4b.2).

    Returns:
        {module_slug: quiz_payload_dict} for all successfully generated quizzes.
        Modules in matched_chapters are absent from the dict (D-09 fill-gaps-only).
        Modules whose generation fails after 3 attempts are absent and appended to failed.
    """
    client = _get_client()
    semaphore = asyncio.Semaphore(concurrency)
    generated: dict[str, dict] = {}

    async def _process_module(m: dict) -> None:
        module_num = m["num"]
        module_slug = m.get("slug") or f"module-{module_num}"
        module_title = m.get("title", f"Module {module_num}")

        # D-09: fill-gaps-only — skip if hand-authored quiz exists
        if module_num in matched_chapters:
            logger.debug("Module %s in matched_chapters — skipping (D-09)", module_slug)
            return

        # Collect all module lesson transcripts (D-12)
        module_transcripts: list[str] = []
        for lesson in m.get("lessons", []):
            vid = lesson.get("video_id")
            if vid and vid in transcripts:
                module_transcripts.append(transcripts[vid])

        if not module_transcripts:
            logger.warning(
                "Module %s has no transcripts — skipping quiz generation", module_slug
            )
            return

        # D-14: check per-module quiz cache before calling API
        cache_path = quiz_cache_path(module_slug, module_transcripts, QUIZ_PROMPT_VERSION)
        cached = read_content_cache(cache_path)
        if cached is not None:
            logger.debug("Cache hit for module %s quiz (D-14)", module_slug)
            generated[module_slug] = cached
            return

        # D-10: scale num_questions to lesson count
        num_questions = _num_questions_for(len(m.get("lessons", [])))

        # D-16: per-module try/except — failures never raise; added to failed list
        try:
            quiz_output = await _generate_quiz_with_retry(
                client,
                module_slug=module_slug,
                module_title=module_title,
                module_transcripts=module_transcripts,
                num_questions=num_questions,
                semaphore=semaphore,
            )
        except Exception as exc:
            logger.warning(
                "Quiz generation failed for module %s: %s — skipping (D-16)",
                module_slug,
                exc,
            )
            failed.append((module_slug, str(exc)))
            return

        # E1: convert to quiz_payload() shape via plan-01 adapter
        payload = quiz_output_to_payload(quiz_output, module_slug)

        # D-14: write to cache for reruns
        write_content_cache(cache_path, payload)

        generated[module_slug] = payload

    # Process all modules concurrently (bounded by semaphore)
    await asyncio.gather(
        *[_process_module(m) for m in modules_raw],
        return_exceptions=False,  # exceptions are caught inside _process_module
    )

    return generated
