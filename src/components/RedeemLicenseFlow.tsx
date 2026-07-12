// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)
//
// Phase 15 Plan 05 (D-02/D-03/D-04) — reusable staged-confirm redeem flow.
//
// Standalone component (no required props) so Phase 16's Library view can
// re-mount it verbatim. State is a discriminated union on `stage` — never
// boolean-soup — mirroring SettingsReportServerSection's submitOutcome
// union pattern. The confirm-stage dialog shell (role="dialog" aria-modal,
// fixed inset-0 z-50 bg-background/60 backdrop-blur-sm) is lifted from
// LabResetDialog.tsx, the one other confirm-dialog precedent in the
// codebase, with Confirm styled bg-primary (additive action, not a delete).
//
// Typed-error rendering (T-15-16): error copy is sourced ONLY from the
// UI-SPEC copy constants below — the thrown IPC error's raw message is
// NEVER rendered into the DOM. downloadAndImportPack is only ever invoked
// from the Confirm click handler (T-15-17 / D-03) — never auto-fired on a
// successful redeem.

import { useEffect, useRef, useState } from "react";
import { Loader2, CheckCircle2 } from "lucide-react";
import {
  redeemLicense,
  downloadAndImportPack,
  type RedeemLicenseResult,
} from "@/lib/tauri-commands";

export interface RedeemLicenseFlowProps {
  /** Called after a successful download+import — Phase 16 can react (e.g. refresh Library). */
  onImported?: (trackId: string) => void;
}

type ErrorKind =
  | "invalid_key"
  | "already_redeemed"
  | "revoked"
  | "issuer_unreachable"
  | "generic";

type Stage =
  | { kind: "entry" }
  | { kind: "validating" }
  | { kind: "confirm"; result: RedeemLicenseResult }
  | { kind: "downloading"; result: RedeemLicenseResult; phase: "download" | "import" }
  | { kind: "success" }
  | { kind: "error"; errorKind: ErrorKind };

// UI-SPEC Copywriting Contract — locked verbatim, the single source of
// truth rendered into the DOM for any thrown redeem error. Mirrors the
// RedeemLicenseError Display strings 1:1 (never a raw `.toString()`).
const ERROR_COPY: Record<ErrorKind, string> = {
  invalid_key: "This license key isn't valid. Check for typos and try again.",
  already_redeemed: "This license key has already been redeemed.",
  revoked: "This license key has been revoked.",
  issuer_unreachable:
    "Couldn't reach the license server. Check your connection and try again.",
  generic:
    "Something went wrong redeeming this key. Try again, or contact support if this keeps happening.",
};

const SUCCESS_COPY =
  "Course imported. It's now available in your track list.";

const DEVICE_FINGERPRINT_STORAGE_KEY = "learnforge.deviceFingerprint";

