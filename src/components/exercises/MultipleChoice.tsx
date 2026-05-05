import { useState, useCallback, useMemo } from "react";
import { Send, CheckCircle2, XCircle, Lightbulb } from "lucide-react";
import { MarkdownRenderer } from "@/components/learning/MarkdownRenderer";
import type { Exercise } from "@/types/exercises";
import { cn } from "@/lib/utils";

interface MultipleChoiceProps {
  exercise: Exercise;
  onComplete: (score: number) => void;
}

export function MultipleChoice({ exercise, onComplete }: MultipleChoiceProps) {
  const options = exercise.metadata.options ?? [];
  const correctIndices = useMemo(
    () => new Set(exercise.metadata.correctIndices ?? []),
    [exercise.metadata.correctIndices]
  );
  const isMultiSelect = correctIndices.size > 1;

  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [submitted, setSubmitted] = useState(false);
  const [score, setScore] = useState<number | null>(null);
  const [hintsRevealed, setHintsRevealed] = useState(0);

  const toggleOption = useCallback(
    (index: number) => {
      if (submitted) return;
      setSelected((prev) => {
        const next = new Set(prev);
        if (isMultiSelect) {
          if (next.has(index)) next.delete(index);
          else next.add(index);
        } else {
          next.clear();
          next.add(index);
        }
        return next;
      });
    },
    [submitted, isMultiSelect]
  );

  const handleSubmit = useCallback(() => {
    if (selected.size === 0 || submitted) return;
    const matches =
      selected.size === correctIndices.size &&
      [...selected].every((i) => correctIndices.has(i));
    const computed = matches ? 100 : 0;
    setScore(computed);
    setSubmitted(true);
    onComplete(computed);
  }, [selected, correctIndices, submitted, onComplete]);

  const revealNextHint = useCallback(() => {
    setHintsRevealed((prev) => Math.min(prev + 1, exercise.hints.length));
  }, [exercise.hints.length]);

  if (options.length === 0) {
    return (
      <div className="rounded-lg border border-border bg-secondary/30 p-4 text-sm text-muted-foreground">
        No options available for this question.
      </div>
    );
  }

  const correct = score === 100;

  return (
    <div className="space-y-4">
      {/* Question */}
      <div className="glass rounded-lg p-5">
        <MarkdownRenderer content={exercise.prompt} />
        {isMultiSelect && (
          <p className="mt-2 text-xs text-muted-foreground">
            Select all that apply.
          </p>
        )}
      </div>

      {/* Options */}
      <div className="space-y-2">
        {options.map((option, index) => {
          const isSelected = selected.has(index);
          const isCorrectChoice = correctIndices.has(index);
          const showCorrectness = submitted;

          return (
            <button
              key={index}
              type="button"
              onClick={() => toggleOption(index)}
              disabled={submitted}
              className={cn(
                "flex w-full items-center gap-3 rounded-lg border px-4 py-3 text-left text-sm transition-colors",
                !submitted && isSelected && "border-primary bg-primary/10",
                !submitted && !isSelected && "border-border bg-background hover:bg-secondary/50",
                submitted && isCorrectChoice && "border-green-500/50 bg-green-500/10",
                submitted && isSelected && !isCorrectChoice && "border-red-500/50 bg-red-500/10",
                submitted && !isSelected && !isCorrectChoice && "border-border bg-background opacity-60",
                submitted && "cursor-default"
              )}
              aria-pressed={isSelected}
            >
              <span
                className={cn(
                  "flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full border text-xs font-semibold",
                  !submitted && isSelected && "border-primary bg-primary text-primary-foreground",
                  !submitted && !isSelected && "border-border text-muted-foreground",
                  submitted && isCorrectChoice && "border-green-500 bg-green-500 text-white",
                  submitted && isSelected && !isCorrectChoice && "border-red-500 bg-red-500 text-white",
                  submitted && !isSelected && !isCorrectChoice && "border-border text-muted-foreground"
                )}
              >
                {String.fromCharCode(65 + index)}
              </span>
              <span className="flex-1 text-foreground">{option}</span>
              {showCorrectness && isCorrectChoice && (
                <CheckCircle2 size={18} className="text-green-500" />
              )}
              {showCorrectness && isSelected && !isCorrectChoice && (
                <XCircle size={18} className="text-red-500" />
              )}
            </button>
          );
        })}
      </div>

      {/* Hints */}
      {exercise.hints.length > 0 && !submitted && (
        <div>
          <button
            onClick={revealNextHint}
            disabled={hintsRevealed >= exercise.hints.length}
            className="flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
          >
            <Lightbulb size={14} />
            <span>
              {hintsRevealed === 0
                ? "Show a hint"
                : hintsRevealed < exercise.hints.length
                  ? "Show another hint"
                  : "All hints revealed"}
            </span>
          </button>
          {hintsRevealed > 0 && (
            <div className="mt-2 space-y-2">
              {exercise.hints.slice(0, hintsRevealed).map((hint, i) => (
                <div
                  key={i}
                  className="rounded-md border border-border bg-secondary/30 px-3 py-2 text-sm text-muted-foreground"
                >
                  <span className="font-medium">Hint {i + 1}:</span> {hint}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Submit button — hidden once submitted */}
      {!submitted && (
        <button
          onClick={handleSubmit}
          disabled={selected.size === 0}
          className={cn(
            "flex items-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors",
            selected.size > 0
              ? "bg-primary text-primary-foreground hover:bg-primary/90"
              : "bg-secondary text-muted-foreground"
          )}
        >
          <Send size={16} />
          <span>Submit Answer</span>
        </button>
      )}

      {/* Result banner */}
      {submitted && (
        <div className="space-y-3">
          <div
            className={cn(
              "flex items-center gap-3 rounded-lg border px-4 py-3",
              correct
                ? "border-green-500/30 bg-green-500/10"
                : "border-red-500/30 bg-red-500/10"
            )}
          >
            {correct ? (
              <CheckCircle2 size={20} className="text-green-500" />
            ) : (
              <XCircle size={20} className="text-red-500" />
            )}
            <div>
              <p className="text-sm font-semibold text-foreground">
                {correct ? "Correct" : "Incorrect"}
              </p>
              <p className="text-xs text-muted-foreground">
                Score: {score ?? 0}/100
              </p>
            </div>
          </div>

          {/* Show correct answer label so wrong answers learn */}
          <div className="rounded-lg border border-border bg-secondary/30 px-4 py-3 text-sm">
            <span className="font-medium text-foreground">Correct answer: </span>
            <span className="text-muted-foreground">
              {[...correctIndices]
                .sort((a, b) => a - b)
                .map((i) => `${String.fromCharCode(65 + i)}. ${options[i]}`)
                .join("; ")}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
