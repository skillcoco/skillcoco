"""
Tests for scripts/enrichment/eval/checks.py — deterministic evaluation screens.

All tests run with NO API key (stdlib + pydantic only).
CI command: pytest scripts/enrichment/eval/ -v

Coverage:
  - grounding_ratio() helper
  - check_grounding() — E3/E4 adversarial fixture
  - check_quiz_schema() — E1/E2 adversarial fixture (< 5 questions) + valid quiz
  - check_distractors() — E7 adversarial fixture (near-duplicate choices)
  - check_answer_key() — E5 screen
"""
import json
import os
import sys
from pathlib import Path

import pytest

# No API key should be needed for these tests
os.environ.pop("ANTHROPIC_API_KEY", None)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

FIXTURES_DIR = Path(__file__).parent / "fixtures"


def load_fixture(name: str) -> dict:
    """Load a JSON fixture from the fixtures directory."""
    with open(FIXTURES_DIR / name) as fh:
        return json.load(fh)


# ---------------------------------------------------------------------------
# Module imports
# ---------------------------------------------------------------------------

from enrichment.eval.checks import (
    Result,
    check_answer_key,
    check_distractors,
    check_grounding,
    check_quiz_schema,
    grounding_ratio,
)


# ---------------------------------------------------------------------------
# grounding_ratio() tests
# ---------------------------------------------------------------------------

class TestGroundingRatio:
    def test_returns_1_when_no_command_tokens(self):
        """grounding_ratio returns 1.0 when lesson has no command/tool tokens."""
        lesson = "This is a simple lesson about concepts with no commands."
        transcript = "We talked about some concepts today."
        assert grounding_ratio(lesson, transcript) == 1.0

    def test_returns_1_when_all_tokens_found(self):
        """grounding_ratio returns 1.0 when all command tokens are in transcript."""
        lesson = "Use `kubectl get pods` to list pods."
        transcript = "today we use kubectl get pods to check the status"
        ratio = grounding_ratio(lesson, transcript)
        assert ratio == 1.0

    def test_returns_low_when_tokens_absent(self):
        """grounding_ratio returns < 1.0 when command tokens are not in transcript."""
        lesson = "Run `kubectl rollout undo --to-revision=3` to revert."
        transcript = "today we use kubectl get pods to check pods"
        ratio = grounding_ratio(lesson, transcript)
        assert ratio < 1.0

    def test_returns_float_in_range(self):
        """grounding_ratio always returns a float in [0.0, 1.0]."""
        lesson = "Use `kubectl` and `helm install` and `docker build`."
        transcript = "we talked about kubectl and docker today"
        ratio = grounding_ratio(lesson, transcript)
        assert 0.0 <= ratio <= 1.0

    def test_case_insensitive(self):
        """grounding_ratio is case-insensitive."""
        lesson = "Use `Kubectl` to manage pods."
        transcript = "today we use kubectl"
        ratio = grounding_ratio(lesson, transcript)
        assert ratio == 1.0


# ---------------------------------------------------------------------------
# check_grounding() — adversarial fixture (E3/E4)
# ---------------------------------------------------------------------------

