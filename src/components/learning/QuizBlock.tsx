import { useState, useMemo, useRef } from "react";
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
  /**
   * Phase 4 (04-05) — optional block-completion signal. Fires once the
   * Submit action resolves (regardless of pass/fail per D-08 — daily
   * challenge completion is engagement-driven). ModuleView callers pass
   * nothing and the prop has zero behavioral effect.
   */
  onComplete?: () => void;
}

function fisherYates<T>(arr: T[]): T[] {
  const a = [...arr];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

export function QuizBlock({ block, moduleId, trackId, onComplete }: QuizBlockProps) {
  const submitQuizAction = useLearningStore((s) => s.submitQuiz);
  const moduleProgress = useLearningStore((s) => s.moduleProgress);
  // Phase 10 Plan 03 (D-09) — suppress unlock celebration copy in free mode.
  // currentTrack is optional in the store (may be null before selectTrack fires).
  // undefined browseMode defaults to linear per D-01.
  const currentTrackBrowseMode = useLearningStore(
    (s) => (s as { currentTrack?: { browseMode?: "linear" | "free" } | null }).currentTrack?.browseMode,
  );
  const isLinearMode = currentTrackBrowseMode !== "free";

  // Look up persisted mastery for this module — set on a prior successful
  // submit_quiz call. >= 0.7 means the learner already cleared this module.
  const priorMastery =
    moduleProgress.find((p) => p.moduleId === moduleId)?.masteryLevel ?? 0;
  const alreadyPassed = priorMastery >= 0.7;
  const [retakeRequested, setRetakeRequested] = useState(false);
  const showAlreadyPassedGate = alreadyPassed && !retakeRequested;

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
  // Phase 4 (04-05) — guard against double-fire on retake / re-render.
  // Daily-challenge completion is a one-shot signal per mount.
  const completionFiredRef = useRef(false);

  const fireCompletionOnce = () => {
    if (!completionFiredRef.current && onComplete) {
      completionFiredRef.current = true;
      onComplete();
    }
  };

  // Empty quiz guard
  if (questions.length === 0) {
    return (
      <div data-testid="quiz-empty" className="glass rounded-md p-6 my-4">
        This quiz has no questions.
      </div>
    );
  }

  // Already-passed gate: prior mastery >= 0.7 and the learner hasn't asked
  // to retake this session.
  if (showAlreadyPassedGate) {
    return (
      <div
        className="glass rounded-md p-6 my-4 space-y-3"
        data-testid="quiz-already-passed"
      >
        <div className="flex items-center gap-2">
          <span className="inline-flex h-7 w-7 items-center justify-center rounded-full bg-green-500/20 text-green-700 dark:text-green-400 text-sm font-semibold">
            ✓
          </span>
          <h3 className="text-lg font-semibold text-foreground m-0">
            You've passed this quiz
          </h3>
        </div>
        <p className="text-sm text-muted-foreground m-0">
          Mastery: <strong className="text-foreground">{Math.round(priorMastery * 100)}%</strong>
          {/* Phase 10 Plan 03 (D-09): suppress unlock phrasing in free mode */}
          {isLinearMode && ". Your progress is saved — the next module is unlocked."}
          {!isLinearMode && ". Your progress is saved."}
        </p>
        <button
          type="button"
          className="glass-strong px-4 py-2 rounded-md text-sm font-medium hover:opacity-90 transition-opacity"
          data-testid="quiz-retake-btn"
          onClick={() => {
            setRetakeRequested(true);
            setAttempt((a) => a + 1);
          }}
        >
          Take it again
        </button>
      </div>
    );
  }

  // Review screen
  if (result !== null) {
    return (
      <ReviewScreen
        questions={questions}
        result={result}
        isLinearMode={isLinearMode}
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
    // Per-question reveal: first selection is FINAL — answers are locked once
    // recorded. Subsequent clicks on other options are ignored. The reveal
    // happens immediately on selection (correctness + explanation surface
    // below the options).
    setAnswers((prev) => {
      if (prev[q.id]) return prev; // already locked
      return { ...prev, [q.id]: optId };
    });
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
        // Phase 4 (04-05) — D-08 engagement-driven: fire regardless of r.passed.
        fireCompletionOnce();
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
          newlyIssuedAchievements: [],
        });
        fireCompletionOnce();
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
        newlyIssuedAchievements: [],
      });
      fireCompletionOnce();
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

      {/* Options — role="option" for testability (listbox item semantics).
          Once an answer is recorded for q, options are locked: the chosen
          option highlights as correct/incorrect and the correct option (if
          different) is also highlighted in green. */}
      {(() => {
        const chosen = answers[q.id];
        const revealed = !!chosen;
        return (
          <>
            <ul role="listbox" className="space-y-2 mb-4">
              {q.options.map((opt) => {
                const isChosen = chosen === opt.id;
                const isCorrect = opt.id === q.correctOptionId;
                const showAsCorrect = revealed && isCorrect;
                const showAsWrong = revealed && isChosen && !isCorrect;
                return (
                  <li
                    key={opt.id}
                    role="option"
                    aria-selected={isChosen}
                  >
                    <button
                      onClick={() => selectOption(opt.id)}
                      disabled={revealed}
                      className={cn(
                        "w-full text-left p-3 rounded border transition-colors",
                        !revealed && !isChosen &&
                          "border-border glass hover:border-foreground/40",
                        !revealed && isChosen &&
                          "border-blue-400 bg-blue-400/10 text-foreground",
                        showAsCorrect &&
                          "border-green-500 bg-green-500/10 text-foreground",
                        showAsWrong &&
                          "border-red-500 bg-red-500/10 text-foreground",
                        revealed && !isChosen && !isCorrect &&
                          "border-border glass opacity-60",
                        revealed && "cursor-default",
                      )}
                      data-testid={`option-${opt.id}`}
                    >
                      <span className="flex items-center justify-between gap-2">
                        <span>{opt.text}</span>
                        {showAsCorrect && (
                          <span aria-hidden className="text-green-600 dark:text-green-400 font-semibold">✓</span>
                        )}
                        {showAsWrong && (
                          <span aria-hidden className="text-red-600 dark:text-red-400 font-semibold">✗</span>
                        )}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>

            {revealed && (() => {
              const isCorrect = chosen === q.correctOptionId;
              const correctOpt = q.options.find((o) => o.id === q.correctOptionId);
              return (
                <div
                  data-testid="answer-feedback"
                  className={cn(
                    "rounded-md border p-3 mb-4 text-sm",
                    isCorrect
                      ? "border-green-500/40 bg-green-500/5"
                      : "border-red-500/40 bg-red-500/5",
                  )}
                >
                  <p className="font-semibold mb-1 text-foreground">
                    {isCorrect ? "Correct" : "Incorrect"}
                  </p>
                  {!isCorrect && correctOpt && (
                    <p className="mb-1 text-foreground/90">
                      Correct answer: <strong>{correctOpt.text}</strong>
                    </p>
                  )}
                  {q.explanation && (
                    <p className="text-foreground/80 m-0">{q.explanation}</p>
                  )}
                </div>
              );
            })()}
          </>
        );
      })()}

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
  isLinearMode,
}: {
  questions: QuizQuestion[];
  result: SubmitQuizResult;
  onRetake: () => void;
  isLinearMode: boolean;
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

      {/* Phase 10 Plan 03 (D-09): unlock celebration shown only in linear mode.
          In free mode the sequential-unlock narrative doesn't apply — modules
          were already openable. cert/mastery gates remain intact either way. */}
      {result.passed && isLinearMode && (
        <p className="text-sm text-foreground/70 mt-4">
          Module mastered — downstream modules unlocked.
        </p>
      )}
    </div>
  );
}
