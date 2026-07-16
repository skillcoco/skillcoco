# SkillCoco Iteration 2: Close the Adaptive Loop

**Date:** 2026-03-02
**Status:** Draft
**Goal:** Make the app testable end-to-end as a real adaptive learning product

---

## Problem

The core content pipeline works (onboarding -> assessment -> path -> content -> exercises -> eval), but:
- Exercise scores don't update mastery (BKT exists but isn't wired)
- Modules never unlock automatically after completing prerequisites
- Exercises aren't auto-generated when opening a module for the first time
- ReviewSession is a stub (SR backend is done, no UI)
- Dashboard shows static data, not real learning state

Without these, the app is a static content viewer. With them, it's a testable adaptive learning product.

## Scope

### 1. Wire BKT to Exercise Completion

**Current state:** `ExerciseContainer.onAllComplete(scores)` fires but does nothing with mastery.

**Change:** After all exercises in a module are scored:
1. Average the scores
2. Call `update_mastery()` with BKT params
3. Persist new mastery_level to module_progress
4. If mastery >= threshold (0.7): mark module completed, unlock dependents in DAG
5. If mastery < threshold: keep module in_progress (learner can retry)

**New Tauri command:** `complete_module_exercises` -- takes module_id + scores array, runs BKT, updates mastery, returns updated progress + newly unlocked modules.

**Frontend change:** `ExerciseContainer` calls `completeModuleExercises()` on all-complete, then `useLearningStore` refreshes module progress to show unlocked modules.

### 2. Auto-Generate Exercises on Module First Visit

**Current state:** `getExercises(moduleId)` returns empty array if none exist. No generation trigger.

**Change:** In `ModuleView`, after content loads, check if exercises exist. If not, call `generate_exercise` 3 times (one per type: conceptual_qa, code_challenge, fill_in_blank) with module context.

**Where:** Frontend logic in ModuleView/ExerciseContainer. No new Rust command needed -- use existing `generate_exercise` 3x.

### 3. ReviewSession UI

**Current state:** Stub page. Backend has `get_due_cards` + `submit_review` + SM-2 algorithm.

**Design:**
- Shows one card at a time
- Front side: concept + question (card.front)
- Tap/click to flip: answer (card.back)
- After flip: 4 quality buttons (Again=1, Hard=3, Good=4, Easy=5)
- Progress bar: X of Y cards
- Session complete screen with stats (cards reviewed, avg quality, next review)
- Navigate back to dashboard

**Styling:** Glassmorphism card with flip animation. Matches existing design system.

### 4. Dashboard Wiring

**Current state:** Shows track cards with hardcoded/empty metrics.

**Change:**
- Due cards count: call `getDueCards().length`
- Track progress: already in store, just wire the display
- "Start Review" button links to ReviewSession when due > 0
- Next recommended action: "Review X cards" or "Continue Module Y"

---

## Data Flow (Post-Iteration)

```
User completes exercises
  -> scores[] sent to complete_module_exercises
  -> BKT update_mastery(prior, scores)
  -> module_progress.mastery_level updated
  -> if mastery >= 0.7: module status = completed
  -> unlock dependent modules in DAG (status locked -> available)
  -> generate SR cards for module concepts
  -> return { updatedProgress, unlockedModules, newCards }

User opens ReviewSession
  -> get_due_cards() returns cards where next_review <= now
  -> user rates each card (quality 1-5)
  -> submit_review() applies SM-2, updates next_review
  -> session complete: show stats
```

## What's NOT in This Iteration

- OAuth flow (API key works, OAuth is Phase 2 polish)
- RuVector semantic search / embeddings
- SONA self-learning optimization
- Topic pack JSON templates (AI generates dynamically, which works)
- Module content progress tracking (scroll depth)
- Comprehensive test suite expansion (will do TDD for new code only)

## Success Criteria

After this iteration, a user can:
1. Create a track and get assessed
2. Study a module (AI-generated content)
3. Complete exercises (auto-generated on first visit)
4. See mastery update and next module unlock
5. Return later and review due SR cards
6. See real progress on dashboard
