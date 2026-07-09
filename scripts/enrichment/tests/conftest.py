"""
Shared pytest fixtures for the enrichment test suite.

Provides:
  - tmp_cache_dir        — isolated cache dir per test (monkeypatches content_cache.CACHE_DIR)
  - mock_anthropic_lesson_response  — simulated successful lesson API response
  - mock_anthropic_truncated        — simulated max_tokens truncated response
  - mock_anthropic_quiz_tool_response — simulated successful quiz tool_use response
  - fixture_transcript   — short realistic transcript string
  - fixture_vtt_with_overlaps — VTT content with overlapping timestamp windows
"""
import pytest
from unittest.mock import AsyncMock, MagicMock
from pathlib import Path


@pytest.fixture
def tmp_cache_dir(tmp_path, monkeypatch):
    """Isolated cache directory per test — avoids touching ~/.learnforge.

    Monkeypatches content_cache.CACHE_DIR so no test writes to the real
    user home directory.
    """
    cache = tmp_path / "transcripts"
    cache.mkdir(parents=True, exist_ok=True)
    import enrichment.content_cache as cc
    monkeypatch.setattr(cc, "CACHE_DIR", cache)
    return cache


@pytest.fixture
def mock_anthropic_lesson_response():
    """Simulated successful lesson generation response (stop_reason=end_turn)."""
    response = MagicMock()
    response.stop_reason = "end_turn"
    response.content = [
        MagicMock(text=(
            "## Kubernetes Deployments\n\n"
            "A deployment manages a set of pod replicas. "
            "As the instructor explained, think of it like a supervisor that "
            "ensures your workers are always at the right headcount.\n\n"
            "### Rolling Updates\n\n"
            "Kubernetes rolls out changes incrementally, replacing old pods with new ones."
        ))
    ]
    return response


@pytest.fixture
def mock_anthropic_truncated():
    """Simulated truncated response (stop_reason=max_tokens)."""
    response = MagicMock()
    response.stop_reason = "max_tokens"
    response.content = [MagicMock(text="## Kubernetes Deploy")]
    return response


@pytest.fixture
def mock_anthropic_quiz_tool_response():
    """Simulated successful quiz generation response with tool_use block."""
    tool_input = {
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
                "explanation": "A Deployment manages a set of pod replicas to ensure availability.",
            },
            {
                "question": "Which command checks the rollout status of a Kubernetes deployment?",
                "choices": [
                    "kubectl get pods",
                    "kubectl rollout status",
                    "kubectl describe node",
                    "kubectl apply -f",
                ],
                "correct_index": 1,
                "explanation": "kubectl rollout status shows deployment rollout progress.",
            },
            {
                "question": "What strategy does Kubernetes use by default for rolling updates?",
                "choices": [
                    "Recreate",
                    "Blue-Green",
                    "RollingUpdate",
                    "Canary",
                ],
                "correct_index": 2,
                "explanation": "RollingUpdate is the default strategy, replacing pods incrementally.",
            },
            {
                "question": "Which kubectl flag pauses a Kubernetes Deployment rollout?",
                "choices": [
                    "--pause",
                    "--stop",
                    "--halt",
                    "--freeze",
                ],
                "correct_index": 0,
                "explanation": "kubectl rollout pause <deployment> uses the --pause concept.",
            },
            {
                "question": "What is the purpose of a ReplicaSet in Kubernetes?",
                "choices": [
                    "Routes external traffic to pods",
                    "Maintains a stable set of replica pods",
                    "Provides persistent volume claims",
                    "Defines resource limits for containers",
                ],
                "correct_index": 1,
                "explanation": "A ReplicaSet ensures the specified number of pod replicas are running.",
            },
        ]
    }
    response = MagicMock()
    response.stop_reason = "end_turn"
    tool_block = MagicMock()
    tool_block.type = "tool_use"
    tool_block.name = "emit_quiz"
    tool_block.input = tool_input
    response.content = [tool_block]
    return response


@pytest.fixture
def fixture_transcript():
    """Short realistic transcript for testing lesson generation."""
    return (
        "So today we're going to look at Kubernetes deployments. "
        "A deployment is basically a supervisor for your pods. "
        "Think of it like a manager that ensures your workers — the pods — "
        "are always at the right headcount. "
        "If a pod goes down, the deployment controller brings a new one up. "
        "We can do rolling updates with kubectl rollout status to check progress."
    )


@pytest.fixture
def fixture_vtt_with_overlaps():
    """VTT content with overlapping timestamp windows for dedup testing."""
    return (
        "WEBVTT\n\n"
        "00:00:00.000 --> 00:00:03.000\n"
        "now let us deploy this\n\n"
        "00:00:01.500 --> 00:00:04.500\n"
        "let us deploy this to Kubernetes\n"
    )
