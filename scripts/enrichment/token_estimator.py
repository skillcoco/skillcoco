"""
Pre-run cost estimation gate for the enrichment pipeline (D-15).

Provides:
  estimate_and_confirm(transcripts, module_count, yes=False) -> None

Prints a rough cost estimate (lesson count, transcript token approximation, USD)
and prompts the user to proceed. If yes=True, prints and returns without prompting.

Token counting uses the ~4 chars/token approximation (OpenAI/Anthropic average).
An exact count via client.messages.count_tokens() is available as a refinement
but is not required for the D-15 pre-run rough gate.

Pricing constants reflect claude-sonnet-4-6 at $3/1M input, $15/1M output.
Stdlib-only — no anthropic SDK imports required for this module.
"""
import sys


# ---------------------------------------------------------------------------
# Pricing constants (claude-sonnet-4-6)
# ---------------------------------------------------------------------------

PRICE_PER_1M_INPUT = 3.00     # $3 per 1M input tokens
PRICE_PER_1M_OUTPUT = 15.00   # $15 per 1M output tokens

# Rough per-generation output estimates for cost approximation
AVG_OUTPUT_TOKENS_PER_LESSON = 800
AVG_OUTPUT_TOKENS_PER_QUIZ = 600


# ---------------------------------------------------------------------------
# Token approximation helper
# ---------------------------------------------------------------------------

def _count_tokens_approx(text: str) -> int:
    """Rough token estimate: ~4 chars per token (OpenAI/Anthropic average).

    Note: An exact count via client.messages.count_tokens() is available as a
    refinement but requires a live API client. The char-approx is sufficient for
    the D-15 pre-run gate which is explicitly a "rough $" estimate.
    """
    return len(text) // 4


# ---------------------------------------------------------------------------
# Estimate + confirm gate (D-15)
# ---------------------------------------------------------------------------

def estimate_and_confirm(
    transcripts: dict[str, str],
    module_count: int,
    yes: bool = False,
) -> None:
    """Print a pre-run cost estimate and gate on user confirmation (D-15).

    Args:
        transcripts: mapping of video_id → transcript text.
        module_count: number of modules (used for quiz output token estimate).
        yes: if True, print estimate and return without prompting (--yes CLI flag).

    Exits via sys.exit(0) if user answers anything other than 'y'.
    Returns None on proceed (yes=True or user confirms with 'y').
    """
    lesson_count = len(transcripts)
    total_input_tokens = sum(_count_tokens_approx(t) for t in transcripts.values())
    lesson_output_tokens = lesson_count * AVG_OUTPUT_TOKENS_PER_LESSON
    quiz_output_tokens = module_count * AVG_OUTPUT_TOKENS_PER_QUIZ
    total_output_tokens = lesson_output_tokens + quiz_output_tokens

    est_cost = (
        (total_input_tokens / 1_000_000) * PRICE_PER_1M_INPUT
        + (total_output_tokens / 1_000_000) * PRICE_PER_1M_OUTPUT
    )

    print("\nEnrichment estimate:")
    print(f"  lessons to generate : {lesson_count}")
    print(f"  transcript tokens   : ~{total_input_tokens:,}")
    print(f"  estimated cost      : ~${est_cost:.3f} USD")

    if yes:
        return  # --yes flag: skip the prompt

    ans = input("Proceed? [y/N] ").strip().lower()
    if ans != "y":
        print("Aborted.")
        sys.exit(0)
