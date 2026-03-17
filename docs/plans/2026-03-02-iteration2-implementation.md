# Iteration 2: Adaptive Loop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the adaptive learning loop so a user can go from onboarding to sustained learning with mastery tracking and spaced repetition review.

**Architecture:** New Rust command `complete_module_exercises` runs BKT, updates mastery, unlocks DAG dependents, generates SR cards. Frontend auto-generates exercises on first module visit, wires exercise completion to mastery update, and implements ReviewSession with flip-card UI.

**Tech Stack:** Rust (Tauri commands, BKT, SM-2), React + TypeScript (Zustand, Lucide), Vitest + React Testing Library

---

### Task 1: Rust command `complete_module_exercises`

**Files:**
- Modify: `src-tauri/src/commands/ai.rs` (add command at bottom)
- Modify: `src-tauri/src/lib.rs:74` (register command)
- Test: `src-tauri/src/commands/ai.rs` (inline #[cfg(test)] module)

This is the critical backend piece. Takes module_id + scores, runs BKT to update mastery, marks module completed if mastery >= 0.7, unlocks dependent modules in the DAG, and generates SR cards for the module's key concepts.

**Step 1: Write the failing test**

Add to the bottom of `src-tauri/src/commands/ai.rs` inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_bkt_mastery_update_logic() {
    use crate::learning::adaptive::{BKTParams, update_mastery};
    let params = BKTParams::default();
    // Score 80/100 -> correct, Score 30/100 -> incorrect
    let scores = vec![80.0, 30.0, 90.0];
    let mut mastery = 0.3; // starting mastery
    for score in &scores {
        let is_correct = *score >= 50.0;
        mastery = update_mastery(&params, mastery, is_correct);
    }
    assert!(mastery > 0.3, "Mastery should increase with 2/3 correct");
    assert!(mastery < 1.0);
}
```

**Step 2: Run test to verify it passes** (this is a unit test of existing BKT, should pass)

Run: `cd src-tauri && cargo test test_bkt_mastery_update_logic -- --nocapture`

**Step 3: Write the command**

Add to bottom of `src-tauri/src/commands/ai.rs` (before any `#[cfg(test)]` block):

```rust
#[derive(Debug, Deserialize)]
pub struct CompleteExercisesRequest {
    pub module_id: String,
    pub track_id: String,
    pub scores: Vec<f64>, // 0-100 per exercise
}

#[derive(Debug, Serialize)]
pub struct CompleteExercisesResult {
    pub mastery_level: f64,
    pub module_completed: bool,
    pub unlocked_modules: Vec<String>, // module IDs now available
    pub cards_created: i32,
}

#[tauri::command]
pub fn complete_module_exercises(
    state: State<AppState>,
    request: CompleteExercisesRequest,
) -> Result<CompleteExercisesResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let params = crate::learning::adaptive::BKTParams::default();

    // 1. Get current mastery
    let current_mastery: f64 = db.conn
        .query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 LIMIT 1",
            [&request.module_id],
            |row| row.get(0),
        )
        .unwrap_or(0.3); // default prior if no progress row

    // 2. Run BKT for each score
    let mut mastery = current_mastery;
    for score in &request.scores {
        let is_correct = *score >= 50.0;
        mastery = crate::learning::adaptive::update_mastery(&params, mastery, is_correct);
    }

    // 3. Compute average score
    let avg_score = if request.scores.is_empty() {
        0.0
    } else {
        request.scores.iter().sum::<f64>() / request.scores.len() as f64
    };

    let module_completed = mastery >= 0.7;
    let new_status = if module_completed { "completed" } else { "in_progress" };

    // 4. Upsert module_progress
    let profile_id: String = db.conn
        .query_row("SELECT id FROM learner_profiles LIMIT 1", [], |row| row.get(0))
        .map_err(|e| format!("No profile: {}", e))?;

    let progress_id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, score, mastery_level, attempts, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, datetime('now'))
             ON CONFLICT(module_id, learner_id) DO UPDATE SET
               status = ?4, score = ?5, mastery_level = ?6,
               attempts = attempts + 1,
               completed_at = CASE WHEN ?4 = 'completed' THEN datetime('now') ELSE completed_at END",
            rusqlite::params![progress_id, request.module_id, profile_id, new_status, avg_score, mastery],
        )
        .map_err(|e| e.to_string())?;

    // 5. If completed, unlock dependent modules in DAG
    let mut unlocked = Vec::new();
    if module_completed {
        // Get the learning path for this track
        let path_row: Option<(String, String)> = db.conn
            .query_row(
                "SELECT modules_json, edges_json FROM learning_paths WHERE track_id = ?1 ORDER BY version DESC LIMIT 1",
                [&request.track_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if let Some((modules_json, edges_json)) = path_row {
            let edges: Vec<serde_json::Value> = serde_json::from_str(&edges_json).unwrap_or_default();

            // Find modules that depend on the completed module
            let dependents: Vec<String> = edges.iter()
                .filter(|e| e["from"].as_str() == Some(&request.module_id))
                .filter_map(|e| e["to"].as_str().map(String::from))
                .collect();

            // For each dependent, check if ALL prerequisites are completed
            for dep_id in &dependents {
                let prereqs: Vec<String> = edges.iter()
                    .filter(|e| e["to"].as_str() == Some(dep_id.as_str()))
                    .filter_map(|e| e["from"].as_str().map(String::from))
                    .collect();

                let all_prereqs_done = prereqs.iter().all(|prereq_id| {
                    db.conn.query_row(
                        "SELECT status FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
                        rusqlite::params![prereq_id, profile_id],
                        |row| row.get::<_, String>(0),
                    )
                    .map(|s| s == "completed")
                    .unwrap_or(false)
                });

                if all_prereqs_done {
                    // Unlock: insert or update progress to 'available'
                    let unlock_id = uuid::Uuid::new_v4().to_string();
                    db.conn.execute(
                        "INSERT INTO module_progress (id, module_id, learner_id, status)
                         VALUES (?1, ?2, ?3, 'available')
                         ON CONFLICT(module_id, learner_id) DO UPDATE SET
                           status = CASE WHEN status = 'locked' THEN 'available' ELSE status END",
                        rusqlite::params![unlock_id, dep_id, profile_id],
                    ).ok();
                    unlocked.push(dep_id.clone());
                }
            }
        }

        // Update track progress_percent
        let total_modules: i64 = db.conn
            .query_row(
                "SELECT COUNT(*) FROM modules m JOIN learning_paths lp ON m.path_id = lp.id WHERE lp.track_id = ?1",
                [&request.track_id],
                |row| row.get(0),
            )
            .unwrap_or(1);
        let completed_modules: i64 = db.conn
            .query_row(
                "SELECT COUNT(*) FROM module_progress mp
                 JOIN modules m ON mp.module_id = m.id
                 JOIN learning_paths lp ON m.path_id = lp.id
                 WHERE lp.track_id = ?1 AND mp.status = 'completed'",
                [&request.track_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let pct = if total_modules > 0 { (completed_modules as f64 / total_modules as f64) * 100.0 } else { 0.0 };
        db.conn.execute(
            "UPDATE learning_tracks SET progress_percent = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![pct, request.track_id],
        ).ok();
    }

    // 6. Generate SR cards for the module (one per key concept from exercise prompts)
    let mut cards_created = 0i32;
    let mut exercise_stmt = db.conn
        .prepare("SELECT id, prompt, exercise_type FROM exercises WHERE module_id = ?1")
        .map_err(|e| e.to_string())?;
    let exercises: Vec<(String, String, String)> = exercise_stmt
        .query_map([&request.module_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    for (ex_id, prompt, ex_type) in &exercises {
        let card_id = uuid::Uuid::new_v4().to_string();
        let concept = prompt.chars().take(80).collect::<String>();
        let front = format!("Recall: {}", prompt.chars().take(200).collect::<String>());
        let back = format!("Review your {} exercise answer.", ex_type.replace('_', " "));
        db.conn.execute(
            "INSERT OR IGNORE INTO sr_cards (id, module_id, concept, card_type, front, back)
             VALUES (?1, ?2, ?3, 'active_recall', ?4, ?5)",
            rusqlite::params![card_id, request.module_id, concept, front, back],
        ).ok();
        cards_created += 1;
    }

    Ok(CompleteExercisesResult {
        mastery_level: mastery,
        module_completed,
        unlocked_modules: unlocked,
        cards_created,
    })
}
```

**Step 4: Register the command in `lib.rs`**

Add after `commands::ai::evaluate_response,`:
```rust
commands::ai::complete_module_exercises,
```

**Step 5: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All 52+ tests pass, compilation succeeds.

**Step 6: Commit**

```bash
git add src-tauri/src/commands/ai.rs src-tauri/src/lib.rs
git commit -m "feat: add complete_module_exercises command with BKT + DAG unlock + SR cards"
```

---

### Task 2: Frontend tauri-commands + store wiring

**Files:**
- Modify: `src/lib/tauri-commands.ts` (add command wrapper)
- Modify: `src/stores/useLearningStore.ts` (add action)
- Modify: `src/types/learning.ts` (add result type)

**Step 1: Add TypeScript types**

Add to bottom of `src/types/learning.ts`:

```typescript
export interface CompleteExercisesResult {
  masteryLevel: number;
  moduleCompleted: boolean;
  unlockedModules: string[];
  cardsCreated: number;
}
```

**Step 2: Add tauri command wrapper**

Add to `src/lib/tauri-commands.ts` after the exercise section:

```typescript
// ── Module Completion ──

export async function completeModuleExercises(
  moduleId: string,
  trackId: string,
  scores: number[],
): Promise<import("@/types/learning").CompleteExercisesResult> {
  return invoke("complete_module_exercises", {
    request: { moduleId, trackId, scores },
  });
}
```

**Step 3: Add store action**

Add to `useLearningStore.ts` interface:
```typescript
completeExercises: (moduleId: string, trackId: string, scores: number[]) => Promise<import("@/types/learning").CompleteExercisesResult>;
```

Add to store implementation:
```typescript
completeExercises: async (moduleId, trackId, scores) => {
  const result = await commands.completeModuleExercises(moduleId, trackId, scores);
  // Refresh module progress to reflect mastery + unlocks
  const progress = await commands.getModuleProgress(trackId);
  set({ moduleProgress: progress });
  return result;
},
```

**Step 4: Commit**

```bash
git add src/lib/tauri-commands.ts src/stores/useLearningStore.ts src/types/learning.ts
git commit -m "feat: add completeModuleExercises command wrapper and store action"
```

---

### Task 3: Wire ExerciseContainer to BKT completion

**Files:**
- Modify: `src/components/exercises/ExerciseContainer.tsx`
- Modify: `src/pages/ModuleView.tsx`

**Step 1: Update ExerciseContainer to report scores**

Change `onAllComplete` prop to pass scores. In `ExerciseContainer.tsx`, change the interface:

```typescript
interface ExerciseContainerProps {
  moduleId: string;
  onAllComplete?: (scores: number[]) => void;
}
```

Change the `handleComplete` callback's inner check:

```typescript
if (next.size === exercises.length && onAllComplete) {
  const allScores = exercises.map((ex) => next.get(ex.id) ?? 0);
  setTimeout(() => onAllComplete(allScores), 0);
}
```

**Step 2: Wire ModuleView to call completeExercises**

In `ModuleView.tsx`, import the store action and update the handler:

```typescript
const { currentTrack, currentPath, moduleProgress, selectTrack, completeExercises } = useLearningStore();
```

Replace `handleExercisesComplete`:

```typescript
const handleExercisesComplete = useCallback(async (scores: number[]) => {
  if (!trackId || !moduleId) return;
  try {
    const result = await completeExercises(moduleId, trackId, scores);
    if (result.moduleCompleted) {
      // Navigate back to track view to see unlocked modules
      navigate(`/track/${trackId}`);
    }
  } catch (err) {
    console.error("Failed to complete exercises:", err);
  }
}, [trackId, moduleId, completeExercises, navigate]);
```

Update the JSX to pass scores:

```tsx
<ExerciseContainer
  moduleId={moduleId}
  onAllComplete={handleExercisesComplete}
/>
```

**Step 3: Compile and verify**

Run: `npx vitest run`
Expected: All existing tests pass.

**Step 4: Commit**

```bash
git add src/components/exercises/ExerciseContainer.tsx src/pages/ModuleView.tsx
git commit -m "feat: wire exercise completion to BKT mastery update and module unlock"
```

---

### Task 4: Auto-generate exercises on first module visit

**Files:**
- Modify: `src/components/exercises/ExerciseContainer.tsx`

When exercises are empty, auto-generate 3 exercises (one per type). This uses the existing `generate_exercise` command.

**Step 1: Add auto-generation logic**

In `ExerciseContainer.tsx`, modify the `useEffect` that loads exercises:

```typescript
useEffect(() => {
  let cancelled = false;

  async function load() {
    setIsLoading(true);
    setError(null);
    try {
      let result = await getExercises(moduleId);

      // Auto-generate if none exist
      if (result.length === 0) {
        const types = ["conceptual_qa", "code_challenge", "fill_in_blank"];
        const generated = [];
        for (const type of types) {
          try {
            const ex = await generateExercise({
              moduleId,
              difficulty: 5,
              type,
              context: `Module exercises for adaptive learning`,
            });
            generated.push(ex as unknown as Exercise);
          } catch (genErr) {
            console.error(`Failed to generate ${type} exercise:`, genErr);
          }
        }
        if (generated.length > 0) {
          result = await getExercises(moduleId); // reload from DB
        }
      }

      if (!cancelled) {
        setExercises(result);
        setCurrentIndex(0);
        setScores(new Map());
      }
    } catch (err) {
      if (!cancelled) {
        setError(String(err));
      }
    } finally {
      if (!cancelled) setIsLoading(false);
    }
  }

  load();
  return () => { cancelled = true; };
}, [moduleId]);
```

Add the import:
```typescript
import { getExercises, generateExercise } from "@/lib/tauri-commands";
```

Update the loading message to indicate generation:

```tsx
{isLoading && (
  <div className="flex h-48 flex-col items-center justify-center text-muted-foreground">
    <Loader2 size={20} className="mb-2 animate-spin" />
    <span>Preparing exercises...</span>
    <span className="mt-1 text-xs text-muted-foreground/70">
      Generating personalized exercises for this module
    </span>
  </div>
)}
```

**Step 2: Commit**

```bash
git add src/components/exercises/ExerciseContainer.tsx
git commit -m "feat: auto-generate exercises on first module visit"
```

---

### Task 5: ReviewSession UI

**Files:**
- Rewrite: `src/pages/ReviewSession.tsx`
- Test: `src/pages/__tests__/ReviewSession.test.tsx`

**Step 1: Write the failing test**

Create `src/pages/__tests__/ReviewSession.test.tsx`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ReviewSession } from "../ReviewSession";

