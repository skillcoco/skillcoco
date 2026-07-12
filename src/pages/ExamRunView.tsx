// Phase 19 plan 19-06 (EXAM-02/EXAM-04) — dedicated exam route.
//
// /track/:trackId/exam/:blockId renders this page as a fresh mount: the
// pre-exam start screen, the in-run chrome (header + ExamTimer + step chip
// + LabBlock examMode=true), and the results screen are internal view
// states of one component — not separate routes. A fresh mount means no
// TutorSidebar toggle state can leak in (T-19-09 / RESEARCH Pitfall 4
// sidestepped by construction); this page never imports ModuleView's tab
// chrome.
//
// The target blockId is validated against examBlocksForTrack (backend-
// resolved exam flags) before anything renders — a learner cannot launch a
// non-exam block in exam mode by editing the URL (T-19-12, D-02 fail-
// closed). The countdown is display-only against the backend-persisted
// deadline_at; the backend independently recomputes timeout on submit
// (T-19-01). submitAttempt forwards NO verdicts (D-15).

import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AlertTriangle, Loader2 } from "lucide-react";
import {
  examAttemptHistory,
  examBlocksForTrack,
  getModuleBlocks,
  getOrCreateProfile,
} from "@/lib/tauri-commands";
import { useExamStore } from "@/stores/useExamStore";
import { useLabStore } from "@/stores/useLabStore";
import { LabBlock } from "@/components/labs/LabBlock";
import { ExamTimer } from "@/components/labs/exam/ExamTimer";
import { ExamResultsPanel } from "@/components/labs/exam/ExamResultsPanel";
import type {
  ExamAttemptHistoryResult,
  ExamAttemptResult,
  ExamAttemptStartResult,
  LabBlockPayload,
  LabSpec,
  ModuleBlock,
} from "@/types/learning";

// 19-02 parses `exam:` frontmatter into LabSpec.exam on the Rust side; the
// TS LabSpec type predates it. Local extension keeps this page's read
// optional and additive (defaults locked by 19-02: 30 min / 70%).
type ExamLabSpec = LabSpec & {
  exam?: { timeLimitMinutes?: number; passThresholdPct?: number } | null;
};

const DEFAULT_TIME_LIMIT_MINUTES = 30;
const DEFAULT_PASS_THRESHOLD_PCT = 70;

type ExamViewState = "loading" | "start" | "running" | "results";

