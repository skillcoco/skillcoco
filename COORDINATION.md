# Coordination: LearnForge ↔ Creator Studio

LearnForge (this repo — open-source learner app, MIT) has a private
companion product, **LearnForge Creator Studio** (educator
course-production workstation, closed-source, separate repo). The two
are independent products with their own development, licensing, and
maintenance since 2026-07-10.

## What moved out of this repo (2026-07-10)

- `scripts/` — the sheet2pack converter + enrichment pipeline
  (transcripts, lesson/quiz generation, eval) and its test suite.
  Creator-side tooling; the learner app never depended on it.
- `.claude/skills/enrich-course` — the authoring skill that drove that
  pipeline.

## The contract between the products

**Pack schema compatibility** (`learnforge-core/topic-packs/pack-schema.json`).
Studio exports licensed course packs; this app imports them. Any schema
change here must keep externally produced packs (including
`licensed:{pack_id}|{licensor}` provenance) importable. That is the only
hard coupling.

## Rules

1. No cross-repo build dependency, in either direction.
2. Bug fixes to shared-ancestry code move across via manual cherry-pick
   when relevant, recorded with a `Cross-ported-from:` line in the
   commit body.
3. Nothing closed-source flows into this repo; MIT stays MIT.
