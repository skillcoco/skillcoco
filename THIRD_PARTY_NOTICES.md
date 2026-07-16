# Third-Party Notices

SkillCoco is licensed under the MIT License. This file lists third-party
software whose code, prompts, or design patterns have influenced or been
incorporated into SkillCoco, along with the required attribution.

For each entry below:

- **Pattern only** — we reimplemented the idea in Rust. No code copied. Listed
  here as good-faith credit, not a legal requirement.
- **Adapted** — short text (e.g. YAML prompts) adapted from upstream, with the
  upstream license preserved as a header comment in the affected file.
- **Copied / substantially adapted** — non-trivial code copied or derived. The
  upstream license header is included in the file and the upstream LICENSE
  text is stored under `licenses/`.

---

## DeepTutor

- **Source:** https://github.com/HKUDS/DeepTutor
- **Copyright:** © HKUDS and DeepTutor contributors
- **License:** Apache License 2.0 — see `licenses/APACHE-2.0-DeepTutor.txt`
- **Status:** Pattern only (planned). Future entries will be added with file
  paths as code lands in Phase 2 and Phase 3.

Patterns adopted (planned, not yet implemented):

| Pattern | Status |
|---|---|
| Two-file Memory (`PROFILE.md` + `SUMMARY.md`, LLM-rewritten with `NO_CHANGE` sentinel) | Pattern only |
| YAML `PromptManager` (singleton, cache, language fallback) | Pattern only |
| Draft → Critique → Revise spine generation | Pattern only |
| Concept Graph (typed nodes/edges, cycle removal, coverage padding) | Pattern only |
| Block taxonomy (text, callout, quiz, flash_cards) | Pattern only |
| User Skills (`SKILL.md` injected into system prompt) | Pattern only |
| `QuizViewer.tsx` React component | Adapted (planned) |

When the QuizViewer component or any DeepTutor YAML prompt is committed, the
status above will be updated and a file-path entry added with the form:

```
| File | Upstream | Status |
|---|---|---|
| src/components/QuizViewer.tsx | web/components/quiz/QuizViewer.tsx | Adapted |
| src-tauri/prompts/spine_synthesizer.yaml | deeptutor/book/prompts/en/spine_synthesizer.yaml | Adapted |
```

Phase 03.1 (Hands-on Labs) inspiration:

- **LAB.md spec format** (Markdown body + YAML frontmatter) — `Pattern only`.
  The `gray_matter` crate is the parser; no DeepTutor parser code copied.
  File paths: `src-tauri/src/labs/spec.rs`,
  `src-tauri/tests/fixtures/labs/specs/*.lab.md`.

---

## portable-pty

- **Source:** https://github.com/wez/wezterm/tree/main/pty
- **Crate:** https://crates.io/crates/portable-pty
- **Version:** 0.9.x
- **License:** MIT — see crate `LICENSE` (no copy redistributed; crate ships
  the license alongside its source on crates.io)
- **Status:** Used as-is from crates.io
- **Purpose:** Cross-platform PTY backend (macOS, Linux, Windows ConPTY) for
  the embedded lab terminal. Used by WezTerm; `Pattern only` is N/A — this
  is a direct dependency, not a code-derivation.

---

## bollard

- **Source:** https://github.com/fussybeaver/bollard
- **Crate:** https://crates.io/crates/bollard
- **Version:** 0.19.x
- **License:** Apache License 2.0 — preserved alongside the crate source on
  crates.io
- **Status:** Used as-is from crates.io
- **Purpose:** Docker daemon API client for the sandbox-isolated lab
  runtime. Powers `DockerProbe`, container lifecycle, exec attach, and
  bind-mount workspace setup.

---

## gray_matter

- **Source:** https://github.com/the-alchemists-of-arland/gray-matter-rs
- **Crate:** https://crates.io/crates/gray_matter
- **Version:** 0.2.x
- **License:** MIT — preserved alongside the crate source on crates.io
- **Status:** Used as-is from crates.io
- **Purpose:** YAML frontmatter parser for the `LAB.md` spec format
  (Markdown body + YAML header).

---

## serde_yaml

- **Source:** https://github.com/dtolnay/serde-yaml
- **Crate:** https://crates.io/crates/serde_yaml
- **Version:** 0.9.x
- **License:** MIT OR Apache-2.0
- **Status:** Used as-is from crates.io
- **Purpose:** YAML deserialization helpers for the `LAB.md` frontmatter
  schema and topic-pack manifest files.

---

## Zeroclaw

- **Source:** Local path dependency (`/Users/gshah/work/agentix/upstream/zeroclaw`)
- **License:** TBD (verify before publishing)
- **Status:** To be removed in Phase 1 per FIX-05 (replaced with ~150 lines of
  direct reqwest calls). This entry will be deleted once removal lands.

---

## RuVector

- **Source:** Local path dependency (`/Users/gshah/work/agentix/upstream/ruvector`)
- **License:** TBD (verify before publishing)
- **Status:** Embedded for vector + graph intelligence. Will be published to
  crates.io in Phase 8.

---

## Maintenance

When you add or modify code derived from a third-party source:

1. Add a file-header comment naming the upstream project, license, and URL.
2. Add or update the entry in this file.
3. If the upstream license text isn't already present, add it under
   `licenses/<UPSTREAM>-<LICENSE>.txt`.
4. Prefer reimplementation over copying — it keeps SkillCoco cleanly MIT.