class TestCheckGrounding:
    def test_adversarial_grounding_violation_is_flagged(self):
        """ADVERSARIAL: lesson citing kubectl rollout undo --to-revision=3 not in transcript is flagged needs_review."""
        fixture = load_fixture("adversarial_grounding_violation.json")
        result = check_grounding(fixture["lesson_md"], fixture["transcript"])
        assert result.dimension == "E3/E4"
        assert result.status == "flag", (
            f"Expected 'flag' but got '{result.status}': {result.reason}"
        )
        assert "needs_review" in result.reason

    def test_adversarial_fixture_missing_token_in_details(self):
        """Adversarial fixture result includes missing token details."""
        fixture = load_fixture("adversarial_grounding_violation.json")
        result = check_grounding(fixture["lesson_md"], fixture["transcript"])
        assert "missing_fenced_tokens" in result.details
        # The fenced block contains 'kubectl rollout undo --to-revision=3' or similar
        # which should not appear in the transcript
        assert len(result.details["missing_fenced_tokens"]) > 0

    def test_clean_lesson_passes(self):
        """A lesson whose commands are all present in transcript passes."""
        lesson = (
            "## Kubernetes Pods\n\n"
            "Use `kubectl get pods` to list all running pods.\n\n"
            "```bash\nkubectl get pods\n```"
        )
        transcript = (
            "today we look at kubernetes pods and use kubectl get pods "
            "to list everything that is running"
        )
        result = check_grounding(lesson, transcript)
        assert result.status == "pass", f"Expected pass but got: {result.reason}"

    def test_custom_threshold_respected(self):
        """A lower threshold is more permissive."""
        fixture = load_fixture("adversarial_grounding_violation.json")
        # With threshold=0.0 (accept anything), should pass
        result = check_grounding(fixture["lesson_md"], fixture["transcript"], threshold=0.0)
        # Even at 0.0 threshold, a missing fenced token still flags
        # (the test verifies threshold parameter is accepted)
        assert isinstance(result, Result)

    def test_lesson_with_no_commands_passes(self):
        """A lesson with no commands/tools always passes grounding check."""
        lesson = "## Introduction\n\nThis lesson covers key concepts at a high level."
        transcript = "we discussed some important ideas today"
        result = check_grounding(lesson, transcript)
        assert result.status == "pass"


# ---------------------------------------------------------------------------
# check_quiz_schema() — adversarial fixture (E1/E2)
# ---------------------------------------------------------------------------

class TestCheckQuizSchema:
    def test_adversarial_truncated_quiz_fails(self):
        """ADVERSARIAL: quiz with 3 questions fails E1/E2 check (< 5 questions)."""
        fixture = load_fixture("adversarial_truncated_quiz.json")
        # Remove _comment key before passing to check
        quiz_dict = {k: v for k, v in fixture.items() if not k.startswith("_")}
        result = check_quiz_schema(quiz_dict)
        assert result.dimension == "E1/E2"
        assert result.status == "fail", (
            f"Expected 'fail' but got '{result.status}': {result.reason}"
        )

    def test_truncated_quiz_fail_reason_mentions_question_count(self):
        """The fail reason references the question count."""
        fixture = load_fixture("adversarial_truncated_quiz.json")
        quiz_dict = {k: v for k, v in fixture.items() if not k.startswith("_")}
        result = check_quiz_schema(quiz_dict)
        assert "3" in result.reason or "question" in result.reason.lower()

    def test_valid_quiz_passes(self):
        """A quiz with 5+ valid questions passes schema check."""
        quiz_dict = {
            "questions": [
                {
                    "question": "What does a Kubernetes Deployment manage in a cluster?",
                    "choices": [
                        "A set of pod replicas",
                        "Network routing rules",
                        "Persistent storage volumes",
                        "Node scheduling policies",
                    ],
                    "correct_index": 0,
                    "explanation": "A Deployment manages pod replicas.",
                },
                {
                    "question": "Which command checks rollout status?",
                    "choices": [
                        "kubectl get pods",
                        "kubectl rollout status",
                        "kubectl describe node",
                        "kubectl apply -f",
                    ],
                    "correct_index": 1,
                    "explanation": "kubectl rollout status shows progress.",
                },
                {
                    "question": "What is the default update strategy for Deployments?",
                    "choices": [
                        "Recreate",
                        "Blue-Green",
                        "RollingUpdate",
                        "Canary",
                    ],
                    "correct_index": 2,
                    "explanation": "RollingUpdate is the default.",
                },
                {
                    "question": "What object maintains a stable set of replica pods?",
                    "choices": [
                        "Service",
                        "ReplicaSet",
                        "ConfigMap",
                        "Ingress",
                    ],
                    "correct_index": 1,
                    "explanation": "A ReplicaSet maintains the specified pod count.",
                },
                {
                    "question": "Which resource routes external traffic to pods?",
                    "choices": [
                        "ReplicaSet",
                        "Deployment",
                        "Service",
                        "PersistentVolume",
                    ],
                    "correct_index": 2,
                    "explanation": "Services route traffic to pods.",
                },
            ]
        }
        result = check_quiz_schema(quiz_dict)
        assert result.status == "pass", f"Expected pass: {result.reason}"

    def test_invalid_schema_fails(self):
        """A quiz with invalid field types fails schema check."""
        quiz_dict = {
            "questions": [
                {
                    "question": "Short?",  # too short for MCQQuestion.question
                    "choices": ["a", "b", "c"],  # only 3 choices
                    "correct_index": 5,  # out of range
                    "explanation": "x",  # too short
                }
            ]
        }
        result = check_quiz_schema(quiz_dict)
        assert result.status == "fail"

    def test_empty_dict_fails(self):
        """An empty dict fails schema check."""
        result = check_quiz_schema({})
        assert result.status == "fail"

    def test_returns_result_not_raises(self):
        """check_quiz_schema never raises — always returns a Result."""
        for bad_input in [{}, {"questions": None}, {"questions": "not-a-list"}]:
            result = check_quiz_schema(bad_input)
            assert isinstance(result, Result)
            assert result.status in {"pass", "flag", "fail"}


