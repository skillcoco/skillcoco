import { useState, useMemo } from "react";
import type {
  ModuleBlock,
  QuizPayload,
  QuizQuestion,
  SubmitQuizResult,
} from "@/types/learning";
import { useLearningStore } from "@/stores/useLearningStore";
import { cn } from "@/lib/utils";

interface QuizBlockProps {
  block: ModuleBlock;
  moduleId: string;
  trackId?: string;
}

function fisherYates<T>(arr: T[]): T[] {
  const a = [...arr];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

export function QuizBlock({ block, moduleId, trackId }: QuizBlockProps) {
  const submitQuizAction = useLearningStore((s) => s.submitQuiz);

  // Bump on retake to trigger re-shuffle via useMemo
  const [attempt, setAttempt] = useState(0);

  const questions: QuizQuestion[] = useMemo(() => {
    let payload: QuizPayload;
    try {
      payload = JSON.parse(block.payloadJson) as QuizPayload;
    } catch {
      payload = { questions: [] };
    }
    // Shuffle options on each attempt; correct_option_id stays the same (id-based scoring)
    return payload.questions.map((q) => ({ ...q, options: fisherYates(q.options) }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [block.payloadJson, attempt]);

  const [current, setCurrent] = useState(0);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [flags, setFlags] = useState<Set<string>>(new Set());
  const [result, setResult] = useState<SubmitQuizResult | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // Empty quiz guard
  if (questions.length === 0) {
    return (
      <div data-testid="quiz-empty" className="glass rounded-md p-6 my-4">
        This quiz has no questions.
      </div>
    );
  }

  // Review screen
  if (result !== null) {
    return (
      <ReviewScreen
        questions={questions}
        result={result}
        onRetake={() => {
          setResult(null);
          setAnswers({});
          setFlags(new Set());
          setCurrent(0);
          setAttempt((a) => a + 1);
        }}
      />
    );
  }

  const q = questions[current];
  const allAnswered = questions.every((qq) => !!answers[qq.id]);
  const isFlagged = flags.has(q.id);

  function selectOption(optId: string) {
    setAnswers((prev) => ({ ...prev, [q.id]: optId }));
  }

  function toggleFlag() {
    setFlags((prev) => {
      const next = new Set(prev);
      if (next.has(q.id)) {
        next.delete(q.id);
      } else {
        next.add(q.id);
      }
      return next;
    });
  }

  async function handleSubmit() {
    setSubmitting(true);
    try {
      // Build answer list
      const answerList = Object.entries(answers).map(([questionId, selectedOptionId]) => ({
        questionId,
        selectedOptionId,
      }));

      // If trackId available, call the store action (real IPC)
      if (trackId) {
        const r = await submitQuizAction({
          moduleId,
          trackId,
          blockId: block.id,
          answers: answerList,
        });
        setResult(r);
      } else {
        // Fallback: compute review locally (for dev/test without trackId)
        const review = questions.map((q) => {
          const selected = answers[q.id] ?? "";
          return {
            questionId: q.id,
            stem: q.stem,
            learnerOptionId: selected,
            correctOptionId: q.correctOptionId,
            isCorrect: selected === q.correctOptionId,
            explanation: q.explanation,
          };
        });
        const correct = review.filter((r) => r.isCorrect).length;
        const scorePercent = questions.length > 0 ? (correct / questions.length) * 100 : 0;
        setResult({
          scorePercent,
          passed: scorePercent >= 70,
          masteryLevel: 0,
          moduleCompleted: false,
          newlyUnlockedModuleIds: [],
          cardsCreated: 0,
          review,
        });
      }
    } catch {
      // On IPC error: compute review locally so user can see their answers
      const review = questions.map((q) => {
        const selected = answers[q.id] ?? "";
        return {
          questionId: q.id,
          stem: q.stem,
          learnerOptionId: selected,
          correctOptionId: q.correctOptionId,
          isCorrect: selected === q.correctOptionId,
          explanation: q.explanation,
        };
      });
      const correct = review.filter((r) => r.isCorrect).length;
      const scorePercent = questions.length > 0 ? (correct / questions.length) * 100 : 0;
      setResult({
        scorePercent,
        passed: scorePercent >= 70,
        masteryLevel: 0,
        moduleCompleted: false,
        newlyUnlockedModuleIds: [],
        cardsCreated: 0,
        review,
      });
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="glass rounded-lg p-6 my-6" data-testid="quiz-block">
      {/* Progress + flag */}
      <div className="flex justify-between items-center mb-4">
        <span
          className="text-sm text-foreground/60"
          data-testid="quiz-progress"
        >
          {current + 1} / {questions.length}
        </span>
        <button
          onClick={toggleFlag}
          aria-pressed={isFlagged}
          className={cn(
            "text-xs px-3 py-1 rounded border transition-colors",
            isFlagged
              ? "border-amber-400 bg-amber-400/10 text-amber-600 dark:text-amber-400"
              : "border-border text-foreground/60 hover:border-foreground/40"
          )}
          data-testid="flag-toggle"
        >
          Flag for review
        </button>
      </div>

      {/* Question stem */}
      <h3 className="text-lg font-medium mb-4">{q.stem}</h3>

      {/* Options — role="option" for testability (listbox item semantics) */}
      <ul role="listbox" className="space-y-2 mb-6">
        {q.options.map((opt) => (
          <li key={opt.id} role="option" aria-selected={answers[q.id] === opt.id}>
            <button
              onClick={() => selectOption(opt.id)}
              className={cn(
                "w-full text-left p-3 rounded border transition-colors",
                answers[q.id] === opt.id
                  ? "border-blue-400 bg-blue-400/10 text-foreground"
                  : "border-border glass hover:border-foreground/40"
              )}
              data-testid={`option-${opt.id}`}
            >
              {opt.text}
            </button>
          </li>
        ))}
      </ul>

      {/* Navigation */}
      <div className="flex justify-between">
        <button
          disabled={current === 0}
          onClick={() => setCurrent((c) => c - 1)}
          className="glass-strong px-4 py-2 rounded disabled:opacity-40"
        >
          Prev
        </button>
        {current < questions.length - 1 ? (
          <button
            onClick={() => setCurrent((c) => c + 1)}
            className="glass-strong px-4 py-2 rounded"
          >
            Next
          </button>
        ) : (
          <button
            disabled={!allAnswered || submitting}
            onClick={handleSubmit}
            className="glass-strong px-4 py-2 rounded disabled:opacity-40"
            data-testid="quiz-submit"
          >
            {submitting ? "Submitting..." : "Submit"}
          </button>
        )}
      </div>
    </div>
  );
}

function ReviewScreen({
  questions,
  result,
  onRetake,
}: {
  questions: QuizQuestion[];
  result: SubmitQuizResult;
  onRetake: () => void;
}) {
  const reviewByQ = new Map(result.review.map((r) => [r.questionId, r]));

  return (
    <div className="glass rounded-lg p-6 my-6" data-testid="quiz-review">
      {/* Score header */}
      <div className="flex justify-between items-center mb-6">
        <div className="flex items-center gap-3">
          {result.passed ? (
            <span
              data-testid="passed-badge"
              className="px-3 py-1 rounded-full text-sm font-semibold bg-green-500/20 text-green-700 dark:text-green-400"
            >
              Passed
            </span>
          ) : (
            <span
              data-testid="failed-badge"
              className="px-3 py-1 rounded-full text-sm font-semibold bg-red-500/20 text-red-700 dark:text-red-400"
            >
              Not yet
            </span>
          )}
          <span className="text-lg font-semibold">
            {Math.round(result.scorePercent)}%
          </span>
        </div>
        {!result.passed && (
          <button
            onClick={onRetake}
            className="glass-strong px-4 py-2 rounded"
            data-testid="retake-btn"
          >
            Retake
          </button>
        )}
      </div>

      {/* Per-question review rows */}
      <div className="space-y-4">
        {questions.map((q) => {
          const r = reviewByQ.get(q.id);
          if (!r) return null;
          const learnerOpt = q.options.find((o) => o.id === r.learnerOptionId);
          const correctOpt = q.options.find((o) => o.id === r.correctOptionId);
          return (
            <div
              key={q.id}
              className={cn(
                "rounded p-4 border-l-4",
                r.isCorrect
                  ? "glass border-green-400"
                  : "glass border-red-400"
              )}
              data-testid="review-row"
              data-correct={r.isCorrect}
            >
              <p className="font-medium mb-2">{q.stem}</p>
              <p className="text-sm text-foreground/80">
                Your answer: {learnerOpt?.text ?? "(not answered)"}
              </p>
              {!r.isCorrect && correctOpt && (
                <p className="text-sm text-foreground/80">
                  Correct: {correctOpt.text}
                </p>
              )}
              {r.explanation && (
                <p className="text-xs text-foreground/70 mt-2">{r.explanation}</p>
              )}
            </div>
          );
        })}
      </div>

      {result.passed && (
        <p className="text-sm text-foreground/70 mt-4">
          Module mastered — downstream modules unlocked.
        </p>
      )}
    </div>
  );
}
