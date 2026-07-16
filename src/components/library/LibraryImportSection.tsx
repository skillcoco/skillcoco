import { useState } from "react";
import { FolderOpen, AlertTriangle, CheckCircle2, Loader2 } from "lucide-react";
import { importCourse, openFileDialog } from "@/lib/tauri-commands";
import { useLearningStore } from "@/stores/useLearningStore";

// ── Phase 16 Plan 03 Task 1 — LibraryImportSection ──
//
// Relocates the SettingsCourseImportSection body (Phase 12 Plan 04) into a
// compact inline row mounted in Library.tsx (LIB-03 — Library hosts the
// import-file entry point). Same import_course gate, same call sequence
// (openFileDialog then importCourse).

interface ImportState {
  status: "idle" | "loading" | "success" | "error";
  trackId?: string;
  moduleCount?: number;
  blockCount?: number;
  warnings?: string[];
  error?: string;
}

export function LibraryImportSection() {
  const [state, setState] = useState<ImportState>({ status: "idle" });
  const loadTracks = useLearningStore((s) => s.loadTracks);

  async function handleImport() {
    setState({ status: "loading" });

    let filePath: string | string[] | null;
    try {
      filePath = await openFileDialog({
        multiple: false,
        filters: [{ name: "SkillCoco Course", extensions: ["json"] }],
      });
    } catch (err) {
      setState({
        status: "error",
        error: `Could not open file picker: ${String(err)}`,
      });
      return;
    }

    // Guard: user cancelled or got back an array (shouldn't happen with multiple:false)
    if (filePath === null || filePath === undefined) {
      setState({ status: "idle" });
      return;
    }
    const resolvedPath = Array.isArray(filePath) ? filePath[0] : filePath;
    if (!resolvedPath) {
      setState({ status: "idle" });
      return;
    }

    try {
      const result = await importCourse({ filePath: resolvedPath });
      setState({
        status: "success",
        trackId: result.trackId,
        moduleCount: result.moduleCount,
        blockCount: result.blockCount,
        warnings: result.warnings,
      });

      // WR-03 — the "Your packs" grid on this same page is fed by
      // useLearningStore.tracks; refresh it so the new pack appears without
      // navigating away and back. Fire-and-forget: a failed refresh must not
      // affect the already-successful import feedback.
      loadTracks().catch(() => {});
    } catch (err) {
      setState({
        status: "error",
        error: String(err),
      });
    }
  }

  return (
    <div className="flex items-start gap-3">
      <div className="flex-1">
        <p className="text-sm font-medium text-foreground">Import course file</p>
        <p className="mt-0.5 text-xs text-muted-foreground">
          Import a SkillCoco .json course file to create a new track. Each
          import creates a fresh independent copy.
        </p>

        {state.status === "success" && (
          <div className="mt-3 rounded-lg border border-green-500/30 bg-green-500/10 p-3 space-y-1">
            <div className="flex items-center gap-2 text-sm font-medium text-green-600 dark:text-green-400">
              <CheckCircle2 size={14} />
              Course imported successfully
            </div>
            <p className="text-xs text-muted-foreground">
              Track ID: <code className="font-mono text-xs">{state.trackId}</code>
            </p>
            <p className="text-xs text-muted-foreground">
              {state.moduleCount} module{state.moduleCount !== 1 ? "s" : ""},{" "}
              {state.blockCount} block{state.blockCount !== 1 ? "s" : ""}
            </p>
            {state.warnings && state.warnings.length > 0 && (
              <div className="mt-2 space-y-1">
                {state.warnings.map((w, i) => (
                  <div key={i} className="flex items-start gap-1.5 text-xs text-amber-600 dark:text-amber-400">
                    <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                    <span>{w}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {state.status === "error" && (
          <div className="mt-3 rounded-lg border border-destructive/30 bg-destructive/10 p-3">
            <div className="flex items-center gap-2 text-sm font-medium text-destructive">
              <AlertTriangle size={14} />
              Import failed
            </div>
            <p className="mt-1 text-xs text-muted-foreground">{state.error}</p>
          </div>
        )}
      </div>
      <button
        type="button"
        onClick={handleImport}
        disabled={state.status === "loading"}
        data-testid="import-course-button"
        className="inline-flex shrink-0 items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground shadow-sm transition-colors hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {state.status === "loading" ? (
          <>
            <Loader2 size={14} className="animate-spin" />
            Importing...
          </>
        ) : (
          <>
            <FolderOpen size={14} />
            Choose file
          </>
        )}
      </button>
    </div>
  );
}