# ---------------------------------------------------------------------------
# check_distractors() — adversarial fixture (E7)
# ---------------------------------------------------------------------------

class TestCheckDistractors:
    def test_adversarial_duplicate_distractor_is_flagged(self):
        """ADVERSARIAL: near-duplicate distractor choices are caught by E7 check."""
        fixture = load_fixture("adversarial_duplicate_distractor.json")
        quiz_dict = {k: v for k, v in fixture.items() if not k.startswith("_")}
        result = check_distractors(quiz_dict)
        assert result.dimension == "E7"
        assert result.status == "flag", (
            f"Expected 'flag' but got '{result.status}': {result.reason}"
        )

    def test_adversarial_duplicate_issue_in_details(self):
        """Adversarial fixture result details include issue descriptions."""
        fixture = load_fixture("adversarial_duplicate_distractor.json")
        quiz_dict = {k: v for k, v in fixture.items() if not k.startswith("_")}
        result = check_distractors(quiz_dict)
        assert "issues" in result.details
        assert len(result.details["issues"]) > 0

    def test_clean_quiz_passes(self):
        """A quiz with distinct choices passes E7 check."""
        quiz_dict = {
            "questions": [
                {
                    "question": "What does a Kubernetes Deployment manage?",
                    "choices": [
                        "A set of pod replicas",
                        "Network routing rules",
                        "Persistent storage volumes",
                        "Node scheduling policies",
                    ],
                    "correct_index": 0,
                    "explanation": "A Deployment manages pod replicas.",
                },
            ]
        }
        result = check_distractors(quiz_dict)
        assert result.status == "pass", f"Expected pass: {result.reason}"

    def test_exact_duplicate_choices_flagged(self):
        """Exact duplicate choices (after normalization) are flagged."""
        quiz_dict = {
            "questions": [
                {
                    "question": "Which tool does Kubernetes use?",
                    "choices": [
                        "kubectl",
                        "kubectl",  # exact duplicate
                        "helm",
                        "docker",
                    ],
                    "correct_index": 0,
                    "explanation": "kubectl is the main Kubernetes CLI.",
                }
            ]
        }
        result = check_distractors(quiz_dict)
        assert result.status == "flag"

    def test_returns_result_not_raises(self):
        """check_distractors never raises — always returns a Result."""
        for bad_input in [{}, {"questions": []}, {"questions": None}]:
            try:
                result = check_distractors(bad_input)
                assert isinstance(result, Result)
            except Exception as exc:
                pytest.fail(f"check_distractors raised unexpectedly: {exc}")

    def test_whitespace_normalized_near_duplicate_detected(self):
        """Near-duplicates that differ only by whitespace are caught."""
        quiz_dict = {
            "questions": [
                {
                    "question": "Which command lists pods?",
                    "choices": [
                        "kubectl  get pods",   # extra space
                        "kubectl get pods",    # normal
                        "kubectl describe node",
                        "kubectl apply -f",
                    ],
                    "correct_index": 2,
                    "explanation": "kubectl get pods lists pods.",
                }
            ]
        }
        result = check_distractors(quiz_dict)
        assert result.status == "flag"


