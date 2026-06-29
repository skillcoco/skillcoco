import { useEffect, useMemo, useRef, useState } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  Play,
  PlayCircle,
  RotateCcw,
  CheckCircle2,
  Lock,
  BookOpen,
  Clock,
  Layers,
  Target,
  ChevronRight,
  X,
} from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { layoutDAG, DAG_NODE_WIDTH, DAG_NODE_HEIGHT } from "@/lib/dag-layout";
import type { PathModule, ModuleStatus } from "@/types";
import { cn, formatDuration } from "@/lib/utils";
import { parsePathModules, pickNextModule } from "@/lib/learning-path";
import { listTopicPacksAdmin } from "@/lib/tauri-commands";
import { CertificationProgress } from "@/components/achievements/CertificationProgress";

// ── Track color helper (matches TrackCard pattern) ──

function getTrackColor(topic: string): string {
  const key = topic.toLowerCase();
  if (key.includes("kubernetes") || key.includes("k8s")) return "hsl(var(--track-kubernetes))";
  if (key.includes("rust")) return "hsl(var(--track-rust))";
  if (key.includes("go") || key.includes("golang")) return "hsl(var(--track-go))";
  if (key.includes("python")) return "hsl(var(--track-python))";
  return "hsl(var(--primary))";
}

// ── Status config ──

interface StatusConfig {
  icon: typeof Lock;
  label: string;
  nodeClass: string;
  iconClass: string;
}

const STATUS_CONFIG: Record<ModuleStatus, StatusConfig> = {
  locked: {
    icon: Lock,
    label: "Locked",
    nodeClass: "opacity-50 border-border cursor-not-allowed",
    iconClass: "text-muted-foreground",
  },
  available: {
    icon: Play,
    label: "Available",
    nodeClass: "border-2 cursor-pointer hover:scale-[1.03] transition-transform",
    iconClass: "text-foreground",
  },
  in_progress: {
    icon: BookOpen,
    label: "In Progress",
    nodeClass: "border-2 cursor-pointer hover:scale-[1.03] transition-transform animate-pulse-border",
    iconClass: "text-foreground",
  },
  completed: {
    icon: CheckCircle2,
    label: "Completed",
    nodeClass: "border-green-500/40 bg-green-500/5 cursor-pointer hover:scale-[1.03] transition-transform",
    iconClass: "text-green-500",
  },
  skipped: {
    icon: ChevronRight,
    label: "Skipped",
    nodeClass: "opacity-60 border-border cursor-pointer",
    iconClass: "text-muted-foreground",
  },
};

// ── Difficulty dots ──

function DifficultyDots({ level, max = 5 }: { level: number; max?: number }) {
  // Map 1-10 scale to 1-5 dots
  const dots = Math.max(1, Math.min(max, Math.ceil(level / 2)));
  return (
    <div className="flex gap-0.5">
      {Array.from({ length: max }, (_, i) => (
        <div
          key={i}
          className={cn(
            "h-1.5 w-1.5 rounded-full",
            i < dots ? "bg-foreground/60" : "bg-foreground/15",
          )}
        />
      ))}
    </div>
  );
}

// ── SVG Edge connector ──

function EdgeConnector({
  fromX,
  fromY,
  toX,
  toY,
  type,
  accentColor,
}: {
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  type: string;
  accentColor: string;
}) {
  // Bezier curve between two points
  const dx = toX - fromX;
  const cp = dx * 0.4;

  const d = `M ${fromX} ${fromY} C ${fromX + cp} ${fromY}, ${toX - cp} ${toY}, ${toX} ${toY}`;

  return (
    <path
      d={d}
      fill="none"
      stroke={type === "prerequisite" ? accentColor : "hsl(var(--muted-foreground))"}
      strokeWidth={type === "prerequisite" ? 2 : 1}
      strokeDasharray={type === "optional" ? "6 4" : type === "recommended" ? "4 2" : "none"}
      strokeOpacity={type === "prerequisite" ? 0.5 : 0.3}
    />
  );
}

