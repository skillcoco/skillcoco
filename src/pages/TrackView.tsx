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
  Download,
  Loader2,
  Crown,
} from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";
import { layoutDAG, DAG_NODE_WIDTH, DAG_NODE_HEIGHT } from "@/lib/dag-layout";
import type { PathModule, ModuleStatus } from "@/types";
import { cn, formatDuration } from "@/lib/utils";
import { getTrackColor } from "@/lib/track-colors";
import {
  parsePathModules,
  pickNextModule,
  computeCertGate,
} from "@/lib/learning-path";
import {
  listTopicPacksAdmin,
  exportCourse,
} from "@/lib/tauri-commands";
import { save } from "@tauri-apps/plugin-dialog";
import { CertificationProgress } from "@/components/achievements/CertificationProgress";

// ── D-16 / D-05 — mastery band helper ──
//
// Locked band taxonomy (18-UI-SPEC.md Mastery Bands table): Novice (0-24%),
// Working (25-59%), Proficient (60-84%), Mastered (85-100%). Practical
// mastery renders "Not assessed" (never "0%") when the module has not been
// assessed via a hands-on lab — see `formatPracticalMastery` below for the
// frontend heuristic and its documented limitation.
export function masteryBand(pct: number): "Novice" | "Working" | "Proficient" | "Mastered" {
  if (pct >= 85) return "Mastered";
  if (pct >= 60) return "Proficient";
  if (pct >= 25) return "Working";
  return "Novice";
}

const MASTERY_BAND_CLASS: Record<ReturnType<typeof masteryBand>, string> = {
  Novice: "text-muted-foreground",
  Working: "text-foreground",
  Proficient: "text-emerald-400",
  Mastered: "text-emerald-500",
};

/**
 * D-16 practical mastery formatter — "{band} · {pct}%" or a "Not assessed"
 * chip, never "0%" (18-UI-SPEC.md D-05 lock).
 *
 * KNOWN LIMITATION (flagged for planner/checkpoint review): the frontend
 * `ModuleProgress` IPC shape carries `practicalMastery: number` with no
 * signal distinguishing "no lab content for this module" from "assessed at
 * exactly 0%" (the backend's `SqliteReportStore::practical_mastery` in
 * `storage_impl/reports.rs` correctly returns `None` via a `lab_progress`
 * table lookup, but that lookup is not exposed on `get_module_progress`).
 * Until a `hasLabContent`-style field is added to that IPC response (an
 * additive, non-breaking backend change outside this frontend-only plan's
 * scope), this helper treats `practicalMastery === 0` as "not assessed" —
 * which is correct for every module that has never had lab activity
 * (the v006 migration default is 0.0) but would also mask a lab genuinely
 * scored at 0%. Surfaced explicitly at the Task 3 human-verify checkpoint.
 */
export function formatPracticalMastery(
  practicalMastery: number,
): { band: string; label: string; className: string } | null {
  if (practicalMastery <= 0) return null;
  const pct = Math.round(practicalMastery * 100);
  const band = masteryBand(pct);
  return { band, label: `${band} · ${pct}%`, className: MASTERY_BAND_CLASS[band] };
}

// ── D-10 UI mirror: exportability predicate ──
//
// Mirrors the Rust is_course_exportable allowlist in course_io.rs (Plan 02).
// Fail-closed: unknown / empty / null → not exportable.
// This is UX-only — the backend export_course command is the authoritative gate
// (T-12-17). A hidden/disabled button does not substitute for the backend check.
//
// Exportable classes:
//   "topic-pack:<id>"   — content derived from a topic pack
//   "imported:<id>"     — previously imported course (D-11 preserves class)
//   <non-empty string>  — AI-generated model name (bare model name, not a reserved prefix)
//
// Non-exportable reserved prefixes: "licensed:", "curated:", ""

const RESERVED_NON_EXPORTABLE_PREFIXES = ["licensed:", "curated:"] as const;

export function isCourseExportable(generatedByModel?: string | null): boolean {
  if (!generatedByModel || generatedByModel.trim() === "") return false;
  const model = generatedByModel.trim();
  if (model.startsWith("topic-pack:")) return true;
  if (model.startsWith("imported:")) return true;
  for (const prefix of RESERVED_NON_EXPORTABLE_PREFIXES) {
    if (model.startsWith(prefix)) return false;
  }
  // Non-empty string that is not a reserved prefix → AI-generated model name
  return true;
}

// ── g73 — Licensed course provenance parser ──
//
// Parses the pipe-encoded provenance string produced by sheet2pack.py --licensed:
//   "licensed:{pack_id}|{licensor}"  → licensed=true, licensor="{licensor}"
//   "licensed:{pack_id}"             → licensed=true, licensor=null (bare, no pipe)
//   anything else                    → licensed=false, licensor=null
//
// Splits on the FIRST "|" only so licensor display text is preserved verbatim.
// Licensor is rendered as React text children — auto-escaped (T-g73-01).

