import { useState } from "react";
import { FolderOpen, Upload, AlertTriangle, CheckCircle2, Loader2 } from "lucide-react";
import { importCourse, openFileDialog } from "@/lib/tauri-commands";

// ── Phase 12 Plan 04 — Course Import Section ──
//
// Surfaces the import_course backend command (Plan 03) in Settings.
// On click: open native file picker → if filePath is a string, call
// importCourse → show trackId/moduleCount/blockCount + any warnings.
// On success, hints the learner that the new track is available in the
// track list (D-09 — fresh copy, not an overwrite).
//
// File-size note: Settings.tsx already exceeds 500 lines, so this section
// lives as a standalone sub-component (mirrors SettingsLabsSection.tsx).

interface ImportState {
  status: "idle" | "loading" | "success" | "error";
  trackId?: string;
  moduleCount?: number;
  blockCount?: number;
  warnings?: string[];
  error?: string;
  /**
   * `true` when the imported pack's signature chain of trust verified
   * successfully (TRUST-01, D-14). Phase 14 Plan 06 (CR-01) — previously
   * discarded from importCourse's result. The authoritative production
   * data path for the TrackView badge is get_path (14-06 Task 1), which
   * survives an app restart; this local state closes the call-site discard
   * the review flagged.
   */
  verified?: boolean;
  /** Publisher name from the verified issuer cert, when `verified` is `true`. */
  issuerName?: string | null;
}

export function SettingsCourseImportSection() {
  const [state, setState] = useState<ImportState>({ status: "idle" });

  async function handleImport() {
    setState({ status: "loading" });

    let filePath: string | string[] | null;
    try {
      filePath = await openFileDialog({
        multiple: false,
        filters: [{ name: "LearnForge Course", extensions: ["json"] }],
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
        verified: result.verified,
        issuerName: result.issuerName,
      });
    } catch (err) {
      setState({
        status: "error",
        error: String(err),
      });
    }
  }

  return (
    <section className="space-y-4">
      <h2 className="text-lg font-semibold text-foreground">Import Course</h2>

      <div className="glass rounded-xl p-5 space-y-4">
        <div className="flex items-start gap-3">
          <div className="flex-1">
            <p className="text-sm font-medium text-foreground">
              Import a course file
            </p>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Import a LearnForge .json course file to create a new track.
              Each import creates a fresh independent copy (D-09).
            </p>
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

        {/* Success feedback */}
        {state.status === "success" && (
          <div className="rounded-lg border border-green-500/30 bg-green-500/10 p-3 space-y-1">
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
            {state.verified === true && state.issuerName && (
              <p className="text-xs text-muted-foreground">
                Verified publisher: <span className="font-medium">{state.issuerName}</span>
              </p>
            )}
            <div className="flex items-start gap-1.5 mt-1">
              <Upload size={12} className="mt-0.5 shrink-0 text-muted-foreground" />
              <p className="text-xs text-muted-foreground">
                The new track is now available in your track list. Lessons and
                quizzes are ready to use immediately — no AI generation required.
              </p>
            </div>
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

        {/* Error feedback */}
        {state.status === "error" && (
          <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-3">
            <div className="flex items-center gap-2 text-sm font-medium text-destructive">
              <AlertTriangle size={14} />
              Import failed
            </div>
            <p className="mt-1 text-xs text-muted-foreground">{state.error}</p>
          </div>
        )}
      </div>
    </section>
  );
}
