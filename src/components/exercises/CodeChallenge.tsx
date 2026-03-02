import { useState, useCallback } from "react";
import { Play, Loader2, CheckCircle2, XCircle, Lightbulb } from "lucide-react";
import { evaluateResponse } from "@/lib/tauri-commands";
import { MarkdownRenderer } from "@/components/learning/MarkdownRenderer";
import type { Exercise } from "@/types/exercises";
import type { EvaluateResponseResult } from "@/types/ai";
import { cn } from "@/lib/utils";

interface CodeChallengeProps {
  exercise: Exercise;
  onComplete: (score: number) => void;
}

export function CodeChallenge({ exercise, onComplete }: CodeChallengeProps) {
  const language = exercise.metadata.language ?? "text";
  const [code, setCode] = useState(exercise.metadata.starterCode ?? "");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [result, setResult] = useState<EvaluateResponseResult | null>(null);
  const [showHints, setShowHints] = useState(false);
  const [hintsRevealed, setHintsRevealed] = useState(0);

  const handleSubmit = useCallback(async () => {
    if (!code.trim() || isSubmitting) return;

    setIsSubmitting(true);
    try {
      const testCaseStr = exercise.metadata.testCases
        ?.map((tc) => `Input: ${tc.input} -> Expected: ${tc.expectedOutput} (${tc.description})`)
        .join("\n") ?? "";

      const evaluation = await evaluateResponse({
        exercisePrompt: exercise.prompt,
        learnerResponse: code,
        rubric: `Evaluate this ${language} code solution. Check correctness, code quality, and adherence to best practices.`,
        expectedAnswer: testCaseStr || undefined,
      });
      setResult(evaluation);
      onComplete(evaluation.score);
    } catch (err) {
      console.error("Failed to evaluate code:", err);
    } finally {
      setIsSubmitting(false);
    }
  }, [code, isSubmitting, exercise, language, onComplete]);

  const revealNextHint = useCallback(() => {
    setShowHints(true);
    setHintsRevealed((prev) => Math.min(prev + 1, exercise.hints.length));
  }, [exercise.hints.length]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Handle Tab key for indentation
      if (e.key === "Tab") {
        e.preventDefault();
        const target = e.currentTarget;
        const start = target.selectionStart;
        const end = target.selectionEnd;
        const newValue = code.substring(0, start) + "  " + code.substring(end);
        setCode(newValue);
        // Restore cursor position after state update
        requestAnimationFrame(() => {
          target.selectionStart = target.selectionEnd = start + 2;
        });
      }
    },
    [code]
  );

  return (
    <div className="space-y-4">
      {/* Challenge prompt */}
      <div className="glass rounded-lg p-5">
        <MarkdownRenderer content={exercise.prompt} />
      </div>

      {/* Test cases (visible ones) */}
      {exercise.metadata.testCases && exercise.metadata.testCases.filter((tc) => !tc.hidden).length > 0 && (
        <div className="rounded-lg border border-border p-4">
          <h4 className="mb-2 text-sm font-semibold text-foreground">Test Cases</h4>
          <div className="space-y-2">
            {exercise.metadata.testCases
              .filter((tc) => !tc.hidden)
              .map((tc, i) => (
                <div
                  key={i}
                  className="flex items-start gap-3 rounded-md bg-secondary/30 px-3 py-2 font-mono text-xs"
                >
                  <div className="flex-1">
                    <span className="text-muted-foreground">Input:</span>{" "}
                    <span className="text-foreground">{tc.input}</span>
                  </div>
                  <div className="flex-1">
                    <span className="text-muted-foreground">Expected:</span>{" "}
                    <span className="text-foreground">{tc.expectedOutput}</span>
                  </div>
                </div>
              ))}
          </div>
        </div>
      )}

      {/* Hints */}
      {exercise.hints.length > 0 && (
        <div>
          <button
            onClick={revealNextHint}
            disabled={hintsRevealed >= exercise.hints.length}
            className="flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
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
          {showHints && hintsRevealed > 0 && (
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

      {/* Code editor */}
      <div className="overflow-hidden rounded-lg border border-border">
        <div className="flex items-center justify-between border-b border-border bg-secondary/50 px-4 py-1.5">
          <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            {language}
          </span>
          <span className="text-xs text-muted-foreground">
            {code.split("\n").length} lines
          </span>
        </div>
        <textarea
          value={code}
          onChange={(e) => setCode(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={result !== null}
          placeholder={`Write your ${language} solution here...`}
          rows={16}
          spellCheck={false}
          className={cn(
            "w-full resize-y bg-card px-4 py-3 font-mono text-sm leading-6 text-foreground placeholder:text-muted-foreground focus:outline-none",
            result && "opacity-70"
          )}
        />
      </div>

      {/* Submit button */}
      {!result && (
        <button
          onClick={handleSubmit}
          disabled={!code.trim() || isSubmitting}
          className={cn(
            "flex items-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors",
            code.trim() && !isSubmitting
              ? "bg-primary text-primary-foreground hover:bg-primary/90"
              : "bg-secondary text-muted-foreground"
          )}
        >
          {isSubmitting ? (
            <>
              <Loader2 size={16} className="animate-spin" />
              <span>Evaluating...</span>
            </>
          ) : (
            <>
              <Play size={16} />
              <span>Submit Solution</span>
            </>
          )}
        </button>
      )}

      {/* Results */}
      {result && (
        <div className="space-y-3">
          {/* Score banner */}
          <div
            className={cn(
              "flex items-center gap-3 rounded-lg border px-4 py-3",
              result.isCorrect
                ? "border-green-500/30 bg-green-500/10"
                : "border-red-500/30 bg-red-500/10"
            )}
          >
            {result.isCorrect ? (
              <CheckCircle2 size={20} className="text-green-500" />
            ) : (
              <XCircle size={20} className="text-red-500" />
            )}
            <div>
              <p className="text-sm font-semibold text-foreground">
                Score: {result.score}/100
              </p>
              <p className="text-xs text-muted-foreground">
                {result.isCorrect ? "Solution accepted" : "Needs improvement"}
              </p>
            </div>
          </div>

          {/* Feedback */}
          <div className="glass rounded-lg p-4">
            <h4 className="mb-2 text-sm font-semibold text-foreground">Feedback</h4>
            <MarkdownRenderer content={result.feedback} className="text-sm" />
          </div>

          {/* Suggestions */}
          {result.hints.length > 0 && (
            <div className="rounded-lg border border-blue-500/30 bg-blue-500/10 p-4">
              <div className="mb-2 flex items-center gap-2">
                <Lightbulb size={16} className="text-blue-500" />
                <h4 className="text-sm font-semibold text-foreground">Suggestions</h4>
              </div>
              <ul className="ml-5 list-disc space-y-1 text-sm text-foreground/80">
                {result.hints.map((h, i) => (
                  <li key={i}>{h}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