// ── Module Node ──

function ModuleNode({
  module,
  status,
  accentColor,
  onClick,
  isSelected,
}: {
  module: PathModule;
  status: ModuleStatus;
  accentColor: string;
  onClick: () => void;
  isSelected: boolean;
}) {
  const config = STATUS_CONFIG[status];
  const Icon = config.icon;

  const borderColor =
    status === "available" || status === "in_progress" ? accentColor : undefined;

  return (
    <div
      onClick={status !== "locked" ? onClick : undefined}
      className={cn(
        "glass absolute flex flex-col gap-1.5 rounded-xl border p-3",
        config.nodeClass,
        isSelected && "ring-2 ring-offset-2 ring-offset-background",
      )}
      style={{
        width: DAG_NODE_WIDTH,
        height: DAG_NODE_HEIGHT,
        borderColor,
        ...(isSelected ? { ringColor: accentColor } : {}),
        ...(status === "in_progress" ? { boxShadow: `0 0 12px ${accentColor}30` } : {}),
      }}
    >
      {/* Top row: icon + title */}
      <div className="flex items-start gap-2">
        <Icon size={16} className={cn("mt-0.5 shrink-0", config.iconClass)} />
        <span className="text-sm font-medium leading-tight text-foreground line-clamp-2">
          {module.title}
        </span>
      </div>

      {/* Bottom row: difficulty + time */}
      <div className="mt-auto flex items-center justify-between">
        <DifficultyDots level={module.difficulty} />
        <span className="text-[11px] text-muted-foreground">
          {module.estimatedMinutes}m
        </span>
      </div>
    </div>
  );
}

// ── Module Detail Panel ──

