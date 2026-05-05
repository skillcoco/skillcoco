import { useState, useEffect, useCallback } from "react";
import { ChevronLeft, ChevronRight, Loader2, BookOpen } from "lucide-react";
import { getExercises, generateExercise } from "@/lib/tauri-commands";
import { ConceptualQA } from "./ConceptualQA";
import { CodeChallenge } from "./CodeChallenge";
import { FillInBlank } from "./FillInBlank";
import { MultipleChoice } from "./MultipleChoice";
import type { Exercise } from "@/types/exercises";
import { cn } from "@/lib/utils";

interface ExerciseContainerProps {
  moduleId: string;
  onAllComplete?: (scores: number[]) => void;
}

export function ExerciseContainer({ moduleId, onAllComplete }: ExerciseContainerProps) {
  const [exercises, setExercises] = useState<Exercise[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [scores, setScores] = useState<Map<string, number>>(new Map());
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setIsLoading(true);
      setError(null);
      try {
        let result = await getExercises(moduleId);

        // Auto-generate if none exist. Default mix favours MCQ — fast,
        // click-and-done UX. fill_in_blank keeps a touch of variety.
        if (result.length === 0) {
          const types = ["multiple_choice", "multiple_choice", "fill_in_blank"];
          const generated = [];
          for (const type of types) {
            try {
              // Module + track context is fetched server-side from moduleId.
              // The optional `context` field is for learner-supplied hints only
              // (e.g. "focus on networking"); leave empty for default behavior.
              const ex = await generateExercise({
                moduleId,
                difficulty: 5,
                type,
                context: "",
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

  const handleComplete = useCallback(
    (score: number) => {
      const exercise = exercises[currentIndex];
      if (!exercise) return;

      setScores((prev) => {
        const next = new Map(prev);
        next.set(exercise.id, score);

        // Check if all exercises are now complete
        if (next.size === exercises.length && onAllComplete) {
          const allScores = exercises.map((ex) => next.get(ex.id) ?? 0);
          // Defer + isolate: parent's onAllComplete is async and may throw.
          // Wrap in Promise.resolve + .catch so an unhandled rejection
          // never propagates up and blanks the React tree.
          setTimeout(() => {
            Promise.resolve()
              .then(() => onAllComplete(allScores))
              .catch((err) => {
                console.error("onAllComplete failed:", err);
              });
          }, 0);
        }

        return next;
      });
    },
    [currentIndex, exercises, onAllComplete]
  );

  const goToNext = useCallback(() => {
    setCurrentIndex((prev) => Math.min(prev + 1, exercises.length - 1));
  }, [exercises.length]);

  const goToPrev = useCallback(() => {
    setCurrentIndex((prev) => Math.max(prev - 1, 0));
  }, []);

  if (isLoading) {
    return (
      <div className="flex h-48 flex-col items-center justify-center text-muted-foreground">
        <Loader2 size={20} className="mb-2 animate-spin" />
        <span>Preparing exercises...</span>
        <span className="mt-1 text-xs text-muted-foreground/70">
          Generating personalized exercises for this module
        </span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-4 text-sm text-foreground">
        Failed to load exercises: {error}
      </div>
    );
  }

  if (exercises.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <BookOpen size={40} className="mb-3 text-muted-foreground/50" />
        <p className="text-sm text-muted-foreground">No exercises available for this module yet.</p>
      </div>
    );
  }

  const currentExercise = exercises[currentIndex];
  const completedCount = scores.size;
  const progressPercent = (completedCount / exercises.length) * 100;

  return (
    <div className="space-y-4">
      {/* Header with progress */}
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-foreground">Exercises</h3>
        <span className="text-sm text-muted-foreground">
          Exercise {currentIndex + 1} of {exercises.length}
        </span>
      </div>

      {/* Progress bar */}
      <div className="h-2 overflow-hidden rounded-full bg-secondary">
        <div
          className="h-full rounded-full bg-primary transition-all duration-300"
          style={{ width: `${progressPercent}%` }}
        />
      </div>

      {/* Exercise type indicator */}
      <div className="flex items-center gap-2">
        <span
          className={cn(
            "rounded-full px-2.5 py-0.5 text-xs font-medium",
            "border border-border bg-secondary text-muted-foreground"
          )}
        >
          {formatExerciseType(currentExercise.type)}
        </span>
        <span className="text-xs text-muted-foreground">
          Difficulty: {currentExercise.difficulty}/10
        </span>
        {scores.has(currentExercise.id) && (
          <span
            className={cn(
              "rounded-full px-2.5 py-0.5 text-xs font-medium",
              (scores.get(currentExercise.id) ?? 0) >= 70
                ? "bg-green-500/10 text-green-600 dark:text-green-400"
                : "bg-red-500/10 text-red-600 dark:text-red-400"
            )}
          >
            Score: {scores.get(currentExercise.id)}/100
          </span>
        )}
      </div>

      {/* Exercise content */}
      <div>
        {currentExercise.type === "conceptual_qa" && (
          <ConceptualQA
            key={currentExercise.id}
            exercise={currentExercise}
            onComplete={handleComplete}
          />
        )}
        {currentExercise.type === "code_challenge" && (
          <CodeChallenge
            key={currentExercise.id}
            exercise={currentExercise}
            onComplete={handleComplete}
          />
        )}
        {currentExercise.type === "fill_in_blank" && (
          <FillInBlank
            key={currentExercise.id}
            exercise={currentExercise}
            onComplete={handleComplete}
          />
        )}
        {currentExercise.type === "multiple_choice" && (
          <MultipleChoice
            key={currentExercise.id}
            exercise={currentExercise}
            onComplete={handleComplete}
          />
        )}
        {!["conceptual_qa", "code_challenge", "fill_in_blank", "multiple_choice"].includes(currentExercise.type) && (
          <div className="glass rounded-lg p-5">
            <p className="text-sm text-muted-foreground">
              Exercise type "{currentExercise.type}" is not yet supported.
            </p>
          </div>
        )}
      </div>

      {/* Navigation */}
      <div className="flex items-center justify-between border-t border-border pt-4">
        <button
          onClick={goToPrev}
          disabled={currentIndex === 0}
          className={cn(
            "flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm transition-colors",
            currentIndex > 0
              ? "text-foreground hover:bg-accent"
              : "cursor-not-allowed text-muted-foreground/50"
          )}
        >
          <ChevronLeft size={16} />
          <span>Previous</span>
        </button>

        {/* Dot indicators */}
        <div className="flex items-center gap-1.5">
          {exercises.map((ex, i) => (
            <button
              key={ex.id}
              onClick={() => setCurrentIndex(i)}
              className={cn(
                "h-2 w-2 rounded-full transition-all",
                i === currentIndex && "h-2.5 w-2.5 bg-primary",
                i !== currentIndex && scores.has(ex.id) && "bg-green-500",
                i !== currentIndex && !scores.has(ex.id) && "bg-secondary"
              )}
              aria-label={`Go to exercise ${i + 1}`}
            />
          ))}
        </div>

        <button
          onClick={goToNext}
          disabled={currentIndex === exercises.length - 1}
          className={cn(
            "flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm transition-colors",
            currentIndex < exercises.length - 1
              ? "text-foreground hover:bg-accent"
              : "cursor-not-allowed text-muted-foreground/50"
          )}
        >
          <span>Next</span>
          <ChevronRight size={16} />
        </button>
      </div>
    </div>
  );
}

function formatExerciseType(type: string): string {
  const labels: Record<string, string> = {
    conceptual_qa: "Conceptual Q&A",
    code_challenge: "Code Challenge",
    fill_in_blank: "Fill in the Blank",
    multiple_choice: "Multiple Choice",
    architecture_design: "Architecture Design",
    scenario_debug: "Scenario Debug",
    teach_back: "Teach Back",
  };
  return labels[type] ?? type;
}