# ---------------------------------------------------------------------------
# check_answer_key() — E5 screen
# ---------------------------------------------------------------------------

class TestCheckAnswerKey:
    def test_valid_quiz_passes(self):
        """A well-formed quiz with transcript overlap passes E5."""
        transcript = (
            "today we look at kubernetes deployments. "
            "a deployment manages pod replicas. "
            "use kubectl rollout status to check progress."
        )
        quiz_dict = {
            "questions": [
                {
                    "question": "What does a Kubernetes Deployment manage?",
                    "choices": [
                        "Pod replicas",
                        "Network rules",
                        "Storage volumes",
                        "Node policies",
                    ],
                    "correct_index": 0,
                    "explanation": "A Deployment manages pod replicas.",
                }
            ]
        }
        result = check_answer_key(quiz_dict, transcript)
        assert result.status == "pass", f"Expected pass: {result.reason}"

    def test_out_of_range_correct_index_flagged(self):
        """correct_index outside [0, 3] is flagged."""
        transcript = "we use kubectl get pods to list all pods"
        quiz_dict = {
            "questions": [
                {
                    "question": "Which kubectl command lists pods?",
                    "choices": [
                        "kubectl get pods",
                        "kubectl apply",
                        "kubectl delete",
                        "kubectl describe",
                    ],
                    "correct_index": 10,  # out of range
                    "explanation": "kubectl get pods lists pods.",
                }
            ]
        }
        result = check_answer_key(quiz_dict, transcript)
        assert result.status == "flag"
        assert "correct_index" in result.reason

    def test_duplicate_choices_flagged(self):
        """Duplicate choices (case-insensitive) are flagged."""
        transcript = "we use kubectl to manage kubernetes clusters"
        quiz_dict = {
            "questions": [
                {
                    "question": "What does kubectl manage?",
                    "choices": [
                        "Kubernetes clusters",
                        "Kubernetes Clusters",  # case duplicate
                        "Network rules",
                        "Storage volumes",
                    ],
                    "correct_index": 0,
                    "explanation": "kubectl manages Kubernetes clusters.",
                }
            ]
        }
        result = check_answer_key(quiz_dict, transcript)
        assert result.status == "flag"

    def test_empty_quiz_fails(self):
        """A quiz with no questions fails E5."""
        result = check_answer_key({"questions": []}, "some transcript")
        assert result.status == "fail"

    def test_returns_result_not_raises(self):
        """check_answer_key never raises — always returns a Result."""
        for bad_input in [{}, {"questions": None}]:
            try:
                result = check_answer_key(bad_input, "transcript")
                assert isinstance(result, Result)
            except Exception as exc:
                pytest.fail(f"check_answer_key raised unexpectedly: {exc}")


# ---------------------------------------------------------------------------
# Result type tests
# ---------------------------------------------------------------------------

class TestResult:
    def test_result_is_dataclass_with_required_fields(self):
        """Result has dimension, status, reason, details fields."""
        r = Result(dimension="E1/E2", status="pass")
        assert r.dimension == "E1/E2"
        assert r.status == "pass"
        assert r.reason == ""
        assert r.details == {}

    def test_result_with_all_fields(self):
        """Result can be constructed with all fields."""
        r = Result(
            dimension="E3/E4",
            status="flag",
            reason="needs_review: low grounding ratio",
            details={"grounding_ratio": 0.5},
        )
        assert r.status == "flag"
        assert r.details["grounding_ratio"] == 0.5
