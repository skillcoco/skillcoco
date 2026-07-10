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

## Roadmap-coupled work (this repo's side)

Creator Studio's roadmap will need the following from the learner app —
each belongs in this repo's own planning when prioritized:

1. **Pack-schema extension for new artifact types** — upcoming external
   packs will carry slide decks, video lessons (bundled mp4), and labs.
   Schema evolution must be coordinated and backward compatible (old
   packs keep importing). Expect a schema RFC before any exporter ships.
2. **Video lesson playback** — render pack-bundled explainer videos
   (local mp4 assets; asset-size strategy needed).
3. **Learner lab polish** — feed `lab_check_step` validation results
   into scoring/grading; add a browser preview pane for web-service labs.
4. **Phase 14 signing rail** — issuer certs + signature envelope over
   `licensed:{pack_id}|{licensor}`; externally produced signed packs
   depend on it.

## Rules

1. No cross-repo build dependency, in either direction.
2. Bug fixes to shared-ancestry code move across via manual cherry-pick
   when relevant, recorded with a `Cross-ported-from:` line in the
   commit body.
3. Nothing closed-source flows into this repo; MIT stays MIT.