// Stable per-install fingerprint — analytics-only (T-15-18), NOT a
// security boundary. Any stable value is fine; persisted so repeat
// redeems from the same install share the same fingerprint.
function getOrCreateDeviceFingerprint(): string {
  try {
    const existing = window.localStorage.getItem(
      DEVICE_FINGERPRINT_STORAGE_KEY,
    );
    if (existing) return existing;
    const generated =
      typeof crypto.randomUUID === "function"
        ? crypto.randomUUID()
        : `df-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    window.localStorage.setItem(DEVICE_FINGERPRINT_STORAGE_KEY, generated);
    return generated;
  } catch {
    // localStorage unavailable (e.g. private mode) — fall back to an
    // ephemeral value; fingerprint is analytics-only so this is safe.
    return `df-ephemeral-${Date.now()}`;
  }
}

function classifyError(err: unknown): ErrorKind {
  const kind =
    err && typeof err === "object" && "kind" in err
      ? String((err as { kind: unknown }).kind)
      : undefined;
  if (
    kind === "invalid_key" ||
    kind === "already_redeemed" ||
    kind === "revoked" ||
    kind === "issuer_unreachable"
  ) {
    return kind;
  }

  const message = err instanceof Error ? err.message : String(err ?? "");
  if (/isn't valid|invalid/i.test(message)) return "invalid_key";
  if (/already been redeemed/i.test(message)) return "already_redeemed";
  if (/revoked/i.test(message)) return "revoked";
  if (/couldn't reach|unreachable|connection/i.test(message)) {
    return "issuer_unreachable";
  }
  return "generic";
}

export function RedeemLicenseFlow({ onImported }: RedeemLicenseFlowProps) {
  const [licenseKey, setLicenseKey] = useState("");
  const [stage, setStage] = useState<Stage>({ kind: "entry" });
  const keyInputRef = useRef<HTMLInputElement | null>(null);

  // License key input is the first-view focal point (UI-checker requirement).
  useEffect(() => {
    keyInputRef.current?.focus();
  }, []);

  const isSubmitting = stage.kind === "validating";
  const isDownloading = stage.kind === "downloading";

  async function handleRedeem() {
    if (!licenseKey.trim() || isSubmitting) return;
    setStage({ kind: "validating" });
    try {
      const result = await redeemLicense({
        licenseKey: licenseKey.trim(),
        deviceFingerprint: getOrCreateDeviceFingerprint(),
      });
      setStage({ kind: "confirm", result });
    } catch (err) {
      setStage({ kind: "error", errorKind: classifyError(err) });
    }
  }

  function handleCancel() {
    // Cancel returns to entry silently — discard the held result. No
    // download/import IPC is ever called from this path (D-03).
    setStage({ kind: "entry" });
  }

  async function handleConfirm() {
    if (stage.kind !== "confirm") return;
    const { result } = stage;
    setStage({ kind: "downloading", result, phase: "download" });
    try {
      // Single network+import round trip — the "Importing course…" copy
      // is shown immediately after the download call resolves-and-hands-
      // off to the backend's import step, per the UI-SPEC two-phase copy.
      const importPromise = downloadAndImportPack({
        downloadUrl: result.downloadUrl,
        packId: result.packId,
        issuerId: result.issuerId,
        issuerName: result.issuerName,
        buyerName: result.buyerName,
        orderId: result.orderId,
        redeemedAt: result.redeemedAt,
        licenseKey: licenseKey.trim(),
      });
      setStage({ kind: "downloading", result, phase: "import" });
      const imported = await importPromise;
      setStage({ kind: "success" });
      onImported?.(imported.trackId);
    } catch (err) {
      setStage({ kind: "error", errorKind: classifyError(err) });
    }
  }

  async function handleRetry() {
    await handleRedeem();
  }

  const errorKind = stage.kind === "error" ? stage.errorKind : null;

  return (
    <div className="space-y-4">
      <div>
        <label
          htmlFor="redeem-license-key"
          className="mb-1.5 block text-xs font-medium text-foreground"
        >
          License key
        </label>
        <input
          ref={keyInputRef}
          id="redeem-license-key"
          type="text"
          autoFocus
          value={licenseKey}
          onChange={(e) => setLicenseKey(e.target.value)}
          placeholder="Paste your license key"
          disabled={isSubmitting}
          className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring disabled:opacity-50"
        />
        {errorKind && (
          <div className="mt-2 space-y-1.5">
            <p
              className={
                errorKind === "issuer_unreachable"
                  ? "text-xs text-amber-500"
                  : "text-xs text-destructive"
              }
            >
              {ERROR_COPY[errorKind]}
            </p>
            {errorKind === "issuer_unreachable" && (
              <button
                type="button"
                onClick={handleRetry}
                className="rounded-lg border border-border px-4 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
              >
                Retry
              </button>
            )}
          </div>
        )}
      </div>

      <button
        type="button"
        onClick={handleRedeem}
        disabled={!licenseKey.trim() || isSubmitting}
        className="flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
      >
        {isSubmitting && <Loader2 size={12} className="animate-spin" />}
        Redeem
      </button>

      {stage.kind === "success" && (
        <div className="flex items-center gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-3">
          <CheckCircle2 size={16} className="shrink-0 text-emerald-500" />
          <p className="text-xs font-medium text-emerald-500">
            {SUCCESS_COPY}
          </p>
        </div>
      )}

      {(stage.kind === "confirm" || stage.kind === "downloading") && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="redeem-confirm-title"
          className="fixed inset-0 z-50 flex items-center justify-center bg-background/60 p-4 backdrop-blur-sm"
        >
          <div
            className="w-full max-w-md space-y-4 rounded-lg border border-border p-5 shadow-xl"
            style={{
              background: "var(--glass-bg)",
              borderColor: "var(--glass-border)",
            }}
          >
            <div>
              <h3
                id="redeem-confirm-title"
                className="text-base font-semibold text-foreground"
              >
                {stage.result.packTitle ?? stage.result.packId}
              </h3>
              <p className="mt-1 text-xs text-muted-foreground">
                Licensed to {stage.result.buyerName} · order #
                {stage.result.orderId}
              </p>
            </div>

            <p className="text-sm text-muted-foreground">
              Confirm to download and add this course to your library.
            </p>

            {stage.kind === "downloading" && (
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
                <Loader2 size={12} className="animate-spin" />
                {stage.phase === "download"
                  ? "Downloading course…"
                  : "Importing course…"}
              </div>
            )}

            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={handleCancel}
                disabled={isDownloading}
                className="rounded-lg border border-border px-4 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleConfirm}
                disabled={isDownloading}
                className="flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
              >
                {isDownloading && <Loader2 size={12} className="animate-spin" />}
                Confirm & Download
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
