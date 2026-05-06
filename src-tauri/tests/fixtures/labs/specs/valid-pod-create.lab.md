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
    check:
      kind: command_regex
      pattern: "pod/web (created|configured)"
    hints:
      - "Use kubectl apply with the -f flag to apply a manifest file."
      - "Run: kubectl apply -f manifests/pod.yaml"
      - "If kubectl errors, check that the kind cluster is up."
  - id: verify-running
    title: Verify the Pod is running
    check:
      kind: exit_code
      expected: 0
    hints:
      - "Use kubectl get pods to list pods in the default namespace."
      - "Run: kubectl get pods"
      - "Wait for STATUS=Running before continuing."
  - id: capture-output
    title: Capture the run output for review
    check:
      kind: file_state
      path: notes/run-output.txt
      contains: "Running"
    hints:
      - "Pipe kubectl output into a file under notes/."
      - "Run: mkdir -p notes && kubectl get pods > notes/run-output.txt"
      - "Verify with cat notes/run-output.txt."
---

# Create and inspect a Pod

In this lab you will write a Pod manifest, apply it to your kind cluster, and confirm
the Pod is running. Each step has 3 progressive hints if you get stuck.

## Step 1 — Write a Pod manifest

Create `manifests/pod.yaml` with a single-container Pod. Use `nginx:alpine` as the
image and name the Pod `web`.

## Step 2 — Apply the manifest

Apply the manifest with `kubectl apply -f manifests/pod.yaml`. The expected output
includes `pod/web created` (or `configured` on a re-apply).

## Step 3 — Verify the Pod is running

Run `kubectl get pods` and confirm the Pod's STATUS is `Running`. The exit code
must be 0.

## Step 4 — Capture the run output

Save the kubectl output to `notes/run-output.txt` so you have a record of what
the cluster looked like when the Pod came up.