function ModuleDetailPanel({
  module,
  status,
  accentColor,
  trackId,
  onClose,
}: {
  module: PathModule;
  status: ModuleStatus;
  accentColor: string;
  trackId: string;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const isClickable = status !== "locked";

  return (
    <div className="glass-strong rounded-xl border border-border p-6">
      <div className="flex items-start justify-between gap-4">
        <div className="flex-1 space-y-4">
          {/* Header */}
          <div>
            <div className="mb-1 flex items-center gap-2">
              <span
                className="rounded-full px-2 py-0.5 text-[11px] font-medium"
                style={{
                  backgroundColor: `${accentColor}15`,
                  color: accentColor,
                }}
              >
                {module.type}
              </span>
              <span className="text-xs text-muted-foreground">
                Difficulty {module.difficulty}/10
              </span>
            </div>
            <h3 className="text-lg font-semibold text-foreground">{module.title}</h3>
            <p className="mt-1 text-sm text-muted-foreground">{module.description}</p>
          </div>

          {/* Stats */}
          <div className="flex gap-6 text-sm text-muted-foreground">
            <div className="flex items-center gap-1.5">
              <Clock size={14} />
              <span>{module.estimatedMinutes} min estimated</span>
            </div>
            <div className="flex items-center gap-1.5">
              <DifficultyDots level={module.difficulty} />
            </div>
          </div>

          {/* Objectives */}
          {module.objectives.length > 0 && (
            <div>
              <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Objectives
              </h4>
              <ul className="space-y-1.5">
                {module.objectives.map((obj, i) => (
                  <li
                    key={i}
                    className="flex items-start gap-2 text-sm text-foreground"
                  >
                    <Target
                      size={12}
                      className="mt-1 shrink-0"
                      style={{ color: accentColor }}
                    />
                    {obj}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {/* Action button */}
          {isClickable && (
            <button
              onClick={() => navigate(`/track/${trackId}/module/${module.id}`)}
              className="inline-flex items-center gap-2 rounded-lg px-5 py-2.5 text-sm font-semibold text-white transition-colors hover:opacity-90"
              style={{ backgroundColor: accentColor }}
            >
              {status === "in_progress" ? (
                <>
                  <BookOpen size={16} />
                  Continue Module
                </>
              ) : status === "completed" ? (
                <>
                  <CheckCircle2 size={16} />
                  Review Module
                </>
              ) : (
                <>
                  <Play size={16} />
                  Start Module
                </>
              )}
            </button>
          )}

          {status === "locked" && (
            <p className="flex items-center gap-2 text-sm text-muted-foreground">
              <Lock size={14} />
              Complete prerequisite modules to unlock.
            </p>
          )}
        </div>

        {/* Close button */}
        <button
          onClick={onClose}
          className="rounded-md p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          <X size={18} />
        </button>
      </div>
    </div>
  );
}

// ── Main TrackView Component ──

export function TrackView() {
  const { trackId } = useParams<{ trackId: string }>();
  const navigate = useNavigate();
  const { currentTrack, currentPath, moduleProgress, selectTrack, isLoading } =
    useLearningStore();
  const [selectedModuleId, setSelectedModuleId] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Phase 5 Plan 05 (Wave 4) — R1 / T-05-17 mitigation: when this track was
  // generated from a Topic Pack AND the pack's source is "skill" (i.e.
  // user-authored), surface a "From skill: <id>" attribution badge so
  // learners can tell at a glance that the content originated outside the
  // bundled curriculum. Bundled packs DO NOT get a badge — bundled is the
  // default expectation. AI-generated tracks DO NOT get a badge — no pack.
  //
  // packId is parsed from `generated_by_model` which the backend sets to
  // `topic-pack:<id>` in `generate_path_from_pack_impl` (Task 1).
  const packId = useMemo(() => {
    const model = currentPath?.generatedByModel ?? "";
    return model.startsWith("topic-pack:") ? model.slice("topic-pack:".length) : null;
  }, [currentPath?.generatedByModel]);

  const [packSource, setPackSource] = useState<"bundled" | "skill" | null>(null);

  useEffect(() => {
    // Only hit IPC when a pack id is actually present. AI-generated tracks
    // are the dominant case — we don't want a needless IPC for them.
    if (!packId) {
      setPackSource(null);
      return;
    }
    let cancelled = false;
    listTopicPacksAdmin()
      .then((packs) => {
        if (cancelled) return;
        const match = packs.find((p) => p.pack.id === packId);
        setPackSource(match?.source ?? null);
      })
      .catch(() => {
        if (!cancelled) setPackSource(null);
      });
    return () => {
      cancelled = true;
    };
  }, [packId]);

  useEffect(() => {
    if (trackId) selectTrack(trackId);
  }, [trackId]);

  // Reset selection when track changes
  useEffect(() => {
    setSelectedModuleId(null);
  }, [currentTrack?.id]);

  const progressMap = useMemo(
    () => new Map(moduleProgress.map((p) => [p.moduleId, p])),
    [moduleProgress],
  );

  // Parse modulesJson/edgesJson from backend (they arrive as JSON strings)
  const pathModules = useMemo(
    () => parsePathModules(currentPath?.modulesJson),
    [currentPath],
  );
  const pathEdges = useMemo(
    () => (currentPath ? (JSON.parse(currentPath.edgesJson || "[]") as import("@/types/learning").PathEdge[]) : []),
    [currentPath],
  );

  const layout = useMemo(() => {
    if (!currentPath) return null;
    return layoutDAG(pathModules, pathEdges);
  }, [currentPath, pathModules, pathEdges]);

  if (isLoading || !currentTrack) {
    return (
      <div className="flex h-64 items-center justify-center text-muted-foreground">
        Loading track...
      </div>
    );
  }

  const accentColor = getTrackColor(currentTrack.topic);
  const modules = pathModules;
  const completedCount = modules.filter(
    (m) => progressMap.get(m.id)?.status === "completed",
  ).length;
  const inProgressCount = modules.filter(
    (m) => progressMap.get(m.id)?.status === "in_progress",
  ).length;
  const totalEstimatedMinutes = modules.reduce(
    (acc, m) => acc + m.estimatedMinutes,
    0,
  );
  const completedMinutes = modules
    .filter((m) => progressMap.get(m.id)?.status === "completed")
    .reduce((acc, m) => acc + m.estimatedMinutes, 0);
  const remainingMinutes = totalEstimatedMinutes - completedMinutes;

  const selectedModule = selectedModuleId
    ? modules.find((m) => m.id === selectedModuleId) ?? null
    : null;
  const selectedStatus: ModuleStatus = selectedModuleId
    ? progressMap.get(selectedModuleId)?.status ?? "locked"
    : "locked";

  function getModuleStatus(moduleId: string): ModuleStatus {
    return progressMap.get(moduleId)?.status ?? "locked";
  }

  // "Continue learning" CTA target: next actionable module (in_progress →
  // first available → none). When the path is fully complete we offer review
  // of the first module.
  const nextModule = pickNextModule(modules, getModuleStatus);
  const allComplete = modules.length > 0 && completedCount === modules.length;
  const ctaTarget = nextModule ?? modules[0] ?? null;
  const ctaLabel = allComplete
    ? "Review course"
    : completedCount === 0 && !nextModule
      ? "Start learning"
      : completedCount === 0
        ? "Start learning"
        : "Continue";

  return (
    <div className="mx-auto max-w-7xl space-y-6 pb-12">
      {/* Track Header */}
      <div className="space-y-4">
        {/* Back + Title */}
        <div className="flex items-center gap-3">
          <Link
            to="/"
            className="rounded-md p-1.5 text-muted-foreground hover:bg-accent"
          >
            <ArrowLeft size={18} />
          </Link>
          <div className="flex-1">
            <div className="flex items-center gap-3">
              <h1 className="text-2xl font-bold text-foreground">
                {currentTrack.topic}
              </h1>
              <span
                className="rounded-full px-2.5 py-0.5 text-xs font-medium"
                style={{
                  backgroundColor: `${accentColor}15`,
                  color: accentColor,
                }}
              >
                {currentTrack.domainModule}
              </span>
            </div>
            <p className="mt-0.5 text-sm text-muted-foreground">
              {currentTrack.goal}
            </p>
            {/* Phase 5 R1 — skill-sourced track attribution. Renders only when
                the path was generated from a user-authored skill pack. */}
            {packId && packSource === "skill" && (
              <p
                data-testid="pack-attribution"
                className="mt-1 text-xs text-amber-600"
              >
                From skill: <code className="font-mono">{packId}</code>
              </p>
            )}
          </div>
          {ctaTarget && (
            <div className="flex flex-col items-end gap-1">
              <button
                type="button"
                onClick={() =>
                  navigate(`/track/${currentTrack.id}/module/${ctaTarget.id}`)
                }
                data-testid="track-continue-cta"
                aria-label={ctaLabel}
                className="inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2.5 text-sm font-semibold text-primary-foreground shadow-sm transition-colors duration-200 hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
              >
                {allComplete ? <RotateCcw size={16} /> : <PlayCircle size={16} />}
                <span>{ctaLabel}</span>
              </button>
              {nextModule && !allComplete && (
                <span className="max-w-[16rem] truncate text-xs text-muted-foreground">
                  Module {modules.indexOf(nextModule) + 1} of {modules.length} ·{" "}
                  {nextModule.title}
                </span>
              )}
            </div>
          )}
          <div className="text-right">
            <div className="text-2xl font-bold text-foreground">
              {Math.round(currentTrack.progressPercent)}%
            </div>
            <div className="text-xs text-muted-foreground">complete</div>
          </div>
        </div>

        {/* Progress bar */}
        <div className="h-2 rounded-full bg-secondary">
          <div
            className="h-2 rounded-full transition-all"
            style={{
              width: `${Math.round(currentTrack.progressPercent)}%`,
              backgroundColor: accentColor,
            }}
          />
        </div>

        {/* Stats row */}
        <div className="flex gap-6">
          <div className="glass flex items-center gap-2 rounded-lg px-4 py-2.5">
            <Layers size={16} style={{ color: accentColor }} />
            <div>
              <div className="text-sm font-semibold text-foreground">
                {completedCount}/{modules.length}
              </div>
              <div className="text-[10px] text-muted-foreground">
                modules completed
              </div>
            </div>
          </div>
          <div className="glass flex items-center gap-2 rounded-lg px-4 py-2.5">
            <Clock size={16} style={{ color: accentColor }} />
            <div>
              <div className="text-sm font-semibold text-foreground">
                {formatDuration(currentTrack.totalTimeSpent)}
              </div>
              <div className="text-[10px] text-muted-foreground">
                time spent
              </div>
            </div>
          </div>
          <div className="glass flex items-center gap-2 rounded-lg px-4 py-2.5">
            <Target size={16} style={{ color: accentColor }} />
            <div>
              <div className="text-sm font-semibold text-foreground">
                ~{Math.round(remainingMinutes)}m
              </div>
              <div className="text-[10px] text-muted-foreground">
                estimated remaining
              </div>
            </div>
          </div>
          {inProgressCount > 0 && (
            <div className="glass flex items-center gap-2 rounded-lg px-4 py-2.5">
              <BookOpen size={16} style={{ color: accentColor }} />
              <div>
                <div className="text-sm font-semibold text-foreground">
                  {inProgressCount}
                </div>
                <div className="text-[10px] text-muted-foreground">
                  in progress
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Phase 6 Plan 06-05 (Wave 4) — D-11 + CERT-11. Three-row earned/
            in-progress/locked indicator. Mounts immediately below the
            track header + stats row, above the DAG, so the learner sees
            progress signals before the modules tree. The component
            handles its own IPC + error state. */}
        {trackId && <CertificationProgress trackId={trackId} />}
      </div>

      {/* DAG Visualization */}
      <div>
        <h2 className="mb-3 text-lg font-semibold text-foreground">
          Learning Path
        </h2>

        {layout && layout.nodes.length > 0 ? (
          <div
            ref={scrollRef}
            className="glass overflow-x-auto rounded-xl p-4"
          >
            <div
              className="relative"
              style={{
                width: layout.width,
                height: layout.height,
                minHeight: 200,
              }}
            >
              {/* SVG layer for edges */}
              <svg
                className="pointer-events-none absolute inset-0"
                width={layout.width}
                height={layout.height}
              >
                {layout.edges.map((edge, i) => (
                  <EdgeConnector
                    key={`${edge.fromId}-${edge.toId}-${i}`}
                    fromX={edge.fromX}
                    fromY={edge.fromY}
                    toX={edge.toX}
                    toY={edge.toY}
                    type={edge.type}
                    accentColor={accentColor}
                  />
                ))}
              </svg>

              {/* Module nodes */}
              {layout.nodes.map((node) => {
                const status = getModuleStatus(node.module.id);
                return (
                  <div
                    key={node.module.id}
                    style={{
                      position: "absolute",
                      left: node.x - DAG_NODE_WIDTH / 2,
                      top: node.y - DAG_NODE_HEIGHT / 2,
                    }}
                  >
                    <ModuleNode
                      module={node.module}
                      status={status}
                      accentColor={accentColor}
                      isSelected={selectedModuleId === node.module.id}
                      onClick={() =>
                        setSelectedModuleId(
                          selectedModuleId === node.module.id
                            ? null
                            : node.module.id,
                        )
                      }
                    />
                  </div>
                );
              })}
            </div>
          </div>
        ) : (
          <div className="glass flex h-48 items-center justify-center rounded-xl text-sm text-muted-foreground">
            No modules in this learning path yet.
          </div>
        )}
      </div>

      {/* Module Detail Panel */}
      {selectedModule && trackId && (
        <ModuleDetailPanel
          module={selectedModule}
          status={selectedStatus}
          accentColor={accentColor}
          trackId={trackId}
          onClose={() => setSelectedModuleId(null)}
        />
      )}
    </div>
  );
}
