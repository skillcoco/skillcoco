"""
Pydantic v2 data contracts for the enrichment pipeline.

Provides:
  - MCQQuestion  — single multiple-choice question with 4 distinct choices
  - QuizOutput   — validated quiz (5-10 questions)
  - LessonOutput — metadata wrapper for generated lesson markdown
  - quiz_output_to_payload() — adapter producing the exact quiz_payload() shape
                               expected by sheet2pack.py (string correctOptionId,
                               opt-{qi}-{oi} ids, 1-based qi/oi).

This module MUST import cleanly with no dependency on sheet2pack.py.
"""
from pydantic import BaseModel, Field, field_validator


class MCQQuestion(BaseModel):
    """A single multiple-choice question with exactly 4 distinct choices."""

    question: str = Field(..., min_length=10)
    choices: list[str] = Field(..., min_length=4, max_length=4)
    correct_index: int = Field(..., ge=0, le=3)
    explanation: str = Field(..., min_length=10)

    @field_validator("choices")
    @classmethod
    def choices_are_distinct(cls, v: list[str]) -> list[str]:
        """Choices must be distinct (case-insensitive, after stripping whitespace)."""
        normalized = [c.strip().lower() for c in v]
        if len(set(normalized)) != len(normalized):
            raise ValueError("All answer choices must be distinct (case-insensitive)")
        return v


class QuizOutput(BaseModel):
    """Validated quiz containing 5–10 MCQ questions."""

    questions: list[MCQQuestion] = Field(..., min_length=5, max_length=10)


class LessonOutput(BaseModel):
    """Metadata wrapper for AI-generated lesson markdown.

    The markdown field must:
    - Be at least 100 characters long
    - Contain at least one markdown heading (line starting with '#')

    The word_count field must be at least 50 words.
    """

    markdown: str = Field(..., min_length=100)
    word_count: int = Field(..., ge=50)
    prompt_version: str

    @field_validator("markdown")
    @classmethod
    def has_heading(cls, v: str) -> str:
        """Lesson markdown must contain at least one heading (line starting with '#')."""
        if not any(line.startswith("#") for line in v.splitlines()):
            raise ValueError(
                "Generated lesson must contain at least one markdown heading "
                "(a line starting with '#')"
            )
        return v


def quiz_output_to_payload(qo: QuizOutput, module_slug: str) -> dict:
    """Convert a validated QuizOutput to the quiz_payload() shape used by sheet2pack.py.

    Produces:
      {
        "questions": [
          {
            "id": "q-{module_slug}-{qi}",       # 1-based qi
            "stem": "...",
            "options": [{"id": "opt-{qi}-{oi}", "text": "..."}],  # 1-based oi
            "correctOptionId": "opt-{qi}-{oi}",  # STRING — correct_index + 1
            "explanation": "...",
          }
        ]
      }

    The D-adapter bridge: correct_index (0-based int) → correctOptionId (1-based string).
    Per Pitfall 5 in AI-SPEC: correctOptionId = f"opt-{qi}-{correct_index + 1}".
    """
    questions = []
    for qi, q in enumerate(qo.questions, 1):
        options = [
            {"id": f"opt-{qi}-{oi}", "text": choice}
            for oi, choice in enumerate(q.choices, 1)
        ]
        correct_option_id = f"opt-{qi}-{q.correct_index + 1}"
        questions.append(
            {
                "id": f"q-{module_slug}-{qi}",
                "stem": q.question,
                "options": options,
                "correctOptionId": correct_option_id,
                "explanation": q.explanation,
            }
        )
    return {"questions": questions}
