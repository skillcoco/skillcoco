---
name: enrich-course
description: Enrich a course xlsx with AI lessons/quizzes using the Claude subscription (no ANTHROPIC_API_KEY). Claude Code generates content in-session and primes the enrichment cache, then sheet2pack.py --enrich assembles at 0 API calls. Use when the user wants to enrich a course, run the enrichment pipeline, or generate lessons/quizzes for a course pack.
---

# Enrich Course (subscription-billed, cache-prime)

Founder decision (2026-07-09): enrichment never bills through `ANTHROPIC_API_KEY`.
Claude Code IS the LLM engine. The pipeline's generators are cache-first (D-14), so
pre-populating the content cache makes `--enrich` a pure assembly run with 0 API calls.

**Cache layout** (`~/.learnforge/transcripts/`, override with `LEARNFORGE_TRANSCRIPTS_DIR`):
- Transcript: `{video_id}.txt` (raw text)
- Lesson: `{video_id}_{sha256(transcript)[:8]}_lesson-v1.json` → `{"markdown": "..."}`
- Quiz: `quiz_{module_slug}_{sha256("".join(transcripts))[:8]}_quiz-v1.json` → quiz payload dict

Always compute paths with the helpers in `scripts/enrichment/content_cache.py`
(`content_cache_path`, `quiz_cache_path`, `write_content_cache`) — never hand-build
filenames. Prompt versions come from `lesson_generator.LESSON_PROMPT_VERSION` and
`quiz_generator.QUIZ_PROMPT_VERSION`.

## Steps

### 1. Fetch transcripts (free, no LLM)

```bash
uv run scripts/sheet2pack.py "<course>.xlsx" -o out/<course>-enriched.json --enrich-only transcripts
```

Caches transcripts by video_id, prints the D-03 skip list (caption-less lessons).

### 2. Enumerate what needs generation

Inline Python (uv run, from `scripts/` so `enrichment` imports resolve): parse the
workbook the same way `sheet2pack.py` does, then for each lesson with a cached
transcript compute `content_cache_path(video_id, transcript, LESSON_PROMPT_VERSION)`
and report cached/uncached; for each module NOT in `compute_matched_chapters()`
(hand-authored quizzes are never touched — D-09) compute `quiz_cache_path(...)` and
report cached/uncached.

### 3. Generate lessons (Claude, in-session) — COHERENT, not isolated

**Coherence requirement (founder, 2026-07-09):** the course must read as one
well-woven story — lessons coherent within their module, modules coherent across
the course. NEVER generate a lesson in isolation.

Process:
1. Extract the full course outline first (module titles + ordered lesson titles
   via `sheet2pack.parse_tracker`) and write a 1-line-per-module story arc.
2. Batch generation BY MODULE — one agent per module, spawned in parallel. Each
   agent gets: the full course story arc, its module's position in the arc, and
   its module's ordered lesson list.
3. Each agent reads ALL its module's transcripts first, THEN writes lessons in
   order — consistent terminology, tone, and heading style across the module.
4. Connective framing allowed: at most one short sentence at a lesson's start/end
   situating it in the module/course, using ONLY outline titles. Module-intro
   lessons may preview the module; summary lessons may recap it. Never invent
   specifics about other lessons' content.
5. Grounding unchanged: ALL technical content from THAT lesson's transcript.

Content rules come from `LESSON_SYSTEM_PROMPT` in
`scripts/enrichment/lesson_generator.py` (read at run time — do not trust a stale
copy): RESTRUCTURE not rewrite; preserve instructor voice/analogies; nothing not
in the transcript; no padding — short transcript = short lesson (D-06); no
top-level `#` heading; markdown only, no preamble.

Validate + write through the real models and helpers (LessonOutput requires
word_count and prompt_version):

```bash
cd scripts && uv run --no-project --with pydantic - <<'EOF'
from enrichment.models import LessonOutput
from enrichment.content_cache import content_cache_path, write_content_cache, read_transcript_cache
from enrichment.lesson_generator import LESSON_PROMPT_VERSION
video_id = "..."
markdown = open(f"/tmp/lesson-{video_id}.md").read()
LessonOutput(markdown=markdown, word_count=len(markdown.split()), prompt_version=LESSON_PROMPT_VERSION)
t = read_transcript_cache(video_id)
write_content_cache(content_cache_path(video_id, t, LESSON_PROMPT_VERSION), {"markdown": markdown})
EOF
```

### 4. Generate quizzes (Claude, in-session)

For each gap module, ground ONLY in that module's cached transcripts (all of them —
D-12). Follow `QUIZ_SYSTEM_PROMPT` in `scripts/enrichment/quiz_generator.py` (read at
run time). Question count = `max(5, min(10, lesson_count))` (D-10); ~half recall /
half applied (D-11); 4 distinct choices; plausible distractors; 0-based
`correct_index`.

Validate via `QuizOutput` (5–10 questions, distinct-choice validator), convert with
`models.quiz_output_to_payload(qo, module_slug)`, write the RESULTING PAYLOAD DICT
(not the raw QuizOutput) to `quiz_cache_path(module_slug, module_transcripts, QUIZ_PROMPT_VERSION)`
via `write_content_cache`.

### 5. Verify 100% cache coverage, then assemble

Re-run step 2's enumeration — every artifact must be `cached`. Then:

```bash
ANTHROPIC_API_KEY=cache-only-dummy uv run scripts/sheet2pack.py "<course>.xlsx" -o out/<course>-enriched.json --enrich --licensed
```

**`--licensed` is REQUIRED for paid SODA packs** (founder, 2026-07-09): stamps
`exportedFrom: licensed:{pack_id}` so the imported course is non-exportable in the
app (fail-closed `is_course_exportable` denylist). Omit only for free/community
packs meant to be re-exportable.

The dummy key satisfies the eager client constructors; cache hits mean no API call
ever fires. If the run reports any `failed` artifact, a cache miss slipped through
(auth error, caught by D-16) — fix that artifact's cache entry and re-run. Expect:
generated/skipped/failed report, `validation: PASS` (ENR-04), ~$0.

### 6. Eval + import

- Tier-1 deterministic checks (offline): run `enrichment/eval/checks.py` screens
  against the generated artifacts (`check_grounding`, `check_quiz_schema`,
  `check_answer_key`, `check_distractors`) and write the run report via
  `enrichment/eval/report.py`.
- LLM-judge tier: skip (it also needs API billing) — advisory anyway until founder
  calibration ≥ 0.7 (AI-SPEC §5). Claude Code session review substitutes.
- Import: LearnForge app → Settings → Import → select the enriched JSON.
- Founder reviews generated lessons/quizzes for fidelity, command accuracy, voice.