export function parseLicensor(generatedByModel?: string | null): {
  licensed: boolean;
  licensor: string | null;
} {
  const model = generatedByModel?.trim();
  if (!model || !model.startsWith("licensed:")) {
    return { licensed: false, licensor: null };
  }
  const pipeIdx = model.indexOf("|");
  if (pipeIdx === -1) {
    return { licensed: true, licensor: null };
  }
  const name = model.slice(pipeIdx + 1).trim();
  return { licensed: true, licensor: name || null };
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
  isOpenable,
}: {
  module: PathModule;
  status: ModuleStatus;
  accentColor: string;
  onClick: () => void;
  isSelected: boolean;
  isOpenable?: boolean;
}) {
  const config = STATUS_CONFIG[status];
  const Icon = config.icon;

  // Phase 10 Plan 03 (D-07): isOpenable overrides status-based gate in free mode.
  const openable = isOpenable !== undefined ? isOpenable : status !== "locked";

  const borderColor =
    status === "available" || status === "in_progress" ? accentColor : undefined;

  return (
    <div
      onClick={openable ? onClick : undefined}
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
  isOpenable,
  practicalMastery,
}: {
  module: PathModule;
  status: ModuleStatus;
  accentColor: string;
  trackId: string;
  onClose: () => void;
  isOpenable?: boolean;
  /** D-16 — raw 0-1 practical mastery for this module, or undefined if no
   * progress row is loaded yet. */
  practicalMastery?: number;
}) {
  const navigate = useNavigate();
  // Phase 10 Plan 03 (D-07): isOpenable overrides status-based lock in free mode.
  const isClickable = isOpenable !== undefined ? isOpenable : status !== "locked";
  const practical = formatPracticalMastery(practicalMastery ?? 0);

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

          {/* Phase 18 Plan 05 (D-16) — practical mastery peer line item.
              Reads as a peer to the Stats row above — no new visual weight
              (no badge shape, no new bold). "Not assessed" (amber chip) is
              rendered instead of "0%" per the 18-UI-SPEC.md D-05 lock. */}
          <div className="flex items-center gap-1.5 text-sm">
            <span className="text-xs font-medium text-muted-foreground">
              Practical
            </span>
            {practical ? (
              <span className={cn("text-sm font-medium", practical.className)}>
                {practical.label}
              </span>
            ) : (
              <span className="rounded-full bg-amber-400/10 px-2 py-0.5 text-xs font-medium text-amber-400">
                Not assessed
              </span>
            )}
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

          {/* Phase 10 Plan 03 (D-09): show lock message only in linear mode.
              In free mode, status may be "locked" in DB but the module IS
              openable — replace hard-lock text with a neutral hint. */}
          {status === "locked" && !isClickable && (
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
  const { currentTrack, currentPath, moduleProgress, selectTrack, isLoading, setTrackBrowseMode } =
    useLearningStore();
  const [selectedModuleId, setSelectedModuleId] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // ── Phase 12 Plan 04 — Export course state ──
  const [exportStatus, setExportStatus] = useState<
    "idle" | "exporting" | "success" | "error"
  >("idle");
  const [exportMessage, setExportMessage] = useState<string | null>(null);

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

  // Certificate gate — drives the explicit "more than finishing" panel in
  // CertificationProgress (100% modules mastered AND avg mastery >= 0.85).
  const certGate = computeCertGate(
    modules,
    (id) => progressMap.get(id)?.masteryLevel ?? 0,
  );

  const selectedModule = selectedModuleId
    ? modules.find((m) => m.id === selectedModuleId) ?? null
    : null;
  const selectedStatus: ModuleStatus = selectedModuleId
    ? progressMap.get(selectedModuleId)?.status ?? "locked"
    : "locked";

  function getModuleStatus(moduleId: string): ModuleStatus {
    return progressMap.get(moduleId)?.status ?? "locked";
  }

  // Phase 10 Plan 03 (D-07) — effectiveOpenable helper.
  // In free mode, ANY module status is openable (frontend presentation only;
  // module_progress.status and cert/mastery gates are unchanged — D-03/D-04).
  // In linear mode, falls back to the sequential lock rule (status !== "locked").
  const browseMode = currentTrack.browseMode ?? "linear";
  function effectiveOpenable(status: ModuleStatus): boolean {
    return browseMode === "free" ? true : status !== "locked";
  }

  // ── Phase 12 Plan 04 — Export handler ──
  //
  // Opens the native save dialog, then calls the backend export_course command.
  // The backend is the authoritative gate (D-10); the UI hides/disables the
  // button for non-exportable courses as a UX convenience only (T-12-17).
  async function handleExport() {
    if (!trackId || !currentTrack) return;
    setExportStatus("exporting");
    setExportMessage(null);
    try {
      const savePath = await save({
        filters: [{ name: "LearnForge Course", extensions: ["json"] }],
        defaultPath: `${currentTrack.topic.replace(/[^a-z0-9]/gi, "_")}.json`,
      });
      if (!savePath) {
        // User cancelled
        setExportStatus("idle");
        return;
      }
      const result = await exportCourse({ trackId, savePath });
      setExportStatus("success");
      setExportMessage(
        `Saved to ${result.savedPath} (${result.moduleCount} modules, ${result.blockCount} blocks)`,
      );
    } catch (err) {
      setExportStatus("error");
      setExportMessage(String(err));
    }
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
            {/* g73 — Premium licensed course badge. Renders when provenance
                starts with "licensed:" (pipe-encoded licensor optional).
                Licensor is auto-escaped as React text child (T-g73-01). */}
            {(() => {
              const { licensed, licensor } = parseLicensor(currentPath?.generatedByModel);
              if (!licensed) return null;
              return (
                <span
                  data-testid="licensed-badge"
                  className="mt-1.5 inline-flex items-center gap-1 rounded-full border border-amber-500/40 bg-gradient-to-r from-amber-500/15 to-yellow-500/10 px-2.5 py-0.5 text-xs font-semibold tracking-wide text-amber-600 shadow-[0_0_12px_rgba(245,158,11,0.25)] dark:text-amber-400"
                >
                  <Crown size={13} />
                  Licensed Course{licensor ? ` · ${licensor}` : ""}
                </span>
              );
            })()}
          </div>
          <div className="flex items-center gap-3">
            {/* Phase 12 Plan 04 — Export course button (D-10 UI mirror).
                Hidden when the course provenance is non-exportable (fail-closed).
                The backend export_course command is the authoritative gate. */}
            {isCourseExportable(currentPath?.generatedByModel) && (
              <button
                type="button"
                onClick={handleExport}
                disabled={exportStatus === "exporting"}
                data-testid="export-course-button"
                title="Export this course to a .json file"
                className="inline-flex items-center gap-1.5 rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {exportStatus === "exporting" ? (
                  <>
                    <Loader2 size={13} className="animate-spin" />
                    Exporting...
                  </>
                ) : (
                  <>
                    <Download size={13} />
                    Export course
                  </>
                )}
              </button>
            )}
            <div className="text-right">
              <div className="text-2xl font-bold text-foreground">
                {Math.round(currentTrack.progressPercent)}%
              </div>
              <div className="text-xs text-muted-foreground">complete</div>
            </div>
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

        {/* Phase 12 Plan 04 — Export feedback message */}
        {exportMessage && exportStatus === "success" && (
          <p className="text-xs text-green-600 dark:text-green-400">
            {exportMessage}
          </p>
        )}
        {exportMessage && exportStatus === "error" && (
          <p className="text-xs text-destructive">{exportMessage}</p>
        )}

        {/* Phase 10 Plan 03 — browse-mode toggle (D-01/D-02).
            Default: "linear" (sequential lock rules active).
            "free": every module is openable; cert/mastery gates intact.
            Presentation-only — does NOT touch computeCertGate or module statuses. */}
        <div className="flex items-center gap-2 text-sm">
          <span className="text-xs font-medium text-muted-foreground">Browse mode:</span>
          <select
            data-testid="browse-mode-toggle"
            value={currentTrack.browseMode ?? "linear"}
            onChange={(e) => {
              const newMode = e.target.value as "linear" | "free";
              setTrackBrowseMode(currentTrack.id, newMode);
            }}
            className="rounded-md border border-border bg-background px-2 py-1 text-xs text-foreground focus:outline-none focus:ring-2 focus:ring-primary"
          >
            <option value="linear">Linear</option>
            <option value="free">Free</option>
          </select>
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
        {trackId && (
          <CertificationProgress trackId={trackId} gate={certGate} />
        )}
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
                      isOpenable={effectiveOpenable(status)}
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

        {/* Continue CTA — sits directly below the Learning Path so the most
            common action (resume the next actionable module) is one click
            from the path the learner just scanned. */}
        {ctaTarget && (
          <div className="mt-4 flex items-center gap-3">
            <button
              type="button"
              onClick={() =>
                navigate(`/track/${currentTrack.id}/module/${ctaTarget.id}`)
              }
              data-testid="track-continue-cta"
              aria-label={ctaLabel}
              className="inline-flex items-center gap-2 rounded-lg bg-primary px-5 py-2.5 text-sm font-semibold text-primary-foreground shadow-sm transition-colors duration-200 hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
            >
              {allComplete ? <RotateCcw size={16} /> : <PlayCircle size={16} />}
              <span>{ctaLabel}</span>
            </button>
            {nextModule && !allComplete && (
              <span className="min-w-0 truncate text-xs text-muted-foreground">
                Module {modules.indexOf(nextModule) + 1} of {modules.length} ·{" "}
                {nextModule.title}
              </span>
            )}
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
          isOpenable={effectiveOpenable(selectedStatus)}
          practicalMastery={progressMap.get(selectedModule.id)?.practicalMastery}
        />
      )}
    </div>
  );
}
