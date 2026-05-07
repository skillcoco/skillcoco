# 2026-05-03 — Strategic Pivot: Usable v1.0 + DeepTutor Integration

## Why we're pivoting

LearnForge has 18 planned phases across 3 milestones, but no phase is complete and
the app is not usable end-to-end. Per the PROJECT.md "Known issues" audit:

- Exercise scores don't update BKT mastery
- Modules don't auto-unlock when prerequisites met
- Generated module content is not persisted to DB
- ReviewSession UI is a stub
- Dashboard shows placeholder counts

A learner can install LearnForge, onboard, and look at a path, but cannot
actually learn. We've been adding new features on top of a broken loop. This
document corrects course toward a usable-first roadmap and incorporates
high-leverage patterns from DeepTutor (HKUDS, Apache 2.0).

## New goal (gate)

**A new user installs LearnForge, picks a topic, learns something real, and
feels mastery move — within 10 minutes, every time, without bugs.**

Every phase from Phase 1 onward must produce a learner-facing improvement.
Architectural phases without learner value are pushed back.

## Phase reordering — v1.0

### Before (2026-03-17 roadmap, 8 phases)

1. Stabilize → 2. Core Extraction → 3. Adaptive Loop → 4. Microlearning →
5. Topic Packs → 6. Certification → 7. Publishing → 8. Web Platform & E2E

### After (2026-05-03 roadmap, 9 phases)

1. **Stabilize + Adaptive Loop** (merged) → 2. **Path Quality** (NEW) →
3. **Content Richness** (NEW) → 4. Microlearning → 5. Topic Packs →
6. Certification → 7. Core Extraction (moved) → 8. Publishing →
9. Web Platform & E2E

### Rationale

- **Phase 1 = Stabilize + Adaptive Loop merged**: stabilization without a working
  loop is meaningless. First ship must be a usable app, not a tested one.
- **Phase 2 (Path Quality) inserted**: directly addresses the recurring
  complaint that AI generates generic paths. Brings DeepTutor's
  Draft → Critique → Revise spine, concept graph, two-file memory, and YAML
  prompt management.
- **Phase 3 (Content Richness) inserted**: replaces flat markdown modules with
  block-based content (text / callout / quiz / flash_cards), giving BKT
  per-block mastery signals.
- **Core Extraction pushed to Phase 7**: pure architectural cleanup for the
  future corporate web app. Zero learner value. Premature before the public
  API stabilizes.
- **Web Platform pushed to Phase 9**: depends on stable, extracted algorithms.

## DeepTutor incorporation map

[DeepTutor](https://github.com/HKUDS/DeepTutor) (HKUDS, Apache 2.0) is the
inspiration for several upgrades. Patterns are reimplemented in Rust; verbatim
copies are the exception and are attributed per the licensing section below.

| Pattern | LearnForge home | Approach |
|---|---|---|
| Two-file Memory (PROFILE.md + SUMMARY.md, LLM-rewritten with `NO_CHANGE` sentinel) | Phase 2 | Reimplement in Rust |
| YAML PromptManager (singleton + cache + language fallback) | Phase 2 | Reimplement in Rust |
| Draft → Critique → Revise spine generation | Phase 2 | Reimplement in Rust; YAML prompts adapted with attribution |
| Concept Graph (typed nodes / edges, cycle removal, coverage padding) | Phase 2 | Reimplement in Rust |
| Block taxonomy (text / callout / quiz / flash_cards first) | Phase 3 | Reimplement in Rust |
| User Skills (`SKILL.md` injected into system prompt) | Phase 5 | Reimplement in Rust |
| QuizViewer.tsx | Phase 3 | Adapt React component (attribution required) |

## Out of scope (DeepTutor patterns we reject)

- **Book Engine as a whole** — too coupled to LlamaIndex + their RAG. Lift the
  staged-pipeline pattern only.
- **TutorBots** — persistent autonomous tutors. Out of LearnForge v1.0 scope.
- **AI Co-Writer, Math Animator (Manim), Visualize** — not learner-facing
  enough to justify the dependency footprint.
- **WebSocket /api/v1/ws protocol** — Tauri IPC already handles our needs.

## Licensing & attribution

DeepTutor is Apache 2.0. LearnForge desktop is MIT. Apache 2.0 → MIT is
compatible *if* required notices are preserved. Three categories:

1. **Patterns / architecture / ideas** — not copyrightable. Reimplement in
   Rust freely. Credit in `THIRD_PARTY_NOTICES.md` as good-faith attribution.
2. **YAML prompts adapted from DeepTutor** — file header comment
   `# Adapted from DeepTutor (HKUDS, Apache 2.0, https://github.com/HKUDS/DeepTutor)`
   plus an entry in `THIRD_PARTY_NOTICES.md`.
3. **React / TS components copied or substantially adapted** — file-header
   Apache 2.0 notice + entry in `THIRD_PARTY_NOTICES.md`. The upstream
   license text is stored at `licenses/APACHE-2.0-DeepTutor.txt`.

**Default:** prefer reimplementation over copying. Keeps the codebase cleanly
MIT and reduces attribution surface.

## Udemy Innovation Fund — dropped

We are no longer participating in the Udemy Innovation Fund. PUBL-04 (Udemy
materials) is removed from the requirements list. Phase 8 (Publishing & OSS
Launch) remains valuable as an open-source launch but is no longer
deadline-bound.

## Done checklist

- [x] PROJECT.md updated with new goal and "Definition of Usable" gate
- [x] ROADMAP.md restructured to 9 phases for v1.0
- [x] REQUIREMENTS.md adds PATH-, MEM-, PROMPT-, BLOCK- IDs and drops PUBL-04
- [x] STATE.md reflects new Phase 1 starting position
- [x] `THIRD_PARTY_NOTICES.md` created at repo root
- [x] `licenses/APACHE-2.0-DeepTutor.txt` placed
- [ ] `/gsd:plan-phase 1` — generate Phase 1 plan (next step)
- [ ] First Phase 1 commit closes one of the Known Issues
