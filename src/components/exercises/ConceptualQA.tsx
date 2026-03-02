import { useState, useCallback } from "react";
import { Send, Loader2, AlertTriangle, Lightbulb, CheckCircle2, XCircle } from "lucide-react";
import { evaluateResponse } from "@/lib/tauri-commands";
import { MarkdownRenderer } from "@/components/learning/MarkdownRenderer";
import type { Exercise } from "@/types/exercises";
import type { EvaluateResponseResult } from "@/types/ai";
import { cn } from "@/lib/utils";

interface ConceptualQAProps {
  exercise: Exercise;
  onComplete: (score: number) => void;
}

export function ConceptualQA({ exercise, onComplete }: ConceptualQAProps) {
  const [answer, setAnswer] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [result, setResult] = useState<EvaluateResponseResult | null>(null);
  const [showHints, setShowHints] = useState(false);
  const [hintsRevealed, setHintsRevealed] = useState(0);

  const handleSubmit = useCallback(async () => {
    if (!answer.trim() || isSubmitting) return;

    setIsSubmitting(true);
    try {
      const evaluation = await evaluateResponse({
        exercisePrompt: exercise.prompt,
        learnerResponse: answer,
        rubric: "Evaluate the learner's understanding of the concept. Check for accuracy, completeness, and depth of understanding.",
      });
      setResult(evaluation);
      onComplete(evaluation.score);
    } catch (err) {
      console.error("Failed to evaluate response:", err);
    } finally {
      setIsSubmitting(false);
    }
  }, [answer, isSubmitting, exercise, onComplete]);

  const revealNextHint = useCallback(() => {
    setShowHints(true);
    setHintsRevealed((prev) => Math.min(prev + 1, exercise.hints.length));
  }, [exercise.hints.length]);

  return (
    <div className="space-y-4">
      {/* Question */}
      <div className="glass rounded-lg p-5">
        <MarkdownRenderer content={exercise.prompt} />
      </div>

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

      {/* Answer area */}
      <div>
        <label className="mb-1.5 block text-sm font-medium text-foreground">
          Your Answer
        </label>
        <textarea
          value={answer}
          onChange={(e) => setAnswer(e.target.value)}
          placeholder="Type your answer here..."
          rows={6}
          disabled={result !== null}
          className={cn(
            "w-full resize-y rounded-lg border border-border bg-background px-4 py-3 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring",
            result && "opacity-70"
          )}
        />
      </div>

      {/* Submit button */}
      {!result && (
        <button
          onClick={handleSubmit}
          disabled={!answer.trim() || isSubmitting}
          className={cn(
            "flex items-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors",
            answer.trim() && !isSubmitting
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
              <Send size={16} />
              <span>Submit Answer</span>
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
                {result.isCorrect ? "Well done" : "Keep practicing"}
              </p>
            </div>
          </div>

          {/* Feedback */}
          <div className="glass rounded-lg p-4">
            <h4 className="mb-2 text-sm font-semibold text-foreground">Feedback</h4>
            <MarkdownRenderer content={result.feedback} className="text-sm" />
          </div>

          {/* Misconceptions */}
          {result.misconceptions.length > 0 && (
            <div className="rounded-lg border border-orange-500/30 bg-orange-500/10 p-4">
              <div className="mb-2 flex items-center gap-2">
                <AlertTriangle size={16} className="text-orange-500" />
                <h4 className="text-sm font-semibold text-foreground">Misconceptions to Address</h4>
              </div>
              <ul className="ml-5 list-disc space-y-1 text-sm text-foreground/80">
                {result.misconceptions.map((m, i) => (
                  <li key={i}>{m}</li>
                ))}
              </ul>
            </div>
          )}

          {/* Additional hints */}
          {result.hints.length > 0 && (
            <div className="rounded-lg border border-blue-500/30 bg-blue-500/10 p-4">
              <div className="mb-2 flex items-center gap-2">
                <Lightbulb size={16} className="text-blue-500" />
                <h4 className="text-sm font-semibold text-foreground">Suggestions for Improvement</h4>
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
