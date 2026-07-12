---
slug: "m1-container-native-lab"
title: "Container-Native GenAI"
image: "alpine:3.20"
requires_docker: true
creates: []
steps:
  - id: "docker-available"
    title: "docker CLI is on PATH and server responds"
    prompt: "**Milestone:** docker CLI is on PATH and server responds\n\nWork toward this milestone, then verify it with:\n\n```bash\ndocker version\n```"
    check:
      kind: "exit_code"
      expected: 0
    hints: ["`Could not resolve host: host.docker.internal`", "`Connection refused` on port 11434", "`model \"qwen2.5:1.5b\" not found`", "Container pull is slow (first run)"]
  - id: "ollama-api-reachable"
    title: "Ollama native API responds on :11434 with model list"
    prompt: "**Milestone:** Ollama native API responds on :11434 with model list\n\nWork toward this milestone, then verify it with:\n\n```bash\ncurl -s http://localhost:11434/api/tags\n```"
    check:
      kind: "command_regex"
      pattern: "models"
      match_stderr: false
    hints: ["`Could not resolve host: host.docker.internal`", "`Connection refused` on port 11434", "`model \"qwen2.5:1.5b\" not found`", "Container pull is slow (first run)"]
  - id: "container-reaches-host-ollama"
    title: "container can reach Ollama via host.docker.internal:11434/api/tags"
    prompt: "**Milestone:** container can reach Ollama via host.docker.internal:11434/api/tags\n\nWork toward this milestone, then verify it with:\n\n```bash\ndocker run --rm curlimages/curl:latest -s http://host.docker.internal:11434/api/tags\n```"
    check:
      kind: "command_regex"
      pattern: "models"
      match_stderr: false
    hints: ["`Could not resolve host: host.docker.internal`", "`Connection refused` on port 11434", "`model \"qwen2.5:1.5b\" not found`", "Container pull is slow (first run)"]
  - id: "generate-returns-completion"
    title: "container POSTs to /api/generate and gets a done:true completion"
    prompt: "**Milestone:** container POSTs to /api/generate and gets a done:true completion\n\nWork toward this milestone, then verify it with:\n\n```bash\ndocker run --rm curlimages/curl:latest -s http://host.docker.internal:11434/api/generate -d '{\"model\":\"qwen2.5:1.5b\",\"prompt\":\"Say hi in 5 words.\",\"stream\":false}'\n```"
    check:
      kind: "command_regex"
      pattern: "\"done\":true"
      match_stderr: false
    hints: ["`Could not resolve host: host.docker.internal`", "`Connection refused` on port 11434", "`model \"qwen2.5:1.5b\" not found`", "Container pull is slow (first run)"]
  - id: "generate-has-response-field"
    title: "completion JSON contains a non-empty response field"
    prompt: "**Milestone:** completion JSON contains a non-empty response field\n\nWork toward this milestone, then verify it with:\n\n```bash\ndocker run --rm curlimages/curl:latest -s http://host.docker.internal:11434/api/generate -d '{\"model\":\"qwen2.5:1.5b\",\"prompt\":\"Say hi.\",\"stream\":false}'\n```"
    check:
      kind: "command_regex"
      pattern: "\"response\":\"[^\"]+\""
      match_stderr: false
    hints: ["`Could not resolve host: host.docker.internal`", "`Connection refused` on port 11434", "`model \"qwen2.5:1.5b\" not found`", "Container pull is slow (first run)"]
---

# Container-Native GenAI

Run a throwaway curlimages/curl container that POSTs to the natively-served
Ollama endpoint via host.docker.internal:11434/api/generate and receives a
real AI response — proving the foundational wiring every later module
relies on.
