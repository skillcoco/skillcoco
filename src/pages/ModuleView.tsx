import { useState, useEffect, useMemo, useRef, useCallback } from "react";
import { useParams, Link } from "react-router-dom";
import { ArrowLeft, MessageCircle, Loader2, ChevronLeft, ChevronRight, RefreshCw } from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { BlockRenderer } from "@/components/learning/BlockRenderer";
import { ExerciseContainer } from "@/components/exercises/ExerciseContainer";
import { TutorSidebar } from "@/components/learning/TutorSidebar";
import { CourseSidebar } from "@/components/learning/CourseSidebar";
import { regenerateModule, generateModuleBlocks } from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";
import type { PathModule } from "@/types/learning";

type Tab = "lessons" | "quiz" | "practice";

export function ModuleView() {
  const { trackId, moduleId } = useParams<{ trackId: string; moduleId: string }>();

  // Store selectors
  const currentTrack = useLearningStore((s) => s.currentTrack);
  const currentPath = useLearningStore((s) => s.currentPath);
  const moduleProgress = useLearningStore((s) => s.moduleProgress);
  const currentLessonId = useLearningStore((s) => s.currentLessonId);
  const blocks = useLearningStore((s) =>
    moduleId ? (s.moduleBlocks.get(moduleId) ?? []) : [],
  );
  const lessonCompletions = useLearningStore((s) =>
    moduleId ? s.lessonCompletions.get(moduleId) : undefined,
  );
  const loadModuleBlocks = useLearningStore((s) => s.loadModuleBlocks);
  const loadLessonCompletions = useLearningStore((s) => s.loadLessonCompletions);
  const selectTrack = useLearningStore((s) => s.selectTrack);
  const setCurrentLesson = useLearningStore((s) => s.setCurrentLesson);

  const [tab, setTab] = useState<Tab>("lessons");
  const [tutorOpen, setTutorOpen] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [kickoffError, setKickoffError] = useState<string | null>(null);
  const [kickingOff, setKickingOff] = useState(false);

  // cancelRef: guards against async operations after unmount/module change
  const cancelRef = useRef(false);
  // scrollRef: scrollable lesson pane; we reset to top whenever the active
  // lesson changes so a sidebar click lands at the lesson's title, not at
  // wherever the user was reading in the previous lesson.
  const scrollRef = useRef<HTMLDivElement>(null);
  // kickoffRef: tracks which moduleId we've already triggered generation for in
  // this mount. Prevents duplicate PagePlanner calls if effects re-run.
  const kickoffRef = useRef<string | null>(null);

  // Load track if not already loaded
  useEffect(() => {
    if (trackId && (!currentTrack || currentTrack.id !== trackId)) {
      selectTrack(trackId);
    }
  }, [trackId, currentTrack, selectTrack]);

  // Parse current module from path
  const pathModules: PathModule[] = useMemo(() => {
    if (!currentPath) return [];
    try {
      return JSON.parse(currentPath.modulesJson || "[]") as PathModule[];
    } catch {
      return [];
    }
  }, [currentPath]);

  const currentModule = pathModules.find((m) => m.id === moduleId);
  const progress = moduleProgress.find((p) => p.moduleId === moduleId);

  // Mount: load existing blocks from cache.
  // W1 LOCK: module metadata (title, objectives) read from currentPath.modulesJson
  // already in the store from Phase 1 selectTrack. No new getModuleMeta IPC created.
  useEffect(() => {
    if (!moduleId) return;
    cancelRef.current = false;
    loadModuleBlocks(moduleId);
    // Restore per-lesson "Completed" checkmarks from DB across app restarts.
    loadLessonCompletions(moduleId);
    return () => {
      cancelRef.current = true;
    };
  }, [moduleId, loadModuleBlocks, loadLessonCompletions]);

  // Kickoff: if a module has zero blocks AND we have its metadata loaded, trigger
  // generate_module_blocks. The IPC short-circuits to cached blocks if any exist
  // (PACK-04), so this is safe even if the cache is stale.
  const kickoffGeneration = useCallback(async () => {
    if (!moduleId || !trackId || !currentModule) return;
    setKickingOff(true);
    setKickoffError(null);
    try {
      await generateModuleBlocks({
        moduleId,
        trackId,
        moduleTitle: currentModule.title,
        objectives: currentModule.objectives,
        learnerLevel: currentTrack?.domainModule ?? "beginner",
      });
      if (!cancelRef.current) {
        await loadModuleBlocks(moduleId);
      }
    } catch (err) {
      if (!cancelRef.current) {
        setKickoffError(String(err));
        // Allow retry — clear the kickoff guard so the user can re-trigger
        kickoffRef.current = null;
      }
    } finally {
      if (!cancelRef.current) {
        setKickingOff(false);
      }
    }
  }, [moduleId, trackId, currentModule, currentTrack, loadModuleBlocks]);

  useEffect(() => {
    if (!moduleId || !currentModule) return;
    if (blocks.length > 0) return;
    if (kickoffRef.current === moduleId) return;
    kickoffRef.current = moduleId;
    kickoffGeneration();
  }, [moduleId, currentModule, blocks.length, kickoffGeneration]);

  // Polling: while any block is pending/generating, poll every 3s
  // Stops when all blocks are ready or failed (or component unmounts)
  useEffect(() => {
    if (!moduleId) return;
    const anyPending = blocks.some(
      (b) => b.status === "pending" || b.status === "generating",
    );
    if (!anyPending) return;

    const interval = setInterval(() => {
      if (cancelRef.current) return;
      loadModuleBlocks(moduleId);
    }, 3000);

    return () => clearInterval(interval);
  }, [moduleId, blocks, loadModuleBlocks]);

  // Legacy detection: exactly 1 block, section type, paramsJson='{}'
  // This is the legacy wrap shim from 03-02 that wraps modules.content
  const isLegacy =
    blocks.length === 1 &&
    blocks[0].blockType === "section" &&
    blocks[0].paramsJson === "{}";

  // Separate blocks by type
  const sectionBlocks = useMemo(
    () => blocks.filter((b) => b.blockType === "section"),
    [blocks],
  );
  const quizBlock = useMemo(
    () => blocks.find((b) => b.blockType === "quiz"),
    [blocks],
  );
  // Phase 03.1 Plan 09 (GAP-01) — extract lab blocks for the Practice tab.
  const labBlocks = useMemo(
    () => blocks.filter((b) => b.blockType === "lab"),
    [blocks],
  );

  // Active lesson resolution: prefer currentLessonId from store; fall back to
  // the first section block. Index is computed against the section list so
  // Prev/Next can hop with bounds checks.
  const activeLessonIndex = useMemo(() => {
    if (sectionBlocks.length === 0) return -1;
    const idx = sectionBlocks.findIndex((b) => b.id === currentLessonId);
    return idx >= 0 ? idx : 0;
  }, [sectionBlocks, currentLessonId]);
  const activeLesson =
    activeLessonIndex >= 0 ? sectionBlocks[activeLessonIndex] : undefined;
  const isLastLesson =
    sectionBlocks.length > 0 &&
    activeLessonIndex === sectionBlocks.length - 1;

  // "What's next" CTA on the last lesson:
  // - If the module has been passed (mastery >= 0.7), point at the next
  //   unlocked module in the path (if any).
  // - Otherwise, link to the Quiz tab in this module.
  const moduleMastery = progress?.masteryLevel ?? 0;
  const moduleAlreadyPassed = moduleMastery >= 0.7;

  const nextModuleId = useMemo(() => {
    if (!moduleId) return null;
    const i = pathModules.findIndex((m) => m.id === moduleId);
    if (i < 0) return null;
    for (let j = i + 1; j < pathModules.length; j++) {
      const m = pathModules[j];
      const p = moduleProgress.find((mp) => mp.moduleId === m.id);
      if (p?.status !== "locked") return m.id;
    }
    return null;
  }, [pathModules, moduleProgress, moduleId]);

  const goToLesson = useCallback(
    (delta: number) => {
      if (activeLessonIndex < 0) return;
      const next = activeLessonIndex + delta;
      if (next < 0 || next >= sectionBlocks.length) return;
      setCurrentLesson(sectionBlocks[next].id);
    },
    [activeLessonIndex, sectionBlocks, setCurrentLesson],
  );

  // Scroll lesson pane to top when the active lesson changes (sidebar click,
  // Prev/Next, or default-to-first on mount). jsdom doesn't implement
  // Element.scrollTo so we feature-check before calling.
  useEffect(() => {
    if (!activeLesson) return;
    const el = scrollRef.current;
    if (el && typeof el.scrollTo === "function") {
      el.scrollTo({ top: 0, behavior: "auto" });
    } else if (el) {
      el.scrollTop = 0;
    }
  }, [activeLesson?.id]);

  // Calls regenerateModule IPC with PagePlanner-first atomicity (used by both
  // legacy banner and the user-facing "Regenerate" button in the header).
  const handleRegenerateLegacy = useCallback(async () => {
    if (!moduleId || !trackId || generating) return;
    setGenerating(true);
    try {
      await regenerateModule({ moduleId, trackId });
      if (!cancelRef.current) {
        await loadModuleBlocks(moduleId);
      }
    } finally {
      if (!cancelRef.current) {
        setGenerating(false);
      }
    }
  }, [moduleId, trackId, generating, loadModuleBlocks]);

  const handleRegenerateConfirmed = useCallback(() => {
    if (generating) return;
    const ok = window.confirm(
      "Regenerate this module? All current lessons, quiz, flashcards, and labs will be replaced. Your mastery progress is preserved.",
    );
    if (ok) {
      void handleRegenerateLegacy();
    }
  }, [generating, handleRegenerateLegacy]);

  const progressPercent =
    progress?.masteryLevel != null
      ? Math.round(progress.masteryLevel * 100)
      : 0;

  if (!currentPath || !currentModule) {
    return (
      <div className="flex h-64 items-center justify-center text-muted-foreground">
        <Loader2 size={20} className="mr-2 animate-spin" />
        <span>Loading module...</span>
      </div>
    );
  }

  return (
    <div className="-m-6 flex h-[calc(100vh-3rem)] overflow-hidden">
      {/* Course (LMS) sidebar — modules list, click to navigate */}
      {currentTrack && (
        <CourseSidebar
          track={currentTrack}
          modules={pathModules}
          progress={moduleProgress}
          currentModuleId={moduleId}
        />
      )}

      {/* Main scrollable content area */}
      <div
        ref={scrollRef}
        className={cn("flex-1 overflow-y-auto", tutorOpen && "lg:mr-96")}
      >
        <div className="mx-auto max-w-4xl space-y-6 p-6">

          {/* Header */}
          <div className="flex items-center gap-3">
            <Link
              to={`/track/${trackId}`}
              className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              <ArrowLeft size={18} />
            </Link>
            <div className="flex-1">
              <h1 className="text-2xl font-bold text-foreground">
                {currentModule.title}
              </h1>
              <p className="text-sm text-muted-foreground">
                {currentModule.estimatedMinutes} min estimated
              </p>
            </div>
          </div>

          {/* Progress bar */}
          <div className="glass rounded-lg p-4">
            <div className="mb-2 flex items-center justify-between text-sm">
              <span className="text-muted-foreground">Module Progress</span>
              <div className="flex items-center gap-3">
                <span className="font-medium text-foreground">{progressPercent}%</span>
                <button
                  type="button"
                  onClick={handleRegenerateConfirmed}
                  disabled={generating}
                  title="Regenerate this module — replaces lessons, quiz, flashcards, and labs"
                  aria-label="Regenerate module"
                  data-testid="regenerate-module-btn"
                  className={cn(
                    "flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium transition-colors",
                    generating
                      ? "cursor-not-allowed text-muted-foreground/50"
                      : "text-muted-foreground hover:bg-accent hover:text-foreground",
                  )}
                >
                  <RefreshCw size={12} className={generating ? "animate-spin" : undefined} />
                  <span>{generating ? "Regenerating..." : "Regenerate"}</span>
                </button>
              </div>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-secondary">
              <div
                className="h-full rounded-full bg-primary transition-all duration-500"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
          </div>

          {/* Legacy banner — shown only when module has a single synthetic section block */}
          {isLegacy && (
            <div
              className="glass rounded-lg p-4 border border-amber-400/30"
              data-testid="legacy-banner"
            >
              <p className="mb-3 text-sm text-foreground/80">
                This module was generated in an older format. Generate as 8-10 lessons?
              </p>
              <button
                onClick={handleRegenerateLegacy}
                disabled={generating}
                className={cn(
                  "glass-strong rounded-md px-4 py-2 text-sm font-medium transition-colors",
                  generating
                    ? "cursor-not-allowed opacity-60"
                    : "hover:bg-accent",
                )}
                data-testid="regenerate-as-lessons-btn"
              >
                {generating ? "Generating..." : "Generate as lessons"}
              </button>
            </div>
          )}

          {/* Tab navigation — Lessons | Quiz | Practice */}
          <div
            className="flex gap-1 rounded-lg border border-border p-1"
            data-testid="module-tabs"
          >
            <TabButton
              active={tab === "lessons"}
              onClick={() => setTab("lessons")}
              testid="tab-lessons"
            >
              Lessons
            </TabButton>
            <TabButton
              active={tab === "quiz"}
              onClick={() => setTab("quiz")}
              testid="tab-quiz"
            >
              Quiz
            </TabButton>
            <TabButton
              active={tab === "practice"}
              onClick={() => setTab("practice")}
              testid="tab-practice"
            >
              Practice
            </TabButton>
          </div>

          {/* Lessons tab */}
          {tab === "lessons" && (
            <div data-testid="lessons-tab">
              {sectionBlocks.length === 0 && blocks.length === 0 && !kickoffError && (
                <div className="flex h-32 flex-col items-center justify-center gap-1 text-sm text-muted-foreground">
                  <div className="flex items-center">
                    <Loader2 size={16} className="mr-2 animate-spin" />
                    <span>{kickingOff ? "Planning lessons..." : "Preparing lessons..."}</span>
                  </div>
                  {kickingOff && (
                    <p className="text-xs text-muted-foreground/70">
                      First-time generation can take 30-60 seconds.
                    </p>
                  )}
                </div>
              )}
              {sectionBlocks.length === 0 && blocks.length === 0 && kickoffError && (
                <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-4">
                  <p className="text-sm font-medium text-foreground">
                    Couldn't generate lessons
                  </p>
                  <p className="mt-1 text-sm text-muted-foreground">{kickoffError}</p>
                  <button
                    onClick={kickoffGeneration}
                    disabled={kickingOff}
                    className="mt-3 rounded-lg bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                  >
                    {kickingOff ? "Retrying..." : "Retry"}
                  </button>
                </div>
              )}
              {activeLesson && (
                <>
                  <div
                    key={activeLesson.id}
                    data-active="true"
                    data-lesson-index={activeLessonIndex}
                  >
                    <BlockRenderer
                      block={activeLesson}
                      lessonIndex={activeLessonIndex}
                      priorCompletedCount={
                        sectionBlocks
                          .slice(0, activeLessonIndex)
                          .filter((sb) => lessonCompletions?.has(sb.id)).length
                      }
                      moduleId={moduleId!}
                      trackId={trackId}
                    />
                  </div>

                  {/* Prev/Next lesson navigation — bottom of the active lesson.
                      On the last lesson, "Next" is replaced by a contextual
                      "What's next?" CTA: Take the quiz, or Continue to next
                      module if the learner has already passed. */}
                  <div className="mt-6 flex items-center justify-between border-t border-border pt-4">
                    <button
                      type="button"
                      aria-label="Previous lesson"
                      onClick={() => goToLesson(-1)}
                      disabled={activeLessonIndex <= 0}
                      className={cn(
                        "flex items-center gap-1.5 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                        activeLessonIndex <= 0
                          ? "cursor-not-allowed text-muted-foreground/40"
                          : "text-foreground hover:bg-accent",
                      )}
                    >
                      <ChevronLeft size={16} />
                      <span>Previous lesson</span>
                    </button>
                    <span className="text-xs text-muted-foreground">
                      Lesson {activeLessonIndex + 1} of {sectionBlocks.length}
                    </span>
                    {isLastLesson ? (
                      <div className="flex items-center gap-2">
                        {moduleAlreadyPassed && nextModuleId ? (
                          <Link
                            to={`/track/${trackId}/module/${nextModuleId}`}
                            className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                          >
                            <span>Continue to next module</span>
                            <ChevronRight size={16} />
                          </Link>
                        ) : (
                          <button
                            type="button"
                            onClick={() => setTab("quiz")}
                            className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                          >
                            <span>Take the quiz</span>
                            <ChevronRight size={16} />
                          </button>
                        )}
                      </div>
                    ) : (
                      <button
                        type="button"
                        aria-label="Next lesson"
                        onClick={() => goToLesson(1)}
                        className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                      >
                        <span>Next lesson</span>
                        <ChevronRight size={16} />
                      </button>
                    )}
                  </div>
                </>
              )}
            </div>
          )}

          {/* Quiz tab */}
          {tab === "quiz" && (
            <div data-testid="quiz-tab">
              {quizBlock ? (
                <BlockRenderer
                  block={quizBlock}
                  moduleId={moduleId!}
                  trackId={trackId}
                />
              ) : (
                <p className="py-8 text-center text-sm text-muted-foreground">
                  Quiz still being prepared...
                </p>
              )}
            </div>
          )}

          {/* Practice tab — Phase 03.1 Plan 09 (GAP-01):
                When the module has at least one lab block, dispatch each
                via BlockRenderer (which routes to LabBlock for blockType
                === "lab"). When no lab blocks exist, fall back to the
                Phase-1 ExerciseContainer so legacy modules keep their
                Practice surface. The skeleton-then-fill lifecycle for
                pending lab blocks is handled by BlockRenderer's existing
                arm; the polling effect above already pulls progress as
                blocks transition from pending → ready. */}
          {tab === "practice" && (
            <div data-testid="practice-tab" className="space-y-4">
              {labBlocks.length > 0 ? (
                <div className="space-y-4">
                  {labBlocks.map((lab) => (
                    <BlockRenderer
                      key={lab.id}
                      block={lab}
                      moduleId={moduleId!}
                      trackId={trackId}
                    />
                  ))}
                </div>
              ) : (
                <>
                  <div className="glass rounded-lg border border-border px-4 py-3 text-sm text-foreground/80">
                    <strong className="font-semibold text-foreground">Bonus practice.</strong>{" "}
                    These coding exercises are optional and don't affect module
                    mastery — passing the <em>Quiz</em> is what completes the
                    module and unlocks the next one.
                  </div>
                  <div className="glass rounded-lg p-6">
                    <ExerciseContainer moduleId={moduleId!} />
                  </div>
                </>
              )}
            </div>
          )}
        </div>
      </div>

      {/* AI Tutor toggle button */}
      <button
        onClick={() => setTutorOpen((prev) => !prev)}
        className={cn(
          "fixed bottom-16 right-6 z-30 flex h-12 w-12 items-center justify-center rounded-full shadow-lg transition-colors",
          tutorOpen
            ? "bg-secondary text-foreground"
            : "bg-primary text-primary-foreground hover:bg-primary/90",
        )}
        aria-label={tutorOpen ? "Close AI Tutor" : "Open AI Tutor"}
      >
        <MessageCircle size={20} />
      </button>

      {/* AI Tutor sidebar */}
      <TutorSidebar
        isOpen={tutorOpen}
        onClose={() => setTutorOpen(false)}
        trackId={trackId ?? ""}
        moduleId={moduleId ?? ""}
        moduleTitle={currentModule.title}
      />
    </div>
  );
}

interface TabButtonProps {
  active: boolean;
  onClick: () => void;
  testid: string;
  children: React.ReactNode;
}

function TabButton({ active, onClick, testid, children }: TabButtonProps) {
  return (
    <button
      data-testid={testid}
      onClick={onClick}
      className={cn(
        "flex flex-1 items-center justify-center rounded-md px-3 py-2 text-sm font-medium transition-colors",
        active
          ? "bg-primary text-primary-foreground"
          : "text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}
