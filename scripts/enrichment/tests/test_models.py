"""
Tests for enrichment.models — MCQQuestion, QuizOutput, LessonOutput, quiz_output_to_payload.

Pre-conditions: pydantic v2 installed (scripts/requirements-enrichment.txt)
Assertions cover: QuizOutput min/max, MCQQuestion distinctness + length,
                  correct_index bounds, LessonOutput.has_heading,
                  and quiz_output_to_payload adapter shape.
"""
import pytest
from pydantic import ValidationError


def _make_question(**kwargs):
    """Helper: create a valid MCQQuestion dict with overrideable fields."""
    base = {
        "question": "What is the purpose of a Kubernetes Deployment resource?",
        "choices": ["Manages pod replicas", "Stores secrets", "Routes traffic", "Defines resource quotas"],
        "correct_index": 0,
        "explanation": "A Deployment ensures the specified number of pod replicas are running.",
    }
    base.update(kwargs)
    return base


def _make_valid_quiz(n_questions=5):
    from enrichment.models import QuizOutput
    questions = []
    for i in range(n_questions):
        questions.append({
            "question": f"Question {i+1}: What does kubectl apply do in a Kubernetes cluster?",
            "choices": [
                f"Option A for question {i+1}",
                f"Option B for question {i+1}",
                f"Option C for question {i+1}",
                f"Option D for question {i+1}",
            ],
            "correct_index": 0,
            "explanation": f"Explanation for question {i+1}: kubectl apply reconciles cluster state.",
        })
    return QuizOutput(questions=questions)


# ---------------------------------------------------------------------------
# QuizOutput constraints
# ---------------------------------------------------------------------------

class TestQuizOutputConstraints:
    def test_valid_5_question_quiz_accepted(self):
        """QuizOutput accepts a valid 5-question quiz and round-trips."""
        from enrichment.models import QuizOutput
        quiz = _make_valid_quiz(5)
        assert len(quiz.questions) == 5

    def test_valid_10_question_quiz_accepted(self):
        """QuizOutput accepts a valid 10-question quiz."""
        from enrichment.models import QuizOutput
        quiz = _make_valid_quiz(10)
        assert len(quiz.questions) == 10

    def test_4_questions_rejected(self):
        """QuizOutput raises ValidationError on 4 questions (below min 5)."""
        from enrichment.models import QuizOutput
        questions = [_make_question() for _ in range(4)]
        with pytest.raises(ValidationError):
            QuizOutput(questions=questions)

    def test_11_questions_rejected(self):
        """QuizOutput raises ValidationError on 11 questions (above max 10)."""
        from enrichment.models import QuizOutput
        questions = [_make_question() for _ in range(11)]
        with pytest.raises(ValidationError):
            QuizOutput(questions=questions)


# ---------------------------------------------------------------------------
# MCQQuestion constraints
# ---------------------------------------------------------------------------

class TestMCQQuestionConstraints:
    def test_non_distinct_choices_rejected(self):
        """MCQQuestion raises ValidationError when choices are not distinct (case-insensitive)."""
        from enrichment.models import MCQQuestion
        with pytest.raises(ValidationError):
            MCQQuestion(**_make_question(choices=[
                "Manages pod replicas",
                "MANAGES POD REPLICAS",  # duplicate (case-insensitive)
                "Routes traffic",
                "Defines resource quotas",
            ]))

    def test_fewer_than_4_choices_rejected(self):
        """MCQQuestion raises ValidationError when len(choices) < 4."""
        from enrichment.models import MCQQuestion
        with pytest.raises(ValidationError):
            MCQQuestion(**_make_question(choices=["A", "B", "C"]))

    def test_more_than_4_choices_rejected(self):
        """MCQQuestion raises ValidationError when len(choices) > 4."""
        from enrichment.models import MCQQuestion
        with pytest.raises(ValidationError):
            MCQQuestion(**_make_question(choices=["A", "B", "C", "D", "E"]))

    def test_correct_index_out_of_range_high_rejected(self):
        """MCQQuestion raises ValidationError when correct_index is 4 (out of 0..3 range)."""
        from enrichment.models import MCQQuestion
        with pytest.raises(ValidationError):
            MCQQuestion(**_make_question(correct_index=4))

    def test_correct_index_negative_rejected(self):
        """MCQQuestion raises ValidationError when correct_index < 0."""
        from enrichment.models import MCQQuestion
        with pytest.raises(ValidationError):
            MCQQuestion(**_make_question(correct_index=-1))

    def test_correct_index_3_accepted(self):
        """MCQQuestion accepts correct_index=3 (max boundary)."""
        from enrichment.models import MCQQuestion
        q = MCQQuestion(**_make_question(correct_index=3))
        assert q.correct_index == 3

    def test_correct_index_0_accepted(self):
        """MCQQuestion accepts correct_index=0 (min boundary)."""
        from enrichment.models import MCQQuestion
        q = MCQQuestion(**_make_question(correct_index=0))
        assert q.correct_index == 0


