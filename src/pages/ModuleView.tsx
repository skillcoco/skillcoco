import { useState, useEffect, useCallback } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  MessageCircle,
  Loader2,
  Target,
  ChevronRight,
  BookOpen,
} from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { generateModuleContent } from "@/lib/tauri-commands";
import { MarkdownRenderer } from "@/components/learning/MarkdownRenderer";
import { TutorSidebar } from "@/components/learning/TutorSidebar";
import { ExerciseContainer } from "@/components/exercises/ExerciseContainer";
import { cn } from "@/lib/utils";

type ViewMode = "content" | "exercises";

export function ModuleView() {
  const { trackId, moduleId } = useParams<{ trackId: string; moduleId: string }>();
  const navigate = useNavigate();
  const { currentTrack, currentPath, moduleProgress, selectTrack } = useLearningStore();

  const [content, setContent] = useState<string | null>(null);
  const [isLoadingContent, setIsLoadingContent] = useState(false);
  const [contentError, setContentError] = useState<string | null>(null);
  const [tutorOpen, setTutorOpen] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>("content");

  // Load track if not already loaded
  useEffect(() => {
    if (trackId && (!currentTrack || currentTrack.id !== trackId)) {
      selectTrack(trackId);
    }
  }, [trackId, currentTrack, selectTrack]);

  // Find the current module in the path
  const currentModule = currentPath?.modules.find((m) => m.id === moduleId);
  const progress = moduleProgress.find((p) => p.moduleId === moduleId);

  // Generate or load module content
  useEffect(() => {
    if (!currentModule || !trackId || !moduleId || content) return;

    let cancelled = false;

    async function loadContent() {
      setIsLoadingContent(true);
      setContentError(null);
      try {
        const result = await generateModuleContent({
          moduleId: moduleId!,
          trackId: trackId!,
          moduleTitle: currentModule!.title,
          objectives: currentModule!.objectives,
          learnerLevel: currentTrack?.domainModule ?? "beginner",
          previousPerformance: progress?.score != null ? `Score: ${progress.score}/100` : undefined,
        });
        if (!cancelled) {
          setContent(result);
        }
      } catch (err) {
        if (!cancelled) {
          setContentError(String(err));
        }
      } finally {
        if (!cancelled) setIsLoadingContent(false);
      }
    }

    loadContent();
    return () => { cancelled = true; };
  }, [currentModule, trackId, moduleId, content, currentTrack, progress]);

  const handleExercisesComplete = useCallback(() => {
    // Could auto-navigate or show a completion state
  }, []);

  // Compute progress percentage
  const progressPercent = progress?.masteryLevel != null ? Math.round(progress.masteryLevel * 100) : 0;

  if (!currentPath || !currentModule) {
    return (
      <div className="flex h-64 items-center justify-center text-muted-foreground">
        <Loader2 size={20} className="mr-2 animate-spin" />
        <span>Loading module...</span>
      </div>
    );
  }

  return (
    <div className={cn("mx-auto max-w-4xl space-y-6", tutorOpen && "mr-96")}>
      {/* Header */}
      <div className="flex items-center gap-3">
        <Link
          to={`/track/${trackId}`}
          className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <ArrowLeft size={18} />
        </Link>
        <div className="flex-1">
          <h1 className="text-2xl font-bold text-foreground">{currentModule.title}</h1>
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

      {/* Objectives */}
      {currentModule.objectives.length > 0 && (
        <div className="glass rounded-lg p-5">
          <div className="mb-3 flex items-center gap-2">
            <Target size={16} className="text-primary" />
            <h2 className="text-sm font-semibold text-foreground">Learning Objectives</h2>
          </div>
          <ul className="ml-5 list-disc space-y-1">
            {currentModule.objectives.map((obj, i) => (
              <li key={i} className="text-sm text-foreground/80">
                {obj}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* View mode tabs */}
      <div className="flex gap-1 rounded-lg border border-border p-1">
        <button
          onClick={() => setViewMode("content")}
          className={cn(
            "flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors",
            viewMode === "content"
              ? "bg-primary text-primary-foreground"
              : "text-muted-foreground hover:bg-accent hover:text-foreground"
          )}
        >
          <BookOpen size={16} />
          <span>Lesson Content</span>
        </button>
        <button
          onClick={() => setViewMode("exercises")}
          className={cn(
            "flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors",
            viewMode === "exercises"
              ? "bg-primary text-primary-foreground"
              : "text-muted-foreground hover:bg-accent hover:text-foreground"
          )}
        >
          <Target size={16} />
          <span>Exercises</span>
        </button>
      </div>

      {/* Content area */}
      {viewMode === "content" && (
        <>
          {isLoadingContent && (
            <div className="flex h-48 flex-col items-center justify-center text-muted-foreground">
              <Loader2 size={24} className="mb-3 animate-spin" />
              <p className="text-sm">Generating module content...</p>
              <p className="mt-1 text-xs text-muted-foreground/70">
                This may take a moment as the AI prepares your lesson.
              </p>
            </div>
          )}

          {contentError && (
            <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-4">
              <p className="text-sm font-medium text-foreground">Failed to load content</p>
              <p className="mt-1 text-sm text-muted-foreground">{contentError}</p>
              <button
                onClick={() => { setContent(null); setContentError(null); }}
                className="mt-3 rounded-lg bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
              >
                Retry
              </button>
            </div>
          )}

          {content && (
            <>
              <div className="glass rounded-lg p-6">
                <MarkdownRenderer content={content} />
              </div>

              {/* Continue to exercises */}
              <div className="flex justify-end">
                <button
                  onClick={() => setViewMode("exercises")}
                  className="flex items-center gap-2 rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                >
                  <span>Continue to Exercises</span>
                  <ChevronRight size={16} />
                </button>
              </div>
            </>
          )}
        </>
      )}

      {/* Exercises area */}
      {viewMode === "exercises" && moduleId && (
        <div className="glass rounded-lg p-6">
          <ExerciseContainer
            moduleId={moduleId}
            onAllComplete={handleExercisesComplete}
          />
        </div>
      )}

      {/* AI Tutor toggle button */}
      <button
        onClick={() => setTutorOpen((prev) => !prev)}
        className={cn(
          "fixed bottom-16 right-6 z-30 flex h-12 w-12 items-center justify-center rounded-full shadow-lg transition-colors",
          tutorOpen
            ? "bg-secondary text-foreground"
            : "bg-primary text-primary-foreground hover:bg-primary/90"
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
