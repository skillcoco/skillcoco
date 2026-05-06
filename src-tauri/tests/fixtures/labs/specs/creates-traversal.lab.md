---
slug: bad-traversal
title: creates contains absolute and traversal paths
image: alpine:3.19
requires_docker: false
creates:
  - /etc/passwd
  - ../../etc/shadow
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

# Path Traversal Guard

The reset command iterates the `creates` list and removes each path under the
workspace root. Any absolute path or `..` segment must be rejected at parse time
so the reset cannot escape the workspace and harm the host.