# ---------------------------------------------------------------------------
# LessonOutput constraints
# ---------------------------------------------------------------------------

class TestLessonOutputConstraints:
    def test_has_heading_validator_rejects_no_heading(self):
        """LessonOutput.has_heading validator rejects markdown with no line starting with '#'."""
        from enrichment.models import LessonOutput
        no_heading_md = "This is a lesson about Kubernetes. " * 30  # enough chars, no heading
        with pytest.raises(ValidationError):
            LessonOutput(
                markdown=no_heading_md,
                word_count=len(no_heading_md.split()),
                prompt_version="lesson-v1",
            )

    def test_valid_lesson_with_heading_accepted(self):
        """LessonOutput accepts valid markdown with at least one heading."""
        from enrichment.models import LessonOutput
        valid_md = "## Introduction to Kubernetes\n\n" + "Kubernetes is a container orchestration platform. " * 20
        lesson = LessonOutput(
            markdown=valid_md,
            word_count=len(valid_md.split()),
            prompt_version="lesson-v1",
        )
        assert "##" in lesson.markdown


# ---------------------------------------------------------------------------
# quiz_output_to_payload adapter
# ---------------------------------------------------------------------------

class TestQuizOutputToPayload:
    def test_question_id_format(self):
        """quiz_output_to_payload produces question id = 'q-{module_slug}-{qi}' (1-based)."""
        from enrichment.models import quiz_output_to_payload
        quiz = _make_valid_quiz(5)
        payload = quiz_output_to_payload(quiz, "mod-3")
        assert payload["questions"][0]["id"] == "q-mod-3-1"
        assert payload["questions"][4]["id"] == "q-mod-3-5"

    def test_option_id_format(self):
        """quiz_output_to_payload produces option id = 'opt-{qi}-{oi}' (1-based)."""
        from enrichment.models import quiz_output_to_payload
        quiz = _make_valid_quiz(5)
        payload = quiz_output_to_payload(quiz, "mod-3")
        assert payload["questions"][0]["options"][0]["id"] == "opt-1-1"
        assert payload["questions"][0]["options"][3]["id"] == "opt-1-4"

    def test_correct_option_id_is_string(self):
        """quiz_output_to_payload correctOptionId is a string, not an integer index."""
        from enrichment.models import quiz_output_to_payload
        quiz = _make_valid_quiz(5)
        payload = quiz_output_to_payload(quiz, "mod-3")
        correct_id = payload["questions"][0]["correctOptionId"]
        assert isinstance(correct_id, str)
        # correct_index=0 → opt-1-1
        assert correct_id == "opt-1-1"

    def test_correct_option_id_adapter_bridge(self):
        """correctOptionId == f'opt-{qi}-{correct_index+1}' — D-adapter bridge (Pitfall 5)."""
        from enrichment.models import MCQQuestion, QuizOutput, quiz_output_to_payload
        # correct_index=2 on question 1 → correctOptionId should be "opt-1-3"
        q = MCQQuestion(
            question="Which kubectl command checks rollout status for a Kubernetes deployment?",
            choices=["kubectl get pods", "kubectl describe deployment", "kubectl rollout status", "kubectl apply -f"],
            correct_index=2,
            explanation="kubectl rollout status shows the deployment rollout progress in real time.",
        )
        quiz = QuizOutput(questions=[q, q, q, q, q])  # 5 identical Qs to satisfy min
        payload = quiz_output_to_payload(quiz, "my-module")
        # First question, correct_index=2 → oi = 3 → "opt-1-3"
        assert payload["questions"][0]["correctOptionId"] == "opt-1-3"

    def test_payload_shape_matches_sheet2pack_quiz_payload(self):
        """Full payload shape matches sheet2pack.quiz_payload output structure."""
        from enrichment.models import quiz_output_to_payload
        quiz = _make_valid_quiz(5)
        payload = quiz_output_to_payload(quiz, "mod-1")
        assert "questions" in payload
        q = payload["questions"][0]
        assert "id" in q
        assert "stem" in q
        assert "options" in q
        assert "correctOptionId" in q
        assert "explanation" in q
        assert isinstance(q["options"], list)
        assert len(q["options"]) == 4
        for opt in q["options"]:
            assert "id" in opt
            assert "text" in opt


# ---------------------------------------------------------------------------
# Standalone import check (no sheet2pack dependency)
# ---------------------------------------------------------------------------

class TestStandaloneImport:
    def test_models_import_without_sheet2pack(self):
        """models.py imports cleanly without any sheet2pack dependency."""
        import sys
        import importlib
        # Ensure sheet2pack is NOT imported by models
        # If models tries to import sheet2pack, it would fail since sheet2pack
        # is not a proper module (it's a script at scripts/sheet2pack.py)
        mod = importlib.import_module("enrichment.models")
        assert hasattr(mod, "QuizOutput")
        assert hasattr(mod, "MCQQuestion")
        assert hasattr(mod, "LessonOutput")
        assert hasattr(mod, "quiz_output_to_payload")
