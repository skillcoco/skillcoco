---
slug: pod-create-and-inspect
title: Create and inspect a Pod
image: kindest/node:v1.30
requires_docker: true
exam:
  timeLimitMinutes: 45
  passThresholdPct: 80
creates:
  - manifests/pod.yaml
  - notes/run-output.txt
steps:
  - id: write-manifest
    title: Write a Pod manifest
    weight: 2
    prompt: |
      Create a `manifests/` directory and place a Pod manifest at
      `manifests/pod.yaml`. The Pod should be named `web`, run a single
      container using the `nginx:alpine` image, and expose port 80. The
      minimum Kubernetes Pod manifest requires `apiVersion`, `kind`,
      `metadata`, and `spec` — make sure all four are present.
    check:
      kind: file_state
      path: manifests/pod.yaml
      contains: "kind: Pod"
    hints:
      - "Create a directory called manifests/ and put pod.yaml there."
      - "The simplest Pod manifest has apiVersion, kind, metadata, and spec."
      - "Try: mkdir -p manifests && cat > manifests/pod.yaml <<EOF ... EOF"
  - id: explain-scheduling
    title: Explain how the Pod was scheduled
    prompt: |
      In your own words, explain what the Kubernetes scheduler considered
      when it placed this Pod on a node (resource requests, taints,
      affinity). Type your explanation as a shell comment or note file.
    check:
      kind: ai_judge
      criteria: "explanation correctly describes scheduler placement factors"
      threshold: 0.7
    hints:
      - "Think about resource requests/limits and node selection."
      - "The scheduler considers taints, tolerations, and affinity rules."
      - "Mention at least one concrete scheduling factor by name."
---

# Create and inspect a Pod (Exam)

In this exam you will write a Pod manifest and explain how it was scheduled.
You have 45 minutes and must score at least 80% to pass. Hints are disabled
during the exam attempt.
