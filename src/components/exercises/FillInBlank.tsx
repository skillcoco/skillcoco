import { useState, useMemo, useCallback } from "react";
import { Send, CheckCircle2, XCircle, RotateCcw } from "lucide-react";
import type { Exercise } from "@/types/exercises";
import { cn } from "@/lib/utils";

interface FillInBlankProps {
  exercise: Exercise;
  onComplete: (score: number) => void;
}

interface BlankState {
  id: string;
  value: string;
  isCorrect: boolean | null;
}

export function FillInBlank({ exercise, onComplete }: FillInBlankProps) {
  const blanks = exercise.metadata.blanks ?? [];
  const template = exercise.metadata.template ?? exercise.prompt;

  const [blankStates, setBlankStates] = useState<BlankState[]>(() =>
    blanks.map((b) => ({
      id: b.id,
      value: "",
      isCorrect: null,
    }))
  );
  const [isSubmitted, setIsSubmitted] = useState(false);

  // Parse the template and split into text segments and blank positions
  const segments = useMemo(() => {
    const parts: Array<{ type: "text"; content: string } | { type: "blank"; index: number }> = [];
    // Support both `___` and `{{blank}}` markers
    const regex = /___|\{\{blank\}\}/g;
    let lastIndex = 0;
    let blankIndex = 0;
    let match;

    while ((match = regex.exec(template)) !== null) {
      if (match.index > lastIndex) {
        parts.push({ type: "text", content: template.slice(lastIndex, match.index) });
      }
      parts.push({ type: "blank", index: blankIndex });
      blankIndex++;
      lastIndex = match.index + match[0].length;
    }

    if (lastIndex < template.length) {
      parts.push({ type: "text", content: template.slice(lastIndex) });
    }

    return parts;
  }, [template]);

  const updateBlank = useCallback((index: number, value: string) => {
    setBlankStates((prev) => {
      const next = [...prev];
      next[index] = { ...next[index], value };
      return next;
    });
  }, []);

  const handleSubmit = useCallback(() => {
    const evaluated = blankStates.map((state, i) => {
      const blank = blanks[i];
      if (!blank) return { ...state, isCorrect: false };

      const isCorrect = blank.acceptedAnswers.some(
        (accepted) => accepted.toLowerCase().trim() === state.value.toLowerCase().trim()
      );
      return { ...state, isCorrect };
    });

    setBlankStates(evaluated);
    setIsSubmitted(true);

    const correctCount = evaluated.filter((b) => b.isCorrect).length;
    const score = blanks.length > 0 ? Math.round((correctCount / blanks.length) * 100) : 0;
    onComplete(score);
  }, [blankStates, blanks, onComplete]);

  const handleReset = useCallback(() => {
    setBlankStates(blanks.map((b) => ({ id: b.id, value: "", isCorrect: null })));
    setIsSubmitted(false);
  }, [blanks]);

  const allFilled = blankStates.every((b) => b.value.trim() !== "");
  const correctCount = blankStates.filter((b) => b.isCorrect === true).length;
  const totalBlanks = blanks.length;

  return (
    <div className="space-y-4">
      {/* Prompt (if separate from template) */}
      {exercise.metadata.template && (
        <div className="glass rounded-lg p-5">
          <p className="text-sm leading-7 text-foreground/90">{exercise.prompt}</p>
        </div>
      )}

      {/* Fill-in-the-blank content */}
      <div className="glass rounded-lg p-6">
        <div className="text-base leading-8 text-foreground/90">
          {segments.map((seg, i) => {
            if (seg.type === "text") {
              return (
                <span key={i} className="whitespace-pre-wrap">
                  {seg.content}
                </span>
              );
            }

            const blank = blankStates[seg.index];
            if (!blank) return null;

            return (
              <span key={i} className="mx-1 inline-block align-baseline">
                <input
                  type="text"
                  value={blank.value}
                  onChange={(e) => updateBlank(seg.index, e.target.value)}
                  disabled={isSubmitted}
                  placeholder={blanks[seg.index]?.hint ?? "..."}
                  className={cn(
                    "inline-block w-40 rounded-md border-b-2 bg-transparent px-2 py-0.5 text-center font-mono text-sm transition-colors focus:outline-none",
                    blank.isCorrect === null && "border-border focus:border-primary",
                    blank.isCorrect === true && "border-green-500 bg-green-500/10 text-green-700 dark:text-green-400",
                    blank.isCorrect === false && "border-red-500 bg-red-500/10 text-red-700 dark:text-red-400"
                  )}
                />
                {isSubmitted && blank.isCorrect === false && blanks[seg.index] && (
                  <span className="ml-1 text-xs text-muted-foreground">
                    (expected: {blanks[seg.index].acceptedAnswers[0]})
                  </span>
                )}
              </span>
            );
          })}
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-3">
        {!isSubmitted ? (
          <button
            onClick={handleSubmit}
            disabled={!allFilled}
            className={cn(
              "flex items-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors",
              allFilled
                ? "bg-primary text-primary-foreground hover:bg-primary/90"
                : "bg-secondary text-muted-foreground"
            )}
          >
            <Send size={16} />
            <span>Check Answers</span>
          </button>
        ) : (
          <button
            onClick={handleReset}
            className="flex items-center gap-2 rounded-lg border border-border px-4 py-2.5 text-sm font-medium text-foreground transition-colors hover:bg-accent"
          >
            <RotateCcw size={16} />
            <span>Try Again</span>
          </button>
        )}
      </div>

      {/* Score summary */}
      {isSubmitted && (
        <div
          className={cn(
            "flex items-center gap-3 rounded-lg border px-4 py-3",
            correctCount === totalBlanks
              ? "border-green-500/30 bg-green-500/10"
              : correctCount > 0
                ? "border-orange-500/30 bg-orange-500/10"
                : "border-red-500/30 bg-red-500/10"
          )}
        >
          {correctCount === totalBlanks ? (
            <CheckCircle2 size={20} className="text-green-500" />
          ) : (
            <XCircle size={20} className={correctCount > 0 ? "text-orange-500" : "text-red-500"} />
          )}
          <p className="text-sm font-medium text-foreground">
            {correctCount} of {totalBlanks} correct
            {correctCount === totalBlanks && " -- all blanks filled correctly"}
          </p>
        </div>
      )}
    </div>
  );
}
