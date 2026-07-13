---
slug: pod-create-and-inspect
title: Create and inspect a Pod
image: kindest/node:v1.30
requires_docker: true
creates:
  - manifests/pod.yaml
  - notes/run-output.txt
steps:
  - id: write-manifest
    title: Write a Pod manifest
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
  - id: apply-manifest
    title: Apply the manifest
    prompt: |
      Apply the manifest you just wrote so Kubernetes creates the Pod in the
      default namespace. Use `kubectl apply -f manifests/pod.yaml`. Successful
      output looks like `pod/web created`.
    check:
      kind: command_regex
      pattern: "pod/web (created|configured)"
    hints:
      - "Use kubectl apply with the -f flag to apply a manifest file."
      - "Run: kubectl apply -f manifests/pod.yaml"
      - "If kubectl errors, check that the kind cluster is up."
  - id: verify-running
    title: Verify the Pod is running
    prompt: |
      List Pods in the default namespace and confirm the `web` Pod's `STATUS`
      is `Running`. Run `kubectl get pods` and wait until STATUS=Running
      before moving on.
    check:
      kind: exit_code
      expected: 0
    hints:
      - "Use kubectl get pods to list pods in the default namespace."
      - "Run: kubectl get pods"
      - "Wait for STATUS=Running before continuing."
  - id: capture-output
    title: Capture the run output for review
    prompt: |
      Save the output of `kubectl get pods` into `notes/run-output.txt` so you
      can refer back to it later. The file must contain the word `Running`.
    check:
      kind: file_state
      path: notes/run-output.txt
      contains: "Running"
    hints:
      - "Pipe kubectl output into a file under notes/."
      - "Run: mkdir -p notes && kubectl get pods > notes/run-output.txt"
      - "Verify with cat notes/run-output.txt."
  - id: no-crash-loop
    title: Confirm the Pod never entered CrashLoopBackOff
    prompt: |
      Re-check `kubectl get pods` output one more time and confirm it never
      showed an `Error` or `CrashLoopBackOff` status.
    check:
      kind: command_absent
      pattern: "Error|CrashLoopBackOff"
    hints:
      - "Run kubectl get pods again and read the STATUS column."
      - "Error/CrashLoopBackOff means the container is failing to start."
      - "If it appears, check kubectl logs pod/web."
---

# Create and inspect a Pod

In this lab you will write a Pod manifest, apply it to your kind cluster, and
confirm the Pod is running. Each step has 3 progressive hints if you get stuck.