export function ExamRunView() {
  const { trackId, blockId } = useParams<{
    trackId: string;
    blockId: string;
  }>();
  const navigate = useNavigate();

  const startAttempt = useExamStore((s) => s.startAttempt);
  const submitAttempt = useExamStore((s) => s.submitAttempt);
  const getAttempt = useExamStore((s) => s.getAttempt);

  const [view, setView] = useState<ExamViewState>("loading");
  const [block, setBlock] = useState<ModuleBlock | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [startError, setStartError] = useState<string | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [startResult, setStartResult] =
    useState<ExamAttemptStartResult | null>(null);
  const [result, setResult] = useState<ExamAttemptResult | null>(null);
  const [history, setHistory] = useState<ExamAttemptHistoryResult | null>(
    null,
  );
  const [grading, setGrading] = useState(false);
  const [confirmSubmit, setConfirmSubmit] = useState(false);
  const [timedOut, setTimedOut] = useState(false);

  // D-11 blind scoring hides verdicts, but the step POSITION chip is
  // allowed — read the live progress row the lab session maintains.
  const blockProgress = useLabStore((s) =>
    blockId ? s.progress.get(blockId) : undefined,
  );

  // Resolve the target block, fail-closed: the blockId must appear in
  // examBlocksForTrack's backend-resolved exam-flag list (T-19-12).
  useEffect(() => {
    if (!trackId || !blockId) return;
    let cancelled = false;
    (async () => {
      try {
        const refs = await examBlocksForTrack({ trackId });
        const ref = refs.find((r) => r.blockId === blockId);
        if (!ref) {
          if (!cancelled) {
            setLoadError(
              "Couldn't start the exam. This block is not an exam. Try again from the course page.",
            );
          }
          return;
        }
        const blocks = await getModuleBlocks(ref.moduleId);
        const target = blocks.find((b) => b.id === blockId) ?? null;
        if (cancelled) return;
        if (!target) {
          setLoadError(
            "Couldn't start the exam. The exam block could not be loaded. Try again from the course page.",
          );
          return;
        }
        setBlock(target);
        setView("start");
      } catch (err) {
        if (!cancelled) {
          setLoadError(
            `Couldn't start the exam. ${err instanceof Error ? err.message : String(err)}. Try again from the course page.`,
          );
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [trackId, blockId]);

  const spec = useMemo<ExamLabSpec | null>(() => {
    if (!block) return null;
    try {
      const payload = JSON.parse(block.payloadJson) as LabBlockPayload;
      return (payload?.spec as ExamLabSpec) ?? null;
    } catch {
      return null;
    }
  }, [block]);

  const timeLimitMinutes =
    spec?.exam?.timeLimitMinutes ?? DEFAULT_TIME_LIMIT_MINUTES;
  const passThresholdPct =
    startResult?.passThresholdPct ??
    spec?.exam?.passThresholdPct ??
    DEFAULT_PASS_THRESHOLD_PCT;
  const totalSteps = startResult?.totalSteps ?? spec?.steps.length ?? 0;
  const currentStep = blockProgress?.currentStep ?? 0;
  const completedCount = new Set(blockProgress?.completedStepIds ?? []).size;

  const handleBegin = useCallback(async () => {
    if (!trackId || !blockId || !block) return;
    setStartError(null);
    try {
      const profile = await getOrCreateProfile();
      const started = await startAttempt(
        profile.id,
        block.moduleId,
        blockId,
        trackId,
      );
      setStartResult(started);
      setTimedOut(false);
      setResult(null);
      setView("running");
    } catch (err) {
      setStartError(
        `Couldn't start the exam. ${err instanceof Error ? err.message : String(err)}. Try again from the course page.`,
      );
    }
  }, [trackId, blockId, block, startAttempt]);

  const doSubmit = useCallback(
    async (auto: boolean) => {
      if (!startResult) return;
      setConfirmSubmit(false);
      setGrading(true);
      setSubmitError(null);
      try {
        // D-15 — no verdicts cross this boundary; currentStep is display-
        // only telemetry. The backend derives every verdict itself.
        await submitAttempt(startResult.attemptId, currentStep);
        const finalResult = await getAttempt(startResult.attemptId);
        setResult(finalResult);
        setTimedOut(auto || finalResult.status === "timed_out_partial");
        setView("results");

        // D-06 — best-attempt history note is supplementary; a fetch
        // failure must NEVER block or break the results screen (own
        // try/catch, own state, results screen already committed above).
        try {
          const historyResult = await examAttemptHistory({
            attemptId: startResult.attemptId,
          });
          setHistory(historyResult);
        } catch (historyErr) {
          // WR-02 — this failure path is otherwise indistinguishable from
          // the legitimate "only attempt" case (both render no note). A
          // dev-visible warning at least surfaces a genuine backend
          // regression during development/QA instead of degrading
          // silently in production and in tests.
          console.warn(
            "examAttemptHistory failed; suppressing the history note",
            historyErr,
          );
          setHistory(null);
        }
      } catch (err) {
        setSubmitError(
          `Couldn't submit your exam. ${err instanceof Error ? err.message : String(err)}. Your progress is saved — try submitting again.`,
        );
      } finally {
        setGrading(false);
      }
    },
    [startResult, currentStep, submitAttempt, getAttempt],
  );

  // D-04 — auto-submit on timeout. ExamTimer fires onExpire exactly once;
  // the backend independently recomputes the deadline on submit.
  const handleExpire = useCallback(() => {
    void doSubmit(true);
  }, [doSubmit]);

  if (!trackId || !blockId) {
    return null;
  }

  if (loadError) {
    return (
      <div className="mx-auto max-w-3xl pb-12">
        <div
          role="alert"
          className="glass-strong rounded-xl border border-destructive/40 p-6 text-sm text-destructive"
        >
          {loadError}
        </div>
      </div>
    );
  }

  if (view === "loading" || !block) {
    return (
      <div className="flex h-64 items-center justify-center gap-2 text-sm text-muted-foreground">
        <Loader2 size={16} className="animate-spin" />
        Loading exam...
      </div>
    );
  }

  // WR-03 — the block loaded but its payload has no parseable spec
  // (payload JSON invalid, or authored via params_json.labMd only).
  // Render an actionable error instead of spinning forever.
  if (!spec) {
    return (
      <div className="mx-auto max-w-3xl pb-12">
        <div
          role="alert"
          className="glass-strong rounded-xl border border-destructive/40 p-6 text-sm text-destructive"
        >
          Couldn&apos;t start the exam. The exam content could not be loaded.
          Try again from the course page.
        </div>
      </div>
    );
  }

  // ── Pre-exam start screen ──
  if (view === "start") {
    return (
      <div className="mx-auto max-w-3xl space-y-6 pb-12">
        <div className="glass-strong space-y-6 rounded-xl border border-border p-6">
          <div>
            <h1 className="text-lg font-semibold text-foreground">
              Exam: {spec.title}
            </h1>
            <p className="mt-2 text-sm text-foreground">
              Time limit: {timeLimitMinutes} minutes
            </p>
            <p className="text-sm text-foreground">
              Pass threshold: {passThresholdPct}%
            </p>
          </div>

          <div>
            <h2 className="text-sm font-semibold text-foreground">
              Before you start
            </h2>
            <ul className="mt-2 space-y-1.5 text-sm text-muted-foreground">
              <li>Hints and the AI tutor are disabled during the exam.</li>
              <li>
                You'll see your step position, but not pass/fail status, until
                you finish.
              </li>
              <li>
                The timer cannot be paused. If time runs out, your exam
                auto-submits with whatever steps are complete.
              </li>
            </ul>
          </div>

          {startError && (
            <div
              role="alert"
              className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
            >
              {startError}
            </div>
          )}

          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={() => navigate(`/track/${trackId}`)}
              className="rounded-lg border border-border px-4 py-2 text-xs font-medium text-foreground transition-colors hover:bg-accent"
            >
              Not now
            </button>
            <button
              type="button"
              onClick={handleBegin}
              data-testid="exam-begin-button"
              className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
            >
              Begin exam
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ── Results screen ──
  if (view === "results" && result) {
    return (
      <div className="mx-auto max-w-3xl space-y-4 pb-12">
        {timedOut && (
          <div
            role="status"
            className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-500"
          >
            Time's up &mdash; your exam has been submitted automatically.
          </div>
        )}
        <ExamResultsPanel
          result={result}
          passThresholdPct={passThresholdPct}
          history={
            history && history.totalAttempts > 1
              ? {
                  ...history,
                  bestAttemptDate: new Date(
                    history.bestAttemptDate,
                  ).toLocaleDateString(),
                }
              : undefined
          }
          onRetake={() => {
            setStartResult(null);
            setResult(null);
            setHistory(null);
            setView("start");
          }}
          onBackToCourse={() => navigate(`/track/${trackId}`)}
        />
      </div>
    );
  }

  // ── In-run chrome ──
  return (
    <div className="flex h-full flex-col gap-3 pb-4">
      <div className="flex items-center justify-between gap-3">
        <h1 className="text-lg font-semibold text-foreground">
          Exam: {spec.title}
        </h1>
        <div className="flex items-center gap-3">
          <div className="glass flex items-center gap-2 rounded-lg px-4 py-2.5">
            <span className="tabular-nums text-sm font-semibold text-foreground">
              Step {Math.min(currentStep + 1, totalSteps)} of {totalSteps}
            </span>
          </div>
          {startResult && (
            <ExamTimer
              deadlineAt={startResult.deadlineAt}
              onExpire={handleExpire}
            />
          )}
          <button
            type="button"
            onClick={() => setConfirmSubmit(true)}
            disabled={grading}
            data-testid="exam-submit-button"
            className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {grading ? (
              <>
                <Loader2 size={13} className="animate-spin" />
                Grading exam&hellip;
              </>
            ) : (
              "Submit exam"
            )}
          </button>
        </div>
      </div>

      {submitError && (
        <div
          role="alert"
          className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
        >
          {submitError}
        </div>
      )}

      {confirmSubmit && (
        <div
          role="alertdialog"
          aria-label="Submit now?"
          className="glass rounded-lg border border-amber-500/30 p-4"
        >
          <div className="flex items-start gap-2">
            <AlertTriangle size={16} className="mt-0.5 shrink-0 text-amber-500" />
            <div className="text-sm text-foreground">
              <p className="font-semibold">Submit now?</p>
              <p className="mt-1 text-muted-foreground">
                You have {Math.max(0, totalSteps - completedCount)} unattempted
                steps. They'll be scored as not completed. This can't be undone
                for this attempt &mdash; but you can always retake.
              </p>
            </div>
          </div>
          <div className="mt-3 flex justify-end gap-2">
            <button
              type="button"
              onClick={() => setConfirmSubmit(false)}
              className="rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent"
            >
              Keep working
            </button>
            <button
              type="button"
              onClick={() => void doSubmit(false)}
              data-testid="exam-confirm-submit-button"
              className="rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
            >
              Submit exam
            </button>
          </div>
        </div>
      )}

      <div className="min-h-0 flex-1">
        <LabBlock block={block} trackId={trackId} examMode={true} />
      </div>
    </div>
  );
}
