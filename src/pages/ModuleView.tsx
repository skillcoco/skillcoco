import { useState, useEffect, useMemo, useRef, useCallback } from "react";
import { useParams, Link } from "react-router-dom";
import { ArrowLeft, MessageCircle, Loader2 } from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { BlockRenderer } from "@/components/learning/BlockRenderer";
import { ExerciseContainer } from "@/components/exercises/ExerciseContainer";
import { TutorSidebar } from "@/components/learning/TutorSidebar";
import { CourseSidebar } from "@/components/learning/CourseSidebar";
import { regenerateModule } from "@/lib/tauri-commands";
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
  const selectTrack = useLearningStore((s) => s.selectTrack);

  const [tab, setTab] = useState<Tab>("lessons");
  const [tutorOpen, setTutorOpen] = useState(false);
  const [generating, setGenerating] = useState(false);

  // cancelRef: guards against async operations after unmount/module change
  const cancelRef = useRef(false);

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

  // Mount: load blocks from cache (no generation on mount)
  // W1 LOCK: reuse existing getModuleContent path for module metadata — we read
  // module title/objectives directly from currentPath.modulesJson (already available
  // in the store from the Phase 1 selectTrack call). No new getModuleMeta IPC created.
  useEffect(() => {
    if (!moduleId) return;
    cancelRef.current = false;
    loadModuleBlocks(moduleId);
    return () => {
      cancelRef.current = true;
    };
  }, [moduleId, loadModuleBlocks]);

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

  // Legacy banner handler: calls regenerateModule IPC with PagePlanner-first atomicity
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
      <div className={cn("flex-1 overflow-y-auto", tutorOpen && "lg:mr-96")}>
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
              <span className="font-medium text-foreground">{progressPercent}%</span>
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
              {sectionBlocks.length === 0 && blocks.length === 0 && (
                <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
                  <Loader2 size={16} className="mr-2 animate-spin" />
                  <span>Preparing lessons...</span>
                </div>
              )}
              {sectionBlocks.map((block, i) => {
                const priorCompletedCount = sectionBlocks
                  .slice(0, i)
                  .filter((sb) => lessonCompletions?.has(sb.id)).length;
                const isActive = currentLessonId === block.id;
                return (
                  <div key={block.id} data-active={String(isActive)}>
                    <BlockRenderer
                      block={block}
                      lessonIndex={i}
                      priorCompletedCount={priorCompletedCount}
                      moduleId={moduleId!}
                      trackId={trackId}
                    />
                  </div>
                );
              })}
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

          {/* Practice tab — Phase 1 exercises preserved, NO BKT side-effect */}
          {tab === "practice" && (
            <div data-testid="practice-tab" className="glass rounded-lg p-6">
              <ExerciseContainer moduleId={moduleId!} />
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
