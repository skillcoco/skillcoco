// Phase 18 (Skill Reports) — Plan 05 (Wave 3) export dialog.
//
// First consumer of `@radix-ui/react-dialog` in this codebase (already an
// existing dependency — zero new packages, T-18-SC). Modal, centered,
// `max-w-md`, `glass` surface — per 18-UI-SPEC.md Copywriting Contract
// (copy is LOCKED verbatim) + Component Inventory.
//
// One Export action drives BOTH exportReportPdf and exportReportJson
// (REP-01 "one action → two files"). The identity input is confirm-at-export
// (D-10) — pre-filled from the learner profile, editable, and the confirmed
// value is what gets baked into the signed report. Dialog.Close (icon-only
// X) carries `aria-label="Close"` per the accessibility contract.

import { useEffect, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { X, Loader2, Download } from "lucide-react";
import { useReportsStore } from "@/stores/useReportsStore";

export type ReportScope = "track" | "whole-profile";

export interface ExportReportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /// Defaults the scope selector — TrackView opens scoped to "track",
  /// Achievements opens scoped to "whole-profile" (UI-SPEC placement).
  defaultScope: ReportScope;
  /// Present when opened from TrackView; used for the "This track: {topic}"
  /// scope option and as the trackId sent to the export IPCs.
  trackId?: string;
  trackTopic?: string;
  /// Pre-fills the identity-confirm input (D-10). Editable.
  learnerName: string;
}

export function ExportReportDialog({
  open,
  onOpenChange,
  defaultScope,
  trackId,
  trackTopic,
  learnerName,
}: ExportReportDialogProps) {
  const exportReportPdf = useReportsStore((s) => s.exportReportPdf);
  const exportReportJson = useReportsStore((s) => s.exportReportJson);

  const [scope, setScope] = useState<ReportScope>(defaultScope);
  const [name, setName] = useState(learnerName);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Re-sync defaults whenever the dialog is (re)opened, so a stale edit from
  // a previous open doesn't leak into a fresh session.
  useEffect(() => {
    if (open) {
      setScope(defaultScope);
      setName(learnerName);
      setError(null);
    }
  }, [open, defaultScope, learnerName]);

  const handleExport = async () => {
    setBusy(true);
    setError(null);
    try {
      const params = {
        scope,
        trackId: scope === "track" ? trackId : undefined,
        trackTopic: scope === "track" ? trackTopic : undefined,
        learnerName: name,
      };
      await Promise.all([
        exportReportPdf(params),
        exportReportJson(params),
      ]);
      onOpenChange(false);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(
        `Couldn't export your report. ${message}. Try again, or check available disk space.`,
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-background/60 backdrop-blur-sm" />
        <Dialog.Content
          className="glass fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 space-y-4 rounded-xl p-5 shadow-xl"
          data-testid="export-report-dialog"
        >
          <div className="flex items-start justify-between">
            <Dialog.Title className="text-lg font-semibold text-foreground">
              Export skill report
            </Dialog.Title>
            <Dialog.Description className="sr-only">
              Export a signed skill report as PDF and JSON, confirming your
              name and the report scope.
            </Dialog.Description>
            <Dialog.Close asChild>
              <button
                type="button"
                aria-label="Close"
                className="rounded-md p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
              >
                <X size={18} />
              </button>
            </Dialog.Close>
          </div>

          <div className="space-y-1.5">
            <label
              htmlFor="report-scope"
              className="block text-xs font-medium text-foreground"
            >
              Report scope
            </label>
            <select
              id="report-scope"
              value={scope}
              onChange={(e) => setScope(e.target.value as ReportScope)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            >
              {trackId && (
                <option value="track">This track: {trackTopic}</option>
              )}
              <option value="whole-profile">
                Whole profile (all tracks)
              </option>
            </select>
          </div>

          <div className="space-y-1.5">
            <label
              htmlFor="report-learner-name"
              className="block text-xs font-medium text-foreground"
            >
              Your name on this report
            </label>
            <input
              id="report-learner-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            />
            <p className="text-xs leading-relaxed text-muted-foreground">
              This name is baked into the signed report. Edit it if it's out
              of date.
            </p>
          </div>

          <p className="text-xs leading-relaxed text-muted-foreground">
            Exports as PDF (for reading) and JSON (for verification),
            together.
          </p>

          {error && (
            <p role="alert" className="text-xs text-destructive">
              {error}
            </p>
          )}

          <div className="flex justify-end gap-2 pt-2">
            <Dialog.Close asChild>
              <button
                type="button"
                disabled={busy}
                className="rounded-lg border border-border px-4 py-2 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
              >
                Cancel
              </button>
            </Dialog.Close>
            <button
              type="button"
              onClick={handleExport}
              disabled={busy}
              className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {busy ? (
                <>
                  <Loader2 size={13} className="animate-spin" />
                  Exporting…
                </>
              ) : (
                <>
                  <Download size={13} />
                  Export report
                </>
              )}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
