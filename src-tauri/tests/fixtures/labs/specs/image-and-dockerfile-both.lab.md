---
slug: bad-both
title: Both image and dockerfile set — XOR violation
image: alpine:3.19
dockerfile: |
  FROM alpine:3.19
  RUN apk add --no-cache curl
requires_docker: true
creates: []
steps:
  - id: noop
    title: Will never reach here
    check:
      kind: exit_code
      expected: 0
    hints:
      - "noop"
      - "noop"
      - "noop"
---

# Image XOR Dockerfile

A LAB.md must declare exactly one of `image` or `dockerfile`. Setting both is
ambiguous (which source wins?) and must fail validation.