// Mock tauri commands
const mockGetDueCards = vi.fn();
const mockSubmitReview = vi.fn();

vi.mock("@/lib/tauri-commands", () => ({
  getDueCards: (...args: unknown[]) => mockGetDueCards(...args),
  submitReview: (...args: unknown[]) => mockSubmitReview(...args),
}));

const MOCK_CARD = {
  id: "card-1",
  moduleId: "mod-1",
  concept: "Kubernetes Pods",
  cardType: "active_recall",
  front: "What is a Pod in Kubernetes?",
  back: "A Pod is the smallest deployable unit in Kubernetes, containing one or more containers.",
  intervalDays: 1,
  easeFactor: 2.5,
  repetitions: 0,
  nextReview: new Date().toISOString(),
  lastReview: null,
};

function renderReviewSession() {
  return render(
    <MemoryRouter>
      <ReviewSession />
    </MemoryRouter>,
  );
}

describe("ReviewSession", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows loading state initially", () => {
    mockGetDueCards.mockReturnValue(new Promise(() => {})); // never resolves
    renderReviewSession();
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it("shows empty state when no cards due", async () => {
    mockGetDueCards.mockResolvedValue([]);
    renderReviewSession();
    expect(await screen.findByText(/no cards due/i)).toBeInTheDocument();
  });

  it("shows card front initially", async () => {
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    renderReviewSession();
    expect(await screen.findByText(/what is a pod/i)).toBeInTheDocument();
  });

  it("reveals answer on click", async () => {
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    renderReviewSession();
    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);
    expect(await screen.findByText(/smallest deployable unit/i)).toBeInTheDocument();
  });

  it("shows rating buttons after reveal", async () => {
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    renderReviewSession();
    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);
    expect(await screen.findByText("Again")).toBeInTheDocument();
    expect(screen.getByText("Hard")).toBeInTheDocument();
    expect(screen.getByText("Good")).toBeInTheDocument();
    expect(screen.getByText("Easy")).toBeInTheDocument();
  });

  it("shows completion when all cards reviewed", async () => {
    mockGetDueCards.mockResolvedValue([MOCK_CARD]);
    mockSubmitReview.mockResolvedValue({ ...MOCK_CARD, intervalDays: 6 });
    renderReviewSession();

    // Reveal
    const revealBtn = await screen.findByText(/show answer/i);
    fireEvent.click(revealBtn);

    // Rate
    const goodBtn = await screen.findByText("Good");
    fireEvent.click(goodBtn);

    // Should show completion
    expect(await screen.findByText(/session complete/i)).toBeInTheDocument();
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `npx vitest run src/pages/__tests__/ReviewSession.test.tsx`
Expected: FAIL (current ReviewSession is a stub)

**Step 3: Implement ReviewSession**

Rewrite `src/pages/ReviewSession.tsx`:

```typescript
import { useState, useEffect, useCallback } from "react";
import { Link } from "react-router-dom";
import {
  ArrowLeft,
  Loader2,
  RotateCcw,
  CheckCircle2,
  ChevronRight,
  Brain,
} from "lucide-react";
import { getDueCards, submitReview } from "@/lib/tauri-commands";
import type { SRCard } from "@/types";
import { cn } from "@/lib/utils";

type SessionState = "loading" | "empty" | "reviewing" | "complete";

export function ReviewSession() {
  const [cards, setCards] = useState<SRCard[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [sessionState, setSessionState] = useState<SessionState>("loading");
  const [reviewedCount, setReviewedCount] = useState(0);
  const [ratings, setRatings] = useState<number[]>([]);

  useEffect(() => {
    async function load() {
      try {
        const due = await getDueCards();
        setCards(due);
        setSessionState(due.length === 0 ? "empty" : "reviewing");
      } catch (err) {
        console.error("Failed to load due cards:", err);
        setSessionState("empty");
      }
    }
    load();
  }, []);

  const currentCard = cards[currentIndex];

  const handleRate = useCallback(
    async (quality: 1 | 3 | 4 | 5) => {
      if (!currentCard) return;

      try {
        await submitReview({
          cardId: currentCard.id,
          quality,
          responseTime: 0,
          response: "",
        });
      } catch (err) {
        console.error("Failed to submit review:", err);
      }

      setRatings((prev) => [...prev, quality]);
      setRevealed(false);
      setReviewedCount((prev) => prev + 1);

      if (currentIndex + 1 >= cards.length) {
        setSessionState("complete");
      } else {
        setCurrentIndex((prev) => prev + 1);
      }
    },
    [currentCard, currentIndex, cards.length],
  );

  const avgRating =
    ratings.length > 0
      ? (ratings.reduce((a, b) => a + b, 0) / ratings.length).toFixed(1)
      : "0";

  // ── Loading ──
  if (sessionState === "loading") {
    return (
      <div className="mx-auto flex h-64 max-w-2xl items-center justify-center text-muted-foreground">
        <Loader2 size={20} className="mr-2 animate-spin" />
        <span>Loading review cards...</span>
      </div>
    );
  }

  // ── Empty ──
  if (sessionState === "empty") {
    return (
      <div className="mx-auto max-w-2xl space-y-6">
        <div className="flex items-center gap-3">
          <Link to="/" className="rounded-md p-1.5 text-muted-foreground hover:bg-accent">
            <ArrowLeft size={18} />
          </Link>
          <h1 className="text-2xl font-bold text-foreground">Review Session</h1>
        </div>
        <div className="glass flex flex-col items-center justify-center rounded-xl py-16 text-center">
          <CheckCircle2 size={48} className="mb-4 text-emerald-500" />
          <h2 className="text-lg font-semibold text-foreground">No cards due for review</h2>
          <p className="mt-1 text-sm text-muted-foreground">
            Keep studying and cards will appear as they become due.
          </p>
          <Link
            to="/"
            className="mt-6 inline-flex items-center gap-1.5 rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
          >
            Back to Dashboard
            <ChevronRight size={16} />
          </Link>
        </div>
      </div>
    );
  }

  // ── Complete ──
  if (sessionState === "complete") {
    return (
      <div className="mx-auto max-w-2xl space-y-6">
        <div className="flex items-center gap-3">
          <Link to="/" className="rounded-md p-1.5 text-muted-foreground hover:bg-accent">
            <ArrowLeft size={18} />
          </Link>
          <h1 className="text-2xl font-bold text-foreground">Review Session</h1>
        </div>
        <div className="glass flex flex-col items-center justify-center rounded-xl py-16 text-center">
          <CheckCircle2 size={48} className="mb-4 text-emerald-500" />
          <h2 className="text-lg font-semibold text-foreground">Session Complete</h2>
          <p className="mt-2 text-sm text-muted-foreground">
            You reviewed {reviewedCount} card{reviewedCount !== 1 ? "s" : ""} with an average
            rating of {avgRating}/5.
          </p>
          <Link
            to="/"
            className="mt-6 inline-flex items-center gap-1.5 rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
          >
            Back to Dashboard
            <ChevronRight size={16} />
          </Link>
        </div>
      </div>
    );
  }

  // ── Reviewing ──
  const progress = ((currentIndex + 1) / cards.length) * 100;

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <Link to="/" className="rounded-md p-1.5 text-muted-foreground hover:bg-accent">
          <ArrowLeft size={18} />
        </Link>
        <div className="flex-1">
          <h1 className="text-2xl font-bold text-foreground">Review Session</h1>
          <p className="text-sm text-muted-foreground">
            Card {currentIndex + 1} of {cards.length}
          </p>
        </div>
      </div>

      {/* Progress bar */}
      <div className="h-2 overflow-hidden rounded-full bg-secondary">
        <div
          className="h-full rounded-full bg-primary transition-all duration-300"
          style={{ width: `${progress}%` }}
        />
      </div>

      {/* Card */}
      <div className="glass rounded-xl p-8">
        {/* Concept badge */}
        <div className="mb-4 flex items-center gap-2">
          <Brain size={14} className="text-primary" />
          <span className="text-xs font-medium text-muted-foreground">
            {currentCard.concept}
          </span>
        </div>

        {/* Front */}
        <div className="mb-6">
          <p className="text-lg font-medium leading-relaxed text-foreground">
            {currentCard.front}
          </p>
        </div>

        {/* Divider + Answer */}
        {revealed ? (
          <>
            <div className="mb-6 border-t border-border" />
            <div className="rounded-lg bg-secondary/50 p-4">
              <p className="text-sm leading-relaxed text-foreground/90">
                {currentCard.back}
              </p>
            </div>

            {/* Rating buttons */}
            <div className="mt-6 grid grid-cols-4 gap-2">
              {([
                { quality: 1 as const, label: "Again", color: "text-red-500", desc: "Forgot" },
                { quality: 3 as const, label: "Hard", color: "text-orange-500", desc: "Struggled" },
                { quality: 4 as const, label: "Good", color: "text-emerald-500", desc: "Recalled" },
                { quality: 5 as const, label: "Easy", color: "text-blue-500", desc: "Instant" },
              ]).map(({ quality, label, color, desc }) => (
                <button
                  key={quality}
                  onClick={() => handleRate(quality)}
                  className="flex flex-col items-center gap-1 rounded-lg border border-border py-3 text-sm transition-colors hover:bg-accent"
                >
                  <span className={cn("font-semibold", color)}>{label}</span>
                  <span className="text-[10px] text-muted-foreground">{desc}</span>
                </button>
              ))}
            </div>
          </>
        ) : (
          <button
            onClick={() => setRevealed(true)}
            className="mt-4 w-full rounded-lg bg-primary py-3 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
          >
            Show Answer
          </button>
        )}
      </div>
    </div>
  );
}
```

**Step 4: Run tests to verify they pass**

Run: `npx vitest run src/pages/__tests__/ReviewSession.test.tsx`
Expected: All 6 tests PASS.

**Step 5: Run all tests**

Run: `npx vitest run`
Expected: All tests pass (21 existing + 6 new = 27).

**Step 6: Commit**

```bash
git add src/pages/ReviewSession.tsx src/pages/__tests__/ReviewSession.test.tsx
git commit -m "feat: implement ReviewSession with flip card UI and SM-2 rating"
```

---

### Task 6: Wire Dashboard to real data

**Files:**
- Modify: `src/pages/Dashboard.tsx`

The Dashboard already imports `useLearningStore` and calls `loadDueCards()`, but uses hardcoded estimates for module counts. Fix this by loading real path data.

**Step 1: Update Dashboard to load real module counts**

Replace the hardcoded `totalModulesAll` / `completedModulesAll` computation:

```typescript
// Replace lines 31-35 with:
const [moduleCounts, setModuleCounts] = useState<Record<string, { total: number; completed: number }>>({});

useEffect(() => {
  // Load module counts for each track
  async function loadModuleCounts() {
    const counts: Record<string, { total: number; completed: number }> = {};
    for (const track of tracks) {
      try {
        const [path, progress] = await Promise.all([
          commands.getPath(track.id),
          commands.getModuleProgress(track.id),
        ]);
        const total = path.modules?.length ?? 0;
        const completed = progress.filter((p) => p.status === "completed").length;
        counts[track.id] = { total, completed };
      } catch {
        counts[track.id] = { total: 0, completed: 0 };
      }
    }
    setModuleCounts(counts);
  }
  if (tracks.length > 0) loadModuleCounts();
}, [tracks]);

const totalModulesAll = Object.values(moduleCounts).reduce((s, c) => s + c.total, 0);
const completedModulesAll = Object.values(moduleCounts).reduce((s, c) => s + c.completed, 0);
```

Add the import:
```typescript
import * as commands from "@/lib/tauri-commands";
```

Update TrackCard rendering to use real counts:

```typescript
const trackCounts = moduleCounts[track.id] ?? { total: 0, completed: 0 };
return (
  <TrackCard
    key={track.id}
    track={track}
    dueReviews={dueCards.length} // TODO: per-track filtering
    totalModules={trackCounts.total}
    completedModules={trackCounts.completed}
    streakDays={0}
    nextModuleName={track.currentModuleId ? `Continue` : null}
  />
);
```

**Step 2: Run tests**

Run: `npx vitest run`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add src/pages/Dashboard.tsx
git commit -m "feat: wire Dashboard to real module counts and due cards"
```

---

### Task 7: Final integration test + verify

**Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass (52+ including new one).

**Step 2: Run all React tests**

Run: `npx vitest run`
Expected: All tests pass (27+).

**Step 3: Verify Rust compiles cleanly**

Run: `cd src-tauri && cargo check`
Expected: No errors.

**Step 4: Commit all remaining changes**

Only if there are unstaged fixes from test runs.

---

## Summary

| Task | What | Files | Tests |
|------|------|-------|-------|
| 1 | `complete_module_exercises` Rust command | `commands/ai.rs`, `lib.rs` | 1 Rust test |
| 2 | TS types + command wrapper + store | `tauri-commands.ts`, `useLearningStore.ts`, `learning.ts` | -- |
| 3 | Wire ExerciseContainer -> BKT | `ExerciseContainer.tsx`, `ModuleView.tsx` | -- |
| 4 | Auto-generate exercises on first visit | `ExerciseContainer.tsx` | -- |
| 5 | ReviewSession UI | `ReviewSession.tsx` | 6 React tests |
| 6 | Dashboard real data | `Dashboard.tsx` | -- |
| 7 | Integration verification | -- | Run all |
