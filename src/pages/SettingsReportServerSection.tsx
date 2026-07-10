// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)
//
// Phase 18 (18-06 / D-13) — Settings report-server section.
//
// Two fields — `reportServerUrl` (plain text) and `reportServerToken`
// (masked, password-style) — persisted via the same preferences_json
// json-merge pattern as SettingsLabsSection (getOrCreateProfile ->
// updateProfile({ preferencesJson })). The token is NEVER logged or
// echoed anywhere except the masked input itself.
//
// "Submit to org server" fires submit_evidence_report for the most
// recent skill report scope this section knows about (whole-profile,
// D-04 default) — it is fire-and-forget: a failed/offline POST renders
// the UI-SPEC non-blocking copy verbatim, never a modal, and never
// blocks the learner flow (D-13).

import { useEffect, useState } from "react";
import { Server, Loader2 } from "lucide-react";
import * as commands from "@/lib/tauri-commands";

interface ParsedPreferences {
  reportServerUrl?: string;
  reportServerToken?: string;
  [key: string]: unknown;
}

function parsePreferences(json: string | null | undefined): ParsedPreferences {
  if (!json) return {};
  try {
    const parsed = JSON.parse(json);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as ParsedPreferences;
    }
  } catch {
    // Treat malformed JSON as empty preferences — the next save overwrites
    // with a clean object.
  }
  return {};
}

// UI-SPEC verbatim copy (Error states table).
const NO_URL_COPY =
  "Add a report server URL in Settings to enable automatic submission.";
const SUBMIT_FAILED_COPY =
  "Couldn't reach your organization's report server. Your report was saved locally; LearnForge will retry automatically.";

export function SettingsReportServerSection() {
  const [reportServerUrl, setReportServerUrl] = useState("");
  const [reportServerToken, setReportServerToken] = useState("");
  const [learnerDisplayName, setLearnerDisplayName] = useState("");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [submitOutcome, setSubmitOutcome] = useState<
    "idle" | "accepted" | "queued" | "no-url"
  >("idle");

  // Hydrate persisted config on mount.
  useEffect(() => {
    let cancelled = false;
    async function hydrate() {
      try {
        const profile = await commands.getOrCreateProfile();
        if (cancelled) return;
        const prefs = parsePreferences(profile.preferencesJson);
        if (typeof prefs.reportServerUrl === "string") {
          setReportServerUrl(prefs.reportServerUrl);
        }
        if (typeof prefs.reportServerToken === "string") {
          setReportServerToken(prefs.reportServerToken);
        }
        if (typeof profile.displayName === "string") {
          setLearnerDisplayName(profile.displayName);
        }
      } catch (err) {
        console.error("SettingsReportServerSection: failed to load profile", err);
      }
    }
    hydrate();
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSave() {
    setSaveError(null);
    setSaved(false);
    try {
      const profile = await commands.getOrCreateProfile();
      const prefs = parsePreferences(profile.preferencesJson);
      const merged = {
        ...prefs,
        reportServerUrl: reportServerUrl.trim(),
        reportServerToken: reportServerToken,
      };
      await commands.updateProfile({
        preferencesJson: JSON.stringify(merged),
      });
      setSaved(true);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setSaveError(`Failed to save: ${msg}`);
    }
  }

  async function handleSubmitToOrg() {
    setSubmitOutcome("idle");
    if (!reportServerUrl.trim()) {
      setSubmitOutcome("no-url");
      return;
    }
    setSubmitting(true);
    try {
      // The backend fallback (assemble_report_inner) is the safety net for
      // any blank/unknown name — the primary path here always passes the
      // learner's real profile display name (CR-02 gap closure).
      const result = await commands.submitEvidenceReport({
        scope: "whole-profile",
        learnerName: learnerDisplayName,
      });
      setSubmitOutcome(result.accepted ? "accepted" : "queued");
    } catch {
      // Fire-and-forget (D-13) — any failure renders the non-blocking
      // "queued" state, never a modal, never a thrown-error UI.
      setSubmitOutcome("queued");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section className="space-y-4">
      <h2 className="text-lg font-semibold text-foreground">
        Report server
      </h2>
      <p className="text-xs text-muted-foreground">
        Optionally submit signed skill reports to your organization's report
        server.
      </p>

      <div className="glass rounded-xl p-5 space-y-4">
        <div className="flex items-start gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-secondary">
            <Server size={20} className="text-foreground" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              Organization submission
            </h3>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Submissions never block your learning flow — a failed or
              offline submission is saved locally and retried automatically.
            </p>
          </div>
        </div>

        <div>
          <label
            htmlFor="report-server-url"
            className="mb-1.5 block text-xs font-medium text-foreground"
          >
            Report server URL
          </label>
          <input
            id="report-server-url"
            type="text"
            value={reportServerUrl}
            onChange={(e) => {
              setReportServerUrl(e.target.value);
              setSaved(false);
            }}
            placeholder="https://reports.example.org"
            className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
          />
        </div>

        <div>
          <label
            htmlFor="report-server-token"
            className="mb-1.5 block text-xs font-medium text-foreground"
          >
            Report server token
          </label>
          <input
            id="report-server-token"
            type="password"
            value={reportServerToken}
            onChange={(e) => {
              setReportServerToken(e.target.value);
              setSaved(false);
            }}
            placeholder="Org-scoped auth token"
            className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
          />
          <p className="mt-1 text-[11px] text-muted-foreground">
            Your token is stored locally and never logged.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={handleSave}
            aria-label="Save report server settings"
            className="rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
          >
            Save
          </button>
          <button
            type="button"
            onClick={handleSubmitToOrg}
            disabled={submitting}
            className="flex items-center gap-1.5 rounded-lg border border-border px-4 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
          >
            {submitting && <Loader2 size={12} className="animate-spin" />}
            {submitting ? "Submitting..." : "Submit to org server"}
          </button>
          {saved && (
            <span className="text-xs font-medium text-emerald-500">
              Saved.
            </span>
          )}
        </div>

        {saveError && (
          <p className="text-xs text-destructive">{saveError}</p>
        )}

        {/* Non-blocking submission outcome — never a modal (D-13 / UI-SPEC). */}
        {submitOutcome === "no-url" && (
          <p role="status" className="text-xs text-amber-500">
            {NO_URL_COPY}
          </p>
        )}
        {submitOutcome === "queued" && (
          <p role="status" className="text-xs text-amber-500">
            {SUBMIT_FAILED_COPY}
          </p>
        )}
        {submitOutcome === "accepted" && (
          <p role="status" className="text-xs font-medium text-emerald-500">
            Submitted to your organization's report server.
          </p>
        )}
      </div>
    </section>
  );
}
